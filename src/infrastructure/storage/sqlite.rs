use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

#[cfg(unix)]
fn encode(p: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes().to_vec()
}
#[cfg(unix)]
fn decode(b: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    std::ffi::OsString::from_vec(b).into()
}
#[cfg(windows)]
fn encode(p: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    p.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}
#[cfg(windows)]
fn decode(b: Vec<u8>) -> PathBuf {
    use std::os::windows::ffi::OsStringExt;
    std::ffi::OsString::from_wide(
        &b.chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect::<Vec<_>>(),
    )
    .into()
}

pub struct Store {
    conn: Connection,
}

pub use crate::domain::{reading_position::ReadingPosition, settings::Snapshot};
impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.busy_timeout(std::time::Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        super::migrations::apply(&mut conn)?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self { conn })
    }
    pub fn setting(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key=?", [key], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())
    }
    fn set(&self, key: &str, value: &[u8]) -> Result<(), String> {
        self.conn.execute("INSERT INTO settings VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value]).map(|_| ()).map_err(|e| e.to_string())
    }
    pub fn root(&self, path: &Path) -> Result<(), String> {
        let p = path.canonicalize().map_err(|e| e.to_string())?;
        std::fs::read_dir(&p).map_err(|e| e.to_string())?;
        self.set("root", &encode(&p))
    }
    pub fn dark_theme(&self, value: bool) -> Result<(), String> {
        self.set("dark_theme", &[u8::from(value)])
    }
    pub fn dark(&self, dark: bool) -> Result<(), String> {
        self.set("dark", &[u8::from(dark)])
    }
    pub fn continuous_scroll(&self, continuous_scroll: bool) -> Result<(), String> {
        self.set("continuous_scroll", &[u8::from(continuous_scroll)])
    }
    pub fn reading_position(&self, p: &Path) -> Result<ReadingPosition, String> {
        self.conn
            .query_row(
                "SELECT page, scroll_fraction, anchor_version, zoom, horizontal_pan FROM docs WHERE path=?",
                [encode(p)],
                |row| { let page = row.get(0)?; let fraction = row.get(1)?; let version: u32 = row.get(2)?;
                    Ok(if version == 0 { ReadingPosition::new(page,fraction) } else { ReadingPosition::content(page,fraction,row.get(3)?,row.get(4)?) }) },
            )
            .optional()
            .map(|p| p.unwrap_or(ReadingPosition::new(1, 0.0)))
            .map_err(|e| e.to_string())
    }
    pub fn position(&self, p: &Path) -> Result<u32, String> {
        self.reading_position(p).map(|position| position.page)
    }
    // Explicit page navigation in single-page mode resets the within-page offset.
    pub fn save_position(&self, p: &Path, page: u32) -> Result<(), String> {
        self.save_reading_position(p, ReadingPosition::new(page, 0.0))
    }
    pub fn save_reading_position(&self, p: &Path, position: ReadingPosition) -> Result<(), String> {
        let position = position.validated();
        self.conn
            .execute(
                "INSERT INTO docs(path,page,scroll_fraction,anchor_version,zoom,horizontal_pan) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(path) DO UPDATE SET page=excluded.page, scroll_fraction=excluded.scroll_fraction, anchor_version=excluded.anchor_version, zoom=excluded.zoom, horizontal_pan=excluded.horizontal_pan",
                params![encode(p), position.page, position.fraction, position.anchor_version, position.zoom, position.horizontal],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    pub fn opened(&mut self, p: &Path, recovery: bool) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO docs(path,opened) VALUES(?1,(SELECT COALESCE(MAX(opened),0)+1 FROM docs)) ON CONFLICT(path) DO UPDATE SET opened=(SELECT COALESCE(MAX(opened),0)+1 FROM docs)", [encode(p)]).map_err(|e| e.to_string())?;
        if !recovery {
            tx.execute("INSERT INTO settings VALUES('pending',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [encode(p)]).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }
    pub fn finish_session(
        &mut self,
        position: Option<(&Path, ReadingPosition)>,
    ) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        if let Some((path, position)) = position {
            let position = position.validated();
            tx.execute("INSERT INTO docs(path,page,scroll_fraction,anchor_version,zoom,horizontal_pan) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(path) DO UPDATE SET page=excluded.page, scroll_fraction=excluded.scroll_fraction, anchor_version=excluded.anchor_version, zoom=excluded.zoom, horizontal_pan=excluded.horizontal_pan", params![encode(path), position.page, position.fraction, position.anchor_version, position.zoom, position.horizontal]).map_err(|e| e.to_string())?;
        }
        tx.execute("DELETE FROM settings WHERE key='pending'", [])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
    pub fn clear_pending(&self) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM settings WHERE key='pending'", [])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    pub fn take_pending(&mut self) -> Result<Option<PathBuf>, String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        let value: Option<Vec<u8>> = tx
            .query_row("SELECT value FROM settings WHERE key='pending'", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM settings WHERE key='pending'", [])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(value.map(decode).filter(|p| p.is_file()))
    }
    pub fn favorite(&self, p: &Path, value: bool) -> Result<(), String> {
        self.conn.execute("INSERT INTO docs(path,favorite) VALUES(?1,?2) ON CONFLICT(path) DO UPDATE SET favorite=excluded.favorite", params![encode(p),value]).map(|_| ()).map_err(|e| e.to_string())
    }
    pub fn remove_history(&self, p: &Path) -> Result<(), String> {
        self.conn
            .execute("UPDATE docs SET opened=NULL WHERE path=?", [encode(p)])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        let paths = |sql| -> Result<Vec<PathBuf>, String> {
            self.conn
                .prepare(sql)
                .map_err(|e| e.to_string())?
                .query_map([], |r| Ok(decode(r.get(0)?)))
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())
        };
        Ok(Snapshot {
            root: self.setting("root")?.map(decode),
            dark_theme: self.setting("dark_theme")? == Some(vec![1]),
            dark: self.setting("dark")? == Some(vec![1]),
            continuous_scroll: self.setting("continuous_scroll")? == Some(vec![1]),
            favorites: paths("SELECT path FROM docs WHERE favorite=1")?,
            history: paths("SELECT path FROM docs WHERE opened IS NOT NULL ORDER BY opened DESC")?,
        })
    }
}

impl crate::services::ports::LibraryRepository for Store {
    fn annotations(
        &self,
        path: &Path,
    ) -> Result<Vec<crate::domain::annotation::Annotation>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id,page,text,note,color FROM annotations WHERE path=? ORDER BY page,id",
            )
            .map_err(|e| e.to_string())?;
        let mut items = stmt
            .query_map([encode(path)], |r| {
                Ok(crate::domain::annotation::Annotation {
                    id: r.get(0)?,
                    page: r.get(1)?,
                    text: r.get(2)?,
                    note: r.get(3)?,
                    color: r.get(4)?,
                    bounds: vec![],
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for item in &mut items {
            item.bounds = self
                .conn
                .prepare(
                    "SELECT l,t,r,b FROM annotation_bounds WHERE annotation_id=? ORDER BY ordinal",
                )
                .map_err(|e| e.to_string())?
                .query_map([item.id], |r| {
                    Ok([r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?])
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
        }
        Ok(items)
    }
    fn save_annotation(
        &mut self,
        path: &Path,
        item: &crate::domain::annotation::Annotation,
    ) -> Result<(), String> {
        item.validate()?;
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        let id = if item.id == 0 {
            let changed = tx.execute("INSERT INTO annotations(path,page,text,note,color) VALUES(?1,?2,?3,?4,?5) ON CONFLICT DO NOTHING", params![encode(path),item.page,item.text,item.note,item.color]).map_err(|e|e.to_string())?;
            if changed == 0 {
                return tx.commit().map_err(|e| e.to_string());
            }
            tx.last_insert_rowid()
        } else {
            let changed = tx
                .execute(
                    "UPDATE annotations SET note=?1,color=?2 WHERE id=?3 AND path=?4",
                    params![item.note, item.color, item.id, encode(path)],
                )
                .map_err(|e| e.to_string())?;
            if changed != 1 {
                return Err("Annotation no longer exists.".into());
            }
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(());
        };
        for (index, b) in item.bounds.iter().enumerate() {
            tx.execute(
                "INSERT INTO annotation_bounds VALUES(?1,?2,?3,?4,?5,?6)",
                params![id, index as i64, b[0], b[1], b[2], b[3]],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }
    fn delete_annotation(&self, path: &Path, id: i64) -> Result<(), String> {
        self.conn
            .execute(
                "DELETE FROM annotations WHERE path=?1 AND id=?2",
                params![encode(path), id],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    fn snapshot(&self) -> Result<crate::domain::settings::Snapshot, String> {
        Store::snapshot(self)
    }
    fn take_pending(&mut self) -> Result<Option<std::path::PathBuf>, String> {
        Store::take_pending(self)
    }
    fn root(&self, path: &Path) -> Result<(), String> {
        Store::root(self, path)
    }
    fn favorite(&self, path: &Path, value: bool) -> Result<(), String> {
        Store::favorite(self, path, value)
    }
    fn remove_history(&self, path: &Path) -> Result<(), String> {
        Store::remove_history(self, path)
    }
    fn reading_position(&self, path: &Path) -> Result<ReadingPosition, String> {
        Store::reading_position(self, path)
    }
    fn opened(&mut self, path: &Path, recovery: bool) -> Result<(), String> {
        Store::opened(self, path, recovery)
    }
    fn save_reading_position(&self, path: &Path, position: ReadingPosition) -> Result<(), String> {
        Store::save_reading_position(self, path, position)
    }
    fn dark_theme(&self, value: bool) -> Result<(), String> {
        Store::dark_theme(self, value)
    }
    fn dark(&self, value: bool) -> Result<(), String> {
        Store::dark(self, value)
    }
    fn continuous_scroll(&self, value: bool) -> Result<(), String> {
        Store::continuous_scroll(self, value)
    }
    fn clear_pending(&self) -> Result<(), String> {
        Store::clear_pending(self)
    }
    fn finish_session(&mut self, position: Option<(&Path, ReadingPosition)>) -> Result<(), String> {
        Store::finish_session(self, position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scroll_anchor_survives_resize_and_clamps_invalid_input() {
        let anchor = ReadingPosition::new(12, 0.5);
        let offset = anchor.scroll_offset(800.0);
        assert!((offset - 9_200.0).abs() < 0.01);
        assert_eq!(
            ReadingPosition::from_scroll_offset(offset, 800.0, 100),
            anchor
        );
        assert_eq!(
            ReadingPosition::from_scroll_offset(anchor.scroll_offset(1600.0), 1600.0, 100),
            anchor
        );
        assert_eq!(
            ReadingPosition::new(0, f32::NAN),
            ReadingPosition::new(1, 0.0)
        );
        assert_eq!(
            ReadingPosition::from_scroll_offset(-10.0, 800.0, 10),
            ReadingPosition::new(1, 0.0)
        );
    }

    #[test]
    fn existing_databases_migrate_and_preserve_scroll_progress() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("legacy.sqlite3");
        let pdf = dir.path().join("book.pdf");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE docs(path BLOB PRIMARY KEY, page INTEGER NOT NULL DEFAULT 1,
            favorite INTEGER NOT NULL DEFAULT 0, opened INTEGER);
            CREATE TABLE settings(key TEXT PRIMARY KEY, value BLOB NOT NULL);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO docs(path,page,favorite) VALUES (?1,8,1)",
            [encode(&pdf)],
        )
        .unwrap();
        drop(conn);
        let store = Store::open(&db).unwrap();
        assert_eq!(
            store.reading_position(&pdf).unwrap(),
            ReadingPosition::new(8, 0.0)
        );
        store
            .save_reading_position(&pdf, ReadingPosition::new(8, 0.625))
            .unwrap();
        drop(store);
        let store = Store::open(&db).unwrap();
        assert_eq!(
            store.reading_position(&pdf).unwrap(),
            ReadingPosition::new(8, 0.625)
        );
        assert_eq!(
            store.snapshot().unwrap().favorites,
            std::slice::from_ref(&pdf)
        );
        store.save_position(&pdf, 9).unwrap();
        assert_eq!(
            store.reading_position(&pdf).unwrap(),
            ReadingPosition::new(9, 0.0)
        );
    }

    #[test]
    fn theme_defaults_to_light_and_persists_independently_of_dark_pages() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("theme.db");
        {
            let store = Store::open(&db).unwrap();
            assert!(!store.snapshot().unwrap().dark_theme);
            store.dark(true).unwrap();
            assert!(!store.snapshot().unwrap().dark_theme);
            store.dark_theme(true).unwrap();
        }
        {
            let store = Store::open(&db).unwrap();
            let snapshot = store.snapshot().unwrap();
            assert!(snapshot.dark_theme);
            assert!(snapshot.dark);
            store.dark_theme(false).unwrap();
        }
        let snapshot = Store::open(&db).unwrap().snapshot().unwrap();
        assert!(!snapshot.dark_theme);
        assert!(snapshot.dark);
    }

    #[test]
    fn durable_independent_state_and_mru() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("state.db");
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("b.pdf");
        std::fs::write(&a, b"source").unwrap();
        std::fs::write(&b, b"source").unwrap();
        {
            let mut s = Store::open(&db).unwrap();
            s.root(dir.path()).unwrap();
            s.opened(&a, false).unwrap();
            s.opened(&b, false).unwrap();
            s.opened(&a, false).unwrap();
            s.favorite(&a, true).unwrap();
            s.save_position(&a, 7).unwrap();
            s.save_position(&b, 0).unwrap();
            s.dark(true).unwrap();
            s.continuous_scroll(true).unwrap();
            assert_eq!(s.snapshot().unwrap().history, [a.clone(), b.clone()]);
        }
        let mut s = Store::open(&db).unwrap();
        assert_eq!(s.position(&a).unwrap(), 7);
        assert_eq!(s.position(&b).unwrap(), 1);
        assert!(s.snapshot().unwrap().dark);
        assert!(s.snapshot().unwrap().continuous_scroll);
        assert_eq!(s.take_pending().unwrap(), Some(a.clone()));
        assert_eq!(s.take_pending().unwrap(), None);
        s.remove_history(&a).unwrap();
        assert_eq!(
            s.snapshot().unwrap().favorites.as_slice(),
            std::slice::from_ref(&a)
        );
        assert_eq!(std::fs::read(&a).unwrap(), b"source");
        std::fs::remove_file(&a).unwrap();
        assert_eq!(
            s.snapshot().unwrap().favorites.as_slice(),
            std::slice::from_ref(&a)
        );
        s.opened(&a, false).unwrap();
        assert_eq!(s.take_pending().unwrap(), None);
    }
}

#[cfg(test)]
mod durability_tests {
    use super::*;
    #[test]
    fn final_anchor_and_recovery_clear_are_atomic_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.pdf");
        std::fs::write(&path, b"source").unwrap();
        let mut store = Store::open(&dir.path().join("db.sqlite3")).unwrap();
        store.opened(&path, false).unwrap();
        store.save_position(&path, 2).unwrap();
        store.conn.execute_batch("CREATE TRIGGER refuse_close BEFORE DELETE ON settings WHEN OLD.key='pending' BEGIN SELECT RAISE(FAIL,'injected write failure'); END;").unwrap();
        let final_position = ReadingPosition::content(8, 0.75, 2.0, 0.1);
        assert!(store.finish_session(Some((&path, final_position))).is_err());
        assert_eq!(
            store.reading_position(&path).unwrap(),
            ReadingPosition::new(2, 0.0)
        );
        assert!(store.setting("pending").unwrap().is_some());
        store
            .conn
            .execute_batch("DROP TRIGGER refuse_close")
            .unwrap();
        store.finish_session(Some((&path, final_position))).unwrap();
        assert_eq!(store.reading_position(&path).unwrap(), final_position);
        assert!(store.setting("pending").unwrap().is_none());
    }
    #[test]
    fn future_schema_is_refused_without_changing_its_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new.sqlite3");
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        drop(conn);
        assert!(Store::open(&path).is_err());
        let conn = Connection::open(&path).unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
                .unwrap(),
            99
        );
    }
}

#[cfg(test)]
mod annotation_tests {
    use super::*;
    use crate::{domain::annotation::Annotation, services::ports::LibraryRepository};

    #[test]
    fn annotations_survive_reopen_and_are_scoped_to_document() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("annotations.db");
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("b.pdf");
        let mut store = Store::open(&db).unwrap();
        let highlight = Annotation {
            id: 0,
            page: 2,
            text: "Text & Unicode বাংলা".into(),
            note: "First note\nSecond line".into(),
            color: Some(3),
            bounds: vec![[0.1, 0.2, 0.4, 0.3], [0.4, 0.2, 0.6, 0.3]],
        };
        store.save_annotation(&a, &highlight).unwrap();
        let bookmark = Annotation {
            id: 0,
            page: 2,
            text: String::new(),
            note: String::new(),
            color: None,
            bounds: vec![],
        };
        store.save_annotation(&a, &bookmark).unwrap();
        store.save_annotation(&a, &bookmark).unwrap();
        drop(store);
        let mut store = Store::open(&db).unwrap();
        let items = store.annotations(&a).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].bounds, highlight.bounds);
        assert_eq!(items[0].note, highlight.note);
        assert!(store.annotations(&b).unwrap().is_empty());
        let mut updated = items[0].clone();
        updated.note = "Changed".into();
        updated.color = Some(1);
        assert!(store.save_annotation(&b, &updated).is_err());
        store.save_annotation(&a, &updated).unwrap();
        store.delete_annotation(&b, updated.id).unwrap();
        assert_eq!(store.annotations(&a).unwrap()[0], updated);
        updated.note.clear();
        store.save_annotation(&a, &updated).unwrap();
        assert!(store.annotations(&a).unwrap()[0].note.is_empty());
        store.delete_annotation(&a, updated.id).unwrap();
        assert_eq!(store.annotations(&a).unwrap().len(), 1);
        assert_eq!(
            store
                .conn
                .query_row("SELECT count(*) FROM annotation_bounds", [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn failed_highlight_creation_is_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("db")).unwrap();
        store.conn.execute_batch("CREATE TRIGGER refuse_bounds BEFORE INSERT ON annotation_bounds BEGIN SELECT RAISE(FAIL,'injected failure'); END;").unwrap();
        let item = Annotation {
            id: 0,
            page: 1,
            text: "hello".into(),
            note: String::new(),
            color: Some(0),
            bounds: vec![[0.1, 0.2, 0.3, 0.4]],
        };
        assert!(store.save_annotation(Path::new("a.pdf"), &item).is_err());
        assert!(store.annotations(Path::new("a.pdf")).unwrap().is_empty());
    }
}

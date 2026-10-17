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

/// A stable position within a virtualized continuous-scrolling PDF.
/// `fraction` is measured within the page's row stride, so it survives
/// viewport resizing, display scaling and zoom changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReadingPosition {
    pub page: u32,
    pub fraction: f32,
}
impl ReadingPosition {
    pub fn new(page: u32, fraction: f32) -> Self {
        Self {
            page: page.max(1),
            fraction: if fraction.is_finite() {
                fraction.clamp(0.0, 0.999_999)
            } else {
                0.0
            },
        }
    }

    pub fn from_scroll_offset(offset: f32, row_stride: f32, total_pages: u32) -> Self {
        if !offset.is_finite() || !row_stride.is_finite() || row_stride <= 0.0 {
            return Self::new(1, 0.0);
        }
        let page_coordinate = (offset / row_stride).max(0.0);
        let max_index = total_pages.saturating_sub(1) as f32;
        let index = page_coordinate.floor().min(max_index);
        Self::new(index as u32 + 1, page_coordinate - index)
    }

    pub fn scroll_offset(self, row_stride: f32) -> f32 {
        (self.page.saturating_sub(1) as f32 + self.fraction) * row_stride
    }
}
#[derive(Default, Debug)]
pub struct Snapshot {
    pub root: Option<PathBuf>,
    pub favorites: Vec<PathBuf>,
    pub history: Vec<PathBuf>,
    pub dark: bool,
    pub continuous_scroll: bool,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.busy_timeout(std::time::Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS docs(path BLOB PRIMARY KEY, page INTEGER NOT NULL DEFAULT 1 CHECK(page>=1), favorite INTEGER NOT NULL DEFAULT 0, opened INTEGER);
            CREATE INDEX IF NOT EXISTS history_order ON docs(opened DESC);").map_err(|e| e.to_string())?;
        // Migrate existing libraries in place without altering favorites/history.
        // A fresh database has the same migration applied once.
        let has_fraction = conn
            .prepare("PRAGMA table_info(docs)")
            .map_err(|e| e.to_string())?
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
            .iter()
            .any(|name| name == "scroll_fraction");
        if !has_fraction {
            conn.execute(
                "ALTER TABLE docs ADD COLUMN scroll_fraction REAL NOT NULL DEFAULT 0.0",
                [],
            ).map_err(|e| e.to_string())?;
        }
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
    pub fn dark(&self, dark: bool) -> Result<(), String> {
        self.set("dark", &[u8::from(dark)])
    }
    pub fn continuous_scroll(&self, continuous_scroll: bool) -> Result<(), String> {
        self.set("continuous_scroll", &[u8::from(continuous_scroll)])
    }
    pub fn reading_position(&self, p: &Path) -> Result<ReadingPosition, String> {
        self.conn
            .query_row(
                "SELECT page, scroll_fraction FROM docs WHERE path=?",
                [encode(p)],
                |row| Ok(ReadingPosition::new(row.get(0)?, row.get(1)?)),
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
        let position = ReadingPosition::new(position.page, position.fraction);
        self.conn.execute(
            "INSERT INTO docs(path,page,scroll_fraction) VALUES(?1,?2,?3) \
             ON CONFLICT(path) DO UPDATE SET \
             page=excluded.page, scroll_fraction=excluded.scroll_fraction",
            params![encode(p), position.page, position.fraction],
        ).map(|_| ()).map_err(|e| e.to_string())
    }
    pub fn opened(&mut self, p: &Path, recovery: bool) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO docs(path,opened) VALUES(?1,(SELECT COALESCE(MAX(opened),0)+1 FROM docs)) ON CONFLICT(path) DO UPDATE SET opened=(SELECT COALESCE(MAX(opened),0)+1 FROM docs)", [encode(p)]).map_err(|e| e.to_string())?;
        if !recovery {
            tx.execute("INSERT INTO settings VALUES('pending',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [encode(p)]).map_err(|e| e.to_string())?;
        }
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
            dark: self.setting("dark")? == Some(vec![1]),
            continuous_scroll: self.setting("continuous_scroll")? == Some(vec![1]),
            favorites: paths("SELECT path FROM docs WHERE favorite=1")?,
            history: paths("SELECT path FROM docs WHERE opened IS NOT NULL ORDER BY opened DESC")?,
        })
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
        assert_eq!(ReadingPosition::from_scroll_offset(offset, 800.0, 100), anchor);
        assert_eq!(ReadingPosition::from_scroll_offset(anchor.scroll_offset(1600.0),
            1600.0, 100), anchor);
        assert_eq!(ReadingPosition::new(0, f32::NAN), ReadingPosition::new(1, 0.0));
        assert_eq!(ReadingPosition::from_scroll_offset(-10.0, 800.0, 10),
            ReadingPosition::new(1, 0.0));
    }

    #[test]
    fn existing_databases_migrate_and_preserve_scroll_progress() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("legacy.sqlite3");
        let pdf = dir.path().join("book.pdf");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE docs(path BLOB PRIMARY KEY, page INTEGER NOT NULL DEFAULT 1,
            favorite INTEGER NOT NULL DEFAULT 0, opened INTEGER);
            CREATE TABLE settings(key TEXT PRIMARY KEY, value BLOB NOT NULL);").unwrap();
        conn.execute("INSERT INTO docs(path,page,favorite) VALUES (?1,8,1)",
            [encode(&pdf)]).unwrap();
        drop(conn);
        let store = Store::open(&db).unwrap();
        assert_eq!(store.reading_position(&pdf).unwrap(), ReadingPosition::new(8, 0.0));
        store.save_reading_position(&pdf, ReadingPosition::new(8, 0.625)).unwrap();
        drop(store);
        let store = Store::open(&db).unwrap();
        assert_eq!(store.reading_position(&pdf).unwrap(), ReadingPosition::new(8, 0.625));
        assert_eq!(store.snapshot().unwrap().favorites, [pdf.clone()]);
        store.save_position(&pdf, 9).unwrap();
        assert_eq!(store.reading_position(&pdf).unwrap(), ReadingPosition::new(9, 0.0));
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

use rusqlite::Connection;

pub fn apply(conn: &mut Connection) -> Result<(), String> {
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if version > 3 {
        return Err("This library database requires a newer Eder version.".into());
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value BLOB NOT NULL);
        CREATE TABLE IF NOT EXISTS docs(path BLOB PRIMARY KEY, page INTEGER NOT NULL DEFAULT 1 CHECK(page>=1), favorite INTEGER NOT NULL DEFAULT 0, opened INTEGER);
        CREATE INDEX IF NOT EXISTS history_order ON docs(opened DESC);").map_err(|e| e.to_string())?;
    let columns = tx
        .prepare("PRAGMA table_info(docs)")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    for (column, definition) in [
        ("scroll_fraction", "REAL NOT NULL DEFAULT 0.0"),
        ("anchor_version", "INTEGER NOT NULL DEFAULT 0"),
        ("zoom", "REAL NOT NULL DEFAULT 1.0"),
        ("horizontal_pan", "REAL NOT NULL DEFAULT 0.0"),
    ] {
        if !columns.iter().any(|c| c == column) {
            tx.execute_batch(&format!(
                "ALTER TABLE docs ADD COLUMN {column} {definition}"
            ))
            .map_err(|e| e.to_string())?;
        }
    }
    tx.execute_batch("CREATE TABLE IF NOT EXISTS annotations(id INTEGER PRIMARY KEY, path BLOB NOT NULL, page INTEGER NOT NULL CHECK(page>=1), text TEXT NOT NULL, note TEXT NOT NULL, color INTEGER);
        CREATE INDEX IF NOT EXISTS annotations_document ON annotations(path,page);
        CREATE UNIQUE INDEX IF NOT EXISTS bookmark_page ON annotations(path,page) WHERE color IS NULL;
        CREATE TABLE IF NOT EXISTS annotation_bounds(annotation_id INTEGER NOT NULL REFERENCES annotations(id) ON DELETE CASCADE, ordinal INTEGER NOT NULL, l REAL NOT NULL, t REAL NOT NULL, r REAL NOT NULL, b REAL NOT NULL, PRIMARY KEY(annotation_id,ordinal));").map_err(|e| e.to_string())?;
    tx.pragma_update(None, "user_version", 3)
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

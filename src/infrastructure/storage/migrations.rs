use rusqlite::Connection;

pub fn apply(conn: &mut Connection) -> Result<(), String> {
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if version > 2 {
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
    tx.pragma_update(None, "user_version", 2)
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

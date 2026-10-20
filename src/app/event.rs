use crate::domain::{document::Entry, reading_position::ReadingPosition, settings::Snapshot};
use std::path::PathBuf;

pub enum Event {
    Snapshot(Snapshot),
    Rows(u64, Result<Vec<Entry>, String>),
    Collections(u64, Vec<Entry>),
    Imported(usize),
    Open(
        PathBuf,
        ReadingPosition,
        u32,
        u64,
        Vec<crate::domain::document::PageGeometry>,
        crate::domain::document::FileRevision,
    ),
    Recovery(PathBuf),
    Notice(String),
    Error(String),
    Fatal(String),
    Dark(bool),
    ReadingMode(bool),
    Closed,
    StorageCommitted,
    ReaderClosed(u64),
    ReaderCloseFailed(u64, String),
    OpenFailed(u64, String),
    ShutdownFailed(String),
}

use super::state::View;
use crate::domain::reading_position::ReadingPosition;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Task {
    SaveAnnotation(PathBuf, Box<crate::domain::annotation::Annotation>),
    DeleteAnnotation(PathBuf, i64),
    ExportAnnotations(PathBuf, PathBuf),
    Load(u64, View, Option<PathBuf>),
    Root(PathBuf),
    Import(Vec<PathBuf>, PathBuf, PathBuf),
    Favorite(PathBuf, bool),
    Remove(PathBuf),
    Open(PathBuf, bool, u64),
    Position(PathBuf, ReadingPosition, bool),
    Dark(bool),
    DarkTheme(bool),
    ReadingMode(bool),
    Close(PathBuf, ReadingPosition, u64),
    Shutdown(Option<(PathBuf, ReadingPosition)>),
    Opened(
        PathBuf,
        bool,
        u64,
        u32,
        Vec<crate::domain::document::PageGeometry>,
        crate::domain::document::FileRevision,
    ),
}

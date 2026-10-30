use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct Snapshot {
    pub root: Option<PathBuf>,
    pub favorites: Vec<PathBuf>,
    pub history: Vec<PathBuf>,
    pub dark: bool,
    pub dark_theme: bool,
    pub continuous_scroll: bool,
    pub reader_annotations_open: bool,
    pub reader_pages_open: bool,
}

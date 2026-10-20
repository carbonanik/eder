use super::reader::Reader;
use crate::domain::{document::Entry, settings::Snapshot};
use std::{collections::HashSet, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Library,
    Reading,
    Favorites,
    Settings,
}
impl View {
    pub fn title(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::Reading => "Currently Reading",
            Self::Favorites => "Favorites",
            Self::Settings => "Settings",
        }
    }
}

pub struct Controller {
    pub state: Snapshot,
    pub favorite_set: HashSet<PathBuf>,
    pub view: View,
    pub folder: Option<PathBuf>,
    pub entries: Vec<Entry>,
    pub document_count: usize,
    pub collections: Vec<Entry>,
    pub reader: Option<Reader>,
    pub listing: u64,
    pub loading: bool,
    pub opening: bool,
    pub message: Option<(String, bool)>,
    pub lifecycle: Lifecycle,
    pub open_request: u64,
}

impl Default for Controller {
    fn default() -> Self {
        Self {
            state: Snapshot::default(),
            favorite_set: HashSet::new(),
            view: View::Library,
            folder: None,
            entries: Vec::new(),
            document_count: 0,
            collections: Vec::new(),
            reader: None,
            listing: 0,
            loading: true,
            opening: false,
            message: None,
            lifecycle: Lifecycle::Starting,
            open_request: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Starting,
    Running,
    Closing,
    Closed,
    Failed,
}

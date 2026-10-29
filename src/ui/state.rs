use super::*;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::mpsc::Receiver,
};
pub(crate) struct UiState {
    pub(crate) annotations: super::annotations::AnnotationUi,
    pub(crate) logo: TextureHandle,
    pub(crate) search_query_cached: String,
    pub(crate) search_indices: Vec<usize>,
    pub(crate) search_listing: u64,
    pub(crate) search: String,
    pub(crate) list_view: bool,
    pub(crate) file_dialog: Option<Receiver<Option<Vec<PathBuf>>>>,
    pub(crate) textures: HashMap<PathBuf, Option<TextureHandle>>,
    pub(crate) lru: VecDeque<PathBuf>,
    pub(crate) pending: HashSet<PathBuf>,
    pub(crate) folder_input: String,
    pub(crate) dialog: Option<Receiver<Option<PathBuf>>>,
    pub pages: VecDeque<super::textures::CachedPage>,
    pub current_page: Option<TextureHandle>,
    pub closing_started: Option<std::time::Instant>,
    pub(crate) shortcuts_open: bool,
}

impl UiState {
    pub fn new(logo: TextureHandle) -> Self {
        Self {
            annotations: Default::default(),
            logo,
            search_query_cached: String::new(),
            search_indices: Vec::new(),
            search_listing: 0,
            search: String::new(),
            list_view: false,
            file_dialog: None,
            textures: HashMap::new(),
            lru: VecDeque::new(),
            pending: HashSet::new(),
            folder_input: String::new(),
            dialog: None,
            pages: VecDeque::new(),
            current_page: None,
            closing_started: None,
            shortcuts_open: false,
        }
    }
}

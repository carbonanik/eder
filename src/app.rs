use eframe::egui::{self, Color32, RichText, TextureHandle, Vec2};
use folio_reader::{
    library::{self, Entry},
    render,
    store::{Snapshot, Store},
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Library,
    Reading,
    Favorites,
    Settings,
}
impl View {
    fn title(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::Reading => "Currently Reading",
            Self::Favorites => "Favorites",
            Self::Settings => "Settings",
        }
    }
}
enum Task {
    Load(u64, View, Option<PathBuf>),
    Root(PathBuf),
    Favorite(PathBuf, bool),
    Remove(PathBuf),
    Open(PathBuf, bool, u64),
    Position(PathBuf, u32, bool),
    Dark(bool),
    ReadingMode(bool),
    Close,
    Shutdown,
}
enum Event {
    Snapshot(Snapshot),
    Rows(u64, Result<Vec<Entry>, String>),
    Open(PathBuf, u32, u32, u64),
    Recovery(PathBuf),
    Notice(String),
    Error(String),
    Fatal(String),
    Dark(bool),
    ReadingMode(bool),
    Closed,
}
struct PageJob {
    path: PathBuf,
    page: u32,
    total: u32,
    dark: bool,
    id: u64,
}
struct PageResult {
    id: u64,
    page: u32,
    result: Result<image::RgbaImage, String>,
    links: Vec<render::PdfLink>,
}
struct ThumbJob {
    path: PathBuf,
    id: u64,
}
struct ThumbResult {
    path: PathBuf,
    id: u64,
    image: Option<image::RgbaImage>,
}
struct Reader {
    path: PathBuf,
    page: u32,
    total: u32,
    texture: Option<TextureHandle>,
    loading: bool,
    zoom: f32,
    cache: VecDeque<(u32, bool, TextureHandle, Vec<render::PdfLink>)>,
    pending_page: Option<u32>,
    queued_pages: VecDeque<u32>,
    failed: HashSet<u32>,
    scroll_to_page: Option<u32>,
    continuous_row_height: Option<f32>,
}

pub struct Folio {
    tx: SyncSender<Task>,
    rx: Receiver<Event>,
    page_rx: Receiver<PageResult>,
    thumb_tx: SyncSender<ThumbJob>,
    thumb_rx: Receiver<ThumbResult>,
    pages: Arc<(Mutex<Option<PageJob>>, Condvar)>,
    page_gen: Arc<AtomicU64>,
    thumb_gen: Arc<AtomicU64>,
    open_gen: Arc<AtomicU64>,
    state: Snapshot,
    favorite_set: HashSet<PathBuf>,
    view: View,
    folder: Option<PathBuf>,
    entries: Vec<Entry>,
    reader: Option<Reader>,
    textures: HashMap<PathBuf, Option<TextureHandle>>,
    lru: VecDeque<PathBuf>,
    pending: HashSet<PathBuf>,
    listing: u64,
    loading: bool,
    opening: bool,
    message: Option<(String, bool)>,
    folder_input: String,
    dialog: Option<Receiver<Option<PathBuf>>>,
    closing: bool,
    closed: bool,
    fatal: bool,
}
fn data_worker(
    path: PathBuf,
    rx: Receiver<Task>,
    tx: SyncSender<Event>,
    ctx: egui::Context,
    open_gen: Arc<AtomicU64>,
) {
    let send = |event| {
        let _ = tx.send(event);
        ctx.request_repaint();
    };
    let mut store = match Store::open(&path) {
        Ok(s) => s,
        Err(e) => {
            send(Event::Fatal(format!("Cannot open local database: {e}")));
            return;
        }
    };
    match store.snapshot() {
        Ok(s) => send(Event::Snapshot(s)),
        Err(e) => send(Event::Error(e)),
    }
    match store.take_pending() {
        Ok(Some(p)) => send(Event::Recovery(p)),
        Ok(None) => {}
        Err(e) => send(Event::Error(e)),
    }
    while let Ok(task) = rx.recv() {
        let result: Result<(), String> = (|| {
            match task {
                Task::Load(id, view, folder) => {
                    let s = store.snapshot()?;
                    let rows = match view {
                        View::Library => match (s.root, folder) {
                            (Some(root), Some(folder)) => library::scan(&root, &folder),
                            _ => Ok(vec![]),
                        },
                        View::Reading => Ok(s.history.into_iter().map(Entry::file).collect()),
                        View::Favorites => {
                            let mut rows: Vec<_> =
                                s.favorites.into_iter().map(Entry::file).collect();
                            rows.sort_by_cached_key(|e| e.name.to_lowercase());
                            Ok(rows)
                        }
                        View::Settings => Ok(vec![]),
                    };
                    send(Event::Rows(id, rows));
                    return Ok(());
                }
                Task::Root(p) => {
                    store.root(&p)?;
                    send(Event::Notice("Library folder updated.".into()));
                }
                Task::Favorite(p, b) => {
                    store.favorite(&p, b)?;
                    send(Event::Notice(
                        if b {
                            "Added to Favorites."
                        } else {
                            "Removed from Favorites."
                        }
                        .into(),
                    ));
                }
                Task::Remove(p) => {
                    store.remove_history(&p)?;
                    send(Event::Notice(
                        "Removed from Currently Reading. The PDF is unchanged.".into(),
                    ));
                }
                Task::Open(p, recovery, id) => {
                    // Canonicalize so aliases do not create duplicate reading positions.
                    let p = p
                        .canonicalize()
                        .map_err(|e| format!("Cannot open PDF: {e}"))?;
                    let total = render::page_count(&p, &open_gen, id)?;
                    if open_gen.load(Ordering::Acquire) != id {
                        return Ok(());
                    }
                    let page = store.position(&p)?.clamp(1, total);
                    store.opened(&p, recovery)?;
                    store.save_position(&p, page)?;
                    send(Event::Open(p, page, total, id));
                }
                Task::Position(p, page, explicit) => {
                    store.save_position(&p, page)?;
                    if explicit {
                        send(Event::Notice(format!("Position saved: page {page}.")));
                    }
                    return Ok(());
                }
                Task::Dark(b) => {
                    store.dark(b)?;
                    send(Event::Dark(b));
                    return Ok(());
                }
                Task::ReadingMode(b) => {
                    store.continuous_scroll(b)?;
                    send(Event::ReadingMode(b));
                    return Ok(());
                }
                Task::Close => {
                    store.clear_pending()?;
                    return Ok(());
                }
                Task::Shutdown => {
                    store.clear_pending()?;
                    send(Event::Closed);
                    return Ok(());
                }
            }
            send(Event::Snapshot(store.snapshot()?));
            Ok(())
        })();
        if let Err(e) = result {
            send(Event::Error(e));
        }
    }
}
impl Folio {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(12.0, 10.0);
        style.spacing.button_padding = Vec2::new(12.0, 9.0);
        style.visuals = egui::Visuals::light();
        style.visuals.panel_fill = Color32::from_rgb(247, 248, 250);
        style.visuals.selection.bg_fill = Color32::from_rgb(209, 228, 225);
        style.visuals.selection.stroke.color = Color32::from_rgb(18, 92, 81);
        cc.egui_ctx.set_style(style);
        let dirs = directories::ProjectDirs::from("com", "Folio", "Folio");
        let mut startup_error = None;
        let data = dirs.as_ref().map(|d| d.data_local_dir().to_path_buf());
        let cache = dirs
            .as_ref()
            .map(|d| d.cache_dir().join("previews-v1"))
            .unwrap_or_else(|| std::env::temp_dir().join("folio-previews-v1"));
        let (tx, rx_task) = mpsc::sync_channel(64);
        let (event_tx, rx) = mpsc::sync_channel(64);
        let open_gen = Arc::new(AtomicU64::new(0));
        if let Some(data) = data {
            match std::fs::create_dir_all(&data) {
                Ok(()) => {
                    let ctx = cc.egui_ctx.clone();
                    let gen = open_gen.clone();
                    std::thread::spawn(move || {
                        data_worker(data.join("folio.sqlite3"), rx_task, event_tx, ctx, gen)
                    });
                }
                Err(e) => startup_error = Some(format!("Cannot create local data folder: {e}")),
            }
        } else {
            startup_error = Some("No local user-data directory is available.".into());
        }
        let page_gen = Arc::new(AtomicU64::new(0));
        let pages = Arc::new((Mutex::new(None::<PageJob>), Condvar::new()));
        let (page_tx, page_rx) = mpsc::sync_channel(1);
        {
            let slot = pages.clone();
            let gen = page_gen.clone();
            let cache = cache.clone();
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || loop {
                let job = {
                    let (lock, cv) = &*slot;
                    let mut value = lock.lock().unwrap();
                    while value.is_none() {
                        value = cv.wait(value).unwrap();
                    }
                    value.take().unwrap()
                };
                let mut result =
                    render::raster(&job.path, job.page, render::PAGE_EDGE, &cache, &gen, job.id);
                if let Ok(image) = &mut result {
                    if job.dark {
                        for pixel in image.pixels_mut() {
                            pixel[0] = 255 - pixel[0];
                            pixel[1] = 255 - pixel[1];
                            pixel[2] = 255 - pixel[2];
                        }
                    }
                }
                // Resolve clickable internal PDF index links outside the UI thread.
                // A missing pdftohtml only disables links, never the page image.
                let links = if result.is_ok() && gen.load(Ordering::Acquire) == job.id {
                    render::page_links(&job.path, job.page, job.total, &gen, job.id)
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                if gen.load(Ordering::Acquire) == job.id {
                    if page_tx
                        .send(PageResult {
                            id: job.id,
                            page: job.page,
                            result,
                            links,
                        })
                        .is_err()
                    {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
        }
        let thumb_gen = Arc::new(AtomicU64::new(0));
        let (thumb_tx, thumb_jobs) = mpsc::sync_channel::<ThumbJob>(12);
        let (thumb_results, thumb_rx) = mpsc::sync_channel(4);
        {
            let gen = thumb_gen.clone();
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                let _ = render::trim_cache(&cache, render::DISK_BUDGET);
                while let Ok(job) = thumb_jobs.recv() {
                    if gen.load(Ordering::Acquire) != job.id {
                        continue;
                    }
                    let image =
                        render::raster(&job.path, 1, render::THUMB_EDGE, &cache, &gen, job.id).ok();
                    if gen.load(Ordering::Acquire) == job.id {
                        if thumb_results
                            .send(ThumbResult {
                                path: job.path,
                                id: job.id,
                                image,
                            })
                            .is_err()
                        {
                            break;
                        }
                        ctx.request_repaint();
                    }
                }
            });
        }
        let fatal = startup_error.is_some();
        Self {
            tx,
            rx,
            page_rx,
            thumb_tx,
            thumb_rx,
            pages,
            page_gen,
            thumb_gen,
            open_gen,
            state: Snapshot::default(),
            favorite_set: HashSet::new(),
            view: View::Library,
            folder: None,
            entries: vec![],
            reader: None,
            textures: HashMap::new(),
            lru: VecDeque::new(),
            pending: HashSet::new(),
            listing: 0,
            loading: true,
            opening: false,
            message: startup_error.map(|e| (e, true)),
            folder_input: String::new(),
            dialog: None,
            closing: false,
            closed: false,
            fatal,
        }
    }
    fn send(&mut self, task: Task) -> bool {
        match self.tx.try_send(task) {
            Ok(()) => true,
            Err(e) => {
                if matches!(e, mpsc::TrySendError::Disconnected(_)) {
                    self.fatal = true;
                }
                self.message = Some((
                    format!("Action could not be queued: {e}. Please retry."),
                    true,
                ));
                false
            }
        }
    }
    fn load(&mut self) {
        self.listing += 1;
        self.thumb_gen.store(self.listing, Ordering::Release);
        self.pending.clear();
        self.loading = self.send(Task::Load(self.listing, self.view, self.folder.clone()));
        self.entries.clear();
    }
    fn open(&mut self, path: PathBuf, recovery: bool) {
        let id = self.open_gen.fetch_add(1, Ordering::AcqRel) + 1;
        self.opening = self.send(Task::Open(path, recovery, id));
        self.thumb_gen.fetch_add(1, Ordering::AcqRel);
        self.pending.clear();
    }
    fn render(&mut self) {
        if let Some(r) = &mut self.reader {
            let id = self.page_gen.fetch_add(1, Ordering::AcqRel) + 1;
            r.pending_page = None;
            r.failed.remove(&r.page);
            if let Some(index) = r
                .cache
                .iter()
                .position(|(page, dark, _, _)| *page == r.page && *dark == self.state.dark)
            {
                let cached = r.cache.remove(index).unwrap();
                r.texture = Some(cached.2.clone());
                r.cache.push_back(cached);
                r.loading = false;
                return;
            }
            r.loading = true;
            r.texture = None;
            *self.pages.0.lock().unwrap() = Some(PageJob {
                path: r.path.clone(),
                page: r.page,
                total: r.total,
                dark: self.state.dark,
                id,
            });
            self.pages.1.notify_one();
        }
    }
    fn turn(&mut self, page: u32) {
        if let Some(r) = &self.reader {
            let page = page.clamp(1, r.total);
            if page != r.page && self.send(Task::Position(r.path.clone(), page, false)) {
                self.reader.as_mut().unwrap().page = page;
                if self.state.continuous_scroll {
                    self.reader.as_mut().unwrap().scroll_to_page = Some(page);
                } else {
                    self.render();
                }
            }
        }
    }
    fn queue_continuous_pages(&mut self, visible: &[u32]) {
        let Some(reader) = &mut self.reader else {
            return;
        };
        let Some(first) = visible.iter().copied().min() else {
            return;
        };
        let last = visible.iter().copied().max().unwrap();
        let mut requested = visible.to_vec();
        for distance in 1..=2 {
            if first > distance {
                requested.push(first - distance);
            }
            if last.saturating_add(distance) <= reader.total {
                requested.push(last + distance);
            }
        }
        let desired: HashSet<_> = requested.iter().copied().collect();
        reader.queued_pages.retain(|page| desired.contains(page));
        for page in requested {
            if reader.pending_page == Some(page)
                || reader.queued_pages.contains(&page)
                || reader.failed.contains(&page)
                || reader
                    .cache
                    .iter()
                    .any(|(cached_page, dark, _, _)| *cached_page == page && *dark == self.state.dark)
            {
                continue;
            }
            reader.queued_pages.push_back(page);
        }
        self.dispatch_continuous_page();
    }
    fn dispatch_continuous_page(&mut self) {
        let id = self.page_gen.load(Ordering::Acquire);
        if let Some(reader) = &mut self.reader {
            if reader.pending_page.is_some() {
                return;
            }
            let Some(page) = reader.queued_pages.pop_front() else {
                return;
            };
            reader.pending_page = Some(page);
            *self.pages.0.lock().unwrap() = Some(PageJob {
                path: reader.path.clone(),
                page,
                total: reader.total,
                dark: self.state.dark,
                id,
            });
            self.pages.1.notify_one();
        }
    }
    fn back(&mut self) {
        if let Some(r) = &self.reader {
            if !self.send(Task::Position(r.path.clone(), r.page, false)) {
                return;
            }
            if !self.send(Task::Close) {
                return;
            }
        }
        self.page_gen.fetch_add(1, Ordering::AcqRel);
        self.reader = None;
        self.load();
    }
    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Snapshot(s) => {
                    let changed = self.state.root != s.root;
                    self.favorite_set = s.favorites.iter().cloned().collect();
                    self.state = s;
                    if changed || self.listing == 0 {
                        self.folder = self.state.root.clone();
                        self.folder_input = self
                            .folder
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                        self.load();
                    } else if self.reader.is_none()
                        && matches!(self.view, View::Reading | View::Favorites)
                    {
                        self.load();
                    }
                }
                Event::Rows(id, result) => {
                    if id == self.listing {
                        self.loading = false;
                        match result {
                            Ok(rows) => self.entries = rows,
                            Err(e) => self.message = Some((e, true)),
                        }
                    }
                }
                Event::Open(path, page, total, id) => {
                    if id == self.open_gen.load(Ordering::Acquire) {
                        self.opening = false;
                        self.reader = Some(Reader {
                            path,
                            page,
                            total,
                            texture: None,
                            loading: true,
                            zoom: 1.0,
                            cache: VecDeque::new(),
                            pending_page: None,
                            queued_pages: VecDeque::new(),
                            failed: HashSet::new(),
                            scroll_to_page: Some(page),
                            continuous_row_height: None,
                        });
                        if self.state.continuous_scroll {
                            self.reader.as_mut().unwrap().loading = false;
                        } else {
                            self.render();
                        }
                    }
                }
                Event::Recovery(p) => self.open(p, true),
                Event::Notice(s) => self.message = Some((s, false)),
                Event::Error(s) => {
                    self.message = Some((s, true));
                    self.opening = false;
                    if self.closing {
                        self.closing = false;
                    }
                }
                Event::Dark(dark) => {
                    self.state.dark = dark;
                    if self.state.continuous_scroll {
                        self.page_gen.fetch_add(1, Ordering::AcqRel);
                        if let Some(reader) = &mut self.reader {
                            reader.pending_page = None;
                            reader.queued_pages.clear();
                        }
                    } else {
                        self.render();
                    }
                }
                Event::ReadingMode(continuous_scroll) => {
                    self.state.continuous_scroll = continuous_scroll;
                }
                Event::Fatal(s) => {
                    self.fatal = true;
                    self.loading = false;
                    self.message = Some((s, true));
                }
                Event::Closed => {
                    self.closed = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        while let Ok(result) = self.page_rx.try_recv() {
            if result.id == self.page_gen.load(Ordering::Acquire) {
                if let Some(r) = &mut self.reader {
                    if r.pending_page == Some(result.page) {
                        r.pending_page = None;
                    }
                    if result.page == r.page {
                        r.loading = false;
                    }
                    match result.result {
                        Ok(image) => {
                            let t = texture(ctx, "page", image);
                            r.cache.push_back((result.page, self.state.dark, t.clone(), result.links));
                            while r
                                .cache
                                .iter()
                                .map(|(_, _, t, _)| t.size()[0] * t.size()[1] * 4)
                                .sum::<usize>()
                                > 128 * 1024 * 1024
                            {
                                r.cache.pop_front();
                            }
                            if result.page == r.page {
                                r.texture = Some(t);
                            }
                        }
                        Err(e) => {
                            r.failed.insert(result.page);
                            self.message = Some((e, true));
                        }
                    }
                }
            }
        }
        self.dispatch_continuous_page();
        while let Ok(result) = self.thumb_rx.try_recv() {
            if result.id == self.thumb_gen.load(Ordering::Acquire) {
                self.pending.remove(&result.path);
                self.touch(&result.path);
                self.textures.insert(
                    result.path,
                    result.image.map(|i| texture(ctx, "preview", i)),
                );
                while self.lru.len() > 64 {
                    if let Some(p) = self.lru.pop_front() {
                        self.textures.remove(&p);
                    }
                }
            }
        }
        if let Some(rx) = &self.dialog {
            if let Ok(p) = rx.try_recv() {
                self.dialog = None;
                if let Some(p) = p {
                    self.send(Task::Root(p));
                } else {
                    self.message=Some(("No folder selected. If your desktop does not support folder selection, use the folder path field.".into(),false));
                }
            }
        }
    }
    fn touch(&mut self, p: &PathBuf) {
        if let Some(i) = self.lru.iter().position(|x| x == p) {
            self.lru.remove(i);
        }
        self.lru.push_back(p.clone());
    }
    fn nav(&mut self, ui: &mut egui::Ui) {
        for view in [View::Library, View::Reading, View::Favorites] {
            let count = match view {
                View::Library => {
                    if self.view == View::Library {
                        Some(self.entries.len())
                    } else {
                        None
                    }
                }
                View::Reading => Some(self.state.history.len()),
                View::Favorites => Some(self.state.favorites.len()),
                _ => None,
            };
            let label = count
                .map(|n| format!("{}  {n}", view.title()))
                .unwrap_or_else(|| view.title().into());
            if ui.selectable_label(self.view == view, label).clicked() {
                self.view = view;
                self.load();
            }
        }
        if ui
            .selectable_label(self.view == View::Settings, "Settings")
            .clicked()
        {
            self.view = View::Settings;
        }
    }
    fn settings(&mut self, ui: &mut egui::Ui) {
        if ui.button("← Back to Library").clicked() {
            self.view = View::Library;
            self.load();
        }
        ui.add_space(24.0);
        ui.heading("Your library, on your device.");
        ui.label("Choose the folder containing your PDFs. Subfolders stay organized as they are.");
        ui.add_space(18.0);
        ui.label(RichText::new("Library folder").strong());
        ui.label(
            self.state
                .root
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "No folder selected".into()),
        );
        if ui
            .add_enabled(self.dialog.is_none(), egui::Button::new("Choose folder…"))
            .clicked()
        {
            let (tx, rx) = mpsc::sync_channel(1);
            self.dialog = Some(rx);
            let ctx = ui.ctx().clone();
            std::thread::spawn(move || {
                let selected = std::panic::catch_unwind(|| {
                    rfd::FileDialog::new()
                        .set_title("Choose PDF library")
                        .pick_folder()
                })
                .unwrap_or(None);
                let _ = tx.send(selected);
                ctx.request_repaint();
            });
        }
        ui.add_space(20.0);
        ui.label("If your desktop does not support the folder dialog, enter a local path:");
        ui.text_edit_singleline(&mut self.folder_input);
        if ui.button("Use folder path").clicked() {
            let p = PathBuf::from(self.folder_input.trim());
            if p.is_absolute() {
                self.send(Task::Root(p));
            } else {
                self.message = Some(("Enter an absolute folder path.".into(), true));
            }
        }
        ui.add_space(28.0);
        ui.separator();
        ui.add_space(18.0);
        ui.label(RichText::new("Reading Mode").strong());
        ui.label("Choose how pages are displayed when you open a PDF.");
        let mut continuous_scroll = self.state.continuous_scroll;
        ui.horizontal(|ui| {
            ui.radio_value(&mut continuous_scroll, false, "Single Page");
            ui.radio_value(&mut continuous_scroll, true, "Continuous Scroll");
        });
        if continuous_scroll != self.state.continuous_scroll {
            self.send(Task::ReadingMode(continuous_scroll));
        }
        ui.add_space(18.0);
        ui.separator();
        ui.label("Local storage only · No account · No document uploads");
        ui.label(
            "PDF rendering requires Poppler (pdfinfo + pdftoppm); clickable index links also require pdftohtml.",
        );
    }
    // Keep the grid responsive and virtualize rows rather than individual documents.
    fn grid_columns(width: f32) -> usize {
        const MIN_TILE_WIDTH: f32 = 176.0;
        const GAP: f32 = 14.0;
        ((width + GAP) / (MIN_TILE_WIDTH + GAP)).floor().max(1.0) as usize
    }

    fn document_tile(&mut self, ui: &mut egui::Ui, e: &Entry, width: f32) {
        const TILE_HEIGHT: f32 = 266.0;
        const PREVIEW_HEIGHT: f32 = 154.0;
        const PADDING: f32 = 12.0;

        if !e.directory && e.available {
            if self.textures.contains_key(&e.path) {
                self.touch(&e.path);
            } else if !self.pending.contains(&e.path)
                && self
                    .thumb_tx
                    .try_send(ThumbJob {
                        path: e.path.clone(),
                        id: self.thumb_gen.load(Ordering::Acquire),
                    })
                    .is_ok()
            {
                self.pending.insert(e.path.clone());
            }
        }

        ui.allocate_ui_with_layout(
            Vec2::new(width, TILE_HEIGHT),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(width);
                ui.set_min_height(TILE_HEIGHT);
                egui::Frame::none()
                    .fill(Color32::WHITE)
                    .rounding(12.0)
                    .inner_margin(PADDING)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 5.0;
                        let content_width = (width - 2.0 * PADDING).max(48.0);
                        ui.set_width(content_width);

                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(content_width, PREVIEW_HEIGHT),
                            if e.available {
                                egui::Sense::click()
                            } else {
                                egui::Sense::hover()
                            },
                        );
                        ui.painter().rect_filled(
                            rect,
                            8.0,
                            if response.hovered() && e.available {
                                Color32::from_rgb(232, 242, 238)
                            } else {
                                Color32::from_rgb(241, 245, 244)
                            },
                        );

                        if e.directory {
                            // A folder illustration takes the place of a PDF cover.
                            let center = rect.center();
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    center - Vec2::new(36.0, 25.0),
                                    Vec2::new(36.0, 14.0),
                                ),
                                4.0,
                                Color32::from_rgb(34, 129, 111),
                            );
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    center - Vec2::new(36.0, 14.0),
                                    Vec2::new(72.0, 52.0),
                                ),
                                7.0,
                                Color32::from_rgb(177, 221, 207),
                            );
                        } else if let Some(Some(texture)) = self.textures.get(&e.path) {
                            let size = texture.size_vec2();
                            let scale = ((rect.width() - 16.0) / size.x)
                                .min((rect.height() - 12.0) / size.y)
                                .min(1.0);
                            let image_rect = egui::Rect::from_center_size(rect.center(), size * scale);
                            ui.painter().image(
                                texture.id(),
                                image_rect,
                                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        } else {
                            let paper = egui::Rect::from_center_size(
                                rect.center(),
                                Vec2::new(85.0, 112.0),
                            );
                            ui.painter().rect_filled(paper, 5.0, Color32::WHITE);
                            ui.painter().text(
                                paper.center(),
                                egui::Align2::CENTER_CENTER,
                                "PDF",
                                egui::FontId::proportional(19.0),
                                Color32::from_rgb(31, 120, 101),
                            );
                        }

                        if response.on_hover_text(e.path.display().to_string()).clicked() {
                            if e.directory {
                                self.folder = Some(e.path.clone());
                                self.load();
                            } else {
                                self.open(e.path.clone(), false);
                            }
                        }

                        let label = egui::Button::new(RichText::new(&e.name).strong())
                            .frame(false)
                            .truncate();
                        if ui
                            .add_sized([content_width, 26.0], label)
                            .on_hover_text(e.path.display().to_string())
                            .clicked()
                            && e.available
                        {
                            if e.directory {
                                self.folder = Some(e.path.clone());
                                self.load();
                            } else {
                                self.open(e.path.clone(), false);
                            }
                        }

                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(if e.directory {
                                    "FOLDER"
                                } else if !e.available {
                                    "UNAVAILABLE"
                                } else {
                                    "PDF DOCUMENT"
                                })
                                .small()
                                .color(if !e.available {
                                    Color32::DARK_RED
                                } else {
                                    Color32::GRAY
                                }),
                            );
                        });

                        if !e.directory {
                            let favored = self.favorite_set.contains(&e.path);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                let remove_width = if self.view == View::Reading { 34.0 } else { 0.0 };
                                let favorite_width = (content_width - remove_width
                                    - if remove_width > 0.0 { 5.0 } else { 0.0 })
                                    .max(24.0);
                                if ui
                                    .add_sized(
                                        [favorite_width, 26.0],
                                        egui::Button::new(if favored { "★ Saved" } else { "☆ Favorite" }),
                                    )
                                    .clicked()
                                {
                                    self.send(Task::Favorite(e.path.clone(), !favored));
                                }
                                if self.view == View::Reading
                                    && ui
                                        .add_sized([remove_width, 26.0], egui::Button::new("✕"))
                                        .on_hover_text("Remove from Currently Reading (PDF stays on disk)")
                                        .clicked()
                                {
                                    self.send(Task::Remove(e.path.clone()));
                                }
                            });
                        }
                    });
            },
        );
    }

    fn browser(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.heading(self.view.title());
            if ui.button("Refresh").clicked() {
                self.textures.clear();
                self.lru.clear();
                self.load();
            }
        });
        if self.view == View::Library {
            if self.state.root.is_none() {
                ui.add_space(50.0);
                ui.heading("A quiet place for your PDFs.");
                ui.label("No library folder is configured. Open Settings to choose one.");
                if ui.button("Open Settings").clicked() {
                    self.view = View::Settings;
                }
                return;
            }
            ui.horizontal_wrapped(|ui| {
                if self.folder != self.state.root {
                    if ui.button("← Parent").clicked() {
                        if let Some(parent) = self.folder.as_ref().and_then(|p| p.parent()) {
                            self.folder = Some(parent.to_path_buf());
                            self.load();
                        }
                    }
                    if ui.button("Library root").clicked() {
                        self.folder = self.state.root.clone();
                        self.load();
                    }
                }
                if let Some(folder) = &self.folder {
                    ui.label(folder.display().to_string());
                }
            });
        }
        ui.add_space(16.0);
        if self.loading || self.opening {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(if self.opening {
                    "Opening PDF…"
                } else {
                    "Loading library…"
                });
            });
            return;
        }
        if self.entries.is_empty() {
            ui.label(match self.view {
                View::Library => "No PDFs or subfolders were found.",
                View::Favorites => "No favorites yet. Select Favorite beside a PDF to keep it here.",
                _ => "PDFs you open appear in Currently Reading.",
            });
            return;
        }

        const GAP: f32 = 14.0;
        const ROW_HEIGHT: f32 = 280.0;
        // Reserve scrollbar room so a column never gets clipped on compact windows.
        let available = (ui.available_width() - 18.0).max(1.0);
        let columns = Self::grid_columns(available);
        let tile_width = ((available - GAP * (columns - 1) as f32) / columns as f32).max(48.0);
        let rows = self.entries.len().div_ceil(columns);

        egui::ScrollArea::vertical()
            .id_salt(("library-grid", self.listing))
            .auto_shrink([false, false])
            .show_rows(ui, ROW_HEIGHT, rows, |ui, visible_rows| {
                for row_index in visible_rows {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = GAP;
                        for col in 0..columns {
                            let index = row_index * columns + col;
                            if let Some(entry) = self.entries.get(index).cloned() {
                                ui.push_id(&entry.path, |ui| {
                                    self.document_tile(ui, &entry, tile_width);
                                });
                            }
                        }
                    });
                }
            });
    }
    fn reader_ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("reader_toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("← Back").clicked() {
                    self.back();
                    return;
                }
                if let Some(r) = &self.reader {
                    ui.label(
                        RichText::new(r.path.file_name().unwrap_or_default().to_string_lossy())
                            .strong(),
                    );
                }
            });
            if self.reader.is_none() {
                return;
            }
            let (page, total, path) = {
                let r = self.reader.as_ref().unwrap();
                (r.page, r.total, r.path.clone())
            };
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(page > 1, egui::Button::new("Previous"))
                    .clicked()
                {
                    self.turn(page - 1);
                }
                let mut jump = page;
                ui.label("Page");
                if ui
                    .add(egui::DragValue::new(&mut jump).range(1..=total))
                    .changed()
                {
                    self.turn(jump);
                }
                ui.label(format!("of {total}"));
                if ui
                    .add_enabled(page < total, egui::Button::new("Next"))
                    .clicked()
                {
                    self.turn(page + 1);
                }
                if ui.button("Save Position").clicked() {
                    self.send(Task::Position(path, page, true));
                }
                let mut dark = self.state.dark;
                if ui.checkbox(&mut dark, "Dark pages").changed() {
                    self.send(Task::Dark(dark));
                }
                if let Some(r) = &mut self.reader {
                    ui.add(egui::Slider::new(&mut r.zoom, 0.5..=3.0).text("Zoom"));
                }
            });
        });
        if self.reader.is_none() {
            return;
        }
        if !ctx.wants_keyboard_input() {
            let page = self.reader.as_ref().unwrap().page;
            if ctx.input(|i| {
                i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::PageDown)
            }) {
                self.turn(page.saturating_add(1));
            }
            if ctx
                .input(|i| i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::PageUp))
            {
                self.turn(page.saturating_sub(1));
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
                self.turn(1);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::End)) {
                self.turn(self.reader.as_ref().unwrap().total);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.back();
                return;
            }
        }
        let mut internal_jump = None;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(if self.state.dark {
                        Color32::from_rgb(24, 27, 31)
                    } else {
                        Color32::from_rgb(218, 223, 226)
                    })
                    .inner_margin(18.0),
            )
            .show(ctx, |ui| {
                if self.state.continuous_scroll {
                    self.continuous_reader_ui(ui);
                    return;
                }
                let r = self.reader.as_ref().unwrap();
                if r.loading {
                    ui.centered_and_justified(|ui| {
                        ui.spinner();
                    });
                    return;
                }
                if let Some(t) = &r.texture {
                    let size = t.size_vec2();
                    let fit = (ui.available_width() / size.x)
                        .min(ui.available_height() / size.y)
                        .min(1.0);
                    let size = size * fit * r.zoom;
                    egui::ScrollArea::both()
                        .id_salt(("page", r.page))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.add_space(((ui.available_width() - size.x) / 2.0).max(0.0));
                                let image = ui.add(egui::Image::new(t).fit_to_exact_size(size));
                                if let Some((_, _, _, links)) = r.cache.iter().find(
                                    |(page, dark, _, _)| *page == r.page && *dark == self.state.dark
                                ) {
                                    internal_jump = clicked_pdf_link(ui, image.rect, r.page, links);
                                }
                            });
                        });
                } else {
                    ui.label("This page could not be rendered. Try another page or return to the library.");
                    if ui.button("Retry page").clicked() {
                        self.render();
                    }
                }
            });
        if let Some(target) = internal_jump {
            self.turn(target);
        }
    }
    fn continuous_reader_ui(&mut self, ui: &mut egui::Ui) {
        let (total, zoom) = {
            let reader = self.reader.as_ref().unwrap();
            (reader.total, reader.zoom)
        };
        let row_height = (ui.available_width() * 1.42 * zoom).max(300.0);
        let row_stride = row_height + ui.spacing().item_spacing.y;
        let reader = self.reader.as_mut().unwrap();
        let previous_row_height = reader.continuous_row_height.replace(row_stride);
        let scroll_to_page = reader.scroll_to_page.take();
        let scroll_salt = ("continuous-pages", reader.path.clone());
        let scroll_id = ui.make_persistent_id(egui::Id::new(scroll_salt.clone()));
        let previous_offset =
            egui::scroll_area::State::load(ui.ctx(), scroll_id).map(|state| state.offset.y);
        let resized_offset = previous_row_height
            .zip(previous_offset)
            .filter(|(previous, _)| (*previous - row_stride).abs() > f32::EPSILON)
            .map(|(previous, offset)| offset * row_stride / previous);
        let initial_page = scroll_to_page.map(|page| page.clamp(1, total));
        let mut first_visible = None;
        let mut visible_pages = Vec::new();
        let mut retry_page = None;
        let mut internal_jump = None;
        let mut draw_pages = |ui: &mut egui::Ui, range: std::ops::Range<usize>| {
            first_visible = range
                .start
                .checked_add(1)
                .map(|page| (page as u32).min(total));
            visible_pages.extend(range.clone().map(|index| index as u32 + 1));
            for index in range {
                let page = index as u32 + 1;
                let reader = self.reader.as_ref().unwrap();
                let texture = reader
                    .cache
                    .iter()
                    .find(|(cached_page, dark, _, _)| *cached_page == page && *dark == self.state.dark)
                    .map(|(_, _, texture, _)| texture.clone());
                let failed = reader.failed.contains(&page);
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), row_height),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.label(format!("Page {page}"));
                        if let Some(texture) = texture {
                            let available = (ui.available_width() - 24.0).max(1.0);
                            let available_height = (row_height - 42.0).max(1.0);
                            let image_size = texture.size_vec2();
                            let scale = (available * zoom / image_size.x)
                                .min(available_height / image_size.y);
                            let image_size = image_size * scale;
                            let image = ui.add(egui::Image::new(&texture).fit_to_exact_size(image_size));
                            if let Some((_, _, _, links)) = reader.cache.iter().find(
                                |(cached_page, dark, _, _)| *cached_page == page && *dark == self.state.dark
                            ) {
                                if let Some(target) = clicked_pdf_link(ui, image.rect, page, links) {
                                    internal_jump = Some(target);
                                }
                            }
                        } else if failed {
                            ui.label("This page could not be rendered.");
                            if ui.button("Retry page").clicked() {
                                retry_page = Some(page);
                            }
                        } else {
                            ui.spinner();
                        }
                    },
                );
            }
        };
        let scroll_area = egui::ScrollArea::vertical()
            .id_salt(scroll_salt)
            .auto_shrink([false, false]);
        if let Some(page) = initial_page {
            scroll_area
                .vertical_scroll_offset((page - 1) as f32 * row_stride)
                .show_rows(ui, row_height, total as usize, &mut draw_pages);
        } else if let Some(offset) = resized_offset {
            scroll_area.vertical_scroll_offset(offset).show_rows(
                ui,
                row_height,
                total as usize,
                &mut draw_pages,
            );
        } else {
            scroll_area.show_rows(ui, row_height, total as usize, &mut draw_pages);
        }
        drop(draw_pages);

        if let Some(page) = first_visible {
            let reader = self.reader.as_ref().unwrap();
            if page != reader.page && self.send(Task::Position(reader.path.clone(), page, false)) {
                self.reader.as_mut().unwrap().page = page;
            }
        }
        if let Some(page) = retry_page {
            self.reader.as_mut().unwrap().failed.remove(&page);
            visible_pages.push(page);
        }
        self.queue_continuous_pages(&visible_pages);
        if let Some(target) = internal_jump {
            self.turn(target);
        }
    }
}
/// Link overlays align Poppler's normalized text bounds to the fitted page image.
/// Only text with a real, internal PDF link is interactive; visible text is unchanged.
fn clicked_pdf_link(
    ui: &mut egui::Ui,
    image_rect: egui::Rect,
    page: u32,
    links: &[render::PdfLink],
) -> Option<u32> {
    let mut destination = None;
    for (index, link) in links.iter().enumerate() {
        let min = egui::pos2(
            image_rect.left() + link.left * image_rect.width(),
            image_rect.top() + link.top * image_rect.height(),
        );
        let max = egui::pos2(
            image_rect.left() + link.right * image_rect.width(),
            image_rect.top() + link.bottom * image_rect.height(),
        );
        let rect = egui::Rect::from_min_max(min, max)
            .expand(2.0)
            .intersect(image_rect)
            .intersect(ui.clip_rect());
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            continue;
        }
        let response = ui
            .interact(rect, ui.id().with(("pdf-link", page, index)), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!("Go to page {}", link.target_page));
        if response.hovered() {
            ui.painter().rect_filled(
                rect,
                2.0,
                Color32::from_rgba_unmultiplied(34, 129, 111, 34),
            );
        }
        if response.clicked() {
            destination = Some(link.target_page);
        }
    }
    destination
}

fn texture(ctx: &egui::Context, name: &str, image: image::RgbaImage) -> TextureHandle {
    let size = [image.width() as usize, image.height() as usize];
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
        egui::TextureOptions::LINEAR,
    )
}
impl eframe::App for Folio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.closed && !self.fatal {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if !self.closing {
                let save = if let Some(r) = &self.reader {
                    self.send(Task::Position(r.path.clone(), r.page, false))
                } else {
                    true
                };
                if save {
                    self.page_gen.fetch_add(1, Ordering::AcqRel);
                    self.thumb_gen.fetch_add(1, Ordering::AcqRel);
                    self.open_gen.fetch_add(1, Ordering::AcqRel);
                    self.closing = self.send(Task::Shutdown);
                }
            }
        }
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if let Some((message, error)) = &self.message {
                    ui.label(RichText::new(message).color(if *error {
                        Color32::DARK_RED
                    } else {
                        Color32::from_rgb(20, 100, 80)
                    }));
                    if ui.small_button("Dismiss").clicked() {
                        self.message = None;
                    }
                } else {
                    ui.label(
                        RichText::new("FOLIO   /   Your reading stays yours.")
                            .small()
                            .color(Color32::GRAY),
                    );
                }
            });
        });
        if self.reader.is_some() {
            self.reader_ui(ctx);
            return;
        }
        if ctx.screen_rect().width() > 760.0 {
            egui::SidePanel::left("sidebar")
                .exact_width(208.0)
                .show(ctx, |ui| {
                    ui.add_space(20.0);
                    ui.heading(
                        RichText::new("Folio.")
                            .size(32.0)
                            .color(Color32::from_rgb(24, 104, 91)),
                    );
                    ui.label("THE PERSONAL PDF LIBRARY");
                    ui.add_space(38.0);
                    self.nav(ui);
                });
        } else {
            egui::TopBottomPanel::top("compact_navigation").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| self.nav(ui));
            });
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(16.0);
            if self.view == View::Settings {
                egui::ScrollArea::vertical().show(ui, |ui| self.settings(ui));
            } else {
                self.browser(ui);
            }
        });
        if self.dialog.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_columns_adapt_to_window_size() {
        assert_eq!(Folio::grid_columns(280.0), 1);
        assert_eq!(Folio::grid_columns(390.0), 2);
        assert_eq!(Folio::grid_columns(850.0), 4);
    }

    #[test]
    fn compact_and_wide_layouts_virtualize_large_libraries() {
        let ctx = egui::Context::default();
        let (tx, _tasks) = mpsc::sync_channel(64);
        let (_events, rx) = mpsc::sync_channel(64);
        let (_page_tx, page_rx) = mpsc::sync_channel(1);
        let (thumb_tx, _thumb_jobs) = mpsc::sync_channel(12);
        let (_thumb_results, thumb_rx) = mpsc::sync_channel(4);
        let root = PathBuf::from("/library");
        let mut app = Folio {
            tx,
            rx,
            page_rx,
            thumb_tx,
            thumb_rx,
            pages: Arc::new((Mutex::new(None), Condvar::new())),
            page_gen: Arc::new(AtomicU64::new(0)),
            thumb_gen: Arc::new(AtomicU64::new(0)),
            open_gen: Arc::new(AtomicU64::new(0)),
            state: Snapshot {
                root: Some(root.clone()),
                ..Default::default()
            },
            favorite_set: HashSet::new(),
            view: View::Library,
            folder: Some(root.clone()),
            entries: (0..10_000)
                .map(|i| Entry {
                    path: root.join(format!("{i}.pdf")),
                    name: format!("Document {i}"),
                    directory: false,
                    available: true,
                })
                .collect(),
            reader: None,
            textures: HashMap::new(),
            lru: VecDeque::new(),
            pending: HashSet::new(),
            listing: 1,
            loading: false,
            opening: false,
            message: None,
            folder_input: String::new(),
            dialog: None,
            closing: false,
            closed: false,
            fatal: false,
        };
        for width in [360.0, 1100.0] {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(width, 760.0),
                )),
                ..Default::default()
            };
            let result = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.browser(ui));
            });
            assert!(
                result.shapes.len() < 500,
                "Only visible rows should generate shapes"
            );
            assert!(
                app.pending.len() <= 12,
                "Thumbnail queue must remain bounded"
            );
        }
        app.reader = Some(Reader {
            path: root.join("sample.pdf"),
            page: 1,
            total: 3,
            texture: None,
            loading: false,
            zoom: 1.0,
            cache: VecDeque::new(),
            pending_page: None,
            queued_pages: VecDeque::new(),
            failed: HashSet::new(),
            scroll_to_page: None,
            continuous_row_height: None,
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.reader_ui(ctx));
        app.turn(999);
        assert_eq!(app.reader.as_ref().unwrap().page, 3);
        app.turn(0);
        assert_eq!(app.reader.as_ref().unwrap().page, 1);

        app.state.continuous_scroll = true;
        let reader = app.reader.as_mut().unwrap();
        reader.total = 10_000;
        reader.scroll_to_page = Some(1);
        let result = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(360.0, 760.0),
                )),
                ..Default::default()
            },
            |ctx| app.reader_ui(ctx),
        );
        assert!(
            result.shapes.len() < 500,
            "Continuous reading should only create widgets for visible pages"
        );
        assert!(
            app.reader.as_ref().unwrap().pending_page.is_some(),
            "Visible pages should be queued for rendering"
        );
        assert!(
            app.reader.as_ref().unwrap().queued_pages.contains(&3),
            "Pages ahead of the visible range should be prefetched"
        );
        app.queue_continuous_pages(&[5_000]);
        assert!(
            app.reader.as_ref().unwrap().queued_pages.contains(&4_998),
            "Pages behind the visible range should be prefetched"
        );
        assert!(
            app.reader.as_ref().unwrap().queued_pages.contains(&5_002),
            "Pages ahead of the visible range should be prefetched"
        );
    }
}

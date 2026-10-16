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
    dark: bool,
    id: u64,
}
struct PageResult {
    id: u64,
    page: u32,
    result: Result<image::RgbaImage, String>,
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
    cache: VecDeque<(u32, bool, TextureHandle)>,
    pending_page: Option<u32>,
    failed: HashSet<u32>,
    scroll_to_page: Option<u32>,
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
                if gen.load(Ordering::Acquire) == job.id {
                    if page_tx
                        .send(PageResult {
                            id: job.id,
                            page: job.page,
                            result,
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
                .position(|(page, dark, _)| *page == r.page && *dark == self.state.dark)
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
    fn queue_continuous_page(&mut self, page: u32) {
        let id = self.page_gen.load(Ordering::Acquire);
        if let Some(reader) = &mut self.reader {
            if reader.pending_page.is_some()
                || reader.failed.contains(&page)
                || reader
                    .cache
                    .iter()
                    .any(|(cached_page, dark, _)| *cached_page == page && *dark == self.state.dark)
            {
                return;
            }
            reader.pending_page = Some(page);
            *self.pages.0.lock().unwrap() = Some(PageJob {
                path: reader.path.clone(),
                page,
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
                            failed: HashSet::new(),
                            scroll_to_page: Some(page),
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
                            r.cache.push_back((result.page, self.state.dark, t.clone()));
                            while r
                                .cache
                                .iter()
                                .map(|(_, _, t)| t.size()[0] * t.size()[1] * 4)
                                .sum::<usize>()
                                > 48 * 1024 * 1024
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
            "PDF rendering requires Poppler (pdfinfo + pdftoppm). See the included setup guide.",
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
                View::Favorites => {
                    "No favorites yet. Select Favorite beside a PDF to keep it here."
                }
                _ => "PDFs you open appear in Currently Reading.",
            });
            return;
        }
        let total = self.entries.len();
        egui::ScrollArea::vertical()
            .id_salt(("library", self.listing))
            .auto_shrink([false, false])
            .show_rows(ui, 100.0, total, |ui, range| {
                for index in range {
                    let Some(e) = self.entries.get(index).cloned() else {
                        break;
                    };
                    ui.push_id(&e.path, |ui| {
                        egui::Frame::none()
                            .fill(Color32::WHITE)
                            .rounding(10.0)
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.set_min_height(78.0);
                                ui.horizontal(|ui| {
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
                                    ui.allocate_ui(Vec2::new(56.0, 78.0), |ui| {
                                        if let Some(Some(t)) = self.textures.get(&e.path) {
                                            ui.add(
                                                egui::Image::new(t)
                                                    .fit_to_exact_size(Vec2::new(52.0, 72.0))
                                                    .maintain_aspect_ratio(true),
                                            );
                                        } else {
                                            let (rect, _) = ui.allocate_exact_size(
                                                Vec2::new(52.0, 72.0),
                                                egui::Sense::hover(),
                                            );
                                            let ink = Color32::from_rgb(29, 115, 99);
                                            if e.directory {
                                                let base = rect.min + Vec2::new(2.0, 22.0);
                                                ui.painter().rect_filled(
                                                    egui::Rect::from_min_size(
                                                        base - Vec2::new(0.0, 6.0),
                                                        Vec2::new(22.0, 12.0),
                                                    ),
                                                    3.0,
                                                    ink,
                                                );
                                                ui.painter().rect_filled(
                                                    egui::Rect::from_min_size(
                                                        base,
                                                        Vec2::new(48.0, 34.0),
                                                    ),
                                                    4.0,
                                                    Color32::from_rgb(201, 226, 219),
                                                );
                                            } else {
                                                ui.painter().rect_filled(
                                                    rect.shrink(3.0),
                                                    4.0,
                                                    Color32::from_rgb(233, 240, 237),
                                                );
                                                ui.painter().text(
                                                    rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    "PDF",
                                                    egui::FontId::proportional(16.0),
                                                    ink,
                                                );
                                            }
                                        }
                                    });
                                    ui.vertical(|ui| {
                                        ui.set_max_width((ui.available_width() - 10.0).max(100.0));
                                        let button =
                                            egui::Button::new(RichText::new(&e.name).strong())
                                                .frame(false)
                                                .truncate();
                                        if ui
                                            .add_enabled(e.available, button)
                                            .on_hover_text(e.path.display().to_string())
                                            .clicked()
                                        {
                                            if e.directory {
                                                self.folder = Some(e.path.clone());
                                                self.load();
                                            } else {
                                                self.open(e.path.clone(), false);
                                            }
                                        }
                                        if !e.available {
                                            ui.label(
                                                RichText::new(
                                                    "Unavailable — moved, deleted, or inaccessible",
                                                )
                                                .small()
                                                .color(Color32::DARK_RED),
                                            );
                                        }
                                        ui.horizontal_wrapped(|ui| {
                                            if e.directory {
                                                ui.label("Folder");
                                            } else {
                                                let favored = self.favorite_set.contains(&e.path);
                                                if ui
                                                    .small_button(if favored {
                                                        "Unfavorite"
                                                    } else {
                                                        "Favorite"
                                                    })
                                                    .clicked()
                                                {
                                                    self.send(Task::Favorite(
                                                        e.path.clone(),
                                                        !favored,
                                                    ));
                                                }
                                                if self.view == View::Reading
                                                    && ui.small_button("Remove from list").clicked()
                                                {
                                                    self.send(Task::Remove(e.path.clone()));
                                                }
                                            }
                                        });
                                    });
                                });
                            });
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
                                ui.add(egui::Image::new(t).fit_to_exact_size(size));
                            });
                        });
                } else {
                    ui.label("This page could not be rendered. Try another page or return to the library.");
                    if ui.button("Retry page").clicked() {
                        self.render();
                    }
                }
            });
    }
    fn continuous_reader_ui(&mut self, ui: &mut egui::Ui) {
        let (total, zoom) = {
            let reader = self.reader.as_ref().unwrap();
            (reader.total, reader.zoom)
        };
        let row_height = (ui.available_width() * 1.42 * zoom).max(300.0);
        let initial_page = self
            .reader
            .as_mut()
            .unwrap()
            .scroll_to_page
            .take()
            .map(|page| page.clamp(1, total));
        let mut first_visible = None;
        let mut requested_page = None;
        let mut retry_page = None;
        let mut draw_pages = |ui: &mut egui::Ui, range: std::ops::Range<usize>| {
            first_visible = range
                .start
                .checked_add(1)
                .map(|page| (page as u32).min(total));
            for index in range {
                let page = index as u32 + 1;
                let reader = self.reader.as_ref().unwrap();
                let texture = reader
                    .cache
                    .iter()
                    .find(|(cached_page, dark, _)| *cached_page == page && *dark == self.state.dark)
                    .map(|(_, _, texture)| texture.clone());
                let failed = reader.failed.contains(&page);
                if texture.is_none() && !failed && requested_page.is_none() {
                    requested_page = Some(page);
                }
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
                            ui.add(egui::Image::new(&texture).fit_to_exact_size(image_size));
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
                ui.add_space(12.0);
            }
        };
        let scroll_area = egui::ScrollArea::vertical()
            .id_salt((
                "continuous-pages",
                self.reader.as_ref().unwrap().path.clone(),
            ))
            .auto_shrink([false, false]);
        if let Some(page) = initial_page {
            scroll_area
                .vertical_scroll_offset((page - 1) as f32 * (row_height + 12.0))
                .show_rows(ui, row_height + 12.0, total as usize, &mut draw_pages);
        } else {
            scroll_area.show_rows(ui, row_height + 12.0, total as usize, &mut draw_pages);
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
            requested_page = Some(page);
        }
        if let Some(page) = requested_page {
            self.queue_continuous_page(page);
        }
    }
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
            failed: HashSet::new(),
            scroll_to_page: None,
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
    }
}

use eder::{
    library::{self, Entry},
    render,
    store::{ReadingPosition, Snapshot, Store},
};
use eframe::egui::{self, Color32, RichText, TextureHandle, Vec2};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
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
    Import(Vec<PathBuf>, PathBuf),
    Favorite(PathBuf, bool),
    Remove(PathBuf),
    Open(PathBuf, bool, u64),
    Position(PathBuf, ReadingPosition, bool),
    Dark(bool),
    ReadingMode(bool),
    Close,
    Shutdown,
}
enum Event {
    Snapshot(Snapshot),
    Rows(u64, Result<Vec<Entry>, String>),
    Collections(u64, Vec<Entry>),
    Imported(usize),
    Open(PathBuf, ReadingPosition, u32, u64),
    Recovery(PathBuf),
    Notice(String),
    Error(String),
    Fatal(String),
    Dark(bool),
    ReadingMode(bool),
    Closed,
}
// Minimal Editorial design tokens — restrained native desktop palette.
const PAPER: Color32 = Color32::from_rgb(244, 241, 233);
const INK: Color32 = Color32::from_rgb(16, 31, 36);
const DEEP: Color32 = Color32::from_rgb(9, 33, 43);
const TEAL: Color32 = Color32::from_rgb(21, 157, 153);
const MUTED: Color32 = Color32::from_rgb(104, 119, 122);
const SAGE: Color32 = Color32::from_rgb(223, 233, 228);
const LIGHT: Color32 = Color32::from_rgb(250, 249, 245);
const HAIRLINE: Color32 = Color32::from_rgb(204, 212, 207);

const PAGE_CACHE_BUDGET: usize = 160 * 1024 * 1024;
const POSITION_SAVE_DELAY: Duration = Duration::from_millis(650);

fn load_logo(ctx: &egui::Context) -> TextureHandle {
    let logo = image::load_from_memory(include_bytes!("../eder-landing/assets/eder-mark.png"))
        .expect("embedded Eder logo must be a valid PNG")
        .to_rgba8();
    let (width, height) = logo.dimensions();
    ctx.load_texture(
        "eder-logo",
        egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], logo.as_raw()),
        egui::TextureOptions::LINEAR,
    )
}

#[derive(Clone, Copy)]
enum UiIcon {
    Library,
    Clock,
    Heart,
    Settings,
    Search,
    Grid,
    List,
    Refresh,
    Home,
    ChevronRight,
    ArrowLeft,
    Plus,
    Star,
    Close,
    Folder,
}

fn paint_icon(painter: &egui::Painter, rect: egui::Rect, icon: UiIcon, color: Color32) {
    let center = rect.center();
    let half = rect.width().min(rect.height()) * 0.34;
    let stroke = egui::Stroke::new(1.8_f32, color);
    let point = |x: f32, y: f32| center + Vec2::new(x * half, y * half);
    let line = |a, b| painter.line_segment([a, b], stroke);
    match icon {
        UiIcon::Library => {
            let page = egui::Rect::from_center_size(center, Vec2::new(half * 1.65, half * 2.0));
            painter.rect_stroke(page, 1.5, stroke);
            line(point(-0.4, -0.35), point(0.4, -0.35));
            line(point(-0.4, 0.05), point(0.4, 0.05));
            line(point(-0.4, 0.45), point(0.2, 0.45));
        }
        UiIcon::Clock => {
            painter.circle_stroke(center, half * 0.9, stroke);
            line(center, point(0.0, -0.55));
            line(center, point(0.45, 0.0));
        }
        UiIcon::Heart => {
            let points = [
                point(0.0, 0.8),
                point(-0.85, 0.0),
                point(-0.85, -0.45),
                point(-0.55, -0.8),
                point(-0.2, -0.8),
                point(0.0, -0.5),
                point(0.2, -0.8),
                point(0.55, -0.8),
                point(0.85, -0.45),
                point(0.85, 0.0),
                point(0.0, 0.8),
            ];
            for segment in points.windows(2) {
                line(segment[0], segment[1]);
            }
        }
        UiIcon::Settings => {
            painter.circle_stroke(center, half * 0.72, stroke);
            painter.circle_stroke(center, half * 0.25, stroke);
            for angle in 0..8 {
                let angle = angle as f32 * std::f32::consts::TAU / 8.0;
                let direction = Vec2::new(angle.cos(), angle.sin());
                line(center + direction * half * 0.78, center + direction * half);
            }
        }
        UiIcon::Search => {
            painter.circle_stroke(
                center - Vec2::new(half * 0.18, half * 0.18),
                half * 0.58,
                stroke,
            );
            line(point(0.25, 0.25), point(0.8, 0.8));
        }
        UiIcon::Grid => {
            let cell = half * 0.72;
            for x in [-0.48, 0.48] {
                for y in [-0.48, 0.48] {
                    painter.rect_stroke(
                        egui::Rect::from_center_size(point(x, y), Vec2::splat(cell)),
                        1.5,
                        stroke,
                    );
                }
            }
        }
        UiIcon::List => {
            for y in [-0.55, 0.0, 0.55] {
                painter.circle_filled(point(-0.72, y), half * 0.09, color);
                line(point(-0.4, y), point(0.75, y));
            }
        }
        UiIcon::Refresh => {
            let points = std::array::from_fn::<_, 17, _>(|step| {
                let angle = 0.8 + step as f32 * (std::f32::consts::TAU - 1.6) / 16.0;
                point(angle.cos() * 0.78, angle.sin() * 0.78)
            });
            for segment in points.windows(2) {
                line(segment[0], segment[1]);
            }
            line(point(0.63, -0.52), point(0.78, -0.55));
            line(point(0.78, -0.55), point(0.78, -0.05));
            line(point(0.78, -0.55), point(0.28, -0.55));
        }
        UiIcon::Home => {
            line(point(-0.9, -0.05), point(0.0, -0.85));
            line(point(0.0, -0.85), point(0.9, -0.05));
            line(point(-0.65, -0.25), point(-0.65, 0.8));
            line(point(0.65, -0.25), point(0.65, 0.8));
            line(point(-0.65, 0.8), point(0.65, 0.8));
            line(point(-0.15, 0.8), point(-0.15, 0.2));
            line(point(0.15, 0.8), point(0.15, 0.2));
        }
        UiIcon::ChevronRight => {
            line(point(-0.25, -0.65), point(0.35, 0.0));
            line(point(0.35, 0.0), point(-0.25, 0.65));
        }
        UiIcon::ArrowLeft => {
            line(point(0.75, 0.0), point(-0.75, 0.0));
            line(point(-0.75, 0.0), point(-0.15, -0.6));
            line(point(-0.75, 0.0), point(-0.15, 0.6));
        }
        UiIcon::Plus => {
            line(point(-0.65, 0.0), point(0.65, 0.0));
            line(point(0.0, -0.65), point(0.0, 0.65));
        }
        UiIcon::Star => {
            let points = std::array::from_fn::<_, 11, _>(|step| {
                let angle = -std::f32::consts::FRAC_PI_2 + step as f32 * std::f32::consts::PI / 5.0;
                let radius = if step % 2 == 0 { 0.9 } else { 0.4 };
                point(angle.cos() * radius, angle.sin() * radius)
            });
            for segment in points.windows(2) {
                line(segment[0], segment[1]);
            }
        }
        UiIcon::Close => {
            line(point(-0.6, -0.6), point(0.6, 0.6));
            line(point(0.6, -0.6), point(-0.6, 0.6));
        }
        UiIcon::Folder => {
            let points = [
                point(-0.9, -0.55),
                point(-0.2, -0.55),
                point(0.0, -0.25),
                point(0.9, -0.25),
                point(0.75, 0.65),
                point(-0.75, 0.65),
                point(-0.9, -0.55),
            ];
            for segment in points.windows(2) {
                line(segment[0], segment[1]);
            }
        }
    }
}

struct IconLabelStyle {
    size: Vec2,
    fill: Color32,
    color: Color32,
    text_size: f32,
    rounding: f32,
}

fn icon_label_button(
    ui: &mut egui::Ui,
    icon: UiIcon,
    label: &str,
    style: IconLabelStyle,
) -> egui::Response {
    let response = ui.add(
        egui::Button::new(
            RichText::new(label)
                .size(style.text_size)
                .color(Color32::TRANSPARENT),
        )
        .min_size(style.size)
        .fill(style.fill)
        .rounding(style.rounding),
    );
    let icon_size = 18.0_f32.min(style.size.y * 0.55);
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(style.text_size),
        style.color,
    );
    let gap = if label.is_empty() { 0.0 } else { 8.0 };
    let group_width = icon_size + gap + galley.size().x;
    let group_left = response.rect.center().x - group_width * 0.5;
    let icon_center_x = group_left + icon_size * 0.5;
    paint_icon(
        ui.painter(),
        egui::Rect::from_center_size(
            egui::pos2(icon_center_x, response.rect.center().y),
            Vec2::splat(icon_size),
        ),
        icon,
        style.color,
    );
    if !label.is_empty() {
        ui.painter().galley(
            egui::pos2(
                group_left + icon_size + gap,
                response.rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            style.color,
        );
    }
    response
}

struct PageJob {
    path: PathBuf,
    page: u32,
    edge: u32,
    dark: bool,
    id: u64,
}
struct PageResult {
    id: u64,
    page: u32,
    edge: u32,
    dark: bool,
    // RGBA -> egui conversion is finished on the worker, not during a UI frame.
    result: Result<egui::ColorImage, String>,
}
struct LinkJob {
    path: PathBuf,
    page: u32,
    total: u32,
    id: u64,
}
struct LinkResult {
    id: u64,
    page: u32,
    links: Vec<render::PdfLink>,
}
struct CachedPage {
    page: u32,
    dark: bool,
    edge: u32,
    texture: TextureHandle,
}

struct ThumbJob {
    path: PathBuf,
    id: u64,
}
struct ThumbResult {
    path: PathBuf,
    id: u64,
    image: Option<egui::ColorImage>,
}
struct Reader {
    path: PathBuf,
    page: u32,
    total: u32,
    texture: Option<TextureHandle>,
    loading: bool,
    zoom: f32,
    cache: VecDeque<CachedPage>,
    links: HashMap<u32, Vec<render::PdfLink>>,
    link_pending: HashSet<u32>,
    target_edge: u32,
    last_visible_page: Option<u32>,
    position_changed_at: Option<Instant>,
    pending_page: Option<u32>,
    queued_pages: VecDeque<u32>,
    failed: HashSet<u32>,
    scroll_to: Option<ReadingPosition>,
    scroll_fraction: f32,
    continuous_row_height: Option<f32>,
}
impl Reader {
    fn reading_position(&self, continuous: bool) -> ReadingPosition {
        ReadingPosition::new(
            self.page,
            if continuous {
                self.scroll_fraction
            } else {
                0.0
            },
        )
    }
}

pub struct Eder {
    tx: SyncSender<Task>,
    rx: Receiver<Event>,
    page_rx: Receiver<PageResult>,
    link_tx: SyncSender<LinkJob>,
    link_rx: Receiver<LinkResult>,
    link_gen: Arc<AtomicU64>,
    thumb_tx: SyncSender<ThumbJob>,
    thumb_rx: Receiver<ThumbResult>,
    pages: Arc<(Mutex<Option<PageJob>>, Condvar)>,
    page_gen: Arc<AtomicU64>,
    thumb_gen: Arc<AtomicU64>,
    open_gen: Arc<AtomicU64>,
    logo: TextureHandle,
    state: Snapshot,
    favorite_set: HashSet<PathBuf>,
    view: View,
    folder: Option<PathBuf>,
    entries: Vec<Entry>,
    document_count: usize,
    search_query_cached: String,
    search_indices: Vec<usize>,
    search_listing: u64,
    collections: Vec<Entry>,
    search: String,
    list_view: bool,
    file_dialog: Option<Receiver<Option<Vec<PathBuf>>>>,
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
                    let root = s.root.clone();
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
                    let folders = root
                        .as_ref()
                        .and_then(|root| library::scan(root, root).ok())
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|entry| entry.directory)
                        .collect();
                    send(Event::Rows(id, rows));
                    send(Event::Collections(id, folders));
                    return Ok(());
                }
                Task::Import(sources, folder) => {
                    let root = store
                        .snapshot()?
                        .root
                        .ok_or_else(|| "Choose a library folder in Settings first.".to_owned())?;
                    let destination = library::within(&root, &folder)?;
                    let mut imported = 0_usize;
                    for source in sources {
                        if !source.is_file()
                            || !source
                                .extension()
                                .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
                        {
                            continue;
                        }
                        let Some(name) = source.file_name() else {
                            continue;
                        };
                        let stem = source.file_stem().unwrap_or_default().to_string_lossy();
                        // create_new prevents replacing any existing document.
                        for index in 0..10000 {
                            let filename = if index == 0 {
                                name.to_os_string()
                            } else {
                                format!("{stem} ({index}).pdf").into()
                            };
                            let target = destination.join(filename);
                            match std::fs::OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .open(&target)
                            {
                                Ok(mut output) => {
                                    let result = (|| -> std::io::Result<()> {
                                        let mut input = std::fs::File::open(&source)?;
                                        std::io::copy(&mut input, &mut output)?;
                                        output.sync_all()
                                    })();
                                    if let Err(error) = result {
                                        drop(output);
                                        let _ = std::fs::remove_file(&target);
                                        return Err(format!(
                                            "Could not import {}: {error}",
                                            source.display()
                                        ));
                                    }
                                    imported += 1;
                                    break;
                                }
                                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                                    continue
                                }
                                Err(error) => return Err(format!("Could not create PDF: {error}")),
                            }
                        }
                    }
                    send(Event::Imported(imported));
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
                    let saved = store.reading_position(&p)?;
                    let position = ReadingPosition::new(saved.page.clamp(1, total), saved.fraction);
                    store.opened(&p, recovery)?;
                    // Do not overwrite the scroll fraction on open: this must survive
                    // until the first restored viewport frame is shown.
                    send(Event::Open(p, position, total, id));
                }
                Task::Position(p, position, explicit) => {
                    store.save_reading_position(&p, position)?;
                    if explicit {
                        let suffix = if position.fraction > 0.001 {
                            format!(" ({:.0}% through page)", position.fraction * 100.0)
                        } else {
                            String::new()
                        };
                        send(Event::Notice(format!(
                            "Position saved: page {}{suffix}.",
                            position.page
                        )));
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
impl Eder {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(14.0, 12.0);
        style.spacing.button_padding = Vec2::new(12.0, 9.0);
        style.visuals = egui::Visuals::light();
        style.visuals.panel_fill = PAPER;
        style.visuals.window_fill = PAPER;
        style.visuals.extreme_bg_color = LIGHT;
        style.visuals.faint_bg_color = SAGE;
        style.visuals.override_text_color = Some(INK);
        style.visuals.widgets.inactive.bg_fill = SAGE;
        style.visuals.widgets.inactive.weak_bg_fill = SAGE;
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(207, 226, 218);
        style.visuals.widgets.active.bg_fill = TEAL;
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, HAIRLINE);
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, HAIRLINE);
        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, TEAL);
        style.visuals.widgets.inactive.rounding = 2.0.into();
        style.visuals.widgets.hovered.rounding = 2.0.into();
        style.visuals.widgets.active.rounding = 2.0.into();
        style.visuals.selection.bg_fill = SAGE;
        style.visuals.selection.stroke.color = TEAL;
        cc.egui_ctx.set_style(style);
        let logo = load_logo(&cc.egui_ctx);
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
        // Cache maintenance runs on its own worker so scanning disk never delays a page.
        let (cache_maint_tx, cache_maint_rx) = mpsc::sync_channel::<()>(1);
        {
            let cache = cache.clone();
            std::thread::spawn(move || {
                while cache_maint_rx.recv().is_ok() {
                    let _ = render::trim_cache(&cache, render::DISK_BUDGET);
                }
            });
        }
        let _ = cache_maint_tx.try_send(());
        {
            let slot = pages.clone();
            let gen = page_gen.clone();
            let cache = cache.clone();
            let cache_maint_tx = cache_maint_tx.clone();
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                let mut render_count = 0_usize;
                loop {
                    let job = {
                        let (lock, cv) = &*slot;
                        let mut value = lock.lock().unwrap();
                        while value.is_none() {
                            value = cv.wait(value).unwrap();
                        }
                        value.take().unwrap()
                    };
                    let mut result =
                        render::raster(&job.path, job.page, job.edge, &cache, &gen, job.id);
                    if let Ok(image) = &mut result {
                        if job.dark {
                            for pixel in image.pixels_mut() {
                                pixel[0] = 255 - pixel[0];
                                pixel[1] = 255 - pixel[1];
                                pixel[2] = 255 - pixel[2];
                            }
                        }
                    }
                    // Avoid building a multi-megabyte ColorImage on the egui thread.
                    let result = result.map(color_image);
                    if gen.load(Ordering::Acquire) == job.id {
                        if page_tx
                            .send(PageResult {
                                id: job.id,
                                page: job.page,
                                edge: job.edge,
                                dark: job.dark,
                                result,
                            })
                            .is_err()
                        {
                            break;
                        }
                        ctx.request_repaint();
                    }
                    render_count += 1;
                    if render_count.is_multiple_of(24) {
                        let _ = cache_maint_tx.try_send(());
                    }
                }
            });
        }
        // Link metadata uses a separate worker; pdftohtml must never gate an image.
        let link_gen = Arc::new(AtomicU64::new(0));
        let (link_tx, link_jobs) = mpsc::sync_channel::<LinkJob>(12);
        let (link_results, link_rx) = mpsc::sync_channel::<LinkResult>(8);
        {
            let gen = link_gen.clone();
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                while let Ok(job) = link_jobs.recv() {
                    if gen.load(Ordering::Acquire) != job.id {
                        continue;
                    }
                    let links = render::page_links(&job.path, job.page, job.total, &gen, job.id)
                        .unwrap_or_default();
                    if gen.load(Ordering::Acquire) == job.id {
                        if link_results
                            .send(LinkResult {
                                id: job.id,
                                page: job.page,
                                links,
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
        let thumb_gen = Arc::new(AtomicU64::new(0));
        let (thumb_tx, thumb_jobs) = mpsc::sync_channel::<ThumbJob>(12);
        let (thumb_results, thumb_rx) = mpsc::sync_channel(4);
        {
            let gen = thumb_gen.clone();
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                while let Ok(job) = thumb_jobs.recv() {
                    if gen.load(Ordering::Acquire) != job.id {
                        continue;
                    }
                    let image =
                        render::raster(&job.path, 1, render::THUMB_EDGE, &cache, &gen, job.id)
                            .ok()
                            .map(color_image);
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
            link_tx,
            link_rx,
            link_gen,
            thumb_tx,
            thumb_rx,
            pages,
            page_gen,
            thumb_gen,
            open_gen,
            logo,
            state: Snapshot::default(),
            favorite_set: HashSet::new(),
            view: View::Library,
            folder: None,
            entries: vec![],
            document_count: 0,
            search_query_cached: String::new(),
            search_indices: Vec::new(),
            search_listing: 0,
            collections: vec![],
            search: String::new(),
            list_view: false,
            file_dialog: None,
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
        self.document_count = 0;
        self.search_listing = 0;
    }
    fn open(&mut self, path: PathBuf, recovery: bool) {
        let id = self.open_gen.fetch_add(1, Ordering::AcqRel) + 1;
        self.opening = self.send(Task::Open(path, recovery, id));
        self.thumb_gen.fetch_add(1, Ordering::AcqRel);
        self.pending.clear();
    }
    // Select quality by physical display pixels, rather than always rasterizing 2400px.
    // Buckets prevent tiny window/zoom changes from causing a new render.
    fn page_edge(viewport: Vec2, zoom: f32, pixels_per_point: f32, continuous: bool) -> u32 {
        let long_side_points = if continuous {
            viewport.x.max(1.0) * 1.42 * zoom
        } else {
            // A fitted page is generally portrait. Account for widescreen/landscape too.
            viewport.y.max(viewport.x * 0.85) * zoom
        };
        let desired = long_side_points * pixels_per_point * 1.12;
        render::edge_bucket(desired)
    }
    fn set_page_edge(&mut self, edge: u32) {
        let Some(reader) = &mut self.reader else {
            return;
        };
        if reader.target_edge == edge {
            return;
        }
        reader.target_edge = edge;
        if self.state.continuous_scroll {
            // Keep low-resolution cached pages visible while better versions arrive.
            self.page_gen.fetch_add(1, Ordering::AcqRel);
            reader.pending_page = None;
            reader.queued_pages.clear();
        } else {
            self.render();
        }
    }
    fn render(&mut self) {
        let Some(r) = &mut self.reader else { return };
        let id = self.page_gen.fetch_add(1, Ordering::AcqRel) + 1;
        r.pending_page = None;
        r.queued_pages.clear();
        r.failed.remove(&r.page);
        let best_index = r
            .cache
            .iter()
            .enumerate()
            .filter(|(_, item)| item.page == r.page && item.dark == self.state.dark)
            .max_by_key(|(_, item)| item.edge)
            .map(|(i, _)| i);
        if let Some(index) = best_index {
            let item = r.cache.remove(index).unwrap();
            let suitable = item.edge >= r.target_edge;
            r.texture = Some(item.texture.clone());
            r.cache.push_back(item);
            if suitable {
                r.loading = false;
                return;
            }
        } else {
            r.texture = None;
        }
        // During a quality upgrade, keep displaying the already available texture.
        r.loading = r.texture.is_none();
        r.pending_page = Some(r.page);
        *self.pages.0.lock().unwrap() = Some(PageJob {
            path: r.path.clone(),
            page: r.page,
            edge: r.target_edge,
            dark: self.state.dark,
            id,
        });
        self.pages.1.notify_one();
    }
    // Explicit page navigation starts at the beginning of that page.
    fn position_changed(&mut self, page: u32) {
        if let Some(reader) = &mut self.reader {
            if page != reader.page || reader.scroll_fraction > 0.000_1 {
                reader.page = page;
                reader.scroll_fraction = 0.0;
                reader.position_changed_at = Some(Instant::now());
            }
        }
    }
    // Track the actual scroll offset, not just the first visible page.
    // Normalizing to a page fraction also survives viewport/zoom changes.
    fn continuous_position_changed(&mut self, offset: f32, row_stride: f32) {
        if let Some(reader) = &mut self.reader {
            let position = ReadingPosition::from_scroll_offset(offset, row_stride, reader.total);
            if position.page != reader.page
                || (position.fraction - reader.scroll_fraction).abs() > 0.000_1
            {
                reader.page = position.page;
                reader.scroll_fraction = position.fraction;
                reader.position_changed_at = Some(Instant::now());
            }
        }
    }
    fn flush_position_if_idle(&mut self, ctx: &egui::Context) {
        let pending = self.reader.as_ref().and_then(|reader| {
            reader.position_changed_at.map(|changed| {
                (
                    reader.path.clone(),
                    reader.reading_position(self.state.continuous_scroll),
                    changed,
                )
            })
        });
        if let Some((path, position, changed)) = pending {
            if changed.elapsed() >= POSITION_SAVE_DELAY {
                if self.send(Task::Position(path, position, false)) {
                    if let Some(reader) = &mut self.reader {
                        reader.position_changed_at = None;
                    }
                }
            } else {
                ctx.request_repaint_after(POSITION_SAVE_DELAY.saturating_sub(changed.elapsed()));
            }
        }
    }
    fn turn(&mut self, page: u32) {
        let Some(r) = &self.reader else { return };
        let page = page.clamp(1, r.total);
        if page == r.page && (!self.state.continuous_scroll || r.scroll_fraction < 0.000_1) {
            return;
        }
        self.position_changed(page);
        if self.state.continuous_scroll {
            self.reader.as_mut().unwrap().scroll_to = Some(ReadingPosition::new(page, 0.0));
        } else {
            self.render();
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
        // Cancel a render that is now far away after a fast scroll or page jump.
        if reader.pending_page.is_some_and(|pending| {
            pending.saturating_add(4) < first || pending > last.saturating_add(4)
        }) {
            self.page_gen.fetch_add(1, Ordering::AcqRel);
            reader.pending_page = None;
            reader.queued_pages.clear();
        }
        let forward = reader
            .last_visible_page
            .is_none_or(|previous| first >= previous);
        reader.last_visible_page = Some(first);
        let mut desired = visible.to_vec();
        // Prefetch in scroll direction first; never spend the pipeline on distant pages.
        for distance in 1..=3 {
            let ahead = last.saturating_add(distance);
            let behind = first.checked_sub(distance);
            let pair = if forward {
                [Some(ahead), behind]
            } else {
                [behind, Some(ahead)]
            };
            for page in pair.into_iter().flatten() {
                if (1..=reader.total).contains(&page) && !desired.contains(&page) {
                    desired.push(page);
                }
            }
        }
        reader.queued_pages.clear();
        for page in desired {
            if reader.pending_page == Some(page)
                || reader.failed.contains(&page)
                || reader.cache.iter().any(|item| {
                    item.page == page
                        && item.dark == self.state.dark
                        && item.edge >= reader.target_edge
                })
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
                edge: reader.target_edge,
                dark: self.state.dark,
                id,
            });
            self.pages.1.notify_one();
        }
    }
    fn queue_links(&mut self, page: u32) {
        let Some(r) = &mut self.reader else { return };
        if r.links.contains_key(&page) || !r.link_pending.insert(page) {
            return;
        }
        let job = LinkJob {
            path: r.path.clone(),
            page,
            total: r.total,
            id: self.link_gen.load(Ordering::Acquire),
        };
        if self.link_tx.try_send(job).is_err() {
            r.link_pending.remove(&page);
        }
    }
    fn back(&mut self) {
        if let Some(r) = &self.reader {
            if !self.send(Task::Position(
                r.path.clone(),
                r.reading_position(self.state.continuous_scroll),
                false,
            )) {
                return;
            }
            if !self.send(Task::Close) {
                return;
            }
        }
        self.page_gen.fetch_add(1, Ordering::AcqRel);
        self.link_gen.fetch_add(1, Ordering::AcqRel);
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
                            Ok(rows) => {
                                self.document_count =
                                    rows.iter().filter(|entry| !entry.directory).count();
                                self.entries = rows;
                                self.search_listing = 0;
                            }
                            Err(e) => self.message = Some((e, true)),
                        }
                    }
                }
                Event::Collections(id, folders) => {
                    if id == self.listing {
                        self.collections = folders;
                    }
                }
                Event::Imported(count) => {
                    self.message = Some((
                        if count == 0 {
                            "No PDF files were selected for import.".into()
                        } else {
                            format!(
                                "Added {count} PDF{} to your library.",
                                if count == 1 { "" } else { "s" }
                            )
                        },
                        false,
                    ));
                    if count > 0 {
                        self.load();
                    }
                }
                Event::Open(path, position, total, id) => {
                    if id == self.open_gen.load(Ordering::Acquire) {
                        self.opening = false;
                        self.page_gen.fetch_add(1, Ordering::AcqRel);
                        self.link_gen.fetch_add(1, Ordering::AcqRel);
                        self.reader = Some(Reader {
                            path,
                            page: position.page,
                            total,
                            texture: None,
                            loading: true,
                            zoom: 1.0,
                            cache: VecDeque::new(),
                            links: HashMap::new(),
                            link_pending: HashSet::new(),
                            target_edge: 1024,
                            last_visible_page: None,
                            position_changed_at: None,
                            pending_page: None,
                            queued_pages: VecDeque::new(),
                            failed: HashSet::new(),
                            scroll_to: Some(position),
                            scroll_fraction: position.fraction,
                            continuous_row_height: None,
                        });
                        if self.state.continuous_scroll {
                            self.reader.as_mut().unwrap().loading = false;
                        }
                        // The first raster starts after the reader panel measures its
                        // viewport; don't render the wrong resolution and cancel it.
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
        // At most one GPU page upload per frame, avoiding bursts during rapid scroll.
        if let Ok(result) = self.page_rx.try_recv() {
            if result.id == self.page_gen.load(Ordering::Acquire) {
                let mut load_links = false;
                if let Some(r) = &mut self.reader {
                    if r.pending_page == Some(result.page) {
                        r.pending_page = None;
                    }
                    match result.result {
                        Ok(image) => {
                            let redundant = r.cache.iter().any(|item| {
                                item.page == result.page
                                    && item.dark == result.dark
                                    && item.edge >= result.edge
                            });
                            if !redundant {
                                let t =
                                    ctx.load_texture("page", image, egui::TextureOptions::LINEAR);
                                r.cache.retain(|item| {
                                    !(item.page == result.page
                                        && item.dark == result.dark
                                        && item.edge <= result.edge)
                                });
                                r.cache.push_back(CachedPage {
                                    page: result.page,
                                    dark: result.dark,
                                    edge: result.edge,
                                    texture: t.clone(),
                                });
                                if result.page == r.page && result.dark == self.state.dark {
                                    r.texture = Some(t);
                                    r.loading = false;
                                }
                                // LRU budget; never evict the current viewport's pages.
                                while r
                                    .cache
                                    .iter()
                                    .map(|item| item.texture.size()[0] * item.texture.size()[1] * 4)
                                    .sum::<usize>()
                                    > PAGE_CACHE_BUDGET
                                {
                                    let current = r.page;
                                    let protected = r.last_visible_page.unwrap_or(current);
                                    let first = protected.saturating_sub(2);
                                    let last = protected.saturating_add(4);
                                    let evict = r.cache.iter().position(|item| {
                                        item.page != current && !(first..=last).contains(&item.page)
                                    });
                                    if let Some(index) = evict {
                                        r.cache.remove(index);
                                    } else {
                                        break;
                                    }
                                }
                            }
                            load_links = true;
                        }
                        Err(e) => {
                            r.failed.insert(result.page);
                            if result.page == r.page {
                                r.loading = false;
                                self.message = Some((e, true));
                            }
                        }
                    }
                }
                if load_links {
                    self.queue_links(result.page);
                }
            }
        }
        // Independent metadata pipeline: no raster blocks on pdftohtml.
        for _ in 0..8 {
            let Ok(result) = self.link_rx.try_recv() else {
                break;
            };
            if result.id == self.link_gen.load(Ordering::Acquire) {
                if let Some(r) = &mut self.reader {
                    r.link_pending.remove(&result.page);
                    r.links.insert(result.page, result.links);
                }
            }
        }
        self.dispatch_continuous_page();
        // Thumbnail upload is also limited to one per frame, and paused in the reader.
        if self.reader.is_none() {
            if let Ok(result) = self.thumb_rx.try_recv() {
                if result.id == self.thumb_gen.load(Ordering::Acquire) {
                    self.pending.remove(&result.path);
                    self.touch(&result.path);
                    self.textures.insert(
                        result.path,
                        result
                            .image
                            .map(|i| ctx.load_texture("preview", i, egui::TextureOptions::LINEAR)),
                    );
                    while self.lru.len() > 64 {
                        if let Some(p) = self.lru.pop_front() {
                            self.textures.remove(&p);
                        }
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
        if let Some(rx) = &self.file_dialog {
            if let Ok(picked) = rx.try_recv() {
                self.file_dialog = None;
                if let Some(paths) = picked {
                    let destination = if self.view == View::Library {
                        self.folder.clone().or_else(|| self.state.root.clone())
                    } else {
                        self.state.root.clone()
                    };
                    if let Some(destination) = destination {
                        self.send(Task::Import(paths, destination));
                    }
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
    fn change_view(&mut self, view: View) {
        if self.view != view {
            self.view = view;
            self.search.clear();
            if view != View::Settings {
                self.load();
            }
        }
    }

    fn browse_folder(&mut self, folder: PathBuf) {
        self.view = View::Library;
        self.folder = Some(folder);
        self.search.clear();
        self.load();
    }

    fn pick_pdfs(&mut self, ctx: &egui::Context) {
        if self.state.root.is_none() {
            self.change_view(View::Settings);
            self.message = Some((
                "Choose your library folder before adding a PDF.".into(),
                false,
            ));
            return;
        }
        if self.file_dialog.is_some() {
            return;
        }
        let (tx, rx) = mpsc::sync_channel(1);
        self.file_dialog = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let paths = std::panic::catch_unwind(|| {
                rfd::FileDialog::new()
                    .add_filter("PDF documents", &["pdf"])
                    .set_title("Add PDFs to Eder")
                    .pick_files()
            })
            .unwrap_or(None);
            let _ = tx.send(paths);
            ctx.request_repaint();
        });
    }

    // Editorial navigation: numeric index, no icons or decorative pills.
    fn nav_item(
        ui: &mut egui::Ui,
        title: &str,
        _icon: UiIcon,
        count: Option<usize>,
        active: bool,
        _indent: f32,
    ) -> bool {
        let width = ui.available_width().max(60.0);
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 54.0), egui::Sense::click());
        let painter = ui.painter();
        if active || response.hovered() {
            painter.rect_filled(
                rect,
                0.0,
                if active {
                    SAGE
                } else {
                    Color32::from_rgb(237, 236, 228)
                },
            );
        }
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(1.0_f32, HAIRLINE),
        );
        let number = match title {
            "Library" => "01",
            "Currently Reading" => "02",
            "Favorites" => "03",
            "Settings" => "04",
            _ => "—",
        };
        painter.text(
            rect.left_center() + Vec2::new(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            number,
            egui::FontId::proportional(10.0),
            if active { TEAL } else { MUTED },
        );
        painter.text(
            rect.left_center() + Vec2::new(43.0, 0.0),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(14.0),
            INK,
        );
        if let Some(count) = count {
            painter.text(
                rect.right_center() - Vec2::new(12.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                format!("{:02}", count),
                egui::FontId::proportional(11.0),
                MUTED,
            );
        }
        response.on_hover_text(title).clicked()
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(25.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.image((self.logo.id(), Vec2::splat(42.0)));
            ui.add_space(8.0);
            ui.label(RichText::new("Eder").size(43.0).strong().color(INK));
            ui.label(
                RichText::new(".")
                    .size(43.0)
                    .strong()
                    .color(Color32::from_rgb(240, 128, 60)),
            );
        });
        ui.label(
            RichText::new("A quieter way to read.")
                .size(12.0)
                .color(MUTED),
        );
        ui.add_space(41.0);
        // ui.label(RichText::new("YOUR WORKSPACE").size(10.0).strong().color(MUTED));
        // ui.add_space(12.0);
        let can_add = self.file_dialog.is_none();
        let button_width = ui.available_width();
        let add = ui
            .add_enabled_ui(can_add, |ui| {
                icon_label_button(
                    ui,
                    UiIcon::Plus,
                    "Add document",
                    IconLabelStyle {
                        size: Vec2::new(button_width, 44.0),
                        fill: DEEP,
                        color: LIGHT,
                        text_size: 13.0,
                        rounding: 2.0,
                    },
                )
            })
            .inner;
        if add.clicked() {
            self.pick_pdfs(ui.ctx());
        }
        ui.add_space(26.0);
        let all_count = if self.view == View::Library {
            Some(self.document_count)
        } else {
            None
        };
        for (view, icon, count) in [
            (View::Library, UiIcon::Library, all_count),
            (View::Reading, UiIcon::Clock, Some(self.state.history.len())),
            (
                View::Favorites,
                UiIcon::Heart,
                Some(self.state.favorites.len()),
            ),
        ] {
            if Self::nav_item(ui, view.title(), icon, count, self.view == view, 0.0) {
                self.change_view(view);
            }
        }
        ui.add_space((ui.available_height() - 60.0).max(18.0));
        if Self::nav_item(
            ui,
            "Settings",
            UiIcon::Settings,
            None,
            self.view == View::Settings,
            0.0,
        ) {
            self.change_view(View::Settings);
        }
        ui.add_space(22.0);
    }

    fn compact_nav(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.image((self.logo.id(), Vec2::splat(26.0)));
            ui.add_space(4.0);
            ui.label(RichText::new("Eder").size(24.0).strong().color(INK));
            ui.label(
                RichText::new(".")
                    .size(24.0)
                    .strong()
                    .color(Color32::from_rgb(240, 128, 60)),
            );
            for view in [
                View::Library,
                View::Reading,
                View::Favorites,
                View::Settings,
            ] {
                if ui
                    .selectable_label(self.view == view, view.title())
                    .clicked()
                {
                    self.change_view(view);
                }
            }
            if icon_label_button(
                ui,
                UiIcon::Plus,
                "Add PDF",
                IconLabelStyle {
                    size: Vec2::new(120.0, 30.0),
                    fill: ui.visuals().widgets.inactive.bg_fill,
                    color: ui.visuals().text_color(),
                    text_size: 14.0,
                    rounding: 4.0,
                },
            )
            .clicked()
            {
                self.pick_pdfs(ui.ctx());
            }
        });
    }

    fn refresh(&mut self) {
        self.textures.clear();
        self.lru.clear();
        self.load();
    }

    fn toolbar_search(&mut self, ui: &mut egui::Ui, width: f32) {
        egui::Frame::none()
            .fill(LIGHT)
            .rounding(2.0)
            .stroke(egui::Stroke::new(1.0_f32, HAIRLINE))
            .inner_margin(egui::Margin::symmetric(10.0, 7.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (icon_rect, _) =
                        ui.allocate_exact_size(Vec2::splat(19.0), egui::Sense::hover());
                    paint_icon(
                        ui.painter(),
                        icon_rect,
                        UiIcon::Search,
                        Color32::from_rgb(40, 66, 87),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .id(egui::Id::new("eder-search"))
                            .hint_text("Search documents...")
                            .desired_width((width - 107.0).max(75.0))
                            .frame(false),
                    );
                    if width > 230.0 {
                        ui.label(
                            RichText::new("Ctrl K")
                                .size(10.0)
                                .color(Color32::from_rgb(132, 147, 166)),
                        );
                    }
                });
            });
    }

    fn view_toggle(&mut self, ui: &mut egui::Ui) {
        let selected = DEEP;
        let unselected = SAGE;
        let dark = INK;
        if icon_label_button(
            ui,
            UiIcon::Grid,
            "",
            IconLabelStyle {
                size: Vec2::new(42.0, 38.0),
                fill: if self.list_view { unselected } else { selected },
                color: if self.list_view { dark } else { Color32::WHITE },
                text_size: 18.0,
                rounding: 8.0,
            },
        )
        .on_hover_text("Grid view")
        .clicked()
        {
            self.list_view = false;
        }
        if icon_label_button(
            ui,
            UiIcon::List,
            "",
            IconLabelStyle {
                size: Vec2::new(42.0, 38.0),
                fill: if self.list_view { selected } else { unselected },
                color: if self.list_view { Color32::WHITE } else { dark },
                text_size: 18.0,
                rounding: 8.0,
            },
        )
        .on_hover_text("List view")
        .clicked()
        {
            self.list_view = true;
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let compact = ui.available_width() < 770.0;
        ui.label(
            RichText::new("WORKSPACE / DOCUMENTS")
                .size(10.0)
                .strong()
                .color(TEAL),
        );
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(self.view.title())
                    .size(if compact { 31.0 } else { 40.0 })
                    .strong()
                    .color(INK),
            );
            if self.view != View::Settings {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(format!("{:02} ITEMS", self.document_count))
                        .size(11.0)
                        .color(MUTED),
                );
            }
        });
        if self.view != View::Settings {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let controls_width = if compact { 140.0 } else { 210.0 };
                let search_width = (ui.available_width() - controls_width).clamp(120.0, 400.0);
                self.toolbar_search(ui, search_width);
                if ui.available_width() > 90.0 {
                    self.view_toggle(ui);
                }
                if ui.available_width() > 38.0
                    && icon_label_button(
                        ui,
                        UiIcon::Refresh,
                        "",
                        IconLabelStyle {
                            size: Vec2::new(36.0, 36.0),
                            fill: SAGE,
                            color: INK,
                            text_size: 12.0,
                            rounding: 2.0,
                        },
                    )
                    .on_hover_text("Refresh documents")
                    .clicked()
                {
                    self.refresh();
                }
            });
        }
    }

    fn breadcrumbs(&mut self, ui: &mut egui::Ui) {
        if self.view != View::Library {
            return;
        }
        let Some(root) = self.state.root.clone() else {
            return;
        };
        let current = self.folder.clone().unwrap_or_else(|| root.clone());
        let mut crumbs = vec![(Some(UiIcon::Home), "Library root".to_owned(), root.clone())];
        if let Ok(relative) = current.strip_prefix(&root) {
            let mut cursor = root.clone();
            for part in relative.components() {
                cursor.push(part.as_os_str());
                crumbs.push((
                    None,
                    part.as_os_str().to_string_lossy().to_string(),
                    cursor.clone(),
                ));
            }
        }
        ui.horizontal_wrapped(|ui| {
            for (index, (icon, label, target)) in crumbs.into_iter().enumerate() {
                if index > 0 {
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(14.0, 24.0), egui::Sense::hover());
                    paint_icon(
                        ui.painter(),
                        rect,
                        UiIcon::ChevronRight,
                        Color32::from_rgb(159, 178, 196),
                    );
                }
                let clicked = if let Some(icon) = icon {
                    icon_label_button(
                        ui,
                        icon,
                        &label,
                        IconLabelStyle {
                            size: Vec2::new(116.0, 30.0),
                            fill: Color32::from_rgb(238, 243, 249),
                            color: Color32::from_rgb(57, 80, 105),
                            text_size: 12.0,
                            rounding: 7.0,
                        },
                    )
                    .clicked()
                } else {
                    ui.add(
                        egui::Button::new(
                            RichText::new(label)
                                .size(12.0)
                                .color(Color32::from_rgb(57, 80, 105)),
                        )
                        .fill(Color32::from_rgb(238, 243, 249))
                        .rounding(7.0),
                    )
                    .clicked()
                };
                if clicked && self.folder.as_ref() != Some(&target) {
                    self.browse_folder(target);
                }
            }
        });
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        let back = icon_label_button(
            ui,
            UiIcon::ArrowLeft,
            "Back to Library",
            IconLabelStyle {
                size: Vec2::new(150.0, 30.0),
                fill: ui.visuals().widgets.inactive.bg_fill,
                color: ui.visuals().text_color(),
                text_size: 14.0,
                rounding: 4.0,
            },
        );
        if back.clicked() {
            self.view = View::Library;
            self.load();
        }
        ui.add_space(24.0);
        ui.label(
            RichText::new("Your library,\non your device.")
                .size(42.0)
                .strong()
                .color(INK),
        );
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
        const MIN_TILE_WIDTH: f32 = 195.0;
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
                    .fill(LIGHT)
                    .stroke(egui::Stroke::new(1.0_f32, HAIRLINE))
                    .rounding(2.0)
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
                                Color32::from_rgb(207, 226, 218)
                            } else {
                                SAGE
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
                            let image_rect =
                                egui::Rect::from_center_size(rect.center(), size * scale);
                            ui.painter().image(
                                texture.id(),
                                image_rect,
                                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        } else {
                            let paper =
                                egui::Rect::from_center_size(rect.center(), Vec2::new(85.0, 112.0));
                            ui.painter().rect_filled(paper, 5.0, Color32::WHITE);
                            ui.painter().text(
                                paper.center(),
                                egui::Align2::CENTER_CENTER,
                                "PDF",
                                egui::FontId::proportional(19.0),
                                TEAL,
                            );
                        }

                        if response
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
                                let remove_width = if self.view == View::Reading {
                                    34.0
                                } else {
                                    0.0
                                };
                                let favorite_width = (content_width
                                    - remove_width
                                    - if remove_width > 0.0 { 5.0 } else { 0.0 })
                                .max(24.0);
                                if icon_label_button(
                                    ui,
                                    UiIcon::Star,
                                    if favored { "Saved" } else { "Favorite" },
                                    IconLabelStyle {
                                        size: Vec2::new(favorite_width, 26.0),
                                        fill: ui.visuals().widgets.inactive.bg_fill,
                                        color: ui.visuals().text_color(),
                                        text_size: 12.0,
                                        rounding: 4.0,
                                    },
                                )
                                .clicked()
                                {
                                    self.send(Task::Favorite(e.path.clone(), !favored));
                                }
                                if self.view == View::Reading
                                    && icon_label_button(
                                        ui,
                                        UiIcon::Close,
                                        "",
                                        IconLabelStyle {
                                            size: Vec2::new(remove_width, 26.0),
                                            fill: ui.visuals().widgets.inactive.bg_fill,
                                            color: ui.visuals().text_color(),
                                            text_size: 12.0,
                                            rounding: 4.0,
                                        },
                                    )
                                    .on_hover_text(
                                        "Remove from Currently Reading (PDF stays on disk)",
                                    )
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

    fn document_row(&mut self, ui: &mut egui::Ui, e: &Entry) {
        if !e.directory
            && e.available
            && !self.textures.contains_key(&e.path)
            && !self.pending.contains(&e.path)
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
        egui::Frame::none()
            .fill(LIGHT)
            .stroke(egui::Stroke::new(1.0_f32, HAIRLINE))
            .rounding(2.0)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_min_height(63.0);
                ui.horizontal(|ui| {
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(50.0, 62.0), egui::Sense::click());
                    ui.painter().rect_filled(rect, 5.0, SAGE);
                    if let Some(Some(texture)) = self.textures.get(&e.path) {
                        let size = texture.size_vec2();
                        let scale = (rect.width() / size.x).min(rect.height() / size.y).min(1.0);
                        ui.painter().image(
                            texture.id(),
                            egui::Rect::from_center_size(rect.center(), size * scale),
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        if e.directory {
                            paint_icon(ui.painter(), rect.shrink(15.0), UiIcon::Folder, TEAL);
                        } else {
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "PDF",
                                egui::FontId::proportional(15.0),
                                TEAL,
                            );
                        }
                    }
                    if response.clicked() && e.available {
                        if e.directory {
                            self.browse_folder(e.path.clone());
                        } else {
                            self.open(e.path.clone(), false);
                        }
                    }
                    ui.vertical(|ui| {
                        let name = ui.add(
                            egui::Button::new(RichText::new(&e.name).strong())
                                .frame(false)
                                .truncate(),
                        );
                        if name.on_hover_text(e.path.display().to_string()).clicked() && e.available
                        {
                            if e.directory {
                                self.browse_folder(e.path.clone());
                            } else {
                                self.open(e.path.clone(), false);
                            }
                        }
                        ui.label(
                            RichText::new(if e.directory {
                                "FOLDER"
                            } else {
                                "PDF DOCUMENT"
                            })
                            .size(11.0)
                            .color(MUTED),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !e.directory {
                            if self.view == View::Reading
                                && icon_label_button(
                                    ui,
                                    UiIcon::Close,
                                    "",
                                    IconLabelStyle {
                                        size: Vec2::new(24.0, 24.0),
                                        fill: ui.visuals().widgets.inactive.bg_fill,
                                        color: ui.visuals().text_color(),
                                        text_size: 12.0,
                                        rounding: 4.0,
                                    },
                                )
                                .on_hover_text("Remove from Currently Reading")
                                .clicked()
                            {
                                self.send(Task::Remove(e.path.clone()));
                            }
                            let favorite = self.favorite_set.contains(&e.path);
                            if icon_label_button(
                                ui,
                                UiIcon::Star,
                                if favorite { "Saved" } else { "Favorite" },
                                IconLabelStyle {
                                    size: Vec2::new(86.0, 24.0),
                                    fill: ui.visuals().widgets.inactive.bg_fill,
                                    color: ui.visuals().text_color(),
                                    text_size: 11.0,
                                    rounding: 4.0,
                                },
                            )
                            .clicked()
                            {
                                self.send(Task::Favorite(e.path.clone(), !favorite));
                            }
                        }
                    });
                });
            });
    }

    fn browser(&mut self, ui: &mut egui::Ui) {
        if self.view == View::Library && self.state.root.is_none() {
            ui.add_space(50.0);
            ui.label(
                RichText::new("A quiet place\nfor your PDFs.")
                    .size(39.0)
                    .strong()
                    .color(INK),
            );
            ui.label("No library folder is configured. Open Settings to choose one.");
            if ui.button("Open Settings").clicked() {
                self.change_view(View::Settings);
            }
            return;
        }
        self.breadcrumbs(ui);
        ui.add_space(26.0);
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
        // Only scan filenames when search text or the loaded folder changes.
        // The scroll loop itself stays virtualized and avoids allocations per frame.
        let query = self.search.trim().to_lowercase();
        if self.search_listing != self.listing || self.search_query_cached != query {
            self.search_indices = if query.is_empty() {
                Vec::new()
            } else {
                self.entries
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| entry.name.to_lowercase().contains(&query))
                    .map(|(index, _)| index)
                    .collect()
            };
            self.search_query_cached = query.clone();
            self.search_listing = self.listing;
        }
        let total = if query.is_empty() {
            self.entries.len()
        } else {
            self.search_indices.len()
        };
        if total == 0 {
            ui.label("No matching documents in this folder. Try a different search.");
            return;
        }
        if self.list_view {
            egui::ScrollArea::vertical()
                .id_salt(("library-list", self.listing))
                .auto_shrink([false, false])
                .show_rows(ui, 92.0, total, |ui, range| {
                    for index in range {
                        let actual = if query.is_empty() {
                            index
                        } else {
                            self.search_indices[index]
                        };
                        if let Some(entry) = self.entries.get(actual).cloned() {
                            ui.push_id(&entry.path, |ui| self.document_row(ui, &entry));
                        }
                    }
                });
            return;
        }

        const GAP: f32 = 18.0;
        const ROW_HEIGHT: f32 = 284.0;
        // Reserve scrollbar room so a column never gets clipped on compact windows.
        let available = (ui.available_width() - 18.0).max(1.0);
        let columns = Self::grid_columns(available);
        let tile_width = ((available - GAP * (columns - 1) as f32) / columns as f32).max(48.0);
        let rows = total.div_ceil(columns);

        egui::ScrollArea::vertical()
            .id_salt(("library-grid", self.listing))
            .auto_shrink([false, false])
            .show_rows(ui, ROW_HEIGHT, rows, |ui, visible_rows| {
                for row_index in visible_rows {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = GAP;
                        for col in 0..columns {
                            let index = row_index * columns + col;
                            let actual = if query.is_empty() {
                                index
                            } else {
                                *self.search_indices.get(index).unwrap_or(&usize::MAX)
                            };
                            if let Some(entry) = self.entries.get(actual).cloned() {
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
        egui::TopBottomPanel::top("reader_toolbar")
            .frame(
                egui::Frame::none()
                    .fill(PAPER)
                    .inner_margin(egui::Margin::symmetric(20.0, 12.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if icon_label_button(
                        ui,
                        UiIcon::ArrowLeft,
                        "Back",
                        IconLabelStyle {
                            size: Vec2::new(78.0, 28.0),
                            fill: ui.visuals().widgets.inactive.bg_fill,
                            color: ui.visuals().text_color(),
                            text_size: 14.0,
                            rounding: 4.0,
                        },
                    )
                    .clicked()
                    {
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
                let (page, total, path, position) = {
                    let r = self.reader.as_ref().unwrap();
                    (
                        r.page,
                        r.total,
                        r.path.clone(),
                        r.reading_position(self.state.continuous_scroll),
                    )
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
                    if ui.button("Save Position").clicked()
                        && self.send(Task::Position(path, position, true))
                    {
                        if let Some(r) = &mut self.reader {
                            r.position_changed_at = None;
                        }
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
                        SAGE
                    })
                    .inner_margin(18.0),
            )
            .show(ctx, |ui| {
                if self.state.continuous_scroll {
                    self.continuous_reader_ui(ui);
                    return;
                }
                let desired = Self::page_edge(ui.available_size(),
                    self.reader.as_ref().unwrap().zoom, ui.ctx().pixels_per_point(), false);
                self.set_page_edge(desired);
                let r = self.reader.as_ref().unwrap();
                if r.loading && r.pending_page.is_none() && r.texture.is_none() {
                    self.render();
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
                                if let Some(links) = r.links.get(&r.page) {
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
        let desired = Self::page_edge(ui.available_size(), zoom, ui.ctx().pixels_per_point(), true);
        self.set_page_edge(desired);
        let row_height = (ui.available_width() * 1.42 * zoom).max(300.0);
        let row_stride = row_height + ui.spacing().item_spacing.y;
        let reader = self.reader.as_mut().unwrap();
        let previous_row_height = reader.continuous_row_height.replace(row_stride);
        let scroll_to = reader.scroll_to.take();
        let scroll_salt = ("continuous-pages", reader.path.clone());
        let scroll_id = ui.make_persistent_id(egui::Id::new(scroll_salt.clone()));
        let previous_offset =
            egui::scroll_area::State::load(ui.ctx(), scroll_id).map(|state| state.offset.y);
        let resized_offset = previous_row_height
            .zip(previous_offset)
            .filter(|(previous, _)| (*previous - row_stride).abs() > f32::EPSILON)
            .map(|(previous, offset)| offset * row_stride / previous);
        let initial_anchor = scroll_to
            .map(|anchor| ReadingPosition::new(anchor.page.clamp(1, total), anchor.fraction));
        let mut visible_pages = Vec::new();
        let mut retry_page = None;
        let mut internal_jump = None;
        let mut draw_pages = |ui: &mut egui::Ui, range: std::ops::Range<usize>| {
            visible_pages.extend(range.clone().map(|index| index as u32 + 1));
            for index in range {
                let page = index as u32 + 1;
                let reader = self.reader.as_ref().unwrap();
                let texture = reader
                    .cache
                    .iter()
                    .filter(|item| item.page == page && item.dark == self.state.dark)
                    .max_by_key(|item| item.edge)
                    .map(|item| item.texture.clone());
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
                            let image =
                                ui.add(egui::Image::new(&texture).fit_to_exact_size(image_size));
                            if let Some(links) = reader.links.get(&page) {
                                if let Some(target) = clicked_pdf_link(ui, image.rect, page, links)
                                {
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
        let result = if let Some(anchor) = initial_anchor {
            // Restore the exact viewport location, including a partial page.
            scroll_area
                .vertical_scroll_offset(anchor.scroll_offset(row_stride))
                .show_rows(ui, row_height, total as usize, &mut draw_pages)
        } else if let Some(offset) = resized_offset {
            scroll_area.vertical_scroll_offset(offset).show_rows(
                ui,
                row_height,
                total as usize,
                &mut draw_pages,
            )
        } else {
            scroll_area.show_rows(ui, row_height, total as usize, &mut draw_pages)
        };
        let actual_offset = result.state.offset.y;
        drop(draw_pages);

        self.continuous_position_changed(actual_offset, row_stride);
        if let Some(page) = retry_page {
            self.reader.as_mut().unwrap().failed.remove(&page);
            visible_pages.push(page);
        }
        // Touch visible textures to keep actually-used pages at the back of the LRU.
        if let Some(reader) = &mut self.reader {
            for &page in &visible_pages {
                if let Some(index) = reader
                    .cache
                    .iter()
                    .position(|item| item.page == page && item.dark == self.state.dark)
                {
                    let item = reader.cache.remove(index).unwrap();
                    reader.cache.push_back(item);
                }
            }
        }
        // A full link-worker queue must not permanently lose link metadata.
        // Retry it opportunistically for already-rendered visible pages.
        for &page in &visible_pages {
            if self
                .reader
                .as_ref()
                .unwrap()
                .cache
                .iter()
                .any(|item| item.page == page)
            {
                self.queue_links(page);
            }
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
            .interact(
                rect,
                ui.id().with(("pdf-link", page, index)),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!("Go to page {}", link.target_page));
        if response.hovered() {
            ui.painter()
                .rect_filled(rect, 2.0, Color32::from_rgba_unmultiplied(34, 129, 111, 34));
        }
        if response.clicked() {
            destination = Some(link.target_page);
        }
    }
    destination
}

fn color_image(image: image::RgbaImage) -> egui::ColorImage {
    let size = [image.width() as usize, image.height() as usize];
    egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw())
}
impl eframe::App for Eder {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.closed && !self.fatal {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if !self.closing {
                let save = if let Some(r) = &self.reader {
                    self.send(Task::Position(
                        r.path.clone(),
                        r.reading_position(self.state.continuous_scroll),
                        false,
                    ))
                } else {
                    true
                };
                if save {
                    self.page_gen.fetch_add(1, Ordering::AcqRel);
                    self.link_gen.fetch_add(1, Ordering::AcqRel);
                    self.thumb_gen.fetch_add(1, Ordering::AcqRel);
                    self.open_gen.fetch_add(1, Ordering::AcqRel);
                    self.closing = self.send(Task::Shutdown);
                }
            }
        }
        if self.message.is_some() {
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
                    }
                });
            });
        }
        if self.reader.is_some() {
            self.reader_ui(ctx);
            // Preserve the optimized reader and precise continuous-scroll saving.
            self.flush_position_if_idle(ctx);
            return;
        }
        if ctx.input(|input| {
            (input.modifiers.command || input.modifiers.ctrl) && input.key_pressed(egui::Key::K)
        }) {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("eder-search")));
        }
        if ctx.screen_rect().width() > 820.0 {
            egui::SidePanel::left("sidebar")
                .exact_width(265.0)
                .frame(
                    egui::Frame::none()
                        .fill(PAPER)
                        .inner_margin(egui::Margin::symmetric(23.0, 8.0)),
                )
                .show(ctx, |ui| self.sidebar(ui));
        } else {
            egui::TopBottomPanel::top("compact_navigation").show(ctx, |ui| self.compact_nav(ui));
        }
        let available = ctx.available_rect().width();
        let header_height = if self.view == View::Settings {
            103.0
        } else if available < 770.0 {
            157.0
        } else {
            151.0
        };
        egui::TopBottomPanel::top("library_toolbar")
            .exact_height(header_height)
            .frame(
                egui::Frame::none()
                    .fill(PAPER)
                    .inner_margin(egui::Margin::symmetric(27.0, 13.0)),
            )
            .show(ctx, |ui| self.toolbar(ui));
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(PAPER)
                    .inner_margin(egui::Margin::symmetric(27.0, 20.0)),
            )
            .show(ctx, |ui| {
                if self.view == View::Settings {
                    egui::ScrollArea::vertical().show(ui, |ui| self.settings(ui));
                } else {
                    self.browser(ui);
                }
            });
        if self.dialog.is_some() || self.file_dialog.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_columns_adapt_to_window_size() {
        assert_eq!(Eder::grid_columns(280.0), 1);
        assert_eq!(Eder::grid_columns(420.0), 2);
        assert_eq!(Eder::grid_columns(850.0), 4);
    }

    #[test]
    fn compact_and_wide_layouts_virtualize_large_libraries() {
        let ctx = egui::Context::default();
        let (tx, _tasks) = mpsc::sync_channel(64);
        let (_events, rx) = mpsc::sync_channel(64);
        let (_page_tx, page_rx) = mpsc::sync_channel(1);
        let (thumb_tx, _thumb_jobs) = mpsc::sync_channel(12);
        let (_thumb_results, thumb_rx) = mpsc::sync_channel(4);
        let (link_tx, _link_jobs) = mpsc::sync_channel::<LinkJob>(12);
        let (_link_results, link_rx) = mpsc::sync_channel::<LinkResult>(8);
        let root = PathBuf::from("/library");
        let mut app = Eder {
            tx,
            rx,
            page_rx,
            link_tx,
            link_rx,
            link_gen: Arc::new(AtomicU64::new(0)),
            thumb_tx,
            thumb_rx,
            pages: Arc::new((Mutex::new(None), Condvar::new())),
            page_gen: Arc::new(AtomicU64::new(0)),
            thumb_gen: Arc::new(AtomicU64::new(0)),
            open_gen: Arc::new(AtomicU64::new(0)),
            logo: load_logo(&ctx),
            state: Snapshot {
                root: Some(root.clone()),
                ..Default::default()
            },
            favorite_set: HashSet::new(),
            view: View::Library,
            folder: Some(root.clone()),
            document_count: 10_000,
            search_query_cached: String::new(),
            search_indices: Vec::new(),
            search_listing: 0,
            collections: Vec::new(),
            search: String::new(),
            list_view: false,
            file_dialog: None,
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
            links: HashMap::new(),
            link_pending: HashSet::new(),
            target_edge: 1024,
            last_visible_page: None,
            position_changed_at: None,
            pending_page: None,
            queued_pages: VecDeque::new(),
            failed: HashSet::new(),
            scroll_to: None,
            scroll_fraction: 0.0,
            continuous_row_height: None,
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.reader_ui(ctx));
        app.turn(999);
        assert_eq!(app.reader.as_ref().unwrap().page, 3);
        app.turn(0);
        assert_eq!(app.reader.as_ref().unwrap().page, 1);
        assert!(app.reader.as_ref().unwrap().position_changed_at.is_some());
        assert!(
            _tasks.try_recv().is_err(),
            "Normal navigation must not write a position per page"
        );

        app.state.continuous_scroll = true;
        app.reader.as_mut().unwrap().total = 20;
        app.continuous_position_changed(5.4 * 850.0, 850.0);
        let saved = app.reader.as_ref().unwrap().reading_position(true);
        assert_eq!(saved.page, 6);
        assert!((saved.fraction - 0.4).abs() < 0.001);
        // Exact same reading point at a different viewport row height.
        let restored = ReadingPosition::from_scroll_offset(saved.scroll_offset(1250.0), 1250.0, 20);
        assert_eq!(restored.page, 6);
        assert!((restored.fraction - saved.fraction).abs() < 0.001);
        app.turn(8);
        assert_eq!(
            app.reader.as_ref().unwrap().reading_position(true),
            ReadingPosition::new(8, 0.0)
        );
        assert_eq!(
            app.reader.as_ref().unwrap().scroll_to,
            Some(ReadingPosition::new(8, 0.0))
        );

        let reader = app.reader.as_mut().unwrap();
        reader.total = 10_000;
        reader.scroll_to = Some(ReadingPosition::new(1, 0.0));
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
        let previous_generation = app.page_gen.load(Ordering::Acquire);
        app.queue_continuous_pages(&[5_000]);
        assert!(
            app.page_gen.load(Ordering::Acquire) > previous_generation,
            "A distant jump should cancel a render far outside the viewport"
        );
        assert!(
            app.reader.as_ref().unwrap().queued_pages.contains(&4_998),
            "Pages behind the visible range should be prefetched"
        );
        assert!(
            app.reader.as_ref().unwrap().queued_pages.contains(&5_002),
            "Pages ahead of the visible range should be prefetched"
        );
    }

    #[test]
    fn adaptive_resolution_is_bucketed_and_bounded() {
        let compact = Eder::page_edge(Vec2::new(360.0, 500.0), 1.0, 1.0, true);
        let wide = Eder::page_edge(Vec2::new(1100.0, 700.0), 1.0, 1.0, true);
        assert!(compact < wide);
        assert!(wide <= render::PAGE_EDGE);
        assert_eq!(
            Eder::page_edge(Vec2::new(3000.0, 2000.0), 3.0, 2.0, true),
            render::PAGE_EDGE
        );
    }
}

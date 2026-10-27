use super::*;
use crate::{
    app::reader::Reader,
    domain::{document::Entry, settings::Snapshot},
    render,
};
use std::collections::{HashMap, HashSet, VecDeque};
#[test]
fn grid_columns_adapt_to_window_size() {
    assert_eq!(Eder::grid_columns(280.0), 1);
    assert_eq!(Eder::grid_columns(420.0), 2);
    assert_eq!(Eder::grid_columns(850.0), 4);
}

#[test]
fn compact_and_wide_layouts_virtualize_large_libraries() {
    let ctx = egui::Context::default();
    let (runtime, _tasks) = RuntimeClient::detached();
    let root = PathBuf::from("/library");
    let mut app = Eder {
        runtime,
        app: crate::app::state::Controller {
            state: Snapshot {
                root: Some(root.clone()),
                ..Default::default()
            },
            favorite_set: HashSet::new(),
            view: View::Library,
            folder: Some(root.clone()),
            document_count: 10_000,
            collections: Vec::new(),
            entries: (0..10_000)
                .map(|i| Entry {
                    path: root.join(format!("{i}.pdf")),
                    name: format!("Document {i}"),
                    directory: false,
                    available: true,
                })
                .collect(),
            reader: None,
            listing: 1,
            loading: false,
            opening: false,
            message: None,
            lifecycle: crate::app::state::Lifecycle::Starting,
            open_request: 0,
        },
        ui: crate::ui::state::UiState {
            logo: load_logo(&ctx),
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
        },
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
            app.ui.pending.len() <= 12,
            "Thumbnail queue must remain bounded"
        );
    }
    app.app.reader = Some(Reader {
        path: root.join("sample.pdf"),
        revision: crate::domain::document::FileRevision {
            bytes: 0,
            modified_nanos: 0,
        },
        geometry: vec![crate::domain::document::PageGeometry::new(400.0, 550.0, 0).unwrap(); 3],
        page: 1,
        total: 3,
        loading: false,
        closing: false,
        zoom: 1.0,
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
        anchor_version: 0,
        horizontal: 0.0,
        layout: None,
    });
    let _ = ctx.run(egui::RawInput::default(), |ctx| app.reader_ui(ctx));
    app.turn(999);
    assert_eq!(app.app.reader.as_ref().unwrap().page, 3);
    app.turn(0);
    assert_eq!(app.app.reader.as_ref().unwrap().page, 1);
    assert!(app
        .app
        .reader
        .as_ref()
        .unwrap()
        .position_changed_at
        .is_some());
    assert!(
        _tasks.try_recv().is_err(),
        "Normal navigation must not write a position per page"
    );

    app.app.state.continuous_scroll = true;
    app.app.reader.as_mut().unwrap().total = 20;
    app.continuous_position_changed(5.4 * 850.0, 850.0);
    let saved = app.app.reader.as_ref().unwrap().reading_position(true);
    assert_eq!(saved.page, 6);
    assert!((saved.fraction - 0.4).abs() < 0.001);
    // Exact same reading point at a different viewport row height.
    let restored = ReadingPosition::from_scroll_offset(saved.scroll_offset(1250.0), 1250.0, 20);
    assert_eq!(restored.page, 6);
    assert!((restored.fraction - saved.fraction).abs() < 0.001);
    app.turn(8);
    assert_eq!(
        app.app.reader.as_ref().unwrap().reading_position(true),
        ReadingPosition::content(8, 0.0, 1.0, 0.0)
    );
    assert_eq!(
        app.app.reader.as_ref().unwrap().scroll_to,
        Some(ReadingPosition::content(8, 0.0, 1.0, 0.0))
    );

    let reader = app.app.reader.as_mut().unwrap();
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
        app.app.reader.as_ref().unwrap().pending_page.is_some(),
        "Visible pages should be queued for rendering"
    );
    assert!(
        app.app.reader.as_ref().unwrap().queued_pages.contains(&3),
        "Pages ahead of the visible range should be prefetched"
    );
    let previous_generation = app.runtime.page_gen.load(Ordering::Acquire);
    app.queue_continuous_pages(&[5_000]);
    assert!(
        app.runtime.page_gen.load(Ordering::Acquire) > previous_generation,
        "A distant jump should cancel a render far outside the viewport"
    );
    assert!(
        app.app
            .reader
            .as_ref()
            .unwrap()
            .queued_pages
            .contains(&4_998),
        "Pages behind the visible range should be prefetched"
    );
    assert!(
        app.app
            .reader
            .as_ref()
            .unwrap()
            .queued_pages
            .contains(&5_002),
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

#[test]
fn stale_rasters_are_discarded_before_texture_upload() {
    let ctx = egui::Context::default();
    let (runtime, _tasks) = RuntimeClient::detached();
    let mut app = Eder {
        runtime,
        app: crate::app::state::Controller {
            open_request: 1,
            ..Default::default()
        },
        ui: crate::ui::state::UiState::new(load_logo(&ctx)),
    };
    let revision = crate::domain::document::FileRevision {
        bytes: 1,
        modified_nanos: 2,
    };
    app.app.on_event(Event::Open(
        "book.pdf".into(),
        ReadingPosition::new(1, 0.0),
        1,
        1,
        vec![crate::domain::document::PageGeometry::new(400.0, 600.0, 0).unwrap()],
        revision,
    ));
    app.app.reader.as_mut().unwrap().pending_page = Some(1);
    app.runtime.page_gen.store(9, Ordering::Release);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    app.runtime.page_rx = rx;
    let key = app.app.reader.as_ref().unwrap().render_key(1, false);
    tx.send(PageResult {
        key: key.clone(),
        id: 8,
        result: Ok(egui::ColorImage::new([2, 2], Color32::WHITE)),
    })
    .unwrap_or_else(|_| panic!("queue disconnected"));
    app.poll(&ctx);
    assert!(app.ui.pages.is_empty());
    assert_eq!(app.app.reader.as_ref().unwrap().pending_page, Some(1));
    let mut wrong_key = key.clone();
    wrong_key.revision.bytes = 99;
    tx.send(PageResult {
        key: wrong_key,
        id: 9,
        result: Ok(egui::ColorImage::new([2, 2], Color32::WHITE)),
    })
    .unwrap_or_else(|_| panic!("queue disconnected"));
    app.poll(&ctx);
    assert!(app.ui.pages.is_empty());
    tx.send(PageResult {
        key,
        id: 9,
        result: Ok(egui::ColorImage::new([2, 2], Color32::WHITE)),
    })
    .unwrap_or_else(|_| panic!("queue disconnected"));
    app.poll(&ctx);
    assert_eq!(app.ui.pages.len(), 1);
    assert!(app.app.reader.as_ref().unwrap().pending_page.is_none());
}

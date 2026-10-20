use super::{
    action::Action,
    effect::Task,
    event::Event,
    reader::Reader,
    state::{Controller, View},
};
use crate::domain::reading_position::ReadingPosition;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    time::Instant,
};

/// Presentation/runtime instructions produced by pure state transitions.
pub enum Transition {
    Reload,
    Recover(PathBuf),
    ResetReader,
    Render,
    InvalidateRenders,
    FolderChanged,
    CloseWindow,
}
impl Controller {
    pub fn begin_listing(&mut self) -> Task {
        self.listing += 1;
        self.entries.clear();
        self.document_count = 0;
        self.loading = true;
        Task::Load(self.listing, self.view, self.folder.clone())
    }
    pub fn handle(&mut self, action: Action, now: Instant) -> bool {
        let Some(reader) = &mut self.reader else {
            return false;
        };
        let position = match action {
            Action::Navigate(page) => {
                ReadingPosition::content(page.clamp(1, reader.total), 0.0, reader.zoom, 0.0)
            }
            Action::ViewportPosition(position) => position,
        };
        let horizontal_changed = (reader.horizontal - position.horizontal).abs() > 0.0001;
        let version_changed = reader.anchor_version != position.anchor_version;
        reader.anchor_version = position.anchor_version;
        reader.horizontal = position.horizontal;
        if position.page == reader.page
            && (position.fraction - reader.scroll_fraction).abs() < 0.0001
            && !horizontal_changed
            && !version_changed
        {
            return false;
        }
        reader.page = position.page;
        reader.scroll_fraction = position.fraction;
        reader.position_changed_at = Some(now);
        true
    }
    pub fn on_event(&mut self, event: Event) -> Vec<Transition> {
        let mut effects = Vec::new();
        match event {
            Event::Snapshot(snapshot) => {
                if self.lifecycle == crate::app::state::Lifecycle::Starting {
                    self.lifecycle = crate::app::state::Lifecycle::Running;
                }
                let changed = self.state.root != snapshot.root;
                self.favorite_set = snapshot.favorites.iter().cloned().collect();
                self.state = snapshot;
                if changed || self.listing == 0 {
                    self.folder = self.state.root.clone();
                    effects.push(Transition::FolderChanged);
                    effects.push(Transition::Reload);
                } else if self.reader.is_none()
                    && matches!(self.view, View::Reading | View::Favorites)
                {
                    effects.push(Transition::Reload);
                }
            }
            Event::Rows(id, result) if id == self.listing => {
                self.loading = false;
                match result {
                    Ok(rows) => {
                        self.document_count = rows.iter().filter(|e| !e.directory).count();
                        self.entries = rows;
                    }
                    Err(error) => self.message = Some((error, true)),
                }
            }
            Event::Rows(..) => {}
            Event::Collections(id, rows) if id == self.listing => self.collections = rows,
            Event::Collections(..) => {}
            Event::Imported(count) => {
                self.message = Some((
                    format!(
                        "Added {count} PDF{} to your library.",
                        if count == 1 { "" } else { "s" }
                    ),
                    false,
                ));
                if count > 0 {
                    effects.push(Transition::Reload);
                }
            }
            Event::Open(path, position, total, id, geometry, revision)
                if id == self.open_request
                    && self.lifecycle != crate::app::state::Lifecycle::Closing =>
            {
                self.opening = false;
                self.reader = Some(Reader {
                    path,
                    geometry,
                    revision,
                    page: position.page,
                    total,
                    loading: !self.state.continuous_scroll,
                    closing: false,
                    zoom: position.zoom,
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
                    anchor_version: position.anchor_version,
                    horizontal: position.horizontal,
                    layout: None,
                });
                effects.push(Transition::ResetReader);
            }
            Event::Open(..) => {}
            Event::Recovery(path) => {
                if self.reader.is_none() && !self.opening && self.open_request == 0 {
                    effects.push(Transition::Recover(path));
                }
            }
            Event::Notice(message) => self.message = Some((message, false)),
            Event::OpenFailed(id, error)
                if id == self.open_request
                    && self.lifecycle != crate::app::state::Lifecycle::Closing =>
            {
                self.opening = false;
                self.message = Some((error, true));
            }
            Event::OpenFailed(..) => {}
            Event::Error(error) => self.message = Some((error, true)),
            Event::ShutdownFailed(error) => {
                self.opening = false;
                if let Some(reader) = &mut self.reader {
                    reader.link_pending.clear();
                }
                self.lifecycle = crate::app::state::Lifecycle::Running;
                self.message = Some((
                    format!("Could not finish saving: {error}. Close again to retry."),
                    true,
                ));
                effects.push(Transition::InvalidateRenders);
                if self.reader.is_some() && !self.state.continuous_scroll {
                    effects.push(Transition::Render);
                }
            }
            Event::Dark(dark) => {
                self.state.dark = dark;
                if self.state.continuous_scroll {
                    effects.push(Transition::InvalidateRenders);
                } else {
                    effects.push(Transition::Render);
                }
            }
            Event::ReadingMode(continuous) => {
                self.state.continuous_scroll = continuous;
                if let Some(reader) = &mut self.reader {
                    reader.scroll_to = Some(reader.reading_position(continuous));
                }
                effects.push(Transition::InvalidateRenders);
                if !continuous {
                    effects.push(Transition::Render);
                }
            }
            Event::Fatal(error) => {
                self.lifecycle = crate::app::state::Lifecycle::Failed;
                self.loading = false;
                self.opening = false;
                self.message = Some((error, true));
            }
            Event::ReaderClosed(id) if id == self.open_request => {
                self.reader = None;
                effects.push(Transition::ResetReader);
                effects.push(Transition::Reload);
            }
            Event::ReaderClosed(_) => {}
            Event::ReaderCloseFailed(id, error) if id == self.open_request => {
                if let Some(reader) = &mut self.reader {
                    reader.closing = false;
                    reader.link_pending.clear();
                }
                self.message = Some((error, true));
                effects.push(Transition::InvalidateRenders);
                if !self.state.continuous_scroll {
                    effects.push(Transition::Render);
                }
            }
            Event::ReaderCloseFailed(..) => {}
            Event::StorageCommitted => {}
            Event::Closed => {
                self.lifecycle = crate::app::state::Lifecycle::Closed;
                effects.push(Transition::CloseWindow);
            }
        }
        effects
    }
}

impl Controller {
    pub fn navigate(&mut self, page: u32, now: Instant) -> bool {
        let Some(reader) = &self.reader else {
            return false;
        };
        let page = page.clamp(1, reader.total);
        if page == reader.page && reader.scroll_fraction < 0.0001 && reader.horizontal < 0.0001 {
            return false;
        }
        self.handle(Action::Navigate(page), now);
        let reader = self.reader.as_mut().unwrap();
        reader.scroll_to = Some(ReadingPosition::content(page, 0.0, reader.zoom, 0.0));
        !self.state.continuous_scroll
    }
    pub fn change_view(&mut self, view: View) -> bool {
        if self.view == view {
            return false;
        }
        self.view = view;
        true
    }
    pub fn browse_folder(&mut self, folder: PathBuf) {
        self.view = View::Library;
        self.folder = Some(folder);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::state::Lifecycle,
        domain::document::{FileRevision, PageGeometry},
    };
    fn opened(id: u64, path: &str) -> Event {
        Event::Open(
            path.into(),
            ReadingPosition::content(2, 0.4, 1.5, 0.2),
            3,
            id,
            vec![PageGeometry::new(400.0, 600.0, 0).unwrap(); 3],
            FileRevision {
                bytes: 10,
                modified_nanos: 20,
            },
        )
    }
    #[test]
    fn obsolete_open_and_failure_cannot_replace_current_session() {
        let mut app = Controller {
            open_request: 2,
            opening: true,
            ..Default::default()
        };
        assert!(app.on_event(opened(1, "a.pdf")).is_empty());
        assert!(app.reader.is_none());
        app.on_event(Event::OpenFailed(1, "old failure".into()));
        assert!(app.opening);
        assert!(app.message.is_none());
        app.on_event(opened(2, "b.pdf"));
        assert_eq!(app.reader.as_ref().unwrap().path, PathBuf::from("b.pdf"));
        assert_eq!(app.reader.as_ref().unwrap().zoom, 1.5);
        app.on_event(Event::ReaderClosed(1));
        assert!(app.reader.is_some());
    }
    #[test]
    fn obsolete_rows_do_not_change_current_listing() {
        let mut app = Controller::default();
        app.begin_listing();
        app.begin_listing();
        app.on_event(Event::Rows(1, Err("stale".into())));
        assert!(app.loading);
        assert!(app.message.is_none());
        app.on_event(Event::Rows(2, Ok(Vec::new())));
        assert!(!app.loading);
    }
    #[test]
    fn closing_ignores_late_open_and_failed_close_is_retryable() {
        let mut app = Controller {
            open_request: 1,
            lifecycle: Lifecycle::Closing,
            ..Default::default()
        };
        app.on_event(opened(1, "a.pdf"));
        assert!(app.reader.is_none());
        app.on_event(Event::ShutdownFailed("disk full".into()));
        assert_eq!(app.lifecycle, Lifecycle::Running);
        assert!(app.message.unwrap().1);
    }
    #[test]
    fn horizontal_pan_changes_schedule_a_save_without_page_navigation() {
        let mut app = Controller {
            open_request: 1,
            ..Default::default()
        };
        app.on_event(opened(1, "a.pdf"));
        let now = Instant::now();
        app.handle(
            Action::ViewportPosition(ReadingPosition::content(2, 0.4, 1.5, 0.6)),
            now,
        );
        let reader = app.reader.as_ref().unwrap();
        assert_eq!(reader.position_changed_at, Some(now));
        assert_eq!(reader.reading_position(true).horizontal, 0.6);
    }
}

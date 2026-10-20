use super::*;

impl Eder {
    pub(crate) fn open(&mut self, path: PathBuf, recovery: bool) {
        if let Some(reader) = &self.app.reader {
            if !self.send(Task::Position(
                reader.path.clone(),
                reader.reading_position(self.app.state.continuous_scroll),
                false,
            )) {
                return;
            }
        }
        let id = self.runtime.open_gen.fetch_add(1, Ordering::AcqRel) + 1;
        self.app.open_request = id;
        self.app.opening = self.send(Task::Open(path, recovery, id));
        self.runtime.thumb_gen.fetch_add(1, Ordering::AcqRel);
        self.ui.pending.clear();
    }
    // Select quality by physical display pixels, rather than always rasterizing 2400px.
    // Buckets prevent tiny window/zoom changes from causing a new render.
    pub(crate) fn page_edge(
        viewport: Vec2,
        zoom: f32,
        pixels_per_point: f32,
        continuous: bool,
    ) -> u32 {
        let long_side_points = if continuous {
            viewport.x.max(1.0) * 1.42 * zoom
        } else {
            // A fitted page is generally portrait. Account for widescreen/landscape too.
            viewport.y.max(viewport.x * 0.85) * zoom
        };
        let desired = long_side_points * pixels_per_point * 1.12;
        crate::policy::edge_bucket(desired)
    }
    pub(crate) fn set_page_edge(&mut self, edge: u32) {
        let Some(reader) = &mut self.app.reader else {
            return;
        };
        if reader.target_edge == edge {
            return;
        }
        reader.target_edge = edge;
        if self.app.state.continuous_scroll {
            // Keep low-resolution cached pages visible while better versions arrive.
            self.runtime.page_gen.fetch_add(1, Ordering::AcqRel);
            reader.pending_page = None;
            reader.queued_pages.clear();
        } else {
            self.render();
        }
    }
    pub(crate) fn render(&mut self) {
        let Some(r) = &mut self.app.reader else {
            return;
        };
        let id = self.runtime.page_gen.fetch_add(1, Ordering::AcqRel) + 1;
        r.pending_page = None;
        r.queued_pages.clear();
        r.failed.remove(&r.page);
        let best_index = self
            .ui
            .pages
            .iter()
            .enumerate()
            .filter(|(_, item)| item.key.page == r.page && item.key.dark == self.app.state.dark)
            .max_by_key(|(_, item)| item.key.edge)
            .map(|(i, _)| i);
        if let Some(index) = best_index {
            let item = self.ui.pages.remove(index).unwrap();
            let suitable = item.key.edge >= r.target_edge;
            self.ui.current_page = Some(item.texture.clone());
            self.ui.pages.push_back(item);
            if suitable {
                r.loading = false;
                return;
            }
        } else {
            self.ui.current_page = None;
        }
        // During a quality upgrade, keep displaying the already available texture.
        r.loading = self.ui.current_page.is_none();
        r.pending_page = Some(r.page);
        self.runtime.pages.replace(PageJob {
            key: r.render_key(r.page, self.app.state.dark),
            id,
        });
    }
    #[cfg(test)]
    pub(crate) fn continuous_position_changed(&mut self, offset: f32, row_stride: f32) {
        if let Some(reader) = &self.app.reader {
            let position = ReadingPosition::from_scroll_offset(offset, row_stride, reader.total);
            self.app.handle(
                crate::app::action::Action::ViewportPosition(position),
                Instant::now(),
            );
        }
    }
    pub(crate) fn flush_position_if_idle(&mut self, ctx: &egui::Context) {
        let pending = self
            .app
            .reader
            .as_ref()
            .filter(|reader| !reader.closing)
            .and_then(|reader| {
                reader.position_changed_at.map(|changed| {
                    (
                        reader.path.clone(),
                        reader.reading_position(self.app.state.continuous_scroll),
                        changed,
                    )
                })
            });
        if let Some((path, position, changed)) = pending {
            if changed.elapsed() >= POSITION_SAVE_DELAY {
                if self.send(Task::Position(path, position, false)) {
                    if let Some(reader) = &mut self.app.reader {
                        reader.position_changed_at = None;
                    }
                }
            } else {
                ctx.request_repaint_after(POSITION_SAVE_DELAY.saturating_sub(changed.elapsed()));
            }
        }
    }
    pub(crate) fn turn(&mut self, page: u32) {
        if self.app.navigate(page, Instant::now()) {
            self.render();
        }
    }
    pub(crate) fn queue_continuous_pages(&mut self, visible: &[u32]) {
        let Some(reader) = &mut self.app.reader else {
            return;
        };
        let cached: Vec<_> = self.ui.pages.iter().map(|item| item.key.clone()).collect();
        if reader.plan_pages(visible, self.app.state.dark, &cached) {
            self.runtime.page_gen.fetch_add(1, Ordering::AcqRel);
        }
        self.dispatch_continuous_page();
    }
    pub(crate) fn dispatch_continuous_page(&mut self) {
        let generation = self.runtime.page_gen.load(Ordering::Acquire);
        if let Some(reader) = &mut self.app.reader {
            if let Some(job) = reader.next_job(generation, self.app.state.dark) {
                self.runtime.pages.replace(job);
            }
        }
    }
    pub(crate) fn queue_links(&mut self, page: u32) {
        let Some(r) = &mut self.app.reader else {
            return;
        };
        if r.links.contains_key(&page) || !r.link_pending.insert(page) {
            return;
        }
        let job = LinkJob {
            path: r.path.clone(),
            page,
            total: r.total,
            id: self.runtime.link_gen.load(Ordering::Acquire),
        };
        if self.runtime.link_tx.try_send(job).is_err() {
            r.link_pending.remove(&page);
        }
    }
    pub(crate) fn back(&mut self) {
        if let Some(reader) = &self.app.reader {
            if reader.closing {
                return;
            }
            if self.send(Task::Close(
                reader.path.clone(),
                reader.reading_position(self.app.state.continuous_scroll),
                self.app.open_request,
            )) {
                let reader = self.app.reader.as_mut().unwrap();
                reader.closing = true;
                reader.pending_page = None;
                reader.queued_pages.clear();
                self.runtime.page_gen.fetch_add(1, Ordering::AcqRel);
                self.runtime.link_gen.fetch_add(1, Ordering::AcqRel);
            }
        }
    }
}

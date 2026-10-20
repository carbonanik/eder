use super::*;

impl Eder {
    pub(crate) fn poll(&mut self, ctx: &egui::Context) {
        if self.app.state.continuous_scroll {
            self.ui.current_page = None;
        }
        for index in 0..32 {
            let Ok(event) = self.runtime.rx.try_recv() else {
                break;
            };
            if index == 31 {
                ctx.request_repaint();
            }
            if matches!(event, Event::StorageCommitted) {
                self.runtime.finish_shutdown();
                continue;
            }
            if matches!(event, Event::ShutdownFailed(_)) {
                self.runtime.resume_after_failed_close();
            }
            for transition in self.app.on_event(event) {
                use crate::app::controller::Transition;
                if self.app.lifecycle == crate::app::state::Lifecycle::Closing
                    && !matches!(transition, Transition::CloseWindow)
                {
                    continue;
                }
                match transition {
                    Transition::Reload => {
                        self.ui.search_listing = 0;
                        self.load();
                    }
                    Transition::Recover(path) => self.open(path, true),
                    Transition::ResetReader => {
                        self.runtime.page_gen.fetch_add(1, Ordering::AcqRel);
                        self.runtime.link_gen.fetch_add(1, Ordering::AcqRel);
                        self.ui.pages.clear();
                        self.ui.current_page = None;
                    }
                    Transition::Render => self.render(),
                    Transition::InvalidateRenders => {
                        self.runtime.page_gen.fetch_add(1, Ordering::AcqRel);
                        if let Some(reader) = &mut self.app.reader {
                            reader.pending_page = None;
                            reader.queued_pages.clear();
                        }
                    }
                    Transition::FolderChanged => {
                        self.ui.folder_input = self
                            .app
                            .folder
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default()
                    }
                    Transition::CloseWindow => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                }
            }
        }
        if self.app.lifecycle == crate::app::state::Lifecycle::Closing {
            return;
        }
        // At most one GPU page upload per frame, avoiding bursts during rapid scroll.
        if let Ok(result) = self.runtime.page_rx.try_recv() {
            ctx.request_repaint();
            if result.id == self.runtime.page_gen.load(Ordering::Acquire) {
                let mut load_links = false;
                if let Some(r) = &mut self.app.reader {
                    if result.key.document.path() != r.path || result.key.revision != r.revision {
                        return;
                    }
                    if r.pending_page == Some(result.key.page) {
                        r.pending_page = None;
                    }
                    match result.result {
                        Ok(image) => {
                            let redundant = self.ui.pages.iter().any(|item| {
                                item.key.page == result.key.page
                                    && item.key.dark == result.key.dark
                                    && item.key.edge >= result.key.edge
                            });
                            if !redundant {
                                let t =
                                    ctx.load_texture("page", image, egui::TextureOptions::LINEAR);
                                self.ui.pages.retain(|item| {
                                    !(item.key.page == result.key.page
                                        && item.key.dark == result.key.dark
                                        && item.key.edge <= result.key.edge)
                                });
                                self.ui.pages.push_back(CachedPage {
                                    key: result.key.clone(),
                                    texture: t.clone(),
                                });
                                if !self.app.state.continuous_scroll
                                    && result.key.page == r.page
                                    && result.key.dark == self.app.state.dark
                                {
                                    self.ui.current_page = Some(t);
                                    r.loading = false;
                                }
                                self.ui.enforce_page_budget();
                            }
                            load_links = true;
                        }
                        Err(e) => {
                            r.failed.insert(result.key.page);
                            if result.key.page == r.page {
                                r.loading = false;
                                self.app.message = Some((e.to_string(), true));
                            }
                        }
                    }
                }
                if load_links {
                    self.queue_links(result.key.page);
                }
            }
        }
        // Independent metadata pipeline: no raster blocks on pdftohtml.
        for _ in 0..8 {
            let Ok(result) = self.runtime.link_rx.try_recv() else {
                break;
            };
            if result.id == self.runtime.link_gen.load(Ordering::Acquire) {
                if let Some(r) = &mut self.app.reader {
                    r.link_pending.remove(&result.page);
                    r.links.insert(result.page, result.links);
                    while r.links.len() > crate::policy::MAX_LINK_PAGES {
                        let farthest = r
                            .links
                            .keys()
                            .copied()
                            .max_by_key(|page| page.abs_diff(r.page));
                        if let Some(page) = farthest {
                            r.links.remove(&page);
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        self.dispatch_continuous_page();
        // Thumbnail upload is also limited to one per frame, and paused in the reader.
        if self.app.reader.is_none() {
            if let Ok(result) = self.runtime.thumb_rx.try_recv() {
                ctx.request_repaint();
                if result.id == self.runtime.thumb_gen.load(Ordering::Acquire) {
                    self.ui.pending.remove(&result.path);
                    self.touch(&result.path);
                    self.ui.textures.insert(
                        result.path,
                        result
                            .image
                            .map(|i| ctx.load_texture("preview", i, egui::TextureOptions::LINEAR)),
                    );
                    while self.ui.lru.len() > crate::policy::THUMBNAIL_ENTRIES {
                        if let Some(p) = self.ui.lru.pop_front() {
                            self.ui.textures.remove(&p);
                        }
                    }
                }
            }
        }
        if let Some(rx) = &self.ui.dialog {
            if let Ok(p) = rx.try_recv() {
                self.ui.dialog = None;
                if let Some(p) = p {
                    self.send(Task::Root(p));
                } else {
                    self.app.message=Some(("No folder selected. If your desktop does not support folder selection, use the folder path field.".into(),false));
                }
            }
        }
        if let Some(rx) = &self.ui.file_dialog {
            if let Ok(picked) = rx.try_recv() {
                self.ui.file_dialog = None;
                if let Some(paths) = picked {
                    let destination = if self.app.view == View::Library {
                        self.app
                            .folder
                            .clone()
                            .or_else(|| self.app.state.root.clone())
                    } else {
                        self.app.state.root.clone()
                    };
                    if let Some(destination) = destination {
                        if let Some(root) = self.app.state.root.clone() {
                            self.send(Task::Import(paths, root, destination));
                        }
                    }
                }
            }
        }
    }
}

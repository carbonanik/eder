use super::*;

impl Eder {
    pub(crate) fn grid_columns(width: f32) -> usize {
        const MIN_TILE_WIDTH: f32 = 195.0;
        const GAP: f32 = 14.0;
        ((width + GAP) / (MIN_TILE_WIDTH + GAP)).floor().max(1.0) as usize
    }

    pub(crate) fn document_tile(&mut self, ui: &mut egui::Ui, e: &Entry, width: f32) {
        const TILE_HEIGHT: f32 = 266.0;
        const PREVIEW_HEIGHT: f32 = 154.0;
        const PADDING: f32 = 12.0;

        if !e.directory && e.available {
            if self.ui.textures.contains_key(&e.path) {
                self.touch(&e.path);
            } else if !self.ui.pending.contains(&e.path)
                && self
                    .runtime
                    .thumb_tx
                    .try_send(ThumbJob {
                        path: e.path.clone(),
                        id: self.runtime.thumb_gen.load(Ordering::Acquire),
                    })
                    .is_ok()
            {
                self.ui.pending.insert(e.path.clone());
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
                        } else if let Some(Some(texture)) = self.ui.textures.get(&e.path) {
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
                                self.app.folder = Some(e.path.clone());
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
                                self.app.folder = Some(e.path.clone());
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
                            let favored = self.app.favorite_set.contains(&e.path);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                let remove_width = if self.app.view == View::Reading {
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
                                if self.app.view == View::Reading
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

    pub(crate) fn document_row(&mut self, ui: &mut egui::Ui, e: &Entry) {
        if !e.directory
            && e.available
            && !self.ui.textures.contains_key(&e.path)
            && !self.ui.pending.contains(&e.path)
            && self
                .runtime
                .thumb_tx
                .try_send(ThumbJob {
                    path: e.path.clone(),
                    id: self.runtime.thumb_gen.load(Ordering::Acquire),
                })
                .is_ok()
        {
            self.ui.pending.insert(e.path.clone());
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
                    if let Some(Some(texture)) = self.ui.textures.get(&e.path) {
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
                            if self.app.view == View::Reading
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
                            let favorite = self.app.favorite_set.contains(&e.path);
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

    pub(crate) fn browser(&mut self, ui: &mut egui::Ui) {
        if self.app.view == View::Library && self.app.state.root.is_none() {
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
        if self.app.loading || self.app.opening {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(if self.app.opening {
                    "Opening PDF…"
                } else {
                    "Loading library…"
                });
            });
            return;
        }
        if self.app.entries.is_empty() {
            ui.label(match self.app.view {
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
        let query = self.ui.search.trim().to_lowercase();
        if self.ui.search_listing != self.app.listing || self.ui.search_query_cached != query {
            self.ui.search_indices = if query.is_empty() {
                Vec::new()
            } else {
                self.app
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| entry.name.to_lowercase().contains(&query))
                    .map(|(index, _)| index)
                    .collect()
            };
            self.ui.search_query_cached = query.clone();
            self.ui.search_listing = self.app.listing;
        }
        let total = if query.is_empty() {
            self.app.entries.len()
        } else {
            self.ui.search_indices.len()
        };
        if total == 0 {
            ui.label("No matching documents in this folder. Try a different search.");
            return;
        }
        if self.ui.list_view {
            egui::ScrollArea::vertical()
                .id_salt(("library-list", self.app.listing))
                .auto_shrink([false, false])
                .show_rows(ui, 92.0, total, |ui, range| {
                    for index in range {
                        let actual = if query.is_empty() {
                            index
                        } else {
                            self.ui.search_indices[index]
                        };
                        if let Some(entry) = self.app.entries.get(actual).cloned() {
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
            .id_salt(("library-grid", self.app.listing))
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
                                *self.ui.search_indices.get(index).unwrap_or(&usize::MAX)
                            };
                            if let Some(entry) = self.app.entries.get(actual).cloned() {
                                ui.push_id(&entry.path, |ui| {
                                    self.document_tile(ui, &entry, tile_width);
                                });
                            }
                        }
                    });
                }
            });
    }
}

use super::*;

impl Eder {
    pub(crate) fn reader_ui(&mut self, ctx: &egui::Context) {
        if self.app.reader.as_ref().is_some_and(|r| r.closing) {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.spinner();
                ui.label("Saving reading position…");
            });
            return;
        }
        if !ctx.wants_keyboard_input()
            && ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)))
        {
            if let Some(text) = self.ui.annotations.selected_text(self.app.reader.as_ref()) {
                ctx.copy_text(text);
            }
        }
        self.reader_toolbar(ctx);
        if self.app.reader.is_none() {
            return;
        }
        self.reader_rail(ctx);
        self.annotation_panel(ctx);
        let panels = (self.ui.annotations.open, self.ui.annotations.pages_open);
        if panels
            != (
                self.app.state.reader_annotations_open,
                self.app.state.reader_pages_open,
            )
            && self.send(Task::ReaderPanels(panels.0, panels.1))
        {
            self.app.state.reader_annotations_open = panels.0;
            self.app.state.reader_pages_open = panels.1;
        }
        let mut internal_jump = None;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(if self.app.state.dark {
                        Color32::from_rgb(14, 23, 30)
                    } else {
                        reader_palette(&ctx.style().visuals).sage
                    })
                    .inner_margin(16.0),
            )
            .show(ctx, |ui| {
                if self.app.state.continuous_scroll {
                    self.continuous_reader_ui(ui);
                    return;
                }
                let desired = Self::page_edge(ui.available_size(),
                    self.app.reader.as_ref().unwrap().zoom, ui.ctx().pixels_per_point(), false);
                self.set_page_edge(desired);
                let keyboard_scroll = self.reader_scroll_delta(ui.ctx());
                let r = self.app.reader.as_ref().unwrap();
                if r.loading && r.pending_page.is_none() && self.ui.current_page.is_none() {
                    self.render();
                }
                let restore = if self.ui.current_page.is_some() { self.app.reader.as_mut().unwrap().scroll_to.take() } else { None };
                let r = self.app.reader.as_ref().unwrap();
                if r.loading {
                    ui.centered_and_justified(|ui| {
                        ui.spinner();
                    });
                    return;
                }
                if let Some(t) = &self.ui.current_page {
                    let size = t.size_vec2();
                    let fit = (ui.available_width() / size.x)
                        .min(ui.available_height() / size.y)
                        .min(1.0);
                    let size = size * fit * r.zoom;
                    let page = r.page; let zoom = r.zoom;
                    let mut area = egui::ScrollArea::both().id_salt(("page", &r.path, r.page));
                    if let Some(anchor) = restore { area=area.scroll_offset(Vec2::new(anchor.horizontal*size.x, if anchor.anchor_version==1 { anchor.fraction*size.y } else { 0.0 })); }
                    let result = area.show(ui, |ui| {
                            ui.scroll_with_delta(keyboard_scroll);
                            ui.horizontal(|ui| {
                                ui.add_space(((ui.available_width() - size.x) / 2.0).max(0.0));
                                let image = ui.add(egui::Image::new(t).fit_to_exact_size(size));
                                super::annotations::text_overlay(ui,image.rect,r.page,r.text.get(&r.page).map(Vec::as_slice).unwrap_or_default(),&r.annotations,&mut self.ui.annotations.selection, self.ui.annotations.pan);
                                if let Some(links) = r.links.get(&r.page) {
                                    internal_jump = clicked_pdf_link(ui, image.rect, r.page, links);
                                }
                            });
                        });
                    self.app.handle(crate::app::action::Action::ViewportPosition(ReadingPosition::content(page,result.state.offset.y/size.y,zoom,result.state.offset.x/size.x)), std::time::Instant::now());
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
    fn reader_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("reader_toolbar")
            .frame(
                egui::Frame::none()
                    .fill(reader_palette(&ctx.style().visuals).paper)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0)),
            )
            .show(ctx, |ui| {
                reader_style(ui);
                toolbar_style(ui);
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                ui.spacing_mut().button_padding = Vec2::new(8.0, 6.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new(&self.ui.logo).fit_to_exact_size(Vec2::splat(24.0)));
                    let title = self
                        .app
                        .reader
                        .as_ref()
                        .map(|r| {
                            r.path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned()
                        })
                        .unwrap_or_default();
                    let title_width = (ui.available_width() - 110.0).max(20.0);
                    ui.add_sized(
                        [title_width, 28.0],
                        egui::Label::new(RichText::new(&title).size(14.0)).truncate(),
                    )
                    .on_hover_text(title);
                    let mut dark_theme = self.app.state.dark_theme;
                    if theme_switch(ui, &mut dark_theme).changed() {
                        self.send(Task::DarkTheme(dark_theme));
                    }
                    ui.menu_button(
                        egui::RichText::new(UiIcon::Settings.glyph().to_string()).font(
                            egui::FontId::new(
                                18.0,
                                egui::FontFamily::Name(MATERIAL_SYMBOLS_FONT.into()),
                            ),
                        ),
                        |ui| {
                            ui.label("Reader settings");
                            let mut continuous = self.app.state.continuous_scroll;
                            if ui
                                .checkbox(&mut continuous, "Continuous scrolling")
                                .changed()
                            {
                                self.send(Task::ReadingMode(continuous));
                            }
                            let mut dark = self.app.state.dark_theme;
                            if ui.checkbox(&mut dark, "Dark interface").changed() {
                                self.send(Task::DarkTheme(dark));
                            }
                            if ui.button("Keyboard shortcuts (F1)").clicked() {
                                self.ui.shortcuts_open = true;
                                ui.close_menu();
                            }
                            ui.weak("Reading position saves automatically");
                        },
                    )
                    .response
                    .on_hover_text("Reader settings");
                });
                if self.app.reader.is_none() {
                    return;
                }
                // Groups wrap as units on narrow windows, keeping navigation together.
                let wide_toolbar = ui.available_width() >= 680.0;
                let group_gap = ((ui.available_width() - 620.0) / 2.0).max(12.0);
                // Size this row from its contents so the selection row below cannot
                // feed the panel's previous height back into the next frame.
                ui.horizontal_wrapped(|ui| {
                    toolbar_group(ui, |ui| {
                        let colors = reader_palette(ui.visuals());
                        if toolbar_icon_label_button(
                            ui,
                            UiIcon::ArrowLeft,
                            "",
                            IconLabelStyle {
                                size: Vec2::new(32.0, 32.0),
                                fill: Color32::TRANSPARENT,
                                color: colors.ink,
                                text_size: 13.0,
                                rounding: 6.0,
                            },
                        )
                        .on_hover_text("Back to library")
                        .clicked()
                        {
                            self.back();
                        }
                        self.reader_navigation_tools(ui);
                    });
                    // Balance the three groups across wide windows; compact windows wrap.
                    ui.add_space(group_gap);
                    toolbar_group(ui, |ui| self.reader_zoom_tools(ui));
                    if wide_toolbar {
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.allocate_ui_with_layout(
                                    Vec2::new(212.0, 34.0),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| self.reader_action_tools(ui),
                                );
                            },
                        );
                    } else {
                        self.reader_action_tools(ui);
                    }
                });
                if self.ui.annotations.selection.is_some() && !self.ui.annotations.pan {
                    ui.horizontal_wrapped(|ui| {
                        ui.weak("Selected text");
                        self.selection_tools(ui);
                    });
                }
            });
    }

    fn reader_action_tools(&mut self, ui: &mut egui::Ui) {
        toolbar_group(ui, |ui| {
            self.annotation_tools(ui);
            let colors = reader_palette(ui.visuals());
            if toolbar_icon_label_button(
                ui,
                UiIcon::Page,
                "",
                IconLabelStyle {
                    size: Vec2::splat(32.0),
                    fill: if self.ui.annotations.pages_open {
                        colors.sage
                    } else {
                        Color32::TRANSPARENT
                    },
                    color: if self.ui.annotations.pages_open {
                        colors.teal
                    } else {
                        colors.ink
                    },
                    text_size: 13.0,
                    rounding: 6.0,
                },
            )
            .on_hover_text("Toggle page navigation")
            .clicked()
            {
                self.ui.annotations.pages_open = !self.ui.annotations.pages_open;
            }
            if toolbar_icon_label_button(
                ui,
                UiIcon::Note,
                "",
                IconLabelStyle {
                    size: Vec2::splat(32.0),
                    fill: if self.ui.annotations.open {
                        colors.sage
                    } else {
                        Color32::TRANSPARENT
                    },
                    color: if self.ui.annotations.open {
                        colors.teal
                    } else {
                        colors.ink
                    },
                    text_size: 13.0,
                    rounding: 6.0,
                },
            )
            .on_hover_text("Toggle annotations")
            .clicked()
            {
                self.ui.annotations.open = !self.ui.annotations.open;
            }
            ui.menu_button(
                egui::RichText::new(UiIcon::MoreVert.glyph().to_string()).font(egui::FontId::new(
                    18.0,
                    egui::FontFamily::Name(MATERIAL_SYMBOLS_FONT.into()),
                )),
                |ui| {
                    let mut dark = self.app.state.dark;
                    if ui.checkbox(&mut dark, "Dark pages").changed() {
                        self.send(Task::Dark(dark));
                    }
                    let fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
                    if toolbar_icon_label_button(
                        ui,
                        UiIcon::Fullscreen,
                        if fullscreen {
                            "Exit fullscreen"
                        } else {
                            "Fullscreen"
                        },
                        IconLabelStyle {
                            size: Vec2::new(160.0, 32.0),
                            fill: Color32::TRANSPARENT,
                            color: reader_palette(ui.visuals()).ink,
                            text_size: 13.0,
                            rounding: 6.0,
                        },
                    )
                    .clicked()
                    {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
                        ui.close_menu();
                    }
                },
            );
        });
    }

    fn reader_navigation_tools(&mut self, ui: &mut egui::Ui) {
        let reader = self.app.reader.as_ref().unwrap();
        let (page, total) = (reader.page, reader.total);
        let mut jump = page;
        if ui
            .add_sized(
                [44.0, 32.0],
                egui::DragValue::new(&mut jump).range(1..=total).speed(0.15),
            )
            .on_hover_text("Type a page number or drag to navigate")
            .changed()
        {
            self.turn(jump);
        }
        ui.weak(format!("/ {total}"));
        let previous = ui
            .add_enabled(
                page > 1,
                egui::Button::new("").min_size(egui::Vec2::splat(32.0)),
            )
            .on_hover_text("Previous page");
        paint_icon(
            ui.painter(),
            previous.rect.shrink(4.0),
            UiIcon::ChevronLeft,
            if previous.enabled() {
                reader_palette(ui.visuals()).ink
            } else {
                reader_palette(ui.visuals()).muted
            },
        );
        if previous.clicked() {
            self.turn(page - 1);
        }
        let next = ui
            .add_enabled(
                page < total,
                egui::Button::new("").min_size(egui::Vec2::splat(32.0)),
            )
            .on_hover_text("Next page");
        paint_icon(
            ui.painter(),
            next.rect.shrink(4.0),
            UiIcon::ChevronRight,
            if next.enabled() {
                reader_palette(ui.visuals()).ink
            } else {
                reader_palette(ui.visuals()).muted
            },
        );
        if next.clicked() {
            self.turn(page + 1);
        }
    }

    fn reader_zoom_tools(&mut self, ui: &mut egui::Ui) {
        let colors = reader_palette(ui.visuals());
        if toolbar_icon_label_button(
            ui,
            UiIcon::Minus,
            "",
            IconLabelStyle {
                size: Vec2::splat(32.0),
                fill: Color32::TRANSPARENT,
                color: colors.ink,
                text_size: 13.0,
                rounding: 6.0,
            },
        )
        .on_hover_text("Zoom out (Ctrl/Cmd+−)")
        .clicked()
        {
            self.zoom_reader(-0.1, false);
        }
        let zoom = self.app.reader.as_ref().unwrap().zoom;
        ui.menu_button(format!("{:.0}%", zoom * 100.0), |ui| {
            for value in [0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0] {
                if ui
                    .selectable_label(
                        (zoom - value).abs() < 0.01,
                        format!("{:.0}%", value * 100.0),
                    )
                    .clicked()
                {
                    self.zoom_reader(value - zoom, false);
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui
                .button("Fit page")
                .on_hover_text("Fit the whole page in this viewport")
                .clicked()
            {
                // Single-page mode fits both dimensions at 100%.
                if self.app.state.continuous_scroll {
                    self.send(Task::ReadingMode(false));
                }
                self.zoom_reader(0.0, true);
                ui.close_menu();
            }
            if ui
                .button("Fit width")
                .on_hover_text("Read full-width pages with vertical scrolling")
                .clicked()
            {
                if !self.app.state.continuous_scroll {
                    self.send(Task::ReadingMode(true));
                }
                self.zoom_reader(0.0, true);
                ui.close_menu();
            }
        })
        .response
        .on_hover_text("Zoom presets and page fitting (Ctrl/Cmd+0 to reset)");
        if toolbar_icon_label_button(
            ui,
            UiIcon::Plus,
            "",
            IconLabelStyle {
                size: Vec2::splat(32.0),
                fill: Color32::TRANSPARENT,
                color: colors.ink,
                text_size: 13.0,
                rounding: 6.0,
            },
        )
        .on_hover_text("Zoom in (Ctrl/Cmd++)")
        .clicked()
        {
            self.zoom_reader(0.1, false);
        }
    }

    fn reader_rail(&mut self, ctx: &egui::Context) {
        // Compact windows keep one sidebar open so the page retains usable space.
        if ctx.screen_rect().width() < 850.0 && self.ui.annotations.open {
            self.ui.annotations.pages_open = false;
        }
        if !self.ui.annotations.pages_open {
            return;
        }
        egui::SidePanel::left("reader_pages")
            .default_width(240.0)
            .width_range(180.0..=290.0)
            .frame(
                egui::Frame::none()
                    .fill(reader_palette(&ctx.style().visuals).paper)
                    .inner_margin(14.0),
            )
            .show(ctx, |ui| {
                reader_style(ui);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.ui.annotations.navigation_tab, 0, "Pages");
                    ui.selectable_value(&mut self.ui.annotations.navigation_tab, 1, "Bookmarks");
                    if ui
                        .small_button("×")
                        .on_hover_text("Close navigation")
                        .clicked()
                    {
                        self.ui.annotations.pages_open = false;
                    }
                });
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(6.0);
                if self.ui.annotations.navigation_tab == 1 {
                    let bookmarks: Vec<_> = self
                        .app
                        .reader
                        .as_ref()
                        .unwrap()
                        .annotations
                        .iter()
                        .filter(|a| a.color.is_none())
                        .map(|a| a.page)
                        .collect();
                    if bookmarks.is_empty() {
                        ui.weak("No bookmarks yet.");
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for page in bookmarks {
                            if ui
                                .selectable_label(
                                    self.app.reader.as_ref().unwrap().page == page,
                                    format!("Page {page}"),
                                )
                                .clicked()
                            {
                                self.turn(page);
                            }
                        }
                    });
                    return;
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.ui.annotations.page_query)
                        .hint_text("Find page number…")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(12.0);
                let (page, total) = {
                    let r = self.app.reader.as_ref().unwrap();
                    (r.page, r.total)
                };
                let query = self.ui.annotations.page_query.trim();
                let targets: Vec<u32> = (1..=total)
                    .filter(|p| query.is_empty() || p.to_string().contains(query))
                    .collect();
                let columns = if ui.available_width() >= 190.0 { 2 } else { 1 };
                let cell_width =
                    (ui.available_width() - 10.0 * (columns - 1) as f32 - 8.0) / columns as f32;
                egui::ScrollArea::vertical()
                    .id_salt("page_navigation")
                    .auto_shrink([false, false])
                    .show_rows(ui, 146.0, targets.len().div_ceil(columns), |ui, range| {
                        for row in range {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 10.0;
                                for &target in targets.iter().skip(row * columns).take(columns) {
                                    let preview = self
                                        .ui
                                        .annotations
                                        .thumbnails
                                        .get(&target)
                                        .and_then(Clone::clone);
                                    if !self.ui.annotations.thumbnails.contains_key(&target)
                                        && self.ui.annotations.thumbnail_pending.len() < 4
                                        && !self.ui.annotations.thumbnail_pending.contains(&target)
                                    {
                                        let job = ThumbJob {
                                            page: Some(target),
                                            path: self.app.reader.as_ref().unwrap().path.clone(),
                                            id: self.runtime.thumb_gen.load(Ordering::Acquire),
                                        };
                                        if self.runtime.thumb_tx.try_send(job).is_ok() {
                                            self.ui.annotations.thumbnail_pending.insert(target);
                                        }
                                    }
                                    let colors = reader_palette(ui.visuals());
                                    let selected = target == page;
                                    let response = egui::Frame::none()
                                        .fill(if selected {
                                            colors.sage
                                        } else {
                                            Color32::TRANSPARENT
                                        })
                                        .stroke(egui::Stroke::new(
                                            1.0_f32,
                                            if selected {
                                                colors.teal
                                            } else {
                                                Color32::TRANSPARENT
                                            },
                                        ))
                                        .inner_margin(8.0)
                                        .rounding(9.0)
                                        .show(ui, |ui| {
                                            ui.set_width((cell_width - 16.0).max(50.0));
                                            ui.set_min_height(122.0);
                                            ui.vertical_centered(|ui| {
                                                if let Some(texture) = preview {
                                                    let size = texture.size_vec2();
                                                    ui.add(
                                                        egui::Image::new(&texture)
                                                            .fit_to_exact_size(
                                                                size * (94.0 / size.y).min(
                                                                    (cell_width - 24.0) / size.x,
                                                                ),
                                                            ),
                                                    );
                                                } else {
                                                    let (rect, _) = ui.allocate_exact_size(
                                                        Vec2::new(62.0, 94.0),
                                                        egui::Sense::hover(),
                                                    );
                                                    ui.painter().rect_filled(
                                                        rect,
                                                        2.0,
                                                        colors.light,
                                                    );
                                                    ui.painter().text(
                                                        rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        target.to_string(),
                                                        egui::FontId::proportional(16.0),
                                                        colors.muted,
                                                    );
                                                }
                                                ui.add_space(4.0);
                                                ui.label(
                                                    RichText::new(format!("Page {target}"))
                                                        .size(12.0),
                                                );
                                            });
                                        })
                                        .response;
                                    if ui
                                        .interact(
                                            response.rect,
                                            ui.id().with(target),
                                            egui::Sense::click(),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        self.turn(target);
                                    }
                                }
                            });
                        }
                    });
            });
    }

    pub(crate) fn continuous_reader_ui(&mut self, ui: &mut egui::Ui) {
        use crate::app::reader::layout::PageLayout;
        let keyboard_scroll = self.reader_scroll_delta(ui.ctx());
        let zoom = self.app.reader.as_ref().unwrap().zoom;
        let desired = Self::page_edge(ui.available_size(), zoom, ui.ctx().pixels_per_point(), true);
        self.set_page_edge(desired);
        let width = (ui.available_width() - 24.0).max(1.0);
        let reader = self.app.reader.as_mut().unwrap();
        let rebuild = reader.layout.as_ref().is_none_or(|l| {
            (l.width - width).abs() > 0.5
                || l.zoom != zoom
                || l.heights.len() != reader.geometry.len()
        });
        let mut restore = reader.scroll_to.take();
        if rebuild {
            if restore.is_none() {
                restore = Some(reader.reading_position(true));
            }
            reader.layout = Some(PageLayout::new(&reader.geometry, width, zoom, 28.0));
        }
        let layout = reader.layout.as_ref().unwrap();
        if let Some(anchor) = &mut restore {
            anchor.page = anchor.page.clamp(1, reader.total);
            if anchor.anchor_version == 0 {
                *anchor = layout.convert_legacy(
                    *anchor,
                    width + 24.0,
                    ui.spacing().item_spacing.y,
                    ui.text_style_height(&egui::TextStyle::Body),
                );
            }
        }
        let mut area = egui::ScrollArea::both()
            .id_salt(("continuous-pages", reader.path.clone()))
            .auto_shrink([false, false]);
        if let Some(anchor) = restore {
            area = area
                .vertical_scroll_offset(layout.offset(anchor))
                .horizontal_scroll_offset(anchor.horizontal * width * zoom);
        }
        let mut visible = Vec::new();
        let mut retry = None;
        let mut jump = None;
        let result = area.show_viewport(ui, |ui, viewport| {
            ui.scroll_with_delta(keyboard_scroll);
            let content_width = (width * zoom).max(viewport.width());
            ui.set_min_size(Vec2::new(content_width, layout.total_height));
            let origin = ui.min_rect().min;
            let range = layout.visible(viewport.min.y, viewport.max.y);
            for index in range {
                let page = index as u32 + 1;
                visible.push(page);
                let rect = egui::Rect::from_min_size(
                    origin
                        + Vec2::new(
                            ((content_width - width * zoom) / 2.0).max(0.0),
                            layout.starts[index],
                        ),
                    Vec2::new(width * zoom, layout.heights[index]),
                );
                let texture = self
                    .ui
                    .pages
                    .iter()
                    .filter(|item| item.key.page == page && item.key.dark == self.app.state.dark)
                    .max_by_key(|item| item.key.edge)
                    .map(|item| item.texture.clone());
                if let Some(texture) = texture {
                    let image = ui.put(
                        rect,
                        egui::Image::new(&texture).fit_to_exact_size(rect.size()),
                    );
                    super::annotations::text_overlay(
                        ui,
                        image.rect,
                        page,
                        reader
                            .text
                            .get(&page)
                            .map(Vec::as_slice)
                            .unwrap_or_default(),
                        &reader.annotations,
                        &mut self.ui.annotations.selection,
                        self.ui.annotations.pan,
                    );
                    if let Some(links) = reader.links.get(&page) {
                        if let Some(target) = clicked_pdf_link(ui, image.rect, page, links) {
                            jump = Some(target);
                        }
                    }
                } else {
                    ui.painter().rect_filled(
                        rect,
                        0.0,
                        if self.app.state.dark {
                            Color32::from_gray(35)
                        } else {
                            Color32::WHITE
                        },
                    );
                    let placeholder = rect.intersect(ui.clip_rect());
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(placeholder), |ui| {
                        ui.centered_and_justified(|ui| {
                            if reader.failed.contains(&page) {
                                if ui.button("Retry page").clicked() {
                                    retry = Some(page);
                                }
                            } else {
                                ui.spinner();
                            }
                        });
                    });
                }
                ui.painter().text(
                    egui::pos2(rect.center().x, rect.bottom() + 12.0),
                    egui::Align2::CENTER_CENTER,
                    format!("Page {page}"),
                    egui::FontId::proportional(12.0),
                    reader_palette(ui.visuals()).muted,
                );
            }
        });
        let mut position = layout.position(result.state.offset.y);
        position.horizontal = result.state.offset.x / (width * zoom).max(1.0);
        self.app.handle(
            crate::app::action::Action::ViewportPosition(position),
            std::time::Instant::now(),
        );
        if let Some(page) = retry {
            self.app.reader.as_mut().unwrap().failed.remove(&page);
        }
        for &page in &visible {
            self.ui.touch_page(page, self.app.state.dark);
            if self.ui.pages.iter().any(|item| item.key.page == page) {
                self.queue_links(page);
            }
        }
        self.queue_continuous_pages(&visible);
        if let Some(target) = jump {
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
    links: &[crate::domain::document::PdfLink],
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

fn toolbar_style(ui: &mut egui::Ui) {
    let colors = reader_palette(ui.visuals());
    let widgets = &mut ui.visuals_mut().widgets;
    for widget in [
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
    ] {
        widget.bg_stroke = egui::Stroke::NONE;
        widget.rounding = 4.0.into();
        widget.expansion = 0.0;
    }
    widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
    widgets.inactive.bg_fill = Color32::TRANSPARENT;
    widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    widgets.hovered.bg_fill = colors.hover;
    widgets.hovered.weak_bg_fill = colors.hover;
    widgets.active.bg_fill = colors.sage;
    widgets.active.weak_bg_fill = colors.sage;
}

fn toolbar_group(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(4.0, 1.0))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), content);
        });
}

/// Reader accents use the shared home and document surface colors.
pub(super) fn reader_palette(visuals: &egui::Visuals) -> Palette {
    let mut colors = palette(visuals);
    if visuals.dark_mode {
        colors.ink = Color32::from_rgb(230, 236, 241);
        colors.muted = Color32::from_rgb(151, 168, 181);
        colors.teal = Color32::from_rgb(82, 161, 250);
    }
    colors
}

pub(super) fn reader_style(ui: &mut egui::Ui) {
    let colors = reader_palette(ui.visuals());
    ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);
    ui.spacing_mut().button_padding = Vec2::new(10.0, 7.0);
    ui.spacing_mut().interact_size.y = 32.0;
    let visuals = ui.visuals_mut();
    visuals.override_text_color = Some(colors.ink);
    visuals.extreme_bg_color = colors.light;
    visuals.selection.bg_fill = colors.sage;
    visuals.selection.stroke.color = colors.teal;
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
    ] {
        widget.rounding = 6.0.into();
        widget.bg_stroke = egui::Stroke::new(1.0_f32, colors.hairline);
    }
    visuals.widgets.inactive.bg_fill = colors.light;
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.hovered.bg_fill = colors.hover;
    visuals.widgets.active.bg_fill = colors.sage;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, colors.hairline);
}

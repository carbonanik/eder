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
                        Color32::from_rgb(24, 27, 31)
                    } else {
                        palette(&ctx.style().visuals).sage
                    })
                    .inner_margin(10.0),
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
                    .fill(palette(&ctx.style().visuals).paper)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                ui.spacing_mut().button_padding = Vec2::new(9.0, 7.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new(&self.ui.logo).fit_to_exact_size(Vec2::splat(24.0)));
                    let back = icon_label_button(ui, UiIcon::ArrowLeft, "Back", IconLabelStyle {
                        size: Vec2::new(80.0, 28.0),
                        fill: ui.visuals().widgets.inactive.bg_fill,
                        color: ui.visuals().text_color(),
                        text_size: 14.0,
                        rounding: 4.0,
                    });
                    if back.clicked() {
                        self.back();
                    }
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
                    let title_width = (ui.available_width() - 80.0).max(20.0);
                    ui.add_sized(
                        [title_width, 28.0],
                        egui::Label::new(RichText::new(&title).strong()).truncate(),
                    )
                    .on_hover_text(title);
                    ui.menu_button("⚙", |ui| {
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
                    })
                    .response
                    .on_hover_text("Reader settings");
                });
                ui.separator();
                if self.app.reader.is_none() {
                    return;
                }
                // Groups wrap as units on narrow windows, keeping navigation together.
                ui.horizontal_wrapped(|ui| {
                    toolbar_group(ui, |ui| self.reader_navigation_tools(ui));
                    toolbar_group(ui, |ui| self.reader_zoom_tools(ui));
                    toolbar_group(ui, |ui| self.annotation_tools(ui));
                    toolbar_group(ui, |ui| {
                        let mut dark = self.app.state.dark;
                        if ui
                            .checkbox(&mut dark, "Dark pages")
                            .on_hover_text("Invert PDF page colors")
                            .changed()
                        {
                            self.send(Task::Dark(dark));
                        }
                        let fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
                        if ui
                            .button(if fullscreen {
                                "Exit fullscreen"
                            } else {
                                "Fullscreen"
                            })
                            .on_hover_text("Toggle fullscreen (F11)")
                            .clicked()
                        {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
                        }
                    });
                });
                if self.ui.annotations.selection.is_some() && !self.ui.annotations.pan {
                    ui.horizontal_wrapped(|ui| {
                        ui.weak("Selected text");
                        self.selection_tools(ui);
                    });
                }
            });
    }

    fn reader_navigation_tools(&mut self, ui: &mut egui::Ui) {
        let reader = self.app.reader.as_ref().unwrap();
        let (page, total) = (reader.page, reader.total);
        if ui
            .add_enabled(page > 1, egui::Button::new("‹"))
            .on_hover_text("Previous page (←)")
            .clicked()
        {
            self.turn(page - 1);
        }
        let mut jump = page;
        if ui
            .add(
                egui::DragValue::new(&mut jump)
                    .range(1..=total)
                    .prefix("Page "),
            )
            .on_hover_text("Type a page number or drag to navigate")
            .changed()
        {
            self.turn(jump);
        }
        ui.weak(format!("/ {total}"));
        if ui
            .add_enabled(page < total, egui::Button::new("›"))
            .on_hover_text("Next page (→)")
            .clicked()
        {
            self.turn(page + 1);
        }
    }

    fn reader_zoom_tools(&mut self, ui: &mut egui::Ui) {
        if ui
            .button("−")
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
        if ui
            .button("+")
            .on_hover_text("Zoom in (Ctrl/Cmd++)")
            .clicked()
        {
            self.zoom_reader(0.1, false);
        }
    }

    fn reader_rail(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("reader_rail")
            .exact_width(48.0)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(palette(&ctx.style().visuals).paper)
                    .inner_margin(egui::Margin::symmetric(5.0, 10.0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                for (filter, icon, label) in [
                    (4, UiIcon::Grid, "Pages"),
                    (1, UiIcon::Bookmark, "Bookmarks"),
                    (0, UiIcon::Annotate, "Annotations"),
                    (3, UiIcon::Note, "Notes"),
                ] {
                    let active = if filter == 4 {
                        self.ui.annotations.pages_open
                    } else {
                        self.ui.annotations.open && self.ui.annotations.filter == filter
                    };
                    let colors = palette(ui.visuals());
                    if icon_label_button(
                        ui,
                        icon,
                        "",
                        IconLabelStyle {
                            size: Vec2::splat(36.0),
                            fill: if active {
                                colors.deep
                            } else {
                                Color32::TRANSPARENT
                            },
                            color: if active { colors.teal } else { colors.muted },
                            text_size: 14.0,
                            rounding: 6.0,
                        },
                    )
                    .on_hover_text(label)
                    .clicked()
                    {
                        if filter == 4 {
                            self.ui.annotations.pages_open = !active;
                            if !active && ctx.screen_rect().width() < 850.0 {
                                self.ui.annotations.open = false;
                            }
                        } else {
                            self.ui.annotations.open = !active;
                            self.ui.annotations.filter = filter;
                            if !active && ctx.screen_rect().width() < 850.0 {
                                self.ui.annotations.pages_open = false;
                            }
                        }
                    }
                }
                ui.add_enabled(false, egui::Button::new("≡").min_size(Vec2::splat(36.0)))
                    .on_disabled_hover_text(
                        "Document outline is not available with this PDF backend",
                    );
            });
        // Small windows display one expanded panel at a time.
        if ctx.screen_rect().width() < 850.0 && self.ui.annotations.open {
            self.ui.annotations.pages_open = false;
        }
        if self.ui.annotations.pages_open {
            egui::SidePanel::left("reader_pages")
                .default_width(172.0)
                .width_range(140.0..=240.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Pages");
                        if ui.small_button("×").on_hover_text("Close pages").clicked() {
                            self.ui.annotations.pages_open = false;
                        }
                    });
                    ui.separator();
                    let (page, total) = {
                        let r = self.app.reader.as_ref().unwrap();
                        (r.page, r.total)
                    };
                    // Virtualize long documents; use cached page previews where available.
                    egui::ScrollArea::vertical()
                        .id_salt("page_navigation")
                        .show_rows(ui, 124.0, total as usize, |ui, range| {
                            for index in range {
                                let target = index as u32 + 1;
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
                                let response = egui::Frame::none()
                                    .fill(if target == page {
                                        palette(ui.visuals()).sage
                                    } else {
                                        Color32::TRANSPARENT
                                    })
                                    .inner_margin(6.0)
                                    .rounding(4.0)
                                    .show(ui, |ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.set_min_height(112.0);
                                        ui.vertical_centered(|ui| {
                                            if let Some(texture) = preview {
                                                let size = texture.size_vec2();
                                                ui.add(
                                                    egui::Image::new(&texture).fit_to_exact_size(
                                                        size * (76.0 / size.y).min(100.0 / size.x),
                                                    ),
                                                );
                                            } else {
                                                let (rect, _) = ui.allocate_exact_size(
                                                    Vec2::new(54.0, 76.0),
                                                    egui::Sense::hover(),
                                                );
                                                ui.painter().rect_filled(
                                                    rect,
                                                    2.0,
                                                    palette(ui.visuals()).light,
                                                );
                                                ui.painter().text(
                                                    rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    target.to_string(),
                                                    egui::FontId::proportional(18.0),
                                                    palette(ui.visuals()).muted,
                                                );
                                            }
                                            ui.label(format!("Page {target}"));
                                        });
                                    })
                                    .response;
                                if ui
                                    .interact(
                                        response.rect,
                                        ui.id().with(target),
                                        egui::Sense::click(),
                                    )
                                    .on_hover_text(format!("Go to page {target}"))
                                    .clicked()
                                {
                                    self.turn(target);
                                }
                            }
                        });
                });
        }
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
                    palette(ui.visuals()).muted,
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

fn toolbar_group(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::none()
        .fill(palette(ui.visuals()).light)
        .rounding(5.0)
        .inner_margin(egui::Margin::symmetric(5.0, 2.0))
        .show(ui, |ui| {
            ui.horizontal(content);
        });
}

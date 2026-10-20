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
                    if let Some(r) = &self.app.reader {
                        ui.label(
                            RichText::new(r.path.file_name().unwrap_or_default().to_string_lossy())
                                .strong(),
                        );
                    }
                });
                if self.app.reader.is_none() {
                    return;
                }
                let (page, total, path, position) = {
                    let r = self.app.reader.as_ref().unwrap();
                    (
                        r.page,
                        r.total,
                        r.path.clone(),
                        r.reading_position(self.app.state.continuous_scroll),
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
                        if let Some(r) = &mut self.app.reader {
                            r.position_changed_at = None;
                        }
                    }
                    let mut dark = self.app.state.dark;
                    if ui.checkbox(&mut dark, "Dark pages").changed() {
                        self.send(Task::Dark(dark));
                    }
                    if let Some(r) = &mut self.app.reader {
                        if ui
                            .add(egui::Slider::new(&mut r.zoom, 0.5..=3.0).text("Zoom"))
                            .changed()
                        {
                            r.scroll_to =
                                Some(r.reading_position(self.app.state.continuous_scroll));
                            r.position_changed_at = Some(std::time::Instant::now());
                        }
                    }
                });
            });
        if self.app.reader.is_none() {
            return;
        }
        if !ctx.wants_keyboard_input() {
            let page = self.app.reader.as_ref().unwrap().page;
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
                self.turn(self.app.reader.as_ref().unwrap().total);
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
                    .fill(if self.app.state.dark {
                        Color32::from_rgb(24, 27, 31)
                    } else {
                        SAGE
                    })
                    .inner_margin(18.0),
            )
            .show(ctx, |ui| {
                if self.app.state.continuous_scroll {
                    self.continuous_reader_ui(ui);
                    return;
                }
                let desired = Self::page_edge(ui.available_size(),
                    self.app.reader.as_ref().unwrap().zoom, ui.ctx().pixels_per_point(), false);
                self.set_page_edge(desired);
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
                            ui.horizontal(|ui| {
                                ui.add_space(((ui.available_width() - size.x) / 2.0).max(0.0));
                                let image = ui.add(egui::Image::new(t).fit_to_exact_size(size));
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
    pub(crate) fn continuous_reader_ui(&mut self, ui: &mut egui::Ui) {
        use crate::app::reader::layout::PageLayout;
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
                    MUTED,
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

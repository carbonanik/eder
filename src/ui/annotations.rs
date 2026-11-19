use super::reader::{reader_palette, reader_style};
use super::*;
use crate::domain::annotation::{Annotation, TextWord};

#[derive(Default)]
pub(crate) struct AnnotationUi {
    pub open: bool,
    pub pages_open: bool,
    pub navigation_tab: usize,
    pub page_query: String,
    pub pan: bool,
    pub thumbnails: std::collections::HashMap<u32, Option<TextureHandle>>,
    pub thumbnail_pending: std::collections::HashSet<u32>,
    pub filter: usize,
    pub selection: Option<Selection>,
    pub highlight_color: u8,
    pub hovered_highlight: Option<(i64, usize)>,
    pub remove_highlight: Option<i64>,
    pub editing: Option<Annotation>,
    pub export: Option<(PathBuf, std::sync::mpsc::Receiver<Option<PathBuf>>)>,
}
impl AnnotationUi {
    pub fn selected_text(&self, reader: Option<&crate::app::reader::Reader>) -> Option<String> {
        let selection = self.selection.as_ref()?;
        let words = reader?.text.get(&selection.page)?;
        (selection.start.max(selection.end) < words.len())
            .then(|| selection.annotation(words, 0).text)
            .filter(|text| !text.is_empty())
    }
}
#[derive(Clone)]
pub(crate) struct Selection {
    pub page: u32,
    pub start: usize,
    pub end: usize,
    /// Character boundaries within the anchor and focus words (end exclusive).
    pub start_offset: usize,
    pub end_offset: usize,
}
impl Selection {
    fn annotation(&self, words: &[TextWord], color: u8) -> Annotation {
        let anchor = (self.start, self.start_offset);
        let focus = (self.end, self.end_offset);
        let (start, end) = (anchor.min(focus), anchor.max(focus));
        let mut text = String::new();
        let mut bounds = Vec::new();
        for (index, word) in words.iter().enumerate().take(end.0 + 1).skip(start.0) {
            let count = word.text.chars().count();
            let from = if index == start.0 {
                start.1.min(count)
            } else {
                0
            };
            let to = if index == end.0 {
                end.1.min(count)
            } else {
                count
            };
            if from >= to {
                continue;
            }
            if !text.is_empty() {
                text.push(' ');
            }
            text.extend(word.text.chars().skip(from).take(to - from));
            let [left, top, right, bottom] = word.bounds;
            let width = right - left;
            bounds.push([
                left + width * from as f32 / count as f32,
                top,
                left + width * to as f32 / count as f32,
                bottom,
            ]);
        }
        Annotation {
            id: 0,
            page: self.page,
            text,
            note: String::new(),
            color: Some(color),
            bounds: continuous_bounds(&bounds),
        }
    }
}

/// Join neighboring boxes on the same line, including their intervening spaces.
/// Large gaps (such as separate columns) and separate lines stay separate.
fn continuous_bounds(bounds: &[[f32; 4]]) -> Vec<[f32; 4]> {
    let mut lines: Vec<[f32; 4]> = Vec::new();
    for &next in bounds {
        if let Some(last) = lines.last_mut() {
            let height = (last[3] - last[1]).min(next[3] - next[1]);
            let overlap = last[3].min(next[3]) - last[1].max(next[1]);
            let gap = (next[0] - last[2]).max(last[0] - next[2]);
            if overlap >= height * 0.5 && gap <= height * 2.0 {
                last[0] = last[0].min(next[0]);
                last[1] = last[1].min(next[1]);
                last[2] = last[2].max(next[2]);
                last[3] = last[3].max(next[3]);
                continue;
            }
        }
        lines.push(next);
    }
    lines
}

fn character_offset(page: egui::Rect, word: &TextWord, pos: egui::Pos2) -> usize {
    let rect = word_rect(page, word.bounds);
    // Poppler supplies word bounds; interpolate character boundaries within them.
    let fraction = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    (fraction * word.text.chars().count() as f32).round() as usize
}

fn color(index: u8) -> Color32 {
    let [r, g, b] = match index {
        1 => [90, 210, 130],
        2 => [90, 170, 255],
        3 => [245, 120, 180],
        _ => [255, 215, 65],
    };
    Color32::from_rgba_unmultiplied(r, g, b, 85)
}
fn word_rect(page: egui::Rect, bounds: [f32; 4]) -> egui::Rect {
    egui::Rect::from_min_max(
        page.min + page.size() * Vec2::new(bounds[0], bounds[1]),
        page.min + page.size() * Vec2::new(bounds[2], bounds[3]),
    )
}
/// One page interaction owns selection; persisted and temporary overlays share geometry.
pub(crate) fn text_overlay(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    page: u32,
    words: &[TextWord],
    annotations: &[Annotation],
    selection: &mut Option<Selection>,
    hovered_highlight: &mut Option<(i64, usize)>,
    remove_highlight: &mut Option<i64>,
    pan: bool,
) {
    let mut highlight_segments = Vec::new();
    for item in annotations.iter().filter(|a| a.page == page) {
        if let Some(c) = item.color {
            for (index, bounds) in continuous_bounds(&item.bounds).into_iter().enumerate() {
                let highlight_rect = word_rect(rect, bounds);
                ui.painter().rect_filled(highlight_rect, 0.0, color(c));
                highlight_segments.push((item.id, index, highlight_rect, item.note.as_str()));
            }
        }
    }
    if pan {
        let response = ui
            .interact(
                rect.intersect(ui.clip_rect()),
                ui.id().with(("pdf-pan", page)),
                egui::Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::Grab);
        if response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            ui.scroll_with_delta(ui.input(|i| i.pointer.delta()));
        }
        return;
    }
    if words.is_empty() {
        return;
    }
    let response = ui.interact(
        rect.intersect(ui.clip_rect()),
        ui.id().with(("pdf-text", page)),
        egui::Sense::click_and_drag(),
    );
    for (id, index, bounds, note) in &highlight_segments {
        let response = ui.interact(
            bounds.intersect(ui.clip_rect()),
            ui.id().with(("highlight-hover", page, id, index)),
            egui::Sense::hover(),
        );
        if !note.is_empty() {
            response.on_hover_text(*note);
        }
    }
    let pointer = ui.input(|input| input.pointer.hover_pos());
    let direct_hover = pointer.and_then(|pos| {
        highlight_segments
            .iter()
            .find(|(_, _, bounds, _)| bounds.contains(pos))
            .map(|(id, index, _, _)| (*id, *index))
    });
    let retained_hover = hovered_highlight.and_then(|(id, index)| {
        let (_, _, bounds, _) =
            highlight_segments
                .iter()
                .find(|(candidate_id, candidate_index, _, _)| {
                    *candidate_id == id && *candidate_index == index
                })?;
        let button_rect = highlight_remove_button_rect(rect, *bounds);
        pointer
            .is_some_and(|pos| bounds.expand(32.0).contains(pos) || button_rect.contains(pos))
            .then_some((id, index))
    });
    *hovered_highlight = direct_hover.or(retained_hover);
    let hit = |pos: egui::Pos2| {
        words
            .iter()
            .position(|w| word_rect(rect, w.bounds).contains(pos))
    };
    if response.hovered() && ui.input(|i| i.pointer.hover_pos()).and_then(hit).is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
    if response.drag_started() {
        if let Some((index, pos)) = ui
            .input(|i| i.pointer.press_origin())
            .and_then(|pos| hit(pos).map(|index| (index, pos)))
        {
            let offset = character_offset(rect, &words[index], pos);
            *selection = Some(Selection {
                page,
                start: index,
                end: index,
                start_offset: offset,
                end_offset: offset,
            });
        } else {
            *selection = None;
        }
    }
    if response.dragged() {
        if let (Some(selected), Some(pos)) = (
            selection.as_mut().filter(|s| s.page == page),
            response.interact_pointer_pos(),
        ) {
            // Continue across spaces by finding the closest word box.
            if let Some((index, _)) = words.iter().enumerate().min_by(|(_, a), (_, b)| {
                word_rect(rect, a.bounds)
                    .distance_to_pos(pos)
                    .total_cmp(&word_rect(rect, b.bounds).distance_to_pos(pos))
            }) {
                selected.end = index;
                selected.end_offset = character_offset(rect, &words[index], pos);
            }
        }
    }
    if response.double_clicked() {
        *selection = response
            .interact_pointer_pos()
            .and_then(hit)
            .map(|index| Selection {
                page,
                start: index,
                end: index,
                start_offset: 0,
                end_offset: words[index].text.chars().count(),
            });
    } else if response.clicked() {
        *selection = None;
    }
    if let Some(selected) = selection.as_ref().filter(|s| s.page == page) {
        for bounds in selected.annotation(words, 0).bounds {
            ui.painter().rect_filled(
                word_rect(rect, bounds),
                0.0,
                Color32::from_rgba_unmultiplied(50, 130, 240, 90),
            );
        }
    }
    if let Some((id, index)) = *hovered_highlight {
        if let Some((_, _, bounds, _)) =
            highlight_segments
                .iter()
                .find(|(candidate_id, candidate_index, _, _)| {
                    *candidate_id == id && *candidate_index == index
                })
        {
            let response = ui.put(
                highlight_remove_button_rect(rect, *bounds),
                egui::Button::new(RichText::new(UiIcon::Delete.glyph().to_string()).font(
                    egui::FontId::new(16.0, egui::FontFamily::Name(MATERIAL_SYMBOLS_FONT.into())),
                )),
            );
            if response.on_hover_text("Remove highlight").clicked() {
                *remove_highlight = Some(id);
                *hovered_highlight = None;
            }
        }
    }
}

fn highlight_remove_button_rect(page: egui::Rect, bounds: egui::Rect) -> egui::Rect {
    let size = Vec2::splat(24.0);
    let min = egui::pos2(bounds.right() + 4.0, bounds.top() - 4.0);
    let min = egui::pos2(
        min.x
            .clamp(page.left(), (page.right() - size.x).max(page.left())),
        min.y
            .clamp(page.top(), (page.bottom() - size.y).max(page.top())),
    );
    egui::Rect::from_min_size(min, size)
}
impl Eder {
    pub(crate) fn annotation_tools(&mut self, ui: &mut egui::Ui) {
        let Some(reader) = &self.app.reader else {
            return;
        };
        let path = reader.path.clone();
        let bookmark = reader
            .annotations
            .iter()
            .find(|a| a.page == reader.page && a.color.is_none())
            .map(|a| a.id);
        let colors = reader_palette(ui.visuals());
        if toolbar_icon_label_button(
            ui,
            UiIcon::Hand,
            "",
            IconLabelStyle {
                size: Vec2::splat(32.0),
                fill: if self.ui.annotations.pan {
                    colors.sage
                } else {
                    Color32::TRANSPARENT
                },
                color: if self.ui.annotations.pan {
                    colors.teal
                } else {
                    colors.ink
                },
                text_size: 13.0,
                rounding: 6.0,
            },
        )
        .on_hover_text("Pan mode: drag the page to move around")
        .clicked()
        {
            self.ui.annotations.pan = true;
        }
        if toolbar_icon_label_button(
            ui,
            UiIcon::Text,
            "",
            IconLabelStyle {
                size: Vec2::splat(32.0),
                fill: if !self.ui.annotations.pan {
                    colors.sage
                } else {
                    Color32::TRANSPARENT
                },
                color: if !self.ui.annotations.pan {
                    colors.teal
                } else {
                    colors.ink
                },
                text_size: 13.0,
                rounding: 6.0,
            },
        )
        .on_hover_text("Text selection mode")
        .clicked()
        {
            self.ui.annotations.pan = false;
        }
        if toolbar_icon_label_button(
            ui,
            UiIcon::Bookmark,
            "",
            IconLabelStyle {
                size: Vec2::splat(32.0),
                fill: if bookmark.is_some() {
                    colors.sage
                } else {
                    Color32::TRANSPARENT
                },
                color: if bookmark.is_some() {
                    colors.teal
                } else {
                    colors.ink
                },
                text_size: 13.0,
                rounding: 6.0,
            },
        )
        .on_hover_text("Toggle bookmark for this page")
        .clicked()
        {
            let task = if let Some(id) = bookmark {
                Task::DeleteAnnotation(path.clone(), id)
            } else {
                Task::SaveAnnotation(
                    path.clone(),
                    Box::new(Annotation {
                        id: 0,
                        page: reader.page,
                        text: String::new(),
                        note: String::new(),
                        color: None,
                        bounds: vec![],
                    }),
                )
            };
            self.send(task);
        }
    }
    pub(crate) fn selection_tools(&mut self, ui: &mut egui::Ui) {
        let Some(reader) = &self.app.reader else {
            return;
        };
        let path = reader.path.clone();
        let selected = self.ui.annotations.selection.as_ref().and_then(|s| {
            self.app
                .reader
                .as_ref()?
                .text
                .get(&s.page)
                .filter(|w| s.start.max(s.end) < w.len())
                .map(|w| s.annotation(w, 0))
        });
        if let Some(item) =
            selected.filter(|item| !self.ui.annotations.pan && !item.text.is_empty())
        {
            let colors = reader_palette(ui.visuals());
            if toolbar_icon_label_button(
                ui,
                UiIcon::Copy,
                "",
                IconLabelStyle {
                    size: Vec2::splat(32.0),
                    fill: colors.light,
                    color: colors.ink,
                    text_size: 13.0,
                    rounding: 6.0,
                },
            )
            .on_hover_text("Copy selected text")
            .clicked()
            {
                ui.ctx().copy_text(item.text.clone());
            }
            let highlight_color = self.ui.annotations.highlight_color;
            if toolbar_icon_label_button(
                ui,
                UiIcon::Annotate,
                "",
                IconLabelStyle {
                    size: Vec2::splat(32.0),
                    fill: color(highlight_color).gamma_multiply(0.2),
                    color: color(highlight_color).to_opaque(),
                    text_size: 13.0,
                    rounding: 6.0,
                },
            )
            .on_hover_text("Highlight selected text")
            .clicked()
            {
                self.send(Task::SaveAnnotation(
                    path.clone(),
                    Box::new(Annotation {
                        color: Some(highlight_color),
                        ..item.clone()
                    }),
                ));
            }

            let color_names = ["Yellow", "Green", "Blue", "Pink"];
            let chosen_color = color(highlight_color).to_opaque();
            let color_button = ui.menu_button("", |menu| {
                menu.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                menu.horizontal(|row| {
                    for index in 0..4_u8 {
                        let response = row
                            .add(egui::Button::new("").min_size(Vec2::splat(30.0)))
                            .on_hover_text(color_names[index as usize]);
                        row.painter().circle_filled(
                            response.rect.center(),
                            8.0,
                            color(index).to_opaque(),
                        );
                        if response.clicked() {
                            self.ui.annotations.highlight_color = index;
                            row.close_menu();
                        }
                    }
                });
            });
            ui.painter()
                .circle_filled(color_button.response.rect.center(), 8.0, chosen_color);
            color_button
                .response
                .on_hover_text("Choose highlight color");

            let colors = reader_palette(ui.visuals());
            if toolbar_icon_label_button(
                ui,
                UiIcon::Note,
                "",
                IconLabelStyle {
                    size: Vec2::splat(32.0),
                    fill: colors.light,
                    color: colors.ink,
                    text_size: 13.0,
                    rounding: 6.0,
                },
            )
            .on_hover_text("Add note")
            .clicked()
            {
                self.ui.annotations.editing = Some(item);
            }
        }
    }
    fn navigate_to_annotation(&mut self, item: &Annotation) {
        self.turn(item.page);
        if let Some(reader) = &mut self.app.reader {
            reader.scroll_to = Some(ReadingPosition::content(
                item.page,
                item.bounds.first().map_or(0.0, |b| (b[1] - 0.08).max(0.0)),
                reader.zoom,
                item.bounds.first().map_or(0.0, |b| (b[0] - 0.08).max(0.0)),
            ));
        }
    }

    pub(crate) fn annotation_panel(&mut self, ctx: &egui::Context) {
        if let Some((source, rx)) = &self.ui.annotations.export {
            if let Ok(destination) = rx.try_recv() {
                let source = source.clone();
                self.ui.annotations.export = None;
                if let Some(destination) = destination {
                    self.send(Task::ExportAnnotations(source, destination));
                }
            }
        }
        if self.ui.annotations.open {
            let max_width = (ctx.available_rect().width() - 80.0).clamp(200.0, 420.0);
            egui::SidePanel::right("annotations")
                .default_width(300.0_f32.min(max_width))
                .frame(
                    egui::Frame::none()
                        .fill(reader_palette(&ctx.style().visuals).paper)
                        .inner_margin(14.0),
                )
                .width_range(200.0..=max_width)
                .resizable(true)
                .show(ctx, |ui| {
                    reader_style(ui);
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 12.0);
                    let colors = reader_palette(ui.visuals());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Annotations")
                                .size(16.0)
                                .strong()
                                .color(colors.ink),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_label_button(
                                ui,
                                UiIcon::Close,
                                "",
                                IconLabelStyle {
                                    size: Vec2::splat(32.0),
                                    fill: Color32::TRANSPARENT,
                                    color: colors.ink,
                                    text_size: 13.0,
                                    rounding: 6.0,
                                },
                            )
                            .on_hover_text("Close annotations (Ctrl/Cmd+Shift+A)")
                            .clicked()
                            {
                                self.ui.annotations.open = false;
                            }
                            ui.menu_button(
                                egui::RichText::new(UiIcon::MoreVert.glyph().to_string()).font(
                                    egui::FontId::new(
                                        18.0,
                                        egui::FontFamily::Name(MATERIAL_SYMBOLS_FONT.into()),
                                    ),
                                ),
                                |menu| {
                                    if icon_label_button(
                                        menu,
                                        UiIcon::Download,
                                        "Export Markdown",
                                        IconLabelStyle {
                                            size: Vec2::new(180.0, 32.0),
                                            fill: Color32::TRANSPARENT,
                                            color: colors.ink,
                                            text_size: 13.0,
                                            rounding: 6.0,
                                        },
                                    )
                                    .on_hover_text("Export annotations as Markdown")
                                    .clicked()
                                        && self.ui.annotations.export.is_none()
                                    {
                                        let path = self.app.reader.as_ref().unwrap().path.clone();
                                        let (tx, rx) = std::sync::mpsc::channel();
                                        let context = ctx.clone();
                                        self.ui.annotations.export = Some((path, rx));
                                        std::thread::spawn(move || {
                                            let file = rfd::FileDialog::new()
                                                .add_filter("Markdown", &["md"])
                                                .set_file_name("annotations.md")
                                                .save_file();
                                            let _ = tx.send(file);
                                            context.request_repaint();
                                        });
                                        menu.close_menu();
                                    }
                                },
                            )
                            .response
                            .on_hover_text("Annotation menu");
                        });
                    });
                    ui.add_space(4.0);
                    egui::Frame::none()
                        .fill(colors.light)
                        .rounding(7.0)
                        .inner_margin(3.0)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            ui.horizontal(|ui| {
                                for (index, label) in [(0, "All"), (2, "Highlights"), (3, "Notes")]
                                {
                                    let count = self
                                        .app
                                        .reader
                                        .as_ref()
                                        .unwrap()
                                        .annotations
                                        .iter()
                                        .filter(|a| match index {
                                            1 => a.color.is_none(),
                                            2 => a.color.is_some(),
                                            3 => !a.note.is_empty(),
                                            _ => true,
                                        })
                                        .count();
                                    if ui
                                        .selectable_label(
                                            self.ui.annotations.filter == index,
                                            format!("{label}  {count}"),
                                        )
                                        .clicked()
                                    {
                                        self.ui.annotations.filter = index;
                                    }
                                }
                            });
                        });
                    ui.add_space(8.0);
                    let reader = self.app.reader.as_ref().unwrap();
                    let path = reader.path.clone();
                    let items: Vec<_> = reader
                        .annotations
                        .iter()
                        .filter(|a| match self.ui.annotations.filter {
                            1 => a.color.is_none(),
                            2 => a.color.is_some(),
                            3 => !a.note.is_empty(),
                            _ => true,
                        })
                        .cloned()
                        .collect();
                    if items.is_empty() {
                        ui.weak("No annotations in this view.");
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for item in items {
                            let mut navigate = false;
                            let colors = reader_palette(ui.visuals());
                            let current = self
                                .app
                                .reader
                                .as_ref()
                                .is_some_and(|r| r.page == item.page);
                            egui::Frame::none()
                                .fill(if current { colors.sage } else { colors.light })
                                .stroke(egui::Stroke::new(
                                    1.0_f32,
                                    if current {
                                        colors.teal
                                    } else {
                                        colors.hairline
                                    },
                                ))
                                .rounding(9.0)
                                .inner_margin(12.0)
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        let tint = item
                                            .color
                                            .map(|c| color(c).to_opaque())
                                            .unwrap_or(colors.teal);
                                        let (dot, _) = ui.allocate_exact_size(
                                            Vec2::splat(10.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().circle_filled(dot.center(), 5.0, tint);
                                        navigate |= ui
                                            .add(
                                                egui::Label::new(
                                                    RichText::new(format!("Page {}", item.page))
                                                        .size(13.0),
                                                )
                                                .sense(egui::Sense::click()),
                                            )
                                            .on_hover_text("Go to annotation")
                                            .clicked();
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.menu_button(
                                                    egui::RichText::new(
                                                        UiIcon::MoreVert.glyph().to_string(),
                                                    )
                                                    .font(egui::FontId::new(
                                                        18.0,
                                                        egui::FontFamily::Name(
                                                            MATERIAL_SYMBOLS_FONT.into(),
                                                        ),
                                                    )),
                                                    |ui| {
                                                        if item.color.is_some()
                                                            && icon_label_button(
                                                                ui,
                                                                UiIcon::Edit,
                                                                "Edit note / color",
                                                                IconLabelStyle {
                                                                    size: Vec2::new(180.0, 32.0),
                                                                    fill: Color32::TRANSPARENT,
                                                                    color: colors.ink,
                                                                    text_size: 13.0,
                                                                    rounding: 6.0,
                                                                },
                                                            )
                                                            .clicked()
                                                        {
                                                            self.ui.annotations.editing =
                                                                Some(item.clone());
                                                            ui.close_menu();
                                                        }
                                                        if !item.note.is_empty()
                                                            && icon_label_button(
                                                                ui,
                                                                UiIcon::Delete,
                                                                "Delete note",
                                                                IconLabelStyle {
                                                                    size: Vec2::new(180.0, 32.0),
                                                                    fill: Color32::TRANSPARENT,
                                                                    color: colors.ink,
                                                                    text_size: 13.0,
                                                                    rounding: 6.0,
                                                                },
                                                            )
                                                            .clicked()
                                                        {
                                                            self.send(Task::SaveAnnotation(
                                                                path.clone(),
                                                                Box::new(Annotation {
                                                                    note: String::new(),
                                                                    ..item.clone()
                                                                }),
                                                            ));
                                                            ui.close_menu();
                                                        }
                                                        if icon_label_button(
                                                            ui,
                                                            UiIcon::Delete,
                                                            "Delete annotation",
                                                            IconLabelStyle {
                                                                size: Vec2::new(180.0, 32.0),
                                                                fill: Color32::TRANSPARENT,
                                                                color: colors.ink,
                                                                text_size: 13.0,
                                                                rounding: 6.0,
                                                            },
                                                        )
                                                        .clicked()
                                                        {
                                                            self.send(Task::DeleteAnnotation(
                                                                path.clone(),
                                                                item.id,
                                                            ));
                                                            ui.close_menu();
                                                        }
                                                    },
                                                )
                                                .response
                                                .on_hover_text("Annotation actions");
                                            },
                                        );
                                    });
                                    for (text, italic) in [(&item.text, false), (&item.note, true)]
                                    {
                                        if text.is_empty() {
                                            continue;
                                        }
                                        let preview: String = text.chars().take(180).collect();
                                        let preview = if text.chars().count() > 180 {
                                            format!("{preview}…")
                                        } else {
                                            preview
                                        };
                                        let rich =
                                            RichText::new(preview).size(13.0).color(colors.muted);
                                        navigate |= ui
                                            .add(
                                                egui::Label::new(if italic {
                                                    rich.italics()
                                                } else {
                                                    rich
                                                })
                                                .wrap()
                                                .sense(egui::Sense::click()),
                                            )
                                            .on_hover_text(text)
                                            .clicked();
                                    }
                                });
                            ui.add_space(4.0);
                            if navigate {
                                self.navigate_to_annotation(&item);
                            }
                        }
                    });
                });
        }
        if let Some(mut item) = self.ui.annotations.editing.take() {
            let mut open = true;
            let mut save = false;
            let mut cancel = false;
            egui::Window::new("Highlight note")
                .open(&mut open)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(&item.text);
                    ui.add(
                        egui::TextEdit::multiline(&mut item.note)
                            .hint_text("Write a note…")
                            .desired_rows(5),
                    );
                    ui.horizontal(|ui| {
                        for (index, name) in ["Yellow", "Green", "Blue", "Pink"].iter().enumerate()
                        {
                            if ui
                                .selectable_label(item.color == Some(index as u8), *name)
                                .clicked()
                            {
                                item.color = Some(index as u8);
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        save = ui.button("Save").clicked();
                        cancel = ui.button("Cancel").clicked();
                    });
                });
            if save {
                let path = self.app.reader.as_ref().unwrap().path.clone();
                if !self.send(Task::SaveAnnotation(path, Box::new(item.clone()))) {
                    self.ui.annotations.editing = Some(item);
                }
            } else if open && !cancel {
                self.ui.annotations.editing = Some(item);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_unicode_selection_copies_and_highlights_in_both_directions() {
        let words = [TextWord {
            text: "aé😀bc".into(),
            bounds: [0.1, 0.1, 0.6, 0.2],
        }];
        for (start_offset, end_offset) in [(1, 3), (3, 1)] {
            let item = Selection {
                page: 1,
                start: 0,
                end: 0,
                start_offset,
                end_offset,
            }
            .annotation(&words, 0);
            assert_eq!(item.text, "é😀");
            assert_eq!(item.bounds.len(), 1);
            assert!((item.bounds[0][0] - 0.2).abs() < 0.00001);
            assert!((item.bounds[0][2] - 0.4).abs() < 0.00001);
            assert!(item.validate().is_ok());
        }
    }

    #[test]
    fn highlight_connects_spaces_but_keeps_lines_and_columns_separate() {
        let bounds = [
            [0.1, 0.1, 0.2, 0.12],
            [0.21, 0.1, 0.3, 0.12],
            [0.1, 0.15, 0.25, 0.17],
            [0.7, 0.15, 0.8, 0.17],
        ];
        assert_eq!(
            continuous_bounds(&bounds),
            vec![[0.1, 0.1, 0.3, 0.12], bounds[2], bounds[3],]
        );
    }

    #[test]
    fn remove_button_stays_next_to_highlight_within_page_bounds() {
        let page = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), Vec2::new(200.0, 300.0));
        let highlight = egui::Rect::from_min_max(egui::pos2(40.0, 80.0), egui::pos2(90.0, 100.0));
        let button = highlight_remove_button_rect(page, highlight);
        assert!(page.contains(button.min));
        assert!(page.contains(button.max));
        assert_eq!(button.size(), Vec2::splat(24.0));
    }

    #[test]
    fn hovering_highlight_shows_remove_button_and_click_marks_it_for_deletion() {
        fn frame(
            ctx: &egui::Context,
            rect: egui::Rect,
            words: &[TextWord],
            annotations: &[Annotation],
            selection: &mut Option<Selection>,
            hovered: &mut Option<(i64, usize)>,
            remove: &mut Option<i64>,
            events: Vec<egui::Event>,
        ) {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::splat(1000.0),
                )),
                events,
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    text_overlay(
                        ui,
                        rect,
                        1,
                        words,
                        annotations,
                        selection,
                        hovered,
                        remove,
                        false,
                    );
                });
            });
        }

        let ctx = egui::Context::default();
        crate::ui::icons::install(&ctx);
        let rect = egui::Rect::from_min_size(egui::pos2(30.0, 30.0), Vec2::new(200.0, 300.0));
        let words = [TextWord {
            text: "hello".into(),
            bounds: [0.1, 0.1, 0.5, 0.2],
        }];
        let annotations = [Annotation {
            id: 41,
            page: 1,
            text: "hello".into(),
            note: "A useful note".into(),
            color: Some(0),
            bounds: vec![words[0].bounds],
        }];
        let highlight_rect = word_rect(rect, words[0].bounds);
        let button_rect = highlight_remove_button_rect(rect, highlight_rect);
        let mut selection = None;
        let mut hovered = None;
        let mut remove = None;

        frame(
            &ctx,
            rect,
            &words,
            &annotations,
            &mut selection,
            &mut hovered,
            &mut remove,
            vec![egui::Event::PointerMoved(highlight_rect.center())],
        );
        assert_eq!(hovered, Some((41, 0)));
        frame(
            &ctx,
            rect,
            &words,
            &annotations,
            &mut selection,
            &mut hovered,
            &mut remove,
            vec![
                egui::Event::PointerMoved(button_rect.center()),
                egui::Event::PointerButton {
                    pos: button_rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        frame(
            &ctx,
            rect,
            &words,
            &annotations,
            &mut selection,
            &mut hovered,
            &mut remove,
            vec![egui::Event::PointerButton {
                pos: button_rect.center(),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(remove, Some(41));
    }

    #[test]
    fn drag_selects_characters_at_different_scales() {
        let words = vec![
            TextWord {
                text: "Hello".into(),
                bounds: [0.1, 0.1, 0.3, 0.2],
            },
            TextWord {
                text: "world".into(),
                bounds: [0.4, 0.1, 0.6, 0.2],
            },
        ];
        let annotations = [Annotation {
            id: 1,
            page: 1,
            text: "Hello".into(),
            note: "A note".into(),
            color: Some(0),
            bounds: vec![words[0].bounds],
        }];
        for (scale, reverse) in [(1.0, false), (1.0, true), (2.0, false), (2.0, true)] {
            let ctx = egui::Context::default();
            crate::ui::icons::install(&ctx);
            let rect =
                egui::Rect::from_min_size(egui::pos2(30.0, 30.0), Vec2::new(200.0, 300.0) * scale);
            let mut selection = None;
            let first = word_rect(rect, words[0].bounds).center();
            let last = word_rect(rect, words[1].bounds).center();
            let (origin, focus) = if reverse {
                (last, first)
            } else {
                (first, last)
            };
            let mut frame = |events: Vec<egui::Event>| {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::splat(1000.0),
                    )),
                    events,
                    ..Default::default()
                };
                let _ = ctx.run(input, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        text_overlay(
                            ui,
                            rect,
                            1,
                            &words,
                            &annotations,
                            &mut selection,
                            &mut None,
                            &mut None,
                            false,
                        );
                    });
                });
            };
            frame(vec![]);
            frame(vec![
                egui::Event::PointerMoved(origin),
                egui::Event::PointerButton {
                    pos: origin,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ]);
            frame(vec![egui::Event::PointerMoved(focus)]);
            frame(vec![egui::Event::PointerButton {
                pos: focus,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }]);
            let selected = selection.unwrap().annotation(&words, 2);
            assert_eq!(selected.text, "lo wor");
            assert_eq!(selected.bounds.len(), 1);
        }
    }
}

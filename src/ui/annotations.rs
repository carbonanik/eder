use super::*;
use crate::domain::annotation::{Annotation, TextWord};

#[derive(Default)]
pub(crate) struct AnnotationUi {
    pub open: bool,
    pub pages_open: bool,
    pub pan: bool,
    pub thumbnails: std::collections::HashMap<u32, Option<TextureHandle>>,
    pub thumbnail_pending: std::collections::HashSet<u32>,
    pub filter: usize,
    pub selection: Option<Selection>,
    pub editing: Option<Annotation>,
    pub export: Option<(PathBuf, std::sync::mpsc::Receiver<Option<PathBuf>>)>,
}
impl AnnotationUi {
    pub fn selected_text(&self, reader: Option<&crate::app::reader::Reader>) -> Option<String> {
        let selection = self.selection.as_ref()?;
        let words = reader?.text.get(&selection.page)?;
        (selection.start.max(selection.end) < words.len())
            .then(|| selection.annotation(words, 0).text)
    }
}
#[derive(Clone)]
pub(crate) struct Selection {
    pub page: u32,
    pub start: usize,
    pub end: usize,
}
impl Selection {
    fn annotation(&self, words: &[TextWord], color: u8) -> Annotation {
        let selected = &words[self.start.min(self.end)..=self.start.max(self.end)];
        Annotation {
            id: 0,
            page: self.page,
            text: selected
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            note: String::new(),
            color: Some(color),
            bounds: selected.iter().map(|w| w.bounds).collect(),
        }
    }
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
    pan: bool,
) {
    for item in annotations.iter().filter(|a| a.page == page) {
        if let Some(c) = item.color {
            for &bounds in &item.bounds {
                ui.painter()
                    .rect_filled(word_rect(rect, bounds), 0.0, color(c));
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
    let hit = |pos: egui::Pos2| {
        words
            .iter()
            .position(|w| word_rect(rect, w.bounds).contains(pos))
    };
    if response.hovered() && ui.input(|i| i.pointer.hover_pos()).and_then(hit).is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
    if response.drag_started() {
        if let Some(index) = ui.input(|i| i.pointer.press_origin()).and_then(hit) {
            *selection = Some(Selection {
                page,
                start: index,
                end: index,
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
            }
        }
    }
    if response.clicked() || response.double_clicked() {
        *selection = response
            .interact_pointer_pos()
            .and_then(hit)
            .map(|index| Selection {
                page,
                start: index,
                end: index,
            });
    }
    if let Some(selected) = selection.as_ref().filter(|s| s.page == page) {
        for word in words
            .iter()
            .take(selected.start.max(selected.end) + 1)
            .skip(selected.start.min(selected.end))
        {
            ui.painter().rect_filled(
                word_rect(rect, word.bounds),
                0.0,
                Color32::from_rgba_unmultiplied(50, 130, 240, 90),
            );
        }
    }
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
        ui.selectable_value(&mut self.ui.annotations.pan, true, "Pan")
            .on_hover_text("Drag the page to move around");
        ui.selectable_value(&mut self.ui.annotations.pan, false, "Text")
            .on_hover_text("Drag over text to select, copy, highlight or annotate");
        if ui
            .button(if bookmark.is_some() { "★" } else { "☆" })
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
        if let Some(item) = selected.filter(|_| !self.ui.annotations.pan) {
            if ui.button("Copy text").clicked() {
                ui.ctx().copy_text(item.text.clone());
            }
            for (index, name) in ["Yellow", "Green", "Blue", "Pink"].iter().enumerate() {
                if ui
                    .add(
                        egui::Button::new(RichText::new("●").color(color(index as u8).to_opaque()))
                            .min_size(Vec2::splat(28.0)),
                    )
                    .on_hover_text(format!(
                        "Highlight selected text in {}",
                        name.to_lowercase()
                    ))
                    .clicked()
                {
                    self.send(Task::SaveAnnotation(
                        path.clone(),
                        Box::new(Annotation {
                            color: Some(index as u8),
                            ..item.clone()
                        }),
                    ));
                }
            }
            if ui.button("Add note").clicked() {
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
                .default_width(310.0_f32.min(max_width))
                .width_range(200.0..=max_width)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 8.0);
                    ui.horizontal(|ui| {
                        ui.heading("Annotations");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button("×")
                                .on_hover_text("Close annotations (Ctrl/Cmd+Shift+A)")
                                .clicked()
                            {
                                self.ui.annotations.open = false;
                            }
                        });
                    });
                    ui.horizontal_wrapped(|ui| {
                        for (index, label) in ["All", "Bookmarks", "Highlights", "Notes"]
                            .iter()
                            .enumerate()
                        {
                            ui.selectable_value(&mut self.ui.annotations.filter, index, *label);
                        }
                    });
                    if ui.button("Export as Markdown…").clicked()
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
                    }
                    ui.separator();
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
                            let colors = palette(ui.visuals());
                            egui::Frame::none()
                                .fill(colors.light)
                                .stroke(egui::Stroke::new(1.0_f32, colors.hairline))
                                .rounding(6.0)
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
                                        let kind = if item.color.is_none() {
                                            "Bookmark"
                                        } else if !item.note.is_empty() {
                                            "Note"
                                        } else {
                                            "Highlight"
                                        };
                                        navigate |= ui
                                            .add(
                                                egui::Label::new(
                                                    RichText::new(format!(
                                                        "{kind} · Page {}",
                                                        item.page
                                                    ))
                                                    .size(13.0),
                                                )
                                                .sense(egui::Sense::click()),
                                            )
                                            .on_hover_text("Go to annotation")
                                            .clicked();
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.menu_button("⋮", |ui| {
                                                    if item.color.is_some()
                                                        && ui.button("Edit note / color").clicked()
                                                    {
                                                        self.ui.annotations.editing =
                                                            Some(item.clone());
                                                        ui.close_menu();
                                                    }
                                                    if !item.note.is_empty()
                                                        && ui.button("Delete note").clicked()
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
                                                    if ui.button("Delete annotation").clicked() {
                                                        self.send(Task::DeleteAnnotation(
                                                            path.clone(),
                                                            item.id,
                                                        ));
                                                        ui.close_menu();
                                                    }
                                                })
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
    fn drag_selects_words_in_both_directions_at_different_scales() {
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
        for scale in [1.0, 2.0] {
            let ctx = egui::Context::default();
            let rect =
                egui::Rect::from_min_size(egui::pos2(30.0, 30.0), Vec2::new(200.0, 300.0) * scale);
            let mut selection = None;
            let first = word_rect(rect, words[0].bounds).center();
            let last = word_rect(rect, words[1].bounds).center();
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
                        text_overlay(ui, rect, 1, &words, &[], &mut selection, false);
                    });
                });
            };
            frame(vec![]);
            frame(vec![
                egui::Event::PointerMoved(last),
                egui::Event::PointerButton {
                    pos: last,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ]);
            frame(vec![egui::Event::PointerMoved(first)]);
            frame(vec![egui::Event::PointerButton {
                pos: first,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }]);
            let selected = selection.unwrap().annotation(&words, 2);
            assert_eq!(selected.text, "Hello world");
            assert_eq!(selected.bounds.len(), 2);
        }
    }
}

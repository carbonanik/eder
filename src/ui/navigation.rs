use super::*;

impl Eder {
    pub(crate) fn nav_item(
        ui: &mut egui::Ui,
        title: &str,
        _icon: UiIcon,
        count: Option<usize>,
        active: bool,
        _indent: f32,
    ) -> bool {
        let width = ui.available_width().max(60.0);
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 54.0), egui::Sense::click());
        let painter = ui.painter();
        if active || response.hovered() {
            painter.rect_filled(
                rect,
                0.0,
                if active {
                    SAGE
                } else {
                    Color32::from_rgb(237, 236, 228)
                },
            );
        }
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(1.0_f32, HAIRLINE),
        );
        let number = match title {
            "Library" => "01",
            "Currently Reading" => "02",
            "Favorites" => "03",
            "Settings" => "04",
            _ => "—",
        };
        painter.text(
            rect.left_center() + Vec2::new(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            number,
            egui::FontId::proportional(10.0),
            if active { TEAL } else { MUTED },
        );
        painter.text(
            rect.left_center() + Vec2::new(43.0, 0.0),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(14.0),
            INK,
        );
        if let Some(count) = count {
            painter.text(
                rect.right_center() - Vec2::new(12.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                format!("{:02}", count),
                egui::FontId::proportional(11.0),
                MUTED,
            );
        }
        response.on_hover_text(title).clicked()
    }

    pub(crate) fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(25.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.image((self.ui.logo.id(), Vec2::splat(42.0)));
            ui.add_space(8.0);
            ui.label(RichText::new("Eder").size(43.0).strong().color(INK));
            ui.label(
                RichText::new(".")
                    .size(43.0)
                    .strong()
                    .color(Color32::from_rgb(240, 128, 60)),
            );
        });
        ui.label(
            RichText::new("A quieter way to read.")
                .size(12.0)
                .color(MUTED),
        );
        ui.add_space(41.0);
        // ui.label(RichText::new("YOUR WORKSPACE").size(10.0).strong().color(MUTED));
        // ui.add_space(12.0);
        let can_add = self.ui.file_dialog.is_none();
        let button_width = ui.available_width();
        let add = ui
            .add_enabled_ui(can_add, |ui| {
                icon_label_button(
                    ui,
                    UiIcon::Plus,
                    "Add document",
                    IconLabelStyle {
                        size: Vec2::new(button_width, 44.0),
                        fill: DEEP,
                        color: LIGHT,
                        text_size: 13.0,
                        rounding: 2.0,
                    },
                )
            })
            .inner;
        if add.clicked() {
            self.pick_pdfs(ui.ctx());
        }
        ui.add_space(26.0);
        let all_count = if self.app.view == View::Library {
            Some(self.app.document_count)
        } else {
            None
        };
        for (view, icon, count) in [
            (View::Library, UiIcon::Library, all_count),
            (
                View::Reading,
                UiIcon::Clock,
                Some(self.app.state.history.len()),
            ),
            (
                View::Favorites,
                UiIcon::Heart,
                Some(self.app.state.favorites.len()),
            ),
        ] {
            if Self::nav_item(ui, view.title(), icon, count, self.app.view == view, 0.0) {
                self.change_view(view);
            }
        }
        ui.add_space((ui.available_height() - 60.0).max(18.0));
        if Self::nav_item(
            ui,
            "Settings",
            UiIcon::Settings,
            None,
            self.app.view == View::Settings,
            0.0,
        ) {
            self.change_view(View::Settings);
        }
        ui.add_space(22.0);
    }

    pub(crate) fn compact_nav(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.image((self.ui.logo.id(), Vec2::splat(26.0)));
            ui.add_space(4.0);
            ui.label(RichText::new("Eder").size(24.0).strong().color(INK));
            ui.label(
                RichText::new(".")
                    .size(24.0)
                    .strong()
                    .color(Color32::from_rgb(240, 128, 60)),
            );
            for view in [
                View::Library,
                View::Reading,
                View::Favorites,
                View::Settings,
            ] {
                if ui
                    .selectable_label(self.app.view == view, view.title())
                    .clicked()
                {
                    self.change_view(view);
                }
            }
            if icon_label_button(
                ui,
                UiIcon::Plus,
                "Add PDF",
                IconLabelStyle {
                    size: Vec2::new(120.0, 30.0),
                    fill: ui.visuals().widgets.inactive.bg_fill,
                    color: ui.visuals().text_color(),
                    text_size: 14.0,
                    rounding: 4.0,
                },
            )
            .clicked()
            {
                self.pick_pdfs(ui.ctx());
            }
        });
    }

    pub(crate) fn toolbar_search(&mut self, ui: &mut egui::Ui, width: f32) {
        egui::Frame::none()
            .fill(LIGHT)
            .rounding(2.0)
            .stroke(egui::Stroke::new(1.0_f32, HAIRLINE))
            .inner_margin(egui::Margin::symmetric(10.0, 7.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (icon_rect, _) =
                        ui.allocate_exact_size(Vec2::splat(19.0), egui::Sense::hover());
                    paint_icon(
                        ui.painter(),
                        icon_rect,
                        UiIcon::Search,
                        Color32::from_rgb(40, 66, 87),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.ui.search)
                            .id(egui::Id::new("eder-search"))
                            .hint_text("Search documents...")
                            .desired_width((width - 107.0).max(75.0))
                            .frame(false),
                    );
                    if width > 230.0 {
                        ui.label(
                            RichText::new("Ctrl K")
                                .size(10.0)
                                .color(Color32::from_rgb(132, 147, 166)),
                        );
                    }
                });
            });
    }

    pub(crate) fn view_toggle(&mut self, ui: &mut egui::Ui) {
        let selected = DEEP;
        let unselected = SAGE;
        let dark = INK;
        if icon_label_button(
            ui,
            UiIcon::Grid,
            "",
            IconLabelStyle {
                size: Vec2::new(42.0, 38.0),
                fill: if self.ui.list_view {
                    unselected
                } else {
                    selected
                },
                color: if self.ui.list_view {
                    dark
                } else {
                    Color32::WHITE
                },
                text_size: 18.0,
                rounding: 8.0,
            },
        )
        .on_hover_text("Grid view")
        .clicked()
        {
            self.ui.list_view = false;
        }
        if icon_label_button(
            ui,
            UiIcon::List,
            "",
            IconLabelStyle {
                size: Vec2::new(42.0, 38.0),
                fill: if self.ui.list_view {
                    selected
                } else {
                    unselected
                },
                color: if self.ui.list_view {
                    Color32::WHITE
                } else {
                    dark
                },
                text_size: 18.0,
                rounding: 8.0,
            },
        )
        .on_hover_text("List view")
        .clicked()
        {
            self.ui.list_view = true;
        }
    }

    pub(crate) fn toolbar(&mut self, ui: &mut egui::Ui) {
        let compact = ui.available_width() < 770.0;
        ui.label(
            RichText::new("WORKSPACE / DOCUMENTS")
                .size(10.0)
                .strong()
                .color(TEAL),
        );
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(self.app.view.title())
                    .size(if compact { 31.0 } else { 40.0 })
                    .strong()
                    .color(INK),
            );
            if self.app.view != View::Settings {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(format!("{:02} ITEMS", self.app.document_count))
                        .size(11.0)
                        .color(MUTED),
                );
            }
        });
        if self.app.view != View::Settings {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let controls_width = if compact { 140.0 } else { 210.0 };
                let search_width = (ui.available_width() - controls_width).clamp(120.0, 400.0);
                self.toolbar_search(ui, search_width);
                if ui.available_width() > 90.0 {
                    self.view_toggle(ui);
                }
                if ui.available_width() > 38.0
                    && icon_label_button(
                        ui,
                        UiIcon::Refresh,
                        "",
                        IconLabelStyle {
                            size: Vec2::new(36.0, 36.0),
                            fill: SAGE,
                            color: INK,
                            text_size: 12.0,
                            rounding: 2.0,
                        },
                    )
                    .on_hover_text("Refresh documents")
                    .clicked()
                {
                    self.refresh();
                }
            });
        }
    }

    pub(crate) fn breadcrumbs(&mut self, ui: &mut egui::Ui) {
        if self.app.view != View::Library {
            return;
        }
        let Some(root) = self.app.state.root.clone() else {
            return;
        };
        let current = self.app.folder.clone().unwrap_or_else(|| root.clone());
        let mut crumbs = vec![(Some(UiIcon::Home), "Library root".to_owned(), root.clone())];
        if let Ok(relative) = current.strip_prefix(&root) {
            let mut cursor = root.clone();
            for part in relative.components() {
                cursor.push(part.as_os_str());
                crumbs.push((
                    None,
                    part.as_os_str().to_string_lossy().to_string(),
                    cursor.clone(),
                ));
            }
        }
        ui.horizontal_wrapped(|ui| {
            for (index, (icon, label, target)) in crumbs.into_iter().enumerate() {
                if index > 0 {
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(14.0, 24.0), egui::Sense::hover());
                    paint_icon(
                        ui.painter(),
                        rect,
                        UiIcon::ChevronRight,
                        Color32::from_rgb(159, 178, 196),
                    );
                }
                let clicked = if let Some(icon) = icon {
                    icon_label_button(
                        ui,
                        icon,
                        &label,
                        IconLabelStyle {
                            size: Vec2::new(116.0, 30.0),
                            fill: Color32::from_rgb(238, 243, 249),
                            color: Color32::from_rgb(57, 80, 105),
                            text_size: 12.0,
                            rounding: 7.0,
                        },
                    )
                    .clicked()
                } else {
                    ui.add(
                        egui::Button::new(
                            RichText::new(label)
                                .size(12.0)
                                .color(Color32::from_rgb(57, 80, 105)),
                        )
                        .fill(Color32::from_rgb(238, 243, 249))
                        .rounding(7.0),
                    )
                    .clicked()
                };
                if clicked && self.app.folder.as_ref() != Some(&target) {
                    self.browse_folder(target);
                }
            }
        });
    }
}

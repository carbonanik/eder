use super::*;

impl Eder {
    pub(crate) fn settings(&mut self, ui: &mut egui::Ui) {
        let back = icon_label_button(
            ui,
            UiIcon::ArrowLeft,
            "Back to Library",
            IconLabelStyle {
                size: Vec2::new(150.0, 30.0),
                fill: ui.visuals().widgets.inactive.bg_fill,
                color: ui.visuals().text_color(),
                text_size: 14.0,
                rounding: 4.0,
            },
        );
        if back.clicked() {
            self.app.view = View::Library;
            self.load();
        }
        ui.add_space(24.0);
        ui.label(
            RichText::new("Your library,\non your device.")
                .size(42.0)
                .strong()
                .color(palette(ui.visuals()).ink),
        );
        ui.label("Choose the folder containing your PDFs. Subfolders stay organized as they are.");
        ui.add_space(18.0);
        ui.label(RichText::new("Library folder").strong());
        ui.label(
            self.app
                .state
                .root
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "No folder selected".into()),
        );
        if ui
            .add_enabled(
                self.ui.dialog.is_none(),
                egui::Button::new("Choose folder…"),
            )
            .clicked()
        {
            self.pick_library_folder(ui.ctx());
        }
        ui.add_space(20.0);
        ui.label("If your desktop does not support the folder dialog, enter a local path:");
        ui.text_edit_singleline(&mut self.ui.folder_input);
        if ui.button("Use folder path").clicked() {
            let p = PathBuf::from(self.ui.folder_input.trim());
            if p.is_absolute() {
                self.send(Task::Root(p));
            } else {
                self.app.message = Some(("Enter an absolute folder path.".into(), true));
            }
        }
        ui.add_space(28.0);
        ui.separator();
        ui.add_space(18.0);
        ui.label(RichText::new("Appearance").strong());
        ui.label("Choose the theme for your workspace.");
        let mut dark_theme = self.app.state.dark_theme;
        ui.horizontal(|ui| {
            if theme_switch(ui, &mut dark_theme).changed() {
                self.send(Task::DarkTheme(dark_theme));
            }
            ui.label(if dark_theme {
                "Dark theme"
            } else {
                "Light theme"
            });
        });
        ui.add_space(18.0);
        ui.separator();
        ui.add_space(18.0);
        ui.label(RichText::new("Reading Mode").strong());
        ui.label("Choose how pages are displayed when you open a PDF.");
        let mut continuous_scroll = self.app.state.continuous_scroll;
        ui.horizontal(|ui| {
            ui.radio_value(&mut continuous_scroll, false, "Single Page");
            ui.radio_value(&mut continuous_scroll, true, "Continuous Scroll");
        });
        if continuous_scroll != self.app.state.continuous_scroll {
            self.send(Task::ReadingMode(continuous_scroll));
        }
        ui.add_space(18.0);
        ui.separator();
        ui.label("Local storage only · No account · No document uploads");
        ui.label(
            "PDF rendering requires Poppler (pdfinfo + pdftoppm); clickable index links also require pdftohtml.",
        );
    }
    // Keep the grid responsive and virtualize rows rather than individual documents.
}

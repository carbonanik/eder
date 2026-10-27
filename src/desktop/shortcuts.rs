use super::*;
use egui::{Key, Modifiers};

// Consume events so a shortcut cannot also activate a focused widget or scroll area.
// Match modifiers explicitly; e.g. Ctrl+Left must remain a text-editing command.
fn pressed(ctx: &egui::Context, modifiers: Modifiers, key: Key) -> bool {
    ctx.input_mut(|input| {
        let mut pressed = false;
        input.events.retain(|event| {
            if let egui::Event::Key {
                key: event_key,
                pressed: true,
                modifiers: event_modifiers,
                repeat,
                ..
            } = event
            {
                let shifted_zoom = matches!(key, Key::Plus | Key::Equals);
                if *event_key == key
                    && event_modifiers.matches_logically(modifiers)
                    && event_modifiers.alt == modifiers.alt
                    && (shifted_zoom || event_modifiers.shift == modifiers.shift)
                {
                    let repeatable = matches!(
                        key,
                        Key::ArrowLeft
                            | Key::ArrowRight
                            | Key::PageUp
                            | Key::PageDown
                            | Key::Space
                            | Key::Plus
                            | Key::Equals
                            | Key::Minus
                    );
                    pressed |= !repeat || repeatable;
                    return false;
                }
            }
            true
        });
        pressed
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(ctx: &egui::Context) -> (Eder, mpsc::Receiver<Task>) {
        let (runtime, tasks) = RuntimeClient::detached();
        let mut app = Eder {
            runtime,
            app: Default::default(),
            ui: crate::ui::state::UiState::new(load_logo(ctx)),
        };
        app.app.on_event(Event::Open(
            "book.pdf".into(),
            crate::domain::reading_position::ReadingPosition::content(2, 0.3, 1.0, 0.2),
            5,
            0,
            vec![],
            crate::domain::document::FileRevision {
                bytes: 1,
                modified_nanos: 1,
            },
        ));
        (app, tasks)
    }

    fn input(key: Key, modifiers: Modifiers, fullscreen: bool) -> egui::RawInput {
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .fullscreen = Some(fullscreen);
        input.modifiers = modifiers;
        // Each simulated shortcut is a fresh physical press, including repeated F11.
        input.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        });
        input.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        });
        input
    }

    #[test]
    fn fullscreen_and_escape_preserve_the_open_document() {
        let ctx = egui::Context::default();
        let (mut app, tasks) = app(&ctx);
        for (key, fullscreen, expected) in [
            (Key::F11, false, true),
            (Key::F11, true, false),
            (Key::Escape, true, false),
        ] {
            let output = ctx.run(input(key, Modifiers::NONE, fullscreen), |ctx| {
                app.keyboard_shortcuts(ctx)
            });
            assert!(output.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|cmd|
                matches!(cmd, egui::ViewportCommand::Fullscreen(value) if *value == expected)));
            assert!(!app.app.reader.as_ref().unwrap().closing);
            assert!(tasks.try_recv().is_err());
        }
        app.ui.shortcuts_open = true;
        let output = ctx.run(input(Key::Escape, Modifiers::NONE, true), |ctx| {
            app.keyboard_shortcuts(ctx)
        });
        assert!(!app.ui.shortcuts_open);
        assert!(!output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|cmd| matches!(cmd, egui::ViewportCommand::Fullscreen(_))));
        let _ = ctx.run(input(Key::Escape, Modifiers::NONE, false), |ctx| {
            app.keyboard_shortcuts(ctx)
        });
        assert!(matches!(tasks.try_recv().unwrap(), Task::Close(..)));
    }

    #[test]
    fn reader_shortcuts_preserve_positions_and_consume_navigation() {
        let ctx = egui::Context::default();
        let (mut app, tasks) = app(&ctx);
        let _ = ctx.run(
            input(
                Key::Plus,
                Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT,
                false,
            ),
            |ctx| app.keyboard_shortcuts(ctx),
        );
        let reader = app.app.reader.as_ref().unwrap();
        assert!((reader.zoom - 1.1).abs() < 0.001);
        assert!((reader.scroll_to.unwrap().fraction - 0.3).abs() < 0.001);
        let _ = ctx.run(
            input(Key::S, Modifiers::MAC_CMD | Modifiers::COMMAND, false),
            |ctx| app.keyboard_shortcuts(ctx),
        );
        assert!(
            matches!(tasks.try_recv().unwrap(), Task::Position(_, position, true) if position.page == 2 && (position.fraction - 0.3).abs() < 0.001)
        );
        assert!(app
            .app
            .reader
            .as_ref()
            .unwrap()
            .position_changed_at
            .is_none());
        let _ = ctx.run(
            input(Key::Num0, Modifiers::CTRL | Modifiers::COMMAND, false),
            |ctx| app.keyboard_shortcuts(ctx),
        );
        assert_eq!(app.app.reader.as_ref().unwrap().zoom, 1.0);
        let _ = ctx.run(input(Key::Space, Modifiers::NONE, false), |ctx| {
            app.keyboard_shortcuts(ctx);
            assert!(!ctx.input(|i| i.key_pressed(Key::Space)));
        });
        assert_eq!(app.app.reader.as_ref().unwrap().page, 3);
        let _ = ctx.run(input(Key::Space, Modifiers::SHIFT, false), |ctx| {
            app.keyboard_shortcuts(ctx)
        });
        assert_eq!(app.app.reader.as_ref().unwrap().page, 2);
    }

    #[test]
    fn text_editing_and_modified_navigation_do_not_turn_pages() {
        let ctx = egui::Context::default();
        let (mut app, _tasks) = app(&ctx);
        let _ = ctx.run(
            input(Key::ArrowRight, Modifiers::CTRL | Modifiers::COMMAND, false),
            |ctx| app.keyboard_shortcuts(ctx),
        );
        assert_eq!(app.app.reader.as_ref().unwrap().page, 2);
        let mut text = "page".to_owned();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.text_edit_singleline(&mut text).request_focus();
            });
        });
        let _ = ctx.run(input(Key::ArrowRight, Modifiers::NONE, false), |ctx| {
            assert!(ctx.wants_keyboard_input());
            app.keyboard_shortcuts(ctx);
            assert!(ctx.input(|i| i.key_pressed(Key::ArrowRight)));
        });
        assert_eq!(app.app.reader.as_ref().unwrap().page, 2);
    }
}

impl Eder {
    pub(crate) fn keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        if pressed(ctx, Modifiers::NONE, Key::F11)
            || pressed(ctx, Modifiers::CTRL | Modifiers::MAC_CMD, Key::F)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }
        if pressed(ctx, Modifiers::NONE, Key::F1) {
            self.ui.shortcuts_open = !self.ui.shortcuts_open;
        }
        if pressed(ctx, Modifiers::NONE, Key::Escape) {
            if self.ui.shortcuts_open {
                self.ui.shortcuts_open = false;
            } else if fullscreen {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            } else if let Some(id) = ctx.memory(|m| m.focused()) {
                ctx.memory_mut(|m| m.surrender_focus(id));
            } else if self.app.reader.is_some() {
                self.back();
            } else if self.app.view == View::Settings {
                self.change_view(View::Library);
            } else if !self.ui.search.is_empty() {
                self.ui.search.clear();
            }
            return;
        }
        if self.ui.shortcuts_open || self.ui.dialog.is_some() || self.ui.file_dialog.is_some() {
            return;
        }
        if pressed(ctx, Modifiers::COMMAND, Key::Q) {
            // The regular close-request path flushes pending reading positions and settings.
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.app.reader.as_ref().is_some_and(|r| r.closing) {
            return;
        }
        if pressed(ctx, Modifiers::COMMAND, Key::W) {
            if self.app.reader.is_some() {
                self.back();
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }
        if self.app.reader.is_some() {
            if pressed(ctx, Modifiers::COMMAND, Key::S) {
                self.save_reading_position();
            }
            if pressed(ctx, Modifiers::COMMAND, Key::Plus)
                || pressed(ctx, Modifiers::COMMAND, Key::Equals)
            {
                self.zoom_reader(0.1, false);
            }
            if pressed(ctx, Modifiers::COMMAND, Key::Minus) {
                self.zoom_reader(-0.1, false);
            }
            if pressed(ctx, Modifiers::COMMAND, Key::Num0) {
                self.zoom_reader(0.0, true);
            }
            if ctx.wants_keyboard_input() {
                return;
            }
            let reader = self.app.reader.as_ref().unwrap();
            let (page, total) = (reader.page, reader.total);
            let target = if pressed(ctx, Modifiers::NONE, Key::Home) {
                Some(1)
            } else if pressed(ctx, Modifiers::NONE, Key::End) {
                Some(total)
            } else if pressed(ctx, Modifiers::NONE, Key::ArrowRight)
                || pressed(ctx, Modifiers::NONE, Key::PageDown)
                || pressed(ctx, Modifiers::NONE, Key::Space)
            {
                Some(page.saturating_add(1))
            } else if pressed(ctx, Modifiers::NONE, Key::ArrowLeft)
                || pressed(ctx, Modifiers::NONE, Key::PageUp)
                || pressed(ctx, Modifiers::SHIFT, Key::Space)
            {
                Some(page.saturating_sub(1))
            } else {
                None
            };
            if let Some(page) = target {
                self.turn(page);
            }
        } else {
            if pressed(ctx, Modifiers::COMMAND, Key::O) {
                self.pick_pdfs(ctx);
            }
            if pressed(ctx, Modifiers::COMMAND, Key::Comma) {
                self.change_view(View::Settings);
            }
            for (key, view) in [
                (Key::Num1, View::Library),
                (Key::Num2, View::Reading),
                (Key::Num3, View::Favorites),
            ] {
                if pressed(ctx, Modifiers::COMMAND, key) {
                    self.change_view(view);
                }
            }
            if pressed(ctx, Modifiers::COMMAND, Key::K) || pressed(ctx, Modifiers::COMMAND, Key::F)
            {
                if self.app.view == View::Settings {
                    self.change_view(View::Library);
                }
                ctx.memory_mut(|m| m.request_focus(egui::Id::new("eder-search")));
            }
            if self.app.view != View::Settings
                && (pressed(ctx, Modifiers::NONE, Key::F5)
                    || pressed(ctx, Modifiers::COMMAND, Key::R))
            {
                self.refresh();
            }
        }
    }

    pub(crate) fn save_reading_position(&mut self) {
        if let Some(reader) = &self.app.reader {
            if self.send(Task::Position(
                reader.path.clone(),
                reader.reading_position(self.app.state.continuous_scroll),
                true,
            )) {
                self.app.reader.as_mut().unwrap().position_changed_at = None;
            }
        }
    }

    fn zoom_reader(&mut self, delta: f32, reset: bool) {
        if let Some(reader) = &mut self.app.reader {
            let zoom = if reset {
                1.0
            } else {
                (reader.zoom + delta).clamp(0.5, 3.0)
            };
            if zoom != reader.zoom {
                let position = reader.reading_position(self.app.state.continuous_scroll);
                reader.zoom = zoom;
                reader.scroll_to = Some(position);
                reader.position_changed_at = Some(Instant::now());
            }
        }
    }

    pub(crate) fn window_controls(&mut self, ui: &mut egui::Ui) {
        let fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
        if ui
            .small_button(if fullscreen {
                "Exit fullscreen"
            } else {
                "Fullscreen"
            })
            .on_hover_text("Toggle fullscreen (F11; Ctrl+Command+F on macOS)")
            .clicked()
        {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }
        if ui
            .small_button("Shortcuts")
            .on_hover_text("Keyboard shortcuts (F1)")
            .clicked()
        {
            self.ui.shortcuts_open = true;
        }
    }

    pub(crate) fn shortcuts_help(&mut self, ctx: &egui::Context) {
        let command = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Ctrl"
        };
        egui::Window::new("Keyboard shortcuts")
            .open(&mut self.ui.shortcuts_open)
            .collapsible(false)
            .resizable(false)
            .vscroll(true)
            .show(ctx, |ui| {
                egui::Grid::new("shortcut-reference")
                    .spacing([24.0, 8.0])
                    .show(ui, |ui| {
                        for (keys, action) in [
                            ("F11".into(), "Toggle fullscreen"),
                            ("F1".into(), "Show / hide keyboard shortcuts"),
                            (
                                "Esc".into(),
                                "Dismiss help / exit fullscreen / leave field / go back",
                            ),
                            (format!("{command}+Q"), "Quit Eder"),
                            (format!("{command}+W"), "Close document / window"),
                            (format!("{command}+O"), "Add PDFs (library)"),
                            (
                                format!("{command}+K or {command}+F"),
                                "Search documents (library)",
                            ),
                            (format!("F5 or {command}+R"), "Refresh documents (library)"),
                            (
                                format!("{command}+1 / 2 / 3"),
                                "Library / Currently Reading / Favorites",
                            ),
                            (format!("{command}+,"), "Settings (library)"),
                            ("→ / Page Down / Space".into(), "Next page"),
                            ("← / Page Up / Shift+Space".into(), "Previous page"),
                            ("Home / End".into(), "First / last page"),
                            (format!("{command}+S"), "Save reading position"),
                            (format!("{command}++ / {command}+-"), "Zoom in / out"),
                            (format!("{command}+0"), "Reset zoom"),
                        ] {
                            ui.label(egui::RichText::new(keys).strong());
                            ui.label(action);
                            ui.end_row();
                        }
                    });
                if cfg!(target_os = "macos") {
                    ui.label("Control+Command+F also toggles fullscreen.");
                }
            });
    }
}

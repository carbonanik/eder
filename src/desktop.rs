#[cfg(test)]
use crate::domain::reading_position::ReadingPosition;
use crate::{
    app::{effect::Task, event::Event, jobs::*, state::View},
    runtime::RuntimeClient,
    ui::{textures::CachedPage, theme::*},
};
use eframe::egui::{self, Color32, RichText, Vec2};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, mpsc, Arc},
    time::{Duration, Instant},
};
pub struct Eder {
    pub(crate) runtime: RuntimeClient<egui::ColorImage>,
    pub(crate) app: crate::app::state::Controller,
    pub(crate) ui: crate::ui::state::UiState,
}

impl Eder {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::ui::theme::configure(&cc.egui_ctx);
        let logo = load_logo(&cc.egui_ctx);
        let runtime = RuntimeClient::start(
            crate::infrastructure::platform::AppPaths::discover(),
            {
                let ctx = cc.egui_ctx.clone();
                Arc::new(move || ctx.request_repaint())
            },
            Arc::new(color_image),
        );
        Self {
            runtime,
            app: crate::app::state::Controller::default(),
            ui: crate::ui::state::UiState::new(logo),
        }
    }
    pub(crate) fn send(&mut self, task: Task) -> bool {
        match self.runtime.submit(task) {
            Ok(()) => true,
            Err(e) => {
                if matches!(e, mpsc::TrySendError::Disconnected(_)) {
                    self.app.lifecycle = crate::app::state::Lifecycle::Failed;
                }
                self.app.message = Some((
                    format!("Action could not be queued: {e}. Please retry."),
                    true,
                ));
                false
            }
        }
    }
}

fn color_image(image: crate::domain::pixels::Pixels) -> egui::ColorImage {
    egui::ColorImage {
        size: image.size,
        pixels: image
            .rgba
            .into_iter()
            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect(),
    }
}
impl eframe::App for Eder {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
        if ctx.input(|i| i.viewport().close_requested())
            && self.app.lifecycle != crate::app::state::Lifecycle::Closed
            && self.app.lifecycle != crate::app::state::Lifecycle::Failed
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.app.lifecycle != crate::app::state::Lifecycle::Closing {
                self.runtime.cancel_pdf();
                let position = self.app.reader.as_ref().map(|r| {
                    (
                        r.path.clone(),
                        r.reading_position(self.app.state.continuous_scroll),
                    )
                });
                if self.send(Task::Shutdown(position)) {
                    self.app.lifecycle = crate::app::state::Lifecycle::Closing;
                    self.ui.closing_started = Some(Instant::now());
                }
            }
        }
        if self.app.lifecycle == crate::app::state::Lifecycle::Closing {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.spinner();
                ui.label("Finishing saves…");
                if self
                    .ui
                    .closing_started
                    .is_some_and(|start| start.elapsed() > Duration::from_secs(5))
                {
                    ui.label("Saving is taking longer than usual.");
                    if ui.button("Exit without waiting for saves").clicked() {
                        self.app.lifecycle = crate::app::state::Lifecycle::Closed;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                ctx.request_repaint_after(Duration::from_millis(250));
            });
            return;
        }
        if self.app.message.is_some() {
            egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if let Some((message, error)) = &self.app.message {
                        ui.label(RichText::new(message).color(if *error {
                            Color32::DARK_RED
                        } else {
                            Color32::from_rgb(20, 100, 80)
                        }));
                        if ui.small_button("Dismiss").clicked() {
                            self.app.message = None;
                        }
                    }
                });
            });
        }
        if self.app.reader.is_some() {
            self.reader_ui(ctx);
            // Preserve the optimized reader and precise continuous-scroll saving.
            self.flush_position_if_idle(ctx);
            return;
        }
        if ctx.input(|input| {
            (input.modifiers.command || input.modifiers.ctrl) && input.key_pressed(egui::Key::K)
        }) {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("eder-search")));
        }
        if ctx.screen_rect().width() > 820.0 {
            egui::SidePanel::left("sidebar")
                .exact_width(265.0)
                .frame(
                    egui::Frame::none()
                        .fill(PAPER)
                        .inner_margin(egui::Margin::symmetric(23.0, 8.0)),
                )
                .show(ctx, |ui| self.sidebar(ui));
        } else {
            egui::TopBottomPanel::top("compact_navigation").show(ctx, |ui| self.compact_nav(ui));
        }
        let available = ctx.available_rect().width();
        let header_height = if self.app.view == View::Settings {
            103.0
        } else if available < 770.0 {
            157.0
        } else {
            151.0
        };
        egui::TopBottomPanel::top("library_toolbar")
            .exact_height(header_height)
            .frame(
                egui::Frame::none()
                    .fill(PAPER)
                    .inner_margin(egui::Margin::symmetric(27.0, 13.0)),
            )
            .show(ctx, |ui| self.toolbar(ui));
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(PAPER)
                    .inner_margin(egui::Margin::symmetric(27.0, 20.0)),
            )
            .show(ctx, |ui| {
                if self.app.view == View::Settings {
                    egui::ScrollArea::vertical().show(ui, |ui| self.settings(ui));
                } else {
                    self.browser(ui);
                }
            });
        if self.ui.dialog.is_some() || self.ui.file_dialog.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
#[path = "ui/tests.rs"]
mod tests;

mod events;
mod library;
mod reader;

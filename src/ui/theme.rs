use super::*;

// Shared semantic colors for both the custom painter and native widgets.
#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub paper: Color32,
    pub ink: Color32,
    pub deep: Color32,
    pub teal: Color32,
    pub muted: Color32,
    pub sage: Color32,
    pub light: Color32,
    pub hairline: Color32,
    pub hover: Color32,
    pub nav_hover: Color32,
    pub on_primary: Color32,
    pub amber: Color32,
    pub subtle_fill: Color32,
    pub subtle_text: Color32,
    pub error: Color32,
    pub success: Color32,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                paper: Color32::from_rgb(18, 29, 34),
                ink: Color32::from_rgb(235, 233, 223),
                deep: Color32::from_rgb(42, 90, 94),
                teal: Color32::from_rgb(90, 205, 190),
                muted: Color32::from_rgb(162, 180, 182),
                sage: Color32::from_rgb(33, 51, 56),
                light: Color32::from_rgb(25, 39, 45),
                hairline: Color32::from_rgb(62, 83, 87),
                hover: Color32::from_rgb(44, 69, 73),
                nav_hover: Color32::from_rgb(30, 46, 52),
                on_primary: Color32::from_rgb(250, 249, 245),
                amber: Color32::from_rgb(244, 173, 104),
                subtle_fill: Color32::from_rgb(32, 48, 58),
                subtle_text: Color32::from_rgb(181, 203, 217),
                error: Color32::from_rgb(255, 145, 140),
                success: Color32::from_rgb(122, 215, 174),
            }
        } else {
            Self {
                paper: Color32::from_rgb(244, 241, 233),
                ink: Color32::from_rgb(16, 31, 36),
                deep: Color32::from_rgb(9, 33, 43),
                teal: Color32::from_rgb(21, 157, 153),
                muted: Color32::from_rgb(104, 119, 122),
                sage: Color32::from_rgb(223, 233, 228),
                light: Color32::from_rgb(250, 249, 245),
                hairline: Color32::from_rgb(204, 212, 207),
                hover: Color32::from_rgb(207, 226, 218),
                nav_hover: Color32::from_rgb(237, 236, 228),
                on_primary: Color32::from_rgb(250, 249, 245),
                amber: Color32::from_rgb(240, 128, 60),
                subtle_fill: Color32::from_rgb(238, 243, 249),
                subtle_text: Color32::from_rgb(57, 80, 105),
                error: Color32::from_rgb(139, 0, 0),
                success: Color32::from_rgb(20, 100, 80),
            }
        }
    }
}

pub(crate) fn palette(visuals: &egui::Visuals) -> Palette {
    Palette::new(visuals.dark_mode)
}

pub(crate) use crate::policy::POSITION_SAVE_DELAY;

pub(crate) fn load_logo(ctx: &egui::Context) -> TextureHandle {
    let logo = image::load_from_memory(include_bytes!("../../eder-landing/assets/eder-mark.png"))
        .expect("embedded Eder logo must be a valid PNG")
        .to_rgba8();
    let (width, height) = logo.dimensions();
    ctx.load_texture(
        "eder-logo",
        egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], logo.as_raw()),
        egui::TextureOptions::LINEAR,
    )
}

#[derive(Clone, Copy)]
pub(crate) enum UiIcon {
    Library,
    Clock,
    Heart,
    Settings,
    Search,
    Grid,
    List,
    Refresh,
    Home,
    ChevronRight,
    ArrowLeft,
    Plus,
    Star,
    Close,
    Folder,
}

pub(crate) fn paint_icon(painter: &egui::Painter, rect: egui::Rect, icon: UiIcon, color: Color32) {
    let center = rect.center();
    let half = rect.width().min(rect.height()) * 0.34;
    let stroke = egui::Stroke::new(1.8_f32, color);
    let point = |x: f32, y: f32| center + Vec2::new(x * half, y * half);
    let line = |a, b| painter.line_segment([a, b], stroke);
    match icon {
        UiIcon::Library => {
            let page = egui::Rect::from_center_size(center, Vec2::new(half * 1.65, half * 2.0));
            painter.rect_stroke(page, 1.5, stroke);
            line(point(-0.4, -0.35), point(0.4, -0.35));
            line(point(-0.4, 0.05), point(0.4, 0.05));
            line(point(-0.4, 0.45), point(0.2, 0.45));
        }
        UiIcon::Clock => {
            painter.circle_stroke(center, half * 0.9, stroke);
            line(center, point(0.0, -0.55));
            line(center, point(0.45, 0.0));
        }
        UiIcon::Heart => {
            let points = [
                point(0.0, 0.8),
                point(-0.85, 0.0),
                point(-0.85, -0.45),
                point(-0.55, -0.8),
                point(-0.2, -0.8),
                point(0.0, -0.5),
                point(0.2, -0.8),
                point(0.55, -0.8),
                point(0.85, -0.45),
                point(0.85, 0.0),
                point(0.0, 0.8),
            ];
            for segment in points.windows(2) {
                line(segment[0], segment[1]);
            }
        }
        UiIcon::Settings => {
            painter.circle_stroke(center, half * 0.72, stroke);
            painter.circle_stroke(center, half * 0.25, stroke);
            for angle in 0..8 {
                let angle = angle as f32 * std::f32::consts::TAU / 8.0;
                let direction = Vec2::new(angle.cos(), angle.sin());
                line(center + direction * half * 0.78, center + direction * half);
            }
        }
        UiIcon::Search => {
            painter.circle_stroke(
                center - Vec2::new(half * 0.18, half * 0.18),
                half * 0.58,
                stroke,
            );
            line(point(0.25, 0.25), point(0.8, 0.8));
        }
        UiIcon::Grid => {
            let cell = half * 0.72;
            for x in [-0.48, 0.48] {
                for y in [-0.48, 0.48] {
                    painter.rect_stroke(
                        egui::Rect::from_center_size(point(x, y), Vec2::splat(cell)),
                        1.5,
                        stroke,
                    );
                }
            }
        }
        UiIcon::List => {
            for y in [-0.55, 0.0, 0.55] {
                painter.circle_filled(point(-0.72, y), half * 0.09, color);
                line(point(-0.4, y), point(0.75, y));
            }
        }
        UiIcon::Refresh => {
            let points = std::array::from_fn::<_, 17, _>(|step| {
                let angle = 0.8 + step as f32 * (std::f32::consts::TAU - 1.6) / 16.0;
                point(angle.cos() * 0.78, angle.sin() * 0.78)
            });
            for segment in points.windows(2) {
                line(segment[0], segment[1]);
            }
            line(point(0.63, -0.52), point(0.78, -0.55));
            line(point(0.78, -0.55), point(0.78, -0.05));
            line(point(0.78, -0.55), point(0.28, -0.55));
        }
        UiIcon::Home => {
            line(point(-0.9, -0.05), point(0.0, -0.85));
            line(point(0.0, -0.85), point(0.9, -0.05));
            line(point(-0.65, -0.25), point(-0.65, 0.8));
            line(point(0.65, -0.25), point(0.65, 0.8));
            line(point(-0.65, 0.8), point(0.65, 0.8));
            line(point(-0.15, 0.8), point(-0.15, 0.2));
            line(point(0.15, 0.8), point(0.15, 0.2));
        }
        UiIcon::ChevronRight => {
            line(point(-0.25, -0.65), point(0.35, 0.0));
            line(point(0.35, 0.0), point(-0.25, 0.65));
        }
        UiIcon::ArrowLeft => {
            line(point(0.75, 0.0), point(-0.75, 0.0));
            line(point(-0.75, 0.0), point(-0.15, -0.6));
            line(point(-0.75, 0.0), point(-0.15, 0.6));
        }
        UiIcon::Plus => {
            line(point(-0.65, 0.0), point(0.65, 0.0));
            line(point(0.0, -0.65), point(0.0, 0.65));
        }
        UiIcon::Star => {
            let points = std::array::from_fn::<_, 11, _>(|step| {
                let angle = -std::f32::consts::FRAC_PI_2 + step as f32 * std::f32::consts::PI / 5.0;
                let radius = if step % 2 == 0 { 0.9 } else { 0.4 };
                point(angle.cos() * radius, angle.sin() * radius)
            });
            for segment in points.windows(2) {
                line(segment[0], segment[1]);
            }
        }
        UiIcon::Close => {
            line(point(-0.6, -0.6), point(0.6, 0.6));
            line(point(0.6, -0.6), point(-0.6, 0.6));
        }
        UiIcon::Folder => {
            let points = [
                point(-0.9, -0.55),
                point(-0.2, -0.55),
                point(0.0, -0.25),
                point(0.9, -0.25),
                point(0.75, 0.65),
                point(-0.75, 0.65),
                point(-0.9, -0.55),
            ];
            for segment in points.windows(2) {
                line(segment[0], segment[1]);
            }
        }
    }
}

pub(crate) struct IconLabelStyle {
    pub size: Vec2,
    pub fill: Color32,
    pub color: Color32,
    pub text_size: f32,
    pub rounding: f32,
}

pub(crate) fn icon_label_button(
    ui: &mut egui::Ui,
    icon: UiIcon,
    label: &str,
    style: IconLabelStyle,
) -> egui::Response {
    let response = ui.add(
        egui::Button::new(
            RichText::new(label)
                .size(style.text_size)
                .color(Color32::TRANSPARENT),
        )
        .min_size(style.size)
        .fill(style.fill)
        .rounding(style.rounding),
    );
    let icon_size = 18.0_f32.min(style.size.y * 0.55);
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(style.text_size),
        style.color,
    );
    let gap = if label.is_empty() { 0.0 } else { 8.0 };
    let group_width = icon_size + gap + galley.size().x;
    let group_left = response.rect.center().x - group_width * 0.5;
    let icon_center_x = group_left + icon_size * 0.5;
    paint_icon(
        ui.painter(),
        egui::Rect::from_center_size(
            egui::pos2(icon_center_x, response.rect.center().y),
            Vec2::splat(icon_size),
        ),
        icon,
        style.color,
    );
    if !label.is_empty() {
        ui.painter().galley(
            egui::pos2(
                group_left + icon_size + gap,
                response.rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            style.color,
        );
    }
    response
}

pub(crate) fn configure(ctx: &egui::Context, dark: bool) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(14.0, 12.0);
    style.spacing.button_padding = Vec2::new(12.0, 9.0);
    let palette = Palette::new(dark);
    style.visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.visuals.panel_fill = palette.paper;
    style.visuals.window_fill = palette.paper;
    style.visuals.extreme_bg_color = palette.light;
    style.visuals.faint_bg_color = palette.sage;
    style.visuals.override_text_color = Some(palette.ink);
    style.visuals.widgets.inactive.bg_fill = palette.sage;
    style.visuals.widgets.inactive.weak_bg_fill = palette.sage;
    style.visuals.widgets.hovered.bg_fill = palette.hover;
    style.visuals.widgets.active.bg_fill = palette.teal;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, palette.hairline);
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, palette.hairline);
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, palette.teal);
    style.visuals.widgets.inactive.rounding = 2.0.into();
    style.visuals.widgets.hovered.rounding = 2.0.into();
    style.visuals.widgets.active.rounding = 2.0.into();
    style.visuals.selection.bg_fill = palette.sage;
    style.visuals.selection.stroke.color = palette.teal;
    style.visuals.hyperlink_color = palette.teal;
    style.visuals.warn_fg_color = palette.amber;
    style.visuals.error_fg_color = palette.error;
    ctx.set_style(style);
}

/// A keyboard-focusable switch with checkbox semantics for assistive technology.
pub(crate) fn theme_switch(ui: &mut egui::Ui, value: &mut bool) -> egui::Response {
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(44.0, 26.0), egui::Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            ui.is_enabled(),
            *value,
            "Dark theme",
        )
    });
    let colors = palette(ui.visuals());
    let amount = ui.ctx().animate_bool(response.id, *value);
    let fill = if *value { colors.teal } else { colors.sage };
    ui.painter().rect(
        rect,
        13.0,
        fill,
        egui::Stroke::new(
            1.0_f32,
            if response.hovered() || response.has_focus() {
                colors.teal
            } else {
                colors.hairline
            },
        ),
    );
    let center = egui::pos2(
        egui::lerp((rect.left() + 13.0)..=(rect.right() - 13.0), amount),
        rect.center().y,
    );
    ui.painter().circle_filled(
        center,
        9.0,
        if *value {
            Color32::from_rgb(18, 29, 34)
        } else {
            colors.ink
        },
    );
    response.on_hover_text("Toggle dark theme")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_can_switch_back_to_original_light_colors() {
        let ctx = egui::Context::default();
        configure(&ctx, true);
        assert!(ctx.style().visuals.dark_mode);
        assert_eq!(ctx.style().visuals.panel_fill, Palette::new(true).paper);
        configure(&ctx, false);
        let style = ctx.style();
        assert!(!style.visuals.dark_mode);
        assert_eq!(style.visuals.panel_fill, Color32::from_rgb(244, 241, 233));
        assert_eq!(style.visuals.text_color(), Color32::from_rgb(16, 31, 36));
    }

    #[test]
    fn dark_theme_text_has_readable_contrast_on_all_surfaces() {
        fn luminance(color: Color32) -> f32 {
            let linear = |channel: u8| {
                let value = f32::from(channel) / 255.0;
                if value <= 0.04045 {
                    value / 12.92
                } else {
                    ((value + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
        }
        let colors = Palette::new(true);
        for background in [colors.paper, colors.light, colors.sage, colors.hover] {
            for foreground in [
                colors.ink,
                colors.muted,
                colors.teal,
                colors.amber,
                colors.error,
                colors.success,
            ] {
                let contrast = (luminance(foreground) + 0.05) / (luminance(background) + 0.05);
                assert!(
                    contrast >= 4.5,
                    "Contrast {contrast} for {foreground:?} on {background:?}"
                );
            }
        }
    }

    #[test]
    fn theme_switch_supports_keyboard_activation() {
        let ctx = egui::Context::default();
        let mut dark = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let response = theme_switch(ui, &mut dark);
                response.request_focus();
            });
        });
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(theme_switch(ui, &mut dark).changed());
            });
        });
        assert!(dark);
    }
}

use super::*;

// Minimal Editorial design tokens — restrained native desktop palette.
pub(crate) const PAPER: Color32 = Color32::from_rgb(244, 241, 233);
pub(crate) const INK: Color32 = Color32::from_rgb(16, 31, 36);
pub(crate) const DEEP: Color32 = Color32::from_rgb(9, 33, 43);
pub(crate) const TEAL: Color32 = Color32::from_rgb(21, 157, 153);
pub(crate) const MUTED: Color32 = Color32::from_rgb(104, 119, 122);
pub(crate) const SAGE: Color32 = Color32::from_rgb(223, 233, 228);
pub(crate) const LIGHT: Color32 = Color32::from_rgb(250, 249, 245);
pub(crate) const HAIRLINE: Color32 = Color32::from_rgb(204, 212, 207);

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

pub(crate) fn configure(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(14.0, 12.0);
    style.spacing.button_padding = Vec2::new(12.0, 9.0);
    style.visuals = egui::Visuals::light();
    style.visuals.panel_fill = PAPER;
    style.visuals.window_fill = PAPER;
    style.visuals.extreme_bg_color = LIGHT;
    style.visuals.faint_bg_color = SAGE;
    style.visuals.override_text_color = Some(INK);
    style.visuals.widgets.inactive.bg_fill = SAGE;
    style.visuals.widgets.inactive.weak_bg_fill = SAGE;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(207, 226, 218);
    style.visuals.widgets.active.bg_fill = TEAL;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, HAIRLINE);
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, HAIRLINE);
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, TEAL);
    style.visuals.widgets.inactive.rounding = 2.0.into();
    style.visuals.widgets.hovered.rounding = 2.0.into();
    style.visuals.widgets.active.rounding = 2.0.into();
    style.visuals.selection.bg_fill = SAGE;
    style.visuals.selection.stroke.color = TEAL;
    ctx.set_style(style);
}

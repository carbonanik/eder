use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, RichText, Vec2};

pub(crate) const MATERIAL_SYMBOLS_FONT: &str = "material_symbols_outlined";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    ChevronLeft,
    ChevronRight,
    ArrowLeft,
    Plus,
    Minus,
    Hand,
    Text,
    Copy,
    Page,
    Fullscreen,
    Star,
    Close,
    Folder,
    Bookmark,
    Annotate,
    Note,
    MoreVert,
}

impl UiIcon {
    pub(crate) const fn glyph(self) -> char {
        match self {
            Self::Library => '\u{e02f}',
            Self::Clock => '\u{efd6}',
            Self::Heart => '\u{e87e}',
            Self::Settings => '\u{e8b8}',
            Self::Search => '\u{ef7a}',
            Self::Grid => '\u{e9b0}',
            Self::List => '\u{e8ef}',
            Self::Refresh => '\u{e5d5}',
            Self::Home => '\u{e9b2}',
            Self::ChevronLeft => '\u{e5cb}',
            Self::ChevronRight => '\u{e5cc}',
            Self::ArrowLeft => '\u{e5c4}',
            Self::Plus => '\u{e145}',
            Self::Minus => '\u{e15b}',
            Self::Hand => '\u{e925}',
            Self::Text => '\u{e262}',
            Self::Copy => '\u{e14d}',
            Self::Page => '\u{ef42}',
            Self::Fullscreen => '\u{e5d0}',
            Self::Star => '\u{f09a}',
            Self::Close => '\u{e5cd}',
            Self::Folder => '\u{e2c7}',
            Self::Bookmark => '\u{e8e7}',
            Self::Annotate => '\u{f097}',
            Self::Note => '\u{f1fc}',
            Self::MoreVert => '\u{e5d4}',
        }
    }
}

pub(crate) fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        MATERIAL_SYMBOLS_FONT.to_owned(),
        FontData::from_owned(
            include_bytes!("../../assets/fonts/MaterialSymbolsOutlined.ttf").to_vec(),
        ),
    );
    fonts.families.insert(
        FontFamily::Name(MATERIAL_SYMBOLS_FONT.into()),
        vec![MATERIAL_SYMBOLS_FONT.to_owned()],
    );
    ctx.set_fonts(fonts);
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
            RichText::new(format!(
                "{}{}",
                if label.is_empty() { "" } else { " " },
                label
            ))
            .size(style.text_size)
            .color(Color32::TRANSPARENT),
        )
        .min_size(style.size)
        .fill(style.fill)
        .rounding(style.rounding),
    );

    let icon_size = 18.0_f32.min(style.size.y * 0.62);
    let icon_font = FontId::new(icon_size, FontFamily::Name(MATERIAL_SYMBOLS_FONT.into()));
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(style.text_size),
        style.color,
    );
    let gap = if label.is_empty() { 0.0 } else { 8.0 };
    let group_width = icon_size + gap + galley.size().x;
    let group_left = response.rect.center().x - group_width * 0.5;

    ui.painter().text(
        egui::pos2(group_left + icon_size * 0.5, response.rect.center().y),
        egui::Align2::CENTER_CENTER,
        icon.glyph(),
        icon_font,
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

pub(crate) fn paint_icon(painter: &egui::Painter, rect: egui::Rect, icon: UiIcon, color: Color32) {
    let size = rect.width().min(rect.height()) * 0.72;
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon.glyph(),
        FontId::new(size, FontFamily::Name(MATERIAL_SYMBOLS_FONT.into())),
        color,
    );
}

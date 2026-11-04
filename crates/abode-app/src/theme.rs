//! Abode's look: warm off-white canvas, floating white panels, forest green
//! for primary actions and selection, coral for highlights and warnings,
//! Inter for UI text and Instrument Serif for titles.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, Response,
    RichText, Sense, Shadow, Stroke, TextStyle, Ui, Vec2, collapsing_header::CollapsingState, vec2,
};
use std::hash::Hash;
use std::sync::Arc;

pub use egui_phosphor::regular as icon;

/// Palette.
pub mod c {
    use eframe::egui::Color32;

    pub const FOREST: Color32 = Color32::from_rgb(14, 74, 66);
    pub const TEAL: Color32 = Color32::from_rgb(22, 134, 118);
    pub const MINT: Color32 = Color32::from_rgb(221, 236, 231);
    pub const MINT_SOFT: Color32 = Color32::from_rgb(234, 244, 240);
    pub const CORAL: Color32 = Color32::from_rgb(234, 90, 54);
    pub const CORAL_DARK: Color32 = Color32::from_rgb(196, 68, 40);
    pub const CORAL_SOFT: Color32 = Color32::from_rgb(253, 233, 226);
    pub const AMBER: Color32 = Color32::from_rgb(176, 112, 10);
    pub const AMBER_SOFT: Color32 = Color32::from_rgb(252, 241, 215);
    pub const CANVAS: Color32 = Color32::from_rgb(244, 243, 239);
    pub const GRID_MINOR: Color32 = Color32::from_rgb(232, 230, 224);
    pub const GRID_MAJOR: Color32 = Color32::from_rgb(216, 213, 205);
    pub const PANEL: Color32 = Color32::WHITE;
    pub const CARD: Color32 = Color32::from_rgb(250, 249, 246);
    pub const FIELD: Color32 = Color32::from_rgb(244, 242, 237);
    pub const BORDER: Color32 = Color32::from_rgb(234, 230, 221);
    pub const BORDER_STRONG: Color32 = Color32::from_rgb(214, 209, 198);
    pub const HOVER: Color32 = Color32::from_rgb(243, 241, 235);
    pub const INK: Color32 = Color32::from_rgb(30, 37, 34);
    pub const MUTED: Color32 = Color32::from_rgb(108, 114, 109);
    pub const FAINT: Color32 = Color32::from_rgb(158, 162, 156);
    pub const WALL: Color32 = Color32::from_rgb(36, 42, 43);
}

pub const MARGIN: f32 = 16.0;
pub const LEFT_W: f32 = 272.0;
pub const RIGHT_W: f32 = 376.0;

pub fn serif() -> FontFamily {
    FontFamily::Name("serif".into())
}
pub fn medium() -> FontFamily {
    FontFamily::Name("medium".into())
}
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add("inter", include_bytes!("../assets/fonts/Inter-Regular.ttf"));
    add("inter-medium", include_bytes!("../assets/fonts/Inter-Medium.ttf"));
    add("inter-semibold", include_bytes!("../assets/fonts/Inter-SemiBold.ttf"));
    add("serif", include_bytes!("../assets/fonts/InstrumentSerif-Regular.ttf"));
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    // Inter first; egui's defaults (plus Phosphor icons) as fallbacks for symbols.
    let defaults = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut prop = vec!["inter".to_owned()];
    prop.extend(defaults.iter().cloned());
    fonts.families.insert(FontFamily::Proportional, prop);
    for (family, primary) in [("medium", "inter-medium"), ("semibold", "inter-semibold"), ("serif", "serif")] {
        let mut list = vec![primary.to_owned()];
        if family == "serif" {
            list.push("inter".to_owned());
        }
        list.extend(defaults.iter().cloned());
        fonts.families.insert(FontFamily::Name(family.into()), list);
    }
    ctx.set_fonts(fonts);
    ctx.set_theme(egui::ThemePreference::Light);
    ctx.style_mut_of(egui::Theme::Light, configure);
}

fn configure(style: &mut egui::Style) {
    use TextStyle::*;
    style.text_styles = [
        (Heading, FontId::new(26.0, serif())),
        (Body, FontId::new(14.0, FontFamily::Proportional)),
        (Button, FontId::new(14.0, medium())),
        (Small, FontId::new(12.0, FontFamily::Proportional)),
        (Monospace, FontId::new(13.0, FontFamily::Monospace)),
    ]
    .into();
    let s = &mut style.spacing;
    s.item_spacing = vec2(8.0, 8.0);
    s.button_padding = vec2(12.0, 6.0);
    s.interact_size = vec2(36.0, 30.0);
    s.window_margin = Margin::same(20);
    s.menu_margin = Margin::same(8);
    s.indent = 16.0;
    s.icon_width = 16.0;
    s.icon_spacing = 6.0;
    s.combo_height = 320.0;

    let v = &mut style.visuals;
    *v = egui::Visuals::light();
    v.panel_fill = c::CANVAS;
    v.window_fill = c::PANEL;
    v.window_stroke = Stroke::new(1.0, c::BORDER);
    v.window_corner_radius = CornerRadius::same(18);
    v.window_shadow = Shadow { offset: [0, 12], blur: 40, spread: 0, color: Color32::from_black_alpha(30) };
    v.popup_shadow = Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(26) };
    v.menu_corner_radius = CornerRadius::same(12);
    v.extreme_bg_color = c::FIELD;
    v.faint_bg_color = c::CARD;
    v.code_bg_color = c::FIELD;
    v.hyperlink_color = c::FOREST;
    v.selection.bg_fill = c::MINT;
    v.selection.stroke = Stroke::new(1.5, c::FOREST);
    v.error_fg_color = c::CORAL_DARK;
    v.warn_fg_color = c::AMBER;
    v.collapsing_header_frame = false;
    v.indent_has_left_vline = false;
    v.slider_trailing_fill = true;
    v.window_highlight_topmost = false;
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);

    let radius = CornerRadius::same(10);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = c::PANEL;
    w.noninteractive.weak_bg_fill = c::PANEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, c::BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0, c::INK);
    w.noninteractive.corner_radius = radius;
    for (state, bg, weak, stroke, fg) in [
        (&mut w.inactive, c::FIELD, c::PANEL, c::BORDER, c::INK),
        (&mut w.hovered, c::HOVER, c::HOVER, c::BORDER_STRONG, c::INK),
        (&mut w.active, c::MINT, c::MINT, c::FOREST, c::FOREST),
        (&mut w.open, c::HOVER, c::HOVER, c::BORDER_STRONG, c::INK),
    ] {
        state.bg_fill = bg;
        state.weak_bg_fill = weak;
        state.bg_stroke = Stroke::new(1.0, stroke);
        state.fg_stroke = Stroke::new(1.3, fg);
        state.corner_radius = radius;
        state.expansion = 0.0;
    }
    style.interaction.tooltip_delay = 0.35;
}

// ---------------------------------------------------------------------------
// Text helpers

pub fn title(text: impl Into<String>) -> RichText {
    RichText::new(text).family(serif()).size(28.0).color(c::INK)
}
pub fn strong(text: impl Into<String>) -> RichText {
    RichText::new(text).family(semibold()).color(c::INK)
}
pub fn caps(text: impl AsRef<str>) -> RichText {
    RichText::new(text.as_ref().to_uppercase()).size(11.0).family(semibold()).color(c::FAINT)
}
pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text).color(c::MUTED)
}
pub fn with_icon(i: &str, text: &str) -> String {
    format!("{i}  {text}")
}

// ---------------------------------------------------------------------------
// Containers

pub fn shadow() -> Shadow {
    Shadow { offset: [0, 6], blur: 28, spread: 0, color: Color32::from_black_alpha(18) }
}

/// A floating white panel.
pub fn panel() -> Frame {
    Frame::new()
        .fill(c::PANEL)
        .stroke(Stroke::new(1.0, c::BORDER))
        .corner_radius(CornerRadius::same(18))
        .inner_margin(Margin::same(14))
        .shadow(shadow())
}

/// A compact floating pill (toolbars, status).
pub fn pill() -> Frame {
    panel().inner_margin(Margin::same(6)).corner_radius(CornerRadius::same(14))
}

/// A soft card inside a panel (notes, reminders...).
pub fn card() -> Frame {
    Frame::new()
        .fill(c::CARD)
        .stroke(Stroke::new(1.0, c::BORDER))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(12))
}

pub fn divider(ui: &mut Ui) {
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, c::BORDER);
    ui.add_space(4.0);
}

pub fn vdivider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 24.0), Sense::hover());
    ui.painter()
        .line_segment([rect.center_top(), rect.center_bottom()], Stroke::new(1.0, c::BORDER));
}

/// A collapsible section with an icon and title, used throughout the inspector.
pub fn section<R>(
    ui: &mut Ui,
    id_salt: impl Hash,
    i: &str,
    label: &str,
    count: Option<usize>,
    default_open: bool,
    body: impl FnOnce(&mut Ui) -> R,
) {
    let id = ui.make_persistent_id(id_salt);
    let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect.expand2(vec2(4.0, 0.0)), 8.0, c::HOVER);
    }
    p.text(rect.left_center() + vec2(2.0, 0.0), Align2::LEFT_CENTER, i, FontId::proportional(17.0), c::FOREST);
    let t = p.text(
        rect.left_center() + vec2(28.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::new(14.0, semibold()),
        c::INK,
    );
    if let Some(n) = count {
        p.text(t.right_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, n.to_string(), FontId::proportional(13.0), c::FAINT);
    }
    let caret = if state.is_open() { icon::CARET_DOWN } else { icon::CARET_RIGHT };
    p.text(rect.right_center() - vec2(4.0, 0.0), Align2::RIGHT_CENTER, caret, FontId::proportional(14.0), c::MUTED);
    if resp.clicked() {
        state.toggle(ui);
    }
    state.store(ui.ctx());
    state.show_body_unindented(ui, |ui| {
        ui.add_space(2.0);
        body(ui);
        ui.add_space(6.0);
    });
    divider(ui);
}

// ---------------------------------------------------------------------------
// Buttons and chips

pub fn primary_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(Color32::WHITE).family(medium()))
            .fill(c::FOREST)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(10))
            .min_size(vec2(0.0, 34.0)),
    )
}

pub fn secondary_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).family(medium()).color(c::INK))
            .fill(c::PANEL)
            .stroke(Stroke::new(1.0, c::BORDER_STRONG))
            .corner_radius(CornerRadius::same(10))
            .min_size(vec2(0.0, 34.0)),
    )
}

/// Small, quiet "add" style button.
pub fn soft_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).family(medium()).color(c::FOREST).size(13.0))
            .fill(c::MINT_SOFT)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(8)),
    )
}

pub fn danger_link(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(egui::Button::new(RichText::new(text).color(c::CORAL_DARK).family(medium())).frame(false))
}

/// A square icon button, highlighted when `active`.
pub fn icon_button(ui: &mut Ui, i: &str, tooltip: &str, active: bool) -> Response {
    icon_button_sized(ui, i, tooltip, active, 34.0)
}

pub fn icon_button_sized(ui: &mut Ui, i: &str, tooltip: &str, active: bool, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let p = ui.painter();
    if active {
        p.rect_filled(rect, 10.0, c::MINT);
    } else if resp.hovered() {
        p.rect_filled(rect, 10.0, c::HOVER);
    }
    let color = if active {
        c::FOREST
    } else if ui.is_enabled() {
        c::INK
    } else {
        c::FAINT
    };
    p.text(rect.center(), Align2::CENTER_CENTER, i, FontId::proportional(size * 0.53), color);
    if tooltip.is_empty() { resp } else { resp.on_hover_text(tooltip) }
}

pub fn chip(ui: &mut Ui, text: impl Into<String>, fg: Color32, bg: Color32) -> Response {
    Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(99))
        .inner_margin(Margin::symmetric(9, 3))
        .show(ui, |ui| ui.label(RichText::new(text).size(12.0).family(medium()).color(fg)))
        .response
}

/// A rounded square holding an icon (dialog and empty-state headers).
pub fn icon_tile(ui: &mut Ui, i: &str, fg: Color32, bg: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(38.0), Sense::hover());
    ui.painter().rect(rect, 11.0, bg, Stroke::new(1.0, c::BORDER), egui::StrokeKind::Inside);
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, i, FontId::proportional(20.0), fg);
}

/// Segmented control (e.g. Plan | 3D).
pub fn segmented<T: Copy + PartialEq>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) {
    Frame::new()
        .fill(c::FIELD)
        .corner_radius(CornerRadius::same(11))
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(|ui| {
                for (v, label) in options {
                    let selected = *value == *v;
                    let b = egui::Button::new(
                        RichText::new(*label)
                            .family(medium())
                            .size(13.0)
                            .color(if selected { c::FOREST } else { c::MUTED }),
                    )
                    .fill(if selected { c::PANEL } else { Color32::TRANSPARENT })
                    .stroke(if selected { Stroke::new(1.0, c::BORDER) } else { Stroke::NONE })
                    .corner_radius(CornerRadius::same(8))
                    .min_size(vec2(0.0, 28.0));
                    if ui.add(b).clicked() {
                        *value = *v;
                    }
                }
            });
        });
}

/// Mix two colours (t = 0 → a, t = 1 → b).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// An icon for a room, guessed from its type or name.
pub fn room_icon(kind: &str, name: &str) -> &'static str {
    let k = format!("{kind} {name}").to_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| k.contains(x));
    if has(&["bed", "nursery"]) {
        icon::BED
    } else if has(&["bath", "toilet", "powder", "ensuite", "washroom"]) {
        icon::BATHTUB
    } else if has(&["kitchen", "pantry"]) {
        icon::COOKING_POT
    } else if has(&["dining"]) {
        icon::FORK_KNIFE
    } else if has(&["living", "family", "lounge", "den"]) {
        icon::COUCH
    } else if has(&["office", "study"]) {
        icon::DESK
    } else if has(&["laundry"]) {
        icon::WASHING_MACHINE
    } else if has(&["hall", "entry", "foyer", "corridor", "mudroom"]) {
        icon::FOOTPRINTS
    } else if has(&["stair"]) {
        icon::STAIRS
    } else if has(&["garage"]) {
        icon::GARAGE
    } else if has(&["utility", "mechanical", "furnace", "boiler"]) {
        icon::WRENCH
    } else if has(&["closet", "storage", "attic"]) {
        icon::ARCHIVE
    } else if has(&["porch", "deck", "patio", "sunroom"]) {
        icon::SUN
    } else {
        icon::SQUARES_FOUR
    }
}

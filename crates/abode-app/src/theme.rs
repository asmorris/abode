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

/// Palette. Colours are functions because they depend on light/dark mode and
/// on high contrast, which the app sets with [`c::set_dark`] and
/// [`c::set_high_contrast`].
pub mod c {
    use eframe::egui::Color32;
    use std::sync::atomic::{AtomicBool, Ordering};

    static DARK: AtomicBool = AtomicBool::new(false);
    static HIGH: AtomicBool = AtomicBool::new(false);

    pub fn set_dark(dark: bool) {
        DARK.store(dark, Ordering::Relaxed);
    }

    pub fn is_dark() -> bool {
        DARK.load(Ordering::Relaxed)
    }

    pub fn set_high_contrast(high: bool) {
        HIGH.store(high, Ordering::Relaxed);
    }

    /// Black and white, strong borders, no faint text: for people who need
    /// more contrast to see what's on screen.
    pub fn is_high_contrast() -> bool {
        HIGH.load(Ordering::Relaxed)
    }

    const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
        Color32::from_rgb(r, g, b)
    }

    const WHITE: Color32 = Color32::WHITE;
    const BLACK: Color32 = Color32::BLACK;

    macro_rules! palette {
        ($($(#[$m:meta])* $name:ident: $light:expr, $dark:expr, $hc_light:expr, $hc_dark:expr;)*) => {$(
            $(#[$m])*
            pub fn $name() -> Color32 {
                match (is_high_contrast(), is_dark()) {
                    (false, false) => $light,
                    (false, true) => $dark,
                    (true, false) => $hc_light,
                    (true, true) => $hc_dark,
                }
            }
        )*};
    }

    // Columns: light, dark, high-contrast light, high-contrast dark. The high
    // contrast colours keep text at 7:1 or better against its background
    // (WCAG AAA) and lines and outlines at 3:1 or better.
    palette! {
        /// Primary buttons, active icons, links.
        forest: rgb(14, 74, 66), rgb(64, 160, 138), rgb(0, 72, 60), rgb(110, 232, 196);
        /// Text and icons drawn on a forest, teal or coral fill.
        on_accent: WHITE, WHITE, WHITE, BLACK;
        /// Selection on the plan / in 3D.
        teal: rgb(22, 134, 118), rgb(84, 204, 178), rgb(0, 96, 82), rgb(96, 240, 216);
        /// Selected rows and tools.
        mint: rgb(221, 236, 231), rgb(34, 66, 60), rgb(198, 232, 220), rgb(0, 58, 48);
        mint_soft: rgb(234, 244, 240), rgb(29, 51, 47), rgb(226, 243, 237), rgb(0, 44, 38);
        coral: rgb(234, 90, 54), rgb(240, 112, 80), rgb(160, 30, 0), rgb(255, 140, 110);
        /// Coral for text and danger.
        coral_dark: rgb(196, 68, 40), rgb(255, 146, 118), rgb(140, 24, 0), rgb(255, 170, 148);
        coral_soft: rgb(253, 233, 226), rgb(74, 42, 34), rgb(255, 226, 216), rgb(84, 22, 8);
        /// Fill for destructive buttons (white text on top).
        danger: rgb(196, 68, 40), rgb(200, 76, 48), rgb(160, 30, 0), rgb(168, 32, 4);
        amber: rgb(176, 112, 10), rgb(236, 180, 80), rgb(112, 66, 0), rgb(255, 208, 96);
        amber_soft: rgb(252, 241, 215), rgb(68, 54, 28), rgb(255, 238, 196), rgb(72, 52, 0);
        canvas: rgb(244, 243, 239), rgb(23, 26, 26), WHITE, BLACK;
        grid_minor: rgb(232, 230, 224), rgb(32, 36, 36), rgb(226, 226, 226), rgb(44, 44, 44);
        grid_major: rgb(216, 213, 205), rgb(44, 49, 49), rgb(178, 178, 178), rgb(96, 96, 96);
        panel: Color32::WHITE, rgb(34, 38, 38), WHITE, BLACK;
        card: rgb(250, 249, 246), rgb(41, 46, 46), WHITE, BLACK;
        field: rgb(244, 242, 237), rgb(46, 51, 51), rgb(242, 242, 242), rgb(24, 24, 24);
        border: rgb(234, 230, 221), rgb(54, 60, 60), rgb(96, 96, 96), rgb(168, 168, 168);
        border_strong: rgb(214, 209, 198), rgb(74, 82, 82), BLACK, WHITE;
        hover: rgb(243, 241, 235), rgb(48, 54, 54), rgb(224, 224, 224), rgb(52, 52, 52);
        ink: rgb(30, 37, 34), rgb(236, 236, 230), BLACK, WHITE;
        muted: rgb(108, 114, 109), rgb(162, 168, 164), rgb(36, 36, 36), rgb(232, 232, 232);
        faint: rgb(158, 162, 156), rgb(112, 119, 116), rgb(64, 64, 64), rgb(204, 204, 204);
        /// Toast background (text on it is white in both modes).
        toast: rgb(30, 37, 34), rgb(58, 64, 64), BLACK, rgb(28, 28, 28);

        // ---- the plan drawing
        wall: rgb(36, 42, 43), rgb(214, 212, 204), BLACK, WHITE;
        wall_hover: rgb(70, 80, 80), rgb(250, 250, 246), rgb(0, 96, 82), rgb(96, 240, 216);
        room_fill: Color32::WHITE, rgb(37, 42, 42), WHITE, rgb(14, 14, 14);
        room_hover: rgb(251, 250, 246), rgb(43, 49, 49), rgb(234, 234, 234), rgb(44, 44, 44);
        room_selected: rgb(232, 243, 239), rgb(33, 58, 53), rgb(206, 236, 226), rgb(0, 64, 54);
        threshold: rgb(233, 230, 222), rgb(58, 64, 62), rgb(200, 200, 200), rgb(84, 84, 84);
        window_glass: Color32::WHITE, rgb(52, 64, 68), WHITE, rgb(30, 64, 84);
        item_fill: rgb(245, 243, 238), rgb(47, 52, 52), rgb(238, 238, 238), rgb(30, 30, 30);
        item_stroke: rgb(140, 138, 130), rgb(132, 138, 134), BLACK, WHITE;
        /// Things drawn for context only (the house on the Outside plan...).
        context_fill: rgb(236, 234, 228), rgb(44, 48, 48), rgb(222, 222, 222), rgb(40, 40, 40);
        context_wall: rgb(150, 150, 144), rgb(110, 114, 112), rgb(86, 86, 86), rgb(170, 170, 170);
    }

    /// How strongly outdoor/roof colours show on the plan.
    pub fn tint_strength() -> f32 {
        match (is_high_contrast(), is_dark()) {
            (true, _) => 0.28,
            (false, true) => 0.32,
            (false, false) => 0.42,
        }
    }

    /// Width of borders and outlines: thicker in high contrast.
    pub fn line() -> f32 {
        if is_high_contrast() { 1.5 } else { 1.0 }
    }

    /// Width of the outline around floating panels, cards and dialogs.
    pub fn outline() -> f32 {
        if is_high_contrast() { 2.0 } else { 1.0 }
    }
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

/// Light, dark or follow the system.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    pub fn apply(self, ctx: &egui::Context) {
        ctx.set_theme(match self {
            Appearance::System => egui::ThemePreference::System,
            Appearance::Light => egui::ThemePreference::Light,
            Appearance::Dark => egui::ThemePreference::Dark,
        });
    }
}

/// Standard or high contrast, or follow the system's accessibility setting
/// (Increase contrast on a Mac, High contrast on Windows and GNOME).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Contrast {
    #[default]
    System,
    Standard,
    High,
}

impl Contrast {
    /// Whether to use high contrast, given whether the system asks for it.
    pub fn is_high(self, system: bool) -> bool {
        match self {
            Contrast::System => system,
            Contrast::Standard => false,
            Contrast::High => true,
        }
    }
}

/// Call at the start of each frame so the palette matches egui's theme.
pub fn sync(ctx: &egui::Context) {
    c::set_dark(ctx.style().visuals.dark_mode);
}

/// Restyle egui's light and dark themes from the palette (after a contrast change).
fn configure_themes(ctx: &egui::Context) {
    let dark = c::is_dark();
    for (theme, d) in [(egui::Theme::Light, false), (egui::Theme::Dark, true)] {
        c::set_dark(d);
        ctx.style_mut_of(theme, configure);
    }
    c::set_dark(dark);
}

/// Switch high contrast on or off.
pub fn set_high_contrast(ctx: &egui::Context, high: bool) {
    if c::is_high_contrast() != high {
        c::set_high_contrast(high);
        configure_themes(ctx);
        ctx.request_repaint();
    }
}

/// Bounds for the size of everything on screen (egui's zoom factor). Above
/// 300% even a large monitor has little room left for the plan.
pub const ZOOM_MIN: f32 = 0.8;
pub const ZOOM_MAX: f32 = 3.0;

pub fn install(ctx: &egui::Context, appearance: Appearance, high_contrast: bool) {
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
    c::set_high_contrast(high_contrast);
    configure_themes(ctx);
    appearance.apply(ctx);
    sync(ctx);
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
    s.menu_margin = Margin::same(10);
    s.indent = 16.0;
    s.icon_width = 16.0;
    s.icon_spacing = 6.0;
    s.combo_height = 320.0;
    s.scroll = if c::is_high_contrast() {
        // Always-visible, wider scroll bars that are easy to find and grab.
        egui::style::ScrollStyle { bar_width: 12.0, bar_inner_margin: 4.0, bar_outer_margin: 2.0, ..egui::style::ScrollStyle::solid() }
    } else {
        egui::style::ScrollStyle {
            bar_width: 8.0,
            floating_width: 3.0,
            floating_allocated_width: 0.0,
            bar_inner_margin: 4.0,
            bar_outer_margin: 2.0,
            ..egui::style::ScrollStyle::floating()
        }
    };
    s.slider_rail_height = 6.0;

    let dark = c::is_dark();
    let high = c::is_high_contrast();
    let v = &mut style.visuals;
    *v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.handle_shape = egui::style::HandleShape::Circle;
    v.panel_fill = c::canvas();
    v.window_fill = c::panel();
    v.window_stroke = Stroke::new(c::outline(), if high { c::border_strong() } else { c::border() });
    v.window_corner_radius = CornerRadius::same(18);
    // Outlines, not shadows, separate things in high contrast.
    let alpha = if high { 0 } else if dark { 110 } else { 30 };
    v.window_shadow = Shadow { offset: [0, 12], blur: 40, spread: 0, color: Color32::from_black_alpha(alpha) };
    v.popup_shadow = Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(alpha.saturating_sub(4)) };
    v.menu_corner_radius = CornerRadius::same(12);
    v.extreme_bg_color = c::field();
    v.faint_bg_color = c::card();
    v.code_bg_color = c::field();
    v.hyperlink_color = c::forest();
    v.selection.bg_fill = c::mint();
    v.selection.stroke = Stroke::new(if high { 2.5_f32 } else { 1.5 }, c::forest());
    v.error_fg_color = c::coral_dark();
    v.warn_fg_color = c::amber();
    v.collapsing_header_frame = false;
    v.indent_has_left_vline = false;
    v.slider_trailing_fill = true;
    v.window_highlight_topmost = false;
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);

    let radius = CornerRadius::same(10);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = c::panel();
    w.noninteractive.weak_bg_fill = c::panel();
    w.noninteractive.bg_stroke = Stroke::new(c::line(), c::border());
    w.noninteractive.fg_stroke = Stroke::new(c::line(), c::ink());
    v.override_text_color = None;
    v.text_cursor.stroke = Stroke::new(2.0_f32, c::forest());
    w.noninteractive.corner_radius = radius;
    for (state, bg, weak, stroke, fg) in [
        (&mut w.inactive, c::field(), c::panel(), c::border(), c::ink()),
        (&mut w.hovered, c::hover(), c::hover(), if high { c::forest() } else { c::border_strong() }, c::ink()),
        (&mut w.active, c::mint(), c::mint(), c::forest(), c::forest()),
        (&mut w.open, c::hover(), c::hover(), c::border_strong(), c::ink()),
    ] {
        state.bg_fill = bg;
        state.weak_bg_fill = weak;
        state.bg_stroke = Stroke::new(if high { 2.0_f32 } else { 1.0 }, stroke);
        state.fg_stroke = Stroke::new(if high { 2.0_f32 } else { 1.3 }, fg);
        state.corner_radius = radius;
        state.expansion = 0.0;
    }
    style.interaction.tooltip_delay = 0.35;
}

// ---------------------------------------------------------------------------
// Text helpers

pub fn title(text: impl Into<String>) -> RichText {
    RichText::new(text).family(serif()).size(28.0).color(c::ink())
}
pub fn strong(text: impl Into<String>) -> RichText {
    RichText::new(text).family(semibold()).color(c::ink())
}
pub fn caps(text: impl AsRef<str>) -> RichText {
    RichText::new(text.as_ref().to_uppercase()).size(11.0).family(semibold()).color(c::faint())
}
pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text).color(c::muted())
}
pub fn with_icon(i: &str, text: &str) -> String {
    format!("{i}  {text}")
}

// ---------------------------------------------------------------------------
// Containers

pub fn shadow() -> Shadow {
    if c::is_high_contrast() {
        return Shadow::NONE;
    }
    let alpha = if c::is_dark() { 90 } else { 18 };
    Shadow { offset: [0, 6], blur: 28, spread: 0, color: Color32::from_black_alpha(alpha) }
}

/// Outline for floating panels and dialogs: solid black or white in high contrast.
pub fn panel_stroke() -> Stroke {
    Stroke::new(c::outline(), if c::is_high_contrast() { c::border_strong() } else { c::border() })
}

/// A floating white panel.
pub fn panel() -> Frame {
    Frame::new()
        .fill(c::panel())
        .stroke(panel_stroke())
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
        .fill(c::card())
        .stroke(Stroke::new(c::line(), c::border()))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(12))
}

pub fn divider(ui: &mut Ui) {
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), c::line()), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, c::border());
    ui.add_space(4.0);
}

pub fn vdivider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 24.0), Sense::hover());
    ui.painter()
        .line_segment([rect.center_top(), rect.center_bottom()], Stroke::new(c::line(), c::border()));
}

/// A collapsible section with an icon and title, used throughout the inspector.
fn focus_id() -> egui::Id {
    egui::Id::new("abode-focus-section")
}

/// Ask the inspector to open, and scroll to, the first section titled one of
/// `titles` the next time it's drawn: e.g. "Reminders" after clicking a
/// reminder on the Reminders page.
pub fn focus_section(ctx: &egui::Context, titles: &[&str]) {
    let titles: Vec<String> = titles.iter().map(|t| t.to_string()).collect();
    ctx.data_mut(|d| d.insert_temp(focus_id(), titles));
}

/// Forget a focus request once the inspector has been drawn (whether or not
/// it had a matching section).
pub fn clear_focus(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<Vec<String>>(focus_id()));
}

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
    let focused = ui
        .ctx()
        .data(|d| d.get_temp::<Vec<String>>(focus_id()))
        .is_some_and(|t| t.iter().any(|t| t == label));
    if focused {
        clear_focus(ui.ctx());
        state.set_open(true);
    }
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    let open = state.is_open();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::CollapsingHeader, true, open, label));
    if focused {
        resp.scroll_to_me(Some(egui::Align::TOP));
    }
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect.expand2(vec2(4.0, 0.0)), 8.0, c::hover());
    }
    focus_ring(ui, &resp, 8.0);
    p.text(rect.left_center() + vec2(2.0, 0.0), Align2::LEFT_CENTER, i, FontId::proportional(17.0), c::forest());
    let t = p.text(
        rect.left_center() + vec2(28.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::new(14.0, semibold()),
        c::ink(),
    );
    if let Some(n) = count {
        p.text(t.right_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, n.to_string(), FontId::proportional(13.0), c::faint());
    }
    let caret = if state.is_open() { icon::CARET_DOWN } else { icon::CARET_RIGHT };
    p.text(rect.right_center() - vec2(4.0, 0.0), Align2::RIGHT_CENTER, caret, FontId::proportional(14.0), c::muted());
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
        egui::Button::new(RichText::new(text).color(c::on_accent()).family(medium()))
            .fill(c::forest())
            .stroke(if c::is_high_contrast() { Stroke::new(2.0_f32, c::border_strong()) } else { Stroke::NONE })
            .corner_radius(CornerRadius::same(10))
            .min_size(vec2(0.0, BUTTON_H)),
    )
}

pub fn secondary_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(secondary_button_widget(text))
}

/// The secondary button itself, for places that need the widget (a menu).
pub fn secondary_button_widget<'a>(text: impl Into<String>) -> egui::Button<'a> {
    egui::Button::new(RichText::new(text).family(medium()).color(c::ink()))
        .fill(c::panel())
        .stroke(Stroke::new(if c::is_high_contrast() { 2.0_f32 } else { 1.0 }, c::border_strong()))
        .corner_radius(CornerRadius::same(10))
        .min_size(vec2(0.0, BUTTON_H))
}

/// Small, quiet "add" style button.
pub fn soft_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).family(medium()).color(c::forest()).size(13.0))
            .fill(c::mint_soft())
            .stroke(if c::is_high_contrast() { Stroke::new(1.5_f32, c::forest()) } else { Stroke::NONE })
            .corner_radius(CornerRadius::same(8)),
    )
}

pub fn danger_link(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(egui::Button::new(RichText::new(text).color(c::coral_dark()).family(medium())).frame(false))
}

/// A square icon button, highlighted when `active`.
pub fn icon_button(ui: &mut Ui, i: &str, tooltip: &str, active: bool) -> Response {
    icon_button_sized(ui, i, tooltip, active, 34.0)
}

pub fn icon_button_sized(ui: &mut Ui, i: &str, tooltip: &str, active: bool, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    // Screen readers read the tooltip as the button's name.
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), active, tooltip));
    let p = ui.painter();
    if active {
        p.rect_filled(rect, 10.0, c::mint());
        if c::is_high_contrast() {
            p.rect_stroke(rect.shrink(1.0), 9.0, Stroke::new(2.0_f32, c::forest()), egui::StrokeKind::Inside);
        }
    } else if resp.hovered() {
        p.rect_filled(rect, 10.0, c::hover());
    }
    focus_ring(ui, &resp, 10.0);
    let color = if active {
        c::forest()
    } else if ui.is_enabled() {
        c::ink()
    } else {
        c::faint()
    };
    p.text(rect.center(), Align2::CENTER_CENTER, i, FontId::proportional(size * 0.53), color);
    if tooltip.is_empty() { resp } else { resp.on_hover_text(tooltip) }
}

/// Outline a custom-drawn control while it has keyboard focus (Tab), so
/// people who don't use a mouse can see where they are.
pub fn focus_ring(ui: &Ui, resp: &Response, radius: f32) {
    if resp.has_focus() {
        let width = if c::is_high_contrast() { 3.0_f32 } else { 2.0 };
        ui.painter()
            .rect_stroke(resp.rect.expand(1.0), radius + 1.0, Stroke::new(width, c::forest()), egui::StrokeKind::Outside);
    }
}

/// Height of the standard buttons, so badges beside them can match.
pub const BUTTON_H: f32 = 34.0;

/// A small status badge (e.g. "Due today").
pub fn chip(ui: &mut Ui, text: impl Into<String>, fg: Color32, bg: Color32) -> Response {
    chip_sized(ui, text, fg, bg, 24.0)
}

/// A badge of a fixed height: pass [`BUTTON_H`] to sit level with buttons.
/// Drawn at an exact size (a frame would stretch to fill its row) with
/// corners rounded like the buttons'.
pub fn chip_sized(ui: &mut Ui, text: impl Into<String>, fg: Color32, bg: Color32, height: f32) -> Response {
    let tall = height >= 30.0;
    let galley = egui::WidgetText::from(RichText::new(text.into()).size(if tall { 13.0 } else { 12.0 }).family(medium()).color(fg))
        .into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, TextStyle::Body);
    let pad = if tall { 12.0 } else { 9.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 2.0 * pad, height), Sense::hover());
    let text = galley.text().to_owned();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text));
    let radius = if tall { 10.0 } else { 7.0 };
    ui.painter().rect_filled(rect, radius, bg);
    if c::is_high_contrast() {
        // Soft fills barely differ from the panel, so outline the badge in its text colour.
        ui.painter().rect_stroke(rect, radius, Stroke::new(1.5_f32, fg), egui::StrokeKind::Inside);
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp
}

/// A rounded square holding an icon (dialog and empty-state headers).
pub fn icon_tile(ui: &mut Ui, i: &str, fg: Color32, bg: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(38.0), Sense::hover());
    ui.painter().rect(rect, 11.0, bg, Stroke::new(c::line(), c::border()), egui::StrokeKind::Inside);
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, i, FontId::proportional(20.0), fg);
}

/// Segmented control (e.g. Plan | 3D).
pub fn segmented<T: Copy + PartialEq>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) {
    Frame::new()
        .fill(c::field())
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
                            .color(if selected { c::forest() } else { c::muted() }),
                    )
                    .fill(if selected { c::panel() } else { Color32::TRANSPARENT })
                    .stroke(match (selected, c::is_high_contrast()) {
                        (true, true) => Stroke::new(2.0_f32, c::forest()),
                        (true, false) => Stroke::new(1.0_f32, c::border()),
                        (false, _) => Stroke::NONE,
                    })
                    .corner_radius(CornerRadius::same(8))
                    .min_size(vec2(0.0, 28.0));
                    let r = ui.add(b);
                    // Read out as a set of radio buttons: "Dark, selected".
                    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, *label));
                    if r.clicked() {
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

/// An icon for a placed item: vehicles get one for their kind.
pub fn object_icon(o: &abode_core::PlacedObject) -> &'static str {
    if o.is_stairs() {
        return icon::STEPS;
    }
    let Some(v) = &o.vehicle else {
        let l = o.label.to_lowercase();
        return if l.contains("charger") && (l.contains("ev") || l.contains("car")) { icon::CHARGING_STATION } else { icon::PACKAGE };
    };
    let b = v.body.to_lowercase();
    if b.contains("motorcycle") {
        icon::MOTORCYCLE
    } else if b.contains("boat") {
        icon::BOAT
    } else if b.contains("trailer") || b.contains("rv") {
        icon::TRUCK_TRAILER
    } else if b.contains("pickup") || b.contains("truck") {
        icon::TRUCK
    } else if b.contains("van") {
        icon::VAN
    } else if b.contains("suv") || b.contains("jeep") {
        icon::JEEP
    } else {
        icon::CAR
    }
}

/// An icon for something outside.
pub fn feature_icon(kind: abode_core::FeatureKind) -> &'static str {
    use abode_core::FeatureKind as K;
    match kind {
        K::Deck => icon::STEPS,
        K::Patio => icon::GRID_FOUR,
        K::Driveway => icon::ROAD_HORIZON,
        K::Carport => icon::GARAGE,
        K::Path => icon::PATH,
        K::GardenBed => icon::FLOWER_TULIP,
        K::Pool => icon::SWIMMING_POOL,
        K::Lawn => icon::PLANT,
        K::PropertyLine => icon::BOUNDING_BOX,
        K::Fence => icon::LINE_SEGMENTS,
        K::RetainingWall => icon::WALL,
        K::Hedge => icon::TREE_EVERGREEN,
        K::Tree => icon::TREE,
        K::Other => icon::POLYGON,
    }
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
    } else if has(&["shed", "workshop", "barn"]) {
        icon::WAREHOUSE
    } else {
        icon::SQUARES_FOUR
    }
}

// ---------------------------------------------------------------------------
// Replacements for stock egui widgets

fn caret_icon(ui: &Ui, rect: egui::Rect, _v: &egui::style::WidgetVisuals, open: bool, _above: egui::AboveOrBelow) {
    let i = if open { icon::CARET_UP } else { icon::CARET_DOWN };
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, i, FontId::proportional(13.0), c::muted());
}

/// A dropdown with Abode's caret.
pub fn combo_box(id_salt: impl Hash) -> egui::ComboBox {
    egui::ComboBox::from_id_salt(id_salt).icon(caret_icon)
}

/// An on/off switch (replaces checkboxes).
pub fn toggle(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let h = 20.0;
    let w = 36.0;
    let galley = (!label.is_empty()).then(|| {
        ui.painter()
            .layout_no_wrap(label.to_string(), FontId::proportional(14.0), c::ink())
    });
    let text_w = galley.as_ref().map(|g| g.size().x + 8.0).unwrap_or(0.0);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(w + text_w, 26.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let checked = *on;
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), checked, label));
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let track = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - h / 2.0), vec2(w, h));
    let p = ui.painter();
    let x = egui::lerp(track.left() + h / 2.0..=track.right() - h / 2.0, t);
    let knob = egui::pos2(x, track.center().y);
    if c::is_high_contrast() {
        // Off: an outlined track with a knob in the text colour. On: a solid
        // track. On or off reads from the shape, not only the colour.
        p.rect(track, h / 2.0, mix(c::panel(), c::forest(), t), Stroke::new(2.0_f32, mix(c::ink(), c::forest(), t)), egui::StrokeKind::Inside);
        p.circle_filled(knob, h / 2.0 - 4.0, mix(c::ink(), c::on_accent(), t));
    } else {
        p.rect_filled(track, h / 2.0, mix(c::border_strong(), c::forest(), t));
        p.circle_filled(knob, h / 2.0 - 2.5, Color32::WHITE);
    }
    focus_ring(ui, &resp, h / 2.0);
    if let Some(g) = galley {
        p.galley(egui::pos2(track.right() + 8.0, rect.center().y - g.size().y / 2.0), g, c::ink());
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::{c, mix};
    use eframe::egui::Color32;

    /// WCAG contrast ratio between two opaque colours.
    fn contrast(a: Color32, b: Color32) -> f32 {
        let lum = |c: Color32| {
            let ch = |v: u8| {
                let v = v as f32 / 255.0;
                if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    /// In both high contrast themes, text reaches WCAG AAA (7:1) on every
    /// background it's drawn on, and outlines reach 3:1.
    #[test]
    fn high_contrast_palette_meets_wcag() {
        let mut failures = Vec::new();
        let mut check = |mode: &str, name: String, r: f32, min: f32| {
            if r < min {
                failures.push(format!("{mode}: {name} is {r:.2}:1 (needs {min}:1)"));
            }
        };
        for dark in [false, true] {
            c::set_dark(dark);
            c::set_high_contrast(true);
            let mode = if dark { "dark" } else { "light" };
            let backgrounds = [
                ("panel", c::panel()),
                ("canvas", c::canvas()),
                ("card", c::card()),
                ("field", c::field()),
                ("hover", c::hover()),
                ("mint", c::mint()),
            ];
            let text = [("ink", c::ink()), ("muted", c::muted()), ("faint", c::faint()), ("forest", c::forest())];
            for (bn, bg) in backgrounds {
                for (tn, fg) in text {
                    check(mode, format!("{tn} on {bn}"), contrast(fg, bg), 7.0);
                }
                check(mode, format!("border on {bn}"), contrast(c::border(), bg), 3.0);
            }
            for (name, fg, bg) in [
                ("coral_dark on panel", c::coral_dark(), c::panel()),
                ("coral_dark on coral_soft", c::coral_dark(), c::coral_soft()),
                ("amber on panel", c::amber(), c::panel()),
                ("amber on amber_soft", c::amber(), c::amber_soft()),
                ("forest on mint_soft", c::forest(), c::mint_soft()),
                ("on_accent on forest", c::on_accent(), c::forest()),
                ("on_accent on teal", c::on_accent(), c::teal()),
                ("on_accent on coral", c::on_accent(), c::coral()),
                ("white on danger", Color32::WHITE, c::danger()),
                ("white on toast", Color32::WHITE, c::toast()),
                ("wall on room", c::wall(), c::room_fill()),
                ("wall on canvas", c::wall(), c::canvas()),
                ("item outline on item", c::item_stroke(), c::item_fill()),
            ] {
                check(mode, name.into(), contrast(fg, bg), 7.0);
            }
            for (name, fg, bg) in [
                ("selection on room", c::teal(), c::room_fill()),
                ("coral on canvas", c::coral(), c::canvas()),
            ] {
                check(mode, name.into(), contrast(fg, bg), 3.0);
            }
            // A switched-on toggle's track stands out from the panel.
            check(mode, "toggle track".into(), contrast(mix(c::panel(), c::forest(), 1.0), c::panel()), 3.0);
        }
        c::set_high_contrast(false);
        c::set_dark(false);
        assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    }
}

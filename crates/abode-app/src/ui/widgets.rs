//! Small reusable form widgets.

use crate::theme::{self, c, icon};
use abode_core::units::{UnitSystem, format_length, parse_length};
use abode_core::{CustomField, ItemDetails};
use chrono::{DateTime, Local, NaiveDate, Utc};
use eframe::egui::{self, RichText, Ui};
use std::hash::Hash;

pub fn fmt_date(d: NaiveDate) -> String {
    d.format("%b %-d, %Y").to_string()
}

pub fn fmt_datetime(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%b %-d, %Y %H:%M").to_string()
}

pub fn fmt_size(bytes: u64) -> String {
    match bytes {
        b if b < 1024 => format!("{b} B"),
        b if b < 1024 * 1024 => format!("{:.0} KB", b as f64 / 1024.0),
        b => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
    }
}

/// A text box for a length in the project's units. Accepts input like
/// `3.5 m`, `350cm`, `12' 6"`. Applies on Enter / focus loss.
pub fn length_edit(ui: &mut Ui, id_salt: impl Hash, value: &mut f64, units: UnitSystem) -> bool {
    length_edit_min(ui, id_salt, value, units, 1e-6)
}

fn parse_signed(text: &str, units: UnitSystem) -> Result<f64, String> {
    let t = text.trim();
    let (neg, body) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t),
    };
    let v = parse_length(body, units).map_err(|e| e.to_string())?;
    Ok(if neg { -v } else { v })
}

/// Like [`length_edit`] but allows values down to `min` (e.g. 0, or negative
/// for a basement's elevation).
pub fn length_edit_min(ui: &mut Ui, id_salt: impl Hash, value: &mut f64, units: UnitSystem, min: f64) -> bool {
    length_edit_sized(ui, id_salt, value, units, min, 110.0)
}

pub fn length_edit_sized(ui: &mut Ui, id_salt: impl Hash, value: &mut f64, units: UnitSystem, min: f64, width: f32) -> bool {
    let id = ui.make_persistent_id(id_salt);
    let mut text = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| format_length(*value, units));
    let parsed = parse_signed(&text, units).and_then(|v| {
        if v < min {
            Err(format!("must be at least {}", format_length(min.max(0.0), units)))
        } else {
            Ok(v)
        }
    });
    let mut edit = egui::TextEdit::singleline(&mut text)
        .id(id.with("edit"))
        .desired_width(width)
        .margin(egui::vec2(8.0, 5.0));
    if parsed.is_err() {
        edit = edit.text_color(c::coral_dark());
    }
    let resp = ui.add(edit);
    let mut changed = false;
    if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    } else {
        if resp.lost_focus() {
            if let Ok(v) = parsed {
                if (v - *value).abs() > 1e-9 {
                    *value = v;
                    changed = true;
                }
            }
        }
        ui.data_mut(|d| d.remove::<String>(id));
    }
    if let Err(e) = &parsed {
        resp.on_hover_text(e.clone());
    }
    changed
}

/// An optional distance (stored in km, shown in km or miles). Empty clears
/// it. Commits when the field loses focus.
pub fn distance_edit(ui: &mut Ui, id_salt: impl Hash, value: &mut Option<f64>, units: UnitSystem, hint: &str, width: f32) -> bool {
    use abode_core::vehicle::{format_distance, parse_distance};
    let id = ui.make_persistent_id(id_salt);
    let shown = value.map(|v| format_distance(v, units)).unwrap_or_default();
    let mut text = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| shown.clone());
    let parsed = parse_distance(&text, units);
    let bad = parsed.is_none() && !text.trim().is_empty();
    let mut edit = egui::TextEdit::singleline(&mut text)
        .id(id.with("edit"))
        .hint_text(hint)
        .desired_width(width)
        .margin(egui::vec2(8.0, 5.0));
    if bad {
        edit = edit.text_color(c::coral_dark());
    }
    let resp = ui.add(edit);
    let mut changed = false;
    if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    } else {
        if resp.lost_focus() && !bad && parsed != *value {
            *value = parsed;
            changed = true;
        }
        ui.data_mut(|d| d.remove::<String>(id));
    }
    if bad {
        resp.on_hover_text(format!("Enter a distance, e.g. 12,500 {}", abode_core::vehicle::distance_unit(units)));
    }
    changed
}

/// Optional length: shows the default as a hint, with a toggle to override.
pub fn opt_length_edit(ui: &mut Ui, id_salt: impl Hash, value: &mut Option<f64>, default: f64, units: UnitSystem) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| match value {
        Some(v) => {
            changed |= length_edit(ui, id_salt, v, units);
            if theme::icon_button_sized(ui, icon::X, "Use default", false, 26.0).clicked() {
                *value = None;
                changed = true;
            }
        }
        None => {
            ui.label(theme::muted(format!("{} (default)", format_length(default, units))));
            if theme::soft_button(ui, "Set").clicked() {
                *value = Some(default);
                changed = true;
            }
        }
    });
    changed
}

/// Calendar popup state, kept per picker while it's open.
#[derive(Clone, Copy)]
struct PickerView {
    year: i32,
    month: u32,
    years: bool,
}

fn add_months(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let m = year * 12 + month as i32 - 1 + delta;
    (m.div_euclid(12), m.rem_euclid(12) as u32 + 1)
}

/// A date button with Abode's own calendar popup. Returns true if changed.
pub fn date_picker(ui: &mut Ui, id_salt: impl Hash, date: &mut NaiveDate, today: NaiveDate) -> bool {
    use chrono::Datelike;
    let id = ui.make_persistent_id(id_salt);
    let popup_id = id.with("popup");
    let resp = ui.add(
        egui::Button::new(RichText::new(format!("{}  {}", icon::CALENDAR_BLANK, fmt_date(*date))).color(c::ink()))
            .fill(c::field())
            .stroke(egui::Stroke::new(1.0_f32, c::border()))
            .corner_radius(egui::CornerRadius::same(10))
            .min_size(egui::vec2(0.0, 30.0)),
    );
    if resp.clicked() {
        ui.data_mut(|d| d.insert_temp(id, PickerView { year: date.year(), month: date.month(), years: false }));
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    let mut changed = false;
    egui::popup::popup_below_widget(ui, popup_id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_width(7.0 * 34.0);
        let mut view = ui
            .data(|d| d.get_temp::<PickerView>(id))
            .unwrap_or(PickerView { year: date.year(), month: date.month(), years: false });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if theme::icon_button_sized(ui, icon::CARET_LEFT, "", false, 28.0).clicked() {
                if view.years {
                    view.year -= 12;
                } else {
                    (view.year, view.month) = add_months(view.year, view.month, -1);
                }
            }
            let title = if view.years {
                let start = view.year - view.year.rem_euclid(12);
                format!("{} – {}", start, start + 11)
            } else {
                NaiveDate::from_ymd_opt(view.year, view.month, 1)
                    .map(|d| d.format("%B %Y").to_string())
                    .unwrap_or_default()
            };
            let w = ui.available_width() - 30.0;
            let t = ui.add_sized(
                [w, 28.0],
                egui::Button::new(RichText::new(title).family(theme::semibold()).color(c::ink())).frame(false),
            );
            if t.on_hover_text("Pick a year").clicked() {
                view.years = !view.years;
            }
            if theme::icon_button_sized(ui, icon::CARET_RIGHT, "", false, 28.0).clicked() {
                if view.years {
                    view.year += 12;
                } else {
                    (view.year, view.month) = add_months(view.year, view.month, 1);
                }
            }
        });
        ui.add_space(4.0);
        if view.years {
            let start = view.year - view.year.rem_euclid(12);
            egui::Grid::new(id.with("years")).spacing([4.0, 4.0]).show(ui, |ui| {
                for (i, y) in (start..start + 12).enumerate() {
                    let selected = y == date.year();
                    let b = egui::Button::new(RichText::new(y.to_string()).color(if selected { egui::Color32::WHITE } else { c::ink() }))
                        .fill(if selected { c::forest() } else { egui::Color32::TRANSPARENT })
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(egui::CornerRadius::same(8))
                        .min_size(egui::vec2(52.0, 30.0));
                    if ui.add(b).clicked() {
                        view.year = y;
                        view.years = false;
                    }
                    if i % 4 == 3 {
                        ui.end_row();
                    }
                }
            });
        } else {
            let first = NaiveDate::from_ymd_opt(view.year, view.month, 1).unwrap_or(today);
            let lead = first.weekday().num_days_from_sunday() as i64;
            let start = first - chrono::Duration::days(lead);
            egui::Grid::new(id.with("days")).spacing([2.0, 2.0]).show(ui, |ui| {
                for d in ["S", "M", "T", "W", "T", "F", "S"] {
                    ui.add_sized([32.0, 20.0], egui::Label::new(RichText::new(d).size(11.0).color(c::faint())));
                }
                ui.end_row();
                for i in 0..42 {
                    let day = start + chrono::Duration::days(i);
                    let in_month = day.month() == view.month;
                    let selected = day == *date;
                    let fg = if selected {
                        egui::Color32::WHITE
                    } else if in_month {
                        c::ink()
                    } else {
                        c::faint()
                    };
                    let b = egui::Button::new(RichText::new(day.day().to_string()).size(13.0).color(fg))
                        .fill(if selected { c::forest() } else { egui::Color32::TRANSPARENT })
                        .stroke(if day == today && !selected { egui::Stroke::new(1.0_f32, c::teal()) } else { egui::Stroke::NONE })
                        .corner_radius(egui::CornerRadius::same(8))
                        .min_size(egui::vec2(32.0, 30.0));
                    if ui.add(b).clicked() {
                        *date = day;
                        changed = true;
                        ui.memory_mut(|m| m.close_popup());
                    }
                    if i % 7 == 6 {
                        ui.end_row();
                    }
                }
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if theme::soft_button(ui, "Today").clicked() {
                    *date = today;
                    changed = true;
                    ui.memory_mut(|m| m.close_popup());
                }
            });
        }
        ui.data_mut(|d| d.insert_temp(id, view));
    });
    changed
}

pub fn date_edit(ui: &mut Ui, id_salt: &str, date: &mut NaiveDate, today: NaiveDate) {
    date_picker(ui, id_salt, date, today);
}

pub fn opt_date_edit(ui: &mut Ui, id_salt: &str, date: &mut Option<NaiveDate>, today: NaiveDate) {
    ui.horizontal(|ui| match date {
        Some(d) => {
            date_picker(ui, id_salt, d, today);
            if theme::icon_button_sized(ui, icon::X, "Clear date", false, 26.0).clicked() {
                *date = None;
            }
        }
        None => {
            if theme::soft_button(ui, format!("{}  Set date", icon::CALENDAR_BLANK)).clicked() {
                *date = Some(today);
            }
        }
    });
}

pub fn field_label(ui: &mut Ui, label: &str) {
    ui.label(RichText::new(label).color(c::muted()).size(13.0));
}

pub fn text_row(ui: &mut Ui, label: &str, value: &mut String, hint: &str) {
    field_label(ui, label);
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(f32::INFINITY)
            .margin(egui::vec2(8.0, 5.0)),
    );
    ui.end_row();
}

pub fn form_grid<R>(ui: &mut Ui, id_salt: impl Hash, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Grid::new(id_salt)
        .num_columns(2)
        .spacing([12.0, 8.0])
        .min_col_width(96.0)
        .show(ui, add)
        .inner
}

/// Lists longer than this get a search box.
pub const SEARCH_OVER: usize = 6;

pub fn combo<T: Copy + PartialEq>(ui: &mut Ui, id_salt: impl Hash, value: &mut T, all: &[T], label: fn(T) -> &'static str) {
    if all.len() > SEARCH_OVER {
        let choices: Vec<Choice> = all.iter().map(|v| Choice::new(label(*v))).collect();
        let selected = all.iter().position(|v| v == value);
        if let Some(Picked::Item(i)) = search_combo(ui, id_salt, label(*value), &choices, selected, None, 0.0) {
            *value = all[i];
        }
        return;
    }
    theme::combo_box(id_salt)
        .selected_text(label(*value))
        .show_ui(ui, |ui| {
            for v in all {
                ui.selectable_value(value, *v, label(*v));
            }
        });
}

/// One entry in a searchable list.
pub struct Choice {
    pub label: String,
    /// Shown as a heading over the entries that share it.
    pub group: String,
    pub icon: &'static str,
    pub enabled: bool,
}

impl Choice {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), group: String::new(), icon: "", enabled: true }
    }
    pub fn group(mut self, g: impl Into<String>) -> Self {
        self.group = g.into();
        self
    }
    pub fn icon(mut self, i: &'static str) -> Self {
        self.icon = i;
        self
    }
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
}

pub enum Picked {
    Item(usize),
    /// Typed text that isn't one of the choices.
    Custom(String),
}

/// How well `q` (lower case) matches: 0 a word starts with it, 1 it's inside
/// the label, 2 its group starts with it.
fn match_rank(q: &str, c: &Choice) -> Option<u8> {
    if q.is_empty() {
        return Some(0);
    }
    let label = c.label.to_lowercase();
    if label.starts_with(q) || label.split(|ch: char| !ch.is_alphanumeric()).any(|w| w.starts_with(q)) {
        Some(0)
    } else if q.len() >= 3 && label.contains(q) {
        Some(1)
    } else if c.group.to_lowercase().split(|ch: char| !ch.is_alphanumeric()).any(|w| !w.is_empty() && w.starts_with(q)) {
        Some(2)
    } else {
        None
    }
}

/// The height of five options (plus a heading), so a dropdown's list never
/// gets squeezed shorter than that when there's more to scroll to.
pub fn five_rows(ui: &Ui) -> f32 {
    let sp = ui.spacing();
    let row = sp.interact_size.y.max(ui.text_style_height(&egui::TextStyle::Button) + 2.0 * sp.button_padding.y) + sp.item_spacing.y;
    row * 5.0 + ui.text_style_height(&egui::TextStyle::Body)
}

/// Screenshots: ABODE_PICKER=<name> opens the dropdown called that.
fn debug_open(ui: &Ui, popup: egui::Id, name: &str) {
    if ui.ctx().cumulative_pass_nr() < 4 && std::env::var("ABODE_PICKER").is_ok_and(|t| t == name) {
        ui.memory_mut(|m| m.open_popup(popup));
    }
}

/// Shows `add_contents` in a dropdown under `button` while it's open (a
/// click on the button opens and closes it). Near the bottom of the window
/// it opens upwards instead, so it isn't squashed. Close it from inside
/// with [`close_dropdown`].
pub fn dropdown<R>(ui: &Ui, button: &egui::Response, popup: egui::Id, add_contents: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    if button.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    let screen = ui.ctx().screen_rect();
    let below = screen.bottom() - button.rect.bottom();
    let above = button.rect.top() - screen.top();
    let dir = if below < 380.0 && above > below { egui::AboveOrBelow::Above } else { egui::AboveOrBelow::Below };
    egui::popup::popup_above_or_below_widget(ui, popup, button, dir, egui::PopupCloseBehavior::CloseOnClickOutside, add_contents)
}

pub fn close_dropdown(ui: &Ui) {
    ui.memory_mut(|m| m.close_popup());
}

/// A button that opens a dropdown (see [`dropdown`]); a stand-in for
/// `menu_button` that flips upwards near the bottom of the window.
pub fn menu_dropdown<R>(ui: &mut Ui, label: impl Into<egui::WidgetText>, add_contents: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let label: egui::WidgetText = label.into();
    let name = label.text().to_string();
    let button = ui.button(label);
    let popup = button.id.with("dropdown");
    debug_open(ui, popup, &name);
    dropdown(ui, &button, popup, add_contents)
}

/// The search box at the top of a dropdown, with room inside it for the
/// icon and caret and a little space around it.
pub fn search_field(ui: &mut Ui, query: &mut String, hint: &str) -> egui::Response {
    ui.add_space(2.0);
    // The icon is painted on its own so the text (and caret) start after it.
    let r = ui.add(
        egui::TextEdit::singleline(query)
            .hint_text(hint)
            .desired_width(f32::INFINITY)
            .margin(egui::Margin { left: 32, right: 10, top: 6, bottom: 6 }),
    );
    ui.painter().text(
        // The response rect starts where the text does, inside the 32px margin.
        egui::pos2(r.rect.left() - 21.0, r.rect.center().y),
        egui::Align2::LEFT_CENTER,
        icon::MAGNIFYING_GLASS,
        egui::FontId::proportional(14.0),
        ui.visuals().weak_text_color(),
    );
    ui.add_space(6.0);
    r
}

/// The inside of a searchable dropdown: a search box (ready to type in),
/// the matching choices under their group headings, and, when `custom` is
/// set, "＋ {custom} “what you typed”".
pub fn search_list(ui: &mut Ui, id: egui::Id, choices: &[Choice], selected: Option<usize>, custom: Option<&str>) -> Option<Picked> {
    let search_id = id.with("search");
    let seen_id = id.with("seen");
    let frame = ui.ctx().cumulative_pass_nr();
    let last = ui.data(|d| d.get_temp::<u64>(seen_id));
    let opened = last.is_none_or(|f| f + 1 < frame);
    ui.data_mut(|d| d.insert_temp(seen_id, frame));
    let mut query: String = if opened { String::new() } else { ui.data(|d| d.get_temp(search_id)).unwrap_or_default() };
    // Screenshots: ABODE_PICKER_QUERY types into the open picker.
    if opened {
        if let Ok(q) = std::env::var("ABODE_PICKER_QUERY") {
            query = q;
        }
    }
    let hint = if custom.is_some() { "Search or type" } else { "Search" };
    let edit = search_field(ui, &mut query, hint);
    if opened {
        edit.request_focus();
    }
    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    ui.data_mut(|d| d.insert_temp(search_id, query.clone()));
    let typed = query.trim().to_string();
    let q = typed.to_lowercase();
    let found: Vec<(usize, u8)> = choices.iter().enumerate().filter_map(|(i, c)| match_rank(&q, c).map(|r| (i, r))).collect();
    let exact = choices.iter().any(|c| c.label.eq_ignore_ascii_case(&typed));
    let mut pick = None;
    if enter {
        pick = found
            .iter()
            .filter(|(i, _)| choices[*i].enabled)
            .min_by_key(|(_, r)| *r)
            .map(|(i, _)| Picked::Item(*i))
            .or_else(|| (custom.is_some() && !typed.is_empty()).then(|| Picked::Custom(typed.clone())));
    }
    let use_typed = |ui: &mut Ui, pick: &mut Option<Picked>| {
        if let (Some(verb), false, false) = (custom, typed.is_empty(), exact) {
            let label = RichText::new(format!("{}  {verb} “{typed}”", icon::PLUS)).color(c::forest());
            if ui.selectable_label(false, label).clicked() {
                *pick = Some(Picked::Custom(typed.clone()));
            }
        }
    };
    if found.is_empty() {
        use_typed(ui, &mut pick);
    }
    egui::ScrollArea::vertical()
        .id_salt(id.with("scroll"))
        .max_height(340.0)
        .min_scrolled_height(five_rows(ui))
        .auto_shrink([false, true])
        .show(ui, |ui| {
        let mut group = "";
        for (i, _) in &found {
            let c = &choices[*i];
            if c.group != group && !c.group.is_empty() {
                ui.label(theme::caps(&c.group));
            }
            group = &c.group;
            let text = if c.icon.is_empty() { c.label.clone() } else { format!("{}  {}", c.icon, c.label) };
            let r = ui.add_enabled(c.enabled, egui::SelectableLabel::new(selected == Some(*i), text));
            if r.clicked() {
                pick = Some(Picked::Item(*i));
            }
        }
        if found.is_empty() && custom.is_none() {
            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                ui.label(theme::muted("Nothing matches."));
            });
        }
    });
    if !found.is_empty() {
        use_typed(ui, &mut pick);
    }
    if pick.is_some() {
        ui.data_mut(|d| d.remove::<String>(search_id));
    }
    pick
}

/// A dropdown with a search box (see [`search_list`]). `width` 0 keeps the
/// default width.
pub fn search_combo(
    ui: &mut Ui,
    id_salt: impl Hash,
    selected_text: impl Into<egui::WidgetText>,
    choices: &[Choice],
    selected: Option<usize>,
    custom: Option<&str>,
    width: f32,
) -> Option<Picked> {
    let id = ui.make_persistent_id(id_salt);
    let text: egui::WidgetText = selected_text.into();
    // Screenshots: ABODE_PICKER=<its current text> opens this picker.
    if std::env::var("ABODE_PICKER").is_ok_and(|t| t == text.text()) && ui.ctx().cumulative_pass_nr() < 4 {
        let popup = ui.make_persistent_id(egui::Id::new(id)).with("popup");
        ui.memory_mut(|m| m.open_popup(popup));
    }
    let mut combo = theme::combo_box(id).selected_text(text).height(600.0).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    if width > 0.0 {
        combo = combo.width(width);
    }
    let mut out = None;
    combo.show_ui(ui, |ui| {
        ui.set_min_width(180.0);
        out = search_list(ui, id, choices, selected, custom);
        if out.is_some() {
            ui.memory_mut(|m| m.close_popup());
        }
    });
    out
}

/// Like [`combo`], but a long list also takes a typed name ("＋ Use “…”"):
/// `value` becomes `other` and the name goes in `name`.
pub fn combo_named<T: Copy + PartialEq>(ui: &mut Ui, id_salt: impl Hash, value: &mut T, name: &mut String, all: &[T], label: fn(T) -> &'static str, other: T) {
    if all.len() <= SEARCH_OVER {
        combo(ui, id_salt, value, all, label);
        return;
    }
    let custom = *value == other && !name.trim().is_empty();
    let text = if custom { name.trim().to_string() } else { label(*value).to_string() };
    let choices: Vec<Choice> = all.iter().map(|v| Choice::new(label(*v))).collect();
    let selected = if custom { None } else { all.iter().position(|v| v == value) };
    match search_combo(ui, id_salt, text, &choices, selected, Some("Use"), 0.0) {
        Some(Picked::Item(i)) => {
            *value = all[i];
            name.clear();
        }
        Some(Picked::Custom(typed)) => {
            *value = other;
            *name = typed;
        }
        None => {}
    }
}

/// Free-text field with a dropdown of suggestions.
pub fn suggest_edit(ui: &mut Ui, id_salt: impl Hash, value: &mut String, suggestions: &[&str], hint: &str) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                // Leave room for the dropdown button (a TextEdit's width excludes its margins).
                .desired_width(ui.available_width() - ui.spacing().interact_size.x - ui.spacing().item_spacing.x - 16.0)
                .margin(egui::vec2(8.0, 5.0)),
        );
        let id = ui.make_persistent_id(&id_salt);
        let button = ui.button(icon::CARET_DOWN);
        let popup = id.with("suggest");
        debug_open(ui, popup, value);
        dropdown(ui, &button, popup, |ui| {
            if suggestions.len() > SEARCH_OVER {
                ui.set_min_width(200.0);
                let choices: Vec<Choice> = suggestions.iter().map(|s| Choice::new(*s)).collect();
                let selected = suggestions.iter().position(|s| s == value);
                match search_list(ui, id, &choices, selected, Some("Use")) {
                    Some(Picked::Item(i)) => *value = suggestions[i].to_string(),
                    Some(Picked::Custom(typed)) => *value = typed,
                    None => return,
                }
                close_dropdown(ui);
                return;
            }
            ui.set_min_width(160.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                for s in suggestions {
                    if ui.selectable_label(s == value, *s).clicked() {
                        *value = s.to_string();
                        close_dropdown(ui);
                    }
                }
            });
        });
    });
}

pub fn privacy_toggle(ui: &mut Ui, private: &mut bool) {
    let (i, tip) = if *private {
        (icon::LOCK, "Private: removed when you export a copy for a new owner. Click to make shareable.")
    } else {
        (icon::LOCK_OPEN, "Shared: included when you export a copy for a new owner. Click to make private.")
    };
    if theme::icon_button_sized(ui, i, tip, *private, 28.0).clicked() {
        *private = !*private;
    }
}

pub fn custom_fields(ui: &mut Ui, id_salt: impl Hash + Copy, fields: &mut Vec<CustomField>) {
    let mut remove = None;
    // Rows sized from the panel's width (not a Grid, whose columns stick at
    // whatever width they had on the first frame): the name takes about
    // two-fifths, the value the rest, then the privacy and remove buttons.
    // (A TextEdit's desired width excludes its 8px side margins.)
    for (i, f) in fields.iter_mut().enumerate() {
        ui.push_id((id_salt, "custom", i), |ui| {
            ui.horizontal_top(|ui| {
                let gap = 6.0;
                ui.spacing_mut().item_spacing.x = gap;
                let buttons = 28.0 * 2.0 + gap * 2.0;
                let room = (ui.available_width() - buttons - gap).max(120.0);
                let key_w = (room * 0.4).floor();
                let value_w = room - key_w;
                ui.add(egui::TextEdit::singleline(&mut f.key).hint_text("Field").desired_width(key_w - 16.0).margin(egui::vec2(8.0, 5.0)));
                // Long values wrap onto more lines, growing the row.
                ui.add(
                    egui::TextEdit::multiline(&mut f.value)
                        .hint_text("Value")
                        .desired_rows(1)
                        .desired_width(value_w - 16.0)
                        .margin(egui::vec2(8.0, 5.0)),
                );
                privacy_toggle(ui, &mut f.private);
                if theme::icon_button_sized(ui, icon::TRASH, "Remove field", false, 28.0).clicked() {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        fields.remove(i);
    }
    if theme::soft_button(ui, format!("{}  Add field", icon::PLUS)).clicked() {
        fields.push(CustomField::default());
    }
}

/// Make/model/serial/dates/warranty for appliances, doors, windows...
pub fn details_editor(ui: &mut Ui, id_salt: &str, d: &mut ItemDetails, today: NaiveDate) {
    form_grid(ui, ("details", id_salt), |ui| {
        text_row(ui, "Make", &mut d.make, "e.g. Carrier");
        text_row(ui, "Model", &mut d.model, "");
        text_row(ui, "Serial #", &mut d.serial, "");
        field_label(ui, "Purchased");
        opt_date_edit(ui, &format!("{id_salt}-purchased"), &mut d.purchased, today);
        ui.end_row();
        field_label(ui, "Installed");
        opt_date_edit(ui, &format!("{id_salt}-installed"), &mut d.installed, today);
        ui.end_row();
        text_row(ui, "Price", &mut d.price, "");
        text_row(ui, "Bought from", &mut d.vendor, "");
        field_label(ui, "Warranty until");
        opt_date_edit(ui, &format!("{id_salt}-warranty"), &mut d.warranty_expires, today);
        ui.end_row();
        text_row(ui, "Warranty details", &mut d.warranty_details, "Coverage, claim number…");
    });
    ui.add_space(6.0);
    ui.label(theme::caps("Custom fields"));
    custom_fields(ui, (id_salt, "details-custom"), &mut d.custom_fields);
}

pub fn danger_button(ui: &mut Ui, text: &str) -> bool {
    theme::danger_link(ui, format!("{}  {text}", icon::TRASH)).clicked()
}

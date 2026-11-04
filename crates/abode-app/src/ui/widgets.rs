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
        edit = edit.text_color(c::CORAL_DARK);
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

pub fn date_edit(ui: &mut Ui, id_salt: &str, date: &mut NaiveDate) {
    ui.add(egui_extras::DatePickerButton::new(date).id_salt(id_salt).calendar_week(false));
}

pub fn opt_date_edit(ui: &mut Ui, id_salt: &str, date: &mut Option<NaiveDate>, today: NaiveDate) {
    ui.horizontal(|ui| match date {
        Some(d) => {
            ui.add(egui_extras::DatePickerButton::new(d).id_salt(id_salt).calendar_week(false));
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
    ui.label(RichText::new(label).color(c::MUTED).size(13.0));
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

pub fn combo<T: Copy + PartialEq>(ui: &mut Ui, id_salt: impl Hash, value: &mut T, all: &[T], label: fn(T) -> &'static str) {
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(label(*value))
        .show_ui(ui, |ui| {
            for v in all {
                ui.selectable_value(value, *v, label(*v));
            }
        });
}

/// Free-text field with a dropdown of suggestions.
pub fn suggest_edit(ui: &mut Ui, id_salt: impl Hash, value: &mut String, suggestions: &[&str], hint: &str) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .desired_width(ui.available_width() - 52.0)
                .margin(egui::vec2(8.0, 5.0)),
        );
        ui.menu_button(icon::CARET_DOWN, |ui| {
            egui::ScrollArea::vertical().id_salt(id_salt).max_height(280.0).show(ui, |ui| {
                for s in suggestions {
                    if ui.button(*s).clicked() {
                        *value = s.to_string();
                        ui.close_menu();
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
    egui::Grid::new(("custom", id_salt)).num_columns(4).spacing([6.0, 6.0]).show(ui, |ui| {
        for (i, f) in fields.iter_mut().enumerate() {
            ui.add(egui::TextEdit::singleline(&mut f.key).hint_text("Field").desired_width(100.0));
            ui.add(egui::TextEdit::singleline(&mut f.value).hint_text("Value").desired_width(140.0));
            privacy_toggle(ui, &mut f.private);
            if theme::icon_button_sized(ui, icon::TRASH, "Remove field", false, 28.0).clicked() {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
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

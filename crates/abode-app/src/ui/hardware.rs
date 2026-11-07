//! The Hardware section: pulls, hinges, handles, towel bars and the like,
//! fitted to an item, a door or window, a room or an outbuilding.

use super::vehicle::list_card;
use super::widgets::*;
use crate::theme::{self, c, icon};
use abode_core::{HARDWARE_FINISHES, HARDWARE_KINDS, Hardware};
use eframe::egui::{self, RichText, Ui};
use std::hash::Hash;

/// Which hardware fits what, so the likeliest kinds are listed first.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fits {
    /// Cabinets, vanities, furniture.
    Item,
    Door,
    Window,
    Room,
    Outside,
}

impl Fits {
    fn first(self) -> &'static [&'static str] {
        match self {
            Fits::Item => &["Cabinet pull", "Cabinet knob", "Cabinet hinge", "Drawer slide", "Faucet", "Shelf pin"],
            Fits::Door => &["Door handle", "Door lever", "Door knob", "Deadbolt", "Door hinge", "Door stop", "Door closer"],
            Fits::Window => &["Window lock", "Window crank", "Curtain rod"],
            Fits::Room => &["Towel bar", "Towel ring", "Toilet paper holder", "Robe hook", "Coat hook", "Grab bar", "Curtain rod", "Switch plate"],
            Fits::Outside => &["Gate latch", "Gate hinge", "Door handle", "Deadbolt", "Door hinge"],
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Fits::Item => "Pulls, knobs, hinges and faucets on it, so a matching replacement is easy to find.",
            Fits::Door => "The handle, lock and hinges, so a matching replacement is easy to find.",
            Fits::Window => "Locks, cranks and curtain rods.",
            Fits::Room => "Towel bars, hooks and other fittings on the walls, so a matching replacement is easy to find.",
            Fits::Outside => "Latches, hinges, handles and locks.",
        }
    }
}

pub fn hardware_section(ui: &mut Ui, id_salt: impl Hash + Copy, list: &mut Vec<Hardware>, fits: Fits) {
    let n = list.len();
    theme::section(ui, ("hardware", id_salt), icon::SCREWDRIVER, "Hardware", Some(n), n > 0, |ui| {
        let mut remove = None;
        let mut copy = None;
        for (i, h) in list.iter_mut().enumerate() {
            let finish = h.finish.trim();
            let chip = (!finish.is_empty()).then(|| (finish.to_string(), c::muted(), c::field()));
            list_card(ui, ("hw-card", id_salt, i), &h.display_name(), chip, |ui| {
                form_grid(ui, ("hw-grid", id_salt, i), |ui| {
                    field_label(ui, "What");
                    suggest_edit(ui, ("hw-kind", id_salt, i), &mut h.kind, HARDWARE_KINDS, "e.g. Cabinet pull");
                    ui.end_row();
                    field_label(ui, "How many");
                    ui.horizontal(|ui| {
                        let mut count = h.quantity.unwrap_or(1);
                        if ui.add(egui::DragValue::new(&mut count).range(1..=999)).changed() {
                            h.quantity = Some(count);
                        }
                    });
                    ui.end_row();
                    text_row(ui, "Brand", &mut h.brand, "e.g. Amerock, Moen, Schlage");
                    text_row(ui, "Model", &mut h.model, "Product or collection name");
                    text_row(ui, "Part #", &mut h.part_number, "SKU or model number");
                    field_label(ui, "Finish");
                    suggest_edit(ui, ("hw-finish", id_salt, i), &mut h.finish, HARDWARE_FINISHES, "");
                    ui.end_row();
                    text_row(ui, "Size", &mut h.size, "e.g. 128 mm centre to centre, 24 in");
                    text_row(ui, "Bought from", &mut h.vendor, "");
                    text_row(ui, "Price", &mut h.price, "Each, or for the set");
                });
                ui.add(egui::TextEdit::multiline(&mut h.notes).hint_text("Notes (spares, where they're kept)").desired_rows(1).desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    if theme::soft_button(ui, format!("{}  Duplicate", icon::COPY)).on_hover_text("Another of the same, to change slightly").clicked() {
                        copy = Some(i);
                    }
                    if danger_button(ui, "Remove") {
                        remove = Some(i);
                    }
                });
            });
        }
        if let Some(i) = copy {
            let dup = list[i].clone();
            list.insert(i + 1, dup);
        }
        if let Some(i) = remove {
            list.remove(i);
        }
        menu_dropdown(ui, RichText::new(format!("{}  Add hardware", icon::PLUS)).color(c::forest()), |ui| {
            ui.set_min_width(260.0);
            // The likeliest kinds first, then everything else.
            let first = fits.first();
            let kinds: Vec<&str> = first.iter().copied().chain(HARDWARE_KINDS.iter().copied().filter(|k| !first.contains(k))).collect();
            let choices: Vec<Choice> = kinds
                .iter()
                .enumerate()
                .map(|(i, k)| Choice::new(*k).group(if i < first.len() { "Common" } else { "Everything else" }))
                .collect();
            let kind = match search_list(ui, ui.make_persistent_id(("hw-add", id_salt)), &choices, None, Some("Add")) {
                Some(Picked::Item(i)) => Some(kinds[i].to_string()),
                Some(Picked::Custom(k)) => Some(k),
                None => None,
            };
            if let Some(kind) = kind {
                list.push(Hardware { kind, quantity: Some(1), ..Default::default() });
                close_dropdown(ui);
            }
        });
        ui.label(theme::muted(fits.hint()));
    });
}

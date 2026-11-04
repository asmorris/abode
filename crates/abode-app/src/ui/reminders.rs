//! Reminders tab: everything that's due, plus warranties expiring.

use super::notes::status_text;
use super::widgets::*;
use super::{Action, Env};
use abode_core::reminders::DueStatus;
use abode_core::*;
use chrono::NaiveDate;
use eframe::egui::{self, RichText, Ui};

pub fn reminders_ui(ui: &mut Ui, project: &mut Project, env: &mut Env) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.heading("Reminders");
        ui.weak("Add reminders to any room, item, door or window from its panel on the right. Marking one done logs it in that item's service history.");
        if ui.button("+ House-wide reminder").clicked() {
            project.reminders.push(Reminder::new(EntityRef::General, "", env.today));
            env.select(EntityRef::General);
        }
        ui.add_space(6.0);

        let mut order: Vec<usize> = (0..project.reminders.len()).collect();
        order.sort_by_key(|&i| {
            let r = &project.reminders[i];
            (r.is_done(), r.due)
        });
        let mut complete = None;
        let mut reveal = None;
        if order.is_empty() {
            ui.weak("No reminders yet.");
        } else {
            egui::Grid::new("reminders-grid")
                .num_columns(6)
                .striped(true)
                .spacing([14.0, 6.0])
                .show(ui, |ui| {
                    for h in ["Status", "Due", "Reminder", "For", "Repeats", ""] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for i in order {
                        let r = &project.reminders[i];
                        ui.label(status_text(ui, r.status(env.today)));
                        ui.label(fmt_date(r.due));
                        ui.label(if r.title.trim().is_empty() { "(untitled)" } else { r.title.as_str() });
                        if ui.link(project.entity_name(r.target)).clicked() {
                            reveal = Some(r.target);
                        }
                        ui.label(r.repeat.map(|x| x.describe()).unwrap_or_else(|| "Once".into()));
                        if !r.is_done() && ui.button("✔ Done").clicked() {
                            complete = Some(r.id);
                        } else if r.is_done() {
                            ui.label("");
                        }
                        ui.end_row();
                    }
                });
        }
        if let Some(id) = complete {
            project.complete_reminder(id, env.today);
            env.toasts.info("Done. Logged in service history.");
        }

        ui.add_space(16.0);
        ui.heading("Warranties");
        let mut warranties: Vec<(NaiveDate, EntityRef)> = Vec::new();
        if let Some(d) = project.roof.warranty_expires {
            warranties.push((d, EntityRef::Roof));
        }
        for l in &project.levels {
            for o in l.objects.values() {
                if let Some(d) = o.details.warranty_expires {
                    warranties.push((d, EntityRef::Object(o.id)));
                }
            }
            for o in l.openings.values() {
                if let Some(d) = o.details.warranty_expires {
                    warranties.push((d, EntityRef::Opening(o.id)));
                }
            }
        }
        warranties.sort();
        if warranties.is_empty() {
            ui.weak("Add warranty dates in an item's details to see them here.");
        }
        egui::Grid::new("warranty-grid").num_columns(3).striped(true).spacing([14.0, 6.0]).show(ui, |ui| {
            for (date, target) in warranties {
                let days = (date - env.today).num_days();
                let status = if days < 0 {
                    RichText::new("Expired").weak()
                } else if days <= 90 {
                    status_text(ui, DueStatus::DueSoon(days))
                } else {
                    RichText::new(format!("{} left", fmt_span(days)))
                };
                ui.label(status);
                ui.label(fmt_date(date));
                if ui.link(project.entity_name(target)).clicked() {
                    reveal = Some(target);
                }
                ui.end_row();
            }
        });
        if let Some(r) = reveal {
            env.actions.push(Action::Reveal(r));
        }
    });
}

fn fmt_span(days: i64) -> String {
    if days > 365 {
        format!("{:.1} years", days as f64 / 365.25)
    } else if days > 60 {
        format!("{} months", days / 30)
    } else {
        format!("{days} days")
    }
}

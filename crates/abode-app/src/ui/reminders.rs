//! Reminders sheet: everything that's due, plus warranties expiring.

use super::notes::status_chip;
use super::widgets::*;
use super::{Action, Env};
use crate::theme::{self, c, icon};
use abode_core::reminders::DueStatus;
use abode_core::*;
use chrono::NaiveDate;
use eframe::egui::{self, RichText, Ui};

pub fn reminders_ui(ui: &mut Ui, project: &mut Project, env: &mut Env) {
    ui.horizontal(|ui| {
        ui.label(theme::muted(
            "Add reminders to anything (a room, an appliance, the roof, the deck) from its panel. Marking one done logs it in that item's service history.",
        ));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::primary_button(ui, format!("{}  House-wide reminder", icon::PLUS)).clicked() {
                project.reminders.push(Reminder::new(EntityRef::General, "", env.today));
                env.select(EntityRef::General);
            }
        });
    });
    ui.add_space(8.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let mut order: Vec<usize> = (0..project.reminders.len()).collect();
        order.sort_by_key(|&i| {
            let r = &project.reminders[i];
            (r.is_done(), r.due)
        });
        let mut complete = None;
        let mut reveal = None;
        if order.is_empty() {
            ui.label(theme::muted("No reminders yet."));
        }
        for i in order {
            let r = &project.reminders[i];
            theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
                    let (fg, bg) = match r.status(env.today) {
                        DueStatus::Overdue(_) => (c::coral_dark(), c::coral_soft()),
                        DueStatus::DueSoon(_) => (c::amber(), c::amber_soft()),
                        DueStatus::Done => (c::muted(), c::field()),
                        DueStatus::Upcoming(_) => (c::forest(), c::mint_soft()),
                    };
                    ui.painter().rect_filled(rect, 10.0, bg);
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, icon::BELL, egui::FontId::proportional(18.0), fg);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let title = if r.title.trim().is_empty() { "(untitled)" } else { r.title.as_str() };
                        ui.label(theme::strong(title));
                        ui.horizontal(|ui| {
                            if ui
                                .add(egui::Button::new(RichText::new(project.entity_name(r.target)).size(12.5).color(c::forest())).frame(false))
                                .clicked()
                            {
                                reveal = Some(r.target);
                            }
                            let repeat = r.repeat.map(|x| x.describe()).unwrap_or_else(|| "Once".into());
                            ui.label(RichText::new(format!("·  due {}  ·  {repeat}", fmt_date(r.due))).size(12.5).color(c::muted()));
                        });
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !r.is_done() && theme::secondary_button(ui, format!("{}  Done", icon::CHECK)).clicked() {
                            complete = Some(r.id);
                        }
                        status_chip(ui, r.status(env.today));
                    });
                });
            });
        }
        if let Some(id) = complete {
            project.complete_reminder(id, env.today);
            env.toasts.info("Done. Logged in service history.");
        }

        ui.add_space(16.0);
        ui.label(RichText::new("Warranties").family(theme::serif()).size(22.0).color(c::ink()));
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
            for r in l.roofs.values() {
                if let Some(d) = r.top_layer().and_then(|x| x.warranty_expires) {
                    warranties.push((d, EntityRef::RoofSection(r.id)));
                }
            }
            for f in l.features.values() {
                if let Some(d) = f.details.warranty_expires {
                    warranties.push((d, EntityRef::Feature(f.id)));
                }
            }
        }
        for b in &project.buildings {
            if let Some(d) = b.details.warranty_expires {
                warranties.push((d, EntityRef::Building(b.id)));
            }
        }
        warranties.sort();
        if warranties.is_empty() {
            ui.label(theme::muted("Add warranty dates in an item's details to see them here."));
        }
        for (date, target) in warranties {
            theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon::RECEIPT).size(18.0).color(c::muted()));
                    if ui
                        .add(egui::Button::new(theme::strong(project.entity_name(target))).frame(false))
                        .clicked()
                    {
                        reveal = Some(target);
                    }
                    ui.label(theme::muted(format!("until {}", fmt_date(date))));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let days = (date - env.today).num_days();
                        if days < 0 {
                            theme::chip(ui, "Expired", c::muted(), c::field());
                        } else if days <= 90 {
                            theme::chip(ui, format!("{days} days left"), c::amber(), c::amber_soft());
                        } else {
                            theme::chip(ui, format!("{} left", fmt_span(days)), c::forest(), c::mint_soft());
                        }
                    });
                });
            });
        }
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

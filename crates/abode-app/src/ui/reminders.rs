//! Reminders sheet: what's coming up, what's snoozed, what's been done, plus
//! warranties expiring.

use super::notes::status_chip_sized;
use super::widgets::*;
use super::{Action, Env};
use crate::theme::{self, c, icon};
use abode_core::reminders::DueStatus;
use abode_core::*;
use chrono::{Days, Months, NaiveDate};
use eframe::egui::{self, RichText, Ui};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Upcoming,
    Snoozed,
    Completed,
}

/// How far ahead the Upcoming tab looks, in days (0 = everything).
const HORIZONS: [(u32, &str); 5] = [(7, "Next week"), (31, "Next month"), (92, "Next 3 months"), (365, "Next year"), (0, "Everything")];

/// Panel sections to open when jumping to a reminder or warranty.
const REMINDERS: &[&str] = &["Reminders"];
/// Wherever each kind of thing keeps its warranty date.
const WARRANTY: &[&str] = &["Purchase & warranty", "Built & warranty", "Whole roof", "Layers"];

/// Something a card asked for this frame.
enum Event {
    Reveal(EntityRef, &'static [&'static str]),
    Complete(ReminderId),
    Snooze(ReminderId, NaiveDate),
    SetSnooze(ReminderId, NaiveDate),
    Unsnooze(ReminderId),
    Reopen(ReminderId),
}

/// Debug aid (screenshots): open the sheet on "snoozed" or "completed".
pub fn set_tab(ctx: &egui::Context, name: &str) {
    let tab = match name {
        "snoozed" => Tab::Snoozed,
        "completed" => Tab::Completed,
        _ => Tab::Upcoming,
    };
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("reminders-tab"), tab));
}

pub fn reminders_ui(ui: &mut Ui, project: &mut Project, env: &mut Env) {
    let today = env.today;
    ui.horizontal(|ui| {
        // Button first from the right, so the text wraps in whatever is left
        // instead of pushing the sheet wider than its header.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::primary_button(ui, format!("{}  House-wide reminder", icon::PLUS)).clicked() {
                project.reminders.push(Reminder::new(EntityRef::General, "", today));
                // Edit it in the house panel.
                theme::focus_section(ui.ctx(), REMINDERS);
                env.actions.push(Action::Reveal(EntityRef::General));
            }
            ui.add_space(16.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(
                    egui::Label::new(theme::muted(
                        "Add reminders to anything (a room, an appliance, the roof, the deck) from its panel. Marking one done logs it in that item's service history.",
                    ))
                    .wrap(),
                );
            });
        });
    });
    ui.add_space(8.0);

    // Which tab, and how far ahead Upcoming looks (remembered between runs).
    let tab_id = egui::Id::new("reminders-tab");
    let horizon_id = egui::Id::new("reminders-horizon");
    let mut tab: Tab = ui.data(|d| d.get_temp(tab_id)).unwrap_or_default();
    let mut horizon: u32 = ui.data_mut(|d| *d.get_persisted_mut_or(horizon_id, 31));
    let until = (horizon > 0).then(|| today.checked_add_days(Days::new(horizon as u64)).unwrap_or(today));

    let mut upcoming: Vec<usize> = Vec::new();
    let mut later = 0;
    let mut snoozed: Vec<usize> = Vec::new();
    let mut completed: Vec<usize> = Vec::new();
    for (i, r) in project.reminders.iter().enumerate() {
        if r.finished_on().is_some() {
            completed.push(i);
        }
        if r.is_done() {
            continue;
        }
        if r.is_snoozed(today) {
            snoozed.push(i);
        } else if until.is_none_or(|u| r.effective_due() <= u) {
            upcoming.push(i);
        } else {
            later += 1;
        }
    }
    let rs = &project.reminders;
    upcoming.sort_by_key(|&i| rs[i].effective_due());
    snoozed.sort_by_key(|&i| rs[i].effective_due());
    completed.sort_by_key(|&i| std::cmp::Reverse(rs[i].finished_on()));

    ui.horizontal(|ui| {
        let labels = [
            (Tab::Upcoming, format!("Upcoming  {}", upcoming.len())),
            (Tab::Snoozed, format!("Snoozed  {}", snoozed.len())),
            (Tab::Completed, format!("Completed  {}", completed.len())),
        ];
        let options: Vec<(Tab, &str)> = labels.iter().map(|(t, l)| (*t, l.as_str())).collect();
        theme::segmented(ui, &mut tab, &options);
        if tab == Tab::Upcoming {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let current = HORIZONS.iter().find(|h| h.0 == horizon).map_or("Next month", |h| h.1);
                egui::ComboBox::from_id_salt("reminders-horizon-combo").selected_text(current).show_ui(ui, |ui| {
                    for (days, label) in HORIZONS {
                        ui.selectable_value(&mut horizon, days, label);
                    }
                });
                ui.label(theme::muted("Showing"));
            });
        }
    });
    ui.data_mut(|d| {
        d.insert_temp(tab_id, tab);
        d.insert_persisted(horizon_id, horizon);
    });
    ui.add_space(8.0);

    let mut event = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match tab {
        Tab::Upcoming => {
            if upcoming.is_empty() {
                let when = HORIZONS.iter().find(|h| h.0 == horizon).map_or("", |h| h.1).to_lowercase();
                ui.label(theme::muted(if horizon == 0 {
                    "Nothing to do. Reminders you add will show up here.".to_string()
                } else {
                    format!("Nothing due in the {when}.")
                }));
            }
            for &i in &upcoming {
                reminder_card(ui, project, &project.reminders[i], Tab::Upcoming, today, &mut event);
            }
            if later > 0 {
                let s = if later == 1 { "" } else { "s" };
                ui.label(theme::muted(format!("{later} more reminder{s} due after that. Choose a longer range to see them.")));
            }
            warranties_ui(ui, project, today, &mut event);
        }
        Tab::Snoozed => {
            if snoozed.is_empty() {
                ui.label(theme::muted("Nothing snoozed. Use Snooze on an upcoming reminder to put it off until later."));
            }
            for &i in &snoozed {
                reminder_card(ui, project, &project.reminders[i], Tab::Snoozed, today, &mut event);
            }
        }
        Tab::Completed => {
            if completed.is_empty() {
                ui.label(theme::muted("Reminders you mark done will show up here."));
            }
            for &i in &completed {
                reminder_card(ui, project, &project.reminders[i], Tab::Completed, today, &mut event);
            }
        }
    });

    match event {
        Some(Event::Reveal(r, sections)) => {
            theme::focus_section(ui.ctx(), sections);
            env.actions.push(Action::Reveal(r));
        }
        Some(Event::Complete(id)) => {
            project.complete_reminder(id, today);
            env.toasts.info("Done. Logged in service history.");
        }
        Some(Event::Snooze(id, until)) => {
            project.snooze_reminder(id, until);
            env.toasts.info(format!("Snoozed until {}.", fmt_date(until)));
        }
        Some(Event::SetSnooze(id, until)) => project.snooze_reminder(id, until),
        Some(Event::Unsnooze(id)) => {
            project.unsnooze_reminder(id);
            env.toasts.info("Back in Upcoming.");
        }
        Some(Event::Reopen(id)) => {
            project.reopen_reminder(id);
            env.toasts.info("Reopened. It's back in Upcoming.");
        }
        None => {}
    }
}

fn reminder_card(ui: &mut Ui, project: &Project, r: &Reminder, tab: Tab, today: NaiveDate, event: &mut Option<Event>) {
    // The whole card opens the reminder where it was made. (Sensed before its
    // contents, so the buttons and link inside still get their own clicks.)
    let card = ui.scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
                let (fg, bg) = match r.status(today) {
                    DueStatus::Overdue(_) => (c::coral_dark(), c::coral_soft()),
                    DueStatus::DueSoon(_) => (c::amber(), c::amber_soft()),
                    DueStatus::Done | DueStatus::Snoozed(_) => (c::muted(), c::field()),
                    DueStatus::Upcoming(_) => (c::forest(), c::mint_soft()),
                };
                let (fg, bg) = if tab == Tab::Completed { (c::muted(), c::field()) } else { (fg, bg) };
                let glyph = match tab {
                    Tab::Upcoming => icon::BELL,
                    Tab::Snoozed => icon::BELL_Z,
                    Tab::Completed => icon::CHECK_CIRCLE,
                };
                ui.painter().rect_filled(rect, 10.0, bg);
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(18.0), fg);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let title = if r.title.trim().is_empty() { "(untitled)" } else { r.title.as_str() };
                    ui.label(theme::strong(title));
                    ui.horizontal(|ui| {
                        if ui
                            .add(egui::Button::new(RichText::new(project.entity_name(r.target)).size(12.5).color(c::forest())).frame(false))
                            .clicked()
                        {
                            *event = Some(Event::Reveal(r.target, REMINDERS));
                        }
                        let repeat = r.repeat.map(|x| x.describe()).unwrap_or_else(|| "Once".into());
                        let when = match tab {
                            Tab::Upcoming => format!("due {}", fmt_date(r.effective_due())),
                            Tab::Snoozed => format!("was due {}", fmt_date(r.due)),
                            Tab::Completed => match (r.is_done(), r.finished_on()) {
                                (true, Some(d)) => format!("done {}", fmt_date(d)),
                                (false, Some(d)) => format!("last done {}  ·  next due {}", fmt_date(d), fmt_date(r.effective_due())),
                                _ => String::new(),
                            },
                        };
                        ui.label(RichText::new(format!("·  {when}  ·  {repeat}")).size(12.5).color(c::muted()));
                    });
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match tab {
                    Tab::Upcoming => {
                        if theme::secondary_button(ui, format!("{}  Done", icon::CHECK)).clicked() {
                            *event = Some(Event::Complete(r.id));
                        }
                        let snooze = theme::secondary_button_widget(format!("{}  Snooze", icon::BELL_Z));
                        egui::menu::menu_custom_button(ui, snooze, |ui| {
                            ui.label(theme::caps("Snooze until"));
                            for (label, until) in snooze_options(today) {
                                if ui.button(format!("{label}  ·  {}", fmt_date(until))).clicked() {
                                    *event = Some(Event::Snooze(r.id, until));
                                    ui.close_menu();
                                }
                            }
                        });
                        status_chip_sized(ui, r.status(today), theme::BUTTON_H);
                    }
                    Tab::Snoozed => {
                        if theme::secondary_button(ui, "Unsnooze").clicked() {
                            *event = Some(Event::Unsnooze(r.id));
                        }
                        if let Some(mut until) = r.snoozed_until {
                            if date_picker(ui, ("snooze-until", r.id), &mut until, today) {
                                *event = Some(Event::SetSnooze(r.id, until.max(today.succ_opt().unwrap_or(today))));
                            }
                        }
                        ui.label(theme::muted("Until"));
                    }
                    Tab::Completed => {
                        if r.is_done() {
                            if theme::secondary_button(ui, "Reopen").on_hover_text("Put it back in Upcoming").clicked() {
                                *event = Some(Event::Reopen(r.id));
                            }
                        } else {
                            // Repeating: done for now, and already scheduled again.
                            theme::chip_sized(ui, "Repeats", c::muted(), c::field(), theme::BUTTON_H);
                        }
                    }
                });
            });
        })
    });
    if card.response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && event.is_none() {
        *event = Some(Event::Reveal(r.target, REMINDERS));
    }
}

/// Snooze presets: (label, date).
fn snooze_options(today: NaiveDate) -> Vec<(&'static str, NaiveDate)> {
    let days = |n: u64| today.checked_add_days(Days::new(n)).unwrap_or(today);
    let months = |n: u32| today.checked_add_months(Months::new(n)).unwrap_or(today);
    vec![
        ("Tomorrow", days(1)),
        ("In 3 days", days(3)),
        ("Next week", days(7)),
        ("In 2 weeks", days(14)),
        ("Next month", months(1)),
        ("In 3 months", months(3)),
    ]
}

fn warranties_ui(ui: &mut Ui, project: &Project, today: NaiveDate, event: &mut Option<Event>) {
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
        let card = ui.scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
            theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon::RECEIPT).size(18.0).color(c::muted()));
                    if ui.add(egui::Button::new(theme::strong(project.entity_name(target))).frame(false)).clicked() {
                        *event = Some(Event::Reveal(target, WARRANTY));
                    }
                    ui.label(theme::muted(format!("until {}", fmt_date(date))));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let days = (date - today).num_days();
                        let h = theme::BUTTON_H;
                        if days < 0 {
                            theme::chip_sized(ui, "Expired", c::muted(), c::field(), h);
                        } else if days <= 90 {
                            theme::chip_sized(ui, format!("{days} days left"), c::amber(), c::amber_soft(), h);
                        } else {
                            theme::chip_sized(ui, format!("{} left", fmt_span(days)), c::forest(), c::mint_soft(), h);
                        }
                    });
                });
            })
        });
        if card.response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && event.is_none() {
            *event = Some(Event::Reveal(target, WARRANTY));
        }
    }
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

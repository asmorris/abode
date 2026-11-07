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
const REMINDERS: &[&str] = &["Reminders", "Appointments & reminders"];
/// Wherever each kind of thing keeps its warranty date.
const WARRANTY: &[&str] = &["Purchase & warranty", "Built & warranty", "Whole roof", "Layers"];
// Sections of a vehicle's panel (see `inspector::vehicle_sections`).
const VEHICLE_WARRANTIES: &[&str] = &["Warranties"];
const VEHICLE_PAPERWORK: &[&str] = &["Registration & insurance"];
const VEHICLE_OWNERSHIP: &[&str] = &["Purchase & ownership"];

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
                        "Add reminders to anything (a room, an appliance, the roof, the deck, a pet) from its panel. Marking one done logs it in that item's service history.",
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
        // Odometer reminders can't be placed in time: show them while they're
        // close (or if distance is all that matters).
        let status = project.reminder_status(r, today);
        let by_km = matches!(status, DueStatus::OverKm(_) | DueStatus::SoonKm(_)) || r.due_by == DueBy::Distance;
        if r.is_snoozed(today) {
            snoozed.push(i);
        } else if by_km || until.is_none_or(|u| r.effective_due() <= u) {
            upcoming.push(i);
        } else {
            later += 1;
        }
    }
    let rs = &project.reminders;
    upcoming.sort_by_key(|&i| (std::cmp::Reverse(project.reminder_status(&rs[i], today).urgency()), rs[i].effective_due()));
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
                let (fg, bg) = match project.reminder_status(r, today).urgency() {
                    3 => (c::coral_dark(), c::coral_soft()),
                    2 => (c::amber(), c::amber_soft()),
                    1 => (c::forest(), c::mint_soft()),
                    _ => (c::muted(), c::field()),
                };
                let (fg, bg) = if tab == Tab::Completed { (c::muted(), c::field()) } else { (fg, bg) };
                let glyph = match tab {
                    Tab::Upcoming if r.is_appointment() => icon::CALENDAR_DOTS,
                    Tab::Upcoming if matches!(r.target, EntityRef::Pet(_)) => icon::PAW_PRINT,
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
                        let units = project.info.units;
                        let km = |k: Option<f64>| k.map(|k| vehicle::format_distance(k, units)).unwrap_or_else(|| "?".into());
                        let repeat = match (r.due_by, r.repeat, r.every_km) {
                            (DueBy::Date, Some(x), _) => x.describe(),
                            (DueBy::Distance, _, Some(k)) => format!("Every {}", km(Some(k))),
                            (DueBy::Either, Some(x), Some(k)) => format!("{} or {}", x.describe(), km(Some(k))),
                            (DueBy::Either, Some(x), None) => x.describe(),
                            (DueBy::Either, None, Some(k)) => format!("Every {}", km(Some(k))),
                            _ => "Once".into(),
                        };
                        let at = match (r.at, r.place.trim()) {
                            (Some(t), "") => format!(" at {}", fmt_time(t)),
                            (Some(t), place) => format!(" at {}, {place}", fmt_time(t)),
                            (None, _) => String::new(),
                        };
                        let due = match r.due_by {
                            DueBy::Date => format!("{}{at}", fmt_date(r.effective_due())),
                            DueBy::Distance => format!("at {}", km(r.due_km)),
                            DueBy::Either => format!("{} or at {}", fmt_date(r.effective_due()), km(r.due_km)),
                        };
                        let when = match tab {
                            Tab::Upcoming => format!("due {due}"),
                            Tab::Snoozed => format!("was due {}", fmt_date(r.due)),
                            Tab::Completed => match (r.is_done(), r.finished_on()) {
                                (true, Some(d)) => format!("done {}", fmt_date(d)),
                                (false, Some(d)) => format!("last done {}  ·  next due {due}", fmt_date(d)),
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
                        status_chip_sized(ui, project.reminder_status(r, today), theme::BUTTON_H, project.info.units);
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

/// Something with an end date (or distance) worth seeing coming.
struct Expiry {
    date: Option<NaiveDate>,
    /// Vehicle warranties: km left before it ends by distance, and the limit.
    km_left: Option<f64>,
    km_limit: Option<f64>,
    target: EntityRef,
    /// e.g. "Powertrain warranty" (or empty for the item's own warranty).
    what: String,
    sections: &'static [&'static str],
}

fn warranties_ui(ui: &mut Ui, project: &Project, today: NaiveDate, event: &mut Option<Event>) {
    ui.add_space(16.0);
    ui.label(RichText::new("Warranties & renewals").family(theme::serif()).size(22.0).color(c::ink()));
    let units = project.info.units;
    let mut list: Vec<Expiry> = Vec::new();
    let mut add = |date: Option<NaiveDate>, target: EntityRef| {
        if date.is_some() {
            list.push(Expiry { date, km_left: None, km_limit: None, target, what: String::new(), sections: WARRANTY });
        }
    };
    add(project.roof.warranty_expires, EntityRef::Roof);
    for l in &project.levels {
        for o in l.objects.values() {
            add(o.details.warranty_expires, EntityRef::Object(o.id));
        }
        for o in l.openings.values() {
            add(o.details.warranty_expires, EntityRef::Opening(o.id));
        }
        for r in l.roofs.values() {
            add(r.top_layer().and_then(|x| x.warranty_expires), EntityRef::RoofSection(r.id));
        }
        for f in l.features.values() {
            add(f.details.warranty_expires, EntityRef::Feature(f.id));
        }
    }
    for b in &project.buildings {
        add(b.details.warranty_expires, EntityRef::Building(b.id));
    }
    // Vehicles: each warranty (by date and/or distance) and the paperwork
    // that needs renewing.
    for (_, id) in project.vehicles() {
        let Some(v) = project.vehicle(id) else { continue };
        let target = EntityRef::Object(id);
        let odo = project.odometer(id);
        for w in &v.warranties {
            let km_left = match (w.until_km, odo) {
                (Some(limit), Some(now)) => Some(limit - now),
                _ => None,
            };
            if w.expires.is_some() || km_left.is_some() {
                let name = if w.name.trim().is_empty() { "Warranty".to_string() } else { w.name.clone() };
                let what = if name.to_lowercase().contains("warranty") { name } else { format!("{name} warranty") };
                list.push(Expiry { date: w.expires, km_left, km_limit: w.until_km, target, what, sections: VEHICLE_WARRANTIES });
            }
        }
        let term = match v.ownership {
            vehicle::Ownership::Leased => "Lease ends",
            vehicle::Ownership::Financed => "Loan paid off",
            vehicle::Ownership::Owned => "",
        };
        for (date, what, sections) in [
            (v.registration_expires, "Registration renewal", VEHICLE_PAPERWORK),
            (v.insurance_renews, "Insurance renewal", VEHICLE_PAPERWORK),
            (v.inspection_due, "Inspection due", VEHICLE_PAPERWORK),
            (if term.is_empty() { None } else { v.term_ends }, term, VEHICLE_OWNERSHIP),
        ] {
            if date.is_some() {
                list.push(Expiry { date, km_left: None, km_limit: None, target, what: what.to_string(), sections });
            }
        }
    }
    // Soonest first; distance-only ones after the dated ones.
    list.sort_by_key(|e| (e.date.is_none(), e.date, e.km_left.map(|k| k as i64)));
    if list.is_empty() {
        ui.label(theme::muted("Add warranty dates in an item's details (or a vehicle's warranties and renewals) to see them here."));
    }
    for e in list {
        let card = ui.scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
            theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let glyph = if e.sections == WARRANTY || e.sections == VEHICLE_WARRANTIES { icon::RECEIPT } else { icon::IDENTIFICATION_CARD };
                    ui.label(RichText::new(glyph).size(18.0).color(c::muted()));
                    if ui.add(egui::Button::new(theme::strong(project.entity_name(e.target))).frame(false)).clicked() {
                        *event = Some(Event::Reveal(e.target, e.sections));
                    }
                    let mut when: Vec<String> = Vec::new();
                    if !e.what.is_empty() {
                        when.push(e.what.clone());
                    }
                    let limit = e.km_limit.map(|k| vehicle::format_distance(k, units));
                    let until = match (e.date, limit) {
                        (Some(d), Some(k)) => format!("until {} or {k}", fmt_date(d)),
                        (Some(d), None) => format!("{} {}", if e.what.is_empty() || e.what.contains("arranty") { "until" } else { "on" }, fmt_date(d)),
                        (None, Some(k)) => format!("until {k}"),
                        (None, None) => String::new(),
                    };
                    when.push(until);
                    ui.label(theme::muted(when.join("  ·  ")));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let h = theme::BUTTON_H;
                        let days = e.date.map(|d| (d - today).num_days());
                        if days.is_some_and(|d| d < 0) || e.km_left.is_some_and(|k| k <= 0.0) {
                            let done = if e.sections == WARRANTY || e.sections == VEHICLE_WARRANTIES { "Expired" } else { "Overdue" };
                            let (fg, bg) = if done == "Overdue" { (c::coral_dark(), c::coral_soft()) } else { (c::muted(), c::field()) };
                            theme::chip_sized(ui, done, fg, bg, h);
                            return;
                        }
                        if let Some(k) = e.km_left {
                            let soon = k < 2000.0;
                            let (fg, bg) = if soon { (c::amber(), c::amber_soft()) } else { (c::forest(), c::mint_soft()) };
                            theme::chip_sized(ui, format!("{} left", vehicle::format_distance(k, units)), fg, bg, h);
                        }
                        if let Some(days) = days {
                            if days <= 90 {
                                theme::chip_sized(ui, format!("{days} days left"), c::amber(), c::amber_soft(), h);
                            } else {
                                theme::chip_sized(ui, format!("{} left", fmt_span(days)), c::forest(), c::mint_soft(), h);
                            }
                        }
                    });
                });
            })
        });
        if card.response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && event.is_none() {
            *event = Some(Event::Reveal(e.target, e.sections));
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

//! Notes, attachments, service history and reminders: shown for every kind
//! of entity in the inspector.

use super::widgets::*;
use super::{Action, AttachTarget, Env};
use crate::theme::{self, c, icon};
use abode_core::reminders::DueStatus;
use abode_core::*;
use chrono::Utc;
use eframe::egui::{self, Align2, FontId, RichText, Sense, Ui, vec2};

/// All three sections for an entity.
pub fn history_sections(ui: &mut Ui, project: &mut Project, env: &mut Env, target: EntityRef) {
    notes_section(ui, project, env, target);
    service_section(ui, project, env, target);
    reminders_section(ui, project, env, target);
}

pub fn notes_section(ui: &mut Ui, project: &mut Project, env: &mut Env, target: EntityRef) {
    let ids: Vec<NoteId> = project.notes_for(target).map(|n| n.id).collect();
    theme::section(ui, ("notes", target), icon::NOTE, "Notes", Some(ids.len()), true, |ui| {
        let mut delete = None;
        for id in ids {
            let Some(note) = project.notes.iter_mut().find(|n| n.id == id) else {
                continue;
            };
            theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let title = ui.add(
                        egui::TextEdit::singleline(&mut note.title)
                            .hint_text("Untitled note")
                            .font(FontId::new(15.0, theme::semibold()))
                            .frame(false)
                            .desired_width(ui.available_width() - 70.0),
                    );
                    privacy_toggle(ui, &mut note.private);
                    if theme::icon_button_sized(ui, icon::TRASH, "Delete note", false, 28.0).clicked() {
                        delete = Some(id);
                    }
                    if title.changed() {
                        note.updated = Utc::now();
                    }
                });
                let body = ui.add(
                    egui::TextEdit::multiline(&mut note.body)
                        .hint_text("Write something…")
                        .frame(false)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                if body.changed() {
                    note.updated = Utc::now();
                }
                attachments_ui(ui, env, &mut note.attachments, AttachTarget::Note(id));
                ui.label(RichText::new(format!("Edited {}", fmt_datetime(note.updated))).size(11.5).color(c::faint()));
            });
            ui.add_space(2.0);
        }
        if let Some(id) = delete {
            project.notes.retain(|n| n.id != id);
        }
        if theme::soft_button(ui, format!("{}  Add note", icon::PLUS)).clicked() {
            project.notes.push(Note::new(target));
        }
    });
}

/// Thumbnails / file tiles plus an "Attach files" button.
pub fn attachments_ui(ui: &mut Ui, env: &mut Env, list: &mut Vec<AttachmentId>, target: AttachTarget) {
    let mut remove = None;
    if !list.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            for (i, id) in list.iter().enumerate() {
                let meta = env.store.attachment_meta(*id).cloned();
                let tex = env.textures.get(ui.ctx(), env.store, *id, 256);
                let size = vec2(96.0, 96.0);
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                let p = ui.painter();
                match &tex {
                    Some(tex) => {
                        // Cover-fit the image into a rounded square.
                        let ts = tex.size_vec2();
                        let scale = (size.x / ts.x).max(size.y / ts.y);
                        let shown = ts * scale;
                        let uv_w = (size.x / shown.x).min(1.0);
                        let uv_h = (size.y / shown.y).min(1.0);
                        let uv = egui::Rect::from_min_max(
                            egui::pos2((1.0 - uv_w) / 2.0, (1.0 - uv_h) / 2.0),
                            egui::pos2((1.0 + uv_w) / 2.0, (1.0 + uv_h) / 2.0),
                        );
                        egui::Image::new((tex.id(), size))
                            .uv(uv)
                            .corner_radius(10)
                            .paint_at(ui, rect);
                    }
                    None => {
                        p.rect(rect, 10.0, c::panel(), egui::Stroke::new(1.0_f32, c::border()), egui::StrokeKind::Inside);
                        p.text(rect.center() - vec2(0.0, 14.0), Align2::CENTER_CENTER, icon::FILE, FontId::proportional(26.0), c::muted());
                        let ext = meta
                            .as_ref()
                            .and_then(|m| std::path::Path::new(&m.filename).extension().map(|e| e.to_string_lossy().to_uppercase()))
                            .unwrap_or_else(|| "FILE".into());
                        let badge = egui::Rect::from_center_size(rect.center() + vec2(0.0, 12.0), vec2(40.0, 16.0));
                        p.rect_filled(badge, 4.0, c::forest());
                        p.text(badge.center(), Align2::CENTER_CENTER, ext, FontId::new(10.0, theme::semibold()), egui::Color32::WHITE);
                        if let Some(m) = &meta {
                            p.text(
                                rect.center_bottom() - vec2(0.0, 12.0),
                                Align2::CENTER_CENTER,
                                fmt_size(m.size),
                                FontId::proportional(10.5),
                                c::faint(),
                            );
                        }
                    }
                }
                if resp.hovered() {
                    p.rect_stroke(rect, 10.0, egui::Stroke::new(2.0_f32, c::teal()), egui::StrokeKind::Inside);
                }
                let name = meta.as_ref().map(|m| m.filename.clone()).unwrap_or_else(|| "(missing file)".into());
                let resp = resp.on_hover_text(format!("{name}\nClick to open · right-click for more"));
                if resp.clicked() {
                    env.actions.push(Action::OpenAttachment(*id));
                }
                resp.context_menu(|ui| {
                    if ui.button(format!("{}  Open", icon::ARROW_SQUARE_OUT)).clicked() {
                        env.actions.push(Action::OpenAttachment(*id));
                        ui.close_menu();
                    }
                    if ui.button(format!("{}  Save a copy…", icon::DOWNLOAD_SIMPLE)).clicked() {
                        env.actions.push(Action::SaveAttachmentAs(*id));
                        ui.close_menu();
                    }
                    if ui.button(format!("{}  Remove", icon::TRASH)).clicked() {
                        remove = Some(i);
                        ui.close_menu();
                    }
                });
            }
        });
    }
    if let Some(i) = remove {
        list.remove(i);
    }
    if ui
        .add(
            egui::Button::new(RichText::new(format!("{}  Attach photos or files", icon::PAPERCLIP)).size(13.0).color(c::muted()))
                .frame(false),
        )
        .on_hover_text("Photos, manuals, receipts, any file")
        .clicked()
    {
        env.actions.push(Action::AttachFiles(target));
    }
}

pub fn service_section(ui: &mut Ui, project: &mut Project, env: &mut Env, target: EntityRef) {
    let mut ids: Vec<(chrono::NaiveDate, ServiceId)> = project.service_for(target).map(|s| (s.date, s.id)).collect();
    ids.sort_by_key(|a| std::cmp::Reverse(a.0));
    let count = ids.len();
    let today = env.today;
    let units = project.info.units;
    let car = matches!(target, EntityRef::Object(o) if project.vehicle(o).is_some());
    theme::section(ui, ("service", target), icon::WRENCH, "Service history", Some(count), count > 0, |ui| {
        let mut delete = None;
        for (_, id) in ids {
            let Some(s) = project.service_records.iter_mut().find(|s| s.id == id) else {
                continue;
            };
            theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                let summary = s.description.lines().next().unwrap_or("").to_string();
                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), ui.make_persistent_id(("svc", id)), summary.is_empty())
                    .show_header(ui, |ui| {
                        ui.label(theme::strong(fmt_date(s.date)));
                        theme::chip(ui, s.kind_label(), c::forest(), c::mint_soft());
                        if let Some(k) = s.odometer {
                            ui.label(theme::muted(vehicle::format_distance(k, units)));
                        }
                        if !summary.is_empty() {
                            ui.add(egui::Label::new(theme::muted(summary)).truncate());
                        }
                    })
                    .body_unindented(|ui| {
                        form_grid(ui, ("svc-grid", id), |ui| {
                            field_label(ui, "Date");
                            date_edit(ui, &format!("svc-date-{id}"), &mut s.date, today);
                            ui.end_row();
                            field_label(ui, "Type");
                            combo_named(ui, ("svc-kind", id), &mut s.kind, &mut s.kind_name, &ServiceKind::ALL, ServiceKind::label, ServiceKind::Other);
                            ui.end_row();
                            if car {
                                field_label(ui, "Odometer");
                                distance_edit(ui, ("svc-km", id), &mut s.odometer, units, "Reading", 140.0);
                                ui.end_row();
                            }
                            text_row(ui, "Done by", &mut s.performed_by, "Company or person");
                            text_row(ui, "Cost", &mut s.cost, "");
                        });
                        ui.add(
                            egui::TextEdit::multiline(&mut s.description)
                                .hint_text("What was done?")
                                .desired_rows(2)
                                .desired_width(f32::INFINITY),
                        );
                        attachments_ui(ui, env, &mut s.attachments, AttachTarget::Service(id));
                        ui.horizontal(|ui| {
                            privacy_toggle(ui, &mut s.private);
                            ui.label(theme::muted(if s.private { "Private" } else { "Shared with new owners" }));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if danger_button(ui, "Delete") {
                                    delete = Some(id);
                                }
                            });
                        });
                    });
            });
        }
        if let Some(id) = delete {
            project.service_records.retain(|s| s.id != id);
        }
        if theme::soft_button(ui, format!("{}  Add service record", icon::PLUS)).clicked() {
            project.service_records.push(ServiceRecord::new(target, env.today));
        }
    });
}

/// Status chip for a reminder.
pub fn status_chip(ui: &mut Ui, status: DueStatus, units: UnitSystem) {
    status_chip_sized(ui, status, 24.0, units);
}

/// [`status_chip`] at a set height (e.g. [`theme::BUTTON_H`] beside buttons).
pub fn status_chip_sized(ui: &mut Ui, status: DueStatus, height: f32, units: UnitSystem) {
    let km = |k: i64| vehicle::format_distance(k as f64, units);
    let (text, fg, bg) = match status {
        DueStatus::Done => ("Done".to_string(), c::muted(), c::field()),
        DueStatus::Overdue(d) => (format!("Overdue {d}d"), c::coral_dark(), c::coral_soft()),
        DueStatus::DueSoon(0) => ("Due today".to_string(), c::amber(), c::amber_soft()),
        DueStatus::DueSoon(d) => (format!("In {d} days"), c::amber(), c::amber_soft()),
        DueStatus::Upcoming(d) => (format!("In {d} days"), c::forest(), c::mint_soft()),
        DueStatus::Snoozed(d) => (format!("Snoozed · {d}d"), c::muted(), c::field()),
        DueStatus::OverKm(k) => (format!("Overdue {}", km(k)), c::coral_dark(), c::coral_soft()),
        DueStatus::SoonKm(k) => (format!("In {}", km(k)), c::amber(), c::amber_soft()),
        DueStatus::LaterKm(0) => ("Needs odometer".to_string(), c::muted(), c::field()),
        DueStatus::LaterKm(k) => (format!("In {}", km(k)), c::forest(), c::mint_soft()),
    };
    theme::chip_sized(ui, text, fg, bg, height);
}

pub fn reminders_section(ui: &mut Ui, project: &mut Project, env: &mut Env, target: EntityRef) {
    let ids: Vec<ReminderId> = project.reminders_for(target).map(|r| r.id).collect();
    let count = ids.len();
    let units = project.info.units;
    // Vehicles can have reminders by odometer too.
    let car = match target {
        EntityRef::Object(o) if project.vehicle(o).is_some() => Some(project.odometer(o)),
        _ => None,
    };
    theme::section(ui, ("reminders", target), icon::BELL, "Reminders", Some(count), count > 0, |ui| {
        let mut delete = None;
        let mut complete = None;
        for id in ids {
            let Some(r) = project.reminders.iter_mut().find(|r| r.id == id) else {
                continue;
            };
            reminder_editor(ui, r, env, car, units, &mut delete, &mut complete);
        }
        if let Some(id) = delete {
            project.reminders.retain(|r| r.id != id);
        }
        if let Some(id) = complete {
            project.complete_reminder(id, env.today);
            env.toasts.info("Done. Logged in service history.");
        }
        ui.horizontal_wrapped(|ui| {
            if theme::soft_button(ui, format!("{}  Add reminder", icon::PLUS)).clicked() {
                project.reminders.push(Reminder::new(target, "", env.today));
            }
            let garden = match target {
                EntityRef::Planting(_) => true,
                EntityRef::Feature(f) => project.is_garden(f),
                _ => false,
            };
            if garden {
                menu_dropdown(ui, format!("{}  Garden tasks", icon::PLANT), |ui| {
                    ui.set_min_width(300.0);
                    let care = abode_core::garden::CARE;
                    let seasonal = abode_core::garden::SEASONAL_CARE;
                    let mut choices: Vec<Choice> = care
                        .iter()
                        .map(|t| {
                            let unit = match (t.unit, t.every) {
                                (RecurrenceUnit::Days, 1) => "day".to_string(),
                                (RecurrenceUnit::Weeks, 1) => "week".to_string(),
                                (RecurrenceUnit::Days, n) => format!("{n} days"),
                                (RecurrenceUnit::Weeks, n) => format!("{n} weeks"),
                                (RecurrenceUnit::Months, n) => format!("{n} months"),
                                (RecurrenceUnit::Years, n) => format!("{n} years"),
                            };
                            Choice::new(format!("{}  ·  every {unit}", t.title)).group("Repeating, starting today")
                        })
                        .collect();
                    choices.extend(seasonal.iter().map(|(title, m, d)| {
                        let due = abode_core::garden::next_on(env.today, *m, *d);
                        let have = project.reminders.iter().any(|r| r.target == target && r.title == *title);
                        Choice::new(format!("{title}  ·  {}", due.format("%b %-d"))).group("Once a year").enabled(!have)
                    }));
                    let id = ui.make_persistent_id(("garden-tasks", target));
                    match search_list(ui, id, &choices, None, Some("Add")) {
                        Some(Picked::Item(i)) if i < care.len() => {
                            let t = &care[i];
                            let mut r = Reminder::new(target, t.title, env.today);
                            r.repeat = Some(Recurrence { every: t.every, unit: t.unit });
                            project.reminders.push(r);
                            close_dropdown(ui);
                        }
                        Some(Picked::Item(i)) => {
                            let (title, m, d) = seasonal[i - care.len()];
                            let mut r = Reminder::new(target, title, abode_core::garden::next_on(env.today, m, d));
                            r.repeat = Some(Recurrence { every: 1, unit: RecurrenceUnit::Years });
                            project.reminders.push(r);
                            close_dropdown(ui);
                        }
                        Some(Picked::Custom(title)) => {
                            project.reminders.push(Reminder::new(target, title, env.today));
                            close_dropdown(ui);
                        }
                        None => {}
                    }
                });
            }
            if let Some(odo) = car {
                menu_dropdown(ui, format!("{}  Common car reminders", icon::CAR), |ui| {
                    ui.set_min_width(340.0);
                    let schedules = vehicle::COMMON_SCHEDULES;
                    let mut choices: Vec<Choice> = schedules
                        .iter()
                        .map(|t| {
                            let every = vehicle::format_distance(t.every_km, units);
                            let months = if t.months % 12 == 0 { format!("{} yr", t.months / 12) } else { format!("{} mo", t.months) };
                            Choice::new(format!("{}  ·  {every} / {months}", t.title)).group("Due by date or odometer, whichever first")
                        })
                        .collect();
                    let have_swaps = matches!(target, EntityRef::Object(o) if project.has_tire_swap_reminders(o));
                    choices.push(Choice::new("Seasonal tire swaps  ·  winter Nov 1, summer Apr 15").group("Yearly, by date").enabled(!have_swaps));
                    let id = ui.make_persistent_id(("car-reminders", target));
                    match search_list(ui, id, &choices, None, Some("Add")) {
                        Some(Picked::Item(i)) if i < schedules.len() => {
                            let t = &schedules[i];
                            let mut r = Reminder::new(target, t.title, env.today.checked_add_months(chrono::Months::new(t.months)).unwrap_or(env.today));
                            r.due_by = DueBy::Either;
                            r.repeat = Some(Recurrence { every: t.months, unit: RecurrenceUnit::Months });
                            r.every_km = Some(t.every_km);
                            r.due_km = Some(odo.unwrap_or(0.0) + t.every_km);
                            project.reminders.push(r);
                            close_dropdown(ui);
                        }
                        Some(Picked::Item(_)) => {
                            if let EntityRef::Object(o) = target {
                                project.add_tire_swap_reminders(o, env.today);
                            }
                            close_dropdown(ui);
                        }
                        Some(Picked::Custom(title)) => {
                            project.reminders.push(Reminder::new(target, title, env.today));
                            close_dropdown(ui);
                        }
                        None => {}
                    }
                });
            }
        });
    });
}

/// `car`: for a vehicle's reminders, its odometer reading (if known), so it
/// can be due by distance as well as date.
#[allow(clippy::too_many_arguments)]
pub fn reminder_editor(
    ui: &mut Ui,
    r: &mut Reminder,
    env: &mut Env,
    car: Option<Option<f64>>,
    units: UnitSystem,
    delete: &mut Option<ReminderId>,
    complete: &mut Option<ReminderId>,
) {
    let id = r.id;
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut r.title)
                    .hint_text("e.g. Replace furnace filter")
                    .font(FontId::new(14.5, theme::semibold()))
                    .frame(false)
                    .desired_width(ui.available_width() - 110.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                status_chip(ui, r.status_at(env.today, car.flatten()), units);
            });
        });
        if car.is_some() {
            ui.horizontal(|ui| {
                ui.label(theme::muted("Due"));
                combo(ui, ("rem-by", id), &mut r.due_by, &DueBy::ALL, DueBy::label);
            });
        } else {
            r.due_by = DueBy::Date;
        }
        if r.due_by.uses_date() {
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::muted(if car.is_some() { "On" } else { "Due" }));
                date_edit(ui, &format!("rem-due-{id}"), &mut r.due, env.today);
                let mut repeats = r.repeat.is_some();
                if theme::toggle(ui, &mut repeats, "Repeats").changed() {
                    r.repeat = repeats.then_some(Recurrence { every: 1, unit: RecurrenceUnit::Years });
                }
                if let Some(rep) = &mut r.repeat {
                    ui.label(theme::muted("every"));
                    ui.add(egui::DragValue::new(&mut rep.every).range(1..=120));
                    combo(ui, ("rem-unit", id), &mut rep.unit, &RecurrenceUnit::ALL, RecurrenceUnit::label);
                }
            });
        }
        if let (Some(odo), true) = (car, r.due_by.uses_distance()) {
            ui.horizontal_wrapped(|ui| {
                ui.label(theme::muted("At"));
                distance_edit(ui, ("rem-km", id), &mut r.due_km, units, "Odometer", 110.0);
                let mut again = r.every_km.is_some();
                if theme::toggle(ui, &mut again, "Repeats").changed() {
                    r.every_km = again.then_some(8000.0);
                }
                if r.every_km.is_some() {
                    ui.label(theme::muted("every"));
                    distance_edit(ui, ("rem-every-km", id), &mut r.every_km, units, "Distance", 100.0);
                }
            });
            match (odo, r.km_left(odo)) {
                (None, _) => {
                    ui.label(theme::muted("Enter the odometer reading in Vehicle so this knows how far there is to go."));
                }
                (Some(now), _) if r.due_km.is_none() => {
                    ui.label(theme::muted(format!("Odometer reads {}.", vehicle::format_distance(now, units))));
                }
                _ => {}
            }
        }
        ui.add(
            egui::TextEdit::multiline(&mut r.notes)
                .hint_text("Notes")
                .desired_rows(1)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if !r.is_done()
                && theme::primary_button(ui, format!("{}  Mark done", icon::CHECK))
                    .on_hover_text("Adds a service record and schedules the next one")
                    .clicked()
            {
                *complete = Some(id);
            }
            if let Some(d) = r.last_done {
                ui.label(theme::muted(format!("Last done {}", fmt_date(d))));
            }
            if r.is_snoozed(env.today) {
                if let Some(s) = r.snoozed_until {
                    ui.label(theme::muted(format!("Snoozed until {}", fmt_date(s))));
                    if theme::soft_button(ui, "Unsnooze").clicked() {
                        r.snoozed_until = None;
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::icon_button_sized(ui, icon::TRASH, "Delete reminder", false, 28.0).clicked() {
                    *delete = Some(id);
                }
            });
        });
    });
}

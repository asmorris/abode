//! Notes, attachments, service history and reminders: shown for every kind
//! of entity in the inspector.

use super::widgets::*;
use super::{Action, AttachTarget, Env};
use abode_core::reminders::DueStatus;
use abode_core::*;
use chrono::Utc;
use eframe::egui::{self, RichText, Ui};

/// All three sections for an entity.
pub fn history_sections(ui: &mut Ui, project: &mut Project, env: &mut Env, target: EntityRef) {
    notes_section(ui, project, env, target);
    service_section(ui, project, env, target);
    reminders_section(ui, project, env, target);
}

pub fn notes_section(ui: &mut Ui, project: &mut Project, env: &mut Env, target: EntityRef) {
    let ids: Vec<NoteId> = project.notes_for(target).map(|n| n.id).collect();
    section(ui, &format!("📝 Notes ({})", ids.len()), ("notes", target), true, |ui| {
        if ui.button("+ Add note").clicked() {
            project.notes.push(Note::new(target));
        }
        let mut delete = None;
        for id in ids {
            let Some(note) = project.notes.iter_mut().find(|n| n.id == id) else {
                continue;
            };
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let title = ui.add(
                        egui::TextEdit::singleline(&mut note.title)
                            .hint_text("Title")
                            .desired_width(ui.available_width() - 70.0),
                    );
                    let icon = if note.private { "🔒" } else { "🔓" };
                    ui.toggle_value(&mut note.private, icon)
                        .on_hover_text("Private notes are removed when you export a copy for a new owner");
                    if ui.small_button("🗑").on_hover_text("Delete note").clicked() {
                        delete = Some(id);
                    }
                    if title.changed() {
                        note.updated = Utc::now();
                    }
                });
                let body = ui.add(
                    egui::TextEdit::multiline(&mut note.body)
                        .hint_text("Write a note…")
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                if body.changed() {
                    note.updated = Utc::now();
                }
                attachments_ui(ui, env, &mut note.attachments, AttachTarget::Note(id));
                ui.weak(format!("Updated {}", fmt_datetime(note.updated)));
            });
        }
        if let Some(id) = delete {
            project.notes.retain(|n| n.id != id);
        }
    });
}

/// Thumbnails / file chips plus an "Attach files" button.
pub fn attachments_ui(ui: &mut Ui, env: &mut Env, list: &mut Vec<AttachmentId>, target: AttachTarget) {
    let mut remove = None;
    if !list.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (i, id) in list.iter().enumerate() {
                let meta = env.store.attachment_meta(*id).cloned();
                ui.vertical(|ui| {
                    ui.set_width(112.0);
                    let tex = env.textures.get(ui.ctx(), env.store, *id, 256);
                    let clicked = match &tex {
                        Some(tex) => {
                            let size = tex.size_vec2();
                            let scale = (104.0 / size.x.max(size.y)).min(1.0);
                            ui.add(egui::Image::new((tex.id(), size * scale)).sense(egui::Sense::click()))
                                .on_hover_text("Open")
                                .clicked()
                        }
                        None => ui
                            .add_sized([104.0, 56.0], egui::Button::new(RichText::new("📄").size(28.0)))
                            .on_hover_text("Open")
                            .clicked(),
                    };
                    if clicked {
                        env.actions.push(Action::OpenAttachment(*id));
                    }
                    match &meta {
                        Some(m) => {
                            ui.add(egui::Label::new(RichText::new(&m.filename).small()).truncate())
                                .on_hover_text(format!("{} · {}", m.filename, fmt_size(m.size)));
                        }
                        None => {
                            ui.small("(missing file)");
                        }
                    }
                    ui.horizontal(|ui| {
                        if ui.small_button("💾").on_hover_text("Save a copy…").clicked() {
                            env.actions.push(Action::SaveAttachmentAs(*id));
                        }
                        if ui.small_button("🗙").on_hover_text("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
        });
    }
    if let Some(i) = remove {
        list.remove(i);
    }
    if ui
        .small_button("📎 Attach files…")
        .on_hover_text("Photos, manuals, receipts, any file")
        .clicked()
    {
        env.actions.push(Action::AttachFiles(target));
    }
}

pub fn service_section(ui: &mut Ui, project: &mut Project, env: &mut Env, target: EntityRef) {
    let mut ids: Vec<(chrono::NaiveDate, ServiceId)> = project.service_for(target).map(|s| (s.date, s.id)).collect();
    ids.sort_by(|a, b| b.0.cmp(&a.0));
    section(ui, &format!("🔧 Service history ({})", ids.len()), ("service", target), !ids.is_empty(), |ui| {
        if ui.button("+ Add service record").clicked() {
            project.service_records.push(ServiceRecord::new(target, env.today));
        }
        let mut delete = None;
        for (_, id) in ids {
            let Some(s) = project.service_records.iter_mut().find(|s| s.id == id) else {
                continue;
            };
            let title = format!(
                "{} · {}{}",
                fmt_date(s.date),
                s.kind.label(),
                if s.description.trim().is_empty() {
                    String::new()
                } else {
                    format!(" · {}", s.description.lines().next().unwrap_or(""))
                }
            );
            egui::CollapsingHeader::new(title).id_salt(("svc", id)).show(ui, |ui| {
                egui::Grid::new(("svc-grid", id)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                    ui.label("Date");
                    date_edit(ui, &format!("svc-date-{id}"), &mut s.date);
                    ui.end_row();
                    ui.label("Type");
                    combo(ui, ("svc-kind", id), &mut s.kind, &ServiceKind::ALL, ServiceKind::label);
                    ui.end_row();
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
                    ui.checkbox(&mut s.private, "🔒 Private");
                    if danger_button(ui, "Delete record") {
                        delete = Some(id);
                    }
                });
            });
        }
        if let Some(id) = delete {
            project.service_records.retain(|s| s.id != id);
        }
    });
}

pub fn status_text(ui: &Ui, status: DueStatus) -> RichText {
    match status {
        DueStatus::Done => RichText::new("✔ Done").weak(),
        DueStatus::Overdue(d) => RichText::new(format!("Overdue {d}d")).color(ui.visuals().error_fg_color),
        DueStatus::DueSoon(0) => RichText::new("Due today").color(ui.visuals().warn_fg_color),
        DueStatus::DueSoon(d) => RichText::new(format!("In {d}d")).color(ui.visuals().warn_fg_color),
        DueStatus::Upcoming(d) => RichText::new(format!("In {d}d")),
    }
}

pub fn reminders_section(ui: &mut Ui, project: &mut Project, env: &mut Env, target: EntityRef) {
    let ids: Vec<ReminderId> = project.reminders_for(target).map(|r| r.id).collect();
    section(ui, &format!("⏰ Reminders ({})", ids.len()), ("reminders", target), !ids.is_empty(), |ui| {
        if ui.button("+ Add reminder").clicked() {
            project.reminders.push(Reminder::new(target, "", env.today));
        }
        let mut delete = None;
        let mut complete = None;
        for id in ids {
            let Some(r) = project.reminders.iter_mut().find(|r| r.id == id) else {
                continue;
            };
            reminder_editor(ui, r, env, &mut delete, &mut complete);
        }
        if let Some(id) = delete {
            project.reminders.retain(|r| r.id != id);
        }
        if let Some(id) = complete {
            project.complete_reminder(id, env.today);
            env.toasts.info("Done. Logged in service history.");
        }
    });
}

pub fn reminder_editor(
    ui: &mut Ui,
    r: &mut Reminder,
    env: &mut Env,
    delete: &mut Option<ReminderId>,
    complete: &mut Option<ReminderId>,
) {
    let id = r.id;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut r.title)
                    .hint_text("e.g. Replace furnace filter")
                    .desired_width(ui.available_width() - 90.0),
            );
            ui.label(status_text(ui, r.status(env.today)));
        });
        ui.horizontal(|ui| {
            ui.label("Due");
            date_edit(ui, &format!("rem-due-{id}"), &mut r.due);
            let mut repeats = r.repeat.is_some();
            if ui.checkbox(&mut repeats, "Repeats").changed() {
                r.repeat = repeats.then_some(Recurrence { every: 1, unit: RecurrenceUnit::Years });
            }
            if let Some(rep) = &mut r.repeat {
                ui.label("every");
                ui.add(egui::DragValue::new(&mut rep.every).range(1..=120));
                combo(ui, ("rem-unit", id), &mut rep.unit, &RecurrenceUnit::ALL, RecurrenceUnit::label);
            }
        });
        ui.add(
            egui::TextEdit::multiline(&mut r.notes)
                .hint_text("Notes")
                .desired_rows(1)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if !r.is_done() && ui.button("✔ Mark done").on_hover_text("Adds a service record and schedules the next one").clicked() {
                *complete = Some(id);
            }
            if let Some(d) = r.last_done {
                ui.weak(format!("Last done {}", fmt_date(d)));
            }
            if ui.small_button("🗑").on_hover_text("Delete reminder").clicked() {
                *delete = Some(id);
            }
        });
    });
}

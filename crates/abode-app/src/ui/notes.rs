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
                ui.label(RichText::new(format!("Edited {}", fmt_datetime(note.updated))).size(11.5).color(c::FAINT));
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
                        p.rect(rect, 10.0, c::PANEL, egui::Stroke::new(1.0, c::BORDER), egui::StrokeKind::Inside);
                        p.text(rect.center() - vec2(0.0, 14.0), Align2::CENTER_CENTER, icon::FILE, FontId::proportional(26.0), c::MUTED);
                        let ext = meta
                            .as_ref()
                            .and_then(|m| std::path::Path::new(&m.filename).extension().map(|e| e.to_string_lossy().to_uppercase()))
                            .unwrap_or_else(|| "FILE".into());
                        let badge = egui::Rect::from_center_size(rect.center() + vec2(0.0, 12.0), vec2(40.0, 16.0));
                        p.rect_filled(badge, 4.0, c::FOREST);
                        p.text(badge.center(), Align2::CENTER_CENTER, ext, FontId::new(10.0, theme::semibold()), egui::Color32::WHITE);
                        if let Some(m) = &meta {
                            p.text(
                                rect.center_bottom() - vec2(0.0, 12.0),
                                Align2::CENTER_CENTER,
                                fmt_size(m.size),
                                FontId::proportional(10.5),
                                c::FAINT,
                            );
                        }
                    }
                }
                if resp.hovered() {
                    p.rect_stroke(rect, 10.0, egui::Stroke::new(2.0, c::TEAL), egui::StrokeKind::Inside);
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
            egui::Button::new(RichText::new(format!("{}  Attach photos or files", icon::PAPERCLIP)).size(13.0).color(c::MUTED))
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
    ids.sort_by(|a, b| b.0.cmp(&a.0));
    let count = ids.len();
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
                        theme::chip(ui, s.kind.label(), c::FOREST, c::MINT_SOFT);
                        if !summary.is_empty() {
                            ui.add(egui::Label::new(theme::muted(summary)).truncate());
                        }
                    })
                    .body_unindented(|ui| {
                        form_grid(ui, ("svc-grid", id), |ui| {
                            field_label(ui, "Date");
                            date_edit(ui, &format!("svc-date-{id}"), &mut s.date);
                            ui.end_row();
                            field_label(ui, "Type");
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
pub fn status_chip(ui: &mut Ui, status: DueStatus) {
    let (text, fg, bg) = match status {
        DueStatus::Done => ("Done".to_string(), c::MUTED, c::FIELD),
        DueStatus::Overdue(d) => (format!("Overdue {d}d"), c::CORAL_DARK, c::CORAL_SOFT),
        DueStatus::DueSoon(0) => ("Due today".to_string(), c::AMBER, c::AMBER_SOFT),
        DueStatus::DueSoon(d) => (format!("In {d} days"), c::AMBER, c::AMBER_SOFT),
        DueStatus::Upcoming(d) => (format!("In {d} days"), c::FOREST, c::MINT_SOFT),
    };
    theme::chip(ui, text, fg, bg);
}

pub fn reminders_section(ui: &mut Ui, project: &mut Project, env: &mut Env, target: EntityRef) {
    let ids: Vec<ReminderId> = project.reminders_for(target).map(|r| r.id).collect();
    let count = ids.len();
    theme::section(ui, ("reminders", target), icon::BELL, "Reminders", Some(count), count > 0, |ui| {
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
        if theme::soft_button(ui, format!("{}  Add reminder", icon::PLUS)).clicked() {
            project.reminders.push(Reminder::new(target, "", env.today));
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
                status_chip(ui, r.status(env.today));
            });
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(theme::muted("Due"));
            date_edit(ui, &format!("rem-due-{id}"), &mut r.due);
            let mut repeats = r.repeat.is_some();
            if ui.checkbox(&mut repeats, "Repeats").changed() {
                r.repeat = repeats.then_some(Recurrence { every: 1, unit: RecurrenceUnit::Years });
            }
            if let Some(rep) = &mut r.repeat {
                ui.label(theme::muted("every"));
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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::icon_button_sized(ui, icon::TRASH, "Delete reminder", false, 28.0).clicked() {
                    *delete = Some(id);
                }
            });
        });
    });
}

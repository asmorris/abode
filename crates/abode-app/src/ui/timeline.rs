//! The timeline: automatic + written change-log entries, and saved versions.

use super::widgets::*;
use super::{Action, Env};
use abode_core::storage::RevisionInfo;
use abode_core::*;
use chrono::NaiveDate;
use eframe::egui::{self, RichText, Ui};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Selection,
    Written,
}

struct EntryDraft {
    occurred: NaiveDate,
    summary: String,
    details: String,
    about_selection: bool,
    private: bool,
}

pub struct TimelineState {
    filter: Filter,
    search: String,
    draft: Option<EntryDraft>,
    /// Cached list of saved versions (reset after each save).
    pub revisions: Option<Vec<RevisionInfo>>,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self { filter: Filter::All, search: String::new(), draft: None, revisions: None }
    }
}

pub fn timeline_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, st: &mut TimelineState, author: &str) {
    egui::SidePanel::right("revisions")
        .resizable(true)
        .default_width(270.0)
        .show_inside(ui, |ui| revisions_ui(ui, env, st));

    let sel = *env.selection;
    ui.horizontal_wrapped(|ui| {
        ui.heading("Timeline");
        ui.separator();
        ui.selectable_value(&mut st.filter, Filter::All, "Everything");
        match sel {
            Some(s) => {
                ui.selectable_value(&mut st.filter, Filter::Selection, format!("Only {}", project.entity_name(s)));
            }
            None => {
                if st.filter == Filter::Selection {
                    st.filter = Filter::All;
                }
            }
        }
        ui.selectable_value(&mut st.filter, Filter::Written, "Written entries");
        ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("🔍 Search").desired_width(160.0));
    });
    ui.weak("Changes are recorded automatically. Write your own entries for things like renovations, repairs or purchases. Past dates are fine.");
    ui.add_space(4.0);

    match &mut st.draft {
        None => {
            if ui.button("✏ Write an entry…").clicked() {
                st.draft = Some(EntryDraft {
                    occurred: env.today,
                    summary: String::new(),
                    details: String::new(),
                    about_selection: sel.is_some(),
                    private: false,
                });
            }
        }
        Some(d) => {
            let mut done = None;
            egui::Frame::group(ui.style()).show(ui, |ui| {
                egui::Grid::new("entry-draft").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                    ui.label("When");
                    date_edit(ui, "entry-date", &mut d.occurred);
                    ui.end_row();
                    ui.label("What happened");
                    ui.add(
                        egui::TextEdit::singleline(&mut d.summary)
                            .hint_text("e.g. Replaced the water heater")
                            .desired_width(f32::INFINITY),
                    );
                    ui.end_row();
                    ui.label("Details");
                    ui.add(egui::TextEdit::multiline(&mut d.details).desired_rows(2).desired_width(f32::INFINITY));
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if let Some(s) = sel {
                        ui.checkbox(&mut d.about_selection, format!("About {}", project.entity_name(s)));
                    }
                    ui.checkbox(&mut d.private, "🔒 Private");
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(!d.summary.trim().is_empty(), egui::Button::new("Add to timeline")).clicked() {
                        done = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        done = Some(false);
                    }
                });
            });
            match done {
                Some(true) => {
                    let mut e = ChangeEntry::manual(author, d.summary.trim(), Some(d.occurred));
                    e.details = d.details.trim().to_string();
                    e.private = d.private;
                    if d.about_selection {
                        e.targets.extend(sel);
                    }
                    project.changelog.push(e);
                    st.draft = None;
                }
                Some(false) => st.draft = None,
                None => {}
            }
        }
    }
    ui.separator();

    let search = st.search.to_lowercase();
    let mut idx: Vec<usize> = (0..project.changelog.len())
        .filter(|&i| {
            let e = &project.changelog[i];
            let filter_ok = match st.filter {
                Filter::All => true,
                Filter::Written => e.kind == ChangeKind::Manual,
                Filter::Selection => sel.is_some_and(|s| e.targets.contains(&s)),
            };
            filter_ok
                && (search.is_empty()
                    || e.summary.to_lowercase().contains(&search)
                    || e.details.to_lowercase().contains(&search))
        })
        .collect();
    idx.sort_by(|&a, &b| {
        let (ea, eb) = (&project.changelog[a], &project.changelog[b]);
        (eb.effective_date(), eb.timestamp).cmp(&(ea.effective_date(), ea.timestamp))
    });

    let mut delete = None;
    let mut reveal = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        if idx.is_empty() {
            ui.weak("Nothing here yet.");
        }
        let mut last_date = None;
        for i in idx {
            let e = &project.changelog[i];
            let date = e.effective_date();
            if last_date != Some(date) {
                ui.add_space(6.0);
                ui.label(RichText::new(fmt_date(date)).strong());
                last_date = Some(date);
            }
            ui.horizontal_wrapped(|ui| {
                let icon = match e.kind {
                    ChangeKind::Added => "➕",
                    ChangeKind::Removed => "➖",
                    ChangeKind::Modified => "✏",
                    ChangeKind::Manual => "📝",
                };
                ui.label(icon);
                let text = RichText::new(&e.summary);
                ui.label(if e.kind == ChangeKind::Manual { text.strong() } else { text });
                if e.private {
                    ui.label("🔒").on_hover_text("Private: not included when you export a copy");
                }
                let time = e.timestamp.with_timezone(&chrono::Local).format("%H:%M");
                let who = if e.author.is_empty() { String::new() } else { format!(" · {}", e.author) };
                if e.kind == ChangeKind::Manual {
                    ui.weak(format!("written {}{who}", fmt_date(e.timestamp.date_naive())));
                    if ui.small_button("🗑").on_hover_text("Delete entry").clicked() {
                        delete = Some(e.id);
                    }
                } else {
                    ui.weak(format!("{time}{who}"));
                }
                for t in &e.targets {
                    if project.exists(*t)
                        && !matches!(t, EntityRef::General)
                        && e.kind == ChangeKind::Manual
                        && ui.small_button(project.entity_name(*t)).clicked()
                    {
                        reveal = Some(*t);
                    }
                }
            });
            if !e.details.is_empty() {
                ui.indent(e.id, |ui| ui.label(&e.details));
            }
        }
    });
    if let Some(id) = delete {
        project.changelog.retain(|e| e.id != id);
    }
    if let Some(r) = reveal {
        env.actions.push(Action::Reveal(r));
    }
}

fn revisions_ui(ui: &mut Ui, env: &mut Env, st: &mut TimelineState) {
    ui.heading("Saved versions");
    ui.weak("Each save keeps a snapshot of the whole house. You can go back to one if you need to.");
    if st.revisions.is_none() {
        st.revisions = Some(env.store.revisions().unwrap_or_default());
    }
    ui.add_space(4.0);
    egui::ScrollArea::vertical().id_salt("revisions-scroll").show(ui, |ui| {
        let revs = st.revisions.as_deref().unwrap_or(&[]);
        if revs.is_empty() {
            ui.weak("Nothing saved yet.");
        }
        for r in revs {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.strong(fmt_datetime(r.created));
                ui.label(&r.message);
                ui.horizontal(|ui| {
                    if !r.author.is_empty() {
                        ui.weak(&r.author);
                    }
                    if ui.small_button("Restore…").clicked() {
                        env.dialogs.restore = Some((r.id, fmt_datetime(r.created)));
                    }
                });
            });
        }
    });
}

//! The timeline sheet: automatic + written change-log entries, and saved versions.

use super::widgets::*;
use super::{Action, Env};
use crate::theme::{self, c, icon};
use abode_core::storage::RevisionInfo;
use abode_core::*;
use chrono::NaiveDate;
use eframe::egui::{self, Align2, FontId, RichText, Sense, Stroke, Ui, vec2};

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
    let rev_w = 270.0;
    let main_w = (ui.available_width() - rev_w - 28.0).max(300.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(main_w);
            entries_ui(ui, project, env, st, author);
        });
        ui.add_space(10.0);
        let (r, _) = ui.allocate_exact_size(vec2(1.0, ui.available_height()), Sense::hover());
        ui.painter().rect_filled(r, 0.0, c::border());
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.set_width(rev_w);
            revisions_ui(ui, env, st);
        });
    });
}

fn entries_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, st: &mut TimelineState, author: &str) {
    let sel = *env.selection;
    if sel.is_none() && st.filter == Filter::Selection {
        st.filter = Filter::All;
    }
    let sel_label = sel.map(|s| format!("Only {}", project.entity_name(s)));
    let mut options = vec![(Filter::All, "Everything"), (Filter::Written, "Written")];
    if let Some(l) = &sel_label {
        options.insert(1, (Filter::Selection, l.as_str()));
    }
    let add_label = format!("{}  Add entry", icon::NOTE_PENCIL);
    let text_w = |ui: &Ui, t: &str, font: FontId| ui.fonts(|f| f.layout_no_wrap(t.to_string(), font, egui::Color32::WHITE).size().x);
    // Measure so nothing overlaps: tabs, search and the button each keep their
    // own space, wrapping onto a second row when the sheet is narrow.
    let seg_w: f32 = options
        .iter()
        .map(|(_, l)| text_w(ui, l, FontId::new(13.0, theme::medium())) + 2.0 * 12.0 + 2.0)
        .sum::<f32>()
        + 8.0;
    let button_w = text_w(ui, &add_label, FontId::new(14.0, theme::medium())) + 2.0 * 12.0;
    let (gap, search_w) = (16.0, 170.0);
    let avail = ui.available_width();
    let one_row = seg_w + gap + search_w + gap + button_w <= avail;
    let button_on_first = one_row || seg_w + gap + button_w <= avail;

    let mut add_clicked = false;
    let mut add_button = |ui: &mut Ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if st.draft.is_none() && theme::primary_button(ui, add_label.clone()).clicked() {
                add_clicked = true;
            }
        });
    };
    let search = |ui: &mut Ui, search: &mut String, width: f32| {
        ui.add(
            egui::TextEdit::singleline(search)
                .hint_text(format!("{}  Search", icon::MAGNIFYING_GLASS))
                .desired_width(width)
                .margin(vec2(10.0, 6.0)),
        );
    };
    let mut filter = st.filter;
    let mut query = std::mem::take(&mut st.search);
    ui.horizontal(|ui| {
        theme::segmented(ui, &mut filter, &options);
        if one_row {
            ui.add_space(gap - ui.spacing().item_spacing.x);
            search(ui, &mut query, search_w);
        }
        if button_on_first {
            ui.add_space(gap);
            add_button(ui);
        }
    });
    if !one_row {
        ui.horizontal(|ui| {
            if button_on_first {
                search(ui, &mut query, ui.available_width());
            } else {
                // Button first (from the right) so the search box gets exactly what's left.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if st.draft.is_none() && theme::primary_button(ui, add_label.clone()).clicked() {
                        add_clicked = true;
                    }
                    ui.add_space(gap);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        search(ui, &mut query, ui.available_width());
                    });
                });
            }
        });
    }
    st.filter = filter;
    st.search = query;
    if add_clicked {
        st.draft = Some(EntryDraft {
            occurred: env.today,
            summary: String::new(),
            details: String::new(),
            about_selection: sel.is_some(),
            private: false,
        });
    }
    ui.label(theme::muted(
        "Changes are recorded automatically. Write your own entries for renovations, repairs or purchases; past dates are fine.",
    ));
    ui.add_space(6.0);

    if let Some(d) = &mut st.draft {
        let mut done = None;
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            form_grid(ui, "entry-draft", |ui| {
                field_label(ui, "When");
                date_edit(ui, "entry-date", &mut d.occurred, env.today);
                ui.end_row();
                field_label(ui, "What happened");
                ui.add(
                    egui::TextEdit::singleline(&mut d.summary)
                        .hint_text("e.g. Replaced the water heater")
                        .desired_width(f32::INFINITY),
                );
                ui.end_row();
                field_label(ui, "Details");
                ui.add(egui::TextEdit::multiline(&mut d.details).desired_rows(2).desired_width(f32::INFINITY));
                ui.end_row();
            });
            ui.horizontal(|ui| {
                if let Some(s) = sel {
                    theme::toggle(ui, &mut d.about_selection, &format!("About {}", project.entity_name(s)));
                }
                theme::toggle(ui, &mut d.private, "Private");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ok = !d.summary.trim().is_empty();
                    if ui.add_enabled_ui(ok, |ui| theme::primary_button(ui, "Add to timeline")).inner.clicked() {
                        done = Some(true);
                    }
                    if theme::secondary_button(ui, "Cancel").clicked() {
                        done = Some(false);
                    }
                });
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
        ui.add_space(6.0);
    }

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
    egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt("timeline-scroll").show(ui, |ui| {
        if idx.is_empty() {
            ui.add_space(20.0);
            ui.label(theme::muted("Nothing here yet. Changes you make will show up here."));
        }
        let mut last_date = None;
        let count = idx.len();
        for (n, i) in idx.into_iter().enumerate() {
            let e = &project.changelog[i];
            let date = e.effective_date();
            if last_date != Some(date) {
                ui.add_space(10.0);
                ui.label(RichText::new(fmt_date(date)).family(theme::serif()).size(20.0).color(c::ink()));
                last_date = Some(date);
            }
            let line = ui.painter().add(egui::Shape::Noop);
            let row = ui.horizontal(|ui| {
                let (dot, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
                let (i, fg, bg) = match e.kind {
                    ChangeKind::Added => (icon::PLUS, c::forest(), c::mint_soft()),
                    ChangeKind::Removed => (icon::MINUS, c::coral_dark(), c::coral_soft()),
                    ChangeKind::Modified => (icon::PENCIL_SIMPLE, c::muted(), c::field()),
                    ChangeKind::Manual => (icon::NOTE_PENCIL, egui::Color32::WHITE, c::forest()),
                };
                ui.painter().circle_filled(dot.center(), 13.0, bg);
                ui.painter().text(dot.center(), Align2::CENTER_CENTER, i, FontId::proportional(13.0), fg);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.horizontal_wrapped(|ui| {
                        let text = if e.kind == ChangeKind::Manual { theme::strong(&e.summary) } else { RichText::new(&e.summary) };
                        ui.label(text);
                        if e.private {
                            ui.label(RichText::new(icon::LOCK).color(c::muted()))
                                .on_hover_text("Private: not included when you export a copy");
                        }
                    });
                    let who = if e.author.is_empty() { String::new() } else { format!(" · {}", e.author) };
                    ui.horizontal(|ui| {
                        let when = if e.kind == ChangeKind::Manual {
                            format!("Written {}{who}", fmt_date(e.timestamp.date_naive()))
                        } else {
                            format!("{}{who}", e.timestamp.with_timezone(&chrono::Local).format("%H:%M"))
                        };
                        ui.label(RichText::new(when).size(12.0).color(c::faint()));
                        if e.kind == ChangeKind::Manual {
                            for t in &e.targets {
                                if project.exists(*t)
                                    && !matches!(t, EntityRef::General)
                                    && ui.add(egui::Button::new(RichText::new(project.entity_name(*t)).size(12.0).color(c::forest())).frame(false)).clicked()
                                {
                                    reveal = Some(*t);
                                }
                            }
                            if theme::icon_button_sized(ui, icon::TRASH, "Delete entry", false, 22.0).clicked() {
                                delete = Some(e.id);
                            }
                        }
                    });
                    if !e.details.is_empty() {
                        ui.label(theme::muted(&e.details));
                    }
                });
                dot
            });
            let dot = row.inner;
            if n + 1 < count {
                ui.painter().set(
                    line,
                    egui::Shape::line_segment(
                        [dot.center_bottom() + vec2(0.0, 1.0), egui::pos2(dot.center().x, row.response.rect.bottom() + 10.0)],
                        Stroke::new(1.5, c::border()),
                    ),
                );
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
    ui.label(theme::strong("Saved versions"));
    ui.label(theme::muted("Each save keeps a snapshot of the whole house. Go back to one if you need to."));
    if st.revisions.is_none() {
        st.revisions = Some(env.store.revisions().unwrap_or_default());
    }
    ui.add_space(4.0);
    egui::ScrollArea::vertical().id_salt("revisions-scroll").auto_shrink([false, false]).show(ui, |ui| {
        let revs = st.revisions.as_deref().unwrap_or(&[]);
        if revs.is_empty() {
            ui.label(theme::muted("Nothing saved yet."));
        }
        for r in revs {
            theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(theme::strong(fmt_datetime(r.created)));
                ui.label(theme::muted(&r.message));
                ui.horizontal(|ui| {
                    if !r.author.is_empty() {
                        ui.label(RichText::new(&r.author).size(12.0).color(c::faint()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::soft_button(ui, format!("{}  Restore", icon::ARROW_COUNTER_CLOCKWISE)).clicked() {
                            env.dialogs.restore = Some((r.id, fmt_datetime(r.created)));
                        }
                    });
                });
            });
        }
    });
}

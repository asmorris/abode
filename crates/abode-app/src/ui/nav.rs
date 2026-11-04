//! Left floating panel: house header + file menu, quick links, and the
//! levels → rooms → items/doors/windows tree.

use super::{Action, Env, MainTab};
use crate::theme::{self, c, icon};
use abode_core::*;
use eframe::egui::{self, Align2, Color32, FontId, Rect, Response, Sense, Ui, text::LayoutJob, vec2};
use std::path::PathBuf;

pub struct FileInfo<'a> {
    pub name: String,
    pub dirty: bool,
    pub saved: bool,
    pub recent: &'a [PathBuf],
    pub can_undo: bool,
    pub can_redo: bool,
}

/// One row in the tree. Returns (row response, caret response if any).
fn nav_row(
    ui: &mut Ui,
    depth: usize,
    i: &str,
    text: &str,
    selected: bool,
    caret: Option<bool>,
    badge: Option<(String, Color32, Color32)>,
) -> (Response, Option<Response>) {
    let h = 32.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 10.0, c::MINT);
    } else if resp.hovered() {
        p.rect_filled(rect, 10.0, c::HOVER);
    }
    let mut x = rect.left() + 6.0 + depth as f32 * 16.0;
    let mut caret_resp = None;
    if let Some(open) = caret {
        let r = Rect::from_center_size(egui::pos2(x + 8.0, rect.center().y), vec2(18.0, 18.0));
        let cr = ui.interact(r, resp.id.with("caret"), Sense::click());
        let col = if cr.hovered() { c::INK } else { c::FAINT };
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            if open { icon::CARET_DOWN } else { icon::CARET_RIGHT },
            FontId::proportional(12.0),
            col,
        );
        caret_resp = Some(cr);
    }
    x += 18.0;
    let p = ui.painter();
    let fg = if selected { c::FOREST } else { c::INK };
    p.text(
        egui::pos2(x + 8.0, rect.center().y),
        Align2::CENTER_CENTER,
        i,
        FontId::proportional(16.0),
        if selected { c::FOREST } else { c::MUTED },
    );
    x += 24.0;
    let badge_w = badge.as_ref().map(|(t, _, _)| 14.0 + t.len() as f32 * 7.0).unwrap_or(0.0);
    let max_w = (rect.right() - x - 8.0 - badge_w).max(10.0);
    let font = FontId::new(14.0, if selected { theme::semibold() } else { egui::FontFamily::Proportional });
    let mut job = LayoutJob::simple_singleline(text.to_string(), font, fg);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w);
    let galley = ui.fonts(|f| f.layout_job(job));
    let pos = egui::pos2(x, rect.center().y - galley.size().y / 2.0);
    p.galley(pos, galley, fg);
    if let Some((t, fg, bg)) = badge {
        let br = Rect::from_center_size(egui::pos2(rect.right() - 8.0 - badge_w / 2.0, rect.center().y), vec2(badge_w, 20.0));
        p.rect_filled(br, 10.0, bg);
        p.text(br.center(), Align2::CENTER_CENTER, t, FontId::new(11.5, theme::semibold()), fg);
    }
    (resp, caret_resp)
}

fn open_state(ui: &Ui, id: egui::Id, default: bool) -> bool {
    ui.data_mut(|d| *d.get_persisted_mut_or(id, default))
}

fn toggle_state(ui: &Ui, id: egui::Id, default: bool) {
    ui.data_mut(|d| {
        let v = d.get_persisted_mut_or(id, default);
        *v = !*v;
    });
}

fn file_menu(ui: &mut Ui, env: &mut Env, file: &FileInfo) {
    let item = |ui: &mut Ui, i: &str, text: &str, shortcut: &str| -> bool {
        ui.add(egui::Button::new(theme::with_icon(i, text)).shortcut_text(shortcut).min_size(vec2(230.0, 28.0)))
            .clicked()
    };
    if item(ui, icon::FILE_PLUS, "New house", "⌘N") {
        env.actions.push(Action::New);
        ui.close_menu();
    }
    if item(ui, icon::SPARKLE, "New sample house", "") {
        env.actions.push(Action::NewSample);
        ui.close_menu();
    }
    if item(ui, icon::FOLDER_OPEN, "Open…", "⌘O") {
        env.actions.push(Action::Open);
        ui.close_menu();
    }
    ui.menu_button(theme::with_icon(icon::CLOCK_COUNTER_CLOCKWISE, "Open recent"), |ui| {
        if file.recent.is_empty() {
            ui.label(theme::muted("Nothing yet"));
        }
        for p in file.recent {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if ui.button(name).on_hover_text(p.display().to_string()).clicked() {
                env.actions.push(Action::OpenPath(p.clone()));
                ui.close_menu();
            }
        }
    });
    ui.separator();
    if item(ui, icon::FLOPPY_DISK, "Save", "⌘S") {
        env.actions.push(Action::Save);
        ui.close_menu();
    }
    if item(ui, icon::FLOPPY_DISK, "Save as…", "⇧⌘S") {
        env.actions.push(Action::SaveAs);
        ui.close_menu();
    }
    if item(ui, icon::NOTE_PENCIL, "Save with a note…", "") {
        env.dialogs.save_message = Some(String::new());
        ui.close_menu();
    }
    ui.separator();
    if item(ui, icon::EXPORT, "Export copy for a new owner…", "") {
        env.actions.push(Action::ExportShared);
        ui.close_menu();
    }
    ui.separator();
    if item(ui, icon::GEAR_SIX, "Settings…", "") {
        env.dialogs.settings = true;
        ui.close_menu();
    }
    if item(ui, icon::SIGN_OUT, "Quit", "⌘Q") {
        env.actions.push(Action::Quit);
        ui.close_menu();
    }
}

pub fn nav_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, tab: &mut MainTab, canvas: &mut MainTab, file: &FileInfo) {
    let sel = *env.selection;
    let mut clicked: Option<EntityRef> = None;

    // Header: logo, house name, file menu
    ui.horizontal(|ui| {
        theme::icon_tile(ui, icon::HOUSE_LINE, c::CORAL, c::CORAL_SOFT);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let name = project.entity_name(EntityRef::General);
            let mut job = LayoutJob::simple_singleline(name, FontId::new(22.0, theme::serif()), c::INK);
            job.wrap = egui::text::TextWrapping::truncate_at_width(150.0);
            let r = ui.add(egui::Label::new(job).sense(Sense::click()));
            if r.on_hover_text("House details & general notes").clicked() {
                clicked = Some(EntityRef::General);
            }
            let (dot, status) = if file.dirty {
                (c::CORAL, "Unsaved changes".to_string())
            } else if file.saved {
                (c::TEAL, format!("Saved · {}", file.name))
            } else {
                (c::FAINT, "Not saved yet".to_string())
            };
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 3.5, dot);
                ui.label(egui::RichText::new(status).size(12.0).color(c::MUTED));
            });
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button(egui::RichText::new(icon::DOTS_THREE).size(18.0), |ui| file_menu(ui, env, file))
                .response
                .on_hover_text("File");
        });
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let before = *canvas;
        theme::segmented(ui, canvas, &[(MainTab::Plan, "Plan"), (MainTab::View3d, "3D")]);
        if *canvas != before {
            *tab = *canvas;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if theme::icon_button(ui, icon::FLOPPY_DISK, "Save (⌘S)", false).clicked() {
                env.actions.push(Action::Save);
            }
            if ui.add_enabled_ui(file.can_redo, |ui| theme::icon_button(ui, icon::ARROW_U_UP_RIGHT, "Redo (⇧⌘Z)", false)).inner.clicked() {
                env.actions.push(Action::Redo);
            }
            if ui.add_enabled_ui(file.can_undo, |ui| theme::icon_button(ui, icon::ARROW_U_UP_LEFT, "Undo (⌘Z)", false)).inner.clicked() {
                env.actions.push(Action::Undo);
            }
        });
    });
    ui.add_space(4.0);
    theme::divider(ui);

    let overdue = project.overdue_count(env.today);
    let rows: [(&str, &str, Option<EntityRef>, Option<MainTab>); 4] = [
        (icon::HOUSE_SIMPLE, "House & notes", Some(EntityRef::General), None),
        (icon::TRIANGLE, "Roof", Some(EntityRef::Roof), None),
        (icon::BELL, "Reminders", None, Some(MainTab::Reminders)),
        (icon::CLOCK_COUNTER_CLOCKWISE, "Timeline", None, Some(MainTab::Timeline)),
    ];
    for (i, label, target, t) in rows {
        let selected = target.is_some_and(|x| sel == Some(x)) || t.is_some_and(|x| *tab == x);
        let badge = (t == Some(MainTab::Reminders) && overdue > 0).then(|| (overdue.to_string(), Color32::WHITE, c::CORAL));
        let (r, _) = nav_row(ui, 0, i, label, selected, None, badge);
        if r.clicked() {
            if let Some(x) = target {
                clicked = Some(x);
            }
            if let Some(x) = t {
                *tab = if *tab == x { *canvas } else { x };
            }
        }
    }

    theme::divider(ui);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(theme::caps("Levels"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::icon_button_sized(ui, icon::PLUS, "Add a level", false, 26.0).clicked() {
                let n = project.levels.len() + 1;
                let id = project.add_level(format!("Level {n}"));
                *env.active_level = id;
                clicked = Some(EntityRef::Level(id));
            }
        });
    });

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for level in &project.levels {
            let lid = level.id;
            let open_id = ui.make_persistent_id(("nav-level", lid));
            let open = open_state(ui, open_id, true);
            let active = *env.active_level == lid;
            let label = if active && project.levels.len() > 1 {
                format!("{}  ·  viewing", level.name)
            } else {
                level.name.clone()
            };
            let (r, caret) = nav_row(ui, 0, icon::STACK, &label, sel == Some(EntityRef::Level(lid)), Some(open), None);
            if caret.is_some_and(|c| c.clicked()) {
                toggle_state(ui, open_id, true);
            } else if r.clicked() {
                clicked = Some(EntityRef::Level(lid));
            }
            if !open {
                continue;
            }
            let mut rooms: Vec<&Room> = level.rooms.values().collect();
            rooms.sort_by_key(|r| r.display_name().to_lowercase());
            if rooms.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(48.0);
                    ui.label(egui::RichText::new("No rooms yet").size(12.5).color(c::FAINT));
                });
            }
            for room in rooms {
                let walls = level.walls_of_room(room);
                let objects: Vec<&PlacedObject> = level.objects.values().filter(|o| o.room == room.id).collect();
                let openings: Vec<&Opening> = level.openings.values().filter(|o| walls.contains(&o.wall)).collect();
                let has_children = !objects.is_empty() || !openings.is_empty();
                let rid = ui.make_persistent_id(("nav-room", room.id));
                let ropen = open_state(ui, rid, false);
                let (r, caret) = nav_row(
                    ui,
                    1,
                    theme::room_icon(&room.kind, &room.name),
                    &room.display_name(),
                    sel == Some(EntityRef::Room(room.id)),
                    has_children.then_some(ropen),
                    None,
                );
                if caret.is_some_and(|c| c.clicked()) {
                    toggle_state(ui, rid, false);
                } else if r.clicked() {
                    clicked = Some(EntityRef::Room(room.id));
                }
                if !(ropen && has_children) {
                    continue;
                }
                for o in objects {
                    let e = EntityRef::Object(o.id);
                    if nav_row(ui, 2, icon::PACKAGE, &o.display_name(), sel == Some(e), None, None).0.clicked() {
                        clicked = Some(e);
                    }
                }
                for o in openings {
                    let e = EntityRef::Opening(o.id);
                    let i = if o.kind.is_door() { icon::DOOR } else { icon::APP_WINDOW };
                    let name = if o.label.trim().is_empty() { o.kind.title() } else { o.label.as_str() };
                    if nav_row(ui, 2, i, name, sel == Some(e), None, None).0.clicked() {
                        clicked = Some(e);
                    }
                }
            }
        }
    });

    if let Some(c) = clicked {
        *env.selection = Some(c);
        if let Some(li) = project.level_index_of(c) {
            *env.active_level = project.levels[li].id;
            env.actions.push(Action::Reveal(c));
        }
        if matches!(*tab, MainTab::Timeline | MainTab::Reminders) && project.level_index_of(c).is_some() {
            *tab = *canvas;
        }
    }
}

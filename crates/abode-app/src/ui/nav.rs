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
    /// A newer release, if one was found.
    pub update: Option<crate::updater::Release>,
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
        p.rect_filled(rect, 10.0, c::mint());
    } else if resp.hovered() {
        p.rect_filled(rect, 10.0, c::hover());
    }
    let mut x = rect.left() + 6.0 + depth as f32 * 16.0;
    let mut caret_resp = None;
    if let Some(open) = caret {
        let r = Rect::from_center_size(egui::pos2(x + 8.0, rect.center().y), vec2(18.0, 18.0));
        let cr = ui.interact(r, resp.id.with("caret"), Sense::click());
        let col = if cr.hovered() { c::ink() } else { c::faint() };
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
    let fg = if selected { c::forest() } else { c::ink() };
    p.text(
        egui::pos2(x + 8.0, rect.center().y),
        Align2::CENTER_CENTER,
        i,
        FontId::proportional(16.0),
        if selected { c::forest() } else { c::muted() },
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
    if item(ui, icon::HOUSE_LINE, "Home", "") {
        env.actions.push(Action::Home);
        ui.close_menu();
    }
    ui.separator();
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
    if item(ui, icon::ARROW_CIRCLE_UP, "Check for updates…", "") {
        env.actions.push(Action::CheckUpdates);
        ui.close_menu();
    }
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
    // While a sheet (timeline, reminders) is open, only its row is highlighted.
    let sheet_open = matches!(*tab, MainTab::Timeline | MainTab::Reminders);
    let sel = if sheet_open { None } else { *env.selection };
    let mut clicked: Option<EntityRef> = None;

    // Header: logo, house name, file menu. The menu sits in the card's
    // top-right corner, centred on the logo; the name and save status share
    // the space between and truncate rather than pushing the card wider.
    const TILE: f32 = 38.0;
    const MENU: f32 = 32.0;
    ui.horizontal_top(|ui| {
        theme::icon_tile(ui, icon::HOUSE_LINE, c::coral(), c::coral_soft());
        let text_width = (ui.available_width() - MENU - ui.spacing().item_spacing.x).max(40.0);
        ui.vertical(|ui| {
            ui.set_width(text_width);
            ui.spacing_mut().item_spacing.y = 0.0;
            let name = project.entity_name(EntityRef::General);
            let mut job = LayoutJob::simple_singleline(name, FontId::new(22.0, theme::serif()), c::ink());
            job.wrap = egui::text::TextWrapping::truncate_at_width(text_width);
            let r = ui.add(egui::Label::new(job).sense(Sense::click()));
            if r.on_hover_text("House details & general notes").clicked() {
                clicked = Some(EntityRef::General);
            }
            let (dot, status) = if file.dirty {
                (c::coral(), "Unsaved changes".to_string())
            } else if file.saved {
                (c::teal(), format!("Saved · {}", file.name))
            } else {
                (c::faint(), "Not saved yet".to_string())
            };
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 3.5, dot);
                ui.add(egui::Label::new(egui::RichText::new(status).size(12.0).color(c::muted())).truncate());
            });
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.vertical(|ui| {
                ui.add_space((TILE - MENU) / 2.0);
                ui.spacing_mut().button_padding = vec2(0.0, 0.0);
                // An empty button with the icon painted at its exact centre
                // (a text button lays its label out off-centre).
                let button = egui::Button::new("").min_size(vec2(MENU, MENU));
                let r = egui::menu::menu_custom_button(ui, button, |ui| file_menu(ui, env, file)).response;
                let fg = ui.style().interact(&r).fg_stroke.color;
                ui.painter().text(r.rect.center(), egui::Align2::CENTER_CENTER, icon::DOTS_THREE, FontId::proportional(18.0), fg);
                r.on_hover_text("File");
            });
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
    if let Some(r) = &file.update {
        theme::card().fill(c::mint_soft()).stroke(egui::Stroke::new(1.0_f32, c::mint())).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon::ARROW_CIRCLE_UP).size(18.0).color(c::forest()));
                ui.label(theme::strong(format!("Abode {} is out", r.version)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::primary_button(ui, "Update").clicked() {
                        env.actions.push(Action::InstallUpdate(r.clone()));
                    }
                });
            });
        });
        ui.add_space(4.0);
    }
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
        let badge = (t == Some(MainTab::Reminders) && overdue > 0).then(|| (overdue.to_string(), Color32::WHITE, c::coral()));
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
    let mut add_level = false;
    let mut add_basement = false;
    let mut moved: Option<(LevelId, bool)> = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        levels_header(ui, &mut add_level, &mut add_basement);
        // Top floor first, like the building itself.
        let house: Vec<&Level> = project.floor_stack(None).into_iter().rev().collect();
        let many = house.len() > 1;
        for level in house {
            let label = if *env.active_level == level.id && many {
                format!("{}  ·  viewing", level.name)
            } else {
                level.name.clone()
            };
            level_tree(ui, project, level, &label, 0, sel, &mut clicked, &mut moved);
        }

        ui.add_space(6.0);
        let mut no_add = false;
        section_header(ui, "Outside", None, &mut no_add);
        if let Some(site) = project.site() {
            site_tree(ui, project, site, sel, *env.active_level == site.id, &mut clicked);
        }
        for b in &project.buildings {
            let open_id = ui.make_persistent_id(("nav-building", b.id));
            let open = open_state(ui, open_id, false);
            let levels: Vec<&Level> = project.floor_stack(Some(b.id)).into_iter().rev().collect();
            let (r, caret) = nav_row(ui, 0, icon::WAREHOUSE, &b.display_name(), sel == Some(EntityRef::Building(b.id)), Some(open), None);
            if caret.is_some_and(|c| c.clicked()) {
                toggle_state(ui, open_id, false);
            } else if r.clicked() {
                clicked = Some(EntityRef::Building(b.id));
            }
            if open {
                for level in levels {
                    level_tree(ui, project, level, &level.name, 1, sel, &mut clicked, &mut moved);
                }
            }
        }
        if project.buildings.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(30.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("Add a shed or garage with {} on the Outside plan", icon::WAREHOUSE))
                            .size(12.0)
                            .color(c::faint()),
                    )
                    .wrap(),
                );
            });
        }
    });
    if add_level || add_basement {
        let name = if add_level {
            format!("Level {}", project.house_levels().count() + 1)
        } else if project.house_levels().any(|l| l.name.eq_ignore_ascii_case("Basement")) {
            "Lower basement".to_string()
        } else {
            "Basement".to_string()
        };
        let id = if add_level { project.add_level(None, name) } else { project.add_basement(None, name) };
        *env.active_level = id;
        clicked = Some(EntityRef::Level(id));
    }
    if let Some((id, up)) = moved {
        project.move_level(id, up);
    }

    if let Some(c) = clicked {
        *env.selection = Some(c);
        if let EntityRef::Building(b) = c {
            if let Some(l) = project.ground_level(Some(b)) {
                *env.active_level = l.id;
            }
        }
        if let Some(li) = project.level_index_of(c) {
            *env.active_level = project.levels[li].id;
            env.actions.push(Action::Reveal(c));
        }
        if sheet_open {
            *tab = *canvas;
        }
    }
}

/// "Levels" with a + that offers a new top floor or a basement.
fn levels_header(ui: &mut Ui, add_level: &mut bool, add_basement: &mut bool) {
    ui.horizontal(|ui| {
        ui.set_min_height(26.0);
        ui.add_space(6.0);
        ui.label(theme::caps("Levels"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let popup = ui.make_persistent_id("nav-add-level");
            let open = ui.memory(|m| m.is_popup_open(popup));
            let r = theme::icon_button_sized(ui, icon::PLUS, if open { "" } else { "Add a level" }, open, 26.0);
            if r.clicked() {
                ui.memory_mut(|m| m.toggle_popup(popup));
            }
            egui::popup::popup_below_widget(ui, popup, &r, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                ui.set_min_width(190.0);
                if ui.button(format!("{}  Floor on top", icon::ARROW_UP)).clicked() {
                    *add_level = true;
                }
                if ui.button(format!("{}  Basement underneath", icon::ARROW_DOWN)).clicked() {
                    *add_basement = true;
                }
            });
        });
    });
}

fn section_header(ui: &mut Ui, title: &str, add_tip: Option<&str>, add: &mut bool) {
    ui.horizontal(|ui| {
        ui.set_min_height(26.0);
        ui.add_space(6.0);
        ui.label(theme::caps(title));
        if let Some(tip) = add_tip {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::icon_button_sized(ui, icon::PLUS, tip, false, 26.0).clicked() {
                    *add = true;
                }
            });
        }
    });
}

fn item_rows(ui: &mut Ui, depth: usize, objects: &[&PlacedObject], sel: Option<EntityRef>, clicked: &mut Option<EntityRef>) {
    for o in objects {
        let e = EntityRef::Object(o.id);
        if nav_row(ui, depth, icon::PACKAGE, &o.display_name(), sel == Some(e), None, None).0.clicked() {
            *clicked = Some(e);
        }
    }
}

/// A floor: its rooms (with items, doors, windows) and its roof sections.
#[allow(clippy::too_many_arguments)]
fn level_tree(
    ui: &mut Ui,
    project: &Project,
    level: &Level,
    label: &str,
    depth: usize,
    sel: Option<EntityRef>,
    clicked: &mut Option<EntityRef>,
    moved: &mut Option<(LevelId, bool)>,
) {
    let lid = level.id;
    let open_id = ui.make_persistent_id(("nav-level", lid));
    let open = open_state(ui, open_id, true);
    let (r, caret) = nav_row(ui, depth, icon::STACK, label, sel == Some(EntityRef::Level(lid)), Some(open), None);
    let (below, above) = project.neighbour_levels(lid);
    r.context_menu(|ui| {
        if ui.add_enabled(above.is_some(), egui::Button::new(format!("{}  Move up", icon::ARROW_UP))).clicked() {
            *moved = Some((lid, true));
            ui.close_menu();
        }
        if ui.add_enabled(below.is_some(), egui::Button::new(format!("{}  Move down", icon::ARROW_DOWN))).clicked() {
            *moved = Some((lid, false));
            ui.close_menu();
        }
    });
    if caret.is_some_and(|c| c.clicked()) {
        toggle_state(ui, open_id, true);
    } else if r.clicked() {
        *clicked = Some(EntityRef::Level(lid));
    }
    if !open {
        return;
    }
    let mut rooms: Vec<&Room> = level.rooms.values().collect();
    rooms.sort_by_key(|r| r.display_name().to_lowercase());
    if rooms.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(48.0 + depth as f32 * 16.0);
            ui.label(egui::RichText::new("No rooms yet").size(12.5).color(c::faint()));
        });
    }
    for room in rooms {
        let walls = level.walls_of_room(room);
        let objects: Vec<&PlacedObject> = level.objects.values().filter(|o| o.room == Some(room.id) && o.roof.is_none()).collect();
        let openings: Vec<&Opening> = level.openings.values().filter(|o| walls.contains(&o.wall)).collect();
        let has_children = !objects.is_empty() || !openings.is_empty();
        let rid = ui.make_persistent_id(("nav-room", room.id));
        let ropen = open_state(ui, rid, false);
        let (r, caret) = nav_row(
            ui,
            depth + 1,
            theme::room_icon(&room.kind, &room.name),
            &room.display_name(),
            sel == Some(EntityRef::Room(room.id)),
            has_children.then_some(ropen),
            None,
        );
        if caret.is_some_and(|c| c.clicked()) {
            toggle_state(ui, rid, false);
        } else if r.clicked() {
            *clicked = Some(EntityRef::Room(room.id));
        }
        if !(ropen && has_children) {
            continue;
        }
        item_rows(ui, depth + 2, &objects, sel, clicked);
        for o in openings {
            let e = EntityRef::Opening(o.id);
            let i = if o.kind.is_door() { icon::DOOR } else { icon::APP_WINDOW };
            let name = if o.label.trim().is_empty() { o.kind.title() } else { o.label.as_str() };
            if nav_row(ui, depth + 2, i, name, sel == Some(e), None, None).0.clicked() {
                *clicked = Some(e);
            }
        }
    }
    for roof in level.roofs.values() {
        let objects: Vec<&PlacedObject> = level.objects.values().filter(|o| o.roof == Some(roof.id)).collect();
        let id = ui.make_persistent_id(("nav-roof", roof.id));
        let ropen = open_state(ui, id, false);
        let (r, caret) = nav_row(
            ui,
            depth + 1,
            icon::TRIANGLE,
            &roof.display_name(),
            sel == Some(EntityRef::RoofSection(roof.id)),
            (!objects.is_empty()).then_some(ropen),
            None,
        );
        if caret.is_some_and(|c| c.clicked()) {
            toggle_state(ui, id, false);
        } else if r.clicked() {
            *clicked = Some(EntityRef::RoofSection(roof.id));
        }
        if ropen {
            item_rows(ui, depth + 2, &objects, sel, clicked);
        }
    }
}

/// The Outside level: lot, decks, fences, trees and loose items.
fn site_tree(ui: &mut Ui, project: &Project, site: &Level, sel: Option<EntityRef>, viewing: bool, clicked: &mut Option<EntityRef>) {
    let _ = project;
    let open_id = ui.make_persistent_id(("nav-site", site.id));
    let open = open_state(ui, open_id, true);
    let label = if viewing { format!("{}  ·  viewing", site.name) } else { site.name.clone() };
    let (r, caret) = nav_row(ui, 0, icon::TREE, &label, sel == Some(EntityRef::Level(site.id)), Some(open), None);
    if caret.is_some_and(|c| c.clicked()) {
        toggle_state(ui, open_id, true);
    } else if r.clicked() {
        *clicked = Some(EntityRef::Level(site.id));
    }
    if !open {
        return;
    }
    let mut features: Vec<&Feature> = site.features.values().collect();
    features.sort_by_key(|f| (f.kind != FeatureKind::PropertyLine, f.kind.label(), f.display_name()));
    if features.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(48.0);
            ui.label(egui::RichText::new("Nothing drawn yet").size(12.5).color(c::faint()));
        });
    }
    for f in features {
        let objects: Vec<&PlacedObject> = site.objects.values().filter(|o| o.feature == Some(f.id)).collect();
        let id = ui.make_persistent_id(("nav-feature", f.id));
        let fopen = open_state(ui, id, false);
        let (r, caret) = nav_row(
            ui,
            1,
            theme::feature_icon(f.kind),
            &f.display_name(),
            sel == Some(EntityRef::Feature(f.id)),
            (!objects.is_empty()).then_some(fopen),
            None,
        );
        if caret.is_some_and(|c| c.clicked()) {
            toggle_state(ui, id, false);
        } else if r.clicked() {
            *clicked = Some(EntityRef::Feature(f.id));
        }
        if fopen {
            item_rows(ui, 2, &objects, sel, clicked);
        }
    }
    let loose: Vec<&PlacedObject> = site.objects.values().filter(|o| o.feature.is_none()).collect();
    item_rows(ui, 1, &loose, sel, clicked);
}

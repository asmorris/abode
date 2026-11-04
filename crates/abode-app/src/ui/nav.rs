//! Left-hand tree: house, roof, levels → rooms → items/doors/windows.

use super::{Env, MainTab};
use abode_core::*;
use eframe::egui::{self, RichText, Ui, collapsing_header::CollapsingState};

pub fn nav_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, tab: &mut MainTab) {
    let sel = *env.selection;
    let mut clicked: Option<EntityRef> = None;
    let mut add_level = false;

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(6.0);
        let house = RichText::new(format!("🏠 {}", project.entity_name(EntityRef::General))).strong();
        if ui
            .selectable_label(sel == Some(EntityRef::General), house)
            .on_hover_text("House details and general notes")
            .clicked()
        {
            clicked = Some(EntityRef::General);
        }
        if ui.selectable_label(sel == Some(EntityRef::Roof), "🔺 Roof").clicked() {
            clicked = Some(EntityRef::Roof);
        }
        let overdue = project.overdue_count(env.today);
        let rem = if overdue > 0 {
            RichText::new(format!("⏰ Reminders · {overdue} overdue")).color(ui.visuals().error_fg_color)
        } else {
            RichText::new("⏰ Reminders")
        };
        if ui.selectable_label(*tab == MainTab::Reminders, rem).clicked() {
            *tab = MainTab::Reminders;
        }
        if ui.selectable_label(*tab == MainTab::Timeline, "🕘 Timeline").clicked() {
            *tab = MainTab::Timeline;
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.strong("Levels");
            if ui.small_button("+").on_hover_text("Add a level").clicked() {
                add_level = true;
            }
        });

        for level in &project.levels {
            let lid = level.id;
            let active = *env.active_level == lid;
            CollapsingState::load_with_default_open(ui.ctx(), ui.make_persistent_id(("nav-level", lid)), true)
                .show_header(ui, |ui| {
                    let mut text = RichText::new(format!("🏢 {}", level.name));
                    if active {
                        text = text.strong();
                    }
                    if ui.selectable_label(sel == Some(EntityRef::Level(lid)), text).clicked() {
                        clicked = Some(EntityRef::Level(lid));
                    }
                })
                .body(|ui| {
                    let mut rooms: Vec<&Room> = level.rooms.values().collect();
                    rooms.sort_by_key(|r| r.display_name().to_lowercase());
                    if rooms.is_empty() {
                        ui.weak("No rooms yet");
                    }
                    for room in rooms {
                        let walls = level.walls_of_room(room);
                        let objects: Vec<&PlacedObject> = level.objects.values().filter(|o| o.room == room.id).collect();
                        let openings: Vec<&Opening> = level.openings.values().filter(|o| walls.contains(&o.wall)).collect();
                        CollapsingState::load_with_default_open(
                            ui.ctx(),
                            ui.make_persistent_id(("nav-room", room.id)),
                            false,
                        )
                        .show_header(ui, |ui| {
                            if ui
                                .selectable_label(sel == Some(EntityRef::Room(room.id)), room.display_name())
                                .clicked()
                            {
                                clicked = Some(EntityRef::Room(room.id));
                            }
                        })
                        .body(|ui| {
                            for o in objects {
                                let r = EntityRef::Object(o.id);
                                if ui.selectable_label(sel == Some(r), format!("📦 {}", o.display_name())).clicked() {
                                    clicked = Some(r);
                                }
                            }
                            for o in openings {
                                let r = EntityRef::Opening(o.id);
                                let icon = if o.kind.is_door() { "🚪" } else { "🔲" };
                                let name = if o.label.trim().is_empty() { o.kind.title() } else { o.label.as_str() };
                                if ui.selectable_label(sel == Some(r), format!("{icon} {name}")).clicked() {
                                    clicked = Some(r);
                                }
                            }
                        });
                    }
                });
        }
    });

    if add_level {
        let n = project.levels.len() + 1;
        let id = project.add_level(format!("Level {n}"));
        *env.active_level = id;
        clicked = Some(EntityRef::Level(id));
    }
    if let Some(c) = clicked {
        *env.selection = Some(c);
        if let Some(li) = project.level_index_of(c) {
            *env.active_level = project.levels[li].id;
            if matches!(*tab, MainTab::Timeline | MainTab::Reminders) {
                *tab = MainTab::Plan;
            }
        }
    }
}

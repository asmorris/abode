//! Right-hand floating panel: edit whatever is selected.

use super::notes::history_sections;
use super::widgets::*;
use super::{Action, Env};
use crate::theme::{self, c, icon};
use abode_core::geometry::{area, perimeter};
use abode_core::units::{UnitSystem, format_area, format_length};
use abode_core::*;
use eframe::egui::{self, RichText, Ui};

/// Small label above the title, e.g. "ROOM".
pub fn kind_label(project: &Project, sel: EntityRef) -> String {
    match sel {
        EntityRef::General => "House".into(),
        EntityRef::Roof => "Roof".into(),
        EntityRef::Level(_) => "Level".into(),
        EntityRef::Room(_) => "Room".into(),
        EntityRef::Wall(_) => "Wall".into(),
        EntityRef::Opening(id) => project
            .find_opening(id)
            .map(|(_, o)| o.kind.title().to_string())
            .unwrap_or_default(),
        EntityRef::Object(id) => project
            .find_object(id)
            .map(|(_, o)| o.category.label().to_string())
            .unwrap_or_default(),
    }
}

pub fn inspector_ui(ui: &mut Ui, project: &mut Project, env: &mut Env) {
    let Some(sel) = *env.selection else { return };
    if !project.exists(sel) {
        *env.selection = None;
        return;
    }
    ui.horizontal(|ui| {
        ui.label(theme::caps(kind_label(project, sel)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::icon_button_sized(ui, icon::X, "Close (Esc)", false, 28.0).clicked() {
                *env.selection = None;
            }
        });
    });
    if env.selection.is_none() {
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .id_salt("inspector-scroll")
        .show(ui, |ui| {
            ui.set_width(ui.available_width() - 6.0);
            match sel {
                EntityRef::General => house_ui(ui, project, env),
                EntityRef::Roof => roof_ui(ui, project, env),
                EntityRef::Level(id) => level_ui(ui, project, env, id),
                EntityRef::Room(id) => room_ui(ui, project, env, id),
                EntityRef::Wall(id) => wall_ui(ui, project, env, id),
                EntityRef::Opening(id) => opening_ui(ui, project, env, id),
                EntityRef::Object(id) => object_ui(ui, project, env, id),
            }
            ui.add_space(12.0);
        });
}

fn heading(ui: &mut Ui, title: &str, subtitle: Option<String>) {
    ui.label(theme::title(title));
    if let Some(s) = subtitle {
        ui.label(theme::muted(s));
    }
    ui.add_space(6.0);
}

fn link(ui: &mut Ui, env: &mut Env, text: &str, target: EntityRef) {
    if ui
        .add(egui::Button::new(RichText::new(text).color(c::FOREST)).frame(false))
        .clicked()
    {
        env.actions.push(Action::Reveal(target));
    }
}

/// A row in a list of related things (walls, items...): icon, link, detail.
fn related_row(ui: &mut Ui, env: &mut Env, i: &str, text: &str, detail: Option<String>, target: EntityRef) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(i).color(c::MUTED));
        link(ui, env, text, target);
        if let Some(d) = detail {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(theme::muted(d));
            });
        }
    });
}

// ---------------------------------------------------------------------------

fn house_ui(ui: &mut Ui, project: &mut Project, env: &mut Env) {
    heading(ui, &project.entity_name(EntityRef::General), Some("Details, general notes and house-wide reminders".into()));
    let units = project.info.units;
    let info = &mut project.info;
    theme::section(ui, "house-details", icon::HOUSE_LINE, "Details", None, true, |ui| {
        form_grid(ui, "house-grid", |ui| {
            text_row(ui, "Name", &mut info.name, "e.g. Maple Street house");
            text_row(ui, "Address", &mut info.address, "");
            field_label(ui, "Year built");
            ui.horizontal(|ui| {
                let mut clear = false;
                match &mut info.year_built {
                    Some(y) => {
                        ui.add(egui::DragValue::new(y).range(1600..=2100));
                        clear = theme::icon_button_sized(ui, icon::X, "Clear", false, 26.0).clicked();
                    }
                    None => {
                        if theme::soft_button(ui, "Set").clicked() {
                            info.year_built = Some(2000);
                        }
                    }
                }
                if clear {
                    info.year_built = None;
                }
            });
            ui.end_row();
            field_label(ui, "Units");
            combo(ui, "units", &mut info.units, &UnitSystem::ALL, UnitSystem::label);
            ui.end_row();
            field_label(ui, "New walls");
            length_edit(ui, "house-wall-thickness", &mut info.default_wall_thickness, units);
            ui.end_row();
            field_label(ui, "Created");
            ui.label(fmt_datetime(info.created));
            ui.end_row();
        });
    });
    let n = info.custom_fields.len();
    theme::section(ui, "house-custom", icon::TAG, "Custom fields", Some(n), n > 0, |ui| {
        custom_fields(ui, "house", &mut info.custom_fields);
    });
    history_sections(ui, project, env, EntityRef::General);
}

fn roof_ui(ui: &mut Ui, project: &mut Project, env: &mut Env) {
    heading(ui, "Roof", None);
    let units = project.info.units;
    let today = env.today;
    let roof = &mut project.roof;
    theme::section(ui, "roof-details", icon::TRIANGLE, "Details", None, true, |ui| {
        form_grid(ui, "roof-grid", |ui| {
            text_row(ui, "Material", &mut roof.material, "e.g. Asphalt shingles, metal");
            text_row(ui, "Colour", &mut roof.color, "");
            text_row(ui, "Pitch", &mut roof.pitch, "e.g. 6/12");
            field_label(ui, "Area");
            ui.horizontal(|ui| {
                let mut clear = false;
                match &mut roof.area_m2 {
                    Some(a) => {
                        let mut v = units.area_to_display(*a);
                        let r = ui.add(
                            egui::DragValue::new(&mut v)
                                .speed(1.0)
                                .range(0.0..=100_000.0)
                                .suffix(format!(" {}", units.area_unit())),
                        );
                        if r.changed() {
                            *a = units.area_from_display(v);
                        }
                        clear = theme::icon_button_sized(ui, icon::X, "Clear", false, 26.0).clicked();
                    }
                    None => {
                        if theme::soft_button(ui, "Set").clicked() {
                            roof.area_m2 = Some(150.0);
                        }
                    }
                }
                if clear {
                    roof.area_m2 = None;
                }
            });
            ui.end_row();
            field_label(ui, "Installed");
            opt_date_edit(ui, "roof-installed", &mut roof.installed, today);
            ui.end_row();
            text_row(ui, "Installed by", &mut roof.installed_by, "Roofing company");
            field_label(ui, "Warranty until");
            opt_date_edit(ui, "roof-warranty", &mut roof.warranty_expires, today);
            ui.end_row();
            text_row(ui, "Warranty details", &mut roof.warranty_details, "");
            field_label(ui, "Last inspected");
            opt_date_edit(ui, "roof-inspected", &mut roof.last_inspected, today);
            ui.end_row();
        });
    });
    let n = roof.custom_fields.len();
    theme::section(ui, "roof-custom", icon::TAG, "Custom fields", Some(n), n > 0, |ui| {
        custom_fields(ui, "roof", &mut roof.custom_fields);
    });
    history_sections(ui, project, env, EntityRef::Roof);
}

fn level_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, id: LevelId) {
    let units = project.info.units;
    let can_delete = project.levels.len() > 1;
    let Some(level) = project.level_mut(id) else { return };
    heading(ui, &level.name.clone(), Some(format!("{} rooms", level.rooms.len())));
    theme::section(ui, ("level-details", id), icon::STACK, "Details", None, true, |ui| {
        form_grid(ui, ("level-grid", id), |ui| {
            text_row(ui, "Name", &mut level.name, "e.g. Basement, Second floor");
            field_label(ui, "Floor height");
            length_edit_min(ui, ("level-elev", id), &mut level.elevation, units, -100.0);
            ui.end_row();
            field_label(ui, "Wall height");
            length_edit(ui, ("level-wallh", id), &mut level.wall_height, units);
            ui.end_row();
        });
        ui.label(theme::muted("Floor height is relative to the main floor (negative for a basement)."));
    });

    theme::section(ui, ("underlay", id), icon::IMAGE_SQUARE, "Blueprint", None, true, |ui| {
        let mut remove = false;
        match &mut level.underlay {
            Some(u) => {
                ui.checkbox(&mut u.visible, "Show blueprint");
                ui.add(egui::Slider::new(&mut u.opacity, 0.05..=1.0).text("Opacity"));
                ui.horizontal(|ui| {
                    field_label(ui, "Rotation");
                    let mut deg = u.rotation.to_degrees();
                    if ui.add(egui::DragValue::new(&mut deg).speed(0.25).suffix("°")).changed() {
                        u.rotation = deg.to_radians();
                    }
                });
                let width_m = u.meters_per_pixel * u.pixel_size[0] as f64;
                ui.label(theme::muted(format!("Image is {} wide", format_length(width_m, units))));
                ui.label(theme::muted(format!(
                    "Set the scale with {} Calibrate on the toolbar: click two points a known distance apart. Line it up with {} Move.",
                    icon::RULER,
                    icon::HAND_GRABBING
                )));
                ui.horizontal(|ui| {
                    if theme::secondary_button(ui, "Replace image…").clicked() {
                        env.actions.push(Action::ImportUnderlay(id));
                    }
                    remove = danger_button(ui, "Remove");
                });
            }
            None => {
                ui.label(theme::muted("Import a scan or photo of a floor plan, then draw your rooms on top of it."));
                if theme::secondary_button(ui, format!("{}  Import blueprint image…", icon::UPLOAD_SIMPLE)).clicked() {
                    env.actions.push(Action::ImportUnderlay(id));
                }
            }
        }
        if remove {
            level.underlay = None;
        }
    });
    history_sections(ui, project, env, EntityRef::Level(id));
    if can_delete {
        ui.add_space(6.0);
        if danger_button(ui, "Delete level…") {
            env.dialogs.delete = Some(EntityRef::Level(id));
        }
    }
}

// ---------------------------------------------------------------------------

fn paint_editor(ui: &mut Ui, paint: &mut Paint, label: &str, today: chrono::NaiveDate, delete: &mut Option<PaintId>) {
    let id = paint.id;
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            ui.make_persistent_id(("paint", id)),
            paint.color_name.is_empty(),
        )
        .show_header(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
            match paint.swatch {
                Some([r, g, b]) => {
                    ui.painter().rect_filled(rect, 6.0, egui::Color32::from_rgb(r, g, b));
                    ui.painter()
                        .rect_stroke(rect, 6.0, egui::Stroke::new(1.0, c::BORDER_STRONG), egui::StrokeKind::Inside);
                }
                None => {
                    ui.painter()
                        .text(rect.center(), egui::Align2::CENTER_CENTER, icon::PAINT_BRUSH, egui::FontId::proportional(16.0), c::MUTED);
                }
            }
            let name = if paint.color_name.is_empty() { "New paint" } else { &paint.color_name };
            ui.label(theme::strong(name));
            ui.label(theme::muted(label));
        })
        .body_unindented(|ui| {
            form_grid(ui, ("paint-grid", id), |ui| {
                field_label(ui, "Surface");
                combo(ui, ("paint-surface", id), &mut paint.surface, &PaintSurface::ALL, PaintSurface::label);
                ui.end_row();
                text_row(ui, "Brand", &mut paint.brand, "e.g. Benjamin Moore");
                text_row(ui, "Colour name", &mut paint.color_name, "e.g. Pale Oak");
                text_row(ui, "Colour code", &mut paint.color_code, "e.g. OC-20");
                field_label(ui, "Finish");
                combo(ui, ("paint-finish", id), &mut paint.finish, &PaintFinish::ALL, PaintFinish::label);
                ui.end_row();
                field_label(ui, "Swatch");
                ui.horizontal(|ui| {
                    let mut has = paint.swatch.is_some();
                    if ui.checkbox(&mut has, "").changed() {
                        paint.swatch = has.then_some([200, 200, 200]);
                    }
                    if let Some(c) = &mut paint.swatch {
                        ui.color_edit_button_srgb(c);
                    }
                });
                ui.end_row();
                field_label(ui, "Painted");
                opt_date_edit(ui, &format!("paint-date-{id}"), &mut paint.applied, today);
                ui.end_row();
                text_row(ui, "Leftover cans", &mut paint.leftover_location, "e.g. Garage shelf");
            });
            ui.add(
                egui::TextEdit::multiline(&mut paint.notes)
                    .hint_text("Notes (coats, primer, store…)")
                    .desired_rows(1)
                    .desired_width(f32::INFINITY),
            );
            if danger_button(ui, "Remove paint") {
                *delete = Some(id);
            }
        });
    });
}

/// Paint list + add buttons for the given targets.
fn paint_section(
    ui: &mut Ui,
    project: &mut Project,
    env: &mut Env,
    id_salt: impl std::hash::Hash,
    targets: &[(PaintTarget, String)],
) {
    let count = project
        .paints
        .iter()
        .filter(|p| targets.iter().any(|(t, _)| *t == p.target))
        .count();
    theme::section(ui, id_salt, icon::PAINT_ROLLER, "Paint", Some(count), count > 0, |ui| {
        let mut delete = None;
        for paint in project.paints.iter_mut() {
            if let Some((_, label)) = targets.iter().find(|(t, _)| *t == paint.target) {
                paint_editor(ui, paint, label, env.today, &mut delete);
            }
        }
        if let Some(id) = delete {
            project.paints.retain(|p| p.id != id);
        }
        if let [(t, label)] = targets {
            if theme::soft_button(ui, format!("{}  Add paint ({label})", icon::PLUS)).clicked() {
                project.paints.push(Paint::new(*t));
            }
        } else {
            ui.menu_button(
                RichText::new(format!("{}  Add paint  {}", icon::PLUS, icon::CARET_DOWN)).color(c::FOREST).size(13.0),
                |ui| {
                    for (t, label) in targets {
                        if ui.button(label).clicked() {
                            project.paints.push(Paint::new(*t));
                            ui.close_menu();
                        }
                    }
                },
            );
        }
    });
}

// ---------------------------------------------------------------------------

fn room_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, id: RoomId) {
    let units = project.info.units;
    let Some(li) = project.level_index_of(EntityRef::Room(id)) else { return };
    let (level_height, pts_area, pts_perim, dims, walls, openings, objects) = {
        let level = &project.levels[li];
        let room = &level.rooms[&id];
        let pts = level.room_points(room);
        let walls: Vec<(WallId, String, f64)> = level
            .walls_of_room(room)
            .into_iter()
            .map(|w| (w, project.entity_name(EntityRef::Wall(w)), level.wall_length(&level.walls[&w])))
            .collect();
        let wall_ids: Vec<WallId> = walls.iter().map(|w| w.0).collect();
        let openings: Vec<(OpeningId, String, bool)> = level
            .openings
            .values()
            .filter(|o| wall_ids.contains(&o.wall))
            .map(|o| (o.id, project.entity_name(EntityRef::Opening(o.id)), o.kind.is_door()))
            .collect();
        let objects: Vec<(ObjectId, String)> = level
            .objects
            .values()
            .filter(|o| o.room == id)
            .map(|o| (o.id, o.display_name()))
            .collect();
        (level.wall_height, area(&pts), perimeter(&pts), level.rect_dims(room), walls, openings, objects)
    };

    let room = project.levels[li].rooms.get_mut(&id).unwrap();
    heading(
        ui,
        &room.display_name(),
        Some(format!("{} · {}", format_area(pts_area, units), format_length(pts_perim, units) + " around")),
    );
    let room_icon = theme::room_icon(&room.kind, &room.name);
    theme::section(ui, ("room-details", id), room_icon, "Details", None, true, |ui| {
        form_grid(ui, ("room-grid", id), |ui| {
            text_row(ui, "Name", &mut room.name, "e.g. Kitchen");
            field_label(ui, "Type");
            suggest_edit(ui, ("room-kind", id), &mut room.kind, ROOM_KIND_SUGGESTIONS, "e.g. Bedroom");
            ui.end_row();
            text_row(ui, "Flooring", &mut room.floor_finish, "e.g. Oak hardwood");
            field_label(ui, "Ceiling");
            opt_length_edit(ui, ("room-ceiling", id), &mut room.ceiling_height, level_height, units);
            ui.end_row();
        });
    });

    let locked = room.shape_locked;
    theme::section(ui, ("room-shape", id), icon::RULER, "Shape & size", None, true, |ui| {
        if locked {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::LOCK).color(c::MUTED));
                ui.label(theme::muted("Shape locked"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::secondary_button(ui, "Unlock…")
                        .on_hover_text("Room shapes rarely change, so they're locked once drawn")
                        .clicked()
                    {
                        env.dialogs.unlock_room = Some(id);
                    }
                });
            });
        } else {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::LOCK_OPEN).color(c::TEAL));
                ui.label(RichText::new("Unlocked: drag corners on the plan").color(c::TEAL));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::secondary_button(ui, "Lock").clicked() {
                        project.levels[li].rooms.get_mut(&id).unwrap().shape_locked = true;
                    }
                });
            });
        }
        if let (false, Some((_, w, d))) = (locked, dims) {
            let (mut w2, mut d2) = (w, d);
            let mut changed = false;
            form_grid(ui, ("room-dims", id), |ui| {
                field_label(ui, "Width");
                changed |= length_edit(ui, ("room-w", id), &mut w2, units);
                ui.end_row();
                field_label(ui, "Depth");
                changed |= length_edit(ui, ("room-d", id), &mut d2, units);
                ui.end_row();
            });
            if changed {
                if let Err(e) = project.levels[li].set_rect_size(id, w2, d2) {
                    env.toasts.error(e.to_string());
                }
            }
        } else if let Some((_, w, d)) = dims {
            ui.label(theme::strong(format!("{} × {}", format_length(w, units), format_length(d, units))));
        }
        ui.add_space(4.0);
        ui.label(theme::caps("Walls"));
        for (wid, name, len) in &walls {
            related_row(ui, env, icon::LINE_SEGMENT, name, Some(format_length(*len, units)), EntityRef::Wall(*wid));
        }
    });

    let mut targets = vec![(PaintTarget::Room(id), "whole room".to_string())];
    for (wid, name, _) in &walls {
        targets.push((PaintTarget::WallFace { wall: *wid, room: Some(id) }, name.clone()));
    }
    paint_section(ui, project, env, ("room-paint", id), &targets);

    theme::section(ui, ("room-openings", id), icon::DOOR, "Doors & windows", Some(openings.len()), true, |ui| {
        for (oid, name, door) in &openings {
            let i = if *door { icon::DOOR } else { icon::APP_WINDOW };
            related_row(ui, env, i, name, None, EntityRef::Opening(*oid));
        }
        ui.label(theme::muted("Add more with the Door and Window tools."));
    });
    theme::section(ui, ("room-objects", id), icon::PACKAGE, "Items", Some(objects.len()), true, |ui| {
        for (oid, name) in &objects {
            related_row(ui, env, icon::PACKAGE, name, None, EntityRef::Object(*oid));
        }
        ui.label(theme::muted("Add appliances, fixtures and furniture with the Item tool."));
    });
    let room = project.levels[li].rooms.get_mut(&id).unwrap();
    let n = room.custom_fields.len();
    theme::section(ui, ("room-custom", id), icon::TAG, "Custom fields", Some(n), n > 0, |ui| {
        custom_fields(ui, ("room", id), &mut room.custom_fields);
    });
    history_sections(ui, project, env, EntityRef::Room(id));
    ui.add_space(6.0);
    if danger_button(ui, "Delete room…") {
        env.dialogs.delete = Some(EntityRef::Room(id));
    }
}

fn wall_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, id: WallId) {
    let units = project.info.units;
    let Some(li) = project.level_index_of(EntityRef::Wall(id)) else { return };
    let level_id = project.levels[li].id;
    let (len, level_height, rooms, openings) = {
        let level = &project.levels[li];
        let wall = &level.walls[&id];
        let rooms: Vec<(RoomId, String)> = level
            .rooms_of_wall(wall)
            .into_iter()
            .map(|r| (r, level.rooms[&r].display_name()))
            .collect();
        let openings: Vec<(OpeningId, String, bool)> = level
            .openings
            .values()
            .filter(|o| o.wall == id)
            .map(|o| (o.id, project.entity_name(EntityRef::Opening(o.id)), o.kind.is_door()))
            .collect();
        (level.wall_length(wall), level.wall_height, rooms, openings)
    };
    let title = project.entity_name(EntityRef::Wall(id));
    heading(ui, &title, Some(format!("{} long", format_length(len, units))));
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::muted("Between"));
        for (i, (rid, name)) in rooms.iter().enumerate() {
            if i > 0 {
                ui.label(theme::muted("and"));
            }
            link(ui, env, name, EntityRef::Room(*rid));
        }
        if rooms.len() == 1 {
            ui.label(theme::muted("and the outside"));
        }
    });
    let wall = project.levels[li].walls.get_mut(&id).unwrap();
    theme::section(ui, ("wall-details", id), icon::LINE_SEGMENT, "Details", None, true, |ui| {
        form_grid(ui, ("wall-grid", id), |ui| {
            text_row(ui, "Label", &mut wall.label, "e.g. North wall, Accent wall");
            field_label(ui, "Thickness");
            length_edit(ui, ("wall-t", id), &mut wall.thickness, units);
            ui.end_row();
            field_label(ui, "Height");
            opt_length_edit(ui, ("wall-h", id), &mut wall.height, level_height, units);
            ui.end_row();
        });
        ui.label(theme::muted("Walls are shared: a wall between two rooms is the same wall in both."));
    });

    theme::section(ui, ("wall-openings", id), icon::DOOR, "Doors & windows", Some(openings.len()), true, |ui| {
        for (oid, name, door) in &openings {
            let i = if *door { icon::DOOR } else { icon::APP_WINDOW };
            related_row(ui, env, i, name, None, EntityRef::Opening(*oid));
        }
        ui.horizontal(|ui| {
            let mut add = None;
            if theme::soft_button(ui, format!("{}  Door", icon::PLUS)).clicked() {
                add = Some(OpeningKind::Door(DoorStyle::Hinged));
            }
            if theme::soft_button(ui, format!("{}  Window", icon::PLUS)).clicked() {
                add = Some(OpeningKind::Window(WindowStyle::DoubleHung));
            }
            if let Some(kind) = add {
                if let Some(oid) = project.add_opening(level_id, id, kind, len / 2.0) {
                    env.select(EntityRef::Opening(oid));
                }
            }
        });
    });

    let mut targets: Vec<(PaintTarget, String)> = rooms
        .iter()
        .map(|(r, name)| (PaintTarget::WallFace { wall: id, room: Some(*r) }, format!("{name} side")))
        .collect();
    if rooms.len() == 1 {
        targets.push((PaintTarget::WallFace { wall: id, room: None }, "outside".into()));
    }
    paint_section(ui, project, env, ("wall-paint", id), &targets);

    let wall = project.levels[li].walls.get_mut(&id).unwrap();
    let n = wall.custom_fields.len();
    theme::section(ui, ("wall-custom", id), icon::TAG, "Custom fields", Some(n), n > 0, |ui| {
        custom_fields(ui, ("wall", id), &mut wall.custom_fields);
    });
    history_sections(ui, project, env, EntityRef::Wall(id));
}

fn opening_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, id: OpeningId) {
    let units = project.info.units;
    let Some(li) = project.level_index_of(EntityRef::Opening(id)) else { return };
    let title = project.entity_name(EntityRef::Opening(id));
    let (wall_id, wall_name, wall_len) = {
        let level = &project.levels[li];
        let o = &level.openings[&id];
        let len = level.walls.get(&o.wall).map(|w| level.wall_length(w)).unwrap_or(0.0);
        (o.wall, project.entity_name(EntityRef::Wall(o.wall)), len)
    };
    let today = env.today;
    heading(ui, &title, None);
    ui.horizontal(|ui| {
        ui.label(theme::muted("On"));
        link(ui, env, &wall_name, EntityRef::Wall(wall_id));
        ui.label(theme::muted(format!("({} long)", format_length(wall_len, units))));
    });
    let level = &mut project.levels[li];
    let o = level.openings.get_mut(&id).unwrap();
    let mut geometry_changed = false;
    theme::section(ui, ("opening-details", id), icon::DOOR_OPEN, "Details", None, true, |ui| {
        form_grid(ui, ("opening-grid", id), |ui| {
            text_row(ui, "Label", &mut o.label, "e.g. Front door, Bay window");
            field_label(ui, "Kind");
            let mut is_door = o.kind.is_door();
            let before = is_door;
            theme::segmented(ui, &mut is_door, &[(true, "Door"), (false, "Window")]);
            if is_door != before {
                if is_door {
                    o.kind = OpeningKind::Door(DoorStyle::Hinged);
                    o.sill_height = 0.0;
                } else {
                    o.kind = OpeningKind::Window(WindowStyle::DoubleHung);
                }
            }
            ui.end_row();
            field_label(ui, "Style");
            match &mut o.kind {
                OpeningKind::Door(s) => combo(ui, ("door-style", id), s, &DoorStyle::ALL, DoorStyle::label),
                OpeningKind::Window(s) => combo(ui, ("window-style", id), s, &WindowStyle::ALL, WindowStyle::label),
            }
            ui.end_row();
            field_label(ui, "Width");
            geometry_changed |= length_edit(ui, ("op-w", id), &mut o.width, units);
            ui.end_row();
            field_label(ui, "Height");
            length_edit(ui, ("op-h", id), &mut o.height, units);
            ui.end_row();
            if !o.kind.is_door() {
                field_label(ui, "Sill height");
                length_edit_min(ui, ("op-sill", id), &mut o.sill_height, units, 0.0);
                ui.end_row();
            }
            field_label(ui, "From wall end");
            geometry_changed |= length_edit_min(ui, ("op-off", id), &mut o.offset, units, 0.0);
            ui.end_row();
            if o.kind.is_door() {
                field_label(ui, "Swing");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut o.flip_hinge, "Flip hinge");
                    ui.checkbox(&mut o.flip_swing, "Flip side");
                });
                ui.end_row();
            }
        });
    });
    if geometry_changed {
        level.clamp_openings();
    }
    let o = level.openings.get_mut(&id).unwrap();
    theme::section(ui, ("op-details", id), icon::RECEIPT, "Purchase & warranty", None, true, |ui| {
        details_editor(ui, &format!("op-{id}"), &mut o.details, today);
    });
    history_sections(ui, project, env, EntityRef::Opening(id));
    ui.add_space(6.0);
    if danger_button(ui, "Delete") {
        env.actions.push(Action::Delete(EntityRef::Opening(id)));
    }
}

fn object_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, id: ObjectId) {
    let units = project.info.units;
    let Some(li) = project.level_index_of(EntityRef::Object(id)) else { return };
    let today = env.today;
    let room_id = project.levels[li].objects[&id].room;
    let room_name = project.entity_name(EntityRef::Room(room_id));
    let name = project.levels[li].objects[&id].display_name();
    heading(ui, &name, None);
    ui.horizontal(|ui| {
        ui.label(theme::muted("In"));
        link(ui, env, &room_name, EntityRef::Room(room_id));
    });
    let o = project.levels[li].objects.get_mut(&id).unwrap();
    theme::section(ui, ("obj-main", id), icon::PACKAGE, "Details", None, true, |ui| {
        form_grid(ui, ("obj-grid", id), |ui| {
            text_row(ui, "Label", &mut o.label, "e.g. Fridge, Furnace");
            field_label(ui, "Category");
            combo(ui, ("obj-cat", id), &mut o.category, &ObjectCategory::ALL, ObjectCategory::label);
            ui.end_row();
            field_label(ui, "Shape");
            let before = o.shape.kind;
            combo(ui, ("obj-shape", id), &mut o.shape.kind, &ShapeKind::ALL, ShapeKind::label);
            if before != o.shape.kind && o.shape.kind == ShapeKind::Circle {
                o.shape.depth = o.shape.width;
            }
            ui.end_row();
            if o.shape.kind == ShapeKind::Circle {
                field_label(ui, "Diameter");
                if length_edit(ui, ("obj-diam", id), &mut o.shape.width, units) {
                    o.shape.depth = o.shape.width;
                }
                ui.end_row();
            } else {
                field_label(ui, "Width");
                length_edit(ui, ("obj-w", id), &mut o.shape.width, units);
                ui.end_row();
                field_label(ui, "Depth");
                length_edit(ui, ("obj-d", id), &mut o.shape.depth, units);
                ui.end_row();
            }
            field_label(ui, "Rotation");
            let mut deg = o.rotation.to_degrees();
            if ui.add(egui::DragValue::new(&mut deg).speed(1.0).suffix("°")).changed() {
                o.rotation = deg.rem_euclid(360.0).to_radians();
            }
            ui.end_row();
            field_label(ui, "Colour");
            ui.horizontal(|ui| {
                let mut custom = o.color.is_some();
                if ui.checkbox(&mut custom, "Custom").changed() {
                    o.color = custom.then(|| o.category.color());
                }
                if let Some(c) = &mut o.color {
                    ui.color_edit_button_srgb(c);
                }
            });
            ui.end_row();
        });
        ui.label(theme::muted("Drag it on the plan to move it, even into another room."));
    });
    theme::section(ui, ("obj-details", id), icon::RECEIPT, "Purchase & warranty", None, true, |ui| {
        details_editor(ui, &format!("obj-{id}"), &mut o.details, today);
    });
    history_sections(ui, project, env, EntityRef::Object(id));
    ui.add_space(6.0);
    if danger_button(ui, "Delete item") {
        env.actions.push(Action::Delete(EntityRef::Object(id)));
    }
}

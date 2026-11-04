//! Right-hand panel: edit whatever is selected.

use super::notes::history_sections;
use super::widgets::*;
use super::{Action, Env};
use abode_core::geometry::{area, perimeter};
use abode_core::units::{UnitSystem, format_area, format_length};
use abode_core::*;
use eframe::egui::{self, RichText, Ui};

pub fn inspector_ui(ui: &mut Ui, project: &mut Project, env: &mut Env) {
    let Some(sel) = *env.selection else {
        ui.add_space(12.0);
        ui.heading("Nothing selected");
        ui.label("Click a room, wall, door, window or item on the plan, or pick something from the list on the left.");
        ui.add_space(8.0);
        if ui.button("🏠 House details & general notes").clicked() {
            env.select(EntityRef::General);
        }
        return;
    };
    if !project.exists(sel) {
        *env.selection = None;
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .id_salt("inspector-scroll")
        .show(ui, |ui| {
            ui.add_space(4.0);
            match sel {
                EntityRef::General => house_ui(ui, project, env),
                EntityRef::Roof => roof_ui(ui, project, env),
                EntityRef::Level(id) => level_ui(ui, project, env, id),
                EntityRef::Room(id) => room_ui(ui, project, env, id),
                EntityRef::Wall(id) => wall_ui(ui, project, env, id),
                EntityRef::Opening(id) => opening_ui(ui, project, env, id),
                EntityRef::Object(id) => object_ui(ui, project, env, id),
            }
            ui.add_space(24.0);
        });
}

fn heading(ui: &mut Ui, kind: &str, title: &str) {
    ui.label(RichText::new(kind.to_uppercase()).small().weak());
    ui.heading(title);
    ui.add_space(4.0);
}

fn link(ui: &mut Ui, env: &mut Env, text: &str, target: EntityRef) {
    if ui.link(text).clicked() {
        env.actions.push(Action::Reveal(target));
    }
}

// ---------------------------------------------------------------------------

fn house_ui(ui: &mut Ui, project: &mut Project, env: &mut Env) {
    heading(ui, "House", &project.entity_name(EntityRef::General));
    let units = project.info.units;
    let info = &mut project.info;
    egui::Grid::new("house-grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        text_row(ui, "Name", &mut info.name, "e.g. Maple Street house");
        text_row(ui, "Address", &mut info.address, "");
        ui.label("Year built");
        ui.horizontal(|ui| {
            let mut clear = false;
            match &mut info.year_built {
                Some(y) => {
                    ui.add(egui::DragValue::new(y).range(1600..=2100));
                    clear = ui.small_button("🗙").clicked();
                }
                None => {
                    if ui.button("Set").clicked() {
                        info.year_built = Some(2000);
                    }
                }
            }
            if clear {
                info.year_built = None;
            }
        });
        ui.end_row();
        ui.label("Units");
        combo(ui, "units", &mut info.units, &UnitSystem::ALL, UnitSystem::label);
        ui.end_row();
        ui.label("New wall thickness");
        length_edit(ui, "house-wall-thickness", &mut info.default_wall_thickness, units);
        ui.end_row();
        ui.label("Created");
        ui.label(fmt_datetime(info.created));
        ui.end_row();
    });
    section(ui, "Custom fields", "house-custom", !info.custom_fields.is_empty(), |ui| {
        custom_fields(ui, "house", &mut info.custom_fields);
    });
    ui.separator();
    ui.label(RichText::new("General notes, history and reminders for the whole house").weak());
    history_sections(ui, project, env, EntityRef::General);
}

fn roof_ui(ui: &mut Ui, project: &mut Project, env: &mut Env) {
    heading(ui, "Roof", "Roof");
    let units = project.info.units;
    let today = env.today;
    let roof = &mut project.roof;
    egui::Grid::new("roof-grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        text_row(ui, "Material", &mut roof.material, "e.g. Asphalt shingles, metal");
        text_row(ui, "Colour", &mut roof.color, "");
        text_row(ui, "Pitch", &mut roof.pitch, "e.g. 6/12");
        ui.label("Area");
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
                    clear = ui.small_button("🗙").clicked();
                }
                None => {
                    if ui.button("Set").clicked() {
                        roof.area_m2 = Some(150.0);
                    }
                }
            }
            if clear {
                roof.area_m2 = None;
            }
        });
        ui.end_row();
        ui.label("Installed");
        opt_date_edit(ui, "roof-installed", &mut roof.installed, today);
        ui.end_row();
        text_row(ui, "Installed by", &mut roof.installed_by, "Roofing company");
        ui.label("Warranty until");
        opt_date_edit(ui, "roof-warranty", &mut roof.warranty_expires, today);
        ui.end_row();
        text_row(ui, "Warranty details", &mut roof.warranty_details, "");
        ui.label("Last inspected");
        opt_date_edit(ui, "roof-inspected", &mut roof.last_inspected, today);
        ui.end_row();
    });
    section(ui, "Custom fields", "roof-custom", !roof.custom_fields.is_empty(), |ui| {
        custom_fields(ui, "roof", &mut roof.custom_fields);
    });
    history_sections(ui, project, env, EntityRef::Roof);
}

fn level_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, id: LevelId) {
    let units = project.info.units;
    let can_delete = project.levels.len() > 1;
    let Some(level) = project.level_mut(id) else { return };
    heading(ui, "Level", &level.name.clone());
    egui::Grid::new(("level-grid", id)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        text_row(ui, "Name", &mut level.name, "e.g. Basement, Second floor");
        ui.label("Floor height");
        length_edit_min(ui, ("level-elev", id), &mut level.elevation, units, -100.0);
        ui.end_row();
        ui.label("Wall height");
        length_edit(ui, ("level-wallh", id), &mut level.wall_height, units);
        ui.end_row();
    });
    ui.weak("Floor height is relative to the main floor (negative for a basement).");
    ui.label(format!("{} rooms", level.rooms.len()));

    section(ui, "🗺 Blueprint to trace over", ("underlay", id), true, |ui| {
        let mut remove = false;
        match &mut level.underlay {
            Some(u) => {
                ui.checkbox(&mut u.visible, "Show blueprint");
                ui.add(egui::Slider::new(&mut u.opacity, 0.05..=1.0).text("Opacity"));
                ui.horizontal(|ui| {
                    ui.label("Rotation");
                    let mut deg = u.rotation.to_degrees();
                    if ui.add(egui::DragValue::new(&mut deg).speed(0.25).suffix("°")).changed() {
                        u.rotation = deg.to_radians();
                    }
                });
                let width_m = u.meters_per_pixel * u.pixel_size[0] as f64;
                ui.label(format!("Image is {} wide", format_length(width_m, units)));
                ui.weak("To set the scale, use 📏 Calibrate on the plan: click two points a known distance apart (a wall with a dimension printed on it works well). Use ✋ Move blueprint to drag it into place.");
                ui.horizontal(|ui| {
                    if ui.button("Replace image…").clicked() {
                        env.actions.push(Action::ImportUnderlay(id));
                    }
                    remove = danger_button(ui, "Remove");
                });
            }
            None => {
                ui.label("Import a scan or photo of a floor plan, then draw your rooms on top of it.");
                if ui.button("Import blueprint image…").clicked() {
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
        ui.add_space(8.0);
        if danger_button(ui, "Delete level…") {
            env.dialogs.delete = Some(EntityRef::Level(id));
        }
    }
}

// ---------------------------------------------------------------------------

fn paint_editor(ui: &mut Ui, paint: &mut Paint, label: &str, today: chrono::NaiveDate, delete: &mut Option<PaintId>) {
    let id = paint.id;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            if let Some([r, g, b]) = paint.swatch {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 3.0, egui::Color32::from_rgb(r, g, b));
            }
            let name = if paint.color_name.is_empty() { "Paint" } else { &paint.color_name };
            ui.strong(name);
            ui.weak(format!("· {label}"));
        });
        egui::CollapsingHeader::new("Details").id_salt(("paint", id)).show(ui, |ui| {
            egui::Grid::new(("paint-grid", id)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label("Surface");
                combo(ui, ("paint-surface", id), &mut paint.surface, &PaintSurface::ALL, PaintSurface::label);
                ui.end_row();
                text_row(ui, "Brand", &mut paint.brand, "e.g. Benjamin Moore");
                text_row(ui, "Colour name", &mut paint.color_name, "e.g. Pale Oak");
                text_row(ui, "Colour code", &mut paint.color_code, "e.g. OC-20");
                ui.label("Finish");
                combo(ui, ("paint-finish", id), &mut paint.finish, &PaintFinish::ALL, PaintFinish::label);
                ui.end_row();
                ui.label("Swatch");
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
                ui.label("Painted");
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

/// Paint list + add buttons. `faces` are the wall faces/room this panel is about.
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
    section(ui, &format!("🎨 Paint ({count})"), id_salt, count > 0, |ui| {
        let mut delete = None;
        for paint in project.paints.iter_mut() {
            if let Some((_, label)) = targets.iter().find(|(t, _)| *t == paint.target) {
                paint_editor(ui, paint, label, env.today, &mut delete);
            }
        }
        if let Some(id) = delete {
            project.paints.retain(|p| p.id != id);
        }
        ui.horizontal_wrapped(|ui| {
            if let [(t, label)] = targets {
                if ui.button(format!("+ Add paint ({label})")).clicked() {
                    project.paints.push(Paint::new(*t));
                }
            } else {
                ui.menu_button("+ Add paint ⏷", |ui| {
                    for (t, label) in targets {
                        if ui.button(label).clicked() {
                            project.paints.push(Paint::new(*t));
                            ui.close_menu();
                        }
                    }
                });
            }
        });
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
        let openings: Vec<(OpeningId, String)> = level
            .openings
            .values()
            .filter(|o| wall_ids.contains(&o.wall))
            .map(|o| (o.id, project.entity_name(EntityRef::Opening(o.id))))
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
    heading(ui, "Room", &room.display_name());
    egui::Grid::new(("room-grid", id)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        text_row(ui, "Name", &mut room.name, "e.g. Kitchen");
        ui.label("Type");
        suggest_edit(ui, ("room-kind", id), &mut room.kind, ROOM_KIND_SUGGESTIONS, "e.g. Bedroom");
        ui.end_row();
        text_row(ui, "Flooring", &mut room.floor_finish, "e.g. Oak hardwood");
        ui.label("Ceiling height");
        opt_length_edit(ui, ("room-ceiling", id), &mut room.ceiling_height, level_height, units);
        ui.end_row();
    });
    ui.label(format!(
        "Area {} · Perimeter {}",
        format_area(pts_area, units),
        format_length(pts_perim, units)
    ));

    let locked = room.shape_locked;
    section(ui, "📐 Shape & size", ("room-shape", id), true, |ui| {
        if locked {
            ui.horizontal(|ui| {
                ui.label("🔒 Shape locked");
                if ui
                    .button("Unlock…")
                    .on_hover_text("Room shapes rarely change, so they're locked once drawn")
                    .clicked()
                {
                    env.dialogs.unlock_room = Some(id);
                }
            });
        } else {
            ui.horizontal(|ui| {
                ui.label("🔓 Unlocked: drag corners on the plan.");
                if ui.button("Lock").clicked() {
                    project.levels[li].rooms.get_mut(&id).unwrap().shape_locked = true;
                }
            });
            ui.weak("It locks again when you select something else.");
        }
        if let (false, Some((_, w, d))) = (locked, dims) {
            let (mut w2, mut d2) = (w, d);
            let mut changed = false;
            egui::Grid::new(("room-dims", id)).num_columns(2).show(ui, |ui| {
                ui.label("Width");
                changed |= length_edit(ui, ("room-w", id), &mut w2, units);
                ui.end_row();
                ui.label("Depth");
                changed |= length_edit(ui, ("room-d", id), &mut d2, units);
                ui.end_row();
            });
            if changed {
                if let Err(e) = project.levels[li].set_rect_size(id, w2, d2) {
                    env.toasts.error(e.to_string());
                }
            }
        } else if let Some((_, w, d)) = dims {
            ui.label(format!("{} × {}", format_length(w, units), format_length(d, units)));
        }
        ui.add_space(4.0);
        ui.label(RichText::new("Walls").strong());
        for (wid, name, len) in &walls {
            ui.horizontal(|ui| {
                link(ui, env, name, EntityRef::Wall(*wid));
                ui.weak(format_length(*len, units));
            });
        }
    });

    let mut targets = vec![(PaintTarget::Room(id), "whole room".to_string())];
    for (wid, name, _) in &walls {
        targets.push((PaintTarget::WallFace { wall: *wid, room: Some(id) }, name.clone()));
    }
    paint_section(ui, project, env, ("room-paint", id), &targets);

    section(ui, &format!("🚪 Doors & windows ({})", openings.len()), ("room-openings", id), true, |ui| {
        for (oid, name) in &openings {
            link(ui, env, name, EntityRef::Opening(*oid));
        }
        ui.weak("Add more with the Door / Window tools on the plan.");
    });
    section(ui, &format!("📦 Items ({})", objects.len()), ("room-objects", id), true, |ui| {
        for (oid, name) in &objects {
            link(ui, env, name, EntityRef::Object(*oid));
        }
        ui.weak("Add appliances, fixtures etc. with the 📦 Item tool.");
    });
    let room = project.levels[li].rooms.get_mut(&id).unwrap();
    section(ui, "Custom fields", ("room-custom", id), !room.custom_fields.is_empty(), |ui| {
        custom_fields(ui, ("room", id), &mut room.custom_fields);
    });
    history_sections(ui, project, env, EntityRef::Room(id));
    ui.add_space(8.0);
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
        let openings: Vec<(OpeningId, String)> = level
            .openings
            .values()
            .filter(|o| o.wall == id)
            .map(|o| (o.id, project.entity_name(EntityRef::Opening(o.id))))
            .collect();
        (level.wall_length(wall), level.wall_height, rooms, openings)
    };
    let title = project.entity_name(EntityRef::Wall(id));
    let wall = project.levels[li].walls.get_mut(&id).unwrap();
    heading(ui, "Wall", &title);
    egui::Grid::new(("wall-grid", id)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        text_row(ui, "Label", &mut wall.label, "e.g. North wall, Accent wall");
        ui.label("Thickness");
        length_edit(ui, ("wall-t", id), &mut wall.thickness, units);
        ui.end_row();
        ui.label("Height");
        opt_length_edit(ui, ("wall-h", id), &mut wall.height, level_height, units);
        ui.end_row();
        ui.label("Length");
        ui.label(format_length(len, units));
        ui.end_row();
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Between");
        for (i, (rid, name)) in rooms.iter().enumerate() {
            if i > 0 {
                ui.label("and");
            }
            link(ui, env, name, EntityRef::Room(*rid));
        }
        if rooms.len() == 1 {
            ui.label("and the outside");
        }
    });
    ui.weak("Walls are shared: a wall between two rooms is the same wall in both.");

    section(ui, &format!("🚪 Doors & windows ({})", openings.len()), ("wall-openings", id), true, |ui| {
        for (oid, name) in &openings {
            link(ui, env, name, EntityRef::Opening(*oid));
        }
        ui.horizontal(|ui| {
            let mut add = None;
            if ui.button("+ Door").clicked() {
                add = Some(OpeningKind::Door(DoorStyle::Hinged));
            }
            if ui.button("+ Window").clicked() {
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
    section(ui, "Custom fields", ("wall-custom", id), !wall.custom_fields.is_empty(), |ui| {
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
    let level = &mut project.levels[li];
    let o = level.openings.get_mut(&id).unwrap();
    heading(ui, o.kind.title(), &title);
    let mut geometry_changed = false;
    egui::Grid::new(("opening-grid", id)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        text_row(ui, "Label", &mut o.label, "e.g. Front door, Bay window");
        ui.label("Kind");
        ui.horizontal(|ui| {
            let is_door = o.kind.is_door();
            if ui.selectable_label(is_door, "Door").clicked() && !is_door {
                o.kind = OpeningKind::Door(DoorStyle::Hinged);
                o.sill_height = 0.0;
            }
            if ui.selectable_label(!is_door, "Window").clicked() && is_door {
                o.kind = OpeningKind::Window(WindowStyle::DoubleHung);
            }
        });
        ui.end_row();
        ui.label("Style");
        match &mut o.kind {
            OpeningKind::Door(s) => combo(ui, ("door-style", id), s, &DoorStyle::ALL, DoorStyle::label),
            OpeningKind::Window(s) => combo(ui, ("window-style", id), s, &WindowStyle::ALL, WindowStyle::label),
        }
        ui.end_row();
        ui.label("Width");
        geometry_changed |= length_edit(ui, ("op-w", id), &mut o.width, units);
        ui.end_row();
        ui.label("Height");
        length_edit(ui, ("op-h", id), &mut o.height, units);
        ui.end_row();
        if !o.kind.is_door() {
            ui.label("Sill height");
            length_edit_min(ui, ("op-sill", id), &mut o.sill_height, units, 0.0);
            ui.end_row();
        }
        ui.label("Centre from wall end");
        geometry_changed |= length_edit_min(ui, ("op-off", id), &mut o.offset, units, 0.0);
        ui.end_row();
        if o.kind.is_door() {
            ui.label("Swing");
            ui.horizontal(|ui| {
                ui.checkbox(&mut o.flip_hinge, "Flip hinge");
                ui.checkbox(&mut o.flip_swing, "Flip side");
            });
            ui.end_row();
        }
    });
    if geometry_changed {
        level.clamp_openings();
    }
    let o = level.openings.get_mut(&id).unwrap();
    ui.horizontal(|ui| {
        ui.label("On");
        link(ui, env, &wall_name, EntityRef::Wall(wall_id));
        ui.weak(format!("({} long)", format_length(wall_len, units)));
    });
    section(ui, "📋 Details & warranty", ("op-details", id), true, |ui| {
        details_editor(ui, &format!("op-{id}"), &mut o.details, today);
    });
    history_sections(ui, project, env, EntityRef::Opening(id));
    ui.add_space(8.0);
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
    let o = project.levels[li].objects.get_mut(&id).unwrap();
    heading(ui, o.category.label(), &o.display_name());
    egui::Grid::new(("obj-grid", id)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        text_row(ui, "Label", &mut o.label, "e.g. Fridge, Furnace");
        ui.label("Category");
        combo(ui, ("obj-cat", id), &mut o.category, &ObjectCategory::ALL, ObjectCategory::label);
        ui.end_row();
        ui.label("Shape");
        let before = o.shape.kind;
        combo(ui, ("obj-shape", id), &mut o.shape.kind, &ShapeKind::ALL, ShapeKind::label);
        if before != o.shape.kind && o.shape.kind == ShapeKind::Circle {
            o.shape.depth = o.shape.width;
        }
        ui.end_row();
        if o.shape.kind == ShapeKind::Circle {
            ui.label("Diameter");
            if length_edit(ui, ("obj-diam", id), &mut o.shape.width, units) {
                o.shape.depth = o.shape.width;
            }
            ui.end_row();
        } else {
            ui.label("Width");
            length_edit(ui, ("obj-w", id), &mut o.shape.width, units);
            ui.end_row();
            ui.label("Depth");
            length_edit(ui, ("obj-d", id), &mut o.shape.depth, units);
            ui.end_row();
        }
        ui.label("Rotation");
        let mut deg = o.rotation.to_degrees();
        if ui
            .add(egui::DragValue::new(&mut deg).speed(1.0).suffix("°"))
            .changed()
        {
            o.rotation = deg.rem_euclid(360.0).to_radians();
        }
        ui.end_row();
        ui.label("Colour");
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
    ui.horizontal(|ui| {
        ui.label("In");
        link(ui, env, &room_name, EntityRef::Room(room_id));
        ui.weak("(drag it on the plan to move it to another room)");
    });
    section(ui, "📋 Details & warranty", ("obj-details", id), true, |ui| {
        details_editor(ui, &format!("obj-{id}"), &mut o.details, today);
    });
    history_sections(ui, project, env, EntityRef::Object(id));
    ui.add_space(8.0);
    if danger_button(ui, "Delete item") {
        env.actions.push(Action::Delete(EntityRef::Object(id)));
    }
}

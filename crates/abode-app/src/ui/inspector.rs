//! Right-hand floating panel: edit whatever is selected.

use super::notes::history_sections;
use super::widgets::*;
use super::{Action, Env};
use crate::theme::{self, c, icon};
use abode_core::geometry::{area, perimeter};
use abode_core::roof::{ROOF_MATERIAL_SUGGESTIONS, format_pitch};
use abode_core::units::{UnitSystem, format_area, format_length};
use abode_core::*;
use eframe::egui::{self, RichText, Ui};

/// Small label above the title, e.g. "ROOM".
pub fn kind_label(project: &Project, sel: EntityRef) -> String {
    match sel {
        EntityRef::General => "House".into(),
        EntityRef::Roof => "Roof".into(),
        EntityRef::Level(id) => match project.level(id) {
            Some(l) if l.is_site() => "Outside".into(),
            _ => "Level".into(),
        },
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
        EntityRef::RoofSection(_) => "Roof section".into(),
        EntityRef::Feature(id) => project
            .find_feature(id)
            .map(|(_, f)| format!("Outside · {}", f.kind.label()))
            .unwrap_or_default(),
        EntityRef::Building(id) => project
            .building(id)
            .map(|b| if b.kind.trim().is_empty() { "Outbuilding".to_string() } else { b.kind.clone() })
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
                EntityRef::RoofSection(id) => roof_section_ui(ui, project, env, id),
                EntityRef::Feature(id) => feature_ui(ui, project, env, id),
                EntityRef::Building(id) => building_ui(ui, project, env, id),
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
        .add(egui::Button::new(RichText::new(text).color(c::forest())).frame(false))
        .clicked()
    {
        env.actions.push(Action::Reveal(target));
    }
}

/// A row in a list of related things (walls, items...): icon, link, detail.
fn related_row(ui: &mut Ui, env: &mut Env, i: &str, text: &str, detail: Option<String>, target: EntityRef) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(i).color(c::muted()));
        link(ui, env, text, target);
        if let Some(d) = detail {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(theme::muted(d));
            });
        }
    });
}

/// Optional custom colour: a switch plus a colour button.
fn color_toggle(ui: &mut Ui, color: &mut Option<[u8; 3]>, default: [u8; 3], label: &str) {
    ui.horizontal(|ui| {
        let mut custom = color.is_some();
        if theme::toggle(ui, &mut custom, label).changed() {
            *color = custom.then_some(default);
        }
        if let Some(c) = color {
            ui.color_edit_button_srgb(c);
        }
    });
}

fn items_list(ui: &mut Ui, env: &mut Env, items: &[(ObjectId, String)], hint: &str) {
    for (oid, name) in items {
        related_row(ui, env, icon::PACKAGE, name, None, EntityRef::Object(*oid));
    }
    ui.label(theme::muted(hint));
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
    let units = project.info.units;
    let today = env.today;
    let sections: Vec<(LevelId, String, RoofId, String, String)> = project
        .levels
        .iter()
        .filter(|l| !l.is_site())
        .flat_map(|l| {
            let level_name = project.entity_name(EntityRef::Level(l.id));
            l.roofs.values().map(move |r| {
                let material = r.top_layer().map(|x| x.material.clone()).filter(|m| !m.is_empty()).unwrap_or_else(|| r.shape.label().into());
                (l.id, level_name.clone(), r.id, r.display_name(), format!("{} · {}", material, format_area(r.surface_area(), units)))
            })
        })
        .collect();
    let total: f64 = project.levels.iter().flat_map(|l| l.roofs.values()).map(|r| r.surface_area()).sum();
    heading(
        ui,
        "Roof",
        Some(if sections.is_empty() {
            "No roof sections drawn yet".into()
        } else {
            format!("{} sections · {} of roofing", sections.len(), format_area(total, units))
        }),
    );
    theme::section(ui, "roof-sections", icon::TRIANGLE, "Sections", Some(sections.len()), true, |ui| {
        let mut last_level = None;
        for (lid, level_name, rid, name, detail) in &sections {
            if last_level != Some(*lid) {
                ui.label(theme::caps(level_name));
                last_level = Some(*lid);
            }
            related_row(ui, env, icon::TRIANGLE, name, Some(detail.clone()), EntityRef::RoofSection(*rid));
        }
        ui.label(theme::muted(
            "Each floor has a Roof layer on the plan. Draw a section over the walls it covers: the main roof, a porch roof, a garage roof.",
        ));
        let first = project.first_house_level();
        if theme::secondary_button(ui, format!("{}  Edit roofs on the plan", icon::TRIANGLE)).clicked() {
            env.actions.push(Action::EditRoof(first));
        }
    });
    let roof = &mut project.roof;
    let has_overall = !roof.material.is_empty() || roof.installed.is_some() || roof.area_m2.is_some();
    theme::section(ui, "roof-details", icon::CLIPBOARD_TEXT, "Whole roof", None, has_overall, |ui| {
        ui.label(theme::muted("Optional, for details that cover every section. Each section has its own layers too."));
        form_grid(ui, "roof-grid", |ui| {
            field_label(ui, "Material");
            suggest_edit(ui, "roof-material", &mut roof.material, ROOF_MATERIAL_SUGGESTIONS, "e.g. Asphalt shingles");
            ui.end_row();
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
                            roof.area_m2 = Some(if total > 0.0 { total } else { 150.0 });
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
        });
    });
    theme::section(ui, "roof-inspection", icon::MAGNIFYING_GLASS, "Inspection", None, true, |ui| {
        form_grid(ui, "roof-insp-grid", |ui| {
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
    let Some(level) = project.level(id) else { return };
    let is_site = level.is_site();
    let building = level.building;
    let siblings = project.levels.iter().filter(|l| !l.is_site() && l.building == building).count();
    let can_delete = !is_site && siblings > 1;
    let roofs: Vec<(RoofId, String)> = level.roofs.values().map(|r| (r.id, r.display_name())).collect();
    let building_name = building.map(|b| project.entity_name(EntityRef::Building(b)));
    let subtitle = if is_site {
        let n = level.features.len();
        format!("{n} things outside · {} items", level.objects.len())
    } else {
        format!("{} rooms", level.rooms.len())
    };
    let level = project.level_mut(id).unwrap();
    heading(ui, &level.name.clone(), Some(subtitle));
    if let (Some(b), Some(name)) = (building, building_name) {
        ui.horizontal(|ui| {
            ui.label(theme::muted("Part of"));
            link(ui, env, &name, EntityRef::Building(b));
        });
    }
    if is_site {
        theme::section(ui, ("site-details", id), icon::TREE, "Outside", None, true, |ui| {
            form_grid(ui, ("site-grid", id), |ui| {
                text_row(ui, "Name", &mut level.name, "e.g. Yard, Lot");
            });
            ui.label(theme::muted(
                "Draw the property line, decks, patios, driveways, fences and trees on this plan. Add a shed or detached garage with the building tool; double-click it to open its own floor plan.",
            ));
        });
    } else {
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
        theme::section(ui, ("level-roofs", id), icon::TRIANGLE, "Roof sections", Some(roofs.len()), true, |ui| {
            for (rid, name) in &roofs {
                related_row(ui, env, icon::TRIANGLE, name, None, EntityRef::RoofSection(*rid));
            }
            if theme::secondary_button(ui, format!("{}  Edit roof layer", icon::TRIANGLE)).clicked() {
                env.actions.push(Action::EditRoof(id));
            }
        });
    }

    let level = project.level_mut(id).unwrap();
    let blueprint = if is_site { "Site plan or survey" } else { "Blueprint" };
    theme::section(ui, ("underlay", id), icon::IMAGE_SQUARE, blueprint, None, true, |ui| {
        let mut remove = false;
        match &mut level.underlay {
            Some(u) => {
                theme::toggle(ui, &mut u.visible, "Show it");
                ui.horizontal(|ui| {
                    field_label(ui, "Opacity");
                    ui.add(egui::Slider::new(&mut u.opacity, 0.05..=1.0).show_value(false));
                });
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
                ui.label(theme::muted(if is_site {
                    "Import a survey or plot plan, then trace the property line and everything outside over it."
                } else {
                    "Import a scan or photo of a floor plan, then draw your rooms on top of it."
                }));
                if theme::secondary_button(ui, format!("{}  Import image…", icon::UPLOAD_SIMPLE)).clicked() {
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
                        .rect_stroke(rect, 6.0, egui::Stroke::new(1.0_f32, c::border_strong()), egui::StrokeKind::Inside);
                }
                None => {
                    ui.painter()
                        .text(rect.center(), egui::Align2::CENTER_CENTER, icon::PAINT_BRUSH, egui::FontId::proportional(16.0), c::muted());
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
                color_toggle(ui, &mut paint.swatch, [200, 200, 200], "");
                ui.end_row();
                field_label(ui, "Applied");
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
    title: &str,
    targets: &[(PaintTarget, String)],
    new_paint: impl Fn(PaintTarget) -> Paint,
) {
    let count = project
        .paints
        .iter()
        .filter(|p| targets.iter().any(|(t, _)| *t == p.target))
        .count();
    theme::section(ui, id_salt, icon::PAINT_ROLLER, title, Some(count), count > 0, |ui| {
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
            if theme::soft_button(ui, format!("{}  Add ({label})", icon::PLUS)).clicked() {
                project.paints.push(new_paint(*t));
            }
        } else {
            ui.menu_button(
                RichText::new(format!("{}  Add paint  {}", icon::PLUS, icon::CARET_DOWN)).color(c::forest()).size(13.0),
                |ui| {
                    for (t, label) in targets {
                        if ui.button(label).clicked() {
                            project.paints.push(new_paint(*t));
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
            .filter(|o| o.room == Some(id) && o.roof.is_none())
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
                ui.label(RichText::new(icon::LOCK).color(c::muted()));
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
                ui.label(RichText::new(icon::LOCK_OPEN).color(c::teal()));
                ui.label(RichText::new("Unlocked: drag corners on the plan").color(c::teal()));
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
    paint_section(ui, project, env, ("room-paint", id), "Paint", &targets, Paint::new);

    theme::section(ui, ("room-openings", id), icon::DOOR, "Doors & windows", Some(openings.len()), true, |ui| {
        for (oid, name, door) in &openings {
            let i = if *door { icon::DOOR } else { icon::APP_WINDOW };
            related_row(ui, env, i, name, None, EntityRef::Opening(*oid));
        }
        ui.label(theme::muted("Add more with the Door and Window tools."));
    });
    theme::section(ui, ("room-objects", id), icon::PACKAGE, "Items", Some(objects.len()), true, |ui| {
        items_list(ui, env, &objects, "Add appliances, fixtures and furniture with the Item tool.");
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
    paint_section(ui, project, env, ("wall-paint", id), "Paint", &targets, Paint::new);

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
                ui.vertical(|ui| {
                    theme::toggle(ui, &mut o.flip_hinge, "Flip hinge");
                    theme::toggle(ui, &mut o.flip_swing, "Flip side");
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
    let (parent, verb) = {
        let o = &project.levels[li].objects[&id];
        match (o.roof, o.feature, o.room) {
            (Some(r), _, _) => (EntityRef::RoofSection(r), "On"),
            (_, Some(f), _) => (EntityRef::Feature(f), "On"),
            (_, _, Some(r)) => (EntityRef::Room(r), "In"),
            _ => (EntityRef::Level(project.levels[li].id), "Out in the"),
        }
    };
    let parent_name = project.entity_name(parent);
    let name = project.levels[li].objects[&id].display_name();
    heading(ui, &name, None);
    ui.horizontal(|ui| {
        ui.label(theme::muted(verb));
        link(ui, env, &parent_name, parent);
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
            let default = o.category.color();
            color_toggle(ui, &mut o.color, default, "Custom");
            ui.end_row();
        });
        ui.label(theme::muted(match verb {
            "In" => "Drag it on the plan to move it, even into another room.",
            _ => "Drag it on the plan to move it.",
        }));
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

// ---------------------------------------------------------------------------
// Roofs

fn roof_layer_editor(ui: &mut Ui, layer: &mut RoofLayer, id: (RoofId, usize), top: bool, today: chrono::NaiveDate) -> bool {
    let mut remove = false;
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), ui.make_persistent_id(("roof-layer", id)), top)
            .show_header(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
                match layer.swatch {
                    Some([r, g, b]) => {
                        ui.painter().rect_filled(rect, 6.0, egui::Color32::from_rgb(r, g, b));
                    }
                    None => {
                        ui.painter()
                            .text(rect.center(), egui::Align2::CENTER_CENTER, icon::STACK_SIMPLE, egui::FontId::proportional(16.0), c::muted());
                    }
                }
                let name = if layer.material.is_empty() { "Layer" } else { &layer.material };
                ui.add(egui::Label::new(theme::strong(name)).truncate());
                if top {
                    theme::chip(ui, "Top", c::forest(), c::mint_soft());
                }
                if let Some(d) = layer.installed {
                    ui.label(theme::muted(d.format("%Y").to_string()));
                }
            })
            .body_unindented(|ui| {
                form_grid(ui, ("roof-layer-grid", id), |ui| {
                    field_label(ui, "Material");
                    suggest_edit(ui, ("roof-layer-mat", id), &mut layer.material, ROOF_MATERIAL_SUGGESTIONS, "e.g. Asphalt shingles");
                    ui.end_row();
                    text_row(ui, "Colour", &mut layer.color, "e.g. Weathered wood");
                    field_label(ui, "Swatch");
                    color_toggle(ui, &mut layer.swatch, [120, 116, 110], "");
                    ui.end_row();
                    field_label(ui, "Installed");
                    opt_date_edit(ui, &format!("rl-inst-{}-{}", id.0, id.1), &mut layer.installed, today);
                    ui.end_row();
                    text_row(ui, "Installed by", &mut layer.installed_by, "Roofing company");
                    field_label(ui, "Warranty until");
                    opt_date_edit(ui, &format!("rl-war-{}-{}", id.0, id.1), &mut layer.warranty_expires, today);
                    ui.end_row();
                    text_row(ui, "Warranty details", &mut layer.warranty_details, "");
                });
                ui.add(
                    egui::TextEdit::multiline(&mut layer.notes)
                        .hint_text("Notes (underlayment, ventilation, why it was left on…)")
                        .desired_rows(1)
                        .desired_width(f32::INFINITY),
                );
                remove = danger_button(ui, "Remove layer");
            });
    });
    remove
}

fn roof_section_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, id: RoofId) {
    let units = project.info.units;
    let today = env.today;
    let Some(li) = project.level_index_of(EntityRef::RoofSection(id)) else { return };
    let level_id = project.levels[li].id;
    let level_name = project.entity_name(EntityRef::Level(level_id));
    let wall_height = project.levels[li].wall_height;
    let items: Vec<(ObjectId, String)> = project.levels[li]
        .objects
        .values()
        .filter(|o| o.roof == Some(id))
        .map(|o| (o.id, o.display_name()))
        .collect();
    let roof = project.levels[li].roofs.get_mut(&id).unwrap();
    heading(
        ui,
        &roof.display_name(),
        Some(format!(
            "{} of roofing · covers {}",
            format_area(roof.surface_area(), units),
            format_area(roof.plan_area(), units)
        )),
    );
    ui.horizontal(|ui| {
        ui.label(theme::muted("On"));
        link(ui, env, &level_name, EntityRef::Level(level_id));
        ui.label(theme::muted("·"));
        link(ui, env, "All roof sections", EntityRef::Roof);
    });
    theme::section(ui, ("roof-shape", id), icon::TRIANGLE, "Shape", None, true, |ui| {
        form_grid(ui, ("roof-shape-grid", id), |ui| {
            text_row(ui, "Name", &mut roof.name, "e.g. Main roof, Porch roof");
            field_label(ui, "Shape");
            let opts: Vec<(RoofShape, &str)> = RoofShape::ALL.iter().map(|s| (*s, s.label())).collect();
            theme::segmented(ui, &mut roof.shape, &opts);
            ui.end_row();
            if roof.shape.is_sloped() {
                field_label(ui, "Pitch");
                ui.horizontal(|ui| {
                    match units {
                        UnitSystem::Imperial => {
                            let mut rise = roof.pitch * 12.0;
                            if ui.add(egui::DragValue::new(&mut rise).range(0.5..=24.0).speed(0.1).fixed_decimals(1).suffix(" in 12")).changed() {
                                roof.pitch = rise / 12.0;
                            }
                        }
                        UnitSystem::Metric => {
                            let mut deg = roof.pitch.atan().to_degrees();
                            if ui.add(egui::DragValue::new(&mut deg).range(2.0..=70.0).speed(0.2).fixed_decimals(1).suffix("°")).changed() {
                                roof.pitch = deg.to_radians().tan();
                            }
                        }
                    }
                    ui.label(theme::muted(format!("rises {}", format_length(roof.rise(), units))));
                });
                ui.end_row();
            }
            if roof.shape.has_direction() {
                field_label(ui, if roof.shape == RoofShape::Shed { "Slopes toward" } else { "Ridge runs" });
                ui.horizontal(|ui| {
                    let mut deg = roof.direction.to_degrees().rem_euclid(360.0);
                    if ui.add(egui::DragValue::new(&mut deg).speed(1.0).fixed_decimals(0).suffix("°")).changed() {
                        roof.direction = deg.rem_euclid(360.0).to_radians();
                    }
                    let tip = if roof.shape == RoofShape::Shed { "Turn the slope 90°" } else { "Turn the ridge 90°" };
                    if theme::icon_button_sized(ui, icon::ARROW_CLOCKWISE, tip, false, 28.0).clicked() {
                        roof.direction = (roof.direction + std::f64::consts::FRAC_PI_2).rem_euclid(std::f64::consts::TAU);
                    }
                });
                ui.end_row();
            }
            field_label(ui, "Overhang");
            length_edit_min(ui, ("roof-overhang", id), &mut roof.overhang, units, 0.0);
            ui.end_row();
            field_label(ui, "Eaves height");
            opt_length_edit(ui, ("roof-eave", id), &mut roof.eave_height, wall_height, units);
            ui.end_row();
        });
        ui.label(theme::muted(format!(
            "Drag the corners on the plan's Roof layer to reshape it. {}",
            if roof.shape.is_sloped() { format!("Pitch {}.", format_pitch(roof.pitch, units)) } else { String::new() }
        )));
    });

    let n = roof.layers.len();
    theme::section(ui, ("roof-layers", id), icon::STACK_SIMPLE, "Layers", Some(n), true, |ui| {
        ui.label(theme::muted("Newest on top. Keep older layers here if they were roofed over rather than torn off."));
        let mut remove = None;
        for i in (0..n).rev() {
            if roof_layer_editor(ui, &mut roof.layers[i], (id, i), i + 1 == n, today) {
                remove = Some(i);
            }
        }
        if let Some(i) = remove {
            roof.layers.remove(i);
        }
        if theme::soft_button(ui, format!("{}  Add a layer on top", icon::PLUS)).clicked() {
            roof.layers.push(RoofLayer { installed: Some(today), ..Default::default() });
        }
    });
    theme::section(ui, ("roof-insp", id), icon::MAGNIFYING_GLASS, "Inspection", None, true, |ui| {
        form_grid(ui, ("roof-insp-grid", id), |ui| {
            field_label(ui, "Last inspected");
            opt_date_edit(ui, &format!("roof-insp-{id}"), &mut roof.last_inspected, today);
            ui.end_row();
        });
    });
    theme::section(ui, ("roof-items", id), icon::SOLAR_PANEL, "On this roof", Some(items.len()), true, |ui| {
        items_list(ui, env, &items, "Add skylights, chimneys, vents, solar panels, gutters and downspouts with the Item tool on the Roof layer.");
    });
    let n = roof.custom_fields.len();
    theme::section(ui, ("roof-custom-s", id), icon::TAG, "Custom fields", Some(n), n > 0, |ui| {
        custom_fields(ui, ("roof-s", id), &mut roof.custom_fields);
    });
    history_sections(ui, project, env, EntityRef::RoofSection(id));
    ui.add_space(6.0);
    if danger_button(ui, "Delete roof section") {
        env.actions.push(Action::Delete(EntityRef::RoofSection(id)));
    }
}

// ---------------------------------------------------------------------------
// Outside

fn feature_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, id: FeatureId) {
    let units = project.info.units;
    let today = env.today;
    let Some(li) = project.level_index_of(EntityRef::Feature(id)) else { return };
    let items: Vec<(ObjectId, String)> = project.levels[li]
        .objects
        .values()
        .filter(|o| o.feature == Some(id))
        .map(|o| (o.id, o.display_name()))
        .collect();
    let house_levels: Vec<(String, f64)> = project.house_levels().filter(|l| l.elevation > 0.1).map(|l| (l.name.clone(), l.elevation)).collect();
    let f = project.levels[li].features.get_mut(&id).unwrap();
    let kind = f.kind;
    let geom = kind.geom();
    let subtitle = match geom {
        FeatureGeom::Area => format!("{} · {} around", format_area(f.area(), units), format_length(f.length(), units)),
        FeatureGeom::Line => format!("{} long", format_length(f.length(), units)),
        FeatureGeom::Point => format!("{} across", format_length(f.size, units)),
    };
    heading(ui, &f.display_name(), Some(subtitle));
    theme::section(ui, ("feature-details", id), theme::feature_icon(kind), "Details", None, true, |ui| {
        form_grid(ui, ("feature-grid", id), |ui| {
            let hint = match kind {
                FeatureKind::Tree => "e.g. Sugar maple",
                FeatureKind::PropertyLine => "e.g. Lot 14",
                _ => "e.g. Back deck",
            };
            text_row(ui, "Name", &mut f.name, hint);
            match geom {
                FeatureGeom::Area if kind != FeatureKind::PropertyLine => {
                    field_label(ui, "Kind");
                    combo(ui, ("feature-kind", id), &mut f.kind, &FeatureKind::AREAS, FeatureKind::label);
                    ui.end_row();
                }
                FeatureGeom::Line => {
                    field_label(ui, "Kind");
                    combo(ui, ("feature-kind", id), &mut f.kind, &FeatureKind::LINES, FeatureKind::label);
                    ui.end_row();
                }
                _ => {}
            }
            let suggestions = f.kind.material_suggestions();
            if !suggestions.is_empty() {
                field_label(ui, if kind == FeatureKind::Tree { "Species" } else { "Material" });
                suggest_edit(ui, ("feature-mat", id), &mut f.material, suggestions, "");
                ui.end_row();
            }
            if let Some(label) = f.kind.height_label() {
                field_label(ui, label);
                ui.vertical(|ui| {
                    length_edit_min(ui, ("feature-h", id), &mut f.height, units, 0.0);
                    if f.kind == FeatureKind::Deck && !house_levels.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            for (name, elev) in &house_levels {
                                if theme::soft_button(ui, format!("Level with {name}")).clicked() {
                                    f.height = *elev;
                                }
                            }
                        });
                    }
                });
                ui.end_row();
            }
            if kind == FeatureKind::Tree {
                field_label(ui, "Canopy");
                length_edit(ui, ("feature-size", id), &mut f.size, units);
                ui.end_row();
                field_label(ui, "Planted");
                opt_date_edit(ui, &format!("tree-planted-{id}"), &mut f.details.installed, today);
                ui.end_row();
            }
            field_label(ui, "Colour");
            let default = f.kind.color();
            color_toggle(ui, &mut f.color, default, "Custom");
            ui.end_row();
        });
        if geom != FeatureGeom::Point {
            ui.label(theme::muted("Drag its corners on the Outside plan to reshape it, or drag it to move it."));
        }
    });

    if kind == FeatureKind::Deck {
        theme::section(ui, ("deck", id), icon::STEPS, "Railing & stairs", None, true, |ui| {
            form_grid(ui, ("deck-grid", id), |ui| {
                field_label(ui, "Railing");
                theme::toggle(ui, &mut f.railing, "");
                ui.end_row();
                if f.railing {
                    text_row(ui, "Railing", &mut f.railing_material, "e.g. Aluminium, glass, wood");
                }
                text_row(ui, "Stairs", &mut f.stairs, "e.g. 4 steps to the lawn");
            });
        });
    }
    if geom == FeatureGeom::Line || kind == FeatureKind::PropertyLine {
        theme::section(ui, ("neighbour", id), icon::USERS, "Neighbours", None, f.shared_with_neighbour, |ui| {
            theme::toggle(ui, &mut f.shared_with_neighbour, "On the lot line / shared");
            if f.shared_with_neighbour {
                ui.add(egui::TextEdit::singleline(&mut f.neighbour).hint_text("Who it's shared with").desired_width(f32::INFINITY));
            }
        });
    }
    if geom == FeatureGeom::Line {
        let n = f.gates.len();
        theme::section(ui, ("gates", id), icon::DOOR_OPEN, "Gates", Some(n), true, |ui| {
            let mut remove = None;
            for (i, g) in f.gates.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut g.label).hint_text("Gate").desired_width(120.0));
                    length_edit_sized(ui, ("gate-w", g.id), &mut g.width, units, 0.3, 80.0);
                    if theme::icon_button_sized(ui, icon::TRASH, "Remove gate", false, 28.0).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                f.gates.remove(i);
            }
            f.clamp_gates();
            ui.label(theme::muted("Add gates with the Gate tool, then drag them along the fence."));
        });
    }
    if kind != FeatureKind::PropertyLine {
        let title = if kind == FeatureKind::Tree { "Details" } else { "Built & warranty" };
        theme::section(ui, ("feature-built", id), icon::RECEIPT, title, None, false, |ui| {
            details_editor(ui, &format!("feat-{id}"), &mut f.details, today);
        });
    }
    if matches!(kind, FeatureKind::Deck | FeatureKind::Fence | FeatureKind::RetainingWall | FeatureKind::Patio | FeatureKind::Pool) {
        let title = if kind == FeatureKind::Deck || kind == FeatureKind::Fence { "Stain & paint" } else { "Paint & sealant" };
        let targets = [(PaintTarget::Feature(id), "whole thing".to_string())];
        paint_section(ui, project, env, ("feature-paint", id), title, &targets, move |t| {
            let mut p = Paint::new(t);
            p.surface = if kind == FeatureKind::Deck { PaintSurface::Decking } else { PaintSurface::Other };
            p.finish = if kind == FeatureKind::Deck || kind == FeatureKind::Fence { PaintFinish::Stain } else { PaintFinish::Sealant };
            p
        });
    }
    if kind.holds_items() {
        theme::section(ui, ("feature-items", id), icon::PACKAGE, "On it", Some(items.len()), true, |ui| {
            items_list(ui, env, &items, "Place a BBQ, hot tub or furniture on it with the Item tool.");
        });
    }
    history_sections(ui, project, env, EntityRef::Feature(id));
    ui.add_space(6.0);
    if danger_button(ui, "Delete") {
        env.actions.push(Action::Delete(EntityRef::Feature(id)));
    }
}

fn building_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, id: BuildingId) {
    let today = env.today;
    let units = project.info.units;
    let levels: Vec<(LevelId, String, usize)> = project.building_levels(id).map(|l| (l.id, l.name.clone(), l.rooms.len())).collect();
    let footprint: f64 = project
        .ground_level(Some(id))
        .map(|l| l.rooms.values().map(|r| area(&l.room_points(r))).sum())
        .unwrap_or(0.0);
    let ground = project.ground_level(Some(id)).map(|l| l.id);
    let Some(b) = project.building_mut(id) else { return };
    heading(ui, &b.display_name(), Some(format!("{} footprint", format_area(footprint, units))));
    theme::section(ui, ("building", id), icon::WAREHOUSE, "Details", None, true, |ui| {
        form_grid(ui, ("building-grid", id), |ui| {
            text_row(ui, "Name", &mut b.name, "e.g. Shed, Garage");
            field_label(ui, "Type");
            suggest_edit(ui, ("building-kind", id), &mut b.kind, BUILDING_KIND_SUGGESTIONS, "e.g. Shed");
            ui.end_row();
        });
        if let Some(l) = ground {
            if theme::primary_button(ui, format!("{}  Open its floor plan", icon::STACK)).clicked() {
                env.actions.push(Action::Reveal(EntityRef::Level(l)));
            }
        }
    });
    theme::section(ui, ("building-built", id), icon::RECEIPT, "Built & warranty", None, true, |ui| {
        details_editor(ui, &format!("bld-{id}"), &mut b.details, today);
    });
    let mut add_level = false;
    theme::section(ui, ("building-levels", id), icon::STACK, "Floors", Some(levels.len()), true, |ui| {
        for (lid, name, rooms) in &levels {
            related_row(ui, env, icon::STACK, name, Some(format!("{rooms} rooms")), EntityRef::Level(*lid));
        }
        add_level = theme::soft_button(ui, format!("{}  Add a floor (loft, upstairs)", icon::PLUS)).clicked();
    });
    if add_level {
        let lid = project.add_level(Some(id), "Loft");
        env.actions.push(Action::Reveal(EntityRef::Level(lid)));
    }
    let targets = [(PaintTarget::Building(id), "outside".to_string())];
    paint_section(ui, project, env, ("building-paint", id), "Exterior paint", &targets, |t| {
        let mut p = Paint::new(t);
        p.surface = PaintSurface::Siding;
        p
    });
    history_sections(ui, project, env, EntityRef::Building(id));
    ui.add_space(6.0);
    if danger_button(ui, "Delete building…") {
        env.dialogs.delete = Some(EntityRef::Building(id));
    }
}

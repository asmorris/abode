//! Right-hand floating panel: edit whatever is selected.

use super::hardware::{Fits, hardware_section};
use super::notes::history_sections;
use super::widgets::*;
use super::{Action, Env};
use crate::theme::{self, c, icon};
use abode_core::geometry::{arc_sagitta, area, bulge_for_sagitta, perimeter};
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
            .map(|(_, o)| if o.is_stairs() { "Stairs".to_string() } else { o.category_label() })
            .unwrap_or_default(),
        EntityRef::RoofSection(_) => "Roof section".into(),
        EntityRef::Feature(id) => project
            .find_feature(id)
            .map(|(_, f)| format!("Outside · {}", f.kind_label()))
            .unwrap_or_default(),
        EntityRef::Building(id) => project
            .building(id)
            .map(|b| if b.kind.trim().is_empty() { "Outbuilding".to_string() } else { b.kind.clone() })
            .unwrap_or_default(),
        EntityRef::Planting(id) => project
            .planting(id)
            .map(|p| format!("Garden · {}", p.layout.label()))
            .unwrap_or_default(),
        EntityRef::Pet(_) => "Pet".into(),
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
                EntityRef::Planting(id) => super::garden::planting_ui(ui, project, env, id),
                // Pets have their own page (see pets.rs), not the inspector.
                EntityRef::Pet(_) => *env.selection = None,
            }
            ui.add_space(12.0);
        });
}

pub(super) fn heading(ui: &mut Ui, title: &str, subtitle: Option<String>) {
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
pub(super) fn related_row(ui: &mut Ui, env: &mut Env, i: &str, text: &str, detail: Option<String>, target: EntityRef) {
    // Detail sits on the right when it fits beside the name, otherwise on
    // its own line underneath so long materials never overlap the link.
    let mut below = None;
    let indent = ui
        .horizontal(|ui| {
            let icon = ui.label(RichText::new(i).color(c::muted()));
            link(ui, env, text, target);
            if let Some(d) = detail {
                let width = egui::WidgetText::from(theme::muted(d.as_str()))
                    .into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body)
                    .size()
                    .x;
                if width + ui.spacing().item_spacing.x * 2.0 <= ui.available_width() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(theme::muted(d));
                    });
                } else {
                    below = Some(d);
                }
            }
            icon.rect.width() + ui.spacing().item_spacing.x
        })
        .inner;
    if let Some(d) = below {
        // Tuck it right under the name (rows are 30px tall, the link text isn't).
        ui.add_space(-ui.spacing().item_spacing.y - 6.0);
        egui::Frame::NONE.inner_margin(egui::Margin { left: indent as i8, ..Default::default() }).show(ui, |ui| {
            ui.add(egui::Label::new(theme::muted(d)).wrap());
        });
    }
}

/// Optional custom colour: a switch plus a colour button.
pub(super) fn color_toggle(ui: &mut Ui, color: &mut Option<[u8; 3]>, default: [u8; 3], label: &str) {
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
    let (floor_area, floors) = project.house_floor_area();
    let floors: Vec<String> = if floors.len() > 1 {
        floors.iter().map(|(l, a)| format!("{} {}", project.entity_name(EntityRef::Level(*l)), format_area(*a, units))).collect()
    } else {
        Vec::new()
    };
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
            field_label(ui, "Floor area");
            let total = |ui: &mut Ui| {
                let text = if floor_area > 0.0 { format_area(floor_area, units) } else { "No rooms yet".into() };
                ui.label(text).on_hover_text(
                    "All the rooms on the house's floors, measured to the middle of the walls (as on the plan). Outbuildings and outside aren't included.",
                );
            };
            if floors.is_empty() {
                total(ui);
            } else {
                // Several floors: the total, then each floor underneath.
                ui.vertical(|ui| {
                    total(ui);
                    for f in &floors {
                        ui.label(theme::muted(f));
                    }
                });
            }
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
    vehicles_section(ui, project, env);
    history_sections(ui, project, env, EntityRef::General);
}

/// Every vehicle and where it's parked, with a pointer to adding one.
fn vehicles_section(ui: &mut Ui, project: &Project, env: &mut Env) {
    let cars = project.vehicles();
    let units = project.info.units;
    theme::section(ui, "house-vehicles", icon::CAR, "Vehicles", Some(cars.len()), !cars.is_empty(), |ui| {
        for (li, id) in &cars {
            let level = &project.levels[*li];
            let o = &level.objects[id];
            let place = match (o.feature, o.room) {
                (Some(f), _) => project.entity_name(EntityRef::Feature(f)),
                (_, Some(r)) => project.entity_name(EntityRef::Room(r)),
                _ => project.entity_name(EntityRef::Level(level.id)),
            };
            let detail = match project.odometer(*id) {
                Some(k) => format!("{place} · {}", vehicle::format_distance(k, units)),
                None => place,
            };
            related_row(ui, env, theme::object_icon(o), &o.display_name(), Some(detail), EntityRef::Object(*id));
        }
        ui.label(theme::muted(format!(
            "Add a car, truck, motorcycle or trailer with {} Item: on the Outside plan (driveway or carport), or on a garage's floor plan.",
            icon::PACKAGE
        )));
    });
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
    let (below, above) = project.neighbour_levels(id);
    let (below, above) = (below.map(|l| l.name.clone()), above.map(|l| l.name.clone()));
    let mut moved: Option<bool> = None;
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
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                field_label(ui, "In the stack");
                let place = match (&above, &below) {
                    (Some(a), Some(b)) => format!("Between {b} and {a}"),
                    (None, Some(b)) => format!("Top floor, above {b}"),
                    (Some(a), None) => format!("Bottom floor, below {a}"),
                    (None, None) => "The only floor".to_string(),
                };
                ui.label(place);
            });
            if above.is_some() || below.is_some() {
                ui.horizontal(|ui| {
                    if ui.add_enabled_ui(above.is_some(), |ui| theme::secondary_button(ui, format!("{}  Move up", icon::ARROW_UP))).inner.clicked() {
                        moved = Some(true);
                    }
                    if ui.add_enabled_ui(below.is_some(), |ui| theme::secondary_button(ui, format!("{}  Move down", icon::ARROW_DOWN))).inner.clicked() {
                        moved = Some(false);
                    }
                });
                ui.label(theme::muted("Moving a floor restacks the others around it. The main floor stays put, so a floor moved below it becomes a basement."));
            }
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

    if let Some(up) = moved {
        project.move_level(id, up);
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
                    ui.label(theme::muted(format!("{:.0}%", u.opacity * 100.0)));
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
                combo_named(ui, ("paint-surface", id), &mut paint.surface, &mut paint.surface_name, &PaintSurface::ALL, PaintSurface::label, PaintSurface::Other);
                ui.end_row();
                text_row(ui, "Brand", &mut paint.brand, "e.g. Benjamin Moore");
                text_row(ui, "Colour name", &mut paint.color_name, "e.g. Pale Oak");
                text_row(ui, "Colour code", &mut paint.color_code, "e.g. OC-20");
                field_label(ui, "Finish");
                combo_named(ui, ("paint-finish", id), &mut paint.finish, &mut paint.finish_name, &PaintFinish::ALL, PaintFinish::label, PaintFinish::Other);
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

/// Paint list + add buttons for the given targets. `surfaces`: for a room,
/// the surfaces its whole-room paint can be for (walls, ceiling...).
#[allow(clippy::too_many_arguments)]
fn paint_section(
    ui: &mut Ui,
    project: &mut Project,
    env: &mut Env,
    id_salt: impl std::hash::Hash,
    title: &str,
    targets: &[(PaintTarget, String)],
    surfaces: &[PaintSurface],
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
            if let Some((t, label)) = targets.iter().find(|(t, _)| *t == paint.target) {
                // Whole-room paint is told apart by surface: walls, ceiling, trim.
                let label = match (t, paint.surface) {
                    (PaintTarget::Room(_), PaintSurface::Walls) => "All walls".to_string(),
                    (PaintTarget::Room(_), PaintSurface::Other) if !paint.surface_name.trim().is_empty() => paint.surface_name.trim().to_string(),
                    (PaintTarget::Room(_), s) => s.label().to_string(),
                    _ => label.clone(),
                };
                paint_editor(ui, paint, &label, env.today, &mut delete);
            }
        }
        if let Some(id) = delete {
            project.paints.retain(|p| p.id != id);
        }
        match (targets, surfaces) {
            ([(t, label)], []) => {
                if theme::soft_button(ui, format!("{}  Add ({label})", icon::PLUS)).clicked() {
                    project.paints.push(new_paint(*t));
                }
            }
            _ => {
                menu_dropdown(ui, RichText::new(format!("{}  Add paint", icon::PLUS)).color(c::forest()), |ui| {
                    ui.set_min_width(240.0);
                    let mut picked = None;
                    // The first target (a whole room) by surface: walls, ceiling, trim...
                    if let (Some((t, _)), false) = (targets.first(), surfaces.is_empty()) {
                        ui.label(theme::caps("Whole room"));
                        for s in surfaces {
                            let text = if *s == PaintSurface::Walls { "All walls" } else { s.label() };
                            if ui.selectable_label(false, text).clicked() {
                                picked = Some(Paint::for_surface(*t, *s));
                            }
                        }
                    }
                    let rest = if surfaces.is_empty() { targets } else { &targets[1..] };
                    if !rest.is_empty() {
                        if !surfaces.is_empty() {
                            ui.add_space(4.0);
                            ui.label(theme::caps("One wall"));
                        }
                        for (t, label) in rest {
                            if ui.selectable_label(false, label).clicked() {
                                picked = Some(new_paint(*t));
                            }
                        }
                    }
                    if let Some(p) = picked {
                        project.paints.push(p);
                        close_dropdown(ui);
                    }
                });
            }
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
        let pts = level.room_outline(room);
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
                ui.label(RichText::new("Unlocked").color(c::teal()))
                    .on_hover_text("Drag corners on the plan, or type sizes below");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::secondary_button(ui, "Lock").clicked() {
                        project.levels[li].rooms.get_mut(&id).unwrap().shape_locked = true;
                    }
                    if theme::soft_button(ui, "Straighten")
                        .on_hover_text(
                            "Line corners up exactly with nearby rooms and square up walls that are a little off (within 2\"). Locked rooms don't change.",
                        )
                        .clicked()
                    {
                        let level_id = project.levels[li].id;
                        match project.straighten_room(level_id, id) {
                            Ok(()) => env.toasts.info("Straightened."),
                            Err(e) => env.toasts.error(e.to_string()),
                        }
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
                let level_id = project.levels[li].id;
                if let Err(e) = project.set_room_size(level_id, id, w2, d2) {
                    env.toasts.error(e.to_string());
                }
            }
        } else if let Some((_, w, d)) = dims {
            ui.label(theme::strong(format!("{} × {}", format_length(w, units), format_length(d, units))));
        }
        ui.add_space(4.0);
        ui.label(theme::caps("Walls"));
        for (wid, name, len) in &walls {
            if locked {
                related_row(ui, env, icon::LINE_SEGMENT, name, Some(format_length(*len, units)), EntityRef::Wall(*wid));
                continue;
            }
            let mut new_len = *len;
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::LINE_SEGMENT).color(c::muted()));
                link(ui, env, name, EntityRef::Wall(*wid));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    changed = length_edit_sized(ui, ("room-wall-len", id, *wid), &mut new_len, units, 0.1, 96.0);
                });
            });
            if changed {
                let level_id = project.levels[li].id;
                if let Err(e) = project.set_wall_length(level_id, id, *wid, new_len) {
                    env.toasts.error(e.to_string());
                }
            }
        }
        // Corners partway along a straight wall (or a curve) only split it up.
        // (Counted by trying it on a copy.)
        let extra = project.levels[li].clone().remove_extra_corners(id).0;
        if extra > 0 {
            let label = if extra == 1 { "Remove the extra corner".to_string() } else { format!("Remove {extra} extra corners") };
            if theme::secondary_button(ui, format!("{}  {label}", icon::MINUS))
                .on_hover_text("Corners partway along a straight wall split it into pieces without changing the room's shape. This joins them back into one wall.")
                .clicked()
            {
                let level_id = project.levels[li].id;
                let n = project.remove_extra_corners(level_id, id);
                env.toasts.info(format!("Joined up {n} wall{}.", if n == 1 { "" } else { "s" }));
            }
        }
    });

    let mut targets = vec![(PaintTarget::Room(id), "whole room".to_string())];
    for (wid, name, _) in &walls {
        targets.push((PaintTarget::WallFace { wall: *wid, room: Some(id) }, name.clone()));
    }
    let surfaces = [PaintSurface::Walls, PaintSurface::Ceiling, PaintSurface::Trim, PaintSurface::Doors];
    paint_section(ui, project, env, ("room-paint", id), "Paint", &targets, &surfaces, Paint::new);

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
    hardware_section(ui, ("room", id), &mut room.hardware, Fits::Room);
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
        let rooms: Vec<(RoomId, String, bool)> = level
            .rooms_of_wall(wall)
            .into_iter()
            .map(|r| (r, level.rooms[&r].display_name(), level.rooms[&r].shape_locked))
            .collect();
        let openings: Vec<(OpeningId, String, bool)> = level
            .openings
            .values()
            .filter(|o| o.wall == id)
            .map(|o| (o.id, project.entity_name(EntityRef::Opening(o.id)), o.kind.is_door()))
            .collect();
        (level.wall_length(wall), level.wall_height, rooms, openings)
    };
    // Curve: how deep, and which room it bulges into (None = outside).
    let (chord, sagitta, first_side) = {
        let level = &project.levels[li];
        let wall = &level.walls[&id];
        let (a, b) = level.wall_points(wall).unwrap_or_default();
        let side = rooms.first().and_then(|r| level.wall_side_of(wall, r.0)).unwrap_or(1.0);
        (a.distance(b), arc_sagitta(a, b, wall.bulge), side)
    };
    let title = project.entity_name(EntityRef::Wall(id));
    heading(ui, &title, Some(format!("{} long", format_length(len, units))));
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::muted("Between"));
        for (i, (rid, name, _)) in rooms.iter().enumerate() {
            if i > 0 {
                ui.label(theme::muted("and"));
            }
            link(ui, env, name, EntityRef::Room(*rid));
        }
        if rooms.len() == 1 {
            ui.label(theme::muted("and the outside"));
        }
    });
    // Length can change only while one of its rooms is unlocked.
    let unlocked = rooms.iter().find(|r| !r.2).map(|r| r.0);
    let locked_room = rooms.first().map(|r| r.1.clone()).unwrap_or_default();
    // Bending reshapes every room the wall is in, so they must all be unlocked.
    let bend_locked = rooms.iter().find(|r| r.2).map(|r| r.1.clone());
    let mut new_len = None;
    let mut new_sagitta = None;
    let mut add_corner = false;
    let mut remove_corner = None;
    // An end where this wall just carries on into the next one: a corner
    // that can go without changing the room's shape.
    let extra_end = {
        let level = &project.levels[li];
        let wall = &level.walls[&id];
        [wall.a, wall.b].into_iter().find(|n| level.is_extra_corner(*n))
    };
    let wall = project.levels[li].walls.get_mut(&id).unwrap();
    theme::section(ui, ("wall-details", id), icon::LINE_SEGMENT, "Details", None, true, |ui| {
        form_grid(ui, ("wall-grid", id), |ui| {
            text_row(ui, "Label", &mut wall.label, "e.g. North wall, Accent wall");
            field_label(ui, "Length");
            if unlocked.is_some() {
                let mut l = len;
                if length_edit_min(ui, ("wall-len", id), &mut l, units, 0.1) {
                    new_len = Some(l);
                }
            } else {
                ui.horizontal(|ui| {
                    ui.label(format_length(len, units));
                    ui.label(RichText::new(icon::LOCK).color(c::muted()))
                        .on_hover_text(format!("Unlock {locked_room}'s shape to change this"));
                });
            }
            ui.end_row();
            field_label(ui, "Curve");
            ui.horizontal(|ui| match &bend_locked {
                None => {
                    let mut depth = sagitta.abs();
                    if length_edit_min(ui, ("wall-curve", id), &mut depth, units, 0.0) {
                        let sign = if sagitta == 0.0 { -first_side } else { sagitta.signum() };
                        new_sagitta = Some(sign * depth.min(chord / 2.0));
                    }
                    if sagitta != 0.0 {
                        // Which way it bulges, named by the room it bulges into.
                        let into_first = sagitta * first_side > 0.0;
                        let mut choice = into_first;
                        let other = rooms.get(1).map(|r| r.1.as_str()).unwrap_or("Out");
                        let first = if rooms.len() > 1 { rooms[0].1.as_str() } else { "In" };
                        theme::segmented(ui, &mut choice, &[(false, other), (true, first)]);
                        if choice != into_first {
                            new_sagitta = Some(-sagitta);
                        }
                    }
                }
                Some(name) => {
                    ui.label(if sagitta == 0.0 { "Straight".to_string() } else { format_length(sagitta.abs(), units) });
                    ui.label(RichText::new(icon::LOCK).color(c::muted()))
                        .on_hover_text(format!("Unlock {name}'s shape to curve this wall"));
                }
            });
            ui.end_row();
            field_label(ui, "Thickness");
            length_edit(ui, ("wall-t", id), &mut wall.thickness, units);
            ui.end_row();
            field_label(ui, "Height");
            opt_length_edit(ui, ("wall-h", id), &mut wall.height, level_height, units);
            ui.end_row();
        });
        ui.horizontal(|ui| {
            add_corner = theme::secondary_button(ui, format!("{}  Add a corner", icon::PLUS))
                .on_hover_text("Split this wall in two, so you can move, curve or straighten each part. Or double-click the wall on the plan.")
                .clicked();
            if let Some(n) = extra_end {
                if theme::secondary_button(ui, format!("{}  Remove corner", icon::MINUS))
                    .on_hover_text("Join this wall with the one it carries straight on into (or that continues its curve), so they're one wall again.")
                    .clicked()
                {
                    remove_corner = Some(n);
                }
            }
        });
        ui.label(theme::muted(
            "Walls are shared: a wall between two rooms is the same wall in both. To curve a wall, set how deep the curve is, or drag the round handle in its middle on the plan. Double-click a wall to add a corner.",
        ));
    });
    if let (Some(l), Some(room)) = (new_len, unlocked) {
        if let Err(e) = project.set_wall_length(level_id, room, id, l) {
            env.toasts.error(e.to_string());
        }
    }
    if add_corner {
        match project.add_corner(level_id, id, len / 2.0) {
            Ok(_) => {
                if let Some(r) = unlocked {
                    env.select(EntityRef::Room(r));
                    env.toasts.info("Added a corner in the middle of the wall. Drag it to reshape the room.");
                } else {
                    env.toasts.info(format!("Added a corner in the middle of the wall. Unlock {locked_room}'s shape to drag it."));
                }
            }
            Err(e) => env.toasts.error(format!("Couldn't add a corner: {e}")),
        }
    }
    if let Some(n) = remove_corner {
        // This wall may be the one that goes; the joined-up wall is the other.
        let joined = {
            let level = &project.levels[li];
            level.walls.values().find(|w| w.id != id && (w.a == n || w.b == n)).map(|w| w.id)
        };
        match project.remove_corner(level_id, n) {
            Ok(()) => {
                let keep = if project.levels[li].walls.contains_key(&id) { Some(id) } else { joined };
                if let Some(w) = keep.filter(|w| project.levels[li].walls.contains_key(w)) {
                    env.select(EntityRef::Wall(w));
                }
            }
            Err(e) => env.toasts.error(format!("Couldn't remove the corner: {e}")),
        }
        // This wall may have been joined into the other; the joined wall shows next frame.
        if !project.levels[li].walls.contains_key(&id) {
            return;
        }
    }
    if let Some(sag) = new_sagitta {
        let (a, b) = project.levels[li].wall_points(&project.levels[li].walls[&id]).unwrap_or_default();
        if let Err(e) = project.set_wall_bulge(level_id, id, bulge_for_sagitta(a, b, sag)) {
            env.toasts.error(format!("Can't curve it that far: {e}"));
        }
    }

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
        .map(|(r, name, _)| (PaintTarget::WallFace { wall: id, room: Some(*r) }, format!("{name} side")))
        .collect();
    if rooms.len() == 1 {
        targets.push((PaintTarget::WallFace { wall: id, room: None }, "outside".into()));
    }
    paint_section(ui, project, env, ("wall-paint", id), "Paint", &targets, &[], Paint::new);

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
                OpeningKind::Door(s) => combo_named(ui, ("door-style", id), s, &mut o.style_name, &DoorStyle::ALL, DoorStyle::label, DoorStyle::Other),
                OpeningKind::Window(s) => combo_named(ui, ("window-style", id), s, &mut o.style_name, &WindowStyle::ALL, WindowStyle::label, WindowStyle::Other),
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
    let fits = if o.kind.is_door() { Fits::Door } else { Fits::Window };
    hardware_section(ui, ("op", id), &mut o.details.hardware, fits);
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
    let level_id = project.levels[li].id;
    let stair = project.stair_info(level_id, &project.levels[li].objects[&id]);
    let stair_to = stair.as_ref().and_then(|s| s.to).map(|l| (l, project.entity_name(EntityRef::Level(l))));
    let odometer = project.odometer(id);
    let vehicle_cx = super::vehicle::VehicleCtx {
        units,
        odometer,
        chargers: project.ev_chargers(),
        has_swaps: project.has_tire_swap_reminders(id),
    };
    let subtitle = project.vehicle(id).map(|v| {
        let mut bits: Vec<String> = Vec::new();
        if !v.plate.trim().is_empty() {
            bits.push(v.plate.trim().to_string());
        }
        if let Some(k) = odometer {
            bits.push(vehicle::format_distance(k, units));
        }
        if bits.is_empty() { "Vehicle".to_string() } else { bits.join("  ·  ") }
    });
    let subtitle = subtitle.or_else(|| {
        let s = stair.as_ref()?;
        let dir = project.levels[li].objects[&id].stairs.as_ref()?.direction;
        Some(match &stair_to {
            Some((_, to)) => format!("{} steps · {} to {to}", s.steps, dir.label()),
            None => format!("{} steps", s.steps),
        })
    });
    heading(ui, &name, subtitle);
    ui.horizontal(|ui| {
        ui.label(theme::muted(verb));
        link(ui, env, &parent_name, parent);
    });
    let o = project.levels[li].objects.get_mut(&id).unwrap();
    theme::section(ui, ("obj-main", id), icon::PACKAGE, "Details", None, true, |ui| {
        form_grid(ui, ("obj-grid", id), |ui| {
            if o.category == ObjectCategory::Vehicle {
                text_row(ui, "Nickname", &mut o.label, "Optional, e.g. Sam's car");
            } else {
                text_row(ui, "Label", &mut o.label, "e.g. Fridge, Furnace");
            }
            field_label(ui, "Category");
            combo_named(ui, ("obj-cat", id), &mut o.category, &mut o.category_name, &ObjectCategory::ALL, ObjectCategory::label, ObjectCategory::Other);
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
    if o.category == ObjectCategory::Vehicle {
        let asked = super::vehicle::vehicle_sections(ui, o, env, &vehicle_cx);
        if asked.add_tire_swaps {
            project.add_tire_swap_reminders(id, today);
            env.toasts.info("Added yearly reminders to swap tires in November and April.");
        }
        history_sections(ui, project, env, EntityRef::Object(id));
        ui.add_space(6.0);
        if danger_button(ui, "Delete vehicle") {
            env.actions.push(Action::Delete(EntityRef::Object(id)));
        }
        return;
    } else {
        if let (Some(st), Some(info)) = (o.stairs.as_mut(), stair.as_ref()) {
            let mut go = None;
            theme::section(ui, ("obj-stairs", id), icon::STEPS, "Stairs", None, true, |ui| {
                form_grid(ui, ("stairs-grid", id), |ui| {
                    field_label(ui, "Goes");
                    theme::segmented(ui, &mut st.direction, &[(stairs::StairDirection::Up, "Up"), (stairs::StairDirection::Down, "Down")]);
                    ui.end_row();
                    field_label(ui, "To");
                    match &stair_to {
                        Some((l, name)) => {
                            ui.horizontal(|ui| {
                                ui.label(name);
                                if theme::soft_button(ui, format!("{}  Go there", icon::ARROW_RIGHT)).clicked() {
                                    go = Some(*l);
                                }
                            });
                        }
                        None => {
                            let text = match (info.rise, st.direction) {
                                (Some(_), _) => "The deck",
                                (None, stairs::StairDirection::Up) => "No floor above yet",
                                (None, stairs::StairDirection::Down) => "No floor below yet",
                            };
                            ui.label(theme::muted(text));
                        }
                    }
                    ui.end_row();
                    field_label(ui, "Steps");
                    ui.horizontal(|ui| match &mut st.steps {
                        Some(n) => {
                            ui.add(egui::DragValue::new(n).range(1..=60));
                            if theme::icon_button_sized(ui, icon::X, "Work it out from the height", false, 26.0).clicked() {
                                st.steps = None;
                            }
                        }
                        None => {
                            ui.label(theme::muted(format!("about {}", info.steps)));
                            if theme::soft_button(ui, "Set").clicked() {
                                st.steps = Some(info.steps);
                            }
                        }
                    });
                    ui.end_row();
                });
                let run = o.shape.depth / info.steps.max(1) as f64;
                let line = match info.rise {
                    Some(r) => {
                        let rise = r / info.steps.max(1) as f64;
                        let steep = if rise > 0.2 { " That's steep: steps are usually no more than about 20 cm high." } else { "" };
                        format!("Each step is about {} high and {} deep.{steep}", format_length(rise, units), format_length(run, units))
                    }
                    None => format!("Each step is about {} deep.", format_length(run, units)),
                };
                ui.add(egui::Label::new(theme::muted(line)).wrap());
                ui.horizontal(|ui| {
                    if theme::soft_button(ui, format!("{}  Turn around", icon::ARROWS_CLOCKWISE)).clicked() {
                        o.rotation = (o.rotation + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU);
                    }
                });
                ui.add(egui::Label::new(theme::muted("The arrow shows the way from this floor. Double-click the stairs on the plan to jump to the other floor.")).wrap());
            });
            if let Some(l) = go {
                *env.active_level = l;
            }
        }
        theme::section(ui, ("obj-details", id), icon::RECEIPT, "Purchase & warranty", None, true, |ui| {
            details_editor(ui, &format!("obj-{id}"), &mut o.details, today);
        });
        hardware_section(ui, ("obj", id), &mut o.details.hardware, Fits::Item);
    }
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

/// Names for an outline's sides ("Top", "Left"...) by where they sit around
/// its middle on the plan, numbered when several face the same way. A line's
/// segments are just numbered.
fn side_names(sides: &[(Vec2, Vec2)], closed: bool) -> Vec<String> {
    if !closed {
        return (1..=sides.len()).map(|i| format!("Section {i}")).collect();
    }
    let n = sides.len() as f64;
    let mid = sides.iter().fold(Vec2::ZERO, |acc, (a, _)| acc + *a) * (1.0 / n);
    let base: Vec<&str> = sides
        .iter()
        .map(|(a, b)| {
            let d = *b - *a;
            let m = (*a + *b) * 0.5;
            // The plan's y grows downwards.
            if d.x.abs() >= d.y.abs() {
                if m.y < mid.y { "Top" } else { "Bottom" }
            } else if m.x < mid.x {
                "Left"
            } else {
                "Right"
            }
        })
        .collect();
    base.iter()
        .enumerate()
        .map(|(i, name)| {
            let same = base.iter().filter(|b| *b == name).count();
            if same > 1 {
                let k = base[..=i].iter().filter(|b| *b == name).count();
                format!("{name} {k}")
            } else {
                (*name).to_string()
            }
        })
        .collect()
}

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
                    combo_named(ui, ("feature-kind", id), &mut f.kind, &mut f.kind_name, &FeatureKind::AREAS, FeatureKind::label, FeatureKind::Other);
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
    let sides = f.sides();
    if kind.is_garden() {
        let in_plan = env.garden_open == Some(id);
        super::garden::garden_sections(ui, project, env, id, in_plan);
    }
    let Some(f) = project.levels[li].features.get_mut(&id) else { return };
    if !sides.is_empty() {
        let names = side_names(&sides, geom == FeatureGeom::Area);
        let title = if geom == FeatureGeom::Area { "Sides" } else { "Sections" };
        theme::section(ui, ("feature-sides", id), icon::RULER, title, Some(sides.len()), true, |ui| {
            for (i, ((a, b), name)) in sides.iter().zip(&names).enumerate() {
                let mut len = a.distance(*b);
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon::LINE_SEGMENT).color(c::muted()));
                    ui.label(name);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        changed = length_edit_sized(ui, ("feature-side", id, i), &mut len, units, 0.1, 96.0);
                    });
                });
                if changed && !f.set_side_length(i, len) {
                    env.toasts.error("That side is too short.");
                }
            }
            ui.label(theme::muted(if geom == FeatureGeom::Area {
                "Changing a side moves the next corner with it, so a rectangle stays a rectangle."
            } else {
                "Changing a section slides the rest of the line along with it."
            }));
        });
    }

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
        if matches!(kind, FeatureKind::Fence | FeatureKind::Deck | FeatureKind::Carport | FeatureKind::Pool) {
            hardware_section(ui, ("feat", id), &mut f.details.hardware, Fits::Outside);
        }
    }
    if matches!(kind, FeatureKind::Deck | FeatureKind::Fence | FeatureKind::RetainingWall | FeatureKind::Patio | FeatureKind::Pool) {
        let title = if kind == FeatureKind::Deck || kind == FeatureKind::Fence { "Stain & paint" } else { "Paint & sealant" };
        let targets = [(PaintTarget::Feature(id), "whole thing".to_string())];
        paint_section(ui, project, env, ("feature-paint", id), title, &targets, &[], move |t| {
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
    let levels: Vec<(LevelId, String, usize)> = project.floor_stack(Some(id)).into_iter().rev().map(|l| (l.id, l.name.clone(), l.rooms.len())).collect();
    let footprint: f64 = project
        .ground_level(Some(id))
        .map(|l| l.rooms.values().map(|r| area(&l.room_outline(r))).sum())
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
    hardware_section(ui, ("bld", id), &mut b.details.hardware, Fits::Outside);
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
    paint_section(ui, project, env, ("building-paint", id), "Exterior paint", &targets, &[], |t| {
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

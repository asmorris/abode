//! Gardens in the inspector: a planting's details, schedule and harvests;
//! and for a garden bed, its season at a glance, frost dates and seeds.

use super::inspector::{color_toggle, heading, related_row};
use super::notes::{notes_section, reminders_section};
use super::widgets::*;
use super::{Action, Env};
use crate::theme::{self, c, icon};
use abode_core::garden::{self as g, CropKind, Family, HARVEST_UNITS, Hint, Stage, Status, Step};
use abode_core::units::{format_area, format_length};
use abode_core::*;
use chrono::{Datelike, NaiveDate};
use eframe::egui::{self, Align2, FontId, RichText, Sense, Stroke, Ui, vec2};

// ---------------------------------------------------------------------------
// Shared bits

fn year_id() -> egui::Id {
    egui::Id::new("garden-year")
}

/// The garden year being looked at (the plan and the inspector share it).
pub fn year(ctx: &egui::Context, today: NaiveDate) -> i32 {
    ctx.data(|d| d.get_temp::<i32>(year_id())).unwrap_or(today.year())
}

pub fn set_year(ctx: &egui::Context, year: i32) {
    ctx.data_mut(|d| d.insert_temp(year_id(), year));
}

/// A colour for a crop: by family, with a few flowers singled out.
pub fn crop_color(name: &str) -> [u8; 3] {
    let Some(c) = g::crop(name) else { return [118, 150, 98] };
    match c.name {
        "Marigold" | "Nasturtium" => return [232, 140, 46],
        "Sunflower" => return [230, 190, 50],
        "Zinnia" => return [222, 90, 140],
        "Sweet pea" => return [170, 130, 210],
        "Lettuce" => return [128, 186, 84],
        "Blueberry" => return [86, 104, 170],
        _ => {}
    }
    match c.family {
        Family::Nightshade => [210, 82, 64],
        Family::Brassica => [88, 140, 126],
        Family::Legume => [112, 160, 70],
        Family::Allium => [168, 128, 190],
        Family::Cucurbit => [214, 170, 56],
        Family::Umbellifer => [228, 128, 52],
        Family::Amaranth => [170, 62, 96],
        Family::Aster => [140, 190, 90],
        Family::Mint => [86, 150, 118],
        Family::Grass => [210, 186, 80],
        Family::Rose => [200, 60, 84],
        Family::Other => [118, 150, 98],
    }
}

pub fn crop_icon(name: &str) -> &'static str {
    match g::crop(name).map(|c| (c.kind, c.family)) {
        Some((CropKind::Flower, _)) => icon::FLOWER,
        Some((CropKind::Fruit, _)) => icon::ORANGE,
        Some((CropKind::Herb, _)) => icon::LEAF,
        Some((_, Family::Umbellifer)) => icon::CARROT,
        Some((_, Family::Nightshade)) => icon::PEPPER,
        Some((_, Family::Grass)) => icon::GRAINS,
        _ => icon::PLANT,
    }
}

/// Pick a crop: type to search the library, or type any plant at all.
pub fn crop_combo(ui: &mut Ui, id: impl std::hash::Hash, crop: &mut String) {
    let text = if crop.trim().is_empty() { "Choose…".to_string() } else { format!("{}  {crop}", crop_icon(crop)) };
    let id = ui.make_persistent_id(id);
    let search_id = id.with("search");
    let seen_id = id.with("seen");
    let frame = ui.ctx().cumulative_pass_nr();
    // Screenshots: ABODE_CROP_SEARCH=<text> opens the first crop picker with that typed in.
    if let (Ok(q), true) = (std::env::var("ABODE_CROP_SEARCH"), frame < 4) {
        let popup = ui.make_persistent_id(egui::Id::new(id)).with("popup");
        ui.memory_mut(|m| m.open_popup(popup));
        ui.data_mut(|d| {
            d.insert_temp(search_id, q);
            d.insert_temp(seen_id, frame);
        });
    }
    theme::combo_box(id)
        .selected_text(text)
        .height(600.0)
        .width(220.0)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            // Just opened: start with an empty search box, ready to type in.
            let last = ui.data(|d| d.get_temp::<u64>(seen_id));
            let opened = last.is_none_or(|f| f + 1 < frame);
            ui.data_mut(|d| d.insert_temp(seen_id, frame));
            let mut query: String = if opened { String::new() } else { ui.data(|d| d.get_temp(search_id)).unwrap_or_default() };
            let edit = search_field(ui, &mut query, "Search or type");
            if opened {
                edit.request_focus();
            }
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.data_mut(|d| d.insert_temp(search_id, query.clone()));
            let typed = query.trim();
            let found = g::search_crops(typed);
            let exact = g::crop(typed).is_some();
            let mut pick: Option<String> = None;
            if enter {
                pick = found.first().map(|c| c.name.to_string()).or_else(|| (!typed.is_empty()).then(|| typed.to_string()));
            }
            let use_typed = |ui: &mut Ui, pick: &mut Option<String>| {
                if !typed.is_empty() && !exact {
                    let label = format!("{}  Use “{typed}”", icon::PLUS);
                    if ui.selectable_label(false, RichText::new(label).color(c::forest())).clicked() {
                        *pick = Some(typed.to_string());
                    }
                }
            };
            if found.is_empty() {
                use_typed(ui, &mut pick);
            }
            egui::ScrollArea::vertical().max_height(340.0).min_scrolled_height(five_rows(ui)).auto_shrink([false, true]).show(ui, |ui| {
                for kind in CropKind::ALL {
                    let in_kind: Vec<_> = found.iter().filter(|c| c.kind == kind).collect();
                    if in_kind.is_empty() {
                        continue;
                    }
                    ui.label(theme::caps(kind.label()));
                    for c in in_kind {
                        let label = format!("{}  {}", crop_icon(c.name), c.name);
                        if ui.selectable_label(crop.eq_ignore_ascii_case(c.name), label).clicked() {
                            pick = Some(c.name.to_string());
                        }
                    }
                }
                if found.is_empty() && !typed.is_empty() {
                    // Popups lay out justified, which stretches wrapped text: left-align it.
                    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.add(egui::Label::new(theme::muted("Not in the list, but you can still plant it. Its dates come from what you fill in.")).wrap());
                    });
                }
            });
            if !found.is_empty() {
                use_typed(ui, &mut pick);
            }
            if let Some(name) = pick {
                *crop = name;
                ui.data_mut(|d| d.remove::<String>(search_id));
                ui.memory_mut(|m| m.close_popup());
            }
        });
}

fn md(d: NaiveDate) -> String {
    d.format("%b %-d").to_string()
}

fn span(a: NaiveDate, b: NaiveDate) -> String {
    if a == b {
        md(a)
    } else if a.month() == b.month() {
        format!("{} – {}", md(a), b.day())
    } else {
        format!("{} – {}", md(a), md(b))
    }
}

fn flower(crop: &str) -> bool {
    g::crop(crop).is_some_and(|c| c.is_flower())
}

fn status_label(s: Status, crop: &str) -> &'static str {
    match s {
        Status::Harvesting if flower(crop) => "Flowering",
        s => s.label(),
    }
}

fn step_label(s: Step, crop: &str) -> &'static str {
    match s {
        Step::Harvest if flower(crop) => "Flowers",
        s => s.label(),
    }
}

fn status_chip(ui: &mut Ui, s: Status, crop: &str) {
    let (fg, bg) = match s {
        Status::Planned => (c::muted(), c::hover()),
        Status::StartedIndoors => (c::amber(), c::amber_soft()),
        Status::Growing => (c::forest(), c::mint_soft()),
        Status::Harvesting => (c::coral_dark(), c::coral_soft()),
        Status::Finished => (c::faint(), c::hover()),
    };
    theme::chip(ui, status_label(s, crop), fg, bg);
}

fn step_icon(s: Step) -> &'static str {
    match s {
        Step::StartIndoors => icon::HOUSE_LINE,
        Step::Transplant | Step::PlantOut => icon::SHOVEL,
        Step::Sow => icon::GRAINS,
        Step::Harvest => icon::BASKET,
    }
}

// ---------------------------------------------------------------------------
// A planting

pub fn planting_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, id: PlantingId) {
    let units = project.info.units;
    let today = env.today;
    let settings = project.garden.clone();
    let Some(p) = project.planting(id) else { return };
    let seed = p.seed.and_then(|s| project.seed(s)).cloned();
    let stages = p.stages(&settings, seed.as_ref());
    let status = p.status(today, &settings, seed.as_ref());
    let hints = project.hints(id);
    let garden = p.garden;
    let garden_name = project.entity_name(EntityRef::Feature(garden));
    let seeds: Vec<(SeedId, String, bool)> = project
        .seeds
        .iter()
        .map(|s| (s.id, s.display_name(), s.crop.eq_ignore_ascii_case(&p.crop)))
        .collect();
    let names: Vec<(PlantingId, String)> = project.plantings.iter().map(|o| (o.id, o.display_name())).collect();
    let name_of = |id: PlantingId| names.iter().find(|(i, _)| *i == id).map(|(_, n)| n.clone()).unwrap_or_default();

    let mut new_seed = None;
    let Some(p) = project.planting_mut(id) else { return };
    let info = p.info();
    let size = match p.layout {
        Layout::Row => format!("{} row", format_length(p.length(), units)),
        Layout::Patch => format!("{} patch", format_area(p.area(), units)),
        Layout::Plant => format!("{} across", format_length(p.size, units)),
    };
    let count = p.plant_count();
    let plants = if count == 1 { "1 plant".to_string() } else { format!("{count} plants") };
    heading(ui, &p.display_name(), Some(format!("{size} · {plants} · {}", p.year)));
    ui.horizontal(|ui| {
        status_chip(ui, status, &p.crop);
        if ui.add(egui::Button::new(RichText::new(format!("{}  {garden_name}", icon::PLANT)).color(c::forest())).frame(false)).clicked() {
            env.actions.push(Action::OpenGarden(garden));
        }
    });
    ui.add_space(4.0);

    // -- What it is
    theme::section(ui, ("planting", id), crop_icon(&p.crop), "Planting", None, true, |ui| {
        form_grid(ui, ("planting-grid", id), |ui| {
            field_label(ui, "Crop");
            ui.horizontal(|ui| {
                let before = p.crop.clone();
                crop_combo(ui, ("planting-crop", id), &mut p.crop);
                if p.crop != before {
                    p.method = g::Method::default_for(p.info());
                }
            });
            ui.end_row();
            if info.is_none() && !p.crop.trim().is_empty() {
                field_label(ui, "");
                theme::toggle(ui, &mut p.returns, "Comes back every year");
                ui.end_row();
            }
            text_row(ui, "Variety", &mut p.variety, "e.g. Sungold");
            field_label(ui, "Seeds");
            let current = p.seed.and_then(|s| seeds.iter().find(|(i, _, _)| *i == s)).map(|(_, n, _)| n.clone());
            let text = current.unwrap_or_else(|| "None".into());
            // This crop's seeds first.
            let ordered: Vec<&(SeedId, String, bool)> = seeds.iter().filter(|s| s.2).chain(seeds.iter().filter(|s| !s.2)).collect();
            if ordered.len() + 1 > SEARCH_OVER {
                let crop_group = format!("{} seeds", p.crop.trim());
                let choices: Vec<Choice> = std::iter::once(Choice::new("None"))
                    .chain(ordered.iter().map(|(_, n, m)| Choice::new(n.clone()).group(if *m { crop_group.clone() } else { "Other seeds".to_string() })))
                    .collect();
                let selected = match p.seed {
                    None => Some(0),
                    Some(s) => ordered.iter().position(|(i, _, _)| *i == s).map(|k| k + 1),
                };
                match search_combo(ui, ("planting-seed", id), text, &choices, selected, Some("Add packet"), 0.0) {
                    Some(Picked::Item(i)) => p.seed = if i == 0 { None } else { Some(ordered[i - 1].0) },
                    // A new packet of this crop, with what was typed as its variety.
                    Some(Picked::Custom(variety)) => {
                        let mut s = SeedPacket::new(p.crop.trim());
                        s.variety = variety;
                        p.seed = Some(s.id);
                        new_seed = Some(s);
                    }
                    None => {}
                }
            } else {
                theme::combo_box(("planting-seed", id)).selected_text(text).show_ui(ui, |ui| {
                    ui.selectable_value(&mut p.seed, None, "None");
                    for (sid, name, matches) in &ordered {
                        let text = if *matches { RichText::new(name) } else { theme::muted(name.as_str()) };
                        ui.selectable_value(&mut p.seed, Some(*sid), text);
                    }
                });
            }
            ui.end_row();
            field_label(ui, "How");
            combo(ui, ("planting-method", id), &mut p.method, &g::Method::ALL, g::Method::label);
            ui.end_row();
            field_label(ui, "Year");
            ui.add(egui::DragValue::new(&mut p.year).range(1900..=2200));
            ui.end_row();
            if p.layout == Layout::Plant {
                field_label(ui, "Spread");
                length_edit_min(ui, ("planting-size", id), &mut p.size, units, 0.05);
                ui.end_row();
            } else {
                field_label(ui, "Spacing");
                let default = info.map_or(0.3, |c| c.spacing_cm as f64 / 100.0);
                opt_length_edit(ui, ("planting-spacing", id), &mut p.spacing, default, units);
                ui.end_row();
                field_label(ui, "Plants");
                ui.horizontal(|ui| match &mut p.count {
                    Some(n) => {
                        ui.add(egui::DragValue::new(n).range(1..=100_000));
                        if theme::icon_button_sized(ui, icon::X, "Work it out from the spacing", false, 26.0).clicked() {
                            p.count = None;
                        }
                    }
                    None => {
                        ui.label(theme::muted(format!("about {}", p.estimated_count())));
                        if theme::soft_button(ui, "Set").clicked() {
                            p.count = Some(p.estimated_count());
                        }
                    }
                });
                ui.end_row();
            }
            field_label(ui, "Colour");
            color_toggle(ui, &mut p.color, crop_color(&p.crop), "Custom");
            ui.end_row();
        });
        if let Some(c) = info {
            ui.add_space(4.0);
            theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                let mut facts = vec![capitalise(c.family.label())];
                if c.perennial {
                    facts.push("perennial".into());
                } else {
                    facts.push(format!("{} days to harvest", p.days_to_harvest.unwrap_or(c.maturity_days as u32)));
                }
                if c.germinate_days.1 > 0 {
                    facts.push(format!("sprouts in {}–{} days", c.germinate_days.0, c.germinate_days.1));
                }
                facts.push(format!(
                    "{} apart, rows {}",
                    format_length(c.spacing_cm as f64 / 100.0, units),
                    format_length(c.row_cm as f64 / 100.0, units)
                ));
                ui.add(egui::Label::new(theme::muted(facts.join(" · "))).wrap());
                ui.add(egui::Label::new(RichText::new(c.tip).size(13.0)).wrap());
            });
        } else if !p.crop.trim().is_empty() {
            ui.add(egui::Label::new(theme::muted("Not in the crop list, so dates come from what you fill in: set the spacing and days to harvest above and below.")).wrap());
        }
    });

    // -- When
    let mut add_reminders = false;
    theme::section(ui, ("planting-when", id), icon::CALENDAR, "Schedule", None, true, |ui| {
        if settings.last_frost.is_none() && info.is_some() {
            ui.add(egui::Label::new(theme::muted("Add your frost dates on the garden bed to see when to start, sow and transplant.")).wrap());
        }
        for s in &stages {
            stage_row(ui, s, today, &p.crop);
        }
        ui.add_space(4.0);
        form_grid(ui, ("planting-dates", id), |ui| {
            if p.method == g::Method::StartedIndoors {
                field_label(ui, "Started");
                opt_date_edit(ui, &format!("planting-started-{id}"), &mut p.started, today);
                ui.end_row();
            }
            field_label(ui, match p.method {
                g::Method::Sown => "Sown",
                g::Method::StartedIndoors => "Transplanted",
                g::Method::Bought => "Planted",
            });
            opt_date_edit(ui, &format!("planting-planted-{id}"), &mut p.planted, today);
            ui.end_row();
            field_label(ui, "Taken out");
            opt_date_edit(ui, &format!("planting-removed-{id}"), &mut p.removed, today);
            ui.end_row();
            if !p.perennial() {
                field_label(ui, "Days to harvest");
                ui.horizontal(|ui| match &mut p.days_to_harvest {
                    Some(d) => {
                        ui.add(egui::DragValue::new(d).range(1..=400));
                        if theme::icon_button_sized(ui, icon::X, "Use the usual for this crop", false, 26.0).clicked() {
                            p.days_to_harvest = None;
                        }
                    }
                    None => {
                        let usual = seed.as_ref().and_then(|s| s.days_to_harvest).or(info.map(|c| c.maturity_days as u32));
                        ui.label(theme::muted(usual.map_or("—".into(), |d| format!("{d} (usual)"))));
                        if theme::soft_button(ui, "Set").clicked() {
                            p.days_to_harvest = Some(usual.unwrap_or(60));
                        }
                    }
                });
                ui.end_row();
            }
        });
        if stages.iter().any(|s| s.done.is_none() && s.to >= today) {
            add_reminders = theme::soft_button(ui, format!("{}  Remind me of these", icon::BELL)).clicked();
        }
    });

    // -- Neighbours
    if !hints.is_empty() || info.is_some() {
        let n = hints.len();
        theme::section(ui, ("planting-hints", id), icon::HANDSHAKE, "Neighbours & rotation", None, n > 0, |ui| {
            // One line per neighbouring crop, however many plants of it.
            let mut lines: Vec<(&str, egui::Color32, String, PlantingId)> = Vec::new();
            let mut counts: Vec<(String, usize)> = Vec::new();
            for h in &hints {
                if let Hint::Good { other } | Hint::Bad { other } = h {
                    let key = format!("{}{}", matches!(h, Hint::Good { .. }), name_of(*other));
                    match counts.iter_mut().find(|(k, _)| *k == key) {
                        Some((_, n)) => *n += 1,
                        None => counts.push((key, 1)),
                    }
                }
            }
            let mut seen = Vec::new();
            for h in &hints {
                let line = match h {
                    Hint::Rotation { other, year, family } => (
                        icon::WARNING,
                        c::amber(),
                        format!("{} grew here in {year}, and it's in the same {}. Rotating families helps keep soil pests and disease down.", name_of(*other), family.label()),
                        *other,
                    ),
                    Hint::Good { other } | Hint::Bad { other } => {
                        let good = matches!(h, Hint::Good { .. });
                        let key = format!("{good}{}", name_of(*other));
                        if seen.contains(&key) {
                            continue;
                        }
                        let n = counts.iter().find(|(k, _)| *k == key).map_or(1, |(_, n)| *n);
                        let who = if n > 1 { format!("{} ({n} plants)", name_of(*other)) } else { name_of(*other) };
                        seen.push(key);
                        if good {
                            (icon::CHECK_CIRCLE, c::forest(), format!("Good next to {who}."), *other)
                        } else {
                            (icon::WARNING_CIRCLE, c::coral(), format!("Doesn't do well next to {who}."), *other)
                        }
                    }
                };
                lines.push(line);
            }
            for (i, col, text, other) in lines {
                let resp = ui
                    .horizontal_top(|ui| {
                        ui.label(RichText::new(i).color(col));
                        ui.add(egui::Label::new(text).wrap().sense(Sense::click()))
                    })
                    .inner;
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    env.select(EntityRef::Planting(other));
                }
            }
            if let Some(c) = info {
                if !c.companions.is_empty() {
                    ui.add(egui::Label::new(theme::muted(format!("Grows well with {}.", c.companions.join(", ").to_lowercase()))).wrap());
                }
                if !c.avoid.is_empty() {
                    ui.add(egui::Label::new(theme::muted(format!("Keep away from {}.", c.avoid.join(", ").to_lowercase()))).wrap());
                }
            }
        });
    }

    // -- Harvests
    let n = p.harvests.len();
    theme::section(ui, ("planting-harvests", id), icon::BASKET, "Harvests", Some(n), n > 0, |ui| {
        let mut remove = None;
        for (i, h) in p.harvests.iter_mut().enumerate() {
            ui.push_id(("harvest", id, i), |ui| {
                theme::card().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        date_edit(ui, &format!("harvest-date-{id}-{i}"), &mut h.date, today);
                        let mut amount = h.amount.unwrap_or(0.0);
                        if ui.add(egui::DragValue::new(&mut amount).range(0.0..=100_000.0).speed(0.1).max_decimals(2)).changed() {
                            h.amount = (amount > 0.0).then_some(amount);
                        }
                        unit_combo(ui, ("harvest-unit", id, i), &mut h.unit);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if theme::icon_button_sized(ui, icon::TRASH, "Remove", false, 26.0).clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                    ui.add(egui::TextEdit::singleline(&mut h.notes).hint_text("Notes (taste, size, pests...)").desired_width(f32::INFINITY));
                });
            });
        }
        if let Some(i) = remove {
            p.harvests.remove(i);
        }
        let totals = p.total_harvest();
        if !totals.is_empty() {
            let t: Vec<String> = totals.iter().map(|(u, a)| format!("{} {u}", trim_num(*a))).collect();
            ui.label(theme::muted(format!("Total: {}", t.join(", "))));
        }
        if theme::soft_button(ui, format!("{}  Log a harvest", icon::PLUS)).clicked() {
            let unit = p.harvests.last().map(|h| h.unit.clone()).unwrap_or_else(|| if units == UnitSystem::Imperial { "lb".into() } else { "kg".into() });
            p.harvests.push(Harvest { date: today, amount: None, unit, notes: String::new() });
        }
    });

    theme::section(ui, ("planting-custom", id), icon::LIST_PLUS, "Custom fields", Some(p.custom_fields.len()), !p.custom_fields.is_empty(), |ui| {
        custom_fields(ui, ("planting-cf", id), &mut p.custom_fields);
    });

    if let Some(s) = new_seed {
        project.seeds.push(s);
    }
    if add_reminders {
        let n = project.add_planting_reminders(id, today);
        env.toasts.info(match n {
            0 => "Those reminders are already set.".to_string(),
            1 => "Added 1 reminder.".to_string(),
            n => format!("Added {n} reminders."),
        });
    }
    notes_section(ui, project, env, EntityRef::Planting(id));
    reminders_section(ui, project, env, EntityRef::Planting(id));
    ui.add_space(6.0);
    if danger_button(ui, "Delete planting") {
        env.actions.push(Action::Delete(EntityRef::Planting(id)));
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn trim_num(x: f64) -> String {
    let s = format!("{x:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A unit picker from the common list (keeps a custom unit if there is one).
fn unit_combo(ui: &mut Ui, id: impl std::hash::Hash, value: &mut String) {
    let text = if value.trim().is_empty() { "unit".to_string() } else { value.clone() };
    let choices: Vec<Choice> = HARVEST_UNITS.iter().map(|u| Choice::new(*u)).collect();
    let selected = HARVEST_UNITS.iter().position(|u| u == value);
    match search_combo(ui, id, text, &choices, selected, Some("Use"), 84.0) {
        Some(Picked::Item(i)) => *value = HARVEST_UNITS[i].to_string(),
        Some(Picked::Custom(u)) => *value = u,
        None => {}
    }
}

fn stage_row(ui: &mut Ui, s: &Stage, today: NaiveDate, crop: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(step_icon(s.step)).color(if s.done.is_some() { c::forest() } else { c::muted() }));
        ui.label(step_label(s.step, crop));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let text = match s.done {
                Some(d) if s.step != Step::Harvest => format!("{} {}", icon::CHECK, md(d)),
                // Planned from frost dates rather than from what happened.
                _ if !s.firm => format!("~ {}", span(s.from, s.to)),
                _ => span(s.from, s.to),
            };
            let late = s.done.is_none() && s.to < today && s.step != Step::Harvest;
            let now = s.done.is_none() && s.from <= today && s.to >= today;
            let color = if late {
                c::coral()
            } else if now {
                c::forest()
            } else {
                c::muted()
            };
            ui.label(RichText::new(text).color(color));
            if now {
                theme::chip(ui, "Now", c::forest(), c::mint_soft());
            } else if late {
                theme::chip(ui, "Missed?", c::coral_dark(), c::coral_soft());
            }
        });
    });
}

// ---------------------------------------------------------------------------
// A garden bed

/// The extra sections for a garden bed's panel.
pub fn garden_sections(ui: &mut Ui, project: &mut Project, env: &mut Env, id: FeatureId, in_plan: bool) {
    let today = env.today;
    let year = year(ui.ctx(), today);
    type Row = (PlantingId, String, String, Status, Option<Stage>, Vec<Stage>);
    let plantings: Vec<Row> = project
        .plantings_in(id, year)
        .into_iter()
        .map(|p| {
            let seed = p.seed.and_then(|s| project.seed(s));
            (
                p.id,
                p.display_name(),
                p.crop.clone(),
                p.status(today, &project.garden, seed),
                p.next_step(today, &project.garden, seed),
                p.stages(&project.garden, seed),
            )
        })
        .collect();
    let years = project.garden_years(id);

    theme::section(ui, ("garden", id), icon::PLANT, "Garden", Some(plantings.len()), true, |ui| {
        ui.horizontal(|ui| {
            if !in_plan && theme::primary_button(ui, format!("{}  Open garden plan", icon::PLANT)).clicked() {
                env.actions.push(Action::OpenGarden(id));
            }
            year_chips(ui, year, &years, today);
        });
        ui.add_space(4.0);
        if plantings.is_empty() {
            ui.add(egui::Label::new(theme::muted(format!("Nothing planned for {year} yet. Open the garden plan and draw rows, patches or single plants."))).wrap());
        }
        let mut order: Vec<usize> = (0..plantings.len()).collect();
        order.sort_by_key(|i| plantings[*i].3 == Status::Finished);
        for (pid, name, crop, status, next, _) in order.iter().map(|i| &plantings[*i]) {
            let detail = match next {
                _ if *status == Status::Finished => Some(status_label(*status, crop).to_lowercase()),
                Some(s) if s.step == Step::Harvest && s.from <= today => Some(status_label(Status::Harvesting, crop).to_lowercase()),
                Some(s) => Some(format!("{} {}", step_label(s.step, crop).to_lowercase(), md(s.from.max(today)))),
                None => Some(status_label(*status, crop).to_lowercase()),
            };
            related_row(ui, env, crop_icon(crop), name, detail, EntityRef::Planting(*pid));
        }
        if !plantings.is_empty() {
            ui.add_space(6.0);
            season_chart(ui, year, &plantings.iter().map(|p| (p.2.clone(), p.1.clone(), p.5.clone())).collect::<Vec<_>>(), &project.garden, today);
        }
        let totals = project.harvest_totals(id, year);
        if !totals.is_empty() {
            ui.add_space(4.0);
            let t: Vec<String> = totals.iter().map(|(c, u, a)| format!("{c} {} {u}", trim_num(*a))).collect();
            ui.add(egui::Label::new(theme::muted(format!("Harvested in {year}: {}", t.join(" · ")))).wrap());
        }
    });
    frost_section(ui, project, env, id);
    seeds_section(ui, project, env, ("garden-seeds", id), year);
}

fn year_chips(ui: &mut Ui, year: i32, years: &[i32], today: NaiveDate) {
    let this = today.year();
    let mut shown: Vec<i32> = years.iter().copied().chain([this, year]).collect();
    shown.sort_unstable();
    shown.dedup();
    shown.reverse();
    shown.truncate(4);
    for y in shown {
        let (fg, bg) = if y == year { (egui::Color32::WHITE, c::forest()) } else { (c::forest(), c::mint_soft()) };
        if theme::chip(ui, y.to_string(), fg, bg).interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            set_year(ui.ctx(), y);
        }
    }
}

/// One bar per planting across the months: starting indoors, in the bed,
/// and harvest.
fn season_chart(ui: &mut Ui, year: i32, rows: &[(String, String, Vec<Stage>)], settings: &GardenSettings, today: NaiveDate) {
    let label_w = 92.0;
    let row_h = 18.0;
    let width = ui.available_width();
    let height = 18.0 + rows.len() as f32 * row_h + 4.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let p = ui.painter_at(rect.expand(2.0));
    let chart = egui::Rect::from_min_max(rect.min + vec2(label_w, 0.0), rect.max);
    let start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap();
    let total = (end - start).num_days() as f32;
    let x = |d: NaiveDate| chart.left() + ((d - start).num_days() as f32 / total).clamp(0.0, 1.0) * chart.width();
    // Months
    for m in 1..=12u32 {
        let d = NaiveDate::from_ymd_opt(year, m, 1).unwrap();
        let x0 = x(d);
        let next = if m == 12 { end } else { NaiveDate::from_ymd_opt(year, m + 1, 1).unwrap() };
        if m % 2 == 1 {
            p.rect_filled(egui::Rect::from_min_max(egui::pos2(x0, chart.top() + 14.0), egui::pos2(x(next), chart.bottom())), 0.0, c::hover());
        }
        let letter = d.format("%b").to_string().chars().next().unwrap_or(' ').to_string();
        p.text(egui::pos2((x0 + x(next)) / 2.0, chart.top() + 6.0), Align2::CENTER_CENTER, letter, FontId::proportional(10.5), c::faint());
    }
    // Frost-free season
    if let (Some(a), Some(b)) = (settings.last_frost_in(year), settings.first_frost_in(year)) {
        p.line_segment([egui::pos2(x(a), chart.top() + 14.0), egui::pos2(x(a), chart.bottom())], Stroke::new(1.0_f32, c::teal().gamma_multiply(0.6)));
        p.line_segment([egui::pos2(x(b), chart.top() + 14.0), egui::pos2(x(b), chart.bottom())], Stroke::new(1.0_f32, c::teal().gamma_multiply(0.6)));
    }
    if today.year() == year {
        p.line_segment([egui::pos2(x(today), chart.top() + 12.0), egui::pos2(x(today), chart.bottom())], Stroke::new(1.5_f32, c::coral()));
    }
    for (i, (crop, name, stages)) in rows.iter().enumerate() {
        let y = chart.top() + 18.0 + i as f32 * row_h + row_h / 2.0;
        let label = p.layout_no_wrap(name.clone(), FontId::proportional(11.5), c::ink());
        let clip = egui::Rect::from_min_max(egui::pos2(rect.left(), y - row_h / 2.0), egui::pos2(chart.left() - 4.0, y + row_h / 2.0));
        ui.painter_at(clip).galley(egui::pos2(rect.left(), y - label.size().y / 2.0), label, c::ink());
        let [r, g_, b] = crop_color(crop);
        let col = egui::Color32::from_rgb(r, g_, b);
        let bar = |a: NaiveDate, b: NaiveDate, fill: egui::Color32, h: f32| {
            let r = egui::Rect::from_min_max(egui::pos2(x(a), y - h / 2.0), egui::pos2(x(b).max(x(a) + 3.0), y + h / 2.0));
            p.rect_filled(r, 3.0, fill);
        };
        // In the ground: from planting out until harvest ends.
        let out = stages.iter().find(|s| matches!(s.step, Step::Sow | Step::Transplant | Step::PlantOut));
        let harvest = stages.iter().find(|s| s.step == Step::Harvest);
        if let Some(s) = stages.iter().find(|s| s.step == Step::StartIndoors) {
            let from = s.done.unwrap_or(s.from);
            let to = out.map_or(s.to, |o| o.done.unwrap_or(o.from));
            bar(from, to, theme::mix(c::canvas(), col, 0.3), 6.0);
        }
        if let Some(o) = out {
            let from = o.done.unwrap_or(o.from);
            let to = harvest.map_or(from + chrono::Duration::days(60), |h| h.from);
            bar(from, to, theme::mix(c::canvas(), col, 0.55), 10.0);
        }
        if let Some(h) = harvest {
            bar(h.from, h.to, col, 10.0);
        }
    }
}

fn frost_section(ui: &mut Ui, project: &mut Project, env: &mut Env, id: FeatureId) {
    let today = env.today;
    let year = today.year();
    let s = &mut project.garden;
    let set = s.last_frost.is_some() && s.first_frost.is_some();
    theme::section(ui, ("garden-frost", id), icon::SNOWFLAKE, "Frost dates", None, !set, |ui| {
        ui.add(egui::Label::new(theme::muted("Average dates for where you live. The planting calendar counts from these. They're for the whole property.")).wrap());
        form_grid(ui, ("frost-grid", id), |ui| {
            field_label(ui, "Last spring frost");
            opt_date_edit(ui, "frost-last", &mut s.last_frost, NaiveDate::from_ymd_opt(year, 5, 10).unwrap_or(today));
            ui.end_row();
            field_label(ui, "First fall frost");
            opt_date_edit(ui, "frost-first", &mut s.first_frost, NaiveDate::from_ymd_opt(year, 10, 10).unwrap_or(today));
            ui.end_row();
            text_row(ui, "Zone", &mut s.zone, "e.g. 6a");
        });
        if let Some(days) = s.season_days(year) {
            ui.label(theme::muted(format!("{days} frost-free days")));
        }
    });
}

/// The seed inventory (shared by every garden).
pub fn seeds_section(ui: &mut Ui, project: &mut Project, env: &mut Env, id_salt: impl std::hash::Hash + Copy, year: i32) {
    let today = env.today;
    let used: Vec<(SeedId, usize)> = project.seeds.iter().map(|s| (s.id, project.plantings.iter().filter(|p| p.seed == Some(s.id)).count())).collect();
    let in_stock = project.seeds.iter().filter(|s| !s.used_up).count();
    theme::section(ui, id_salt, icon::PACKAGE, "Seeds", Some(in_stock), false, |ui| {
        ui.add(egui::Label::new(theme::muted("Seed packets on hand, for every garden. Pick one on a planting to use its days to harvest. Seeds go with you: they're left out of a copy for a new owner.")).wrap());
        let mut remove = None;
        let mut order: Vec<usize> = (0..project.seeds.len()).collect();
        order.sort_by_key(|i| (project.seeds[*i].used_up, project.seeds[*i].crop.to_lowercase()));
        for i in order {
            let s = &mut project.seeds[i];
            let sid = s.id;
            let chip = if s.used_up {
                Some(("Used up".to_string(), c::faint(), c::hover()))
            } else if s.is_old(year) {
                Some(("Old".to_string(), c::amber(), c::amber_soft()))
            } else {
                None
            };
            let n = used.iter().find(|(i, _)| *i == sid).map_or(0, |(_, n)| *n);
            seed_card(ui, (id_salt, sid), &s.display_name(), chip, |ui| {
                form_grid(ui, ("seed-grid", sid), |ui| {
                    field_label(ui, "Crop");
                    crop_combo(ui, ("seed-crop", sid), &mut s.crop);
                    ui.end_row();
                    text_row(ui, "Variety", &mut s.variety, "e.g. Sungold");
                    text_row(ui, "From", &mut s.source, "Brand, shop, or saved");
                    field_label(ui, "Packed for");
                    ui.horizontal(|ui| match &mut s.packed_for {
                        Some(y) => {
                            ui.add(egui::DragValue::new(y).range(1990..=2200));
                            if theme::icon_button_sized(ui, icon::X, "Clear", false, 26.0).clicked() {
                                s.packed_for = None;
                            }
                        }
                        None => {
                            if theme::soft_button(ui, "Set").clicked() {
                                s.packed_for = Some(today.year());
                            }
                        }
                    });
                    ui.end_row();
                    text_row(ui, "Left", &mut s.amount, "e.g. half a packet");
                    field_label(ui, "Germination");
                    ui.horizontal(|ui| {
                        let mut v = s.germination.unwrap_or(0.0);
                        if ui.add(egui::DragValue::new(&mut v).range(0.0..=100.0).suffix(" %")).changed() {
                            s.germination = (v > 0.0).then_some(v);
                        }
                    });
                    ui.end_row();
                    field_label(ui, "Days to harvest");
                    ui.horizontal(|ui| {
                        let mut v = s.days_to_harvest.unwrap_or(0);
                        if ui.add(egui::DragValue::new(&mut v).range(0..=400)).changed() {
                            s.days_to_harvest = (v > 0).then_some(v);
                        }
                    });
                    ui.end_row();
                    field_label(ui, "");
                    ui.horizontal(|ui| {
                        theme::toggle(ui, &mut s.organic, "Organic");
                        theme::toggle(ui, &mut s.used_up, "Used up");
                    });
                    ui.end_row();
                });
                if let Some(y) = s.good_until() {
                    let crop = s.crop.to_lowercase();
                    ui.label(theme::muted(if y < year {
                        format!("Probably past its best: {crop} seed usually keeps until about {y}. Test a few on a damp paper towel.")
                    } else {
                        format!("Should keep until about {y}.")
                    }));
                }
                ui.add(egui::TextEdit::multiline(&mut s.notes).hint_text("Notes").desired_rows(1).desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    if n > 0 {
                        ui.label(theme::muted(if n == 1 { "Used in 1 planting".to_string() } else { format!("Used in {n} plantings") }));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::danger_link(ui, "Remove").clicked() {
                            remove = Some(sid);
                        }
                    });
                });
            });
            ui.add_space(4.0);
        }
        if let Some(sid) = remove {
            project.seeds.retain(|s| s.id != sid);
            for p in &mut project.plantings {
                if p.seed == Some(sid) {
                    p.seed = None;
                }
            }
        }
        if theme::soft_button(ui, format!("{}  Add seeds", icon::PLUS)).clicked() {
            let mut s = SeedPacket::new("");
            s.bought = Some(today);
            s.packed_for = Some(today.year());
            project.seeds.push(s);
        }
    });
}

fn seed_card(ui: &mut Ui, id_salt: impl std::hash::Hash, title: &str, chip: Option<(String, egui::Color32, egui::Color32)>, body: impl FnOnce(&mut Ui)) {
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        let id = ui.make_persistent_id(id_salt);
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, title.trim().is_empty() || title == "Seeds")
            .show_header(ui, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((text, fg, bg)) = chip {
                        theme::chip(ui, text, fg, bg);
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(theme::strong(title)).truncate());
                    });
                });
            })
            .body_unindented(body);
    });
}

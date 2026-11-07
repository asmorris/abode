//! The Pets page: each pet's profile, vet, vaccinations, medications and
//! weight, with their appointments, reminders, vet visits and notes.
//!
//! Pets are private. They aren't in a copy for a new owner, so nothing here
//! has a privacy toggle.

use super::notes::{attachments_ui, notes_section, reminders_section, service_section};
use super::vehicle::list_card;
use super::widgets::*;
use super::{Action, AttachTarget, Env};
use crate::theme::{self, c, icon};
use abode_core::pet::*;
use abode_core::units::UnitSystem;
use abode_core::*;
use chrono::NaiveDate;
use eframe::egui::{self, Color32, RichText, Sense, Ui, vec2};

fn selected_id() -> egui::Id {
    egui::Id::new("pets-selected")
}

/// Show this pet next time the page is drawn.
pub fn select(ctx: &egui::Context, id: PetId) {
    ctx.data_mut(|d| d.insert_temp(selected_id(), id));
}

pub fn species_icon(s: Species) -> &'static str {
    match s {
        Species::Dog => icon::DOG,
        Species::Cat => icon::CAT,
        Species::Bird => icon::BIRD,
        Species::Fish => icon::FISH,
        Species::Horse => icon::HORSE,
        Species::Rabbit => icon::RABBIT,
        _ => icon::PAW_PRINT,
    }
}

/// Something the page asked for, done once drawing is finished.
enum Event {
    Add(Species, String),
    VaccineReminder(usize, bool),
    RecordDose(usize),
    RemoveVaccine(usize),
    MedReminder(usize),
    StopMedReminder(usize),
    RemoveMed(usize),
}

pub fn pets_ui(ui: &mut Ui, project: &mut Project, env: &mut Env) {
    let today = env.today;
    let mut event: Option<Event> = None;

    // Which pet: the one asked for, else the first.
    let mut current: Option<PetId> = ui.ctx().data(|d| d.get_temp(selected_id()));
    if current.is_none_or(|id| project.pet(id).is_none()) {
        current = project.pets_in_order().first().map(|p| p.id);
    }

    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            add_pet_menu(ui, &mut event);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(
                    egui::Label::new(theme::muted(
                        "Vaccines, medications, vet visits and appointments for the whole crew. Private to you: pets aren't in a copy for a new owner.",
                    ))
                    .wrap(),
                );
            });
        });
    });
    ui.add_space(8.0);

    if project.pets.is_empty() {
        empty(ui, &mut event);
    } else {
        // One pill per pet.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            for pet in project.pets_in_order() {
                if pet_pill(ui, env, pet, current == Some(pet.id)).clicked() {
                    current = Some(pet.id);
                }
            }
        });
        ui.add_space(10.0);
    }
    if let Some(id) = current {
        ui.ctx().data_mut(|d| d.insert_temp(selected_id(), id));
        egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt(("pets-scroll", id)).show(ui, |ui| {
            ui.set_width(ui.available_width() - 6.0);
            pet_page(ui, project, env, id, &mut event);
            ui.add_space(16.0);
        });
    }

    let Some(e) = event else {
        if let Some(id) = current {
            project.sync_pet_reminders(id);
        }
        return;
    };
    match (e, current) {
        (Event::Add(species, name), _) => {
            let id = project.add_pet(name, species);
            select(ui.ctx(), id);
        }
        (Event::VaccineReminder(i, on), Some(id)) => project.set_vaccine_reminder(id, i, on),
        (Event::RecordDose(i), Some(id)) => {
            project.record_dose(id, i, today);
            env.toasts.info("New dose recorded for today.");
        }
        (Event::RemoveVaccine(i), Some(id)) => project.remove_vaccination(id, i),
        (Event::MedReminder(i), Some(id)) => {
            project.add_medication_reminder(id, i, today);
            env.toasts.info("Reminder added. Adjust it under Appointments & reminders.");
        }
        (Event::StopMedReminder(i), Some(id)) => {
            if let Some(rid) = project.pet_mut(id).and_then(|p| p.medications.get_mut(i)).and_then(|m| m.reminder.take()) {
                project.reminders.retain(|r| r.id != rid);
            }
        }
        (Event::RemoveMed(i), Some(id)) => project.remove_medication(id, i),
        _ => {}
    }
    if let Some(id) = current {
        project.sync_pet_reminders(id);
    }
}

/// Pick a kind of animal from the list, type any other ("Ferret"), or
/// choose "Other animal…" and name the kind later.
fn add_pet_menu(ui: &mut Ui, event: &mut Option<Event>) {
    menu_dropdown(ui, RichText::new(format!("{}  Add pet", icon::PLUS)).color(c::forest()), |ui| {
        ui.set_min_width(240.0);
        let listed: Vec<Species> = Species::ALL.iter().copied().filter(|s| *s != Species::Other).collect();
        let choices: Vec<Choice> = listed.iter().map(|s| Choice::new(s.label()).icon(species_icon(*s))).collect();
        let picked = search_list(ui, ui.make_persistent_id("add-pet"), &choices, None, Some("Add"));
        // Always in view, under the list: for anything that isn't on it.
        ui.separator();
        if ui.selectable_label(false, RichText::new(format!("{}  Other animal…", icon::PLUS)).color(c::forest())).on_hover_text("Add it, then say what kind of animal under About").clicked() {
            *event = Some(Event::Add(Species::Other, String::new()));
            close_dropdown(ui);
        }
        match picked {
            Some(Picked::Item(i)) => {
                *event = Some(Event::Add(listed[i], String::new()));
                close_dropdown(ui);
            }
            // A ferret, a tarantula...
            Some(Picked::Custom(kind)) => {
                *event = Some(Event::Add(Species::Other, kind));
                close_dropdown(ui);
            }
            None => {}
        }
    });
}

fn empty(ui: &mut Ui, event: &mut Option<Event>) {
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(theme::strong("No pets yet"));
        ui.label(theme::muted("Add a pet to keep their vet, vaccines, medications and appointments in one place."));
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            for s in [Species::Dog, Species::Cat, Species::Bird, Species::Rabbit, Species::Fish] {
                if theme::secondary_button(ui, format!("{}  {}", species_icon(s), s.label())).clicked() {
                    *event = Some(Event::Add(s, String::new()));
                }
            }
            // Everything else, including animals not on the list.
            add_pet_menu(ui, event);
        });
    });
}

/// A pet's photo, cover-fitted into a rounded square (or circle).
fn photo(ui: &mut Ui, env: &mut Env, pet: &Pet, size: f32, round: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let tex = pet.photo.and_then(|id| env.textures.get(ui.ctx(), env.store, id, 512));
    match tex {
        Some(tex) => {
            let ts = tex.size_vec2();
            let scale = (size / ts.x).max(size / ts.y);
            let shown = ts * scale;
            let (uw, uh) = ((size / shown.x).min(1.0), (size / shown.y).min(1.0));
            let uv = egui::Rect::from_min_max(egui::pos2((1.0 - uw) / 2.0, (1.0 - uh) / 2.0), egui::pos2((1.0 + uw) / 2.0, (1.0 + uh) / 2.0));
            egui::Image::new((tex.id(), vec2(size, size))).uv(uv).corner_radius(round).paint_at(ui, rect);
        }
        None => {
            ui.painter().rect_filled(rect, round, c::mint_soft());
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, species_icon(pet.species), egui::FontId::proportional(size * 0.5), c::forest());
        }
    }
    resp
}

fn pet_pill(ui: &mut Ui, env: &mut Env, pet: &Pet, on: bool) -> egui::Response {
    let fill = if on { c::mint_soft() } else { c::field() };
    let r = egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(255))
        .inner_margin(egui::Margin { left: 4, right: 14, top: 4, bottom: 4 })
        .stroke(if on { egui::Stroke::new(1.0_f32, c::forest()) } else { egui::Stroke::NONE })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                photo(ui, env, pet, 28.0, 14.0);
                let mut name = RichText::new(pet.display_name()).size(14.0).color(if on { c::forest() } else { c::ink() });
                if on {
                    name = name.family(theme::semibold());
                }
                ui.label(name);
                if pet.is_remembered() {
                    ui.label(RichText::new(icon::HEART).size(13.0).color(c::muted()));
                }
            });
        });
    ui.interact(r.response.rect, ui.id().with(("pet-pill", pet.id)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn chip_for(status: VaccineStatus) -> (String, Color32, Color32) {
    match status {
        VaccineStatus::Overdue(d) => (format!("Overdue {d}d"), c::coral_dark(), c::coral_soft()),
        VaccineStatus::DueSoon(0) => ("Due today".into(), c::amber(), c::amber_soft()),
        VaccineStatus::DueSoon(d) => (format!("Due in {d} days"), c::amber(), c::amber_soft()),
        VaccineStatus::Current(d) if d > 365 => (format!("Good for {:.1} years", d as f64 / 365.25), c::forest(), c::mint_soft()),
        VaccineStatus::Current(d) => (format!("Good for {d} days"), c::forest(), c::mint_soft()),
        VaccineStatus::NoDate => ("No due date".into(), c::muted(), c::field()),
    }
}

fn days_chip(days: i64) -> (String, Color32, Color32) {
    match days {
        d if d < 0 => (format!("{}d ago", -d), c::muted(), c::field()),
        0 => ("Today".into(), c::amber(), c::amber_soft()),
        1 => ("Tomorrow".into(), c::amber(), c::amber_soft()),
        d if d <= 7 => (format!("In {d} days"), c::amber(), c::amber_soft()),
        d => (format!("In {d} days"), c::forest(), c::mint_soft()),
    }
}

/// One fact in the header: a heading, a line, maybe a chip; clicking opens
/// that section.
struct Fact {
    title: &'static str,
    line: String,
    chip: Option<(String, Color32, Color32)>,
    section: &'static str,
}

fn facts(project: &Project, pet: &Pet, units: UnitSystem, today: NaiveDate) -> Vec<Fact> {
    let target = EntityRef::Pet(pet.id);
    let mut out = Vec::new();
    let next = project.reminders_for(target).filter(|r| r.is_appointment() && !r.is_done() && r.effective_due() >= today).min_by_key(|r| (r.effective_due(), r.at));
    out.push(match next {
        Some(r) => {
            let when = match r.at {
                Some(t) => format!("{} · {}", r.effective_due().format("%a %b %-d"), fmt_time(t)),
                None => fmt_date(r.effective_due()),
            };
            Fact { title: "Next appointment", line: when, chip: Some(days_chip((r.effective_due() - today).num_days())), section: "Appointments & reminders" }
        }
        None => Fact { title: "Next appointment", line: "None booked".into(), chip: Some(("Book one".into(), c::muted(), c::field())), section: "Appointments & reminders" },
    });
    out.push(match pet.next_vaccine(today) {
        Some(v) => Fact { title: "Vaccines", line: v.display_name(), chip: Some(chip_for(v.status(today))), section: "Vaccinations" },
        None if pet.vaccinations.is_empty() => Fact { title: "Vaccines", line: "None recorded".into(), chip: None, section: "Vaccinations" },
        None => Fact { title: "Vaccines", line: format!("{} recorded", pet.current_vaccines().len()), chip: None, section: "Vaccinations" },
    });
    let meds: Vec<String> = pet.active_medications(today).map(|m| m.display_name()).collect();
    out.push(Fact {
        title: "Medications",
        line: if meds.is_empty() { "None right now".into() } else { meds.join(", ") },
        chip: (!meds.is_empty()).then(|| (format!("{} active", meds.len()), c::forest(), c::mint_soft())),
        section: "Medications",
    });
    out.push(match pet.latest_weight() {
        Some(w) => {
            let chip = pet.weight_change().filter(|d| d.abs() >= 0.005).map(|d| {
                let sign = if d > 0.0 { "+" } else { "−" };
                (format!("{sign}{} since last", format_weight(d.abs(), units)), c::muted(), c::field())
            });
            Fact { title: "Weight", line: format!("{} · {}", format_weight(w.kg, units), w.date.format("%b %-d, %Y")), chip, section: "Weight" }
        }
        None => Fact { title: "Weight", line: "Not recorded".into(), chip: None, section: "Weight" },
    });
    out
}

fn pet_page(ui: &mut Ui, project: &mut Project, env: &mut Env, id: PetId, event: &mut Option<Event>) {
    let today = env.today;
    let units = project.info.units;
    let Some(snapshot) = project.pet(id).cloned() else { return };

    // Header: photo, name, summary, and the facts that matter day to day.
    let facts = facts(project, &snapshot, units, today);
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let r = photo(ui, env, &snapshot, 96.0, 20.0).on_hover_text(if snapshot.photo.is_some() { "Change photo" } else { "Add a photo" });
            if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                env.actions.push(Action::AttachFiles(AttachTarget::PetPhoto(id)));
            }
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                if let Some(pet) = project.pet_mut(id) {
                    let kind = if pet.species == Species::Other && pet.species_name.trim().is_empty() { "pet".to_string() } else { pet.species_label().to_lowercase() };
                    let hint = format!("Name your {kind}");
                    ui.add(
                        egui::TextEdit::singleline(&mut pet.name)
                            .hint_text(hint)
                            .font(egui::FontId::new(28.0, theme::serif()))
                            .frame(false)
                            .desired_width(ui.available_width().min(420.0)),
                    );
                }
                ui.label(theme::muted(snapshot.summary(today)));
                ui.horizontal_wrapped(|ui| {
                    if let Some(d) = snapshot.passed {
                        theme::chip(ui, format!("{}  In memory · {}", icon::HEART, fmt_date(d)), c::muted(), c::field());
                    } else if let Some((on, age)) = snapshot.next_birthday(today) {
                        let days = (on - today).num_days();
                        if days <= 30 {
                            let text = if days == 0 { format!("{}  Turns {age} today!", icon::CAKE) } else { format!("{}  Turns {age} in {days} days", icon::CAKE) };
                            theme::chip(ui, text, c::amber(), c::amber_soft());
                        }
                    }
                    if !snapshot.allergies.trim().is_empty() {
                        theme::chip(ui, format!("{}  Allergic: {}", icon::WARNING, short(&snapshot.allergies, 32)), c::coral_dark(), c::coral_soft());
                    }
                    if !snapshot.vet.phone.trim().is_empty() {
                        theme::chip(ui, format!("{}  Vet {}", icon::PHONE, snapshot.vet.phone.trim()), c::forest(), c::mint_soft());
                    }
                    if !snapshot.emergency_vet.phone.trim().is_empty() {
                        theme::chip(ui, format!("{}  Emergency {}", icon::SIREN, snapshot.emergency_vet.phone.trim()), c::coral_dark(), c::coral_soft());
                    }
                });
            });
        });
        ui.add_space(8.0);
        let cols = if ui.available_width() > 760.0 { 4 } else { 2 };
        let w = (ui.available_width() - 12.0 * (cols as f32 - 1.0)) / cols as f32;
        egui::Grid::new(("pet-facts", id)).num_columns(cols).spacing([12.0, 10.0]).min_col_width(w).max_col_width(w).show(ui, |ui| {
            for (i, f) in facts.iter().enumerate() {
                let r = ui.scope_builder(egui::UiBuilder::new().sense(Sense::click()), |ui| {
                    ui.set_width(w);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        ui.label(theme::caps(f.title));
                        ui.add(egui::Label::new(theme::strong(f.line.clone())).truncate());
                        if let Some((t, fg, bg)) = &f.chip {
                            theme::chip(ui, t.clone(), *fg, *bg);
                        }
                    });
                });
                if r.response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    theme::focus_section(ui.ctx(), &[f.section]);
                }
                if (i + 1) % cols == 0 {
                    ui.end_row();
                }
            }
        });
    });
    ui.add_space(10.0);

    // Two columns when there's room: who they are on the left, their health
    // and schedule on the right.
    let wide = ui.available_width() > 900.0;
    if wide {
        ui.columns(2, |cols| {
            let (left, right) = cols.split_at_mut(1);
            profile_sections(&mut left[0], project, env, id);
            health_sections(&mut right[0], project, env, id, event);
        });
    } else {
        health_sections(ui, project, env, id, event);
        profile_sections(ui, project, env, id);
    }
}

fn short(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() > n { format!("{}…", s.chars().take(n - 1).collect::<String>().trim_end()) } else { s.to_string() }
}

fn profile_sections(ui: &mut Ui, project: &mut Project, env: &mut Env, id: PetId) {
    let today = env.today;
    let Some(pet) = project.pet_mut(id) else { return };

    theme::section(ui, ("pet-about", id), icon::PAW_PRINT, "About", None, true, |ui| {
        form_grid(ui, ("pet-about-grid", id), |ui| {
            field_label(ui, "Species");
            combo_named(ui, ("pet-species", id), &mut pet.species, &mut pet.species_name, &Species::ALL, Species::label, Species::Other);
            ui.end_row();
            if pet.species == Species::Other {
                text_row(ui, "Kind of animal", &mut pet.species_name, "e.g. Ferret, Tarantula, Axolotl");
            }
            field_label(ui, "Breed");
            suggest_edit(ui, ("pet-breed", id), &mut pet.breed, pet.species.breeds(), "");
            ui.end_row();
            field_label(ui, "Sex");
            ui.horizontal(|ui| {
                combo(ui, ("pet-sex", id), &mut pet.sex, &Sex::ALL, Sex::label);
                let label = match pet.sex {
                    Sex::Female => "Spayed",
                    Sex::Male => "Neutered",
                    Sex::Unknown => "Spayed / neutered",
                };
                theme::toggle(ui, &mut pet.fixed, label);
            });
            ui.end_row();
            field_label(ui, "Born");
            ui.horizontal(|ui| {
                opt_date_edit(ui, &format!("pet-born-{id}"), &mut pet.birthday, today);
                if pet.birthday.is_some() {
                    theme::toggle(ui, &mut pet.birthday_estimated, "Estimated");
                }
            });
            ui.end_row();
            field_label(ui, "Adopted");
            opt_date_edit(ui, &format!("pet-adopted-{id}"), &mut pet.adopted, today);
            ui.end_row();
            text_row(ui, "From", &mut pet.adopted_from, "Shelter, rescue or breeder");
            text_row(ui, "Colour & markings", &mut pet.markings, "e.g. Black with a white blaze");
        });
        if let (Some(a), None) = (pet.adopted, pet.passed) {
            if a <= today {
                ui.label(theme::muted(format!("Part of the family for {}.", span_words(a, today))));
            }
        }
    });

    theme::section(ui, ("pet-id", id), icon::IDENTIFICATION_BADGE, "ID & licence", None, false, |ui| {
        form_grid(ui, ("pet-id-grid", id), |ui| {
            text_row(ui, "Microchip #", &mut pet.microchip, "15 digits, on the adoption papers");
            text_row(ui, "Registered with", &mut pet.microchip_registry, "e.g. PetLink, AKC Reunite");
            text_row(ui, "Licence / tag", &mut pet.licence, "City or county licence number");
            field_label(ui, "Licence renews");
            opt_date_edit(ui, &format!("pet-lic-{id}"), &mut pet.licence_renews, today);
            ui.end_row();
        });
        ui.label(theme::muted("Keep the microchip registry's contact details up to date when you move."));
    });

    theme::section(ui, ("pet-care", id), icon::BOWL_FOOD, "Food & care", None, false, |ui| {
        form_grid(ui, ("pet-care-grid", id), |ui| {
            text_row(ui, "Food", &mut pet.food, "Brand and formula");
            text_row(ui, "Feeding", &mut pet.feeding, "How much, and when");
            text_row(ui, "Treats", &mut pet.treats, "Favourites, and what to avoid");
            text_row(ui, "Grooming", &mut pet.grooming, "e.g. Brush weekly, groomer every 6 weeks");
        });
        ui.label(theme::caps("For a sitter"));
        ui.add(egui::TextEdit::multiline(&mut pet.care_notes).hint_text("Walks, routines, where the leash and food are, quirks…").desired_rows(3).desired_width(f32::INFINITY));
    });

    theme::section(ui, ("pet-health", id), icon::HEARTBEAT, "Health", None, false, |ui| {
        form_grid(ui, ("pet-health-grid", id), |ui| {
            text_row(ui, "Allergies", &mut pet.allergies, "Food, medication, flea products…");
            text_row(ui, "Conditions", &mut pet.conditions, "Ongoing conditions to know about");
            text_row(ui, "Temperament", &mut pet.temperament, "With people, other animals, at the vet");
        });
    });

    for (key, glyph, title, vet, hint) in [
        ("pet-vet", icon::FIRST_AID, "Vet", &mut pet.vet, "Their regular clinic"),
        ("pet-er", icon::SIREN, "Emergency vet", &mut pet.emergency_vet, "Open nights and weekends"),
    ] {
        let open = !vet.is_empty();
        theme::section(ui, (key, id), glyph, title, None, open, |ui| {
            form_grid(ui, (key, "grid", id), |ui| {
                text_row(ui, "Clinic", &mut vet.clinic, hint);
                if key == "pet-vet" {
                    text_row(ui, "Vet", &mut vet.vet, "Who they usually see");
                }
                text_row(ui, "Phone", &mut vet.phone, "");
                text_row(ui, "Email", &mut vet.email, "");
                text_row(ui, "Address", &mut vet.address, "");
            });
            ui.add(egui::TextEdit::multiline(&mut vet.notes).hint_text("Hours, client number, portal login hint…").desired_rows(1).desired_width(f32::INFINITY));
        });
    }

    theme::section(ui, ("pet-ins", id), icon::SHIELD_PLUS, "Insurance", None, !pet.insurance.provider.trim().is_empty(), |ui| {
        let ins = &mut pet.insurance;
        form_grid(ui, ("pet-ins-grid", id), |ui| {
            text_row(ui, "Provider", &mut ins.provider, "e.g. Trupanion, Healthy Paws");
            text_row(ui, "Policy #", &mut ins.policy, "");
            text_row(ui, "Claims phone", &mut ins.phone, "");
            text_row(ui, "Coverage", &mut ins.coverage, "e.g. 90% accident & illness");
            text_row(ui, "Deductible", &mut ins.deductible, "");
            field_label(ui, "Renews");
            opt_date_edit(ui, &format!("pet-ins-renew-{id}"), &mut ins.renews, today);
            ui.end_row();
        });
    });

    let n = pet.documents.len();
    theme::section(ui, ("pet-docs", id), icon::FILES, "Documents", Some(n), n > 0, |ui| {
        ui.label(theme::muted("Vaccine certificates, adoption papers, insurance policy, lab results: photos or PDFs."));
        attachments_ui(ui, env, &mut pet.documents, AttachTarget::PetDocs(id));
    });

    theme::section(ui, ("pet-more", id), icon::DOTS_THREE_CIRCLE, "More", None, false, |ui| {
        ui.label(theme::caps("Custom fields"));
        custom_fields(ui, ("pet-custom", id), &mut pet.custom_fields);
        ui.add_space(8.0);
        form_grid(ui, ("pet-more-grid", id), |ui| {
            field_label(ui, "In memory");
            opt_date_edit(ui, &format!("pet-passed-{id}"), &mut pet.passed, today);
            ui.end_row();
        });
        ui.label(theme::muted("If they've passed, set the date: their records stay, and they move to the end of the list."));
        ui.add_space(6.0);
        if pet.photo.is_some() && theme::secondary_button(ui, format!("{}  Remove photo", icon::IMAGE)).clicked() {
            pet.photo = None;
        }
        if danger_button(ui, "Delete this pet") {
            env.dialogs.delete = Some(EntityRef::Pet(id));
        }
    });
}

fn health_sections(ui: &mut Ui, project: &mut Project, env: &mut Env, id: PetId, event: &mut Option<Event>) {
    let target = EntityRef::Pet(id);
    reminders_section(ui, project, env, target);
    vaccinations(ui, project, env, id, event);
    medications(ui, project, env, id, event);
    weight(ui, project, env, id);
    service_section(ui, project, env, target);
    notes_section(ui, project, env, target);
}

fn vaccinations(ui: &mut Ui, project: &mut Project, env: &mut Env, id: PetId, event: &mut Option<Event>) {
    let today = env.today;
    let Some(pet) = project.pet_mut(id) else { return };
    let species = pet.species;
    // The latest dose of each vaccine gets a card; earlier ones are listed below.
    let mut current = pet.current_vaccine_indices();
    current.sort_by_key(|i| (pet.vaccinations[*i].due.is_none(), pet.vaccinations[*i].due));
    let n = current.len();
    theme::section(ui, ("pet-vax", id), icon::SYRINGE, "Vaccinations", Some(n), true, |ui| {
        for &i in &current {
            let v = &mut pet.vaccinations[i];
            let chip = chip_for(v.status(today));
            list_card(ui, ("pet-vax-card", id, i), &v.display_name(), Some(chip), |ui| {
                form_grid(ui, ("pet-vax-grid", id, i), |ui| {
                    field_label(ui, "Vaccine");
                    suggest_edit(ui, ("pet-vax-name", id, i), &mut v.name, species.vaccines(), "");
                    ui.end_row();
                    field_label(ui, "Given");
                    opt_date_edit(ui, &format!("pet-vax-given-{id}-{i}"), &mut v.given, today);
                    ui.end_row();
                    field_label(ui, "Next due");
                    ui.horizontal(|ui| {
                        opt_date_edit(ui, &format!("pet-vax-due-{id}-{i}"), &mut v.due, today);
                        if v.due.is_none() {
                            if let Some(g) = v.given {
                                for (label, months) in [("+1 yr", 12), ("+3 yr", 36)] {
                                    if theme::soft_button(ui, label).on_hover_text("Due this long after it was given").clicked() {
                                        v.due = g.checked_add_months(chrono::Months::new(months));
                                    }
                                }
                            }
                        }
                    });
                    ui.end_row();
                    text_row(ui, "Given by", &mut v.given_by, "Clinic or vet");
                    text_row(ui, "Lot / tag #", &mut v.lot, "Rabies tag number, vaccine lot");
                });
                ui.add(egui::TextEdit::multiline(&mut v.notes).hint_text("Notes (reactions, why it's needed)").desired_rows(1).desired_width(f32::INFINITY));
                ui.horizontal_wrapped(|ui| {
                    let mut remind = v.reminder.is_some();
                    let can = v.due.is_some();
                    let t = ui.add_enabled_ui(can, |ui| theme::toggle(ui, &mut remind, "Remind me when it's due")).inner;
                    if t.changed() {
                        *event = Some(Event::VaccineReminder(i, remind));
                    }
                    if theme::secondary_button(ui, format!("{}  Given again today", icon::SYRINGE))
                        .on_hover_text("Records a new dose today, due again after the same gap")
                        .clicked()
                    {
                        *event = Some(Event::RecordDose(i));
                    }
                    if danger_button(ui, "Remove") {
                        *event = Some(Event::RemoveVaccine(i));
                    }
                });
            });
        }
        // Earlier doses.
        let earlier: Vec<usize> = (0..pet.vaccinations.len()).filter(|i| !current.contains(i)).collect();
        if !earlier.is_empty() {
            let id_salt = ui.make_persistent_id(("pet-vax-earlier", id));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id_salt, false)
                .show_header(ui, |ui| {
                    ui.label(theme::muted(format!("Earlier doses ({})", earlier.len())));
                })
                .body_unindented(|ui| {
                    let mut sorted = earlier.clone();
                    sorted.sort_by_key(|i| std::cmp::Reverse(pet.vaccinations[*i].given));
                    for i in sorted {
                        let v = &pet.vaccinations[i];
                        ui.horizontal(|ui| {
                            ui.label(theme::strong(v.display_name()));
                            ui.label(theme::muted(format!("given {}", v.given.map(fmt_date).unwrap_or_else(|| "?".into()))));
                            if !v.given_by.trim().is_empty() {
                                ui.label(theme::muted(format!("· {}", v.given_by.trim())));
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if theme::icon_button_sized(ui, icon::TRASH, "Remove this record", false, 26.0).clicked() {
                                    *event = Some(Event::RemoveVaccine(i));
                                }
                            });
                        });
                    }
                });
        }
        menu_dropdown(ui, RichText::new(format!("{}  Add vaccine", icon::PLUS)).color(c::forest()), |ui| {
            ui.set_min_width(260.0);
            let have: Vec<String> = pet.vaccinations.iter().map(|v| v.name.to_lowercase()).collect();
            let list = species.vaccines();
            let choices: Vec<Choice> = list.iter().map(|k| Choice::new(*k).enabled(!have.contains(&k.to_lowercase()))).collect();
            let name = match search_list(ui, ui.make_persistent_id(("pet-add-vax", id)), &choices, None, Some("Add")) {
                Some(Picked::Item(i)) => Some(list[i].to_string()),
                Some(Picked::Custom(n)) => Some(n),
                None => None,
            };
            if let Some(name) = name {
                let given_by = pet.vet.clinic.clone();
                pet.vaccinations.push(Vaccination { name, given: Some(today), given_by, ..Default::default() });
                close_dropdown(ui);
            }
        });
        ui.label(theme::muted("Turn on a reminder and it follows the due date. Marking it done records the next dose."));
    });
}

fn medications(ui: &mut Ui, project: &mut Project, env: &mut Env, id: PetId, event: &mut Option<Event>) {
    let today = env.today;
    let Some(pet) = project.pet_mut(id) else { return };
    let mut order: Vec<usize> = (0..pet.medications.len()).collect();
    order.sort_by_key(|i| !pet.medications[*i].is_active(today));
    let active = pet.active_medications(today).count();
    theme::section(ui, ("pet-meds", id), icon::PILL, "Medications", Some(active), active > 0, |ui| {
        let mut past_header = false;
        for i in order {
            let m = &mut pet.medications[i];
            let is_active = m.is_active(today);
            if !is_active && !past_header {
                ui.add_space(4.0);
                ui.label(theme::caps("Past"));
                past_header = true;
            }
            let chip = match (is_active, m.ends) {
                (true, None) => ("Ongoing".to_string(), c::forest(), c::mint_soft()),
                (true, Some(e)) => (format!("Until {}", e.format("%b %-d")), c::amber(), c::amber_soft()),
                (false, Some(e)) if e < today => (format!("Ended {}", e.format("%b %Y")), c::muted(), c::field()),
                (false, _) => ("Not started".to_string(), c::muted(), c::field()),
            };
            let title = if m.dose.trim().is_empty() { m.display_name() } else { format!("{} · {}", m.display_name(), m.dose.trim()) };
            list_card(ui, ("pet-med-card", id, i), &title, Some(chip), |ui| {
                form_grid(ui, ("pet-med-grid", id, i), |ui| {
                    text_row(ui, "Medication", &mut m.name, "");
                    text_row(ui, "Dose", &mut m.dose, "e.g. 1 chew, 75 mg, ½ tablet");
                    field_label(ui, "How often");
                    suggest_edit(ui, ("pet-med-often", id, i), &mut m.how_often, MED_FREQUENCIES, "");
                    ui.end_row();
                    text_row(ui, "For", &mut m.purpose, "What it treats");
                    text_row(ui, "Prescribed by", &mut m.prescribed_by, "");
                    field_label(ui, "Started");
                    opt_date_edit(ui, &format!("pet-med-start-{id}-{i}"), &mut m.started, today);
                    ui.end_row();
                    field_label(ui, "Ends");
                    opt_date_edit(ui, &format!("pet-med-end-{id}-{i}"), &mut m.ends, today);
                    ui.end_row();
                    text_row(ui, "Refills", &mut m.refills, "Refills left, pharmacy, Rx #");
                });
                ui.add(egui::TextEdit::multiline(&mut m.notes).hint_text("Notes (with food? side effects?)").desired_rows(1).desired_width(f32::INFINITY));
                ui.horizontal_wrapped(|ui| {
                    if m.reminder.is_some() {
                        theme::chip(ui, format!("{}  Reminder on", icon::BELL), c::forest(), c::mint_soft());
                        if theme::soft_button(ui, "Stop reminding").clicked() {
                            *event = Some(Event::StopMedReminder(i));
                        }
                    } else if is_active {
                        let hint = match m.suggested_repeat() {
                            Some(r) => format!("A reminder that repeats {}", r.describe().to_lowercase()),
                            None => "A monthly reminder (to refill, say). Change how often it repeats under Appointments & reminders.".into(),
                        };
                        if theme::secondary_button(ui, format!("{}  Remind me", icon::BELL)).on_hover_text(hint).clicked() {
                            *event = Some(Event::MedReminder(i));
                        }
                    }
                    if is_active && m.ends.is_none() && theme::soft_button(ui, "Stopped today").on_hover_text("Ends it today; it moves to Past").clicked() {
                        m.ends = Some(today);
                    }
                    if danger_button(ui, "Remove") {
                        *event = Some(Event::RemoveMed(i));
                    }
                });
            });
        }
        if theme::soft_button(ui, format!("{}  Add medication", icon::PLUS)).clicked() {
            let prescribed_by = pet.vet.vet.clone();
            pet.medications.push(Medication { started: Some(today), prescribed_by, ..Default::default() });
        }
        ui.label(theme::muted("Preventatives, prescriptions and supplements. Monthly ones (flea, tick, heartworm) are worth a reminder."));
    });
}

fn weight(ui: &mut Ui, project: &mut Project, env: &mut Env, id: PetId) {
    let today = env.today;
    let units = project.info.units;
    let Some(pet) = project.pet_mut(id) else { return };
    let n = pet.weights.len();
    theme::section(ui, ("pet-weight", id), icon::SCALES, "Weight", Some(n), n > 0, |ui| {
        let mut w = pet.weights.clone();
        w.sort_by_key(|e| e.date);
        if w.len() >= 2 {
            sparkline(ui, &w, units);
            ui.add_space(4.0);
        }
        // Add a reading.
        let key = ui.make_persistent_id(("pet-weight-new", id));
        let (mut date, mut kg): (NaiveDate, Option<f64>) = ui.data(|d| d.get_temp(key)).unwrap_or((today, None));
        let mut add = false;
        ui.horizontal(|ui| {
            date_edit(ui, &format!("pet-weight-date-{id}"), &mut date, today);
            if weight_edit(ui, ("pet-weight-kg", id), &mut kg, units, &format!("Weight ({})", weight_unit(units)), 110.0) && kg.is_some() {
                add = true;
            }
            if ui.add_enabled(kg.is_some(), egui::Button::new(format!("{}  Add", icon::PLUS))).clicked() {
                add = true;
            }
        });
        if add {
            if let Some(k) = kg {
                pet.weights.retain(|e| e.date != date);
                pet.weights.push(WeightEntry { date, kg: k });
                kg = None;
                date = today;
            }
        }
        ui.data_mut(|d| d.insert_temp(key, (date, kg)));

        let mut remove = None;
        for e in w.iter().rev().take(8) {
            ui.horizontal(|ui| {
                ui.label(theme::muted(fmt_date(e.date)));
                ui.label(theme::strong(format_weight(e.kg, units)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::icon_button_sized(ui, icon::X, "Remove this reading", false, 24.0).clicked() {
                        remove = Some(e.date);
                    }
                });
            });
        }
        if w.len() > 8 {
            ui.label(theme::muted(format!("And {} earlier readings.", w.len() - 8)));
        }
        if let Some(d) = remove {
            pet.weights.retain(|e| e.date != d);
        }
    });
}

/// A small chart of weight over time.
fn sparkline(ui: &mut Ui, w: &[WeightEntry], units: UnitSystem) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 96.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 10.0, c::field());
    // Room on the left for the weights, and below for the dates.
    let inner = egui::Rect::from_min_max(rect.min + vec2(58.0, 12.0), rect.max - vec2(14.0, 24.0));
    let (lo, hi) = w.iter().fold((f64::MAX, f64::MIN), |(lo, hi), e| (lo.min(e.kg), hi.max(e.kg)));
    let pad = ((hi - lo) * 0.15).max(0.1);
    let (lo, hi) = (lo - pad, hi + pad);
    let (t0, t1) = (w[0].date, w[w.len() - 1].date);
    let span = (t1 - t0).num_days().max(1) as f32;
    let at = |e: &WeightEntry| {
        let x = inner.left() + inner.width() * ((e.date - t0).num_days() as f32 / span);
        let y = inner.bottom() - inner.height() * ((e.kg - lo) / (hi - lo)) as f32;
        egui::pos2(x, y)
    };
    let pts: Vec<egui::Pos2> = w.iter().map(at).collect();
    p.add(egui::Shape::line(pts.clone(), egui::Stroke::new(2.0_f32, c::forest())));
    for q in &pts {
        p.circle_filled(*q, 3.0, c::forest());
    }
    let font = egui::FontId::proportional(11.0);
    p.text(egui::pos2(rect.left() + 8.0, inner.top()), egui::Align2::LEFT_CENTER, format_weight(hi - pad, units), font.clone(), c::muted());
    p.text(egui::pos2(rect.left() + 8.0, inner.bottom()), egui::Align2::LEFT_CENTER, format_weight(lo + pad, units), font.clone(), c::muted());
    p.text(egui::pos2(inner.left(), rect.bottom() - 2.0), egui::Align2::LEFT_BOTTOM, t0.format("%b %Y").to_string(), font.clone(), c::faint());
    p.text(egui::pos2(inner.right(), rect.bottom() - 2.0), egui::Align2::RIGHT_BOTTOM, t1.format("%b %Y").to_string(), font, c::faint());
}

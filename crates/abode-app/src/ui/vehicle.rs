//! The panel sections for a vehicle: what it is, its odometer, ownership,
//! paperwork, warranties, tires and parts. (Its notes, service history and
//! reminders are the usual history sections, shown after these.)
//!
//! Section titles are also used by the Reminders page to jump straight to
//! the right place, so keep them in step with `reminders.rs`.

use super::notes::attachments_ui;
use super::widgets::*;
use super::{Action, AttachTarget, Env};
use crate::theme::{self, c, icon};
use abode_core::units::UnitSystem;
use abode_core::vehicle::*;
use abode_core::*;
use chrono::NaiveDate;
use eframe::egui::{self, Ui};

const BODY_STYLES: &[&str] = &["Sedan", "Hatchback", "SUV / crossover", "Pickup truck", "Minivan", "Coupe", "Convertible", "Wagon", "Van", "Motorcycle", "Trailer", "RV", "Boat"];
const DRIVETRAINS: &[&str] = &["FWD", "RWD", "AWD", "4WD"];
const TRANSMISSIONS: &[&str] = &["Automatic", "Manual", "CVT", "Dual-clutch", "Single-speed (EV)"];

/// What else the vehicle panel needs from the project.
pub struct VehicleCtx {
    pub units: UnitSystem,
    /// Best known odometer reading (including later service records).
    pub odometer: Option<f64>,
    /// EV chargers placed anywhere: (id, name).
    pub chargers: Vec<(ObjectId, String)>,
    /// It already has the seasonal tire-swap reminders.
    pub has_swaps: bool,
}

/// Asked for by the panel; done by the caller, which can reach the project.
#[derive(Default)]
pub struct VehicleRequests {
    pub add_tire_swaps: bool,
}

/// Everything specific to a vehicle.
pub fn vehicle_sections(ui: &mut Ui, o: &mut PlacedObject, env: &mut Env, cx: &VehicleCtx) -> VehicleRequests {
    let mut out = VehicleRequests::default();
    let (units, odometer, today) = (cx.units, cx.odometer, env.today);
    let id = o.id;
    let details = &mut o.details;
    let v = o.vehicle.get_or_insert_with(Default::default);

    theme::section(ui, ("veh-main", id), icon::CAR, "Vehicle", None, true, |ui| {
        form_grid(ui, ("veh-grid", id), |ui| {
            text_row(ui, "Year", &mut v.year, "e.g. 2021");
            text_row(ui, "Make", &mut details.make, "e.g. Toyota");
            text_row(ui, "Model", &mut details.model, "e.g. RAV4");
            text_row(ui, "Trim", &mut v.trim, "e.g. XLE");
            field_label(ui, "Body");
            suggest_edit(ui, ("veh-body", id), &mut v.body, BODY_STYLES, "");
            ui.end_row();
            text_row(ui, "Colour", &mut v.color, "e.g. Lunar Rock");
            text_row(ui, "VIN", &mut v.vin, "17 characters, on the dash or door jamb");
            text_row(ui, "Plate", &mut v.plate, "");
            text_row(ui, "Registered in", &mut v.plate_region, "State or province");
            field_label(ui, "Fuel");
            combo_named(ui, ("veh-fuel", id), &mut v.fuel, &mut v.fuel_name, &Fuel::ALL, Fuel::label, Fuel::Other);
            ui.end_row();
            text_row(ui, "Engine", &mut v.engine, if v.fuel == Fuel::Electric { "e.g. 75 kWh battery" } else { "e.g. 2.5L 4-cylinder" });
            field_label(ui, "Transmission");
            suggest_edit(ui, ("veh-trans", id), &mut v.transmission, TRANSMISSIONS, "");
            ui.end_row();
            field_label(ui, "Drivetrain");
            suggest_edit(ui, ("veh-drive", id), &mut v.drivetrain, DRIVETRAINS, "");
            ui.end_row();
            text_row(ui, "Main driver", &mut v.driver, "");
            text_row(ui, "Keys", &mut v.keys, "e.g. 2 fobs, spare in the kitchen drawer");
        });
        ui.add_space(6.0);
        ui.label(theme::caps("Custom fields"));
        custom_fields(ui, ("veh-custom", id), &mut details.custom_fields);
    });

    theme::section(ui, ("veh-odo", id), icon::GAUGE, "Odometer", None, true, |ui| {
        form_grid(ui, ("veh-odo-grid", id), |ui| {
            field_label(ui, "Reading");
            if distance_edit(ui, ("veh-odo-now", id), &mut v.odometer, units, "e.g. 42,000", 140.0) {
                v.odometer_date = v.odometer.map(|_| today);
            }
            ui.end_row();
        });
        if let (Some(d), Some(_)) = (v.odometer_date, v.odometer) {
            ui.label(theme::muted(format!("Entered {}.", fmt_date(d))));
        }
        if let (Some(best), Some(entered)) = (odometer, v.odometer) {
            if best > entered + 0.5 {
                ui.label(theme::muted(format!("A service record shows {}, so that's used for reminders.", format_distance(best, units))));
            }
        }
        if let (Some(now), Some(start)) = (odometer, v.purchase_km) {
            if now > start {
                ui.label(theme::muted(format!("Driven {} since you bought it.", format_distance(now - start, units))));
            }
        }
        ui.label(theme::muted("Update it now and then (or add it to service records) so odometer reminders and warranty limits stay accurate."));
    });

    theme::section(ui, ("veh-own", id), icon::RECEIPT, "Purchase & ownership", None, false, |ui| {
        form_grid(ui, ("veh-own-grid", id), |ui| {
            field_label(ui, "Purchased");
            opt_date_edit(ui, &format!("veh-bought-{id}"), &mut details.purchased, today);
            ui.end_row();
            text_row(ui, "Price", &mut details.price, "");
            text_row(ui, "Bought from", &mut details.vendor, "Dealer or seller");
            field_label(ui, "Odometer then");
            distance_edit(ui, ("veh-bought-km", id), &mut v.purchase_km, units, "", 140.0);
            ui.end_row();
            field_label(ui, "Ownership");
            combo(ui, ("veh-ownership", id), &mut v.ownership, &Ownership::ALL, Ownership::label);
            ui.end_row();
            if v.ownership != Ownership::Owned {
                let leased = v.ownership == Ownership::Leased;
                text_row(ui, if leased { "Leasing company" } else { "Lender" }, &mut v.lender, "");
                text_row(ui, "Account #", &mut v.account, "");
                text_row(ui, "Payment", &mut v.payment, "e.g. $450 / month");
                field_label(ui, if leased { "Lease ends" } else { "Paid off" });
                opt_date_edit(ui, &format!("veh-term-{id}"), &mut v.term_ends, today);
                ui.end_row();
                if leased {
                    field_label(ui, "Distance allowed");
                    distance_edit(ui, ("veh-lease-km", id), &mut v.lease_km, units, "Over the whole lease", 140.0);
                    ui.end_row();
                }
            }
            text_row(ui, "Service dealer", &mut v.dealer, "Where it's serviced");
            text_row(ui, "Dealer phone", &mut v.dealer_phone, "");
        });
        if let (Ownership::Leased, Some(allowed), Some(now)) = (v.ownership, v.lease_km, odometer) {
            let used = now - v.purchase_km.unwrap_or(0.0);
            let left = allowed - used;
            let text = if left >= 0.0 {
                format!("{} of the lease allowance left.", format_distance(left, units))
            } else {
                format!("{} over the lease allowance.", format_distance(-left, units))
            };
            ui.label(theme::muted(text));
        }
    });

    theme::section(ui, ("veh-papers", id), icon::IDENTIFICATION_CARD, "Registration & insurance", None, false, |ui| {
        form_grid(ui, ("veh-papers-grid", id), |ui| {
            field_label(ui, "Registration renews");
            opt_date_edit(ui, &format!("veh-reg-{id}"), &mut v.registration_expires, today);
            ui.end_row();
            field_label(ui, "Inspection due");
            opt_date_edit(ui, &format!("veh-insp-{id}"), &mut v.inspection_due, today);
            ui.end_row();
            text_row(ui, "Insurer", &mut v.insurer, "");
            text_row(ui, "Policy #", &mut v.policy, "");
            field_label(ui, "Insurance renews");
            opt_date_edit(ui, &format!("veh-ins-{id}"), &mut v.insurance_renews, today);
            ui.end_row();
            text_row(ui, "Claims phone", &mut v.insurance_phone, "");
            text_row(ui, "Roadside assistance", &mut v.roadside_phone, "Phone or membership #");
        });
        ui.label(theme::muted("Renewal dates show up on the Reminders page under Warranties & renewals."));
    });

    let n = v.documents.len();
    theme::section(ui, ("veh-docs", id), icon::FILES, "Documents", Some(n), n > 0, |ui| {
        ui.label(theme::muted("Registration, insurance card, owner's manual, bill of sale, warranty booklets: photos or PDFs."));
        attachments_ui(ui, env, &mut v.documents, AttachTarget::Vehicle(id));
    });

    if v.fuel.plugs_in() {
        theme::section(ui, ("veh-charge", id), icon::CHARGING_STATION, "Charging", None, true, |ui| {
            form_grid(ui, ("veh-charge-grid", id), |ui| {
                text_row(ui, "Battery", &mut v.battery, "e.g. 75 kWh");
                text_row(ui, "Battery health", &mut v.battery_health, "e.g. 94% (dealer report, Oct 2026)");
                text_row(ui, "Range", &mut v.range, "e.g. 430 km, ~350 km in winter");
                field_label(ui, "Charging port");
                suggest_edit(ui, ("veh-port", id), &mut v.connector, CONNECTORS, "");
                ui.end_row();
                text_row(ui, "Cable & adapters", &mut v.cable, "e.g. Mobile connector + J1772 adapter, in the trunk");
                field_label(ui, "Charges at");
                let current = v.charger.and_then(|c| cx.chargers.iter().find(|(i, _)| *i == c)).map(|(_, n)| n.clone());
                ui.horizontal(|ui| {
                    let named = v.charger.is_none() && !v.charger_name.trim().is_empty();
                    let text = current.clone().or_else(|| named.then(|| v.charger_name.trim().to_string())).unwrap_or_else(|| "Not set".into());
                    if cx.chargers.len() + 1 > SEARCH_OVER {
                        let choices: Vec<Choice> = std::iter::once(Choice::new("Not set")).chain(cx.chargers.iter().map(|(_, n)| Choice::new(n.clone()))).collect();
                        let selected = match v.charger {
                            None if named => None,
                            None => Some(0),
                            Some(c) => cx.chargers.iter().position(|(i, _)| *i == c).map(|p| p + 1),
                        };
                        match search_combo(ui, ("veh-charger", id), text, &choices, selected, Some("Use"), 0.0) {
                            Some(Picked::Item(i)) => {
                                v.charger = if i == 0 { None } else { Some(cx.chargers[i - 1].0) };
                                v.charger_name.clear();
                            }
                            // A charger somewhere else ("Work"), by name.
                            Some(Picked::Custom(name)) => {
                                v.charger = None;
                                v.charger_name = name;
                            }
                            None => {}
                        }
                    } else {
                        theme::combo_box(("veh-charger", id)).selected_text(text).show_ui(ui, |ui| {
                            if ui.selectable_label(v.charger.is_none() && !named, "Not set").clicked() {
                                v.charger = None;
                                v.charger_name.clear();
                            }
                            for (cid, name) in &cx.chargers {
                                if ui.selectable_value(&mut v.charger, Some(*cid), name).clicked() {
                                    v.charger_name.clear();
                                }
                            }
                        });
                    }
                    if let (Some(c), Some(_)) = (v.charger, &current) {
                        if theme::icon_button_sized(ui, icon::ARROW_SQUARE_OUT, "Open the charger", false, 28.0).clicked() {
                            env.actions.push(Action::Reveal(EntityRef::Object(c)));
                        }
                    }
                });
                ui.end_row();
            });
            if cx.chargers.is_empty() {
                ui.label(theme::muted(format!(
                    "Place an EV charger with {} Item (outside, or in the garage) to pick it here.",
                    icon::PACKAGE
                )));
            }
        });
    }

    // Expired warranties are kept but hidden until asked for.
    let expired = |w: &VehicleWarranty| matches!(w.left(today, odometer), WarrantyLeft::Expired);
    let n_expired = v.warranties.iter().filter(|w| expired(w)).count();
    let n = v.warranties.len() - n_expired;
    theme::section(ui, ("veh-warranties", id), icon::SHIELD_CHECK, "Warranties", Some(n), n > 0, |ui| {
        let show_id = ui.make_persistent_id(("veh-show-expired", id));
        let show_expired: bool = ui.data(|d| d.get_temp(show_id)).unwrap_or(false);
        let mut remove = None;
        for (i, w) in v.warranties.iter_mut().enumerate() {
            if !show_expired && expired(w) {
                continue;
            }
            let title = if w.name.trim().is_empty() { "Warranty".to_string() } else { w.name.clone() };
            list_card(ui, ("veh-w", id, i), &title, Some(warranty_chip(w, today, odometer, units)), |ui| {
                form_grid(ui, ("veh-w-grid", id, i), |ui| {
                    field_label(ui, "Coverage");
                    suggest_edit(ui, ("veh-w-name", id, i), &mut w.name, WARRANTY_KINDS, "");
                    ui.end_row();
                    text_row(ui, "Provider", &mut w.provider, "Manufacturer, dealer or insurer");
                    text_row(ui, "Contract #", &mut w.contract, "");
                    field_label(ui, "Starts");
                    opt_date_edit(ui, &format!("veh-w-start-{id}-{i}"), &mut w.starts, today);
                    ui.end_row();
                    field_label(ui, "Ends");
                    opt_date_edit(ui, &format!("veh-w-end-{id}-{i}"), &mut w.expires, today);
                    ui.end_row();
                    field_label(ui, "Or at");
                    distance_edit(ui, ("veh-w-km", id, i), &mut w.until_km, units, "Odometer limit", 140.0);
                    ui.end_row();
                    text_row(ui, "Deductible", &mut w.deductible, "");
                    text_row(ui, "Claims phone", &mut w.phone, "");
                });
                ui.add(egui::TextEdit::multiline(&mut w.details).hint_text("What's covered, exclusions, transferable?").desired_rows(2).desired_width(f32::INFINITY));
                if danger_button(ui, "Remove warranty") {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            v.warranties.remove(i);
        }
        if n == 0 && n_expired > 0 && !show_expired {
            ui.label(theme::muted("No current warranties."));
        }
        if n_expired > 0 {
            let noun = if n_expired == 1 { "warranty" } else { "warranties" };
            let text = if show_expired { format!("{}  Hide expired", icon::EYE_SLASH) } else { format!("{}  Show {n_expired} expired {noun}", icon::EYE) };
            if theme::soft_button(ui, text).clicked() {
                ui.data_mut(|d| d.insert_temp(show_id, !show_expired));
            }
        }
        menu_dropdown(ui, egui::RichText::new(format!("{}  Add warranty", icon::PLUS)).color(c::forest()), |ui| {
            ui.set_min_width(260.0);
            let choices: Vec<Choice> = WARRANTY_KINDS.iter().map(|k| Choice::new(*k)).collect();
            let name = match search_list(ui, ui.make_persistent_id(("veh-add-warranty", id)), &choices, None, Some("Add")) {
                Some(Picked::Item(i)) => Some(WARRANTY_KINDS[i].to_string()),
                Some(Picked::Custom(n)) => Some(n),
                None => None,
            };
            if let Some(name) = name {
                v.warranties.push(VehicleWarranty { name, starts: details.purchased, ..Default::default() });
                close_dropdown(ui);
            }
        });
    });

    let n = v.tires.len();
    theme::section(ui, ("veh-tires", id), icon::TIRE, "Tires", Some(n), n > 0, |ui| {
        let mut remove = None;
        let mut mount = None;
        for (i, t) in v.tires.iter_mut().enumerate() {
            let mut title = t.season.label().to_string();
            let name = [t.brand.trim(), t.model.trim()].join(" ");
            if !name.trim().is_empty() {
                title = format!("{title} · {}", name.trim());
            }
            let chip = if t.mounted { ("On the vehicle".to_string(), c::forest(), c::mint_soft()) } else { ("In storage".to_string(), c::muted(), c::field()) };
            list_card(ui, ("veh-t", id, i), &title, Some(chip), |ui| {
                form_grid(ui, ("veh-t-grid", id, i), |ui| {
                    field_label(ui, "Season");
                    combo(ui, ("veh-t-season", id, i), &mut t.season, &TireSeason::ALL, TireSeason::label);
                    ui.end_row();
                    text_row(ui, "Brand", &mut t.brand, "e.g. Michelin");
                    text_row(ui, "Model", &mut t.model, "e.g. CrossClimate 2");
                    text_row(ui, "Size", &mut t.size, "e.g. 225/65R17 102H (on the sidewall)");
                    text_row(ui, "DOT date", &mut t.dot_code, "Week and year made, e.g. 2423");
                    field_label(ui, "Purchased");
                    opt_date_edit(ui, &format!("veh-t-bought-{id}-{i}"), &mut t.purchased, today);
                    ui.end_row();
                    text_row(ui, "Price", &mut t.price, "");
                    text_row(ui, "Bought from", &mut t.vendor, "");
                    field_label(ui, "Installed");
                    opt_date_edit(ui, &format!("veh-t-on-{id}-{i}"), &mut t.installed, today);
                    ui.end_row();
                    field_label(ui, "Odometer then");
                    distance_edit(ui, ("veh-t-km", id, i), &mut t.installed_km, units, "", 140.0);
                    ui.end_row();
                    field_label(ui, "Treadwear warranty");
                    distance_edit(ui, ("veh-t-wkm", id, i), &mut t.warranty_km, units, "e.g. 80,000", 140.0);
                    ui.end_row();
                    text_row(ui, "Tread depth", &mut t.tread_depth, "e.g. 7/32\" on Oct 2026");
                    text_row(ui, "Rims", &mut t.rims, "e.g. Own steel rims");
                    if !t.mounted {
                        text_row(ui, "Stored at", &mut t.stored_at, "e.g. Garage loft, tire shop");
                    }
                });
                if let (true, Some(now), Some(start)) = (t.mounted, odometer, t.installed_km) {
                    if now > start {
                        let mut s = format!("Driven {} on these.", format_distance(now - start, units));
                        if let Some(w) = t.warranty_km {
                            s.push_str(&format!(" Treadwear warranty: {} to go.", format_distance((w - (now - start)).max(0.0), units)));
                        }
                        ui.label(theme::muted(s));
                    }
                }
                ui.add(egui::TextEdit::multiline(&mut t.notes).hint_text("Notes").desired_rows(1).desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    if !t.mounted && theme::secondary_button(ui, format!("{}  Put these on", icon::ARROWS_CLOCKWISE)).on_hover_text("Marks these as on the vehicle and the others as in storage").clicked() {
                        mount = Some(i);
                    }
                    if danger_button(ui, "Remove set") {
                        remove = Some(i);
                    }
                });
            });
        }
        if let Some(i) = mount {
            for (j, t) in v.tires.iter_mut().enumerate() {
                t.mounted = i == j;
            }
            let t = &mut v.tires[i];
            t.installed = Some(today);
            t.installed_km = odometer;
        }
        if let Some(i) = remove {
            v.tires.remove(i);
        }
        ui.horizontal_wrapped(|ui| {
            if theme::soft_button(ui, format!("{}  Add tire set", icon::PLUS)).clicked() {
                let first = v.tires.is_empty();
                v.tires.push(TireSet { mounted: first, ..Default::default() });
            }
            if !cx.has_swaps
                && v.tires.len() > 1
                && theme::secondary_button(ui, format!("{}  Remind me to swap", icon::BELL))
                    .on_hover_text("Yearly reminders: winter tires on Nov 1, summer tires on Apr 15")
                    .clicked()
            {
                out.add_tire_swaps = true;
            }
        });
        ui.label(theme::muted("Keep winter and summer sets here; Put these on swaps which set is on the vehicle and starts their distance from today's odometer."));
    });

    let n = v.parts.len();
    theme::section(ui, ("veh-parts", id), icon::ENGINE, "Parts & fluids", Some(n), n > 0, |ui| {
        let mut remove = None;
        for (i, p) in v.parts.iter_mut().enumerate() {
            let title = if p.name.trim().is_empty() { "Part".to_string() } else { p.name.clone() };
            let summary = [p.spec.trim(), p.part_number.trim()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ");
            // Keep the summary short so the name stays readable.
            let summary = if summary.chars().count() > 22 { format!("{}…", summary.chars().take(21).collect::<String>().trim_end()) } else { summary };
            let chip = (!summary.is_empty()).then(|| (summary, c::muted(), c::field()));
            list_card(ui, ("veh-p", id, i), &title, chip, |ui| {
                let hint = COMMON_PARTS.iter().find(|(n, _)| *n == p.name).map_or("", |(_, h)| *h);
                form_grid(ui, ("veh-p-grid", id, i), |ui| {
                    text_row(ui, "Name", &mut p.name, "");
                    text_row(ui, "Spec", &mut p.spec, hint);
                    text_row(ui, "Part #", &mut p.part_number, "");
                    text_row(ui, "Capacity / qty", &mut p.capacity, "e.g. 4.8 qt, 2 bulbs");
                    field_label(ui, "Last replaced");
                    opt_date_edit(ui, &format!("veh-p-date-{id}-{i}"), &mut p.last_replaced, today);
                    ui.end_row();
                    field_label(ui, "Odometer then");
                    distance_edit(ui, ("veh-p-km", id, i), &mut p.last_km, units, "", 140.0);
                    ui.end_row();
                });
                ui.add(egui::TextEdit::multiline(&mut p.notes).hint_text("Notes").desired_rows(1).desired_width(f32::INFINITY));
                if danger_button(ui, "Remove") {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            v.parts.remove(i);
        }
        ui.horizontal_wrapped(|ui| {
            menu_dropdown(ui, egui::RichText::new(format!("{}  Add part or fluid", icon::PLUS)).color(c::forest()), |ui| {
                ui.set_min_width(260.0);
                let choices: Vec<Choice> = COMMON_PARTS.iter().map(|(name, _)| Choice::new(*name).enabled(!v.parts.iter().any(|p| p.name == *name))).collect();
                let name = match search_list(ui, ui.make_persistent_id(("veh-add-part", id)), &choices, None, Some("Add")) {
                    Some(Picked::Item(i)) => Some(COMMON_PARTS[i].0.to_string()),
                    Some(Picked::Custom(n)) => Some(n),
                    None => None,
                };
                if let Some(name) = name {
                    v.parts.push(VehiclePart { name, ..Default::default() });
                    close_dropdown(ui);
                }
            });
        });
        ui.label(theme::muted("What it takes (oil grade, filter and bulb numbers, wiper sizes) so the next trip to the parts store is quick. For when they're due, add a reminder below."));
    });
    out
}

/// A collapsible card in a list (a warranty, a tire set, a part).
pub(crate) fn list_card(ui: &mut Ui, id_salt: impl std::hash::Hash, title: &str, chip: Option<(String, egui::Color32, egui::Color32)>, body: impl FnOnce(&mut Ui)) {
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        let id = ui.make_persistent_id(id_salt);
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
            .show_header(ui, |ui| {
                // Badge on the right, then the title in what's left (cut
                // short rather than widening the panel).
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

fn warranty_chip(w: &VehicleWarranty, today: NaiveDate, odometer: Option<f64>, units: UnitSystem) -> (String, egui::Color32, egui::Color32) {
    match w.left(today, odometer) {
        WarrantyLeft::Expired => ("Expired".into(), c::muted(), c::field()),
        WarrantyLeft::Unknown => ("Add an end date".into(), c::muted(), c::field()),
        WarrantyLeft::Active { days, km } => {
            let mut parts = Vec::new();
            if let Some(d) = days {
                parts.push(if d > 365 { format!("{:.1} years", d as f64 / 365.25) } else { format!("{d} days") });
            }
            if let Some(k) = km {
                parts.push(format_distance(k, units));
            }
            let soon = days.is_some_and(|d| d <= 90) || km.is_some_and(|k| k < 2000.0);
            let (fg, bg) = if soon { (c::amber(), c::amber_soft()) } else { (c::forest(), c::mint_soft()) };
            (format!("{} left", parts.join(" or ")), fg, bg)
        }
    }
}

// ---------------------------------------------------------------------------
// The Vehicles page: every vehicle at a glance.

enum PageEvent {
    /// Open it in the panel, at these sections.
    Reveal(EntityRef, &'static [&'static str]),
    SetOdometer(ObjectId, Option<f64>),
}

/// One fact on a vehicle's card: a heading, a line, and maybe a chip.
struct Fact {
    title: &'static str,
    line: String,
    chip: Option<(String, egui::Color32, egui::Color32)>,
    sections: &'static [&'static str],
}

pub fn vehicles_ui(ui: &mut Ui, project: &mut Project, env: &mut Env) {
    let today = env.today;
    let units = project.info.units;
    ui.add(
        egui::Label::new(theme::muted(
            "Every vehicle, what's due next and what needs renewing. Click one for everything else: warranties, tires, parts, documents and service history.",
        ))
        .wrap(),
    );
    ui.add_space(8.0);
    let cars = project.vehicles();
    let mut event = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        if cars.is_empty() {
            theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(theme::strong("No vehicles yet"));
                ui.label(theme::muted(format!(
                    "Add a car, truck, motorcycle or trailer with {} Item: on the Outside plan (on the driveway, or under a carport), or on a garage's floor plan.",
                    icon::PACKAGE
                )));
                if let Some(site) = project.site_id() {
                    if theme::secondary_button(ui, format!("{}  Go to the Outside plan", icon::TREE)).clicked() {
                        event = Some(PageEvent::Reveal(EntityRef::Level(site), &[]));
                    }
                }
            });
        }
        for (li, id) in &cars {
            vehicle_card(ui, project, *li, *id, units, today, &mut event);
        }
    });
    match event {
        Some(PageEvent::Reveal(r, sections)) => {
            theme::focus_section(ui.ctx(), sections);
            env.actions.push(Action::Reveal(r));
        }
        Some(PageEvent::SetOdometer(id, km)) => {
            if let Some(v) = project.levels.iter_mut().find_map(|l| l.objects.get_mut(&id)).and_then(|o| o.vehicle.as_mut()) {
                v.odometer = km;
                v.odometer_date = km.map(|_| today);
            }
        }
        None => {}
    }
}

fn vehicle_card(ui: &mut Ui, project: &Project, li: usize, id: ObjectId, units: UnitSystem, today: NaiveDate, event: &mut Option<PageEvent>) {
    let level = &project.levels[li];
    let o = &level.objects[&id];
    let Some(v) = o.vehicle.as_deref() else { return };
    let target = EntityRef::Object(id);
    let odometer = project.odometer(id);
    let place = match (o.feature, o.room) {
        (Some(f), _) => project.entity_name(EntityRef::Feature(f)),
        (_, Some(r)) => project.entity_name(EntityRef::Room(r)),
        _ => project.entity_name(EntityRef::Level(level.id)),
    };
    let facts = facts_for(project, id, v, odometer, units, today);

    let card = ui.scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                theme::icon_tile(ui, theme::object_icon(o), c::forest(), c::mint_soft());
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(egui::RichText::new(o.display_name()).family(theme::serif()).size(20.0).color(c::ink()));
                    let mut sub = vec![place.clone()];
                    if !v.plate.trim().is_empty() {
                        sub.push(v.plate.trim().to_string());
                    }
                    if !v.driver.trim().is_empty() {
                        sub.push(v.driver.trim().to_string());
                    }
                    ui.label(theme::muted(sub.join("  ·  ")));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut km = v.odometer.or(odometer);
                    if distance_edit(ui, ("page-odo", id), &mut km, units, "Not set", 120.0) {
                        *event = Some(PageEvent::SetOdometer(id, km));
                    }
                    ui.label(theme::muted("Odometer"));
                });
            });
            ui.add_space(6.0);
            let cols = if ui.available_width() > 1150.0 { facts.len() } else if ui.available_width() > 860.0 { 4 } else { 2 };
            let w = (ui.available_width() - 12.0 * (cols as f32 - 1.0)) / cols as f32;
            egui::Grid::new(("page-facts", id)).num_columns(cols).spacing([12.0, 10.0]).min_col_width(w).max_col_width(w).show(ui, |ui| {
                for (i, f) in facts.iter().enumerate() {
                    let r = ui.scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
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
                        *event = Some(PageEvent::Reveal(target, f.sections));
                    }
                    if (i + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
        });
    });
    if card.response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && event.is_none() {
        *event = Some(PageEvent::Reveal(target, &[]));
    }
    ui.add_space(4.0);
}

fn chip_for_days(days: i64) -> (String, egui::Color32, egui::Color32) {
    if days < 0 {
        (format!("Overdue {}d", -days), c::coral_dark(), c::coral_soft())
    } else if days <= 30 {
        (if days == 0 { "Today".to_string() } else { format!("In {days} days") }, c::amber(), c::amber_soft())
    } else if days > 365 {
        (format!("In {:.1} years", days as f64 / 365.25), c::forest(), c::mint_soft())
    } else {
        (format!("In {} days", days), c::forest(), c::mint_soft())
    }
}

/// Next reminder, next renewal, warranty, and tires (or charging).
fn facts_for(project: &Project, id: ObjectId, v: &Vehicle, odometer: Option<f64>, units: UnitSystem, today: NaiveDate) -> Vec<Fact> {
    use abode_core::reminders::DueStatus;
    let mut facts = Vec::new();

    // What's due next: the most pressing open reminder.
    let next = project
        .reminders_for(EntityRef::Object(id))
        .filter(|r| !r.is_done() && !r.is_snoozed(today))
        .map(|r| (project.reminder_status(r, today), r))
        .max_by(|(a, ra), (b, rb)| a.urgency().cmp(&b.urgency()).then(rb.effective_due().cmp(&ra.effective_due())));
    facts.push(match next {
        Some((status, r)) => {
            let km = |k: i64| format_distance(k as f64, units);
            let chip = match status {
                DueStatus::Overdue(d) => (format!("Overdue {d}d"), c::coral_dark(), c::coral_soft()),
                DueStatus::OverKm(k) => (format!("Overdue {}", km(k)), c::coral_dark(), c::coral_soft()),
                DueStatus::DueSoon(d) => chip_for_days(d),
                DueStatus::SoonKm(k) => (format!("In {}", km(k)), c::amber(), c::amber_soft()),
                DueStatus::LaterKm(0) => ("Needs odometer".into(), c::muted(), c::field()),
                DueStatus::LaterKm(k) => (format!("In {}", km(k)), c::forest(), c::mint_soft()),
                DueStatus::Upcoming(d) => chip_for_days(d),
                DueStatus::Done | DueStatus::Snoozed(_) => (String::new(), c::muted(), c::field()),
            };
            let title = if r.title.trim().is_empty() { "(untitled)".to_string() } else { r.title.clone() };
            Fact { title: "Next up", line: title, chip: Some(chip), sections: &["Reminders"] }
        }
        None => Fact { title: "Next up", line: "No reminders".into(), chip: Some(("Add one".into(), c::muted(), c::field())), sections: &["Reminders"] },
    });

    // The next renewal.
    let term = match v.ownership {
        Ownership::Leased => "Lease ends",
        Ownership::Financed => "Loan paid off",
        Ownership::Owned => "",
    };
    let renewals = [
        (v.registration_expires, "Registration", &["Registration & insurance"][..]),
        (v.insurance_renews, "Insurance", &["Registration & insurance"][..]),
        (v.inspection_due, "Inspection", &["Registration & insurance"][..]),
        (if term.is_empty() { None } else { v.term_ends }, term, &["Purchase & ownership"][..]),
    ];
    let soonest = renewals.iter().filter_map(|(d, what, s)| d.map(|d| (d, *what, *s))).min_by_key(|(d, _, _)| *d);
    facts.push(match soonest {
        Some((d, what, sections)) => Fact { title: "Next renewal", line: format!("{what} · {}", fmt_date(d)), chip: Some(chip_for_days((d - today).num_days())), sections },
        None => Fact { title: "Next renewal", line: "No dates yet".into(), chip: Some(("Add them".into(), c::muted(), c::field())), sections: &["Registration & insurance"] },
    });

    // Warranty: the active one that ends soonest.
    let active: Vec<(&VehicleWarranty, Option<i64>, Option<f64>)> = v
        .warranties
        .iter()
        .filter_map(|w| match w.left(today, odometer) {
            WarrantyLeft::Active { days, km } => Some((w, days, km)),
            _ => None,
        })
        .collect();
    let soonest = active.iter().min_by_key(|(_, d, _)| d.unwrap_or(i64::MAX));
    facts.push(match soonest {
        Some((w, days, km)) => {
            let mut left = Vec::new();
            if let Some(d) = days {
                left.push(if *d > 365 { format!("{:.1} years", *d as f64 / 365.25) } else { format!("{d} days") });
            }
            if let Some(k) = km {
                left.push(format_distance(*k, units));
            }
            let soon = days.is_some_and(|d| d <= 90) || km.is_some_and(|k| k < 2000.0);
            let (fg, bg) = if soon { (c::amber(), c::amber_soft()) } else { (c::forest(), c::mint_soft()) };
            let name = if w.name.trim().is_empty() { "Warranty".to_string() } else { w.name.clone() };
            let more = if active.len() > 1 { format!(" (+{} more)", active.len() - 1) } else { String::new() };
            Fact { title: "Warranty", line: format!("{name}{more}"), chip: Some((format!("{} left", left.join(" or ")), fg, bg)), sections: &["Warranties"] }
        }
        None if v.warranties.is_empty() => Fact { title: "Warranty", line: "None added".into(), chip: None, sections: &["Warranties"] },
        None => Fact { title: "Warranty", line: format!("All {} expired", v.warranties.len()), chip: Some(("Expired".into(), c::muted(), c::field())), sections: &["Warranties"] },
    });

    // Tires, or charging for a plug-in.
    if v.fuel.plugs_in() {
        let charger = v.charger.and_then(|c| project.find_object(c)).map(|(_, o)| o.display_name());
        let line = match (&charger, v.connector.trim()) {
            (Some(c), "") => c.clone(),
            (Some(c), port) => format!("{c} · {port}"),
            (None, "") => "Charger not set".into(),
            (None, port) => port.to_string(),
        };
        let chip = (!v.range.trim().is_empty()).then(|| (v.range.trim().to_string(), c::muted(), c::field()));
        facts.push(Fact { title: "Charging", line, chip, sections: &["Charging"] });
    }
    let mounted = v.tires.iter().find(|t| t.mounted);
    let stored = v.tires.iter().find(|t| !t.mounted);
    facts.push(match mounted {
        Some(t) => {
            let name = [t.brand.trim(), t.model.trim()].join(" ").trim().to_string();
            let line = if name.is_empty() { t.season.label().to_string() } else { format!("{} · {name}", t.season.label()) };
            let chip = stored.map(|s| {
                let at = if s.stored_at.trim().is_empty() { "in storage".to_string() } else { format!("at {}", s.stored_at.trim()) };
                (format!("{} {at}", s.season.label()), c::muted(), c::field())
            });
            Fact { title: "Tires on it", line, chip, sections: &["Tires"] }
        }
        None => Fact { title: "Tires on it", line: "Not recorded".into(), chip: None, sections: &["Tires"] },
    });
    facts
}

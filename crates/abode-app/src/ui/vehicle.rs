//! The panel sections for a vehicle: what it is, its odometer, ownership,
//! paperwork, warranties, tires and parts. (Its notes, service history and
//! reminders are the usual history sections, shown after these.)
//!
//! Section titles are also used by the Reminders page to jump straight to
//! the right place, so keep them in step with `reminders.rs`.

use super::widgets::*;
use crate::theme::{self, c, icon};
use abode_core::units::UnitSystem;
use abode_core::vehicle::*;
use abode_core::*;
use chrono::NaiveDate;
use eframe::egui::{self, Ui};

const BODY_STYLES: &[&str] = &["Sedan", "Hatchback", "SUV / crossover", "Pickup truck", "Minivan", "Coupe", "Convertible", "Wagon", "Van", "Motorcycle", "Trailer", "RV", "Boat"];
const DRIVETRAINS: &[&str] = &["FWD", "RWD", "AWD", "4WD"];
const TRANSMISSIONS: &[&str] = &["Automatic", "Manual", "CVT", "Dual-clutch", "Single-speed (EV)"];

/// Everything specific to a vehicle. `odometer` is the best known reading
/// (including later service records).
pub fn vehicle_sections(ui: &mut Ui, o: &mut PlacedObject, units: UnitSystem, today: NaiveDate, odometer: Option<f64>) {
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
            combo(ui, ("veh-fuel", id), &mut v.fuel, &Fuel::ALL, Fuel::label);
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

    let n = v.warranties.len();
    theme::section(ui, ("veh-warranties", id), icon::SHIELD_CHECK, "Warranties", Some(n), n > 0, |ui| {
        let mut remove = None;
        for (i, w) in v.warranties.iter_mut().enumerate() {
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
        egui::menu::menu_button(ui, egui::RichText::new(format!("{}  Add warranty", icon::PLUS)).color(c::forest()), |ui| {
            for kind in WARRANTY_KINDS {
                if ui.button(*kind).clicked() {
                    v.warranties.push(VehicleWarranty { name: kind.to_string(), starts: details.purchased, ..Default::default() });
                    ui.close_menu();
                }
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
        if theme::soft_button(ui, format!("{}  Add tire set", icon::PLUS)).clicked() {
            let first = v.tires.is_empty();
            v.tires.push(TireSet { mounted: first, ..Default::default() });
        }
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
            egui::menu::menu_button(ui, egui::RichText::new(format!("{}  Add part or fluid", icon::PLUS)).color(c::forest()), |ui| {
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for (name, _) in COMMON_PARTS {
                        let have = v.parts.iter().any(|p| p.name == *name);
                        if ui.add_enabled(!have, egui::Button::new(*name)).clicked() {
                            v.parts.push(VehiclePart { name: name.to_string(), ..Default::default() });
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    if ui.button("Something else…").clicked() {
                        v.parts.push(VehiclePart::default());
                        ui.close_menu();
                    }
                });
            });
        });
        ui.label(theme::muted("What it takes (oil grade, filter and bulb numbers, wiper sizes) so the next trip to the parts store is quick. For when they're due, add a reminder below."));
    });
}

/// A collapsible card in a list (a warranty, a tire set, a part).
fn list_card(ui: &mut Ui, id_salt: impl std::hash::Hash, title: &str, chip: Option<(String, egui::Color32, egui::Color32)>, body: impl FnOnce(&mut Ui)) {
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

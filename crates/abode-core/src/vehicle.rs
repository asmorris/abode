//! Cars, trucks, motorcycles and trailers: everything an owner tracks about
//! one. A vehicle is a [`PlacedObject`] (category [`ObjectCategory::Vehicle`])
//! parked in a garage, under a carport or on the driveway, with a
//! [`Vehicle`] record hung off it. Its notes, service history and reminders
//! use the same machinery as everything else in the house.
//!
//! Distances are stored in kilometres and shown in miles for imperial
//! projects.

use crate::ids::*;
use crate::model::*;
use crate::units::UnitSystem;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

pub const KM_PER_MI: f64 = 1.609_344;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fuel {
    #[default]
    Gasoline,
    Diesel,
    Hybrid,
    PlugInHybrid,
    Electric,
    None,
    Other,
}

impl Fuel {
    pub const ALL: [Fuel; 7] = [Fuel::Gasoline, Fuel::Diesel, Fuel::Hybrid, Fuel::PlugInHybrid, Fuel::Electric, Fuel::None, Fuel::Other];
    pub fn label(self) -> &'static str {
        match self {
            Fuel::Gasoline => "Gasoline",
            Fuel::Diesel => "Diesel",
            Fuel::Hybrid => "Hybrid",
            Fuel::PlugInHybrid => "Plug-in hybrid",
            Fuel::Electric => "Electric",
            Fuel::None => "None (trailer)",
            Fuel::Other => "Other",
        }
    }
    pub fn is_electric(self) -> bool {
        matches!(self, Fuel::Electric | Fuel::PlugInHybrid | Fuel::Hybrid)
    }
    /// Charges from a plug (so it has a charging port, cable, charger...).
    pub fn plugs_in(self) -> bool {
        matches!(self, Fuel::Electric | Fuel::PlugInHybrid)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ownership {
    #[default]
    Owned,
    Financed,
    Leased,
}

impl Ownership {
    pub const ALL: [Ownership; 3] = [Ownership::Owned, Ownership::Financed, Ownership::Leased];
    pub fn label(self) -> &'static str {
        match self {
            Ownership::Owned => "Owned outright",
            Ownership::Financed => "Financed (loan)",
            Ownership::Leased => "Leased",
        }
    }
}

/// One warranty or protection plan. A vehicle usually has several (basic,
/// powertrain, corrosion...), each ending at a date or a distance,
/// whichever comes first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VehicleWarranty {
    pub name: String,
    pub provider: String,
    pub contract: String,
    pub starts: Option<NaiveDate>,
    pub expires: Option<NaiveDate>,
    /// Ends at this odometer reading (km).
    pub until_km: Option<f64>,
    pub deductible: String,
    pub phone: String,
    pub details: String,
}

/// Common warranties, for the "Add warranty" menu.
pub const WARRANTY_KINDS: &[&str] = &[
    "Basic (bumper-to-bumper)",
    "Powertrain",
    "Corrosion / rust",
    "Emissions",
    "Hybrid / EV battery",
    "Extended warranty",
    "Roadside assistance",
    "Tire & wheel protection",
    "Paint & interior protection",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TireSeason {
    #[default]
    AllSeason,
    AllWeather,
    Summer,
    Winter,
    Other,
}

impl TireSeason {
    pub const ALL: [TireSeason; 5] = [TireSeason::AllSeason, TireSeason::AllWeather, TireSeason::Summer, TireSeason::Winter, TireSeason::Other];
    pub fn label(self) -> &'static str {
        match self {
            TireSeason::AllSeason => "All-season",
            TireSeason::AllWeather => "All-weather",
            TireSeason::Summer => "Summer",
            TireSeason::Winter => "Winter",
            TireSeason::Other => "Other",
        }
    }
}

/// A set of tires: the ones on the car, or a seasonal set in storage.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TireSet {
    pub season: TireSeason,
    pub brand: String,
    pub model: String,
    /// e.g. 225/45R17 94V
    pub size: String,
    /// DOT date code (week and year made), e.g. 2423.
    pub dot_code: String,
    pub purchased: Option<NaiveDate>,
    pub price: String,
    pub vendor: String,
    pub installed: Option<NaiveDate>,
    pub installed_km: Option<f64>,
    /// On the vehicle now (otherwise in storage).
    pub mounted: bool,
    pub stored_at: String,
    /// On their own rims.
    pub rims: String,
    pub tread_depth: String,
    pub warranty_km: Option<f64>,
    pub notes: String,
}

/// A part or fluid: what it takes, and when it was last replaced.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VehiclePart {
    pub name: String,
    /// What it takes: oil grade, bulb type, battery group size...
    pub spec: String,
    pub part_number: String,
    pub capacity: String,
    pub last_replaced: Option<NaiveDate>,
    pub last_km: Option<f64>,
    pub notes: String,
}

/// Parts and fluids worth knowing, for the "Add part" menu: (name, a hint
/// for what goes in Spec).
pub const COMMON_PARTS: &[(&str, &str)] = &[
    ("Engine oil", "e.g. 0W-20 full synthetic"),
    ("Oil filter", "Part number"),
    ("Engine air filter", "Part number"),
    ("Cabin air filter", "Part number"),
    ("Wiper blades", "e.g. 26\" driver, 17\" passenger, 12\" rear"),
    ("12V battery", "Group size, e.g. H6 / 48"),
    ("Spark plugs", "e.g. Iridium, gap 0.044\""),
    ("Brake pads (front)", "Ceramic / semi-metallic"),
    ("Brake pads (rear)", "Ceramic / semi-metallic"),
    ("Brake rotors", ""),
    ("Brake fluid", "e.g. DOT 3, DOT 4"),
    ("Coolant", "e.g. Pink long-life"),
    ("Transmission fluid", "e.g. ATF WS, CVT NS-3"),
    ("Differential / transfer case fluid", "e.g. 75W-90"),
    ("Power steering fluid", ""),
    ("Washer fluid", "e.g. -40° winter"),
    ("Headlight bulbs", "e.g. H11 low, 9005 high"),
    ("Tail / brake light bulbs", "e.g. 7443"),
    ("Serpentine belt", "Part number"),
    ("Timing belt / chain", "Interval"),
    ("Key fob battery", "e.g. CR2032"),
    ("Tire pressure", "e.g. 35 psi front, 33 psi rear"),
    ("Lug nut torque", "e.g. 80 ft-lb"),
    ("Fuel", "e.g. Regular 87, Premium 91"),
];

/// A typical maintenance interval, for quick reminders. Real schedules vary
/// by vehicle: the owner's manual wins.
pub struct ScheduleTemplate {
    pub title: &'static str,
    pub every_km: f64,
    pub months: u32,
}

pub const COMMON_SCHEDULES: &[ScheduleTemplate] = &[
    ScheduleTemplate { title: "Oil & filter change", every_km: 8000.0, months: 6 },
    ScheduleTemplate { title: "Tire rotation", every_km: 10000.0, months: 6 },
    ScheduleTemplate { title: "Brake inspection", every_km: 20000.0, months: 12 },
    ScheduleTemplate { title: "Cabin air filter", every_km: 24000.0, months: 12 },
    ScheduleTemplate { title: "Engine air filter", every_km: 32000.0, months: 24 },
    ScheduleTemplate { title: "Wiper blades", every_km: 20000.0, months: 12 },
    ScheduleTemplate { title: "Battery test", every_km: 20000.0, months: 12 },
    ScheduleTemplate { title: "Brake fluid flush", every_km: 48000.0, months: 36 },
    ScheduleTemplate { title: "Transmission fluid", every_km: 96000.0, months: 60 },
    ScheduleTemplate { title: "Coolant flush", every_km: 160000.0, months: 60 },
    ScheduleTemplate { title: "Spark plugs", every_km: 160000.0, months: 96 },
];

/// Seasonal tire swaps: (title, month, day), each yearly.
pub const TIRE_SWAPS: &[(&str, u32, u32)] = &[("Swap to winter tires", 11, 1), ("Swap to summer tires", 4, 15)];

/// The next `month`/`day` on or after `today`.
pub fn next_yearly(today: NaiveDate, month: u32, day: u32) -> NaiveDate {
    use chrono::Datelike;
    let this = NaiveDate::from_ymd_opt(today.year(), month, day).unwrap_or(today);
    if this >= today { this } else { NaiveDate::from_ymd_opt(today.year() + 1, month, day).unwrap_or(today) }
}

impl PlacedObject {
    /// An EV charger (by its label: "EV charger", "Tesla Wall Connector charger"...).
    pub fn is_ev_charger(&self) -> bool {
        self.vehicle.is_none() && self.label.to_lowercase().contains("charger")
    }
}

/// Everything about one vehicle beyond the make/model/price/warranty an item
/// already has in [`ItemDetails`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Vehicle {
    // What it is
    pub year: String,
    pub trim: String,
    pub body: String,
    pub color: String,
    pub vin: String,
    pub plate: String,
    pub plate_region: String,
    pub fuel: Fuel,
    /// A typed-in fuel when `fuel` is Other.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fuel_name: String,
    pub engine: String,
    pub transmission: String,
    pub drivetrain: String,
    /// Who drives it.
    pub driver: String,

    // Odometer (km)
    pub odometer: Option<f64>,
    pub odometer_date: Option<NaiveDate>,
    pub purchase_km: Option<f64>,

    // Ownership
    pub ownership: Ownership,
    pub lender: String,
    pub account: String,
    pub payment: String,
    /// Loan paid off / lease ends.
    pub term_ends: Option<NaiveDate>,
    /// Lease distance allowance over the whole term (km).
    pub lease_km: Option<f64>,

    // Paperwork
    pub registration_expires: Option<NaiveDate>,
    pub inspection_due: Option<NaiveDate>,
    pub insurer: String,
    pub policy: String,
    pub insurance_renews: Option<NaiveDate>,
    pub insurance_phone: String,
    pub roadside_phone: String,
    pub dealer: String,
    pub dealer_phone: String,
    pub keys: String,

    pub warranties: Vec<VehicleWarranty>,
    pub tires: Vec<TireSet>,
    pub parts: Vec<VehiclePart>,

    /// Registration, insurance card, owner's manual, bill of sale...
    pub documents: Vec<AttachmentId>,

    // Charging (electric and plug-in hybrid)
    /// Battery capacity, e.g. "75 kWh".
    pub battery: String,
    /// e.g. "94% (Oct 2026 report)".
    pub battery_health: String,
    /// e.g. "430 km EPA, ~350 km in winter".
    pub range: String,
    /// The car's charging port, e.g. "NACS", "J1772 + CCS1".
    pub connector: String,
    /// Mobile / portable cable and adapters kept with it.
    pub cable: String,
    /// The EV charger it uses at home (a placed item).
    pub charger: Option<ObjectId>,
    /// A charger that isn't one of the placed items ("Work", "Public"), by name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub charger_name: String,
}

/// Charging ports and plugs, for suggestions.
pub const CONNECTORS: &[&str] = &["NACS (Tesla)", "J1772", "CCS1", "CCS2", "Type 2", "CHAdeMO", "GB/T"];

impl Vehicle {
    /// "2021 Toyota RAV4 XLE" from its year, the item's make/model, and trim.
    pub fn title(&self, details: &ItemDetails) -> String {
        [self.year.as_str(), details.make.as_str(), details.model.as_str(), self.trim.as_str()]
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// How a warranty stands today.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WarrantyLeft {
    /// Still covered: days and km left (whichever is known).
    Active { days: Option<i64>, km: Option<f64> },
    Expired,
    /// No end date or distance entered.
    Unknown,
}

impl VehicleWarranty {
    pub fn left(&self, today: NaiveDate, odometer: Option<f64>) -> WarrantyLeft {
        let days = self.expires.map(|e| (e - today).num_days());
        let km = match (self.until_km, odometer) {
            (Some(limit), Some(now)) => Some(limit - now),
            _ => None,
        };
        if days.is_some_and(|d| d < 0) || km.is_some_and(|k| k <= 0.0) {
            WarrantyLeft::Expired
        } else if days.is_none() && km.is_none() {
            WarrantyLeft::Unknown
        } else {
            WarrantyLeft::Active { days, km }
        }
    }
}

// ---------------------------------------------------------------------------
// Distances

pub fn km_to_display(km: f64, units: UnitSystem) -> f64 {
    match units {
        UnitSystem::Metric => km,
        UnitSystem::Imperial => km / KM_PER_MI,
    }
}

pub fn km_from_display(v: f64, units: UnitSystem) -> f64 {
    match units {
        UnitSystem::Metric => v,
        UnitSystem::Imperial => v * KM_PER_MI,
    }
}

pub fn distance_unit(units: UnitSystem) -> &'static str {
    match units {
        UnitSystem::Metric => "km",
        UnitSystem::Imperial => "mi",
    }
}

/// "12,345 km" / "7,671 mi".
pub fn format_distance(km: f64, units: UnitSystem) -> String {
    format!("{} {}", group_thousands(km_to_display(km, units).round() as i64), distance_unit(units))
}

pub fn group_thousands(n: i64) -> String {
    let s = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// Parse "12,345", "12345 km", "7,600 mi" (a unit overrides the project's).
pub fn parse_distance(input: &str, units: UnitSystem) -> Option<f64> {
    let s = input.trim().to_lowercase().replace([',', '_', ' '], "");
    if s.is_empty() {
        return None;
    }
    let (num, unit) = if let Some(n) = s.strip_suffix("km") {
        (n, Some(UnitSystem::Metric))
    } else if let Some(n) = s.strip_suffix("miles").or(s.strip_suffix("mi")) {
        (n, Some(UnitSystem::Imperial))
    } else if let Some(n) = s.strip_suffix('k') {
        return n.parse::<f64>().ok().filter(|v| *v >= 0.0).map(|v| km_from_display(v * 1000.0, units));
    } else {
        (s.as_str(), None)
    };
    let v: f64 = num.parse().ok()?;
    (v >= 0.0).then(|| km_from_display(v, unit.unwrap_or(units)))
}

// ---------------------------------------------------------------------------
// Project-level lookups

impl Project {
    pub fn vehicle(&self, id: ObjectId) -> Option<&Vehicle> {
        self.levels.iter().find_map(|l| l.objects.get(&id)).and_then(|o| o.vehicle.as_deref())
    }

    /// The best known odometer reading for a vehicle: the one entered on it,
    /// or a later service record's, whichever is higher.
    pub fn odometer(&self, id: ObjectId) -> Option<f64> {
        let v = self.vehicle(id)?;
        let logged = self
            .service_for(EntityRef::Object(id))
            .filter_map(|s| s.odometer)
            .fold(None, |m: Option<f64>, k| Some(m.map_or(k, |m| m.max(k))));
        match (v.odometer, logged) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        }
    }

    /// The odometer of whatever a reminder is about, if it's a vehicle.
    pub fn odometer_for(&self, target: EntityRef) -> Option<f64> {
        match target {
            EntityRef::Object(id) => self.odometer(id),
            _ => None,
        }
    }

    /// Add yearly winter and summer tire-swap reminders for a vehicle (any it
    /// already has are left alone). Returns how many were added.
    pub fn add_tire_swap_reminders(&mut self, id: ObjectId, today: NaiveDate) -> usize {
        let target = EntityRef::Object(id);
        let mut added = 0;
        for (title, month, day) in TIRE_SWAPS {
            if self.reminders_for(target).any(|r| r.title == *title) {
                continue;
            }
            let mut r = Reminder::new(target, *title, next_yearly(today, *month, *day));
            r.repeat = Some(Recurrence { every: 1, unit: RecurrenceUnit::Years });
            self.reminders.push(r);
            added += 1;
        }
        added
    }

    pub fn has_tire_swap_reminders(&self, id: ObjectId) -> bool {
        self.reminders_for(EntityRef::Object(id)).any(|r| TIRE_SWAPS.iter().any(|(t, _, _)| r.title == *t))
    }

    /// EV chargers placed anywhere: (id, name).
    pub fn ev_chargers(&self) -> Vec<(ObjectId, String)> {
        let mut v: Vec<(ObjectId, String)> = self
            .levels
            .iter()
            .flat_map(|l| l.objects.values().filter(|o| o.is_ev_charger()).map(|o| (o.id, o.display_name())))
            .collect();
        v.sort_by(|a, b| a.1.cmp(&b.1));
        v
    }

    /// All vehicles, with where they're parked: (level index, object id).
    pub fn vehicles(&self) -> Vec<(usize, ObjectId)> {
        let mut v: Vec<(usize, ObjectId)> = self
            .levels
            .iter()
            .enumerate()
            .flat_map(|(li, l)| l.objects.values().filter(|o| o.vehicle.is_some()).map(move |o| (li, o.id)))
            .collect();
        v.sort_by_key(|(li, id)| (self.levels[*li].objects[id].display_name().to_lowercase(), *id));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn distances_parse_and_format_in_either_unit() {
        assert_eq!(parse_distance("12,345", UnitSystem::Metric), Some(12345.0));
        assert!((parse_distance("10,000 mi", UnitSystem::Metric).unwrap() - 16093.44).abs() < 1e-6);
        assert!((parse_distance("100", UnitSystem::Imperial).unwrap() - 160.9344).abs() < 1e-6);
        assert_eq!(parse_distance("80k", UnitSystem::Metric), Some(80000.0));
        assert_eq!(parse_distance("", UnitSystem::Metric), None);
        assert_eq!(parse_distance("abc", UnitSystem::Metric), None);
        assert_eq!(format_distance(1234567.4, UnitSystem::Metric), "1,234,567 km");
        assert_eq!(format_distance(16093.44, UnitSystem::Imperial), "10,000 mi");
    }

    #[test]
    fn warranty_ends_at_date_or_distance_whichever_first() {
        let w = VehicleWarranty { expires: Some(d(2028, 1, 1)), until_km: Some(60000.0), ..Default::default() };
        assert_eq!(w.left(d(2027, 1, 1), Some(10000.0)), WarrantyLeft::Active { days: Some(365), km: Some(50000.0) });
        assert_eq!(w.left(d(2027, 1, 1), Some(61000.0)), WarrantyLeft::Expired);
        assert_eq!(w.left(d(2028, 6, 1), Some(10000.0)), WarrantyLeft::Expired);
        assert_eq!(VehicleWarranty::default().left(d(2027, 1, 1), None), WarrantyLeft::Unknown);
    }

    #[test]
    fn tire_swaps_land_on_the_next_season_once() {
        assert_eq!(next_yearly(d(2026, 11, 6), 11, 1), d(2027, 11, 1));
        assert_eq!(next_yearly(d(2026, 11, 6), 4, 15), d(2027, 4, 15));
        assert_eq!(next_yearly(d(2026, 3, 1), 4, 15), d(2026, 4, 15));
        let mut p = Project::new("T");
        let site = p.site_id().unwrap();
        let car = p.add_object(site, "Car", ObjectCategory::Vehicle, Shape::new(ShapeKind::Rectangle, 1.8, 4.6), crate::Vec2::new(0.0, 0.0)).unwrap();
        assert!(!p.has_tire_swap_reminders(car));
        assert_eq!(p.add_tire_swap_reminders(car, d(2026, 9, 1)), 2);
        assert_eq!(p.add_tire_swap_reminders(car, d(2026, 9, 1)), 0, "not added twice");
        assert!(p.has_tire_swap_reminders(car));
        assert_eq!(p.reminders[0].due, d(2026, 11, 1));
    }

    #[test]
    fn vehicle_documents_are_kept_and_chargers_found() {
        let mut p = Project::new("T");
        let site = p.site_id().unwrap();
        let car = p.add_object(site, "Car", ObjectCategory::Vehicle, Shape::new(ShapeKind::Rectangle, 1.8, 4.6), crate::Vec2::new(0.0, 0.0)).unwrap();
        let charger = p.add_object(site, "EV charger", ObjectCategory::Electrical, Shape::new(ShapeKind::Rectangle, 0.35, 0.2), crate::Vec2::new(3.0, 0.0)).unwrap();
        let doc = AttachmentId::new();
        p.levels.iter_mut().find_map(|l| l.objects.get_mut(&car)).unwrap().vehicle.as_mut().unwrap().documents.push(doc);
        assert!(p.referenced_attachments().contains(&doc), "saving must not throw the file away");
        assert_eq!(p.ev_chargers(), vec![(charger, "EV charger".to_string())]);
    }

    #[test]
    fn odometer_takes_the_latest_service_reading() {
        let mut p = Project::new("T");
        let site = p.site_id().unwrap();
        let shape = Shape::new(ShapeKind::Rectangle, 4.6, 1.8);
        let car = p.add_object(site, "Car", ObjectCategory::Vehicle, shape, crate::Vec2::new(0.0, 0.0)).unwrap();
        assert!(p.vehicle(car).is_some(), "placing a vehicle gives it a vehicle record");
        assert_eq!(p.odometer(car), None);
        let mut s = ServiceRecord::new(EntityRef::Object(car), d(2026, 3, 1));
        s.odometer = Some(42000.0);
        p.service_records.push(s);
        assert_eq!(p.odometer(car), Some(42000.0));
        assert_eq!(p.vehicles().len(), 1);
    }
}

//! Pets: who they are, their vet, vaccinations, medications and weight.
//!
//! Their reminders, appointments, vet visits and notes are the usual
//! [`Reminder`], [`ServiceRecord`] and [`Note`] records pointing at
//! [`EntityRef::Pet`]. Pets are private: they leave with their owner, so a
//! copy for a new owner drops them (see `sharing.rs`).

use crate::ids::*;
use crate::model::*;
use crate::units::UnitSystem;
use chrono::{Datelike, Months, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Species {
    #[default]
    Dog,
    Cat,
    Bird,
    Rabbit,
    GuineaPig,
    Hamster,
    Fish,
    Reptile,
    Horse,
    Other,
}

impl Species {
    pub const ALL: [Species; 10] = [
        Species::Dog,
        Species::Cat,
        Species::Bird,
        Species::Rabbit,
        Species::GuineaPig,
        Species::Hamster,
        Species::Fish,
        Species::Reptile,
        Species::Horse,
        Species::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Species::Dog => "Dog",
            Species::Cat => "Cat",
            Species::Bird => "Bird",
            Species::Rabbit => "Rabbit",
            Species::GuineaPig => "Guinea pig",
            Species::Hamster => "Hamster",
            Species::Fish => "Fish",
            Species::Reptile => "Reptile",
            Species::Horse => "Horse",
            Species::Other => "Other",
        }
    }

    /// Common breeds, as suggestions (anything can be typed).
    pub fn breeds(self) -> &'static [&'static str] {
        match self {
            Species::Dog => &[
                "Mixed breed",
                "Labrador Retriever",
                "Golden Retriever",
                "German Shepherd",
                "French Bulldog",
                "Bulldog",
                "Poodle",
                "Goldendoodle",
                "Labradoodle",
                "Beagle",
                "Rottweiler",
                "Dachshund",
                "German Shorthaired Pointer",
                "Pembroke Welsh Corgi",
                "Australian Shepherd",
                "Yorkshire Terrier",
                "Boxer",
                "Cavalier King Charles Spaniel",
                "Siberian Husky",
                "Bernese Mountain Dog",
                "Shih Tzu",
                "Border Collie",
                "Chihuahua",
                "Pug",
                "Great Dane",
                "Boston Terrier",
                "Miniature Schnauzer",
                "Pit Bull Terrier",
            ],
            Species::Cat => &[
                "Domestic shorthair",
                "Domestic longhair",
                "Domestic medium hair",
                "Maine Coon",
                "Ragdoll",
                "Siamese",
                "British Shorthair",
                "Persian",
                "Bengal",
                "Sphynx",
                "Abyssinian",
                "Scottish Fold",
                "Russian Blue",
                "Norwegian Forest Cat",
                "Birman",
            ],
            Species::Bird => &["Budgie", "Cockatiel", "Lovebird", "Parrotlet", "African Grey", "Conure", "Cockatoo", "Canary", "Finch", "Chicken"],
            Species::Rabbit => &["Holland Lop", "Mini Lop", "Netherland Dwarf", "Lionhead", "Mini Rex", "Flemish Giant", "Mixed breed"],
            Species::GuineaPig => &["American", "Abyssinian", "Peruvian", "Teddy", "Skinny pig"],
            Species::Hamster => &["Syrian", "Dwarf (Campbell's)", "Winter White", "Roborovski", "Chinese"],
            Species::Fish => &["Betta", "Goldfish", "Guppy", "Tetra", "Angelfish", "Koi", "Community tank"],
            Species::Reptile => &["Leopard gecko", "Crested gecko", "Bearded dragon", "Ball python", "Corn snake", "Red-eared slider", "Tortoise"],
            Species::Horse => &["Quarter Horse", "Thoroughbred", "Arabian", "Appaloosa", "Paint", "Morgan", "Pony"],
            Species::Other => &[],
        }
    }

    /// Vaccines vets usually give, as suggestions.
    pub fn vaccines(self) -> &'static [&'static str] {
        match self {
            Species::Dog => &[
                "Rabies",
                "DHPP (distemper, parvo)",
                "Bordetella (kennel cough)",
                "Leptospirosis",
                "Lyme disease",
                "Canine influenza",
            ],
            Species::Cat => &["Rabies", "FVRCP (distemper)", "FeLV (feline leukemia)"],
            Species::Rabbit => &["RHDV2", "Myxomatosis"],
            Species::Horse => &["Tetanus", "Eastern/Western encephalomyelitis", "West Nile virus", "Rabies", "Equine influenza", "Rhinopneumonitis (EHV)", "Strangles"],
            Species::Bird | Species::GuineaPig | Species::Hamster | Species::Fish | Species::Reptile => &[],
            Species::Other => &["Rabies"],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sex {
    #[default]
    Unknown,
    Male,
    Female,
}

impl Sex {
    pub const ALL: [Sex; 3] = [Sex::Female, Sex::Male, Sex::Unknown];
    pub fn label(self) -> &'static str {
        match self {
            Sex::Unknown => "Not sure",
            Sex::Male => "Male",
            Sex::Female => "Female",
        }
    }
}

/// A clinic: the regular vet, or the emergency one.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Vet {
    pub clinic: String,
    /// The vet they usually see.
    pub vet: String,
    pub phone: String,
    pub email: String,
    pub address: String,
    /// Hours, client number, portal login hint...
    pub notes: String,
}

impl Vet {
    pub fn is_empty(&self) -> bool {
        [&self.clinic, &self.vet, &self.phone, &self.email, &self.address, &self.notes].iter().all(|s| s.trim().is_empty())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PetInsurance {
    pub provider: String,
    pub policy: String,
    pub phone: String,
    pub coverage: String,
    pub deductible: String,
    pub renews: Option<NaiveDate>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Vaccination {
    pub name: String,
    pub given: Option<NaiveDate>,
    /// When the next dose (booster) is due, or when this one runs out.
    pub due: Option<NaiveDate>,
    /// Clinic or vet who gave it.
    pub given_by: String,
    /// Lot number or rabies tag number.
    pub lot: String,
    pub notes: String,
    /// The reminder that tracks `due`, kept in step with it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reminder: Option<ReminderId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaccineStatus {
    /// No due date: given once, or not recorded.
    NoDate,
    Current(i64),
    DueSoon(i64),
    Overdue(i64),
}

/// How soon a vaccine counts as due soon.
pub const VACCINE_SOON_DAYS: i64 = 30;

impl Vaccination {
    pub fn status(&self, today: NaiveDate) -> VaccineStatus {
        match self.due {
            None => VaccineStatus::NoDate,
            Some(d) => {
                let days = (d - today).num_days();
                if days < 0 {
                    VaccineStatus::Overdue(-days)
                } else if days <= VACCINE_SOON_DAYS {
                    VaccineStatus::DueSoon(days)
                } else {
                    VaccineStatus::Current(days)
                }
            }
        }
    }

    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() { "Vaccine".into() } else { self.name.trim().to_string() }
    }

    fn reminder_title(&self) -> String {
        format!("{} due", self.display_name())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Medication {
    pub name: String,
    pub dose: String,
    /// "Twice a day with food", "Monthly"...
    pub how_often: String,
    /// What it's for.
    pub purpose: String,
    pub prescribed_by: String,
    pub started: Option<NaiveDate>,
    /// Blank: ongoing.
    pub ends: Option<NaiveDate>,
    /// Refills left, pharmacy, prescription number.
    pub refills: String,
    pub notes: String,
    /// A repeating reminder to give it (or refill it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reminder: Option<ReminderId>,
}

impl Medication {
    pub fn is_active(&self, today: NaiveDate) -> bool {
        self.ends.is_none_or(|e| e >= today) && self.started.is_none_or(|s| s <= today)
    }

    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() { "Medication".into() } else { self.name.trim().to_string() }
    }

    /// A reminder schedule that matches how often it's given, if that's
    /// something worth a reminder (not "twice a day").
    pub fn suggested_repeat(&self) -> Option<Recurrence> {
        let s = self.how_often.to_lowercase();
        let r = |every, unit| Some(Recurrence { every, unit });
        if s.contains("3 month") || s.contains("three month") || s.contains("quarter") {
            r(3, RecurrenceUnit::Months)
        } else if s.contains("6 month") || s.contains("six month") {
            r(6, RecurrenceUnit::Months)
        } else if s.contains("month") {
            r(1, RecurrenceUnit::Months)
        } else if s.contains("2 week") || s.contains("two week") || s.contains("fortnight") {
            r(2, RecurrenceUnit::Weeks)
        } else if s.contains("week") {
            r(1, RecurrenceUnit::Weeks)
        } else if s.contains("year") || s.contains("annual") {
            r(1, RecurrenceUnit::Years)
        } else if s.contains("once a day") || s.contains("daily") || s.contains("every day") {
            r(1, RecurrenceUnit::Days)
        } else {
            None
        }
    }
}

pub const MED_FREQUENCIES: &[&str] = &[
    "Once a day",
    "Twice a day",
    "Three times a day",
    "Every other day",
    "Once a week",
    "Every 2 weeks",
    "Once a month",
    "Every 3 months",
    "As needed",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WeightEntry {
    pub date: NaiveDate,
    pub kg: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pet {
    pub id: PetId,
    pub name: String,
    pub species: Species,
    /// A typed-in species when `species` is Other ("Ferret").
    #[serde(skip_serializing_if = "String::is_empty")]
    pub species_name: String,
    pub breed: String,
    pub sex: Sex,
    /// Spayed or neutered.
    pub fixed: bool,
    pub birthday: Option<NaiveDate>,
    /// The birthday is a best guess (rescues).
    pub birthday_estimated: bool,
    /// Gotcha day.
    pub adopted: Option<NaiveDate>,
    pub adopted_from: String,
    /// Colour and markings.
    pub markings: String,
    pub photo: Option<AttachmentId>,

    pub microchip: String,
    pub microchip_registry: String,
    pub licence: String,
    pub licence_renews: Option<NaiveDate>,

    // Health and care
    pub allergies: String,
    pub conditions: String,
    pub temperament: String,
    pub food: String,
    pub feeding: String,
    pub treats: String,
    pub grooming: String,
    /// What a sitter needs to know.
    pub care_notes: String,

    pub vet: Vet,
    pub emergency_vet: Vet,
    pub insurance: PetInsurance,
    pub vaccinations: Vec<Vaccination>,
    pub medications: Vec<Medication>,
    pub weights: Vec<WeightEntry>,
    pub documents: Vec<AttachmentId>,
    pub custom_fields: Vec<CustomField>,
    /// In memory of.
    pub passed: Option<NaiveDate>,
}

impl Default for Pet {
    fn default() -> Self {
        Self {
            id: PetId::new(),
            name: String::new(),
            species: Species::Dog,
            species_name: String::new(),
            breed: String::new(),
            sex: Sex::Unknown,
            fixed: false,
            birthday: None,
            birthday_estimated: false,
            adopted: None,
            adopted_from: String::new(),
            markings: String::new(),
            photo: None,
            microchip: String::new(),
            microchip_registry: String::new(),
            licence: String::new(),
            licence_renews: None,
            allergies: String::new(),
            conditions: String::new(),
            temperament: String::new(),
            food: String::new(),
            feeding: String::new(),
            treats: String::new(),
            grooming: String::new(),
            care_notes: String::new(),
            vet: Vet::default(),
            emergency_vet: Vet::default(),
            insurance: PetInsurance::default(),
            vaccinations: Vec::new(),
            medications: Vec::new(),
            weights: Vec::new(),
            documents: Vec::new(),
            custom_fields: Vec::new(),
            passed: None,
        }
    }
}

impl Pet {
    pub fn new(name: impl Into<String>, species: Species) -> Self {
        Self { name: name.into(), species, ..Default::default() }
    }

    pub fn display_name(&self) -> String {
        if !self.name.trim().is_empty() {
            self.name.trim().to_string()
        } else if self.species == Species::Other && self.species_name.trim().is_empty() {
            "New pet".into()
        } else {
            format!("New {}", self.species_label().to_lowercase())
        }
    }

    pub fn species_label(&self) -> String {
        custom_label(self.species == Species::Other, &self.species_name, self.species.label())
    }

    /// "Golden Retriever · Female · 3 years"
    pub fn summary(&self, today: NaiveDate) -> String {
        let mut parts = Vec::new();
        parts.push(if self.breed.trim().is_empty() { self.species_label() } else { self.breed.trim().to_string() });
        if self.sex != Sex::Unknown {
            parts.push(self.sex.label().to_string());
        }
        if let Some(a) = self.age(today) {
            parts.push(a);
        }
        parts.join(" · ")
    }

    /// How old they are (or were), in words: "3 years", "7 months", "2 weeks".
    pub fn age(&self, today: NaiveDate) -> Option<String> {
        let born = self.birthday?;
        let to = self.passed.unwrap_or(today);
        let about = if self.birthday_estimated { "About " } else { "" };
        Some(format!("{about}{}", span_words(born, to)))
    }

    /// Their next birthday on or after `today` (and how old they'll be).
    pub fn next_birthday(&self, today: NaiveDate) -> Option<(NaiveDate, i32)> {
        let born = self.birthday?;
        if self.passed.is_some() {
            return None;
        }
        let mut years = today.year() - born.year();
        loop {
            let on = born.with_year(born.year() + years).or_else(|| NaiveDate::from_ymd_opt(born.year() + years, 2, 28))?;
            if on >= today {
                return Some((on, years));
            }
            years += 1;
        }
    }

    pub fn latest_weight(&self) -> Option<WeightEntry> {
        self.weights.iter().copied().max_by_key(|w| w.date)
    }

    /// The change since the reading before the latest one.
    pub fn weight_change(&self) -> Option<f64> {
        let mut w: Vec<WeightEntry> = self.weights.clone();
        w.sort_by_key(|e| e.date);
        let n = w.len();
        (n >= 2).then(|| w[n - 1].kg - w[n - 2].kg)
    }

    pub fn active_medications(&self, today: NaiveDate) -> impl Iterator<Item = &Medication> {
        self.medications.iter().filter(move |m| m.is_active(today))
    }

    /// The vaccine that most needs attention: overdue first, then the
    /// soonest due.
    pub fn next_vaccine(&self, today: NaiveDate) -> Option<&Vaccination> {
        // Only the latest dose of each vaccine counts.
        self.current_vaccines().into_iter().filter(|v| v.status(today) != VaccineStatus::NoDate).min_by_key(|v| v.due)
    }

    /// The most recent record of each vaccine (by name).
    pub fn current_vaccines(&self) -> Vec<&Vaccination> {
        self.current_vaccine_indices().into_iter().map(|i| &self.vaccinations[i]).collect()
    }

    /// Where in `vaccinations` the most recent record of each vaccine is.
    pub fn current_vaccine_indices(&self) -> Vec<usize> {
        let mut latest: Vec<usize> = Vec::new();
        for (i, v) in self.vaccinations.iter().enumerate() {
            let key = v.name.trim().to_lowercase();
            match latest.iter_mut().find(|l| self.vaccinations[**l].name.trim().to_lowercase() == key) {
                // Later records win ties (a dose recorded twice on one day).
                Some(l) if v.given >= self.vaccinations[*l].given => *l = i,
                Some(_) => {}
                None => latest.push(i),
            }
        }
        latest
    }

    pub fn is_remembered(&self) -> bool {
        self.passed.is_some()
    }
}

/// "3 years", "1 year, 4 months", "7 months", "3 weeks".
pub fn span_words(from: NaiveDate, to: NaiveDate) -> String {
    let mut months = (to.year() - from.year()) * 12 + to.month() as i32 - from.month() as i32;
    if to.day() < from.day() {
        months -= 1;
    }
    let plural = |n: i32, w: &str| if n == 1 { format!("1 {w}") } else { format!("{n} {w}s") };
    if months >= 24 {
        plural(months / 12, "year")
    } else if months >= 12 {
        let m = months % 12;
        if m == 0 { plural(1, "year") } else { format!("1 year, {}", plural(m, "month")) }
    } else if months >= 1 {
        plural(months, "month")
    } else {
        let days = (to - from).num_days().max(0) as i32;
        if days >= 7 { plural(days / 7, "week") } else { plural(days, "day") }
    }
}

const LB_PER_KG: f64 = 2.204_622_621_8;

/// A weight in the house's units: "12.4 kg" or "27.3 lb". Small pets get grams or ounces.
pub fn format_weight(kg: f64, units: UnitSystem) -> String {
    match units {
        UnitSystem::Metric if kg < 1.0 => format!("{:.0} g", kg * 1000.0),
        UnitSystem::Metric => format!("{} kg", trim_float(kg, 1)),
        UnitSystem::Imperial if kg * LB_PER_KG < 1.0 => format!("{} oz", trim_float(kg * LB_PER_KG * 16.0, 1)),
        UnitSystem::Imperial => format!("{} lb", trim_float(kg * LB_PER_KG, 1)),
    }
}

pub fn weight_unit(units: UnitSystem) -> &'static str {
    match units {
        UnitSystem::Metric => "kg",
        UnitSystem::Imperial => "lb",
    }
}

pub fn kg_to_display(kg: f64, units: UnitSystem) -> f64 {
    match units {
        UnitSystem::Metric => kg,
        UnitSystem::Imperial => kg * LB_PER_KG,
    }
}

pub fn display_to_kg(v: f64, units: UnitSystem) -> f64 {
    match units {
        UnitSystem::Metric => v,
        UnitSystem::Imperial => v / LB_PER_KG,
    }
}

/// Read a typed weight: "12.4" (house units), "12.4 kg", "27 lb", "350 g", "8 oz".
pub fn parse_weight(input: &str, units: UnitSystem) -> Option<f64> {
    let s = input.trim().to_lowercase().replace([',', ' '], "");
    if s.is_empty() {
        return None;
    }
    let num = |n: &str| n.parse::<f64>().ok().filter(|v| *v > 0.0);
    let kg = if let Some(n) = s.strip_suffix("kg") {
        num(n)?
    } else if let Some(n) = s.strip_suffix("lbs").or(s.strip_suffix("lb")) {
        num(n)? / LB_PER_KG
    } else if let Some(n) = s.strip_suffix("oz") {
        num(n)? / LB_PER_KG / 16.0
    } else if let Some(n) = s.strip_suffix('g') {
        num(n)? / 1000.0
    } else {
        display_to_kg(num(&s)?, units)
    };
    Some(kg)
}

fn trim_float(v: f64, places: usize) -> String {
    let s = format!("{v:.places$}");
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// Kinds of vet visit, for the visit history (anything can be typed).
pub const VISIT_KINDS: &[&str] = &[
    "Checkup",
    "Vaccination",
    "Illness",
    "Injury",
    "Dental cleaning",
    "Surgery",
    "Lab work",
    "Emergency",
    "Grooming",
    "Boarding",
    "Training",
];

/// One-tap reminders: (title, every, unit, which species it suits).
pub struct PetReminder {
    pub title: &'static str,
    pub every: u32,
    pub unit: RecurrenceUnit,
    pub species: &'static [Species],
}

const FURRY: &[Species] = &[Species::Dog, Species::Cat, Species::Rabbit, Species::GuineaPig, Species::Hamster, Species::Horse, Species::Other];

pub const COMMON_REMINDERS: &[PetReminder] = &[
    PetReminder { title: "Annual checkup", every: 1, unit: RecurrenceUnit::Years, species: &[Species::Dog, Species::Cat, Species::Rabbit, Species::Bird, Species::Horse, Species::Reptile, Species::Other] },
    PetReminder { title: "Flea & tick prevention", every: 1, unit: RecurrenceUnit::Months, species: &[Species::Dog, Species::Cat, Species::Other] },
    PetReminder { title: "Heartworm prevention", every: 1, unit: RecurrenceUnit::Months, species: &[Species::Dog, Species::Cat] },
    PetReminder { title: "Deworming", every: 3, unit: RecurrenceUnit::Months, species: &[Species::Dog, Species::Cat, Species::Horse] },
    PetReminder { title: "Nail trim", every: 4, unit: RecurrenceUnit::Weeks, species: FURRY },
    PetReminder { title: "Grooming", every: 6, unit: RecurrenceUnit::Weeks, species: &[Species::Dog, Species::Cat, Species::Rabbit] },
    PetReminder { title: "Dental cleaning", every: 1, unit: RecurrenceUnit::Years, species: &[Species::Dog, Species::Cat] },
    PetReminder { title: "Hoof trim (farrier)", every: 6, unit: RecurrenceUnit::Weeks, species: &[Species::Horse] },
    PetReminder { title: "Change bedding", every: 1, unit: RecurrenceUnit::Weeks, species: &[Species::Rabbit, Species::GuineaPig, Species::Hamster, Species::Reptile] },
    PetReminder { title: "Clean the cage", every: 1, unit: RecurrenceUnit::Weeks, species: &[Species::Bird] },
    PetReminder { title: "Water change", every: 2, unit: RecurrenceUnit::Weeks, species: &[Species::Fish] },
    PetReminder { title: "Replace filter", every: 1, unit: RecurrenceUnit::Months, species: &[Species::Fish] },
    PetReminder { title: "Replace UVB bulb", every: 6, unit: RecurrenceUnit::Months, species: &[Species::Reptile] },
    PetReminder { title: "Order food", every: 1, unit: RecurrenceUnit::Months, species: &Species::ALL },
];

// ---------------------------------------------------------------------------
// The project's pets

impl Project {
    pub fn pet(&self, id: PetId) -> Option<&Pet> {
        self.pets.iter().find(|p| p.id == id)
    }

    pub fn pet_mut(&mut self, id: PetId) -> Option<&mut Pet> {
        self.pets.iter_mut().find(|p| p.id == id)
    }

    pub fn add_pet(&mut self, name: impl Into<String>, species: Species) -> PetId {
        let pet = Pet::new(name, species);
        let id = pet.id;
        self.pets.push(pet);
        id
    }

    /// Remove a pet and everything about it (notes, visits, reminders).
    pub fn delete_pet(&mut self, id: PetId) {
        let target = EntityRef::Pet(id);
        self.pets.retain(|p| p.id != id);
        self.notes.retain(|n| n.target != target);
        self.service_records.retain(|s| s.target != target);
        self.reminders.retain(|r| r.target != target);
    }

    /// Keep the reminders a pet's vaccines and medications made in step
    /// with them: titles and due dates follow; ones whose vaccine or
    /// medication is gone (or lost its date) are dropped.
    pub fn sync_pet_reminders(&mut self, id: PetId) {
        let Some(pet) = self.pet(id) else { return };
        let target = EntityRef::Pet(id);
        let mut wanted: Vec<(ReminderId, String, Option<NaiveDate>)> = Vec::new();
        for v in &pet.vaccinations {
            if let Some(r) = v.reminder {
                wanted.push((r, v.reminder_title(), v.due));
            }
        }
        let meds: Vec<(ReminderId, String)> = pet
            .medications
            .iter()
            .filter_map(|m| m.reminder.map(|r| (r, m.display_name())))
            .collect();
        // Vaccine reminders: the due date follows the vaccine's.
        let mut gone = Vec::new();
        for (rid, title, due) in &wanted {
            match (self.reminders.iter_mut().find(|r| r.id == *rid), due) {
                (Some(r), Some(d)) => {
                    r.title = title.clone();
                    if r.due != *d {
                        r.due = *d;
                        r.completed = None;
                        r.snoozed_until = None;
                    }
                }
                (Some(_), None) | (None, _) => gone.push(*rid),
            }
        }
        for (rid, title) in &meds {
            match self.reminders.iter_mut().find(|r| r.id == *rid) {
                Some(r) => r.title = title.clone(),
                None => gone.push(*rid),
            }
        }
        // Forget links to reminders that were deleted, and delete reminders
        // whose vaccine lost its date.
        self.reminders.retain(|r| !(gone.contains(&r.id) && r.target == target));
        if let Some(pet) = self.pet_mut(id) {
            for v in &mut pet.vaccinations {
                if v.reminder.is_some_and(|r| gone.contains(&r)) {
                    v.reminder = None;
                }
            }
            for m in &mut pet.medications {
                if m.reminder.is_some_and(|r| gone.contains(&r)) {
                    m.reminder = None;
                }
            }
        }
    }

    /// Remove a vaccine record and the reminder it made.
    pub fn remove_vaccination(&mut self, id: PetId, index: usize) {
        let Some(pet) = self.pet_mut(id) else { return };
        if index < pet.vaccinations.len() {
            let v = pet.vaccinations.remove(index);
            if let Some(rid) = v.reminder {
                self.reminders.retain(|r| r.id != rid);
            }
        }
    }

    /// Remove a medication and the reminder it made.
    pub fn remove_medication(&mut self, id: PetId, index: usize) {
        let Some(pet) = self.pet_mut(id) else { return };
        if index < pet.medications.len() {
            let m = pet.medications.remove(index);
            if let Some(rid) = m.reminder {
                self.reminders.retain(|r| r.id != rid);
            }
        }
    }

    /// Turn the reminder for a vaccine's due date on or off.
    pub fn set_vaccine_reminder(&mut self, id: PetId, index: usize, on: bool) {
        let target = EntityRef::Pet(id);
        let Some(pet) = self.pet(id) else { return };
        let Some(v) = pet.vaccinations.get(index) else { return };
        match (on, v.reminder, v.due) {
            (true, None, Some(due)) => {
                let r = Reminder::new(target, v.reminder_title(), due);
                let rid = r.id;
                self.reminders.push(r);
                if let Some(v) = self.pet_mut(id).and_then(|p| p.vaccinations.get_mut(index)) {
                    v.reminder = Some(rid);
                }
            }
            (false, Some(rid), _) => {
                self.reminders.retain(|r| r.id != rid);
                if let Some(v) = self.pet_mut(id).and_then(|p| p.vaccinations.get_mut(index)) {
                    v.reminder = None;
                }
            }
            _ => {}
        }
    }

    /// Record a new dose of a vaccine given on `on`: a new record, due again
    /// after the same gap as last time. Its reminder moves to the new dose.
    pub fn record_dose(&mut self, id: PetId, index: usize, on: NaiveDate) -> Option<usize> {
        let pet = self.pet_mut(id)?;
        let old = pet.vaccinations.get(index)?.clone();
        let gap = match (old.given, old.due) {
            (Some(g), Some(d)) if d > g => {
                // Keep whole months/years when that's what it was.
                let months = (d.year() - g.year()) * 12 + d.month() as i32 - g.month() as i32;
                if months > 0 && d.day() == g.day() { on.checked_add_months(Months::new(months as u32)) } else { on.checked_add_signed(d - g) }
            }
            _ => None,
        };
        let dose = Vaccination { given: Some(on), due: gap, reminder: old.reminder, lot: String::new(), notes: String::new(), ..old };
        pet.vaccinations[index].reminder = None;
        pet.vaccinations.push(dose);
        let new = pet.vaccinations.len() - 1;
        self.sync_pet_reminders(id);
        Some(new)
    }

    /// Add a repeating reminder for a medication (how often it's given, or
    /// monthly), starting `from`.
    pub fn add_medication_reminder(&mut self, id: PetId, index: usize, from: NaiveDate) {
        let target = EntityRef::Pet(id);
        let Some(pet) = self.pet(id) else { return };
        let Some(m) = pet.medications.get(index) else { return };
        if m.reminder.is_some() {
            return;
        }
        let mut r = Reminder::new(target, m.display_name(), from);
        r.repeat = Some(m.suggested_repeat().unwrap_or(Recurrence { every: 1, unit: RecurrenceUnit::Months }));
        if !m.dose.trim().is_empty() {
            r.notes = m.dose.trim().to_string();
        }
        let rid = r.id;
        self.reminders.push(r);
        if let Some(m) = self.pet_mut(id).and_then(|p| p.medications.get_mut(index)) {
            m.reminder = Some(rid);
        }
    }

    /// When a pet's reminder is marked done: a vaccine's reminder records
    /// the dose (and comes due again for the next one).
    pub(crate) fn pet_reminder_done(&mut self, rid: ReminderId, on: NaiveDate) {
        let found = self.pets.iter().find_map(|p| p.vaccinations.iter().position(|v| v.reminder == Some(rid)).map(|i| (p.id, i)));
        if let Some((pid, i)) = found {
            self.record_dose(pid, i, on);
        }
    }

    /// Reminders that a pet's vaccines keep (shown with the vaccine).
    pub fn vaccine_reminders(&self, id: PetId) -> Vec<ReminderId> {
        self.pet(id).map(|p| p.vaccinations.iter().filter_map(|v| v.reminder).collect()).unwrap_or_default()
    }

    /// Pets still with you, then ones you've lost.
    pub fn pets_in_order(&self) -> Vec<&Pet> {
        let mut v: Vec<&Pet> = self.pets.iter().collect();
        v.sort_by_key(|p| p.passed.is_some());
        v
    }

    /// Every attachment pets point at (photos and documents).
    pub fn pet_attachments(&self) -> Vec<AttachmentId> {
        self.pets.iter().flat_map(|p| p.photo.into_iter().chain(p.documents.iter().copied())).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn ages_in_words() {
        assert_eq!(span_words(d(2023, 5, 10), d(2026, 11, 7)), "3 years");
        assert_eq!(span_words(d(2025, 7, 10), d(2026, 11, 7)), "1 year, 3 months");
        assert_eq!(span_words(d(2026, 4, 1), d(2026, 11, 7)), "7 months");
        assert_eq!(span_words(d(2026, 10, 20), d(2026, 11, 7)), "2 weeks");
        let mut p = Pet::new("Biscuit", Species::Dog);
        p.birthday = Some(d(2022, 3, 3));
        p.birthday_estimated = true;
        assert_eq!(p.age(d(2026, 11, 7)).as_deref(), Some("About 4 years"));
        assert_eq!(p.next_birthday(d(2026, 11, 7)), Some((d(2027, 3, 3), 5)));
    }

    #[test]
    fn weights_in_house_units() {
        assert_eq!(format_weight(12.4, UnitSystem::Metric), "12.4 kg");
        assert_eq!(format_weight(0.35, UnitSystem::Metric), "350 g");
        assert_eq!(format_weight(10.0, UnitSystem::Imperial), "22 lb");
        assert!((display_to_kg(kg_to_display(7.3, UnitSystem::Imperial), UnitSystem::Imperial) - 7.3).abs() < 1e-9);
        assert_eq!(parse_weight("22", UnitSystem::Imperial).map(|k| (k * 10.0).round()), Some(100.0));
        assert_eq!(parse_weight("350 g", UnitSystem::Imperial), Some(0.35));
        assert_eq!(parse_weight("12.4kg", UnitSystem::Imperial), Some(12.4));
        assert_eq!(parse_weight("heavy", UnitSystem::Metric), None);
    }

    #[test]
    fn vaccine_reminders_follow_the_vaccine() {
        let mut p = Project::new("T");
        let id = p.add_pet("Biscuit", Species::Dog);
        p.pet_mut(id).unwrap().vaccinations.push(Vaccination {
            name: "Rabies".into(),
            given: Some(d(2025, 6, 1)),
            due: Some(d(2026, 6, 1)),
            ..Default::default()
        });
        p.set_vaccine_reminder(id, 0, true);
        assert_eq!(p.reminders.len(), 1);
        assert_eq!(p.reminders[0].due, d(2026, 6, 1));
        assert_eq!(p.reminders[0].title, "Rabies due");

        // Moving the due date moves the reminder.
        p.pet_mut(id).unwrap().vaccinations[0].due = Some(d(2028, 6, 1));
        p.sync_pet_reminders(id);
        assert_eq!(p.reminders[0].due, d(2028, 6, 1));

        // Marking the reminder done records the dose and schedules the next.
        let rid = p.reminders[0].id;
        p.complete_reminder(rid, d(2026, 11, 7));
        let pet = p.pet(id).unwrap();
        assert_eq!(pet.vaccinations.len(), 2);
        let latest = pet.current_vaccines()[0].clone();
        assert_eq!(latest.given, Some(d(2026, 11, 7)));
        assert_eq!(latest.due, Some(d(2029, 11, 7)));
        assert_eq!(latest.reminder, Some(rid));
        assert_eq!(p.reminders[0].due, d(2029, 11, 7));
        assert!(!p.reminders[0].is_done());
        // The visit is logged against the pet.
        assert_eq!(p.service_records.len(), 1);
        assert_eq!(p.service_records[0].target, EntityRef::Pet(id));

        // Deleting the pet takes its reminders and history with it.
        p.delete_pet(id);
        assert!(p.reminders.is_empty() && p.service_records.is_empty());
    }

    #[test]
    fn medication_reminders_repeat_like_the_dose() {
        let mut p = Project::new("T");
        let id = p.add_pet("Mochi", Species::Cat);
        p.pet_mut(id).unwrap().medications.push(Medication { name: "Revolution Plus".into(), how_often: "Once a month".into(), ..Default::default() });
        p.add_medication_reminder(id, 0, d(2026, 11, 7));
        assert_eq!(p.reminders[0].repeat, Some(Recurrence { every: 1, unit: RecurrenceUnit::Months }));
        // Deleting the medication drops its reminder.
        p.remove_medication(id, 0);
        assert!(p.reminders.is_empty());
        // So does deleting the reminder itself (the link is forgotten).
        p.pet_mut(id).unwrap().medications.push(Medication { name: "Gabapentin".into(), ..Default::default() });
        p.add_medication_reminder(id, 0, d(2026, 11, 7));
        p.reminders.clear();
        p.sync_pet_reminders(id);
        assert_eq!(p.pet(id).unwrap().medications[0].reminder, None);
    }

    #[test]
    fn latest_dose_of_each_vaccine() {
        let mut pet = Pet::new("Biscuit", Species::Dog);
        pet.vaccinations.push(Vaccination { name: "Rabies".into(), given: Some(d(2023, 6, 1)), due: Some(d(2024, 6, 1)), ..Default::default() });
        pet.vaccinations.push(Vaccination { name: "rabies".into(), given: Some(d(2024, 6, 1)), due: Some(d(2027, 6, 1)), ..Default::default() });
        pet.vaccinations.push(Vaccination { name: "DHPP".into(), given: Some(d(2024, 6, 1)), due: Some(d(2026, 12, 1)), ..Default::default() });
        assert_eq!(pet.current_vaccines().len(), 2);
        let next = pet.next_vaccine(d(2026, 11, 7)).unwrap();
        assert_eq!(next.name, "DHPP");
        assert_eq!(next.status(d(2026, 11, 7)), VaccineStatus::DueSoon(24));
    }
}

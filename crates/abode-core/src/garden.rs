//! Gardens: what's planted where and when in a garden bed, the seeds you
//! have, and a planting calendar worked out from your frost dates.
//!
//! A garden is an outside [`Feature`] of kind [`FeatureKind::GardenBed`].
//! Each [`Planting`] is a row, a patch or a single plant inside one, for one
//! year. Perennials stay in the plan every year until they're taken out.

use crate::geometry::{Vec2, area, bounds, distance_to_polyline, point_in_polygon, polyline_length};
use crate::ids::*;
use crate::model::*;
use crate::site::FeatureKind;
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Settings

/// Where you garden: frost dates drive the planting calendar. Only the month
/// and day are used; they apply to every year.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GardenSettings {
    /// Average last spring frost.
    pub last_frost: Option<NaiveDate>,
    /// Average first fall frost.
    pub first_frost: Option<NaiveDate>,
    /// Hardiness zone, e.g. "6a".
    pub zone: String,
}

impl GardenSettings {
    pub fn last_frost_in(&self, year: i32) -> Option<NaiveDate> {
        self.last_frost.and_then(|d| on_year(d, year))
    }

    pub fn first_frost_in(&self, year: i32) -> Option<NaiveDate> {
        self.first_frost.and_then(|d| on_year(d, year))
    }

    /// Days between the frosts: the frost-free growing season.
    pub fn season_days(&self, year: i32) -> Option<i64> {
        Some((self.first_frost_in(year)? - self.last_frost_in(year)?).num_days())
    }
}

fn on_year(d: NaiveDate, year: i32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(year, d.month(), d.day()).or_else(|| NaiveDate::from_ymd_opt(year, d.month(), 28))
}

// ---------------------------------------------------------------------------
// Crop library

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Nightshade,
    Brassica,
    Legume,
    Allium,
    Cucurbit,
    Umbellifer,
    Amaranth,
    Aster,
    Mint,
    Grass,
    Rose,
    Other,
}

impl Family {
    pub fn label(self) -> &'static str {
        match self {
            Family::Nightshade => "tomato family (nightshades)",
            Family::Brassica => "cabbage family (brassicas)",
            Family::Legume => "pea and bean family (legumes)",
            Family::Allium => "onion family (alliums)",
            Family::Cucurbit => "squash family (cucurbits)",
            Family::Umbellifer => "carrot family",
            Family::Amaranth => "beet and spinach family",
            Family::Aster => "lettuce and daisy family",
            Family::Mint => "mint family",
            Family::Grass => "grass family",
            Family::Rose => "rose family",
            Family::Other => "other",
        }
    }

    /// Families worth rotating: they share pests and diseases that build up
    /// in the soil, or (legumes) leave it richer for whatever comes next.
    pub fn rotates(self) -> bool {
        !matches!(self, Family::Other | Family::Mint | Family::Rose | Family::Aster)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CropKind {
    Vegetable,
    Herb,
    Fruit,
    Flower,
}

impl CropKind {
    pub fn label(self) -> &'static str {
        match self {
            CropKind::Vegetable => "Vegetables",
            CropKind::Herb => "Herbs",
            CropKind::Fruit => "Fruit",
            CropKind::Flower => "Flowers",
        }
    }
    pub const ALL: [CropKind; 4] = [CropKind::Vegetable, CropKind::Herb, CropKind::Fruit, CropKind::Flower];
}

/// A planting window in weeks relative to a frost date (negative = before).
pub type Weeks = Option<(i8, i8)>;

/// What a seed packet would tell you, for a typical variety.
pub struct Crop {
    pub name: &'static str,
    pub family: Family,
    pub kind: CropKind,
    pub perennial: bool,
    /// Between plants in a row (cm).
    pub spacing_cm: u16,
    /// Between rows (cm).
    pub row_cm: u16,
    pub germinate_days: (u8, u8),
    /// From sowing outside, or from transplanting for crops usually started
    /// indoors (how seed packets count it). 0 for perennials.
    pub maturity_days: u16,
    /// Weeks from the last spring frost.
    pub start_indoors: Weeks,
    pub transplant: Weeks,
    pub direct_sow: Weeks,
    /// Weeks from the first fall frost (fall crops, garlic).
    pub fall_sow: Weeks,
    /// How long it keeps producing once ready.
    pub harvest_weeks: u8,
    /// Perennials: the months it's usually harvested, every year.
    pub harvest_months: Option<(u8, u8)>,
    /// How many years seed usually stays good.
    pub seed_years: u8,
    pub companions: &'static [&'static str],
    pub avoid: &'static [&'static str],
    pub tip: &'static str,
}

macro_rules! crop {
    ($name:literal, $family:ident, $kind:ident, spacing $sp:literal, rows $row:literal, germ ($g0:literal, $g1:literal), days $days:literal,
     indoors $ind:expr, transplant $tr:expr, sow $sow:expr, fall $fall:expr, harvest $hw:literal, seed $sy:literal,
     good [$($good:literal),*], bad [$($bad:literal),*], $tip:literal) => {
        Crop {
            name: $name,
            family: Family::$family,
            kind: CropKind::$kind,
            perennial: false,
            spacing_cm: $sp,
            row_cm: $row,
            germinate_days: ($g0, $g1),
            maturity_days: $days,
            start_indoors: $ind,
            transplant: $tr,
            direct_sow: $sow,
            fall_sow: $fall,
            harvest_weeks: $hw,
            harvest_months: None,
            seed_years: $sy,
            companions: &[$($good),*],
            avoid: &[$($bad),*],
            tip: $tip,
        }
    };
}

macro_rules! perennial {
    ($name:literal, $family:ident, $kind:ident, spacing $sp:literal, rows $row:literal, plant $plant:expr, months ($m0:literal, $m1:literal),
     good [$($good:literal),*], bad [$($bad:literal),*], $tip:literal) => {
        Crop {
            name: $name,
            family: Family::$family,
            kind: CropKind::$kind,
            perennial: true,
            spacing_cm: $sp,
            row_cm: $row,
            germinate_days: (0, 0),
            maturity_days: 0,
            start_indoors: None,
            transplant: $plant,
            direct_sow: None,
            fall_sow: None,
            harvest_weeks: 0,
            harvest_months: Some(($m0, $m1)),
            seed_years: 0,
            companions: &[$($good),*],
            avoid: &[$($bad),*],
            tip: $tip,
        }
    };
}

/// Bulbs and fall-planted perennials: planted weeks before the first fall
/// frost, flowering in `months` from the next year.
macro_rules! bulb {
    ($name:literal, $family:ident, spacing $sp:literal, rows $row:literal, fall $fall:expr, months ($m0:literal, $m1:literal), $tip:literal) => {
        Crop {
            name: $name,
            family: Family::$family,
            kind: CropKind::Flower,
            perennial: true,
            spacing_cm: $sp,
            row_cm: $row,
            germinate_days: (0, 0),
            maturity_days: 0,
            start_indoors: None,
            transplant: None,
            direct_sow: None,
            fall_sow: $fall,
            harvest_weeks: 0,
            harvest_months: Some(($m0, $m1)),
            seed_years: 0,
            companions: &[],
            avoid: &[],
            tip: $tip,
        }
    };
}

/// Common vegetables, herbs, fruit and flowers. Dates are typical; seed
/// packets for a particular variety may differ, and every field can be
/// overridden on the planting.
pub const CROPS: &[Crop] = &[
    // Vegetables
    crop!("Tomato", Nightshade, Vegetable, spacing 60, rows 90, germ (5, 10), days 70, indoors Some((-8, -6)), transplant Some((1, 3)), sow None, fall None, harvest 10, seed 5,
        good ["Basil", "Carrot", "Onion", "Parsley", "Marigold"], bad ["Potato", "Fennel", "Cabbage", "Broccoli", "Kale", "Corn", "Dill"],
        "Plant deep: roots grow along the buried stem. Stake or cage at planting."),
    crop!("Pepper", Nightshade, Vegetable, spacing 45, rows 60, germ (7, 14), days 70, indoors Some((-10, -8)), transplant Some((2, 4)), sow None, fall None, harvest 8, seed 2,
        good ["Basil", "Onion", "Carrot", "Marigold"], bad ["Fennel", "Kale", "Broccoli"],
        "Wait for warm soil; cold nights stall them."),
    crop!("Eggplant", Nightshade, Vegetable, spacing 50, rows 75, germ (7, 14), days 75, indoors Some((-10, -8)), transplant Some((2, 4)), sow None, fall None, harvest 8, seed 4,
        good ["Bean", "Thyme", "Marigold"], bad ["Fennel", "Potato"],
        "Loves heat. Row cover early on keeps flea beetles off."),
    crop!("Potato", Nightshade, Vegetable, spacing 30, rows 75, germ (14, 28), days 90, indoors None, transplant None, sow Some((-2, 2)), fall None, harvest 3, seed 1,
        good ["Bean", "Corn", "Cabbage", "Marigold"], bad ["Tomato", "Cucumber", "Squash", "Zucchini", "Pumpkin", "Sunflower", "Raspberry"],
        "Plant seed potatoes; hill soil over the stems as they grow."),
    crop!("Lettuce", Aster, Vegetable, spacing 25, rows 30, germ (4, 10), days 50, indoors Some((-6, -4)), transplant Some((-2, 0)), sow Some((-4, 2)), fall Some((-8, -6)), harvest 4, seed 5,
        good ["Carrot", "Radish", "Onion", "Strawberry", "Beet"], bad [],
        "Sow a short row every 2 to 3 weeks for a steady supply. Bolts in summer heat."),
    crop!("Spinach", Amaranth, Vegetable, spacing 10, rows 30, germ (7, 14), days 40, indoors None, transplant None, sow Some((-6, -3)), fall Some((-8, -6)), harvest 4, seed 3,
        good ["Strawberry", "Pea", "Radish", "Lettuce"], bad [],
        "A cool-weather crop: sow early spring or late summer."),
    crop!("Kale", Brassica, Vegetable, spacing 45, rows 60, germ (5, 10), days 55, indoors Some((-6, -4)), transplant Some((-2, 0)), sow Some((-4, -2)), fall Some((-12, -10)), harvest 16, seed 4,
        good ["Onion", "Beet", "Dill", "Potato"], bad ["Tomato", "Strawberry", "Pepper"],
        "Pick the lower leaves and it keeps going. A light frost sweetens it."),
    crop!("Broccoli", Brassica, Vegetable, spacing 45, rows 60, germ (5, 10), days 60, indoors Some((-8, -6)), transplant Some((-2, 0)), sow None, fall Some((-14, -12)), harvest 4, seed 4,
        good ["Onion", "Dill", "Beet", "Potato"], bad ["Tomato", "Strawberry", "Pepper"],
        "Cut the main head before the buds open; side shoots follow."),
    crop!("Cabbage", Brassica, Vegetable, spacing 45, rows 60, germ (5, 10), days 70, indoors Some((-8, -6)), transplant Some((-2, 0)), sow None, fall Some((-14, -12)), harvest 3, seed 4,
        good ["Onion", "Dill", "Beet", "Potato"], bad ["Tomato", "Strawberry"],
        "Even watering stops heads from splitting."),
    crop!("Cauliflower", Brassica, Vegetable, spacing 45, rows 60, germ (5, 10), days 65, indoors Some((-8, -6)), transplant Some((-1, 1)), sow None, fall Some((-14, -12)), harvest 2, seed 4,
        good ["Onion", "Dill", "Beet"], bad ["Tomato", "Strawberry"],
        "Fold leaves over the head when it's egg-sized to keep it white."),
    crop!("Brussels sprouts", Brassica, Vegetable, spacing 60, rows 75, germ (5, 10), days 100, indoors Some((-8, -6)), transplant Some((0, 2)), sow None, fall None, harvest 8, seed 4,
        good ["Onion", "Dill", "Potato"], bad ["Tomato", "Strawberry"],
        "Long season; tastes best after a few frosts."),
    crop!("Radish", Brassica, Vegetable, spacing 5, rows 20, germ (3, 7), days 25, indoors None, transplant None, sow Some((-4, 4)), fall Some((-6, -2)), harvest 2, seed 5,
        good ["Lettuce", "Carrot", "Pea", "Cucumber", "Spinach"], bad [],
        "Fast: sow between slower crops and pull before they're needed."),
    crop!("Carrot", Umbellifer, Vegetable, spacing 5, rows 30, germ (10, 21), days 70, indoors None, transplant None, sow Some((-3, 2)), fall None, harvest 6, seed 3,
        good ["Tomato", "Onion", "Lettuce", "Rosemary", "Radish", "Pea"], bad ["Dill", "Parsnip"],
        "Keep the seedbed moist until they're up; thin to a finger apart."),
    crop!("Parsnip", Umbellifer, Vegetable, spacing 10, rows 45, germ (14, 28), days 110, indoors None, transplant None, sow Some((-2, 2)), fall None, harvest 8, seed 1,
        good ["Onion", "Radish"], bad ["Carrot"],
        "Use fresh seed every year. Sweetest after frost."),
    crop!("Beet", Amaranth, Vegetable, spacing 10, rows 30, germ (5, 10), days 55, indoors None, transplant None, sow Some((-2, 4)), fall Some((-10, -8)), harvest 4, seed 4,
        good ["Onion", "Lettuce", "Cabbage", "Kale", "Garlic"], bad ["Pole bean"],
        "Each seed is a cluster: thin the seedlings."),
    crop!("Swiss chard", Amaranth, Vegetable, spacing 30, rows 45, germ (5, 10), days 55, indoors None, transplant None, sow Some((-2, 2)), fall None, harvest 16, seed 4,
        good ["Bean", "Onion", "Cabbage"], bad [],
        "Cut the outer stalks and it keeps growing all season."),
    crop!("Onion", Allium, Vegetable, spacing 10, rows 30, germ (7, 14), days 100, indoors Some((-12, -10)), transplant Some((-4, -2)), sow None, fall None, harvest 3, seed 1,
        good ["Carrot", "Tomato", "Lettuce", "Beet", "Cabbage"], bad ["Bean", "Pea"],
        "Sets or transplants are easiest. Harvest when the tops fall over."),
    crop!("Green onion", Allium, Vegetable, spacing 5, rows 20, germ (7, 14), days 60, indoors None, transplant None, sow Some((-4, 2)), fall None, harvest 8, seed 1,
        good ["Carrot", "Tomato", "Lettuce"], bad ["Bean", "Pea"],
        "Sow every few weeks for a steady supply."),
    crop!("Leek", Allium, Vegetable, spacing 15, rows 45, germ (7, 14), days 120, indoors Some((-12, -10)), transplant Some((-2, 0)), sow None, fall None, harvest 10, seed 2,
        good ["Carrot", "Celery", "Onion"], bad ["Bean", "Pea"],
        "Hill soil up the stems for longer white shanks."),
    crop!("Garlic", Allium, Vegetable, spacing 15, rows 30, germ (0, 0), days 270, indoors None, transplant None, sow None, fall Some((1, 5)), harvest 2, seed 0,
        good ["Tomato", "Carrot", "Beet", "Lettuce"], bad ["Bean", "Pea"],
        "Plant cloves in fall, mulch over winter. Cut scapes in June; harvest when the lower leaves brown."),
    crop!("Pea", Legume, Vegetable, spacing 5, rows 60, germ (7, 14), days 60, indoors None, transplant None, sow Some((-6, -4)), fall Some((-10, -8)), harvest 3, seed 3,
        good ["Carrot", "Radish", "Spinach", "Corn", "Lettuce"], bad ["Onion", "Garlic", "Leek", "Green onion"],
        "Give them a trellis. They fix nitrogen, so leafy crops do well here next year."),
    crop!("Bush bean", Legume, Vegetable, spacing 10, rows 45, germ (7, 10), days 55, indoors None, transplant None, sow Some((1, 6)), fall None, harvest 3, seed 3,
        good ["Corn", "Carrot", "Cucumber", "Potato", "Squash", "Swiss chard"], bad ["Onion", "Garlic", "Leek", "Green onion"],
        "Sow again every 3 weeks until midsummer."),
    crop!("Pole bean", Legume, Vegetable, spacing 15, rows 90, germ (7, 10), days 65, indoors None, transplant None, sow Some((1, 4)), fall None, harvest 6, seed 3,
        good ["Corn", "Squash", "Radish", "Cucumber"], bad ["Onion", "Garlic", "Leek", "Beet"],
        "Needs a pole or trellis about 2 m tall."),
    crop!("Corn", Grass, Vegetable, spacing 25, rows 75, germ (7, 10), days 80, indoors None, transplant None, sow Some((1, 4)), fall None, harvest 2, seed 2,
        good ["Bean", "Squash", "Pea", "Cucumber", "Pumpkin"], bad ["Tomato"],
        "Plant in a block of short rows, not one long row, so it pollinates."),
    crop!("Cucumber", Cucurbit, Vegetable, spacing 30, rows 120, germ (3, 10), days 55, indoors Some((-4, -3)), transplant Some((2, 3)), sow Some((1, 4)), fall None, harvest 6, seed 5,
        good ["Bean", "Pea", "Radish", "Corn", "Dill", "Marigold", "Nasturtium"], bad ["Potato", "Sage"],
        "Trellis them to save space and keep fruit clean."),
    crop!("Zucchini", Cucurbit, Vegetable, spacing 90, rows 120, germ (5, 10), days 50, indoors Some((-4, -3)), transplant Some((2, 3)), sow Some((1, 4)), fall None, harvest 8, seed 5,
        good ["Bean", "Corn", "Nasturtium"], bad ["Potato"],
        "One or two plants is usually plenty. Pick them small."),
    crop!("Winter squash", Cucurbit, Vegetable, spacing 120, rows 180, germ (5, 10), days 95, indoors Some((-4, -3)), transplant Some((2, 3)), sow Some((1, 3)), fall None, harvest 3, seed 5,
        good ["Corn", "Bean", "Nasturtium"], bad ["Potato"],
        "Harvest before hard frost and cure in the sun for a week or two."),
    crop!("Pumpkin", Cucurbit, Vegetable, spacing 150, rows 240, germ (5, 10), days 105, indoors Some((-4, -3)), transplant Some((2, 3)), sow Some((1, 3)), fall None, harvest 3, seed 5,
        good ["Corn", "Bean", "Nasturtium"], bad ["Potato"],
        "Needs lots of room: vines run 3 m or more."),
    crop!("Melon", Cucurbit, Vegetable, spacing 60, rows 150, germ (4, 10), days 85, indoors Some((-4, -3)), transplant Some((2, 4)), sow Some((2, 4)), fall None, harvest 3, seed 5,
        good ["Corn", "Nasturtium", "Radish"], bad ["Potato"],
        "Ripe when it slips off the vine with a gentle push."),
    crop!("Celery", Umbellifer, Vegetable, spacing 25, rows 60, germ (14, 21), days 100, indoors Some((-12, -10)), transplant Some((1, 3)), sow None, fall None, harvest 6, seed 3,
        good ["Leek", "Tomato", "Cabbage", "Bean"], bad ["Carrot", "Parsnip"],
        "Thirsty: never let it dry out."),
    // Herbs
    crop!("Basil", Mint, Herb, spacing 25, rows 30, germ (5, 10), days 60, indoors Some((-6, -4)), transplant Some((1, 3)), sow Some((1, 3)), fall None, harvest 12, seed 5,
        good ["Tomato", "Pepper"], bad ["Sage"],
        "Pinch off flower buds to keep the leaves coming. Hates cold."),
    crop!("Parsley", Umbellifer, Herb, spacing 20, rows 30, germ (14, 28), days 75, indoors Some((-10, -8)), transplant Some((-2, 0)), sow Some((-3, 0)), fall None, harvest 16, seed 2,
        good ["Tomato", "Asparagus", "Carrot"], bad [],
        "Slow to sprout: soak seed overnight first."),
    crop!("Cilantro", Umbellifer, Herb, spacing 15, rows 30, germ (7, 10), days 45, indoors None, transplant None, sow Some((-2, 2)), fall Some((-8, -6)), harvest 4, seed 2,
        good ["Tomato", "Spinach", "Bean"], bad ["Fennel"],
        "Bolts fast in heat: sow small amounts often."),
    crop!("Dill", Umbellifer, Herb, spacing 30, rows 45, germ (7, 21), days 45, indoors None, transplant None, sow Some((0, 4)), fall None, harvest 6, seed 3,
        good ["Cabbage", "Cucumber", "Lettuce", "Onion", "Broccoli"], bad ["Carrot", "Tomato"],
        "Attracts beneficial insects. Self-seeds readily."),
    perennial!("Chives", Allium, Herb, spacing 20, rows 30, plant Some((-2, 0)), months (5, 9),
        good ["Carrot", "Tomato", "Strawberry"], bad ["Bean", "Pea"],
        "Divide clumps every few years."),
    perennial!("Mint", Mint, Herb, spacing 45, rows 60, plant Some((0, 4)), months (6, 9),
        good ["Cabbage", "Tomato"], bad [],
        "Spreads aggressively: grow it in a pot or a contained bed."),
    perennial!("Thyme", Mint, Herb, spacing 25, rows 45, plant Some((0, 2)), months (5, 10),
        good ["Cabbage", "Eggplant", "Strawberry"], bad [],
        "Likes it dry and sunny. Trim after flowering."),
    perennial!("Oregano", Mint, Herb, spacing 30, rows 45, plant Some((0, 2)), months (6, 9),
        good ["Tomato", "Pepper", "Bean"], bad [],
        "Flavour is strongest just before it flowers."),
    perennial!("Sage", Mint, Herb, spacing 45, rows 60, plant Some((0, 2)), months (5, 10),
        good ["Cabbage", "Carrot", "Rosemary"], bad ["Cucumber", "Basil"],
        "Replace woody plants every 4 or 5 years."),
    crop!("Rosemary", Mint, Herb, spacing 60, rows 90, germ (14, 21), days 90, indoors None, transplant Some((1, 3)), sow None, fall None, harvest 20, seed 1,
        good ["Carrot", "Bean", "Cabbage", "Sage"], bad [],
        "Not hardy in cold winters: bring it inside or treat it as an annual."),
    // Fruit
    perennial!("Strawberry", Rose, Fruit, spacing 30, rows 90, plant Some((-2, 2)), months (6, 7),
        good ["Spinach", "Lettuce", "Bean", "Onion", "Thyme"], bad ["Cabbage", "Broccoli", "Kale", "Cauliflower"],
        "Pinch off first-year flowers for bigger crops later. Renew the bed every 3 or 4 years."),
    perennial!("Raspberry", Rose, Fruit, spacing 60, rows 180, plant Some((-4, 0)), months (7, 9),
        good ["Garlic", "Marigold"], bad ["Potato", "Tomato", "Eggplant"],
        "Cut out canes that have fruited, after harvest."),
    perennial!("Blueberry", Other, Fruit, spacing 120, rows 240, plant Some((-4, 0)), months (7, 8),
        good ["Strawberry", "Thyme"], bad [],
        "Needs acidic soil (pH 4.5 to 5.5). Plant two varieties for better crops."),
    perennial!("Rhubarb", Other, Vegetable, spacing 90, rows 120, plant Some((-4, -2)), months (5, 7),
        good ["Garlic", "Onion", "Cabbage"], bad [],
        "Don't harvest the first year. Leaves are poisonous; eat only the stalks."),
    perennial!("Asparagus", Other, Vegetable, spacing 45, rows 120, plant Some((-4, -2)), months (5, 6),
        good ["Tomato", "Parsley", "Basil"], bad ["Onion", "Garlic", "Potato"],
        "Wait 2 years before the first real harvest. Beds last 20 years."),
    // Flowers
    crop!("Marigold", Aster, Flower, spacing 25, rows 30, germ (5, 10), days 50, indoors Some((-6, -4)), transplant Some((0, 2)), sow Some((0, 2)), fall None, harvest 14, seed 2,
        good ["Tomato", "Pepper", "Bean", "Potato", "Cucumber"], bad [],
        "Planted around vegetables, they help deter pests."),
    crop!("Sunflower", Aster, Flower, spacing 45, rows 75, germ (7, 14), days 80, indoors None, transplant None, sow Some((1, 4)), fall None, harvest 4, seed 5,
        good ["Cucumber", "Corn"], bad ["Potato", "Pole bean"],
        "Plant on the north side so it doesn't shade other crops."),
    crop!("Nasturtium", Other, Flower, spacing 30, rows 30, germ (7, 14), days 50, indoors None, transplant None, sow Some((1, 3)), fall None, harvest 14, seed 5,
        good ["Cucumber", "Squash", "Zucchini", "Bean", "Tomato", "Radish"], bad [],
        "Edible flowers and leaves; a trap crop for aphids."),
    crop!("Zinnia", Aster, Flower, spacing 30, rows 45, germ (5, 10), days 70, indoors Some((-6, -4)), transplant Some((1, 2)), sow Some((1, 3)), fall None, harvest 12, seed 4,
        good ["Tomato", "Bean"], bad [],
        "Cut-and-come-again: the more you cut, the more it flowers."),
    crop!("Cosmos", Aster, Flower, spacing 30, rows 45, germ (5, 10), days 75, indoors Some((-6, -4)), transplant Some((1, 2)), sow Some((1, 3)), fall None, harvest 12, seed 3,
        good ["Tomato", "Squash", "Pepper"], bad [],
        "Thrives in poor soil; too much fertilizer means leaves, not flowers."),
    crop!("Calendula", Aster, Flower, spacing 25, rows 30, germ (7, 14), days 55, indoors Some((-6, -4)), transplant Some((0, 2)), sow Some((-2, 2)), fall None, harvest 14, seed 5,
        good ["Tomato", "Carrot", "Asparagus"], bad [],
        "Edible petals. Keeps flowering if you pick the spent heads."),
    crop!("Snapdragon", Other, Flower, spacing 25, rows 30, germ (10, 14), days 100, indoors Some((-10, -8)), transplant Some((-2, 0)), sow None, fall None, harvest 10, seed 3,
        good [], bad [],
        "Likes cool weather. Pinch young plants for bushier stems."),
    crop!("Petunia", Nightshade, Flower, spacing 30, rows 30, germ (7, 14), days 90, indoors Some((-10, -8)), transplant Some((1, 2)), sow None, fall None, harvest 16, seed 3,
        good ["Bean", "Tomato"], bad [],
        "Seed is tiny and needs light to sprout: press it onto the soil, don't cover it."),
    crop!("Sweet alyssum", Brassica, Flower, spacing 20, rows 20, germ (5, 10), days 50, indoors Some((-6, -4)), transplant Some((0, 2)), sow Some((-2, 2)), fall None, harvest 16, seed 4,
        good ["Lettuce", "Potato", "Bean"], bad [],
        "Low and spreading: good edging. Draws in hoverflies that eat aphids."),
    crop!("Dahlia", Aster, Flower, spacing 60, rows 75, germ (0, 0), days 100, indoors None, transplant Some((1, 3)), sow None, fall None, harvest 10, seed 0,
        good [], bad [],
        "Plant tubers once the soil warms. In cold winters, dig them up after frost and store them dry."),
    perennial!("Lavender", Mint, Flower, spacing 45, rows 60, plant Some((0, 2)), months (6, 8),
        good ["Cabbage", "Rosemary"], bad [],
        "Needs sun and sharp drainage. Trim after flowering, never into old wood."),
    perennial!("Coneflower", Aster, Flower, spacing 45, rows 60, plant Some((0, 2)), months (7, 9),
        good [], bad [],
        "Drought tolerant once settled. Leave seed heads for the birds."),
    perennial!("Black-eyed Susan", Aster, Flower, spacing 45, rows 60, plant Some((0, 2)), months (7, 9),
        good [], bad [],
        "Self-seeds; divide clumps every few years."),
    bulb!("Tulip", Other, spacing 12, rows 15, fall Some((-2, 4)), months (4, 5),
        "Plant bulbs pointy end up, about 15 to 20 cm deep. Let the leaves die back before cutting them."),
    bulb!("Daffodil", Other, spacing 15, rows 20, fall Some((-2, 4)), months (3, 4),
        "Squirrels leave them alone. Plant in clumps for the best show."),
    bulb!("Peony", Other, spacing 90, rows 90, fall Some((-4, 2)), months (5, 6),
        "Plant the eyes no more than 5 cm deep or it won't flower. Lives for decades."),
    crop!("Sweet pea", Legume, Flower, spacing 15, rows 45, germ (10, 14), days 80, indoors Some((-8, -6)), transplant Some((-2, 0)), sow Some((-6, -4)), fall None, harvest 8, seed 3,
        good ["Carrot", "Radish", "Lettuce"], bad ["Onion", "Garlic"],
        "Grow up a trellis; flowers are not edible."),
];

/// Crops matching a search: by name, or by kind ("flower", "herb").
pub fn search_crops(query: &str) -> Vec<&'static Crop> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return CROPS.iter().collect();
    }
    let mut starts: Vec<&Crop> = Vec::new();
    let mut rest: Vec<&Crop> = Vec::new();
    for c in CROPS {
        let name = c.name.to_lowercase();
        let kind = c.kind.label().to_lowercase();
        let family_word = q.len() >= 4 && c.family.label().split(|ch: char| !ch.is_alphabetic()).any(|w| w.starts_with(&q));
        if name.starts_with(&q) || name.split([' ', '-']).any(|w| w.starts_with(&q)) {
            starts.push(c);
        } else if (q.len() >= 3 && name.contains(&q)) || kind.starts_with(&q) || family_word {
            rest.push(c);
        }
    }
    starts.extend(rest);
    starts
}

pub fn crop(name: &str) -> Option<&'static Crop> {
    let n = name.trim();
    CROPS.iter().find(|c| c.name.eq_ignore_ascii_case(n))
}

/// Whether `entry` from a companion list (like "Bean") names `crop_name`
/// (like "Pole bean").
fn names(entry: &str, crop_name: &str) -> bool {
    let e = entry.to_lowercase();
    let c = crop_name.to_lowercase();
    c == e || c.split_whitespace().any(|w| w == e || w.trim_end_matches('s') == e)
}

// ---------------------------------------------------------------------------
// Plantings

/// How a planting is laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// A line of plants (two or more points).
    #[default]
    Row,
    /// A block (an outline).
    Patch,
    /// One plant (one point, with its spread as `size`).
    Plant,
}

impl Layout {
    pub fn label(self) -> &'static str {
        match self {
            Layout::Row => "Row",
            Layout::Patch => "Patch",
            Layout::Plant => "Plant",
        }
    }
}

/// How it got started.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// Seeds sown straight into the bed.
    #[default]
    Sown,
    /// Seeds started indoors, then transplanted.
    StartedIndoors,
    /// Bought plants (seedlings, crowns, canes, sets...).
    Bought,
}

impl Method {
    pub const ALL: [Method; 3] = [Method::Sown, Method::StartedIndoors, Method::Bought];
    pub fn label(self) -> &'static str {
        match self {
            Method::Sown => "Sown in the bed",
            Method::StartedIndoors => "Started indoors",
            Method::Bought => "Bought plants",
        }
    }
    /// Best way to grow a crop if nobody says otherwise.
    pub fn default_for(c: Option<&Crop>) -> Method {
        match c {
            Some(c) if c.perennial => Method::Bought,
            Some(c) if c.direct_sow.is_some() || c.fall_sow.is_some() => Method::Sown,
            Some(c) if c.start_indoors.is_some() => Method::StartedIndoors,
            Some(_) => Method::Bought,
            None => Method::Sown,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Harvest {
    pub date: NaiveDate,
    #[serde(default)]
    pub amount: Option<f64>,
    /// "kg", "lb", "bunches", "heads"...
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub notes: String,
}

pub const HARVEST_UNITS: &[&str] = &["kg", "g", "lb", "oz", "pieces", "bunches", "heads", "bags", "baskets"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Planting {
    pub id: PlantingId,
    /// The garden bed it's in.
    pub garden: FeatureId,
    /// The growing year (fall-planted garlic: the year it went in).
    pub year: i32,
    #[serde(default)]
    pub layout: Layout,
    /// Row: the line; Patch: the outline; Plant: one point.
    pub points: Vec<Vec2>,
    /// Plant: how wide it grows.
    #[serde(default)]
    pub size: f64,
    /// Crop name; matches [`CROPS`] when it's a common one.
    pub crop: String,
    #[serde(default)]
    pub variety: String,
    #[serde(default)]
    pub seed: Option<SeedId>,
    #[serde(default)]
    pub method: Method,
    /// How many plants (blank: worked out from the spacing).
    #[serde(default)]
    pub count: Option<u32>,
    /// Between plants (m).
    #[serde(default)]
    pub spacing: Option<f64>,
    /// Overrides the crop's days to harvest.
    #[serde(default)]
    pub days_to_harvest: Option<u32>,
    #[serde(default)]
    pub started: Option<NaiveDate>,
    /// Sown in the bed, or transplanted/planted out.
    #[serde(default)]
    pub planted: Option<NaiveDate>,
    /// Pulled out or finished.
    #[serde(default)]
    pub removed: Option<NaiveDate>,
    #[serde(default)]
    pub harvests: Vec<Harvest>,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
    /// For plants not in the crop list: whether it comes back every year.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub returns: bool,
}

impl Planting {
    pub fn new(garden: FeatureId, year: i32, layout: Layout, points: Vec<Vec2>, crop_name: &str) -> Self {
        let c = crop(crop_name);
        Self {
            id: PlantingId::new(),
            garden,
            year,
            layout,
            points,
            size: c.map_or(0.4, |c| (c.spacing_cm as f64 / 100.0).max(0.15)),
            crop: crop_name.to_string(),
            variety: String::new(),
            seed: None,
            method: Method::default_for(c),
            count: None,
            spacing: None,
            days_to_harvest: None,
            started: None,
            planted: None,
            removed: None,
            harvests: Vec::new(),
            color: None,
            custom_fields: Vec::new(),
            returns: false,
        }
    }

    pub fn info(&self) -> Option<&'static Crop> {
        crop(&self.crop)
    }

    pub fn perennial(&self) -> bool {
        self.info().map_or(self.returns, |c| c.perennial)
    }

    pub fn family(&self) -> Family {
        self.info().map_or(Family::Other, |c| c.family)
    }

    /// "Tomato ‘Sungold’".
    pub fn display_name(&self) -> String {
        let crop = if self.crop.trim().is_empty() { "Planting" } else { self.crop.trim() };
        if self.variety.trim().is_empty() {
            crop.to_string()
        } else {
            format!("{crop} ‘{}’", self.variety.trim())
        }
    }

    /// Spacing between plants: the planting's own, or the crop's.
    pub fn spacing_m(&self) -> f64 {
        self.spacing
            .or_else(|| self.info().map(|c| c.spacing_cm as f64 / 100.0))
            .unwrap_or(0.3)
            .max(0.01)
    }

    pub fn row_spacing_m(&self) -> f64 {
        self.info().map_or(0.45, |c| c.row_cm as f64 / 100.0).max(self.spacing_m())
    }

    pub fn length(&self) -> f64 {
        match self.layout {
            Layout::Row => polyline_length(&self.points),
            _ => 0.0,
        }
    }

    pub fn area(&self) -> f64 {
        match self.layout {
            Layout::Patch => area(&self.points),
            Layout::Plant => std::f64::consts::PI * (self.size / 2.0).powi(2),
            Layout::Row => 0.0,
        }
    }

    /// How many plants fit, from the spacing.
    pub fn estimated_count(&self) -> u32 {
        let s = self.spacing_m();
        match self.layout {
            Layout::Row => (self.length() / s).floor() as u32 + 1,
            Layout::Patch => ((self.area() / (s * self.row_spacing_m())).round() as u32).max(1),
            Layout::Plant => 1,
        }
    }

    pub fn plant_count(&self) -> u32 {
        self.count.unwrap_or_else(|| self.estimated_count())
    }

    /// Whether it's in the garden in `year`: planted that year, or a
    /// perennial (or overwintering crop) still in the ground.
    pub fn in_year(&self, year: i32) -> bool {
        if self.year == year {
            return true;
        }
        if self.year > year {
            return false;
        }
        let gone = self.removed.map(|d| d.year());
        if self.perennial() {
            return gone.is_none_or(|g| g >= year);
        }
        // Fall-planted crops (garlic) carry on into the next year.
        let fall = self.info().is_some_and(|c| c.fall_sow.is_some() && c.direct_sow.is_none() && c.transplant.is_none());
        fall && self.year + 1 == year && gone.is_none_or(|g| g >= year)
    }

    pub fn contains(&self, p: Vec2) -> bool {
        match self.layout {
            Layout::Patch => point_in_polygon(p, &self.points),
            Layout::Plant => self.points.first().is_some_and(|c| c.distance(p) <= self.size / 2.0),
            Layout::Row => false,
        }
    }

    /// Distance from `p` to the planting's edge (0 inside a patch or plant).
    pub fn distance_to(&self, p: Vec2) -> f64 {
        match self.layout {
            Layout::Row => distance_to_polyline(&self.points, p),
            Layout::Patch => {
                if point_in_polygon(p, &self.points) {
                    0.0
                } else {
                    let mut closed = self.points.clone();
                    closed.extend(self.points.first().copied());
                    distance_to_polyline(&closed, p)
                }
            }
            Layout::Plant => self.points.first().map_or(f64::INFINITY, |c| (c.distance(p) - self.size / 2.0).max(0.0)),
        }
    }

    /// The gap between two plantings (0 if they touch or overlap).
    pub fn gap(&self, other: &Planting) -> f64 {
        let edge = |a: &Planting, b: &Planting| {
            let pts: Vec<Vec2> = match a.layout {
                Layout::Plant => a.points.iter().take(1).copied().collect(),
                _ => a.points.clone(),
            };
            let shrink = if a.layout == Layout::Plant { a.size / 2.0 } else { 0.0 };
            let mut best = pts.iter().map(|p| b.distance_to(*p) - shrink).fold(f64::INFINITY, f64::min);
            // Segment interiors (a row crossing a patch has no corner inside it).
            if a.layout != Layout::Plant {
                let mut segs = a.points.clone();
                if a.layout == Layout::Patch {
                    segs.extend(a.points.first().copied());
                }
                for w in segs.windows(2) {
                    for t in [0.25, 0.5, 0.75] {
                        best = best.min(b.distance_to(w[0] + (w[1] - w[0]) * t));
                    }
                }
            }
            best
        };
        edge(self, other).min(edge(other, self)).max(0.0)
    }

    /// Bounding box, grown by the plant's spread.
    pub fn bounds(&self) -> Option<(Vec2, Vec2)> {
        let (lo, hi) = bounds(&self.points)?;
        let r = match self.layout {
            Layout::Plant => self.size / 2.0,
            Layout::Row => self.spacing_m() / 2.0,
            Layout::Patch => 0.0,
        };
        Some((lo - Vec2::new(r, r), hi + Vec2::new(r, r)))
    }

    fn days(&self, seed: Option<&SeedPacket>) -> Option<i64> {
        self.days_to_harvest
            .or(seed.and_then(|s| s.days_to_harvest))
            .map(|d| d as i64)
            .or_else(|| self.info().filter(|c| !c.perennial).map(|c| c.maturity_days as i64))
            .filter(|d| *d > 0)
    }

    pub fn total_harvest(&self) -> Vec<(String, f64)> {
        let mut out: Vec<(String, f64)> = Vec::new();
        for h in &self.harvests {
            let Some(a) = h.amount else { continue };
            match out.iter_mut().find(|(u, _)| *u == h.unit) {
                Some((_, t)) => *t += a,
                None => out.push((h.unit.clone(), a)),
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Seeds

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeedPacket {
    pub id: SeedId,
    pub crop: String,
    #[serde(default)]
    pub variety: String,
    /// Brand or where they came from ("saved from 2025").
    #[serde(default)]
    pub source: String,
    /// "Packed for" year on the packet.
    #[serde(default)]
    pub packed_for: Option<i32>,
    #[serde(default)]
    pub bought: Option<NaiveDate>,
    /// What's left: "about half", "30 seeds"...
    #[serde(default)]
    pub amount: String,
    /// Germination rate from the packet or your own test (%).
    #[serde(default)]
    pub germination: Option<f64>,
    #[serde(default)]
    pub days_to_harvest: Option<u32>,
    #[serde(default)]
    pub organic: bool,
    #[serde(default)]
    pub used_up: bool,
    #[serde(default)]
    pub notes: String,
}

impl SeedPacket {
    pub fn new(crop: &str) -> Self {
        Self {
            id: SeedId::new(),
            crop: crop.to_string(),
            variety: String::new(),
            source: String::new(),
            packed_for: None,
            bought: None,
            amount: String::new(),
            germination: None,
            days_to_harvest: None,
            organic: false,
            used_up: false,
            notes: String::new(),
        }
    }

    pub fn display_name(&self) -> String {
        let crop = if self.crop.trim().is_empty() { "Seeds" } else { self.crop.trim() };
        if self.variety.trim().is_empty() {
            crop.to_string()
        } else {
            format!("{crop} ‘{}’", self.variety.trim())
        }
    }

    /// The last year the seed is likely still good, from the crop's usual
    /// shelf life.
    pub fn good_until(&self) -> Option<i32> {
        let year = self.packed_for.or(self.bought.map(|d| d.year()))?;
        let life = crop(&self.crop)?.seed_years;
        (life > 0).then(|| year + life as i32 - 1)
    }

    pub fn is_old(&self, year: i32) -> bool {
        self.good_until().is_some_and(|y| y < year)
    }
}

// ---------------------------------------------------------------------------
// Schedule

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    StartIndoors,
    Transplant,
    Sow,
    /// Perennials and bought plants.
    PlantOut,
    Harvest,
}

impl Step {
    pub fn label(self) -> &'static str {
        match self {
            Step::StartIndoors => "Start indoors",
            Step::Transplant => "Transplant",
            Step::Sow => "Sow",
            Step::PlantOut => "Plant",
            Step::Harvest => "Harvest",
        }
    }
}

/// One step of the calendar: when it should happen, and when it did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage {
    pub step: Step,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub done: Option<NaiveDate>,
    /// Worked out from what actually happened (rather than frost dates).
    pub firm: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Planned,
    StartedIndoors,
    Growing,
    Harvesting,
    Finished,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Planned => "Planned",
            Status::StartedIndoors => "Started indoors",
            Status::Growing => "Growing",
            Status::Harvesting => "Harvesting",
            Status::Finished => "Finished",
        }
    }
}

fn window(base: Option<NaiveDate>, w: Weeks) -> Option<(NaiveDate, NaiveDate)> {
    let (a, b) = w?;
    let base = base?;
    Some((base + Duration::weeks(a as i64), base + Duration::weeks(b as i64)))
}

impl Planting {
    /// The calendar for this planting: planned windows from the frost dates,
    /// firmed up by the dates you've filled in.
    pub fn stages(&self, settings: &GardenSettings, seed: Option<&SeedPacket>) -> Vec<Stage> {
        let c = self.info();
        let last = settings.last_frost_in(self.year);
        let first = settings.first_frost_in(self.year);
        let mut out = Vec::new();
        let plan = |w: Option<(NaiveDate, NaiveDate)>, done: Option<NaiveDate>, step: Step| -> Option<Stage> {
            match (w, done) {
                (Some((from, to)), _) => Some(Stage { step, from, to, done, firm: false }),
                (None, Some(d)) => Some(Stage { step, from: d, to: d, done, firm: true }),
                (None, None) => None,
            }
        };
        let fall_only = c.is_some_and(|c| c.direct_sow.is_none() && c.fall_sow.is_some());
        match self.method {
            Method::StartedIndoors => {
                out.extend(plan(c.and_then(|c| window(last, c.start_indoors)), self.started, Step::StartIndoors));
                out.extend(plan(c.and_then(|c| window(last, c.transplant)), self.planted, Step::Transplant));
            }
            Method::Sown => {
                let w = c.and_then(|c| if fall_only { window(first, c.fall_sow) } else { window(last, c.direct_sow) });
                out.extend(plan(w, self.planted, Step::Sow));
            }
            Method::Bought => {
                let w = c.and_then(|c| if fall_only { window(first, c.fall_sow) } else { window(last, c.transplant.or(c.direct_sow)) });
                out.extend(plan(w, self.planted, Step::PlantOut));
            }
        }
        // Harvest: from when it went in (or was meant to), plus days to harvest.
        if let Some(c) = c.filter(|c| c.perennial) {
            if let Some((m0, m1)) = c.harvest_months {
                // Perennials: their usual months, the year after planting at the earliest.
                let year = self.year + 1;
                let from = NaiveDate::from_ymd_opt(year, m0 as u32, 1);
                let to = NaiveDate::from_ymd_opt(year, m1 as u32 % 12 + 1, 1).map(|d| d - Duration::days(1));
                if let (Some(from), Some(to)) = (from, to) {
                    out.push(Stage { step: Step::Harvest, from, to, done: self.harvests.first().map(|h| h.date), firm: false });
                }
            }
        } else if let Some(days) = self.days(seed) {
            let out_step = out.iter().rev().find(|s| s.step != Step::StartIndoors).copied();
            let (base, firm) = match (self.planted, out_step) {
                (Some(d), _) => (Some(d), true),
                (None, Some(s)) => (Some(s.from), false),
                (None, None) => (None, false),
            };
            if let Some(base) = base {
                let from = base + Duration::days(days);
                let weeks = c.map_or(3, |c| c.harvest_weeks.max(1)) as i64;
                let mut to = from + Duration::weeks(weeks);
                if let Some(f) = first.filter(|f| *f > from && *f < to && c.is_some_and(|c| c.family == Family::Nightshade || c.family == Family::Cucurbit || c.name == "Basil")) {
                    // Frost-tender crops stop at the first frost.
                    to = f;
                }
                out.push(Stage { step: Step::Harvest, from, to, done: self.harvests.first().map(|h| h.date), firm });
            }
        }
        out
    }

    pub fn status(&self, today: NaiveDate, settings: &GardenSettings, seed: Option<&SeedPacket>) -> Status {
        if self.removed.is_some_and(|d| d <= today) {
            return Status::Finished;
        }
        let harvest = self.stages(settings, seed).into_iter().find(|s| s.step == Step::Harvest);
        let in_ground = self.planted.is_some_and(|d| d <= today);
        // An annual whose harvest window is over is done for the year.
        if in_ground && !self.perennial() && harvest.is_some_and(|h| h.to < today) {
            return Status::Finished;
        }
        if !self.harvests.is_empty() || (in_ground && harvest.is_some_and(|h| h.from <= today)) {
            return Status::Harvesting;
        }
        if in_ground || (self.perennial() && self.year < today.year()) {
            return Status::Growing;
        }
        if self.started.is_some_and(|d| d <= today) {
            return Status::StartedIndoors;
        }
        Status::Planned
    }

    /// The next thing to do, if any: a step not done yet whose window hasn't
    /// passed, or the harvest coming up.
    pub fn next_step(&self, today: NaiveDate, settings: &GardenSettings, seed: Option<&SeedPacket>) -> Option<Stage> {
        if self.removed.is_some() {
            return None;
        }
        self.stages(settings, seed)
            .into_iter()
            .find(|s| s.done.is_none() && s.to >= today)
    }
}

// ---------------------------------------------------------------------------
// Hints: rotation and neighbours

#[derive(Clone, Debug, PartialEq)]
pub enum Hint {
    /// Same family grew here recently.
    Rotation { other: PlantingId, year: i32, family: Family },
    Good { other: PlantingId },
    Bad { other: PlantingId },
}

/// How close two plantings must be to count as neighbours.
pub const NEIGHBOUR_GAP: f64 = 0.6;

// ---------------------------------------------------------------------------
// Care

pub struct CareTemplate {
    pub title: &'static str,
    pub every: u32,
    pub unit: RecurrenceUnit,
}

/// Repeating care, for a garden bed or a planting.
pub const CARE: &[CareTemplate] = &[
    CareTemplate { title: "Water deeply", every: 3, unit: RecurrenceUnit::Days },
    CareTemplate { title: "Weed", every: 1, unit: RecurrenceUnit::Weeks },
    CareTemplate { title: "Check for pests", every: 1, unit: RecurrenceUnit::Weeks },
    CareTemplate { title: "Fertilize", every: 3, unit: RecurrenceUnit::Weeks },
    CareTemplate { title: "Tie up and prune", every: 2, unit: RecurrenceUnit::Weeks },
];

/// Once a year: (title, month, day).
pub const SEASONAL_CARE: &[(&str, u32, u32)] = &[
    ("Order seeds", 1, 15),
    ("Test the soil", 4, 1),
    ("Add compost to the beds", 4, 15),
    ("Mulch the beds", 6, 1),
    ("Clean up and put the garden to bed", 10, 30),
];

pub fn next_on(today: NaiveDate, month: u32, day: u32) -> NaiveDate {
    let this = NaiveDate::from_ymd_opt(today.year(), month, day).unwrap_or(today);
    if this >= today {
        this
    } else {
        NaiveDate::from_ymd_opt(today.year() + 1, month, day).unwrap_or(today)
    }
}

// ---------------------------------------------------------------------------
// Project-level helpers

impl Project {
    pub fn planting(&self, id: PlantingId) -> Option<&Planting> {
        self.plantings.iter().find(|p| p.id == id)
    }

    pub fn planting_mut(&mut self, id: PlantingId) -> Option<&mut Planting> {
        self.plantings.iter_mut().find(|p| p.id == id)
    }

    pub fn seed(&self, id: SeedId) -> Option<&SeedPacket> {
        self.seeds.iter().find(|s| s.id == id)
    }

    pub fn is_garden(&self, id: FeatureId) -> bool {
        self.find_feature(id).is_some_and(|(_, f)| f.kind.is_garden())
    }

    /// What's in a garden in `year`, drawn bottom to top (patches first).
    pub fn plantings_in(&self, garden: FeatureId, year: i32) -> Vec<&Planting> {
        let mut v: Vec<&Planting> = self.plantings.iter().filter(|p| p.garden == garden && p.in_year(year)).collect();
        v.sort_by_key(|p| match p.layout {
            Layout::Patch => 0,
            Layout::Row => 1,
            Layout::Plant => 2,
        });
        v
    }

    /// Years with something planted in a garden, newest first.
    pub fn garden_years(&self, garden: FeatureId) -> Vec<i32> {
        let mut v: Vec<i32> = self.plantings.iter().filter(|p| p.garden == garden).map(|p| p.year).collect();
        v.sort_unstable();
        v.dedup();
        v.reverse();
        v
    }

    /// Rotation and companion hints for a planting.
    pub fn hints(&self, id: PlantingId) -> Vec<Hint> {
        let Some(p) = self.planting(id) else { return Vec::new() };
        let mut out = Vec::new();
        let family = p.family();
        if family.rotates() && !p.perennial() {
            let mut seen = Vec::new();
            for back in 1..=2 {
                let year = p.year - back;
                for o in self.plantings.iter().filter(|o| o.garden == p.garden && o.year == year && o.id != p.id) {
                    if o.family() == family && !seen.contains(&o.id) && p.gap(o) < 0.15 {
                        seen.push(o.id);
                        out.push(Hint::Rotation { other: o.id, year, family });
                    }
                }
            }
        }
        if let Some(c) = p.info() {
            for o in self.plantings.iter().filter(|o| o.garden == p.garden && o.id != p.id && o.in_year(p.year)) {
                if o.crop.eq_ignore_ascii_case(&p.crop) || p.gap(o) > NEIGHBOUR_GAP {
                    continue;
                }
                let theirs = o.info();
                let bad = c.avoid.iter().any(|e| names(e, &o.crop)) || theirs.is_some_and(|t| t.avoid.iter().any(|e| names(e, &p.crop)));
                let good = c.companions.iter().any(|e| names(e, &o.crop)) || theirs.is_some_and(|t| t.companions.iter().any(|e| names(e, &p.crop)));
                if bad {
                    out.push(Hint::Bad { other: o.id });
                } else if good {
                    out.push(Hint::Good { other: o.id });
                }
            }
        }
        out
    }

    /// Add reminders for the planting's upcoming steps (start indoors, sow,
    /// transplant, harvest). Skips ones already there and ones in the past.
    /// Returns how many were added.
    pub fn add_planting_reminders(&mut self, id: PlantingId, today: NaiveDate) -> usize {
        let Some(p) = self.planting(id) else { return 0 };
        let seed = p.seed.and_then(|s| self.seed(s));
        let name = p.display_name();
        let target = EntityRef::Planting(id);
        let mut add = Vec::new();
        for s in p.stages(&self.garden, seed) {
            if s.done.is_some() || s.to < today {
                continue;
            }
            let title = match s.step {
                Step::StartIndoors => format!("Start {name} indoors"),
                Step::Transplant => format!("Harden off and transplant {name}"),
                Step::Sow => format!("Sow {name}"),
                Step::PlantOut => format!("Plant {name}"),
                Step::Harvest => format!("Check {name} for harvest"),
            };
            let due = s.from.max(today);
            if !self.reminders.iter().any(|r| r.target == target && r.title == title) {
                add.push(Reminder::new(target, title, due));
            }
        }
        let n = add.len();
        self.reminders.extend(add);
        n
    }

    /// Harvest totals for a garden in a year, by crop and unit.
    pub fn harvest_totals(&self, garden: FeatureId, year: i32) -> Vec<(String, String, f64)> {
        let mut out: Vec<(String, String, f64)> = Vec::new();
        for p in self.plantings.iter().filter(|p| p.garden == garden) {
            for h in p.harvests.iter().filter(|h| h.date.year() == year) {
                let Some(a) = h.amount else { continue };
                let crop = p.crop.trim().to_string();
                match out.iter_mut().find(|(c, u, _)| *c == crop && *u == h.unit) {
                    Some((_, _, t)) => *t += a,
                    None => out.push((crop, h.unit.clone(), a)),
                }
            }
        }
        out
    }
}

impl Crop {
    /// "harvest", or "bloom" for flowers.
    pub fn is_flower(&self) -> bool {
        self.kind == CropKind::Flower
    }
}

impl FeatureKind {
    /// Areas you can plan plantings in.
    pub fn is_garden(self) -> bool {
        self == FeatureKind::GardenBed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::rect_points;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn garden(p: &mut Project) -> FeatureId {
        p.ensure_site();
        p.add_feature(FeatureKind::GardenBed, rect_points(Vec2::ZERO, 4.0, 2.0)).unwrap()
    }

    fn settings() -> GardenSettings {
        GardenSettings { last_frost: Some(d(2000, 5, 10)), first_frost: Some(d(2000, 10, 10)), zone: "6a".into() }
    }

    #[test]
    fn library_is_consistent() {
        for c in CROPS {
            assert_eq!(CROPS.iter().filter(|x| x.name.eq_ignore_ascii_case(c.name)).count(), 1, "duplicate crop {}", c.name);
            if !c.perennial {
                assert!(c.maturity_days > 0, "{} has no days to harvest", c.name);
                assert!(c.direct_sow.is_some() || c.transplant.is_some() || c.fall_sow.is_some(), "{} can't be planted", c.name);
            }
            for w in [c.start_indoors, c.transplant, c.direct_sow, c.fall_sow].into_iter().flatten() {
                assert!(w.0 <= w.1, "{} has a backwards window", c.name);
            }
        }
    }

    #[test]
    fn tomato_calendar_from_frost_dates() {
        let mut p = Project::new("T");
        let g = garden(&mut p);
        let mut t = Planting::new(g, 2026, Layout::Row, vec![Vec2::new(0.5, 0.5), Vec2::new(3.5, 0.5)], "Tomato");
        assert_eq!(t.method, Method::StartedIndoors);
        let s = settings();
        let stages = t.stages(&s, None);
        let steps: Vec<Step> = stages.iter().map(|s| s.step).collect();
        assert_eq!(steps, vec![Step::StartIndoors, Step::Transplant, Step::Harvest]);
        // Start 8 to 6 weeks before May 10; transplant 1 to 3 weeks after.
        assert_eq!(stages[0].from, d(2026, 3, 15));
        assert_eq!(stages[1].from, d(2026, 5, 17));
        // 70 days after the planned transplant, for 10 weeks.
        assert_eq!(stages[2].from, d(2026, 7, 26));
        assert_eq!(stages[2].to, d(2026, 10, 4));
        assert!(!stages[2].firm);
        // Once it's actually in, harvest counts from that day.
        t.planted = Some(d(2026, 5, 24));
        let h = t.stages(&s, None).into_iter().find(|s| s.step == Step::Harvest).unwrap();
        assert_eq!(h.from, d(2026, 8, 2));
        assert!(h.firm);
        assert_eq!(t.status(d(2026, 6, 1), &s, None), Status::Growing);
        assert_eq!(t.status(d(2026, 8, 3), &s, None), Status::Harvesting);
        assert_eq!(t.status(d(2026, 11, 1), &s, None), Status::Finished);
        // A 3 m row at 60 cm.
        assert_eq!(t.estimated_count(), 6);
    }

    #[test]
    fn garlic_is_planted_in_fall_and_carries_over() {
        let mut p = Project::new("T");
        let g = garden(&mut p);
        let gar = Planting::new(g, 2025, Layout::Patch, rect_points(Vec2::ZERO, 1.0, 1.0), "Garlic");
        let s = settings();
        let sow = gar.stages(&s, None)[0];
        assert_eq!(sow.step, Step::Sow);
        assert_eq!(sow.from, d(2025, 10, 17));
        assert!(gar.in_year(2026) && !gar.in_year(2027));
    }

    #[test]
    fn perennials_stay_until_removed() {
        let mut p = Project::new("T");
        let g = garden(&mut p);
        let mut s = Planting::new(g, 2024, Layout::Patch, rect_points(Vec2::ZERO, 1.0, 1.0), "Strawberry");
        assert!(s.in_year(2026));
        s.removed = Some(d(2025, 9, 1));
        assert!(s.in_year(2025) && !s.in_year(2026));
    }

    #[test]
    fn rotation_and_companion_hints() {
        let mut p = Project::new("T");
        let g = garden(&mut p);
        let row = |y: f64| vec![Vec2::new(0.2, y), Vec2::new(3.8, y)];
        let old = Planting::new(g, 2025, Layout::Row, row(0.5), "Potato");
        let tom = Planting::new(g, 2026, Layout::Row, row(0.5), "Tomato");
        let basil = Planting::new(g, 2026, Layout::Row, row(0.9), "Basil");
        let fennel_free = Planting::new(g, 2026, Layout::Row, row(1.8), "Corn");
        let (old_id, tom_id, basil_id) = (old.id, tom.id, basil.id);
        p.plantings.extend([old, tom, basil, fennel_free]);
        let hints = p.hints(tom_id);
        assert!(hints.contains(&Hint::Rotation { other: old_id, year: 2025, family: Family::Nightshade }));
        assert!(hints.contains(&Hint::Good { other: basil_id }));
        // Corn is 1.3 m away: not a neighbour.
        assert_eq!(hints.len(), 2);
    }

    #[test]
    fn search_finds_by_name_then_kind() {
        let names = |q: &str| search_crops(q).iter().map(|c| c.name).collect::<Vec<_>>();
        assert_eq!(names("tom")[0], "Tomato");
        assert!(names("bean").starts_with(&["Bush bean", "Pole bean"]));
        let flowers = names("flower");
        assert!(flowers.contains(&"Tulip") && flowers.contains(&"Marigold") && !flowers.contains(&"Tomato"));
        assert!(names("zzz").is_empty());
        assert_eq!(names("be"), vec!["Beet", "Bush bean", "Pole bean"]);
        assert!(names("brassica").contains(&"Kale"));
        assert!(names("susan").contains(&"Black-eyed Susan"));
        assert_eq!(search_crops("").len(), CROPS.len());
    }

    #[test]
    fn custom_plants_can_come_back_every_year() {
        let mut p = Project::new("T");
        let g = garden(&mut p);
        let mut h = Planting::new(g, 2024, Layout::Plant, vec![Vec2::ZERO], "Hydrangea");
        assert!(h.info().is_none() && !h.in_year(2025));
        h.returns = true;
        assert!(h.in_year(2026));
        // Tulips go in in the fall and flower every spring.
        let t = Planting::new(g, 2025, Layout::Patch, rect_points(Vec2::ZERO, 1.0, 1.0), "Tulip");
        assert_eq!(t.method, Method::Bought);
        let s = t.stages(&settings(), None);
        assert_eq!((s[0].step, s[0].from), (Step::PlantOut, d(2025, 9, 26)));
        assert_eq!((s[1].step, s[1].from), (Step::Harvest, d(2026, 4, 1)));
    }

    #[test]
    fn companion_names_match_loosely() {
        assert!(names("Bean", "Pole bean"));
        assert!(names("Onion", "Green onion"));
        assert!(!names("Pea", "Sweet pepper"));
        assert!(!names("Pea", "Peanut"));
    }

    #[test]
    fn reminders_for_upcoming_steps() {
        let mut p = Project::new("T");
        p.garden = settings();
        let g = garden(&mut p);
        let t = Planting::new(g, 2026, Layout::Row, vec![Vec2::ZERO, Vec2::new(2.0, 0.0)], "Tomato");
        let id = t.id;
        p.plantings.push(t);
        // Mid-April: starting indoors has passed; transplant and harvest are ahead.
        assert_eq!(p.add_planting_reminders(id, d(2026, 4, 20)), 2);
        assert_eq!(p.add_planting_reminders(id, d(2026, 4, 20)), 0);
        assert!(p.reminders.iter().any(|r| r.title == "Harden off and transplant Tomato" && r.due == d(2026, 5, 17)));
    }

    #[test]
    fn old_seed() {
        let mut s = SeedPacket::new("Onion");
        s.packed_for = Some(2025);
        assert_eq!(s.good_until(), Some(2025));
        assert!(s.is_old(2026));
        let mut t = SeedPacket::new("Tomato");
        t.packed_for = Some(2023);
        assert!(!t.is_old(2026));
    }

    #[test]
    fn seeds_stay_with_the_owner_and_plantings_stay_with_the_house() {
        let mut p = Project::new("T");
        let g = garden(&mut p);
        let before = p.clone();
        let seed = SeedPacket::new("Tomato");
        let mut t = Planting::new(g, 2026, Layout::Plant, vec![Vec2::new(1.0, 1.0)], "Tomato");
        t.seed = Some(seed.id);
        t.custom_fields.push(CustomField { key: "Cost".into(), value: "$4".into(), private: true });
        p.seeds.push(seed);
        p.plantings.push(t);
        crate::diff::record_changes(&mut p, &before, "me", chrono::Utc::now());
        assert!(p.changelog.iter().any(|c| c.private), "seed packets are logged privately");
        let shared = p.sanitized_for_sharing();
        assert!(shared.seeds.is_empty());
        assert_eq!(shared.plantings.len(), 1);
        assert!(shared.plantings[0].seed.is_none() && shared.plantings[0].custom_fields.is_empty());
        assert!(shared.changelog.iter().all(|c| !c.private));
    }

    #[test]
    fn deleting_a_garden_removes_its_plantings() {
        let mut p = Project::new("T");
        let g = garden(&mut p);
        let t = Planting::new(g, 2026, Layout::Plant, vec![Vec2::new(1.0, 1.0)], "Tomato");
        let id = t.id;
        p.plantings.push(t);
        p.notes.push(Note::new(EntityRef::Planting(id)));
        p.translate_feature(g, Vec2::new(1.0, 0.0));
        assert_eq!(p.planting(id).unwrap().points[0], Vec2::new(2.0, 1.0));
        assert!(p.delete_entity(EntityRef::Feature(g)));
        assert!(p.plantings.is_empty() && p.notes.is_empty());
    }
}

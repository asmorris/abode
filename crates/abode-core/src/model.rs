//! The house model.
//!
//! Layout: a [`Project`] has [`Level`]s (floors). Each level is a small planar
//! graph: [`Node`]s (corner points) joined by [`Wall`]s, with [`Room`]s being
//! closed loops of nodes. Two rooms that share an edge share the *same* wall.
//! Doors/windows ([`Opening`]) sit on a wall; [`PlacedObject`]s sit in a room.
//!
//! Notes, service records, reminders and paint live at project level and point
//! at what they describe with an [`EntityRef`], so anything can have them.

use crate::geometry::Vec2;
use crate::ids::*;
pub use crate::roof::*;
pub use crate::site::*;
use crate::units::UnitSystem;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_WALL_HEIGHT: f64 = 2.44; // 8 ft
pub const DEFAULT_WALL_THICKNESS: f64 = 0.15;

fn default_wall_height() -> f64 {
    DEFAULT_WALL_HEIGHT
}
fn default_wall_thickness() -> f64 {
    DEFAULT_WALL_THICKNESS
}

// ---------------------------------------------------------------------------
// Project

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    #[serde(default)]
    pub info: HouseInfo,
    #[serde(default)]
    pub roof: Roof,
    #[serde(default)]
    pub levels: Vec<Level>,
    /// Outbuildings (shed, detached garage...). Their levels are in `levels`.
    #[serde(default)]
    pub buildings: Vec<Building>,
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub service_records: Vec<ServiceRecord>,
    #[serde(default)]
    pub reminders: Vec<Reminder>,
    #[serde(default)]
    pub paints: Vec<Paint>,
    #[serde(default)]
    pub changelog: Vec<ChangeEntry>,
    /// Frost dates and zone, for the planting calendar.
    #[serde(default)]
    pub garden: crate::garden::GardenSettings,
    #[serde(default)]
    pub plantings: Vec<crate::garden::Planting>,
    /// Seed packets on hand.
    #[serde(default)]
    pub seeds: Vec<crate::garden::SeedPacket>,
    /// The family's pets (private: they leave with the owner).
    #[serde(default)]
    pub pets: Vec<crate::pet::Pet>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HouseInfo {
    pub name: String,
    pub address: String,
    pub year_built: Option<i32>,
    pub units: UnitSystem,
    pub default_wall_thickness: f64,
    pub created: DateTime<Utc>,
    pub custom_fields: Vec<CustomField>,
    /// Sidebar sections this household doesn't need (no car, no yard...).
    /// Hiding one keeps its information.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<SidebarItem>,
}

/// A sidebar section that can be hidden.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarItem {
    Roof,
    Vehicles,
    Pets,
    Reminders,
    Timeline,
    Outside,
}

impl SidebarItem {
    pub const ALL: [SidebarItem; 6] =
        [SidebarItem::Roof, SidebarItem::Vehicles, SidebarItem::Pets, SidebarItem::Reminders, SidebarItem::Timeline, SidebarItem::Outside];

    pub fn label(self) -> &'static str {
        match self {
            SidebarItem::Roof => "Roof",
            SidebarItem::Vehicles => "Vehicles",
            SidebarItem::Pets => "Pets",
            SidebarItem::Reminders => "Reminders",
            SidebarItem::Timeline => "Timeline",
            SidebarItem::Outside => "Outside",
        }
    }

    /// Who might not need it.
    pub fn hint(self) -> &'static str {
        match self {
            SidebarItem::Roof => "Often the building's job in a condo",
            SidebarItem::Vehicles => "Cars, bikes, trailers and their upkeep",
            SidebarItem::Pets => "Vets, vaccines and medications",
            SidebarItem::Reminders => "Everything coming due, in one list",
            SidebarItem::Timeline => "Every change, and your own entries",
            SidebarItem::Outside => "The yard, decks, gardens and outbuildings",
        }
    }
}

impl HouseInfo {
    pub fn shows(&self, item: SidebarItem) -> bool {
        !self.hidden.contains(&item)
    }

    pub fn set_shown(&mut self, item: SidebarItem, shown: bool) {
        self.hidden.retain(|h| *h != item);
        if !shown {
            self.hidden.push(item);
        }
    }
}

impl Default for HouseInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            address: String::new(),
            year_built: None,
            units: UnitSystem::Metric,
            default_wall_thickness: DEFAULT_WALL_THICKNESS,
            created: Utc::now(),
            custom_fields: Vec::new(),
            hidden: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Roof {
    pub material: String,
    pub color: String,
    pub pitch: String,
    pub area_m2: Option<f64>,
    pub installed: Option<NaiveDate>,
    pub installed_by: String,
    pub warranty_expires: Option<NaiveDate>,
    pub warranty_details: String,
    pub last_inspected: Option<NaiveDate>,
    pub custom_fields: Vec<CustomField>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CustomField {
    pub key: String,
    pub value: String,
    /// Private fields are stripped when exporting a copy for a new owner.
    #[serde(default)]
    pub private: bool,
}

// ---------------------------------------------------------------------------
// Levels and geometry

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelKind {
    /// A floor of the house or of an outbuilding.
    #[default]
    Floor,
    /// The lot around the house ("Outside"). There's exactly one.
    Site,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Level {
    pub id: LevelId,
    pub name: String,
    #[serde(default)]
    pub kind: LevelKind,
    /// The outbuilding this floor belongs to (`None` = the house).
    #[serde(default)]
    pub building: Option<BuildingId>,
    /// Height of this floor above the ground floor, metres.
    #[serde(default)]
    pub elevation: f64,
    /// Default wall height for this floor, metres.
    #[serde(default = "default_wall_height")]
    pub wall_height: f64,
    /// A traced blueprint image shown under the plan.
    #[serde(default)]
    pub underlay: Option<Underlay>,
    #[serde(default)]
    pub nodes: BTreeMap<NodeId, Node>,
    #[serde(default)]
    pub walls: BTreeMap<WallId, Wall>,
    #[serde(default)]
    pub rooms: BTreeMap<RoomId, Room>,
    #[serde(default)]
    pub openings: BTreeMap<OpeningId, Opening>,
    #[serde(default)]
    pub objects: BTreeMap<ObjectId, PlacedObject>,
    /// Roof sections sitting on this level's walls.
    #[serde(default)]
    pub roofs: BTreeMap<RoofId, RoofSection>,
    /// Decks, fences, trees... (only on the site level).
    #[serde(default)]
    pub features: BTreeMap<FeatureId, Feature>,
}

impl Level {
    pub fn new(name: impl Into<String>, elevation: f64) -> Self {
        Self {
            id: LevelId::new(),
            name: name.into(),
            kind: LevelKind::Floor,
            building: None,
            elevation,
            wall_height: DEFAULT_WALL_HEIGHT,
            underlay: None,
            nodes: BTreeMap::new(),
            walls: BTreeMap::new(),
            rooms: BTreeMap::new(),
            openings: BTreeMap::new(),
            objects: BTreeMap::new(),
            roofs: BTreeMap::new(),
            features: BTreeMap::new(),
        }
    }

    pub fn new_site() -> Self {
        Self { kind: LevelKind::Site, ..Self::new("Outside", 0.0) }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Underlay {
    pub attachment: AttachmentId,
    pub pixel_size: [u32; 2],
    /// World position of the image's top-left pixel.
    pub origin: Vec2,
    pub meters_per_pixel: f64,
    /// Radians.
    #[serde(default)]
    pub rotation: f64,
    pub opacity: f32,
    pub visible: bool,
}

impl Underlay {
    pub fn new(attachment: AttachmentId, pixel_size: [u32; 2]) -> Self {
        // Until calibrated, assume the image is ~15 m across.
        let mpp = 15.0 / pixel_size[0].max(1) as f64;
        Self {
            attachment,
            pixel_size,
            origin: Vec2::ZERO,
            meters_per_pixel: mpp,
            rotation: 0.0,
            opacity: 0.5,
            visible: true,
        }
    }

    pub fn pixel_to_world(&self, px: Vec2) -> Vec2 {
        self.origin + (px * self.meters_per_pixel).rotated(self.rotation)
    }

    pub fn world_to_pixel(&self, w: Vec2) -> Vec2 {
        (w - self.origin).rotated(-self.rotation) / self.meters_per_pixel
    }

    /// Rescale so that world points `a` and `b` (as currently drawn) are
    /// `real_distance` metres apart, keeping `a` fixed.
    pub fn calibrate(&mut self, a: Vec2, b: Vec2, real_distance: f64) -> bool {
        let pa = self.world_to_pixel(a);
        let pb = self.world_to_pixel(b);
        let px = pa.distance(pb);
        if px < 1e-6 || real_distance <= 0.0 {
            return false;
        }
        self.meters_per_pixel = real_distance / px;
        self.origin = a - (pa * self.meters_per_pixel).rotated(self.rotation);
        true
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub pos: Vec2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wall {
    pub id: WallId,
    pub a: NodeId,
    pub b: NodeId,
    #[serde(default)]
    pub label: String,
    #[serde(default = "default_wall_thickness")]
    pub thickness: f64,
    /// Overrides the level's wall height.
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
}

impl Wall {
    pub fn new(a: NodeId, b: NodeId, thickness: f64) -> Self {
        Self {
            id: WallId::new(),
            a,
            b,
            label: String::new(),
            thickness,
            height: None,
            custom_fields: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Room {
    pub id: RoomId,
    pub name: String,
    #[serde(default)]
    pub kind: String,
    /// Corner nodes in order (positive signed area).
    pub nodes: Vec<NodeId>,
    #[serde(default)]
    pub ceiling_height: Option<f64>,
    #[serde(default)]
    pub floor_finish: String,
    /// Locked rooms can't be reshaped without explicitly unlocking first.
    #[serde(default)]
    pub shape_locked: bool,
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
    /// Towel bars, hooks, a robe hook: fittings that aren't items of their own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hardware: Vec<Hardware>,
}

impl Room {
    pub fn new(name: impl Into<String>, nodes: Vec<NodeId>) -> Self {
        Self {
            id: RoomId::new(),
            name: name.into(),
            kind: String::new(),
            nodes,
            ceiling_height: None,
            floor_finish: String::new(),
            shape_locked: false,
            custom_fields: Vec::new(),
            hardware: Vec::new(),
        }
    }

    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() {
            "Unnamed room".to_string()
        } else {
            self.name.clone()
        }
    }
}

pub const ROOM_KIND_SUGGESTIONS: &[&str] = &[
    "Bedroom",
    "Bathroom",
    "Kitchen",
    "Living room",
    "Dining room",
    "Family room",
    "Office",
    "Hallway",
    "Laundry",
    "Utility / mechanical",
    "Garage",
    "Closet",
    "Storage",
    "Basement",
    "Attic",
    "Porch / deck",
    "Sunroom",
    "Mudroom",
];

// ---------------------------------------------------------------------------
// Doors and windows

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoorStyle {
    Hinged,
    Double,
    Sliding,
    Pocket,
    Bifold,
    Garage,
    Other,
}

impl DoorStyle {
    pub const ALL: [DoorStyle; 7] = [
        DoorStyle::Hinged,
        DoorStyle::Double,
        DoorStyle::Sliding,
        DoorStyle::Pocket,
        DoorStyle::Bifold,
        DoorStyle::Garage,
        DoorStyle::Other,
    ];
    pub fn label(self) -> &'static str {
        match self {
            DoorStyle::Hinged => "Hinged",
            DoorStyle::Double => "Double",
            DoorStyle::Sliding => "Sliding",
            DoorStyle::Pocket => "Pocket",
            DoorStyle::Bifold => "Bifold",
            DoorStyle::Garage => "Garage",
            DoorStyle::Other => "Other",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowStyle {
    Fixed,
    Casement,
    SingleHung,
    DoubleHung,
    Slider,
    Awning,
    Bay,
    Other,
}

impl WindowStyle {
    pub const ALL: [WindowStyle; 8] = [
        WindowStyle::Fixed,
        WindowStyle::Casement,
        WindowStyle::SingleHung,
        WindowStyle::DoubleHung,
        WindowStyle::Slider,
        WindowStyle::Awning,
        WindowStyle::Bay,
        WindowStyle::Other,
    ];
    pub fn label(self) -> &'static str {
        match self {
            WindowStyle::Fixed => "Fixed",
            WindowStyle::Casement => "Casement",
            WindowStyle::SingleHung => "Single-hung",
            WindowStyle::DoubleHung => "Double-hung",
            WindowStyle::Slider => "Slider",
            WindowStyle::Awning => "Awning",
            WindowStyle::Bay => "Bay",
            WindowStyle::Other => "Other",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpeningKind {
    Door(DoorStyle),
    Window(WindowStyle),
}

impl OpeningKind {
    pub fn is_door(self) -> bool {
        matches!(self, OpeningKind::Door(_))
    }
    pub fn noun(self) -> &'static str {
        match self {
            OpeningKind::Door(_) => "door",
            OpeningKind::Window(_) => "window",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            OpeningKind::Door(_) => "Door",
            OpeningKind::Window(_) => "Window",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Opening {
    pub id: OpeningId,
    pub wall: WallId,
    pub kind: OpeningKind,
    /// A typed-in style when the style is Other.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style_name: String,
    #[serde(default)]
    pub label: String,
    /// Distance from the wall's `a` node to the opening's centre, metres.
    pub offset: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub sill_height: f64,
    #[serde(default)]
    pub flip_hinge: bool,
    #[serde(default)]
    pub flip_swing: bool,
    #[serde(default)]
    pub details: ItemDetails,
}

/// `label`, or `name` when the value is "Other" and a name was typed in.
pub fn custom_label(is_other: bool, name: &str, label: &str) -> String {
    if is_other && !name.trim().is_empty() {
        name.trim().to_string()
    } else {
        label.to_string()
    }
}

impl Opening {
    /// The style's name, or the typed-in one for Other.
    pub fn style_label(&self) -> String {
        let (other, label) = match self.kind {
            OpeningKind::Door(s) => (s == DoorStyle::Other, s.label()),
            OpeningKind::Window(s) => (s == WindowStyle::Other, s.label()),
        };
        custom_label(other, &self.style_name, label)
    }

    pub fn new(wall: WallId, kind: OpeningKind, offset: f64) -> Self {
        let (width, height, sill) = match kind {
            OpeningKind::Door(DoorStyle::Garage) => (2.74, 2.13, 0.0),
            OpeningKind::Door(DoorStyle::Double | DoorStyle::Sliding) => (1.52, 2.03, 0.0),
            OpeningKind::Door(_) => (0.81, 2.03, 0.0),
            OpeningKind::Window(_) => (0.91, 1.22, 0.91),
        };
        Self {
            id: OpeningId::new(),
            wall,
            kind,
            style_name: String::new(),
            label: String::new(),
            offset,
            width,
            height,
            sill_height: sill,
            flip_hinge: false,
            flip_swing: false,
            details: ItemDetails::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Objects in rooms (appliances, fixtures, furniture...)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectCategory {
    Appliance,
    Hvac,
    Plumbing,
    Electrical,
    Fixture,
    Furniture,
    Storage,
    Outdoor,
    Roof,
    /// A car, truck, motorcycle, trailer... (has a [`crate::vehicle::Vehicle`] record).
    Vehicle,
    Other,
}

impl ObjectCategory {
    pub const ALL: [ObjectCategory; 11] = [
        ObjectCategory::Appliance,
        ObjectCategory::Hvac,
        ObjectCategory::Plumbing,
        ObjectCategory::Electrical,
        ObjectCategory::Fixture,
        ObjectCategory::Furniture,
        ObjectCategory::Storage,
        ObjectCategory::Outdoor,
        ObjectCategory::Roof,
        ObjectCategory::Vehicle,
        ObjectCategory::Other,
    ];
    pub fn label(self) -> &'static str {
        match self {
            ObjectCategory::Appliance => "Appliance",
            ObjectCategory::Hvac => "Heating / cooling",
            ObjectCategory::Plumbing => "Plumbing",
            ObjectCategory::Electrical => "Electrical",
            ObjectCategory::Fixture => "Fixture",
            ObjectCategory::Furniture => "Furniture",
            ObjectCategory::Storage => "Storage",
            ObjectCategory::Outdoor => "Outdoor",
            ObjectCategory::Roof => "Roof & gutters",
            ObjectCategory::Vehicle => "Vehicle",
            ObjectCategory::Other => "Other",
        }
    }
    /// Default display colour (sRGB).
    pub fn color(self) -> [u8; 3] {
        match self {
            ObjectCategory::Appliance => [96, 125, 139],
            ObjectCategory::Hvac => [230, 126, 34],
            ObjectCategory::Plumbing => [52, 152, 219],
            ObjectCategory::Electrical => [241, 196, 15],
            ObjectCategory::Fixture => [149, 165, 166],
            ObjectCategory::Furniture => [141, 110, 99],
            ObjectCategory::Storage => [121, 134, 203],
            ObjectCategory::Outdoor => [96, 160, 104],
            ObjectCategory::Roof => [168, 104, 84],
            ObjectCategory::Vehicle => [84, 110, 148],
            ObjectCategory::Other => [120, 144, 156],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    Rectangle,
    Circle,
    Ellipse,
    Triangle,
}

impl ShapeKind {
    pub const ALL: [ShapeKind; 4] = [
        ShapeKind::Rectangle,
        ShapeKind::Circle,
        ShapeKind::Ellipse,
        ShapeKind::Triangle,
    ];
    pub fn label(self) -> &'static str {
        match self {
            ShapeKind::Rectangle => "Rectangle",
            ShapeKind::Circle => "Circle",
            ShapeKind::Ellipse => "Ellipse",
            ShapeKind::Triangle => "Triangle",
        }
    }
}

/// A primitive footprint, centred on the object's position.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub kind: ShapeKind,
    /// Width (x) and depth (y) of the bounding box, metres. Circles use `width`.
    pub width: f64,
    pub depth: f64,
}

impl Shape {
    pub fn new(kind: ShapeKind, width: f64, depth: f64) -> Self {
        let depth = if kind == ShapeKind::Circle { width } else { depth };
        Self { kind, width, depth }
    }

    /// Outline in local coordinates (centred on the origin, unrotated).
    pub fn outline(&self) -> Vec<Vec2> {
        let (hw, hd) = (self.width / 2.0, self.depth / 2.0);
        match self.kind {
            ShapeKind::Rectangle => vec![
                Vec2::new(-hw, -hd),
                Vec2::new(hw, -hd),
                Vec2::new(hw, hd),
                Vec2::new(-hw, hd),
            ],
            ShapeKind::Triangle => vec![Vec2::new(0.0, -hd), Vec2::new(hw, hd), Vec2::new(-hw, hd)],
            ShapeKind::Circle | ShapeKind::Ellipse => {
                let hd = if self.kind == ShapeKind::Circle { hw } else { hd };
                (0..40)
                    .map(|i| {
                        let t = i as f64 / 40.0 * std::f64::consts::TAU;
                        Vec2::new(hw * t.cos(), hd * t.sin())
                    })
                    .collect()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacedObject {
    pub id: ObjectId,
    /// The room this object belongs to (none for things outside or on a roof).
    #[serde(default)]
    pub room: Option<RoomId>,
    /// Outside: the deck/patio it sits on, if any.
    #[serde(default)]
    pub feature: Option<FeatureId>,
    /// On a roof (skylight, chimney, solar panel, gutters...).
    #[serde(default)]
    pub roof: Option<RoofId>,
    pub label: String,
    pub category: ObjectCategory,
    /// A typed-in category when `category` is Other.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub category_name: String,
    pub shape: Shape,
    /// Centre, in level (world) coordinates.
    pub position: Vec2,
    /// Radians.
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub details: ItemDetails,
    /// Cars and other vehicles: registration, odometer, tires, parts...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle: Option<Box<crate::vehicle::Vehicle>>,
    /// Stairs to the floor above or below.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stairs: Option<crate::stairs::Stairs>,
}

impl PlacedObject {
    /// The category's name, or the typed-in one for Other.
    pub fn category_label(&self) -> String {
        custom_label(self.category == ObjectCategory::Other, &self.category_name, self.category.label())
    }

    pub fn world_outline(&self) -> Vec<Vec2> {
        self.shape
            .outline()
            .into_iter()
            .map(|p| self.position + p.rotated(self.rotation))
            .collect()
    }

    pub fn display_name(&self) -> String {
        if self.label.trim().is_empty() {
            // A vehicle with no nickname goes by "2021 Toyota RAV4".
            if let Some(v) = &self.vehicle {
                let t = v.title(&self.details);
                if !t.is_empty() {
                    return t;
                }
                if !v.body.trim().is_empty() {
                    return v.body.trim().to_string();
                }
            }
            self.category_label()
        } else {
            self.label.clone()
        }
    }

    pub fn display_color(&self) -> [u8; 3] {
        self.color.unwrap_or_else(|| self.category.color())
    }
}

/// A quick-pick list of common things to place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Shown everywhere (the plain shapes).
    Any,
    Indoor,
    Outdoor,
    Roof,
    /// Vehicles: outside, and on floors with somewhere to park (a garage).
    Parking,
    /// Outside, in any outbuilding (shed, garage), or on a floor with a
    /// garage: an EV charger, say.
    Garage,
}

/// What kind of floor a plan shows, beyond indoor/outdoor/roof.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Spot {
    /// It has somewhere to park: a garage building, or a garage room.
    pub parking: bool,
    /// It's a floor of an outbuilding (shed, garage...).
    pub outbuilding: bool,
}

pub struct ObjectPreset {
    pub place: Place,
    pub label: &'static str,
    pub category: ObjectCategory,
    pub shape: ShapeKind,
    pub width: f64,
    pub depth: f64,
}

impl ObjectPreset {
    /// Whether to offer it on a plan of `place`; `spot` says whether that
    /// floor has a garage or is an outbuilding, for vehicles and chargers.
    pub fn fits(&self, place: Place, spot: Spot) -> bool {
        let indoor = place == Place::Indoor;
        match self.place {
            Place::Any => true,
            Place::Parking => place == Place::Outdoor || (indoor && spot.parking),
            Place::Garage => place == Place::Outdoor || (indoor && (spot.parking || spot.outbuilding)),
            p => p == place,
        }
    }
}

pub const OBJECT_PRESETS: &[ObjectPreset] = &[
    ObjectPreset { place: Place::Any, label: "Rectangle", category: ObjectCategory::Other, shape: ShapeKind::Rectangle, width: 0.6, depth: 0.6 },
    ObjectPreset { place: Place::Any, label: "Circle", category: ObjectCategory::Other, shape: ShapeKind::Circle, width: 0.6, depth: 0.6 },
    ObjectPreset { place: Place::Any, label: "Ellipse", category: ObjectCategory::Other, shape: ShapeKind::Ellipse, width: 0.8, depth: 0.5 },
    ObjectPreset { place: Place::Any, label: "Triangle", category: ObjectCategory::Other, shape: ShapeKind::Triangle, width: 0.6, depth: 0.6 },
    ObjectPreset { place: Place::Any, label: "Stairs", category: ObjectCategory::Fixture, shape: ShapeKind::Rectangle, width: 0.9, depth: 3.0 },
    ObjectPreset { place: Place::Indoor, label: "Fridge", category: ObjectCategory::Appliance, shape: ShapeKind::Rectangle, width: 0.9, depth: 0.75 },
    ObjectPreset { place: Place::Indoor, label: "Stove / range", category: ObjectCategory::Appliance, shape: ShapeKind::Rectangle, width: 0.76, depth: 0.66 },
    ObjectPreset { place: Place::Indoor, label: "Dishwasher", category: ObjectCategory::Appliance, shape: ShapeKind::Rectangle, width: 0.6, depth: 0.6 },
    ObjectPreset { place: Place::Indoor, label: "Microwave", category: ObjectCategory::Appliance, shape: ShapeKind::Rectangle, width: 0.6, depth: 0.4 },
    ObjectPreset { place: Place::Indoor, label: "Washer", category: ObjectCategory::Appliance, shape: ShapeKind::Rectangle, width: 0.69, depth: 0.71 },
    ObjectPreset { place: Place::Indoor, label: "Dryer", category: ObjectCategory::Appliance, shape: ShapeKind::Rectangle, width: 0.69, depth: 0.71 },
    ObjectPreset { place: Place::Indoor, label: "Furnace", category: ObjectCategory::Hvac, shape: ShapeKind::Rectangle, width: 0.56, depth: 0.74 },
    ObjectPreset { place: Place::Indoor, label: "Air conditioner", category: ObjectCategory::Hvac, shape: ShapeKind::Rectangle, width: 0.8, depth: 0.8 },
    ObjectPreset { place: Place::Indoor, label: "Heat pump", category: ObjectCategory::Hvac, shape: ShapeKind::Rectangle, width: 0.9, depth: 0.4 },
    ObjectPreset { place: Place::Indoor, label: "Water heater", category: ObjectCategory::Plumbing, shape: ShapeKind::Circle, width: 0.56, depth: 0.56 },
    ObjectPreset { place: Place::Indoor, label: "Water softener", category: ObjectCategory::Plumbing, shape: ShapeKind::Circle, width: 0.35, depth: 0.35 },
    ObjectPreset { place: Place::Indoor, label: "Sump pump", category: ObjectCategory::Plumbing, shape: ShapeKind::Circle, width: 0.45, depth: 0.45 },
    ObjectPreset { place: Place::Indoor, label: "Toilet", category: ObjectCategory::Plumbing, shape: ShapeKind::Ellipse, width: 0.4, depth: 0.7 },
    ObjectPreset { place: Place::Indoor, label: "Bathtub", category: ObjectCategory::Plumbing, shape: ShapeKind::Rectangle, width: 1.52, depth: 0.76 },
    ObjectPreset { place: Place::Indoor, label: "Shower", category: ObjectCategory::Plumbing, shape: ShapeKind::Rectangle, width: 0.9, depth: 0.9 },
    ObjectPreset { place: Place::Indoor, label: "Sink", category: ObjectCategory::Plumbing, shape: ShapeKind::Rectangle, width: 0.6, depth: 0.5 },
    ObjectPreset { place: Place::Indoor, label: "Electrical panel", category: ObjectCategory::Electrical, shape: ShapeKind::Rectangle, width: 0.36, depth: 0.1 },
    ObjectPreset { place: Place::Indoor, label: "Smoke / CO detector", category: ObjectCategory::Electrical, shape: ShapeKind::Circle, width: 0.15, depth: 0.15 },
    ObjectPreset { place: Place::Indoor, label: "Fireplace", category: ObjectCategory::Hvac, shape: ShapeKind::Rectangle, width: 1.2, depth: 0.5 },
    ObjectPreset { place: Place::Outdoor, label: "BBQ / grill", category: ObjectCategory::Outdoor, shape: ShapeKind::Rectangle, width: 1.3, depth: 0.6 },
    ObjectPreset { place: Place::Outdoor, label: "Hot tub", category: ObjectCategory::Outdoor, shape: ShapeKind::Rectangle, width: 2.1, depth: 2.1 },
    ObjectPreset { place: Place::Outdoor, label: "Air conditioner", category: ObjectCategory::Hvac, shape: ShapeKind::Rectangle, width: 0.8, depth: 0.8 },
    ObjectPreset { place: Place::Outdoor, label: "Heat pump", category: ObjectCategory::Hvac, shape: ShapeKind::Rectangle, width: 0.9, depth: 0.4 },
    ObjectPreset { place: Place::Outdoor, label: "Hose tap", category: ObjectCategory::Plumbing, shape: ShapeKind::Circle, width: 0.2, depth: 0.2 },
    ObjectPreset { place: Place::Outdoor, label: "Outdoor light", category: ObjectCategory::Electrical, shape: ShapeKind::Circle, width: 0.2, depth: 0.2 },
    ObjectPreset { place: Place::Outdoor, label: "Outdoor outlet", category: ObjectCategory::Electrical, shape: ShapeKind::Rectangle, width: 0.15, depth: 0.1 },
    ObjectPreset { place: Place::Outdoor, label: "Patio table", category: ObjectCategory::Furniture, shape: ShapeKind::Circle, width: 1.2, depth: 1.2 },
    ObjectPreset { place: Place::Outdoor, label: "Lawn mower", category: ObjectCategory::Outdoor, shape: ShapeKind::Rectangle, width: 0.55, depth: 1.0 },
    ObjectPreset { place: Place::Outdoor, label: "Irrigation controller", category: ObjectCategory::Outdoor, shape: ShapeKind::Rectangle, width: 0.3, depth: 0.15 },
    ObjectPreset { place: Place::Outdoor, label: "Septic tank", category: ObjectCategory::Plumbing, shape: ShapeKind::Rectangle, width: 2.4, depth: 1.5 },
    ObjectPreset { place: Place::Outdoor, label: "Well", category: ObjectCategory::Plumbing, shape: ShapeKind::Circle, width: 0.6, depth: 0.6 },
    ObjectPreset { place: Place::Roof, label: "Skylight", category: ObjectCategory::Roof, shape: ShapeKind::Rectangle, width: 0.6, depth: 1.2 },
    ObjectPreset { place: Place::Roof, label: "Chimney", category: ObjectCategory::Roof, shape: ShapeKind::Rectangle, width: 0.6, depth: 0.6 },
    ObjectPreset { place: Place::Roof, label: "Roof vent", category: ObjectCategory::Roof, shape: ShapeKind::Circle, width: 0.3, depth: 0.3 },
    ObjectPreset { place: Place::Roof, label: "Solar panels", category: ObjectCategory::Electrical, shape: ShapeKind::Rectangle, width: 3.4, depth: 2.0 },
    ObjectPreset { place: Place::Roof, label: "Gutter", category: ObjectCategory::Roof, shape: ShapeKind::Rectangle, width: 6.0, depth: 0.15 },
    ObjectPreset { place: Place::Roof, label: "Downspout", category: ObjectCategory::Roof, shape: ShapeKind::Rectangle, width: 0.1, depth: 0.1 },
    ObjectPreset { place: Place::Roof, label: "Satellite dish", category: ObjectCategory::Electrical, shape: ShapeKind::Circle, width: 0.6, depth: 0.6 },
    ObjectPreset { place: Place::Garage, label: "EV charger", category: ObjectCategory::Electrical, shape: ShapeKind::Rectangle, width: 0.35, depth: 0.2 },
    ObjectPreset { place: Place::Parking, label: "Car", category: ObjectCategory::Vehicle, shape: ShapeKind::Rectangle, width: 1.85, depth: 4.6 },
    ObjectPreset { place: Place::Parking, label: "SUV / crossover", category: ObjectCategory::Vehicle, shape: ShapeKind::Rectangle, width: 1.9, depth: 4.8 },
    ObjectPreset { place: Place::Parking, label: "Pickup truck", category: ObjectCategory::Vehicle, shape: ShapeKind::Rectangle, width: 2.05, depth: 5.9 },
    ObjectPreset { place: Place::Parking, label: "Minivan", category: ObjectCategory::Vehicle, shape: ShapeKind::Rectangle, width: 2.0, depth: 5.2 },
    ObjectPreset { place: Place::Parking, label: "Motorcycle", category: ObjectCategory::Vehicle, shape: ShapeKind::Rectangle, width: 0.85, depth: 2.2 },
    ObjectPreset { place: Place::Parking, label: "Trailer", category: ObjectCategory::Vehicle, shape: ShapeKind::Rectangle, width: 2.0, depth: 4.0 },
    ObjectPreset { place: Place::Parking, label: "Boat (on trailer)", category: ObjectCategory::Vehicle, shape: ShapeKind::Rectangle, width: 2.4, depth: 6.5 },
];

/// Custom fields a new item of this kind starts with (blank), as prompts for
/// what's worth writing down.
pub fn starter_fields(label: &str) -> &'static [&'static str] {
    match label {
        "EV charger" => &["Level", "Power", "Connector", "Breaker", "App / Wi-Fi"],
        _ => &[],
    }
}

// ---------------------------------------------------------------------------
// Details shared by objects, doors and windows

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemDetails {
    pub make: String,
    pub model: String,
    pub serial: String,
    pub purchased: Option<NaiveDate>,
    pub installed: Option<NaiveDate>,
    pub price: String,
    pub vendor: String,
    pub warranty_expires: Option<NaiveDate>,
    pub warranty_details: String,
    pub custom_fields: Vec<CustomField>,
    /// Pulls, hinges, handles, faucets: what's fitted to it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hardware: Vec<Hardware>,
}

/// A fitting on something: cabinet pulls on a vanity, the handle and hinges
/// on a door, a towel bar in a bathroom. Enough to buy a matching one.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hardware {
    /// What it is: "Cabinet pull", "Hinge"...
    pub kind: String,
    pub quantity: Option<u32>,
    pub brand: String,
    /// Product name or collection.
    pub model: String,
    pub part_number: String,
    pub finish: String,
    /// e.g. 128 mm centre to centre, 24" bar.
    pub size: String,
    pub vendor: String,
    pub price: String,
    pub notes: String,
}

impl Hardware {
    pub fn display_name(&self) -> String {
        let kind = if self.kind.trim().is_empty() { "Hardware" } else { self.kind.trim() };
        let name = match self.quantity {
            Some(n) if n > 1 => format!("{n} × {kind}"),
            _ => kind.to_string(),
        };
        if self.brand.trim().is_empty() { name } else { format!("{name} · {}", self.brand.trim()) }
    }
}

/// Kinds of hardware, as suggestions (anything can be typed).
pub const HARDWARE_KINDS: &[&str] = &[
    "Cabinet pull",
    "Cabinet knob",
    "Cabinet hinge",
    "Drawer slide",
    "Door handle",
    "Door lever",
    "Door knob",
    "Deadbolt",
    "Door hinge",
    "Door stop",
    "Door closer",
    "Window lock",
    "Window crank",
    "Towel bar",
    "Towel ring",
    "Towel hook",
    "Robe hook",
    "Toilet paper holder",
    "Grab bar",
    "Shower head",
    "Faucet",
    "Drain",
    "Shelf bracket",
    "Shelf pin",
    "Curtain rod",
    "Coat hook",
    "Gate latch",
    "Gate hinge",
    "Switch plate",
    "Outlet cover",
];

pub const HARDWARE_FINISHES: &[&str] = &[
    "Matte black",
    "Brushed nickel",
    "Satin nickel",
    "Polished chrome",
    "Stainless steel",
    "Brushed brass",
    "Satin brass",
    "Polished brass",
    "Brushed gold",
    "Oil-rubbed bronze",
    "Antique copper",
    "White",
];

// ---------------------------------------------------------------------------
// References to entities

/// Points at something in the house that notes, history etc. can attach to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum EntityRef {
    /// The house as a whole (general notes live here).
    General,
    Roof,
    Level(LevelId),
    Room(RoomId),
    Wall(WallId),
    Opening(OpeningId),
    Object(ObjectId),
    /// One roof section (the house-wide roof record is [`EntityRef::Roof`]).
    RoofSection(RoofId),
    /// Something outside: deck, fence, tree...
    Feature(FeatureId),
    /// An outbuilding.
    Building(BuildingId),
    /// A row, patch or plant in a garden bed.
    Planting(PlantingId),
    /// A pet.
    Pet(PetId),
}

impl EntityRef {
    /// Stable string key, e.g. `room:0190...`.
    pub fn key(&self) -> String {
        match self {
            EntityRef::General => "general".into(),
            EntityRef::Roof => "roof".into(),
            EntityRef::Level(id) => format!("level:{id}"),
            EntityRef::Room(id) => format!("room:{id}"),
            EntityRef::Wall(id) => format!("wall:{id}"),
            EntityRef::Opening(id) => format!("opening:{id}"),
            EntityRef::Object(id) => format!("object:{id}"),
            EntityRef::RoofSection(id) => format!("roof:{id}"),
            EntityRef::Feature(id) => format!("feature:{id}"),
            EntityRef::Building(id) => format!("building:{id}"),
            EntityRef::Planting(id) => format!("planting:{id}"),
            EntityRef::Pet(id) => format!("pet:{id}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Notes, attachments, service history, reminders

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: NoteId,
    pub target: EntityRef,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    /// Private notes are stripped when exporting a copy for a new owner.
    #[serde(default)]
    pub private: bool,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    #[serde(default)]
    pub attachments: Vec<AttachmentId>,
}

impl Note {
    pub fn new(target: EntityRef) -> Self {
        let now = Utc::now();
        Self {
            id: NoteId::new(),
            target,
            title: String::new(),
            body: String::new(),
            private: false,
            created: now,
            updated: now,
            attachments: Vec::new(),
        }
    }
}

/// Metadata about an embedded file. The bytes live in the file's
/// `attachments` table and are loaded on demand.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttachmentMeta {
    pub id: AttachmentId,
    pub filename: String,
    pub mime: String,
    pub size: u64,
    pub added: DateTime<Utc>,
}

impl AttachmentMeta {
    pub fn is_image(&self) -> bool {
        self.mime.starts_with("image/")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceKind {
    Maintenance,
    Repair,
    Inspection,
    Installation,
    Replacement,
    Cleaning,
    Other,
}

impl ServiceKind {
    pub const ALL: [ServiceKind; 7] = [
        ServiceKind::Maintenance,
        ServiceKind::Repair,
        ServiceKind::Inspection,
        ServiceKind::Installation,
        ServiceKind::Replacement,
        ServiceKind::Cleaning,
        ServiceKind::Other,
    ];
    pub fn label(self) -> &'static str {
        match self {
            ServiceKind::Maintenance => "Maintenance",
            ServiceKind::Repair => "Repair",
            ServiceKind::Inspection => "Inspection",
            ServiceKind::Installation => "Installation",
            ServiceKind::Replacement => "Replacement",
            ServiceKind::Cleaning => "Cleaning",
            ServiceKind::Other => "Other",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceRecord {
    pub id: ServiceId,
    pub target: EntityRef,
    pub date: NaiveDate,
    pub kind: ServiceKind,
    /// A typed-in kind of work when `kind` is Other.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind_name: String,
    #[serde(default)]
    pub performed_by: String,
    #[serde(default)]
    pub cost: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub attachments: Vec<AttachmentId>,
    /// Vehicles: the odometer reading (km) when it was done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub odometer: Option<f64>,
}

impl ServiceRecord {
    /// The kind of work, or the typed-in one for Other.
    pub fn kind_label(&self) -> String {
        custom_label(self.kind == ServiceKind::Other, &self.kind_name, self.kind.label())
    }

    pub fn new(target: EntityRef, date: NaiveDate) -> Self {
        Self {
            id: ServiceId::new(),
            target,
            date,
            kind: ServiceKind::Maintenance,
            kind_name: String::new(),
            performed_by: String::new(),
            cost: String::new(),
            description: String::new(),
            private: false,
            attachments: Vec::new(),
            odometer: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecurrenceUnit {
    Days,
    Weeks,
    Months,
    Years,
}

impl RecurrenceUnit {
    pub const ALL: [RecurrenceUnit; 4] = [
        RecurrenceUnit::Days,
        RecurrenceUnit::Weeks,
        RecurrenceUnit::Months,
        RecurrenceUnit::Years,
    ];
    pub fn label(self) -> &'static str {
        match self {
            RecurrenceUnit::Days => "days",
            RecurrenceUnit::Weeks => "weeks",
            RecurrenceUnit::Months => "months",
            RecurrenceUnit::Years => "years",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recurrence {
    pub every: u32,
    pub unit: RecurrenceUnit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reminder {
    pub id: ReminderId,
    pub target: EntityRef,
    pub title: String,
    #[serde(default)]
    pub notes: String,
    pub due: NaiveDate,
    #[serde(default)]
    pub repeat: Option<Recurrence>,
    /// For one-off reminders: when it was done.
    #[serde(default)]
    pub completed: Option<NaiveDate>,
    #[serde(default)]
    pub last_done: Option<NaiveDate>,
    /// Put off until this date: it doesn't count as due (or overdue) before then.
    #[serde(default)]
    pub snoozed_until: Option<NaiveDate>,
    /// Vehicles: whether it's due by date, by odometer, or whichever first.
    #[serde(default)]
    pub due_by: DueBy,
    /// Due at this odometer reading (km).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_km: Option<f64>,
    /// And again every this many km after it's done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every_km: Option<f64>,
    /// Appointments: the time on the due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<chrono::NaiveTime>,
    /// Appointments: where (the vet, the groomer).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub place: String,
}

/// What makes a reminder due. Only vehicles (with an odometer) use distance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DueBy {
    #[default]
    Date,
    Distance,
    /// Whichever comes first ("every 8,000 km or 6 months").
    Either,
}

impl DueBy {
    pub const ALL: [DueBy; 3] = [DueBy::Date, DueBy::Distance, DueBy::Either];
    pub fn label(self) -> &'static str {
        match self {
            DueBy::Date => "By date",
            DueBy::Distance => "By odometer",
            DueBy::Either => "Whichever first",
        }
    }
    pub fn uses_date(self) -> bool {
        self != DueBy::Distance
    }
    pub fn uses_distance(self) -> bool {
        self != DueBy::Date
    }
}

impl Reminder {
    pub fn new(target: EntityRef, title: impl Into<String>, due: NaiveDate) -> Self {
        Self {
            id: ReminderId::new(),
            target,
            title: title.into(),
            notes: String::new(),
            due,
            repeat: None,
            completed: None,
            last_done: None,
            snoozed_until: None,
            due_by: DueBy::Date,
            due_km: None,
            every_km: None,
            at: None,
            place: String::new(),
        }
    }

    /// An appointment: a reminder with a time.
    pub fn is_appointment(&self) -> bool {
        self.at.is_some()
    }
}

// ---------------------------------------------------------------------------
// Paint

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaintTarget {
    /// The whole room (all walls unless a wall has its own paint).
    Room(RoomId),
    /// One face of a wall: the side facing `room`, or the exterior if `None`.
    WallFace { wall: WallId, room: Option<RoomId> },
    /// Stain or paint on a deck, fence...
    Feature(FeatureId),
    /// The outside of an outbuilding.
    Building(BuildingId),
}

impl PaintTarget {
    /// The thing this paint is on (for notes, timeline links...).
    pub fn entity(self) -> EntityRef {
        match self {
            PaintTarget::Room(r) => EntityRef::Room(r),
            PaintTarget::WallFace { wall, .. } => EntityRef::Wall(wall),
            PaintTarget::Feature(f) => EntityRef::Feature(f),
            PaintTarget::Building(b) => EntityRef::Building(b),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaintSurface {
    Walls,
    Ceiling,
    Trim,
    Doors,
    Cabinets,
    Siding,
    Decking,
    Railings,
    Other,
}

impl PaintSurface {
    pub const ALL: [PaintSurface; 9] = [
        PaintSurface::Walls,
        PaintSurface::Ceiling,
        PaintSurface::Trim,
        PaintSurface::Doors,
        PaintSurface::Cabinets,
        PaintSurface::Siding,
        PaintSurface::Decking,
        PaintSurface::Railings,
        PaintSurface::Other,
    ];
    pub fn label(self) -> &'static str {
        match self {
            PaintSurface::Walls => "Walls",
            PaintSurface::Ceiling => "Ceiling",
            PaintSurface::Trim => "Trim",
            PaintSurface::Doors => "Doors",
            PaintSurface::Cabinets => "Cabinets",
            PaintSurface::Siding => "Siding",
            PaintSurface::Decking => "Decking",
            PaintSurface::Railings => "Railings",
            PaintSurface::Other => "Other",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaintFinish {
    Flat,
    Matte,
    Eggshell,
    Satin,
    SemiGloss,
    Gloss,
    Stain,
    Sealant,
    Other,
}

impl PaintFinish {
    pub const ALL: [PaintFinish; 9] = [
        PaintFinish::Flat,
        PaintFinish::Matte,
        PaintFinish::Eggshell,
        PaintFinish::Satin,
        PaintFinish::SemiGloss,
        PaintFinish::Gloss,
        PaintFinish::Stain,
        PaintFinish::Sealant,
        PaintFinish::Other,
    ];
    pub fn label(self) -> &'static str {
        match self {
            PaintFinish::Flat => "Flat",
            PaintFinish::Matte => "Matte",
            PaintFinish::Eggshell => "Eggshell",
            PaintFinish::Satin => "Satin",
            PaintFinish::SemiGloss => "Semi-gloss",
            PaintFinish::Gloss => "Gloss",
            PaintFinish::Stain => "Stain",
            PaintFinish::Sealant => "Sealant",
            PaintFinish::Other => "Other",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Paint {
    pub id: PaintId,
    pub target: PaintTarget,
    pub surface: PaintSurface,
    /// A typed-in surface when `surface` is Other.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub surface_name: String,
    #[serde(default)]
    pub brand: String,
    #[serde(default)]
    pub color_name: String,
    #[serde(default)]
    pub color_code: String,
    pub finish: PaintFinish,
    /// A typed-in finish when `finish` is Other.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub finish_name: String,
    /// Approximate colour for display.
    #[serde(default)]
    pub swatch: Option<[u8; 3]>,
    #[serde(default)]
    pub applied: Option<NaiveDate>,
    /// Where the leftover cans are stored.
    #[serde(default)]
    pub leftover_location: String,
    #[serde(default)]
    pub notes: String,
}

impl Paint {
    /// Paint for one surface, with the finish it usually gets (flat
    /// ceilings, semi-gloss trim and doors).
    pub fn for_surface(target: PaintTarget, surface: PaintSurface) -> Self {
        let finish = match surface {
            PaintSurface::Ceiling => PaintFinish::Flat,
            PaintSurface::Trim | PaintSurface::Doors | PaintSurface::Cabinets => PaintFinish::SemiGloss,
            PaintSurface::Decking | PaintSurface::Railings => PaintFinish::Stain,
            _ => PaintFinish::Eggshell,
        };
        Self { surface, finish, ..Self::new(target) }
    }

    pub fn new(target: PaintTarget) -> Self {
        Self {
            id: PaintId::new(),
            target,
            surface: PaintSurface::Walls,
            surface_name: String::new(),
            brand: String::new(),
            color_name: String::new(),
            color_code: String::new(),
            finish: PaintFinish::Eggshell,
            finish_name: String::new(),
            swatch: None,
            applied: None,
            leftover_location: String::new(),
            notes: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Change log (the human-readable timeline)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
    /// Written by a person, e.g. "Replaced the water heater".
    Manual,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChangeEntry {
    pub id: ChangeId,
    /// When it was recorded.
    pub timestamp: DateTime<Utc>,
    /// For manual entries: when it actually happened (may be in the past).
    #[serde(default)]
    pub occurred: Option<NaiveDate>,
    #[serde(default)]
    pub author: String,
    pub kind: ChangeKind,
    pub summary: String,
    #[serde(default)]
    pub details: String,
    /// Entities this entry is about (for per-room/per-object timelines).
    #[serde(default)]
    pub targets: Vec<EntityRef>,
    /// For automatic entries: key of the record that changed.
    #[serde(default)]
    pub record: Option<String>,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub fields: Vec<String>,
    /// Private entries are stripped on export.
    #[serde(default)]
    pub private: bool,
}

impl ChangeEntry {
    pub fn manual(author: &str, summary: impl Into<String>, occurred: Option<NaiveDate>) -> Self {
        Self {
            id: ChangeId::new(),
            timestamp: Utc::now(),
            occurred,
            author: author.to_string(),
            kind: ChangeKind::Manual,
            summary: summary.into(),
            details: String::new(),
            targets: Vec::new(),
            record: None,
            subject: String::new(),
            owner: None,
            fields: Vec::new(),
            private: false,
        }
    }

    /// The date to show and sort by.
    pub fn effective_date(&self) -> NaiveDate {
        self.occurred.unwrap_or_else(|| self.timestamp.date_naive())
    }
}

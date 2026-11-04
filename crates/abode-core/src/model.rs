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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Level {
    pub id: LevelId,
    pub name: String,
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
}

impl Level {
    pub fn new(name: impl Into<String>, elevation: f64) -> Self {
        Self {
            id: LevelId::new(),
            name: name.into(),
            elevation,
            wall_height: DEFAULT_WALL_HEIGHT,
            underlay: None,
            nodes: BTreeMap::new(),
            walls: BTreeMap::new(),
            rooms: BTreeMap::new(),
            openings: BTreeMap::new(),
            objects: BTreeMap::new(),
        }
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

impl Opening {
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
    Other,
}

impl ObjectCategory {
    pub const ALL: [ObjectCategory; 8] = [
        ObjectCategory::Appliance,
        ObjectCategory::Hvac,
        ObjectCategory::Plumbing,
        ObjectCategory::Electrical,
        ObjectCategory::Fixture,
        ObjectCategory::Furniture,
        ObjectCategory::Storage,
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
    /// The room this object belongs to.
    pub room: RoomId,
    pub label: String,
    pub category: ObjectCategory,
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
}

impl PlacedObject {
    pub fn world_outline(&self) -> Vec<Vec2> {
        self.shape
            .outline()
            .into_iter()
            .map(|p| self.position + p.rotated(self.rotation))
            .collect()
    }

    pub fn display_name(&self) -> String {
        if self.label.trim().is_empty() {
            self.category.label().to_string()
        } else {
            self.label.clone()
        }
    }

    pub fn display_color(&self) -> [u8; 3] {
        self.color.unwrap_or_else(|| self.category.color())
    }
}

/// A quick-pick list of common things to place in a room.
pub struct ObjectPreset {
    pub label: &'static str,
    pub category: ObjectCategory,
    pub shape: ShapeKind,
    pub width: f64,
    pub depth: f64,
}

pub const OBJECT_PRESETS: &[ObjectPreset] = &[
    ObjectPreset { label: "Rectangle", category: ObjectCategory::Other, shape: ShapeKind::Rectangle, width: 0.6, depth: 0.6 },
    ObjectPreset { label: "Circle", category: ObjectCategory::Other, shape: ShapeKind::Circle, width: 0.6, depth: 0.6 },
    ObjectPreset { label: "Ellipse", category: ObjectCategory::Other, shape: ShapeKind::Ellipse, width: 0.8, depth: 0.5 },
    ObjectPreset { label: "Triangle", category: ObjectCategory::Other, shape: ShapeKind::Triangle, width: 0.6, depth: 0.6 },
    ObjectPreset { label: "Fridge", category: ObjectCategory::Appliance, shape: ShapeKind::Rectangle, width: 0.9, depth: 0.75 },
    ObjectPreset { label: "Stove / range", category: ObjectCategory::Appliance, shape: ShapeKind::Rectangle, width: 0.76, depth: 0.66 },
    ObjectPreset { label: "Dishwasher", category: ObjectCategory::Appliance, shape: ShapeKind::Rectangle, width: 0.6, depth: 0.6 },
    ObjectPreset { label: "Microwave", category: ObjectCategory::Appliance, shape: ShapeKind::Rectangle, width: 0.6, depth: 0.4 },
    ObjectPreset { label: "Washer", category: ObjectCategory::Appliance, shape: ShapeKind::Rectangle, width: 0.69, depth: 0.71 },
    ObjectPreset { label: "Dryer", category: ObjectCategory::Appliance, shape: ShapeKind::Rectangle, width: 0.69, depth: 0.71 },
    ObjectPreset { label: "Furnace", category: ObjectCategory::Hvac, shape: ShapeKind::Rectangle, width: 0.56, depth: 0.74 },
    ObjectPreset { label: "Air conditioner", category: ObjectCategory::Hvac, shape: ShapeKind::Rectangle, width: 0.8, depth: 0.8 },
    ObjectPreset { label: "Heat pump", category: ObjectCategory::Hvac, shape: ShapeKind::Rectangle, width: 0.9, depth: 0.4 },
    ObjectPreset { label: "Water heater", category: ObjectCategory::Plumbing, shape: ShapeKind::Circle, width: 0.56, depth: 0.56 },
    ObjectPreset { label: "Water softener", category: ObjectCategory::Plumbing, shape: ShapeKind::Circle, width: 0.35, depth: 0.35 },
    ObjectPreset { label: "Sump pump", category: ObjectCategory::Plumbing, shape: ShapeKind::Circle, width: 0.45, depth: 0.45 },
    ObjectPreset { label: "Toilet", category: ObjectCategory::Plumbing, shape: ShapeKind::Ellipse, width: 0.4, depth: 0.7 },
    ObjectPreset { label: "Bathtub", category: ObjectCategory::Plumbing, shape: ShapeKind::Rectangle, width: 1.52, depth: 0.76 },
    ObjectPreset { label: "Shower", category: ObjectCategory::Plumbing, shape: ShapeKind::Rectangle, width: 0.9, depth: 0.9 },
    ObjectPreset { label: "Sink", category: ObjectCategory::Plumbing, shape: ShapeKind::Rectangle, width: 0.6, depth: 0.5 },
    ObjectPreset { label: "Electrical panel", category: ObjectCategory::Electrical, shape: ShapeKind::Rectangle, width: 0.36, depth: 0.1 },
    ObjectPreset { label: "Smoke / CO detector", category: ObjectCategory::Electrical, shape: ShapeKind::Circle, width: 0.15, depth: 0.15 },
    ObjectPreset { label: "Fireplace", category: ObjectCategory::Hvac, shape: ShapeKind::Rectangle, width: 1.2, depth: 0.5 },
];

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
}

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
}

impl ServiceRecord {
    pub fn new(target: EntityRef, date: NaiveDate) -> Self {
        Self {
            id: ServiceId::new(),
            target,
            date,
            kind: ServiceKind::Maintenance,
            performed_by: String::new(),
            cost: String::new(),
            description: String::new(),
            private: false,
            attachments: Vec::new(),
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
        }
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaintSurface {
    Walls,
    Ceiling,
    Trim,
    Doors,
    Cabinets,
    Other,
}

impl PaintSurface {
    pub const ALL: [PaintSurface; 6] = [
        PaintSurface::Walls,
        PaintSurface::Ceiling,
        PaintSurface::Trim,
        PaintSurface::Doors,
        PaintSurface::Cabinets,
        PaintSurface::Other,
    ];
    pub fn label(self) -> &'static str {
        match self {
            PaintSurface::Walls => "Walls",
            PaintSurface::Ceiling => "Ceiling",
            PaintSurface::Trim => "Trim",
            PaintSurface::Doors => "Doors",
            PaintSurface::Cabinets => "Cabinets",
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
    Other,
}

impl PaintFinish {
    pub const ALL: [PaintFinish; 7] = [
        PaintFinish::Flat,
        PaintFinish::Matte,
        PaintFinish::Eggshell,
        PaintFinish::Satin,
        PaintFinish::SemiGloss,
        PaintFinish::Gloss,
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
            PaintFinish::Other => "Other",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Paint {
    pub id: PaintId,
    pub target: PaintTarget,
    pub surface: PaintSurface,
    #[serde(default)]
    pub brand: String,
    #[serde(default)]
    pub color_name: String,
    #[serde(default)]
    pub color_code: String,
    pub finish: PaintFinish,
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
    pub fn new(target: PaintTarget) -> Self {
        Self {
            id: PaintId::new(),
            target,
            surface: PaintSurface::Walls,
            brand: String::new(),
            color_name: String::new(),
            color_code: String::new(),
            finish: PaintFinish::Eggshell,
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

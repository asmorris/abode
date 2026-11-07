//! Outside the house: the lot (property line), decks, patios, fences, trees,
//! and outbuildings like a shed or detached garage.
//!
//! All of it lives on one special "Outside" level (see [`LevelKind::Site`]),
//! drawn around the house's ground floor. Outbuildings are small buildings
//! with their own levels, edited with the same tools as the house.

use crate::geometry::*;
use crate::ids::*;
use crate::model::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureGeom {
    /// A closed outline (deck, patio...).
    Area,
    /// An open polyline (fence, hedge...).
    Line,
    /// A single point with a size (tree).
    Point,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureKind {
    Deck,
    Patio,
    Driveway,
    /// A roof on posts to park under.
    Carport,
    Path,
    GardenBed,
    Pool,
    Lawn,
    PropertyLine,
    Fence,
    RetainingWall,
    Hedge,
    Tree,
    /// Any other kind of area; its name is in [`Feature::kind_name`].
    Other,
}

impl FeatureKind {
    pub const AREAS: [FeatureKind; 9] = [
        FeatureKind::Deck,
        FeatureKind::Patio,
        FeatureKind::Driveway,
        FeatureKind::Carport,
        FeatureKind::Path,
        FeatureKind::GardenBed,
        FeatureKind::Pool,
        FeatureKind::Lawn,
        FeatureKind::Other,
    ];
    pub const LINES: [FeatureKind; 3] = [FeatureKind::Fence, FeatureKind::RetainingWall, FeatureKind::Hedge];

    pub fn geom(self) -> FeatureGeom {
        match self {
            FeatureKind::Fence | FeatureKind::RetainingWall | FeatureKind::Hedge => FeatureGeom::Line,
            FeatureKind::Tree => FeatureGeom::Point,
            _ => FeatureGeom::Area,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FeatureKind::Deck => "Deck",
            FeatureKind::Patio => "Patio",
            FeatureKind::Driveway => "Driveway",
            FeatureKind::Carport => "Carport",
            FeatureKind::Path => "Path",
            FeatureKind::GardenBed => "Garden bed",
            FeatureKind::Pool => "Pool",
            FeatureKind::Lawn => "Lawn",
            FeatureKind::PropertyLine => "Property line",
            FeatureKind::Fence => "Fence",
            FeatureKind::RetainingWall => "Retaining wall",
            FeatureKind::Hedge => "Hedge",
            FeatureKind::Tree => "Tree",
            FeatureKind::Other => "Other",
        }
    }

    /// Default height: deck surface above ground, fence/wall/hedge height,
    /// pool depth, tree height.
    pub fn default_height(self) -> f64 {
        match self {
            FeatureKind::Deck => 0.6,
            FeatureKind::Fence => 1.83,
            FeatureKind::RetainingWall => 0.9,
            FeatureKind::Hedge => 1.5,
            FeatureKind::Pool => 1.5,
            FeatureKind::Tree => 8.0,
            FeatureKind::GardenBed => 0.3,
            FeatureKind::Carport => 2.4,
            _ => 0.0,
        }
    }

    /// What `height` means for this kind, for labels.
    pub fn height_label(self) -> Option<&'static str> {
        Some(match self {
            FeatureKind::Deck => "Height above ground",
            FeatureKind::Fence | FeatureKind::RetainingWall | FeatureKind::Hedge | FeatureKind::Tree => "Height",
            FeatureKind::GardenBed => "Bed height",
            FeatureKind::Pool => "Depth",
            FeatureKind::Carport => "Clearance",
            _ => return None,
        })
    }

    pub fn material_suggestions(self) -> &'static [&'static str] {
        match self {
            FeatureKind::Deck => &["Pressure-treated pine", "Cedar", "Redwood", "Composite (Trex)", "PVC", "Ipe", "Aluminium"],
            FeatureKind::Patio | FeatureKind::Path => &["Concrete", "Stamped concrete", "Pavers", "Flagstone", "Brick", "Gravel", "Stone"],
            FeatureKind::Driveway => &["Asphalt", "Concrete", "Pavers", "Gravel", "Brick"],
            FeatureKind::Carport => &["Steel", "Aluminium", "Wood", "Polycarbonate roof", "Metal roof", "Shingled roof"],
            FeatureKind::GardenBed => &["Wood frame", "Stone edging", "Brick edging", "Mulch", "Raised cedar"],
            FeatureKind::Pool => &["Concrete / gunite", "Vinyl liner", "Fibreglass", "Above-ground"],
            FeatureKind::Fence => &["Wood (privacy)", "Wood (picket)", "Vinyl", "Chain link", "Aluminium", "Wrought iron", "Composite"],
            FeatureKind::RetainingWall => &["Concrete block", "Poured concrete", "Natural stone", "Timber", "Gabion"],
            FeatureKind::Hedge => &["Cedar", "Boxwood", "Privet", "Yew", "Laurel"],
            FeatureKind::Tree => &["Maple", "Oak", "Birch", "Spruce", "Pine", "Apple", "Cherry"],
            FeatureKind::Lawn => &["Grass", "Clover", "Artificial turf"],
            FeatureKind::PropertyLine | FeatureKind::Other => &[],
        }
    }

    /// Default display colour (sRGB).
    pub fn color(self) -> [u8; 3] {
        match self {
            FeatureKind::Deck => [196, 160, 118],
            FeatureKind::Patio => [205, 200, 190],
            FeatureKind::Driveway => [178, 176, 172],
            FeatureKind::Carport => [140, 150, 160],
            FeatureKind::Path => [214, 206, 190],
            FeatureKind::GardenBed => [150, 118, 88],
            FeatureKind::Pool => [140, 196, 214],
            FeatureKind::Lawn => [196, 222, 176],
            FeatureKind::PropertyLine => [120, 120, 120],
            FeatureKind::Fence => [150, 116, 82],
            FeatureKind::RetainingWall => [150, 146, 140],
            FeatureKind::Hedge => [96, 140, 88],
            FeatureKind::Tree => [110, 158, 96],
            FeatureKind::Other => [196, 188, 172],
        }
    }

    /// Whether items can sit on it (a BBQ on the deck...).
    pub fn holds_items(self) -> bool {
        matches!(self, FeatureKind::Deck | FeatureKind::Patio | FeatureKind::Driveway | FeatureKind::Carport | FeatureKind::Lawn | FeatureKind::Other)
    }
}

/// A gate in a fence (or an opening in a hedge or wall).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gate {
    pub id: GateId,
    #[serde(default)]
    pub label: String,
    /// Distance along the line to the gate's centre.
    pub offset: f64,
    pub width: f64,
}

impl Gate {
    pub fn new(offset: f64) -> Self {
        Self { id: GateId::new(), label: String::new(), offset, width: 1.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    pub id: FeatureId,
    pub kind: FeatureKind,
    /// A typed-in kind ("Hot tub pad") when `kind` is Other.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind_name: String,
    #[serde(default)]
    pub name: String,
    /// Area: outline (positive winding). Line: the path. Point: one point.
    pub points: Vec<Vec2>,
    #[serde(default)]
    pub material: String,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    /// See [`FeatureKind::default_height`].
    #[serde(default)]
    pub height: f64,
    /// Tree canopy diameter.
    #[serde(default)]
    pub size: f64,
    #[serde(default)]
    pub railing: bool,
    #[serde(default)]
    pub railing_material: String,
    #[serde(default)]
    pub stairs: String,
    /// Fences and walls on the lot line.
    #[serde(default)]
    pub shared_with_neighbour: bool,
    #[serde(default)]
    pub neighbour: String,
    #[serde(default)]
    pub gates: Vec<Gate>,
    #[serde(default)]
    pub details: ItemDetails,
}

impl Feature {
    pub fn new(kind: FeatureKind, points: Vec<Vec2>) -> Self {
        let points = if kind.geom() == FeatureGeom::Area { positive_winding(&points) } else { points };
        Self {
            id: FeatureId::new(),
            kind,
            kind_name: String::new(),
            name: String::new(),
            points,
            material: String::new(),
            color: None,
            height: kind.default_height(),
            size: if kind == FeatureKind::Tree { 5.0 } else { 0.0 },
            railing: kind == FeatureKind::Deck,
            railing_material: String::new(),
            stairs: String::new(),
            shared_with_neighbour: false,
            neighbour: String::new(),
            gates: Vec::new(),
            details: ItemDetails::default(),
        }
    }

    /// The kind's name, or the typed-in one for Other.
    pub fn kind_label(&self) -> String {
        custom_label(self.kind == FeatureKind::Other, &self.kind_name, self.kind.label())
    }

    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() {
            self.kind_label()
        } else {
            self.name.clone()
        }
    }

    pub fn display_color(&self) -> [u8; 3] {
        self.color.unwrap_or_else(|| self.kind.color())
    }

    /// Thickness of a line feature on the plan, metres.
    pub fn line_width(&self) -> f64 {
        match self.kind {
            FeatureKind::Hedge => 0.8,
            FeatureKind::RetainingWall => 0.3,
            _ => 0.1,
        }
    }

    pub fn contains(&self, p: Vec2) -> bool {
        match self.kind.geom() {
            FeatureGeom::Area => point_in_polygon(p, &self.points),
            FeatureGeom::Line => distance_to_polyline(&self.points, p) <= self.line_width() / 2.0,
            FeatureGeom::Point => self.points.first().is_some_and(|c| c.distance(p) <= self.size.max(0.5) / 2.0),
        }
    }

    pub fn length(&self) -> f64 {
        match self.kind.geom() {
            FeatureGeom::Area => perimeter(&self.points),
            FeatureGeom::Line => polyline_length(&self.points),
            FeatureGeom::Point => 0.0,
        }
    }

    pub fn area(&self) -> f64 {
        if self.kind.geom() == FeatureGeom::Area { area(&self.points) } else { 0.0 }
    }

    /// The sides of an area's outline or the segments of a line:
    /// (start, end) for each one, in order.
    pub fn sides(&self) -> Vec<(Vec2, Vec2)> {
        let n = self.points.len();
        match self.kind.geom() {
            FeatureGeom::Area if n >= 3 => (0..n).map(|i| (self.points[i], self.points[(i + 1) % n])).collect(),
            FeatureGeom::Line if n >= 2 => (0..n - 1).map(|i| (self.points[i], self.points[i + 1])).collect(),
            _ => Vec::new(),
        }
    }

    /// Make side `i` `length` long, keeping its direction and its start.
    ///
    /// A line slides everything after the side along with it. An outline
    /// moves the side's end together with the next corner, so the next side
    /// keeps its length and angle and the one after it stretches: on a
    /// rectangle the opposite side changes too and it stays a rectangle.
    pub fn set_side_length(&mut self, i: usize, length: f64) -> bool {
        let sides = self.sides();
        let Some(&(a, b)) = sides.get(i) else { return false };
        let old = a.distance(b);
        if length < 0.1 || old < 1e-9 {
            return false;
        }
        let delta = (b - a) * (length / old - 1.0);
        let n = self.points.len();
        match self.kind.geom() {
            FeatureGeom::Line => {
                let before: f64 = sides[..=i].iter().map(|(a, b)| a.distance(*b)).sum();
                for p in &mut self.points[i + 1..] {
                    *p += delta;
                }
                // Gates past this side slide with it.
                for g in &mut self.gates {
                    if g.offset > before {
                        g.offset += length - old;
                    }
                }
                self.clamp_gates();
            }
            FeatureGeom::Area => {
                let end = (i + 1) % n;
                let next = (i + 2) % n;
                self.points[end] += delta;
                // A triangle has no "next corner" to spare.
                if n > 3 {
                    self.points[next] += delta;
                }
            }
            FeatureGeom::Point => return false,
        }
        true
    }

    /// Keep gates on the line and inside its length.
    pub fn clamp_gates(&mut self) {
        let len = polyline_length(&self.points);
        for g in &mut self.gates {
            g.width = g.width.clamp(0.3, len.max(0.3));
            g.offset = g.offset.clamp(g.width / 2.0, (len - g.width / 2.0).max(g.width / 2.0));
        }
    }
}

// ---------------------------------------------------------------------------
// Outbuildings

pub const BUILDING_KIND_SUGGESTIONS: &[&str] = &[
    "Shed",
    "Detached garage",
    "Workshop",
    "Studio",
    "Pool house",
    "Greenhouse",
    "Carport",
    "Barn",
    "Gazebo",
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Building {
    pub id: BuildingId,
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub details: ItemDetails,
}

impl Building {
    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() {
            "Building".into()
        } else {
            self.name.clone()
        }
    }
}

// ---------------------------------------------------------------------------

impl Level {
    pub fn is_site(&self) -> bool {
        self.kind == LevelKind::Site
    }

    /// The area feature at `p` that items can sit on (smallest first).
    pub fn feature_at(&self, p: Vec2) -> Option<FeatureId> {
        self.features
            .values()
            .filter(|f| f.kind.holds_items() && f.kind.geom() == FeatureGeom::Area && f.contains(p))
            .min_by(|a, b| a.area().total_cmp(&b.area()))
            .map(|f| f.id)
    }

    /// The roof section covering `p` (smallest first).
    pub fn roof_at(&self, p: Vec2) -> Option<RoofId> {
        self.roofs
            .values()
            .filter_map(|r| {
                let fp = r.footprint();
                point_in_polygon(p, &fp).then(|| (area(&fp), r.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }

    /// Plan points of everything on the level (for fitting the view).
    pub fn extent_points(&self) -> Vec<Vec2> {
        let mut pts: Vec<Vec2> = self.nodes.values().map(|n| n.pos).collect();
        for f in self.features.values() {
            match f.kind.geom() {
                FeatureGeom::Point => {
                    if let Some(c) = f.points.first() {
                        let r = f.size / 2.0;
                        pts.push(*c + Vec2::new(-r, -r));
                        pts.push(*c + Vec2::new(r, r));
                    }
                }
                _ => pts.extend(f.points.iter().copied()),
            }
        }
        for r in self.roofs.values() {
            pts.extend(r.footprint());
        }
        for o in self.objects.values() {
            pts.push(o.position);
        }
        pts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates_stay_on_the_fence() {
        let mut f = Feature::new(FeatureKind::Fence, vec![Vec2::ZERO, Vec2::new(5.0, 0.0), Vec2::new(5.0, 5.0)]);
        f.gates.push(Gate::new(20.0));
        f.gates.push(Gate::new(-3.0));
        f.clamp_gates();
        assert!((f.gates[0].offset - 9.5).abs() < 1e-9);
        assert!((f.gates[1].offset - 0.5).abs() < 1e-9);
        assert!((f.length() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn rectangle_lot_stays_a_rectangle() {
        let mut f = Feature::new(FeatureKind::PropertyLine, rect_points(Vec2::ZERO, 10.0, 20.0));
        let top = f.sides().iter().position(|(a, b)| (a.y - b.y).abs() < 1e-9).unwrap();
        assert!(f.set_side_length(top, 15.0));
        let lens: Vec<f64> = f.sides().iter().map(|(a, b)| a.distance(*b)).collect();
        assert_eq!(lens.iter().filter(|l| (*l - 15.0).abs() < 1e-9).count(), 2);
        assert_eq!(lens.iter().filter(|l| (*l - 20.0).abs() < 1e-9).count(), 2);
        assert!((f.area() - 300.0).abs() < 1e-9);
    }

    #[test]
    fn fence_segment_slides_the_rest() {
        let mut f = Feature::new(FeatureKind::Fence, vec![Vec2::ZERO, Vec2::new(5.0, 0.0), Vec2::new(5.0, 5.0)]);
        f.gates.push(Gate::new(8.0));
        assert!(f.set_side_length(0, 7.0));
        assert_eq!(f.points[2], Vec2::new(7.0, 5.0));
        assert!((f.length() - 12.0).abs() < 1e-9);
        assert!((f.gates[0].offset - 10.0).abs() < 1e-9);
    }

    #[test]
    fn line_features_hit_near_the_line() {
        let f = Feature::new(FeatureKind::Hedge, vec![Vec2::ZERO, Vec2::new(5.0, 0.0)]);
        assert!(f.contains(Vec2::new(2.0, 0.3)));
        assert!(!f.contains(Vec2::new(2.0, 1.0)));
    }
}

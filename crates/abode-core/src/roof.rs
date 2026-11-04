//! Roof sections: an outline drawn on a level's plan, a shape (flat, shed,
//! gable, hip), a pitch and an overhang, plus the layers of roofing on it.
//!
//! Geometry: every sloped shape is "height rises with distance from an eave
//! line". Each eave is a linear function `f(p) = n·p + c` (distance inward
//! from that eave), and the roof height is `base + pitch × min_k f_k(p)`.
//! The region where one `f_k` is the smallest is one flat facet, which we get
//! by clipping the footprint against half-planes. That's exact for
//! rectangles (and any convex outline), which is what roofs mostly are; an
//! L-shaped roof is drawn as two overlapping sections.

use crate::geometry::*;
use crate::ids::*;
use crate::model::CustomField;
use crate::units::UnitSystem;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

pub const DEFAULT_OVERHANG: f64 = 0.3;
/// 6 in 12.
pub const DEFAULT_PITCH: f64 = 0.5;
const FASCIA: f64 = 0.18;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoofShape {
    Flat,
    Shed,
    Gable,
    Hip,
}

impl RoofShape {
    pub const ALL: [RoofShape; 4] = [RoofShape::Gable, RoofShape::Hip, RoofShape::Shed, RoofShape::Flat];
    pub fn label(self) -> &'static str {
        match self {
            RoofShape::Flat => "Flat",
            RoofShape::Shed => "Shed",
            RoofShape::Gable => "Gable",
            RoofShape::Hip => "Hip",
        }
    }
    pub fn is_sloped(self) -> bool {
        self != RoofShape::Flat
    }
    /// Whether `direction` means anything for this shape.
    pub fn has_direction(self) -> bool {
        matches!(self, RoofShape::Shed | RoofShape::Gable)
    }
}

pub const ROOF_MATERIAL_SUGGESTIONS: &[&str] = &[
    "Asphalt shingles (3-tab)",
    "Asphalt shingles (architectural)",
    "Metal (standing seam)",
    "Metal (corrugated)",
    "Clay tile",
    "Concrete tile",
    "Slate",
    "Cedar shakes",
    "Wood shingles",
    "EPDM membrane",
    "TPO membrane",
    "Modified bitumen",
    "Built-up (tar & gravel)",
    "Rolled roofing",
    "Synthetic slate",
    "Polycarbonate",
];

/// One layer of roofing. Layers are stored bottom (oldest) first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoofLayer {
    pub material: String,
    pub color: String,
    pub swatch: Option<[u8; 3]>,
    pub installed: Option<NaiveDate>,
    pub installed_by: String,
    pub warranty_expires: Option<NaiveDate>,
    pub warranty_details: String,
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoofSection {
    pub id: RoofId,
    #[serde(default)]
    pub name: String,
    /// Outline at the walls (before overhang), positive winding.
    pub outline: Vec<Vec2>,
    pub shape: RoofShape,
    /// Rise over run (0.5 = 6 in 12).
    pub pitch: f64,
    /// Radians. Gable: the ridge runs this way. Shed: the roof slopes down
    /// toward this direction.
    #[serde(default)]
    pub direction: f64,
    #[serde(default = "default_overhang")]
    pub overhang: f64,
    /// Height of the eaves above this level's floor. Defaults to the level's
    /// wall height.
    #[serde(default)]
    pub eave_height: Option<f64>,
    #[serde(default)]
    pub layers: Vec<RoofLayer>,
    #[serde(default)]
    pub last_inspected: Option<NaiveDate>,
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
}

fn default_overhang() -> f64 {
    DEFAULT_OVERHANG
}

/// A linear "distance from an eave" function: `n·p + c`.
#[derive(Clone, Copy, Debug)]
struct Eave {
    n: Vec2,
    c: f64,
}

impl Eave {
    fn at(self, p: Vec2) -> f64 {
        self.n.dot(p) + self.c
    }
}

impl RoofSection {
    pub fn new(outline: &[Vec2], shape: RoofShape) -> Self {
        let outline = positive_winding(outline);
        let direction = longest_edge_angle(&outline);
        let direction = if shape == RoofShape::Shed { direction + std::f64::consts::FRAC_PI_2 } else { direction };
        Self {
            id: RoofId::new(),
            name: String::new(),
            outline,
            shape,
            pitch: DEFAULT_PITCH,
            direction,
            overhang: DEFAULT_OVERHANG,
            eave_height: None,
            layers: vec![RoofLayer::default()],
            last_inspected: None,
            custom_fields: Vec::new(),
        }
    }

    pub fn display_name(&self) -> String {
        if self.name.trim().is_empty() {
            format!("{} roof", self.shape.label())
        } else {
            self.name.clone()
        }
    }

    /// The top (newest) layer, which is what you see.
    pub fn top_layer(&self) -> Option<&RoofLayer> {
        self.layers.last()
    }

    /// Outline including the overhang.
    pub fn footprint(&self) -> Vec<Vec2> {
        offset_polygon(&self.outline, self.overhang.max(0.0))
    }

    fn eaves(&self) -> Vec<Eave> {
        let o = &self.outline;
        if o.len() < 3 {
            return Vec::new();
        }
        match self.shape {
            RoofShape::Flat => Vec::new(),
            RoofShape::Shed => {
                let u = Vec2::new(self.direction.cos(), self.direction.sin());
                let max = o.iter().map(|p| u.dot(*p)).fold(f64::MIN, f64::max);
                vec![Eave { n: -u, c: max }]
            }
            RoofShape::Gable => {
                let r = Vec2::new(self.direction.cos(), self.direction.sin());
                let n = r.perp();
                let min = o.iter().map(|p| n.dot(*p)).fold(f64::MAX, f64::min);
                let max = o.iter().map(|p| n.dot(*p)).fold(f64::MIN, f64::max);
                vec![Eave { n, c: -min }, Eave { n: -n, c: max }]
            }
            RoofShape::Hip => {
                let k = o.len();
                (0..k)
                    .filter_map(|i| {
                        let (a, b) = (o[i], o[(i + 1) % k]);
                        if a.distance(b) < 1e-6 {
                            return None;
                        }
                        // perp() points inward for positive winding.
                        let n = (b - a).normalized().perp();
                        Some(Eave { n, c: -n.dot(a) })
                    })
                    .collect()
            }
        }
    }

    fn base(&self, level_floor: f64, level_wall_height: f64) -> f64 {
        level_floor + self.eave_height.unwrap_or(level_wall_height)
    }

    /// Height above the ground floor at plan point `p`.
    pub fn height_at(&self, base: f64, p: Vec2) -> f64 {
        let e = self.eaves();
        if e.is_empty() {
            return base + 0.05;
        }
        base + self.pitch * e.iter().map(|e| e.at(p)).fold(f64::MAX, f64::min)
    }

    /// The flat pieces of the roof surface, as plan polygons.
    pub fn facets(&self) -> Vec<Vec<Vec2>> {
        let fp = self.footprint();
        let eaves = self.eaves();
        if eaves.len() <= 1 {
            return if fp.len() >= 3 { vec![fp] } else { Vec::new() };
        }
        let mut out = Vec::new();
        for (k, ek) in eaves.iter().enumerate() {
            let mut poly = fp.clone();
            for (j, ej) in eaves.iter().enumerate() {
                if j == k || poly.len() < 3 {
                    continue;
                }
                // Keep where f_k <= f_j, i.e. (n_k - n_j)·p <= c_j - c_k.
                // Ties go to the lower index so overlapping eaves don't double up.
                let eps = if j < k { 1e-9 } else { 0.0 };
                poly = clip_half_plane(&poly, ek.n - ej.n, ej.c - ek.c - eps);
            }
            if poly.len() >= 3 && area(&poly) > 1e-6 {
                out.push(poly);
            }
        }
        out
    }

    /// Ridge, hip and valley lines (facet edges inside the footprint).
    pub fn ridge_lines(&self) -> Vec<(Vec2, Vec2)> {
        let fp = self.footprint();
        let on_boundary = |m: Vec2| {
            let n = fp.len();
            (0..n).any(|i| distance_to_segment(m, fp[i], fp[(i + 1) % n]) < 1e-6)
        };
        let mut out: Vec<(Vec2, Vec2)> = Vec::new();
        for f in self.facets() {
            let n = f.len();
            for i in 0..n {
                let (a, b) = (f[i], f[(i + 1) % n]);
                if a.distance(b) < 1e-6 || on_boundary(a.lerp(b, 0.5)) {
                    continue;
                }
                let dup = out
                    .iter()
                    .any(|(p, q)| (p.distance(a) < 1e-6 && q.distance(b) < 1e-6) || (p.distance(b) < 1e-6 && q.distance(a) < 1e-6));
                if !dup {
                    out.push((a, b));
                }
            }
        }
        out
    }

    /// Area of the roof surface itself (sloped), m².
    pub fn surface_area(&self) -> f64 {
        let a = area(&self.footprint());
        if self.shape.is_sloped() { a * (1.0 + self.pitch * self.pitch).sqrt() } else { a }
    }

    /// Area covered on the plan (including overhang), m².
    pub fn plan_area(&self) -> f64 {
        area(&self.footprint())
    }

    /// How far the highest point is above the eaves (at the walls).
    pub fn rise(&self) -> f64 {
        let e = self.eaves();
        if e.is_empty() {
            return 0.0;
        }
        self.facets()
            .iter()
            .flatten()
            .map(|p| self.pitch * e.iter().map(|e| e.at(*p)).fold(f64::MAX, f64::min))
            .fold(0.0, f64::max)
    }

    /// 3D geometry for display: roof surfaces, gable-end walls and fascia,
    /// each as a polygon of (plan point, height).
    pub fn mesh(&self, level_floor: f64, level_wall_height: f64) -> RoofMesh {
        let base = self.base(level_floor, level_wall_height);
        let h = |p: Vec2| self.height_at(base, p);
        let surfaces: Vec<Vec<(Vec2, f64)>> = self
            .facets()
            .into_iter()
            .map(|f| f.into_iter().map(|p| (p, h(p))).collect())
            .collect();

        // Fascia boards around the footprint.
        let fp = self.footprint();
        let mut fascia = Vec::new();
        let n = fp.len();
        let breaks = self.ridge_lines();
        for i in 0..n {
            let (a, b) = (fp[i], fp[(i + 1) % n]);
            for (p, q) in split_at_lines(a, b, &breaks) {
                fascia.push(vec![(p, h(p) - FASCIA), (q, h(q) - FASCIA), (q, h(q)), (p, h(p))]);
            }
        }

        // Gable ends / the tall side of a shed roof, at the walls.
        let mut gables = Vec::new();
        let o = &self.outline;
        let n = o.len();
        if self.shape.is_sloped() {
            for i in 0..n {
                let (a, b) = (o[i], o[(i + 1) % n]);
                for (p, q) in split_at_lines(a, b, &breaks) {
                    let (hp, hq) = (h(p), h(q));
                    if hp - base > 0.02 || hq - base > 0.02 {
                        gables.push(vec![(p, base), (q, base), (q, hq.max(base)), (p, hp.max(base))]);
                    }
                }
            }
        }
        RoofMesh { surfaces, fascia, gables }
    }
}

pub struct RoofMesh {
    pub surfaces: Vec<Vec<(Vec2, f64)>>,
    pub fascia: Vec<Vec<(Vec2, f64)>>,
    pub gables: Vec<Vec<(Vec2, f64)>>,
}

/// Split segment a→b where it crosses any of `lines`, so heights along each
/// piece are linear.
fn split_at_lines(a: Vec2, b: Vec2, lines: &[(Vec2, Vec2)]) -> Vec<(Vec2, Vec2)> {
    let d = b - a;
    let len = d.length();
    if len < 1e-9 {
        return Vec::new();
    }
    let mut ts = vec![0.0, 1.0];
    for (p, q) in lines {
        let e = *q - *p;
        let denom = d.cross(e);
        if denom.abs() < 1e-12 {
            continue;
        }
        let t = (*p - a).cross(e) / denom;
        let u = (*p - a).cross(d) / denom;
        if t > 1e-6 && t < 1.0 - 1e-6 && (-1e-6..=1.0 + 1e-6).contains(&u) {
            ts.push(t);
        }
    }
    ts.sort_by(f64::total_cmp);
    ts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    ts.windows(2).map(|w| (a + d * w[0], a + d * w[1])).collect()
}

fn longest_edge_angle(poly: &[Vec2]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| (poly[i], poly[(i + 1) % n]))
        .max_by(|x, y| x.0.distance(x.1).total_cmp(&y.0.distance(y.1)))
        .map(|(a, b)| {
            let d = b - a;
            d.y.atan2(d.x)
        })
        .unwrap_or(0.0)
}

/// "6/12" in imperial, "26.6°" in metric.
pub fn format_pitch(pitch: f64, units: UnitSystem) -> String {
    match units {
        UnitSystem::Imperial => {
            let x = pitch * 12.0;
            if (x - x.round()).abs() < 0.05 { format!("{:.0}/12", x) } else { format!("{:.1}/12", x) }
        }
        UnitSystem::Metric => format!("{:.1}°", pitch.atan().to_degrees()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, d: f64) -> Vec<Vec2> {
        rect_points(Vec2::ZERO, w, d)
    }

    #[test]
    fn gable_has_two_facets_and_one_ridge() {
        let mut r = RoofSection::new(&rect(10.0, 6.0), RoofShape::Gable);
        r.overhang = 0.0;
        assert_eq!(r.facets().len(), 2);
        let ridges = r.ridge_lines();
        assert_eq!(ridges.len(), 1);
        let (a, b) = ridges[0];
        assert!((a.y - 3.0).abs() < 1e-6 && (b.y - 3.0).abs() < 1e-6);
        assert!((r.rise() - 3.0 * DEFAULT_PITCH).abs() < 1e-6);
    }

    #[test]
    fn hip_on_rectangle_has_four_facets() {
        let mut r = RoofSection::new(&rect(10.0, 6.0), RoofShape::Hip);
        r.overhang = 0.0;
        let f = r.facets();
        assert_eq!(f.len(), 4);
        let total: f64 = f.iter().map(|p| area(p)).sum();
        assert!((total - 60.0).abs() < 1e-6);
        // ridge + four hips
        assert_eq!(r.ridge_lines().len(), 5);
    }

    #[test]
    fn shed_slopes_one_way() {
        let mut r = RoofSection::new(&rect(4.0, 3.0), RoofShape::Shed);
        r.overhang = 0.0;
        r.direction = std::f64::consts::FRAC_PI_2; // downhill toward +y
        assert_eq!(r.facets().len(), 1);
        assert!((r.height_at(0.0, Vec2::new(1.0, 3.0))).abs() < 1e-9);
        assert!((r.height_at(0.0, Vec2::new(1.0, 0.0)) - 3.0 * r.pitch).abs() < 1e-9);
        let m = r.mesh(0.0, 2.4);
        assert!(!m.gables.is_empty());
    }

    #[test]
    fn overhang_drops_the_eaves() {
        let r = RoofSection::new(&rect(10.0, 6.0), RoofShape::Gable);
        let fp = r.footprint();
        assert!((area(&fp) - 10.6 * 6.6).abs() < 1e-6);
        let eave = r.height_at(0.0, Vec2::new(5.0, -0.3));
        assert!((eave + 0.3 * r.pitch).abs() < 1e-6);
        assert!(r.surface_area() > r.plan_area());
    }

    #[test]
    fn pitch_formats() {
        assert_eq!(format_pitch(0.5, UnitSystem::Imperial), "6/12");
        assert_eq!(format_pitch(1.0, UnitSystem::Metric), "45.0°");
    }
}

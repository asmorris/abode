//! Simple drawings of the plans, for the export. Not as detailed as the
//! app's plan, but enough to see what's where. Each is a list of shapes in
//! metres (y pointing down, as on the plan) that's written out as an SVG
//! file or drawn straight into the PDF.

use crate::garden::{Layout, Planting};
use crate::geometry::{self, Vec2};
use crate::model::*;
use crate::site::{Feature, FeatureGeom, FeatureKind};
use crate::units::{UnitSystem, format_area};
use std::fmt::Write;

pub type Rgb = [u8; 3];

const INK: Rgb = [47, 52, 50];
const MUTED: Rgb = [123, 129, 126];
const ROOM: Rgb = [244, 240, 232];
const WINDOW: Rgb = [79, 134, 184];
const WHITE: Rgb = [255, 255, 255];

/// Pixels per metre in the SVG's default size.
const SCALE: f64 = 40.0;
/// Space around the drawing, metres.
const PAD: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cap {
    Round,
    Square,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Polygon { points: Vec<Vec2>, fill: Option<Rgb>, stroke: Option<Rgb>, width: f64 },
    Polyline { points: Vec<Vec2>, stroke: Rgb, width: f64, cap: Cap, dash: Option<(f64, f64)> },
    Circle { center: Vec2, radius: f64, fill: Rgb, stroke: Rgb, width: f64 },
    /// Centred on `at`.
    Text { at: Vec2, size: f64, color: Rgb, bold: bool, text: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drawing {
    pub title: String,
    pub shapes: Vec<Shape>,
    min: Vec2,
    max: Vec2,
}

impl Drawing {
    fn new(title: &str) -> Self {
        Self { title: title.to_string(), shapes: Vec::new(), min: Vec2::new(f64::MAX, f64::MAX), max: Vec2::new(f64::MIN, f64::MIN) }
    }

    fn grow(&mut self, ps: &[Vec2]) {
        for p in ps {
            self.min = self.min.min(*p);
            self.max = self.max.max(*p);
        }
    }

    fn polygon(&mut self, points: &[Vec2], fill: Option<Rgb>, stroke: Option<Rgb>, width: f64) {
        if points.len() >= 3 {
            self.grow(points);
            self.shapes.push(Shape::Polygon { points: points.to_vec(), fill, stroke, width });
        }
    }

    fn line(&mut self, points: &[Vec2], stroke: Rgb, width: f64) {
        self.polyline(points, stroke, width, Cap::Round, None);
    }

    fn polyline(&mut self, points: &[Vec2], stroke: Rgb, width: f64, cap: Cap, dash: Option<(f64, f64)>) {
        if points.len() >= 2 {
            self.grow(points);
            self.shapes.push(Shape::Polyline { points: points.to_vec(), stroke, width, cap, dash });
        }
    }

    fn circle(&mut self, center: Vec2, radius: f64, fill: Rgb, stroke: Rgb, width: f64) {
        self.grow(&[center - Vec2::new(radius, radius), center + Vec2::new(radius, radius)]);
        self.shapes.push(Shape::Circle { center, radius, fill, stroke, width });
    }

    /// A label of one or more lines centred on `at` (later lines smaller).
    fn label(&mut self, at: Vec2, size: f64, color: Rgb, bold: bool, lines: &[String]) {
        let lines: Vec<&String> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
        let top = at.y - (lines.len() as f64 - 1.0) * size * 0.6;
        for (i, l) in lines.iter().enumerate() {
            let size = if i == 0 { size } else { size * 0.8 };
            let at = Vec2::new(at.x, top + i as f64 * size * 1.2);
            self.shapes.push(Shape::Text { at, size, color, bold, text: l.to_string() });
        }
    }

    /// The area shown, padding included: (left, top, width, height), metres.
    pub fn view(&self) -> (f64, f64, f64, f64) {
        (self.min.x - PAD, self.min.y - PAD, self.max.x - self.min.x + 2.0 * PAD, self.max.y - self.min.y + 2.0 * PAD)
    }

    /// None when nothing was drawn.
    fn done(self) -> Option<Self> {
        (self.min.x <= self.max.x).then_some(self)
    }

    pub fn to_svg(&self) -> String {
        let (x, y, w, h) = self.view();
        // Keep big plans a sensible size on screen; it's vector, so it scales.
        let k = SCALE.min(1600.0 / w.max(h));
        let mut out = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{x:.3} {y:.3} {w:.3} {h:.3}\" width=\"{:.0}\" height=\"{:.0}\" font-family=\"Helvetica, Arial, sans-serif\">\n<title>{}</title>\n<rect x=\"{x:.3}\" y=\"{y:.3}\" width=\"{w:.3}\" height=\"{h:.3}\" fill=\"#ffffff\"/>\n",
            w * k,
            h * k,
            esc(&self.title)
        );
        for s in &self.shapes {
            let _ = match s {
                Shape::Polygon { points, fill, stroke, width } => writeln!(
                    out,
                    r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="{width:.3}" stroke-linejoin="round"/>"#,
                    pts(points),
                    fill.map(hex).unwrap_or_else(|| "none".into()),
                    stroke.map(hex).unwrap_or_else(|| "none".into())
                ),
                Shape::Polyline { points, stroke, width, cap, dash } => {
                    let cap = match cap {
                        Cap::Round => "round",
                        Cap::Square => "square",
                    };
                    let dash = dash.map(|(a, b)| format!(r#" stroke-dasharray="{a:.3} {b:.3}""#)).unwrap_or_default();
                    writeln!(
                        out,
                        r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{width:.3}" stroke-linecap="{cap}" stroke-linejoin="round"{dash}/>"#,
                        pts(points),
                        hex(*stroke)
                    )
                }
                Shape::Circle { center, radius, fill, stroke, width } => writeln!(
                    out,
                    r#"<circle cx="{:.3}" cy="{:.3}" r="{radius:.3}" fill="{}" stroke="{}" stroke-width="{width:.3}"/>"#,
                    center.x,
                    center.y,
                    hex(*fill),
                    hex(*stroke)
                ),
                Shape::Text { at, size, color, bold, text } => writeln!(
                    out,
                    r#"<text x="{:.3}" y="{:.3}" font-size="{size:.3}" fill="{}" text-anchor="middle" dominant-baseline="middle"{}>{}</text>"#,
                    at.x,
                    at.y,
                    hex(*color),
                    if *bold { r#" font-weight="600""# } else { "" },
                    esc(text)
                ),
            };
        }
        out.push_str("</svg>\n");
        out
    }
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn hex([r, g, b]: Rgb) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// A lighter version of a colour, for fills.
fn tint([r, g, b]: Rgb, t: f64) -> Rgb {
    let m = |c: u8| (c as f64 + (255.0 - c as f64) * t).round() as u8;
    [m(r), m(g), m(b)]
}

fn pts(poly: &[Vec2]) -> String {
    poly.iter().map(|p| format!("{:.3},{:.3}", p.x, p.y)).collect::<Vec<_>>().join(" ")
}

/// Points along a circular arc from `from` around `center` to `to` (the short way).
fn arc(center: Vec2, from: Vec2, to: Vec2) -> Vec<Vec2> {
    let r = from.distance(center);
    let a0 = (from.y - center.y).atan2(from.x - center.x);
    let mut a1 = (to.y - center.y).atan2(to.x - center.x);
    let tau = std::f64::consts::TAU;
    while a1 - a0 > tau / 2.0 {
        a1 -= tau;
    }
    while a0 - a1 > tau / 2.0 {
        a1 += tau;
    }
    (0..=12).map(|i| a0 + (a1 - a0) * i as f64 / 12.0).map(|a| center + Vec2::new(a.cos(), a.sin()) * r).collect()
}

fn rooms_and_walls(d: &mut Drawing, level: &Level, units: UnitSystem, labels: bool) {
    for room in level.rooms_outside_in() {
        d.polygon(&level.room_outline(room), Some(ROOM), None, 0.0);
    }
    for wall in level.walls.values() {
        if let Some(pts) = level.wall_polyline(wall) {
            d.polyline(&pts, INK, wall.thickness.max(0.05), Cap::Square, None);
        }
    }
    for o in level.openings.values() {
        let Some((a, b, dir)) = level.opening_segment(o) else { continue };
        let t = level.walls.get(&o.wall).map(|w| w.thickness).unwrap_or(0.15) + 0.02;
        // A gap in the wall...
        d.line(&[a, b], WHITE, t);
        match o.kind {
            OpeningKind::Window(_) => d.line(&[a, b], WINDOW, 0.06),
            OpeningKind::Door(_) => {
                // ...and the door leaf, swung open, with its swing.
                let (hinge, n) = if o.flip_hinge { (b, -dir) } else { (a, dir) };
                let side = if o.flip_swing { -1.0 } else { 1.0 };
                let leaf = hinge + n.perp() * (o.width * side);
                d.line(&[hinge, leaf], MUTED, 0.03);
                d.line(&arc(hinge, leaf, hinge + n * o.width), MUTED, 0.015);
            }
        }
    }
    if labels {
        let around = level.rooms_around();
        for room in level.rooms.values() {
            if level.room_outline(room).len() >= 3 {
                let area = level.room_area_with(room, &around);
                d.label(level.room_label_point(room), 0.3, INK, true, &[room.display_name(), format_area(area, units)]);
            }
        }
    }
}

fn objects(d: &mut Drawing, level: &Level, on_roof: bool) {
    for o in level.objects.values() {
        if o.roof.is_some() != on_roof {
            continue;
        }
        let outline = o.world_outline();
        let color = o.display_color();
        d.polygon(&outline, Some(tint(color, 0.55)), Some(color), 0.03);
        if let Some(st) = &o.stairs {
            let steps = st.steps.unwrap_or_else(|| ((o.shape.depth / crate::stairs::STEP_RUN).round() as u32).max(2));
            for step in o.step_outlines(steps) {
                d.polygon(&step, None, Some(color), 0.015);
            }
        }
        if !outline.is_empty() {
            let size = (o.shape.width.min(o.shape.depth) * 0.3).clamp(0.12, 0.22);
            d.label(geometry::centroid(&outline), size, INK, false, &[o.display_name()]);
        }
    }
}

fn feature(d: &mut Drawing, f: &Feature, labels: bool) {
    let color = f.display_color();
    match f.kind.geom() {
        FeatureGeom::Area if f.kind == FeatureKind::PropertyLine => {
            let mut ring = f.points.clone();
            if let Some(first) = ring.first().copied() {
                ring.push(first);
            }
            d.polyline(&ring, [138, 109, 59], 0.08, Cap::Round, Some((0.5, 0.25)));
        }
        FeatureGeom::Area => d.polygon(&f.points, Some(tint(color, 0.35)), Some(color), 0.04),
        FeatureGeom::Line => d.line(&f.points, color, f.line_width().max(0.06)),
        FeatureGeom::Point => {
            if let Some(c) = f.points.first() {
                d.circle(*c, (f.size / 2.0).max(0.3), tint(color, 0.4), color, 0.04);
                if labels {
                    d.label(*c, 0.28, INK, false, &[f.display_name()]);
                }
            }
        }
    }
}

/// A floor: rooms, walls, doors, windows and items.
pub fn floor(level: &Level, title: &str, units: UnitSystem) -> Option<Drawing> {
    let mut d = Drawing::new(title);
    rooms_and_walls(&mut d, level, units, true);
    objects(&mut d, level, false);
    d.done()
}

/// The yard: outside areas, fences, trees, and the house and outbuildings
/// as grey footprints.
pub fn site(project: &Project, title: &str) -> Option<Drawing> {
    let site = project.site()?;
    let mut d = Drawing::new(title);
    let mut footprints: Vec<(&Level, String)> = project.ground_level(None).map(|l| (l, String::new())).into_iter().collect();
    for b in &project.buildings {
        if let Some(l) = project.ground_level(Some(b.id)) {
            footprints.push((l, b.display_name()));
        }
    }
    let feats: Vec<&Feature> = site.features.values().collect();
    // Areas first so the house, lines and trees sit on top of them.
    for f in feats.iter().filter(|f| f.kind.geom() == FeatureGeom::Area) {
        feature(&mut d, f, false);
    }
    for (l, name) in &footprints {
        for room in l.rooms_outside_in() {
            d.polygon(&l.room_outline(room), Some([217, 214, 208]), Some([154, 150, 143]), 0.05);
        }
        if !name.is_empty() {
            let pts: Vec<Vec2> = l.nodes.values().map(|n| n.pos).collect();
            if let Some((a, b)) = geometry::bounds(&pts) {
                d.label(a.lerp(b, 0.5), 0.35, INK, true, std::slice::from_ref(name));
            }
        }
    }
    if let Some((a, b)) = project.ground_level(None).and_then(|l| l.bounds()) {
        d.label(a.lerp(b, 0.5), 0.45, INK, true, &[project.entity_name(EntityRef::General)]);
    }
    for f in feats.iter().filter(|f| f.kind.geom() == FeatureGeom::Line) {
        feature(&mut d, f, true);
    }
    for f in feats.iter().filter(|f| f.kind.geom() == FeatureGeom::Point) {
        feature(&mut d, f, true);
    }
    objects(&mut d, site, false);
    // Area names last, so nothing covers them, along the top of each area
    // so they don't sit on the things inside it.
    for f in feats.iter().filter(|f| f.kind.geom() == FeatureGeom::Area && f.kind != FeatureKind::PropertyLine) {
        if let (true, Some((min, max))) = (f.points.len() >= 3, geometry::bounds(&f.points)) {
            let x = geometry::label_point(&f.points).x;
            let y = if max.y - min.y > 1.4 { min.y + 0.45 } else { (min.y + max.y) / 2.0 };
            d.label(Vec2::new(x, y), 0.35, INK, true, &[f.display_name()]);
        }
    }
    d.done()
}

/// Roof sections over the floor plan they sit on.
pub fn roof(level: &Level, title: &str, units: UnitSystem) -> Option<Drawing> {
    if level.roofs.is_empty() {
        return None;
    }
    let mut d = Drawing::new(title);
    rooms_and_walls(&mut d, level, units, false);
    for r in level.roofs.values() {
        let color = r.top_layer().and_then(|l| l.swatch).unwrap_or([150, 120, 100]);
        d.polygon(&r.footprint(), Some(tint(color, 0.45)), Some(color), 0.05);
        for (a, b) in r.ridge_lines() {
            d.line(&[a, b], color, 0.04);
        }
    }
    objects(&mut d, level, true);
    for r in level.roofs.values() {
        let fp = r.footprint();
        if fp.len() >= 3 {
            d.label(geometry::label_point(&fp), 0.35, INK, true, &[r.display_name()]);
        }
    }
    d.done()
}

/// A garden bed and what's planted in it.
pub fn garden(bed: &Feature, plantings: &[&Planting], title: &str) -> Option<Drawing> {
    let mut d = Drawing::new(title);
    d.polygon(&bed.points, Some(tint([150, 118, 88], 0.6)), Some([150, 118, 90]), 0.05);
    for p in plantings {
        let color = p.color.unwrap_or([96, 150, 80]);
        match p.layout {
            Layout::Row => d.line(&p.points, tint(color, 0.2), (p.spacing_m() * 0.8).clamp(0.08, 0.6)),
            Layout::Patch => d.polygon(&p.points, Some(tint(color, 0.45)), Some(color), 0.03),
            Layout::Plant => {
                if let Some(c) = p.points.first() {
                    d.circle(*c, (p.size / 2.0).max(0.1), tint(color, 0.35), color, 0.02);
                }
            }
        }
    }
    for p in plantings {
        let at = match p.layout {
            Layout::Patch if p.points.len() >= 3 => geometry::label_point(&p.points),
            _ => match geometry::bounds(&p.points) {
                Some((a, b)) => a.lerp(b, 0.5),
                None => continue,
            },
        };
        d.label(at, 0.16, INK, false, &[p.display_name()]);
    }
    d.done()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_the_sample() {
        let p = crate::sample::sample_house();
        let l = &p.levels[p.levels.iter().position(|l| !l.is_site()).unwrap()];
        let s = floor(l, "Main floor", p.info.units).unwrap().to_svg();
        assert!(s.starts_with("<svg") && s.contains("Kitchen") && s.trim_end().ends_with("</svg>"));
        assert!(site(&p, "Outside").unwrap().to_svg().contains("<polygon"));
    }

    #[test]
    fn nothing_to_draw() {
        assert!(floor(&Level::new("Empty", 0.0), "Empty", UnitSystem::Metric).is_none());
    }

    #[test]
    fn arcs_go_the_short_way() {
        let a = arc(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0));
        assert!(a.iter().all(|p| p.x >= -1e-9 && p.y >= -1e-9));
        assert!((a.last().unwrap().y - 1.0).abs() < 1e-9);
    }
}

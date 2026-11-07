//! Simple SVG drawings of the plans, for the export. Not as detailed as the
//! app's plan, but enough to see what's where. Coordinates are metres, with
//! y pointing down as on the plan.

use crate::garden::{Layout, Planting};
use crate::geometry::{self, Vec2};
use crate::model::*;
use crate::site::{Feature, FeatureGeom, FeatureKind};
use crate::units::{UnitSystem, format_area};
use std::fmt::Write;

const INK: &str = "#2f3432";
const MUTED: &str = "#7b817e";
const ROOM: &str = "#f4f0e8";
const WINDOW: &str = "#4f86b8";

/// Pixels per metre in the SVG's default size.
const SCALE: f64 = 40.0;

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// A lighter version of a colour, for fills.
fn tint([r, g, b]: [u8; 3], t: f64) -> String {
    let m = |c: u8| (c as f64 + (255.0 - c as f64) * t).round() as u8;
    hex([m(r), m(g), m(b)])
}

fn pts(poly: &[Vec2]) -> String {
    poly.iter().map(|p| format!("{:.3},{:.3}", p.x, p.y)).collect::<Vec<_>>().join(" ")
}

struct Svg {
    body: String,
    min: Vec2,
    max: Vec2,
}

impl Svg {
    fn new() -> Self {
        Self { body: String::new(), min: Vec2::new(f64::MAX, f64::MAX), max: Vec2::new(f64::MIN, f64::MIN) }
    }

    fn grow(&mut self, ps: &[Vec2]) {
        for p in ps {
            self.min = self.min.min(*p);
            self.max = self.max.max(*p);
        }
    }

    fn polygon(&mut self, poly: &[Vec2], fill: &str, stroke: &str, width: f64) {
        if poly.len() < 3 {
            return;
        }
        self.grow(poly);
        let _ = writeln!(self.body, r#"<polygon points="{}" fill="{fill}" stroke="{stroke}" stroke-width="{width:.3}" stroke-linejoin="round"/>"#, pts(poly));
    }

    fn polyline(&mut self, line: &[Vec2], stroke: &str, width: f64, dash: Option<&str>) {
        if line.len() < 2 {
            return;
        }
        self.grow(line);
        let dash = dash.map(|d| format!(r#" stroke-dasharray="{d}""#)).unwrap_or_default();
        let _ = writeln!(
            self.body,
            r#"<polyline points="{}" fill="none" stroke="{stroke}" stroke-width="{width:.3}" stroke-linecap="round" stroke-linejoin="round"{dash}/>"#,
            pts(line)
        );
    }

    fn circle(&mut self, c: Vec2, r: f64, fill: &str, stroke: &str, width: f64) {
        self.grow(&[c - Vec2::new(r, r), c + Vec2::new(r, r)]);
        let _ = writeln!(self.body, r#"<circle cx="{:.3}" cy="{:.3}" r="{r:.3}" fill="{fill}" stroke="{stroke}" stroke-width="{width:.3}"/>"#, c.x, c.y);
    }

    fn label(&mut self, at: Vec2, size: f64, color: &str, bold: bool, lines: &[String]) {
        let lines: Vec<&String> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
        if lines.is_empty() {
            return;
        }
        let weight = if bold { r#" font-weight="600""# } else { "" };
        let top = at.y - (lines.len() as f64 - 1.0) * size * 0.6;
        for (i, l) in lines.iter().enumerate() {
            let size = if i == 0 { size } else { size * 0.8 };
            let y = top + i as f64 * size * 1.2;
            let _ = writeln!(
                self.body,
                r#"<text x="{:.3}" y="{y:.3}" font-size="{size:.3}" fill="{color}" text-anchor="middle" dominant-baseline="middle"{weight}>{}</text>"#,
                at.x,
                esc(l)
            );
        }
    }

    /// The finished drawing, or None when nothing was drawn.
    fn finish(self, title: &str) -> Option<String> {
        if self.min.x > self.max.x {
            return None;
        }
        let pad = 1.0;
        let (x, y) = (self.min.x - pad, self.min.y - pad);
        let (w, h) = (self.max.x - self.min.x + 2.0 * pad, self.max.y - self.min.y + 2.0 * pad);
        // Keep big plans a sensible size on screen; it's vector, so it scales.
        let k = SCALE.min(1600.0 / w.max(h));
        Some(format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{x:.3} {y:.3} {w:.3} {h:.3}\" width=\"{:.0}\" height=\"{:.0}\" font-family=\"Helvetica, Arial, sans-serif\">\n<title>{}</title>\n<rect x=\"{x:.3}\" y=\"{y:.3}\" width=\"{w:.3}\" height=\"{h:.3}\" fill=\"#ffffff\"/>\n{}</svg>\n",
            w * k,
            h * k,
            esc(title),
            self.body
        ))
    }
}

fn rooms_and_walls(svg: &mut Svg, level: &Level, units: UnitSystem, labels: bool) {
    for room in level.rooms.values() {
        svg.polygon(&level.room_points(room), ROOM, "none", 0.0);
    }
    for wall in level.walls.values() {
        if let Some((a, b)) = level.wall_points(wall) {
            svg.grow(&[a, b]);
            let _ = writeln!(
                svg.body,
                r#"<line x1="{:.3}" y1="{:.3}" x2="{:.3}" y2="{:.3}" stroke="{INK}" stroke-width="{:.3}" stroke-linecap="square"/>"#,
                a.x,
                a.y,
                b.x,
                b.y,
                wall.thickness.max(0.05)
            );
        }
    }
    for o in level.openings.values() {
        let Some((a, b, dir)) = level.opening_segment(o) else { continue };
        let t = level.walls.get(&o.wall).map(|w| w.thickness).unwrap_or(0.15) + 0.02;
        // A gap in the wall...
        svg.polyline(&[a, b], "#ffffff", t, None);
        match o.kind {
            OpeningKind::Window(_) => svg.polyline(&[a, b], WINDOW, 0.06, None),
            OpeningKind::Door(_) => {
                // ...and the door leaf, swung open.
                let (hinge, n) = if o.flip_hinge { (b, -dir) } else { (a, dir) };
                let side = if o.flip_swing { -1.0 } else { 1.0 };
                let leaf = hinge + n.perp() * (o.width * side);
                let shut = hinge + n * o.width;
                svg.polyline(&[hinge, leaf], MUTED, 0.03, None);
                // The swing, as a quarter circle from the open leaf to the wall.
                let sweep = u8::from((leaf - hinge).cross(shut - hinge) > 0.0);
                let _ = writeln!(
                    svg.body,
                    r#"<path d="M {:.3} {:.3} A {w:.3} {w:.3} 0 0 {sweep} {:.3} {:.3}" fill="none" stroke="{MUTED}" stroke-width="0.015"/>"#,
                    leaf.x,
                    leaf.y,
                    shut.x,
                    shut.y,
                    w = o.width
                );
            }
        }
    }
    if labels {
        for room in level.rooms.values() {
            let poly = level.room_points(room);
            if poly.len() < 3 {
                continue;
            }
            let at = geometry::label_point(&poly);
            svg.label(at, 0.3, INK, true, &[room.display_name(), format_area(geometry::area(&poly), units)]);
        }
    }
}

fn objects(svg: &mut Svg, level: &Level, on_roof: bool) {
    for o in level.objects.values() {
        if o.roof.is_some() != on_roof {
            continue;
        }
        let outline = o.world_outline();
        let color = o.display_color();
        svg.polygon(&outline, &tint(color, 0.55), &hex(color), 0.03);
        if let Some(st) = &o.stairs {
            let steps = st.steps.unwrap_or_else(|| ((o.shape.depth / crate::stairs::STEP_RUN).round() as u32).max(2));
            for step in o.step_outlines(steps) {
                svg.polygon(&step, "none", &hex(color), 0.015);
            }
        }
        if !outline.is_empty() {
            let c = geometry::centroid(&outline);
            let size = (o.shape.width.min(o.shape.depth) * 0.3).clamp(0.12, 0.22);
            svg.label(c, size, INK, false, &[o.display_name()]);
        }
    }
}

fn feature(svg: &mut Svg, f: &Feature, labels: bool) {
    let color = f.display_color();
    match f.kind.geom() {
        FeatureGeom::Area if f.kind == FeatureKind::PropertyLine => {
            let mut ring = f.points.clone();
            if let Some(first) = ring.first().copied() {
                ring.push(first);
            }
            svg.polyline(&ring, "#8a6d3b", 0.08, Some("0.5 0.25"));
        }
        FeatureGeom::Area => {
            svg.polygon(&f.points, &tint(color, 0.35), &hex(color), 0.04);
            if labels && f.points.len() >= 3 {
                svg.label(geometry::label_point(&f.points), 0.35, INK, true, &[f.display_name()]);
            }
        }
        FeatureGeom::Line => {
            svg.polyline(&f.points, &hex(color), f.line_width().max(0.06), None);
        }
        FeatureGeom::Point => {
            if let Some(c) = f.points.first() {
                svg.circle(*c, (f.size / 2.0).max(0.3), &tint(color, 0.4), &hex(color), 0.04);
                if labels {
                    svg.label(*c, 0.28, INK, false, &[f.display_name()]);
                }
            }
        }
    }
}

/// A floor: rooms, walls, doors, windows and items.
pub fn floor(level: &Level, title: &str, units: UnitSystem) -> Option<String> {
    let mut svg = Svg::new();
    rooms_and_walls(&mut svg, level, units, true);
    objects(&mut svg, level, false);
    svg.finish(title)
}

/// The yard: outside areas, fences, trees, and the house and outbuildings
/// as grey footprints.
pub fn site(project: &Project, title: &str, units: UnitSystem) -> Option<String> {
    let site = project.site()?;
    let mut svg = Svg::new();
    let mut footprints: Vec<(&Level, String)> = project.ground_level(None).map(|l| (l, String::new())).into_iter().collect();
    for b in &project.buildings {
        if let Some(l) = project.ground_level(Some(b.id)) {
            footprints.push((l, b.display_name()));
        }
    }
    // Areas first so lines and trees sit on top of them.
    let mut feats: Vec<&Feature> = site.features.values().collect();
    feats.sort_by_key(|f| match f.kind.geom() {
        FeatureGeom::Area => 0,
        FeatureGeom::Line => 1,
        FeatureGeom::Point => 2,
    });
    for f in feats.iter().filter(|f| f.kind.geom() == FeatureGeom::Area) {
        feature(&mut svg, f, false);
    }
    for (l, name) in &footprints {
        for room in l.rooms.values() {
            svg.polygon(&l.room_points(room), "#d9d6d0", "#9a968f", 0.05);
        }
        if !name.is_empty() {
            let pts: Vec<Vec2> = l.nodes.values().map(|n| n.pos).collect();
            if let Some((a, b)) = geometry::bounds(&pts) {
                svg.label(a.lerp(b, 0.5), 0.35, INK, true, std::slice::from_ref(name));
            }
        }
    }
    if let Some((a, b)) = project.ground_level(None).and_then(|l| l.bounds()) {
        let name = if project.info.name.trim().is_empty() { "House".to_string() } else { project.info.name.clone() };
        svg.label(a.lerp(b, 0.5), 0.45, INK, true, &[name]);
    }
    for f in feats.iter().filter(|f| f.kind.geom() != FeatureGeom::Area) {
        feature(&mut svg, f, true);
    }
    objects(&mut svg, site, false);
    // Area names last, so nothing covers them, along the top of each area
    // so they don't sit on the things inside it.
    for f in feats.iter().filter(|f| f.kind.geom() == FeatureGeom::Area && f.kind != FeatureKind::PropertyLine) {
        if let (true, Some((min, max))) = (f.points.len() >= 3, geometry::bounds(&f.points)) {
            let x = geometry::label_point(&f.points).x;
            let y = if max.y - min.y > 1.4 { min.y + 0.45 } else { (min.y + max.y) / 2.0 };
            svg.label(Vec2::new(x, y), 0.35, INK, true, &[f.display_name()]);
        }
    }
    let _ = units;
    svg.finish(title)
}

/// Roof sections over the floor plan they sit on.
pub fn roof(level: &Level, title: &str, units: UnitSystem) -> Option<String> {
    if level.roofs.is_empty() {
        return None;
    }
    let mut svg = Svg::new();
    rooms_and_walls(&mut svg, level, units, false);
    for r in level.roofs.values() {
        let color = r.top_layer().and_then(|l| l.swatch).unwrap_or([150, 120, 100]);
        svg.polygon(&r.footprint(), &tint(color, 0.45), &hex(color), 0.05);
        for (a, b) in r.ridge_lines() {
            svg.polyline(&[a, b], &hex(color), 0.04, None);
        }
    }
    objects(&mut svg, level, true);
    for r in level.roofs.values() {
        let fp = r.footprint();
        if fp.len() >= 3 {
            svg.label(geometry::label_point(&fp), 0.35, INK, true, &[r.display_name()]);
        }
    }
    svg.finish(title)
}

/// A garden bed and what's planted in it in one year.
pub fn garden(bed: &Feature, plantings: &[&Planting], title: &str) -> Option<String> {
    let mut svg = Svg::new();
    svg.polygon(&bed.points, &tint([150, 118, 88], 0.6), "#96765a", 0.05);
    for p in plantings {
        let color = p.color.unwrap_or([96, 150, 80]);
        match p.layout {
            Layout::Row => svg.polyline(&p.points, &tint(color, 0.2), (p.spacing_m() * 0.8).clamp(0.08, 0.6), None),
            Layout::Patch => svg.polygon(&p.points, &tint(color, 0.45), &hex(color), 0.03),
            Layout::Plant => {
                if let Some(c) = p.points.first() {
                    svg.circle(*c, (p.size / 2.0).max(0.1), &tint(color, 0.35), &hex(color), 0.02);
                }
            }
        }
    }
    for p in plantings {
        let at = match p.layout {
            Layout::Patch if p.points.len() >= 3 => geometry::label_point(&p.points),
            _ if !p.points.is_empty() => {
                let (a, b) = geometry::bounds(&p.points).unwrap_or((p.points[0], p.points[0]));
                a.lerp(b, 0.5)
            }
            _ => continue,
        };
        svg.label(at, 0.16, INK, false, &[p.display_name()]);
    }
    svg.finish(title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_the_sample() {
        let p = crate::sample::sample_house();
        let l = &p.levels[p.levels.iter().position(|l| !l.is_site()).unwrap()];
        let s = floor(l, "Main floor", p.info.units).unwrap();
        assert!(s.starts_with("<svg") && s.contains("Kitchen") && s.trim_end().ends_with("</svg>"));
        assert!(site(&p, "Outside", p.info.units).unwrap().contains("<polygon"));
    }

    #[test]
    fn nothing_to_draw() {
        assert!(floor(&Level::new("Empty", 0.0), "Empty", UnitSystem::Metric).is_none());
    }
}

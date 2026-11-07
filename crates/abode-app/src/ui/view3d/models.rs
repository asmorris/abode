//! Low-poly models for the 3D view: trees, vehicles, decks and fences.
//! Everything here is built from a few solids (tapered prisms, lumpy
//! spheres, cylinders) so it stays cheap to draw with the 2D painter.

use super::{Face, Scene, V3, add, cut_polyline, norm, p3, rgb, rgba, scale, shade, sub};
use abode_core::geometry::*;
use abode_core::*;
use eframe::egui::{self, Color32, Stroke};
use std::f64::consts::TAU;
use std::hash::{Hash, Hasher};

// ---------------------------------------------------------------------------
// Small helpers

/// A repeatable random sequence, so each tree keeps its shape between frames.
pub(super) struct Rng(u64);

impl Rng {
    pub fn new(seed: impl Hash) -> Self {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        seed.hash(&mut h);
        Rng(h.finish() | 1)
    }

    /// 0..1
    pub fn f(&mut self) -> f64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f()
    }
}

/// An item's own frame: local (x across, y along its depth) → world plan.
struct Frame {
    pos: Vec2,
    rot: f64,
}

impl Frame {
    fn of(obj: &PlacedObject) -> Self {
        Frame { pos: obj.position, rot: obj.rotation }
    }
    fn at(&self, x: f64, y: f64) -> Vec2 {
        self.pos + Vec2::new(x, y).rotated(self.rot)
    }
    fn rect(&self, x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<Vec2> {
        vec![self.at(x0, y0), self.at(x1, y0), self.at(x1, y1), self.at(x0, y1)]
    }
    /// A local direction as a 3D vector.
    fn dir(&self, x: f64, y: f64, z: f64) -> V3 {
        let d = Vec2::new(x, y).rotated(self.rot);
        [d.x as f32, z as f32, d.y as f32]
    }
}

fn ring(center: Vec2, r: f64, sides: usize, start: f64) -> Vec<Vec2> {
    (0..sides).map(|i| center + Vec2::new(1.0, 0.0).rotated(start + i as f64 / sides as f64 * TAU) * r).collect()
}

fn luma(c: Color32) -> f32 {
    0.3 * c.r() as f32 + 0.59 * c.g() as f32 + 0.11 * c.b() as f32
}

/// Very dark colours read as holes in the scene; lift them towards `to`.
pub(super) fn lift_dark(c: Color32, to: Color32) -> Color32 {
    let l = luma(c);
    if l >= 48.0 {
        return c;
    }
    crate::theme::mix(c, to, (48.0 - l) / 48.0 * 0.85)
}

/// Push a soft shadow (two layers: a wide faint one and the core).
pub(super) fn shadow(scene: &Scene, shadows: &mut Vec<egui::Mesh>, outline: &[Vec2], spread: f64, alpha: u8) {
    if outline.len() < 3 {
        return;
    }
    let c = outline.iter().fold(Vec2::ZERO, |a, p| a + *p) * (1.0 / outline.len() as f64);
    let grow = |d: f64| -> Vec<Vec2> {
        outline
            .iter()
            .map(|p| {
                let v = *p - c;
                let l = v.length();
                if l < 1e-9 { *p } else { *p + v * (d / l) }
            })
            .collect()
    };
    for (pts, a) in [(grow(spread), alpha / 3), (grow(spread * 0.5), alpha / 3), (grow(0.0), alpha / 3)] {
        if let Some((_, m)) = scene.flat(&pts, 0.025, Color32::from_black_alpha(a)) {
            shadows.push(m);
        }
    }
}

// ---------------------------------------------------------------------------
// Trees

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum TreeForm {
    /// Maples, oaks, fruit trees: a broad, rounded crown.
    Round,
    /// Spruce, pine, fir, cedar: stacked cones.
    Conifer,
    /// Birch, poplar, aspen: tall and narrow with a pale trunk.
    Columnar,
    /// Bushes and shrubs: no trunk, sitting on the ground.
    Shrub,
}

pub(super) fn tree_form(f: &Feature) -> TreeForm {
    let text = format!("{} {} {}", f.name, f.material, f.kind_name).to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| text.contains(w));
    if has(&["bush", "shrub", "hydrangea", "lilac", "boxwood", "rose", "forsythia", "spirea", "azalea", "rhododendron"]) {
        TreeForm::Shrub
    } else if has(&[
        "spruce", "pine", "fir", "cedar", "cypress", "hemlock", "juniper", "yew", "larch", "thuja", "arborvitae", "sequoia", "redwood",
        "conifer", "evergreen", "christmas",
    ]) {
        TreeForm::Conifer
    } else if has(&["birch", "poplar", "aspen", "columnar"]) {
        TreeForm::Columnar
    } else if f.height < 2.2 {
        TreeForm::Shrub
    } else {
        TreeForm::Round
    }
}

/// A lumpy, slightly irregular ellipsoid: one clump of leaves.
#[allow(clippy::too_many_arguments)]
fn blob(scene: &Scene, out: &mut Vec<Face>, center: Vec2, cy: f64, r: f64, vr: f64, color: Color32, rng: &mut Rng) {
    let (lon, lat) = (9usize, 6usize);
    let start = rng.range(0.0, TAU);
    let jitter: Vec<f64> = (0..lon * (lat + 1)).map(|_| rng.range(0.9, 1.08)).collect();
    let point = |i: usize, j: usize| {
        let a = start + i as f64 / lon as f64 * TAU;
        let b = -std::f64::consts::FRAC_PI_2 + j as f64 / lat as f64 * std::f64::consts::PI;
        let k = jitter[(i % lon) * (lat + 1) + j];
        let rr = r * b.cos() * k;
        // Never below the ground: bushes sit flat on it.
        let h = (cy + vr * b.sin() * k).max(0.02);
        p3(center + Vec2::new(a.cos() * rr, a.sin() * rr), h)
    };
    let mid = p3(center, cy);
    // Flat caps instead of pointed poles: needle-sharp triangles spike under anti-aliasing.
    for (j, up) in [(1, false), (lat - 1, true)] {
        let mut cap: Vec<V3> = (0..lon).map(|i| point(i, j)).collect();
        if !up {
            cap.reverse();
        }
        scene.face(out, &cap, [0.0, if up { 1.0 } else { -1.0 }, 0.0], color, Stroke::NONE);
    }
    for i in 0..lon {
        for j in 1..lat - 1 {
            let quad = [point(i, j), point(i + 1, j), point(i + 1, j + 1), point(i, j + 1)];
            let c = scale(add(add(quad[0], quad[1]), add(quad[2], quad[3])), 0.25);
            let d = sub(c, mid);
            // Normal of the ellipsoid there, not of the lumpy quad: smoother shading.
            let n = norm([d[0] / r as f32, d[1] / vr as f32, d[2] / r as f32]);
            scene.face(out, &quad, n, color, Stroke::NONE);
        }
    }
}

/// A tapered, many-sided column (trunks, wheels' cousins).
#[allow(clippy::too_many_arguments)]
fn column(scene: &Scene, out: &mut Vec<Face>, center: Vec2, r0: f64, r1: f64, lo: f64, hi: f64, sides: usize, color: Color32) {
    let start = 0.3;
    scene.solid(out, &ring(center, r0, sides, start), &ring(center, r1, sides, start), lo, hi, color, color);
}

pub(super) fn tree(scene: &Scene, out: &mut Vec<Face>, shadows: &mut Vec<egui::Mesh>, f: &Feature, selected: bool) {
    let Some(center) = f.points.first().copied() else { return };
    let form = tree_form(f);
    let mut rng = Rng::new(f.id);
    let height = f.height.max(0.6);
    let radius = (f.size / 2.0).max(0.3);

    // Colour: the tree's own if it has one, else a shade that suits the
    // kind of tree, nudged a little so a row of trees isn't uniform.
    let base = match (selected, f.color, form) {
        (true, ..) => crate::theme::c::teal(),
        (false, Some(c), _) => rgb(c),
        (false, None, TreeForm::Conifer) if f.name.to_lowercase().contains("blue") => Color32::from_rgb(84, 122, 118),
        (false, None, TreeForm::Conifer) => Color32::from_rgb(64, 112, 84),
        (false, None, TreeForm::Columnar) => Color32::from_rgb(132, 168, 88),
        (false, None, TreeForm::Shrub) => Color32::from_rgb(96, 142, 82),
        (false, None, TreeForm::Round) => Color32::from_rgb(106, 152, 88),
    };
    let leaves = if selected {
        base
    } else {
        let warm = rng.range(0.0, 0.14) as f32;
        shade(crate::theme::mix(base, Color32::from_rgb(160, 162, 84), warm), rng.range(0.92, 1.06) as f32)
    };
    let bark = Color32::from_rgb(112, 86, 62);
    let lean = Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)) * (radius * 0.08);

    match form {
        TreeForm::Round => {
            let vr = (height * 0.36).min(radius * 1.0);
            let cy = height - vr;
            let trunk_r = (radius * 0.1).clamp(0.07, 0.28);
            column(scene, out, center, trunk_r * 1.25, trunk_r * 0.8, 0.0, cy, 7, bark);
            let top = center + lean;
            if radius > 1.1 {
                // Big crowns: a few overlapping clumps, so the outline isn't a ball.
                let a0 = rng.range(0.0, TAU);
                for k in 0..3 {
                    let a = a0 + k as f64 * TAU / 3.0 + rng.range(-0.4, 0.4);
                    let off = Vec2::new(a.cos(), a.sin()) * (radius * 0.42);
                    let h = cy - vr * rng.range(0.05, 0.25);
                    blob(scene, out, top + off, h, radius * 0.58, vr * 0.72, shade(leaves, rng.range(0.94, 1.04) as f32), &mut rng);
                }
                blob(scene, out, top, cy + vr * 0.12, radius * 0.72, vr * 0.82, leaves, &mut rng);
            } else {
                blob(scene, out, top, cy, radius, vr, leaves, &mut rng);
            }
            shadow(scene, shadows, &ring(center + scene.sun_offset(cy * 0.5), radius * 0.95, 20, 0.0), 0.35, 46);
        }
        TreeForm::Columnar => {
            // An open oval crown from about a third of the way up.
            let vr = (height * 0.36).max(radius * 1.1);
            let cy = height - vr;
            let trunk_r = (radius * 0.09).clamp(0.06, 0.2);
            let birch = Color32::from_rgb(226, 222, 212);
            column(scene, out, center, trunk_r * 1.15, trunk_r * 0.8, 0.0, cy + vr * 0.3, 7, birch);
            let top = center + lean;
            let side = Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalized() * (radius * 0.28);
            blob(scene, out, top - side, cy - vr * 0.22, radius * 0.72, vr * 0.62, shade(leaves, 0.96), &mut rng);
            blob(scene, out, top + side, cy + vr * 0.12, radius * 0.78, vr * 0.72, leaves, &mut rng);
            blob(scene, out, top + side * 0.3, cy + vr * 0.5, radius * 0.5, vr * 0.45, shade(leaves, 1.05), &mut rng);
            shadow(scene, shadows, &ring(center + scene.sun_offset(cy * 0.5), radius * 0.8, 20, 0.0), 0.3, 42);
        }
        TreeForm::Conifer => {
            let trunk_r = (radius * 0.1).clamp(0.06, 0.22);
            let skirt = height * 0.14;
            column(scene, out, center, trunk_r * 1.2, trunk_r, 0.0, skirt + 0.2, 6, bark);
            let tiers = if height > 6.0 { 4 } else { 3 };
            let span = height - skirt;
            let th = span / tiers as f64 * 1.6;
            for k in 0..tiers {
                let t = k as f64 / (tiers - 1) as f64;
                let lo = skirt + t * (span - th);
                let rb = radius * (1.0 - t * 0.55);
                let rt = rb * 0.12;
                // Alternate the shade a little so each tier reads separately.
                let col = shade(leaves, if k % 2 == 0 { 0.94 } else { 1.04 } + 0.04 * t as f32);
                let c = center + lean * t;
                let start = rng.range(0.0, TAU);
                scene.solid(out, &ring(c, rb, 8, start), &ring(c, rt, 8, start), lo, lo + th, col, col);
            }
            shadow(scene, shadows, &ring(center + scene.sun_offset(height * 0.25), radius * 0.85, 20, 0.0), 0.3, 46);
        }
        TreeForm::Shrub => {
            let vr = height * 0.5;
            blob(scene, out, center, vr * 0.95, radius, vr, leaves, &mut rng);
            if radius > 0.45 {
                let a0 = rng.range(0.0, TAU);
                for k in 0..2 {
                    let a = a0 + k as f64 * 2.4;
                    let off = Vec2::new(a.cos(), a.sin()) * (radius * 0.45);
                    let col = shade(leaves, rng.range(0.92, 1.06) as f32);
                    blob(scene, out, center + off, vr * 0.7, radius * 0.62, vr * 0.72, col, &mut rng);
                }
            }
            shadow(scene, shadows, &ring(center + scene.sun_offset(vr * 0.4), radius * 1.02, 20, 0.0), 0.2, 50);
        }
    }
}

// ---------------------------------------------------------------------------
// Vehicles

#[derive(Clone, Copy)]
struct CarSpec {
    /// Ground clearance, beltline (top of the body) and roof height (m).
    clear: f64,
    belt: f64,
    roof: f64,
    /// Where the cabin starts and ends, as a fraction of the length from the front.
    cab: (f64, f64),
    /// How far the windscreen and rear window lean in (m).
    rake: (f64, f64),
    wheel: f64,
    /// A pickup's open bed behind the cabin.
    bed: bool,
}

#[derive(Clone, Copy)]
enum VehicleShape {
    Car(CarSpec),
    Motorcycle,
    Trailer,
    Boat,
}

fn vehicle_shape(obj: &PlacedObject) -> VehicleShape {
    let body = obj.vehicle.as_ref().map(|v| v.body.clone()).filter(|b| !b.trim().is_empty()).unwrap_or_else(|| obj.label.clone()).to_lowercase();
    let has = |w: &str| body.contains(w);
    let spec = |clear, belt, roof, cab, rake, wheel| CarSpec { clear, belt, roof, cab, rake, wheel, bed: false };
    let (w, d) = (obj.shape.width, obj.shape.depth);
    if has("motorcycle") || has("motorbike") || has("scooter") || has("bike") {
        VehicleShape::Motorcycle
    } else if has("boat") || has("canoe") || has("kayak") {
        VehicleShape::Boat
    } else if has("trailer") {
        VehicleShape::Trailer
    } else if has("pickup") || has("truck") {
        VehicleShape::Car(CarSpec { bed: true, ..spec(0.26, 1.0, 1.85, (0.24, 0.56), (0.42, 0.05), 0.4) })
    } else if has("rv") || has("camper") || has("motorhome") {
        VehicleShape::Car(spec(0.3, 1.25, 3.0, (0.04, 0.99), (0.5, 0.0), 0.45))
    } else if has("minivan") {
        VehicleShape::Car(spec(0.18, 0.9, 1.76, (0.2, 0.97), (0.62, 0.1), 0.34))
    } else if has("van") {
        VehicleShape::Car(spec(0.2, 1.0, 2.1, (0.12, 0.99), (0.38, 0.02), 0.36))
    } else if has("suv") || has("crossover") || has("jeep") {
        VehicleShape::Car(spec(0.22, 0.92, 1.66, (0.27, 0.95), (0.5, 0.14), 0.36))
    } else if has("hatch") {
        VehicleShape::Car(spec(0.16, 0.8, 1.46, (0.3, 0.93), (0.6, 0.22), 0.32))
    } else if has("wagon") || has("estate") {
        VehicleShape::Car(spec(0.16, 0.8, 1.46, (0.3, 0.95), (0.55, 0.12), 0.32))
    } else if has("coupe") || has("convertible") || has("sports") {
        VehicleShape::Car(spec(0.13, 0.74, 1.3, (0.34, 0.74), (0.62, 0.5), 0.32))
    } else if has("sedan") || has("car") {
        VehicleShape::Car(spec(0.16, 0.78, 1.44, (0.3, 0.78), (0.58, 0.45), 0.32))
    } else if d < 3.0 && w < 1.1 {
        VehicleShape::Motorcycle
    } else if d < 3.0 {
        VehicleShape::Trailer
    } else {
        VehicleShape::Car(spec(0.2, 0.88, 1.6, (0.28, 0.94), (0.52, 0.16), 0.35))
    }
}

/// The car's paint: its own colour, else the colour named in its details.
pub(super) fn vehicle_color(obj: &PlacedObject) -> Color32 {
    let named = obj.vehicle.as_ref().map(|v| v.color.to_lowercase()).and_then(|c| {
        const NAMES: &[(&str, [u8; 3])] = &[
            ("white", [238, 238, 234]),
            ("pearl", [236, 234, 226]),
            ("black", [30, 31, 34]),
            ("silver", [192, 196, 200]),
            ("grey", [128, 132, 136]),
            ("gray", [128, 132, 136]),
            ("red", [176, 44, 40]),
            ("blue", [52, 86, 150]),
            ("green", [56, 102, 70]),
            ("yellow", [230, 192, 60]),
            ("orange", [222, 120, 44]),
            ("brown", [112, 80, 56]),
            ("beige", [204, 190, 160]),
            ("gold", [196, 164, 96]),
        ];
        NAMES.iter().find(|(n, _)| c.contains(n)).map(|(_, rgb)| *rgb)
    });
    let own = obj.color.filter(|c| *c != obj.category.color());
    rgb(own.or(named).unwrap_or(obj.category.color()))
}

/// A wheel: an eight-sided cylinder lying across the car.
#[allow(clippy::too_many_arguments)]
fn wheel(scene: &Scene, out: &mut Vec<Face>, fr: &Frame, x0: f64, x1: f64, y: f64, base: f64, r: f64) {
    let tyre = Color32::from_rgb(44, 45, 48);
    let hub = Color32::from_rgb(168, 170, 172);
    let n = 8;
    let pt = |x: f64, a: f64, rr: f64| p3(fr.at(x, y + a.cos() * rr), base + r + a.sin() * rr);
    let ang = |i: usize| (i as f64 + 0.5) / n as f64 * TAU;
    for i in 0..n {
        let (a, b) = (ang(i), ang(i + 1));
        let quad = [pt(x0, a, r), pt(x1, a, r), pt(x1, b, r), pt(x0, b, r)];
        let m = (a + b) / 2.0;
        scene.face(out, &quad, norm(fr.dir(0.0, m.cos(), m.sin())), tyre, Stroke::NONE);
    }
    // Both sidewalls, and a hubcap on whichever is facing out.
    for (x, s) in [(x0, -1.0), (x1, 1.0)] {
        let mut wall: Vec<V3> = (0..n).map(|i| pt(x, ang(i), r)).collect();
        if s < 0.0 {
            wall.reverse();
        }
        let nrm = norm(fr.dir(s, 0.0, 0.0));
        scene.face(out, &wall, nrm, tyre, Stroke::NONE);
        let xo = x + s * 0.004;
        let cap: Vec<V3> = (0..n).map(|i| pt(xo, ang(i), r * 0.55)).collect();
        scene.face(out, &cap, nrm, hub, Stroke::NONE);
    }
}

pub(super) fn vehicle(scene: &Scene, out: &mut Vec<Face>, shadows: &mut Vec<egui::Mesh>, obj: &PlacedObject, base: f64, selected: bool) {
    let fr = Frame::of(obj);
    let (w, d) = (obj.shape.width.max(0.4), obj.shape.depth.max(0.8));
    let (hw, hd) = (w / 2.0, d / 2.0);
    let paint = if selected {
        crate::theme::c::teal()
    } else {
        lift_dark(vehicle_color(obj), Color32::from_rgb(62, 64, 70))
    };
    let glass = Color32::from_rgb(66, 82, 96);
    let on_ground = base < 0.1;

    let shape = vehicle_shape(obj);
    match shape {
        VehicleShape::Car(s) => {
            // Rounded corners: chamfer the footprint so it isn't a shoebox.
            let (cf, cb) = ((w * 0.16).min(0.3), (w * 0.1).min(0.2));
            let body = vec![
                fr.at(-hw + cf, -hd),
                fr.at(hw - cf, -hd),
                fr.at(hw, -hd + cf),
                fr.at(hw, hd - cb),
                fr.at(hw - cb, hd),
                fr.at(-hw + cb, hd),
                fr.at(-hw, hd - cb),
                fr.at(-hw, -hd + cf),
            ];
            let (lo, belt, roof) = (base + s.clear, base + s.belt, base + s.roof);
            scene.solid(out, &body, &body, lo, belt, paint, shade(paint, 1.04));

            // The cabin: glass sides, leaning windscreen, painted roof.
            let (c0, c1) = (-hd + s.cab.0 * d, -hd + s.cab.1 * d);
            let inset = (w * 0.09).min(0.18);
            let bottom = fr.rect(-hw + 0.05, hw - 0.05, c0, c1);
            let top = fr.rect(-hw + inset, hw - inset, c0 + s.rake.0, c1 - s.rake.1);
            scene.solid(out, &bottom, &top, belt, roof, glass, shade(paint, 1.02));

            if s.bed {
                // An open bed behind the cab: a darker inset on the body top.
                let bed = fr.rect(-hw + 0.09, hw - 0.09, c1 + 0.06, hd - 0.08);
                let pts: Vec<V3> = bed.iter().map(|p| p3(*p, belt + 0.006)).collect();
                scene.face(out, &pts, [0.0, 1.0, 0.0], shade(paint, 0.55), Stroke::NONE);
            }

            // Lights.
            let lamp = |x0: f64, x1: f64, y: f64, ny: f64, col: Color32, out: &mut Vec<Face>| {
                let (z0, z1) = (belt - 0.17, belt - 0.06);
                let q = [p3(fr.at(x0, y), z0), p3(fr.at(x1, y), z0), p3(fr.at(x1, y), z1), p3(fr.at(x0, y), z1)];
                scene.face(out, &q, norm(fr.dir(0.0, ny, 0.0)), col, Stroke::NONE);
            };
            let head = Color32::from_rgb(250, 246, 226);
            let tail = Color32::from_rgb(186, 48, 44);
            let (fx, bx) = (hw - cf, hw - cb);
            lamp(-fx + 0.04, -fx + 0.3, -hd - 0.004, -1.0, head, out);
            lamp(fx - 0.3, fx - 0.04, -hd - 0.004, -1.0, head, out);
            lamp(-bx + 0.04, -bx + 0.26, hd + 0.004, 1.0, tail, out);
            lamp(bx - 0.26, bx - 0.04, hd + 0.004, 1.0, tail, out);

            let r = s.wheel.min(d * 0.09).max(0.2);
            for y in [-hd + d * 0.19, hd - d * 0.2] {
                for (x0, x1) in [(-hw + 0.0, -hw + 0.22), (hw - 0.22, hw - 0.0)] {
                    wheel(scene, out, &fr, x0, x1, y, base, r);
                }
            }
            if on_ground {
                shadow(scene, shadows, &body, 0.22, 60);
            }
        }
        VehicleShape::Motorcycle => {
            let r = 0.3_f64.min(d * 0.16);
            let (y0, y1) = (-hd + r + 0.05, hd - r - 0.05);
            let frame = fr.rect(-0.16, 0.16, y0 + 0.15, y1 - 0.1);
            scene.solid(out, &frame, &frame, base + r * 0.9, base + r * 2.1, paint, shade(paint, 1.04));
            let seat = fr.rect(-0.15, 0.15, (y0 + y1) / 2.0 - 0.05, y1 - 0.15);
            scene.solid(out, &seat, &seat, base + r * 2.1, base + r * 2.1 + 0.1, Color32::from_rgb(40, 40, 42), Color32::from_rgb(52, 52, 54));
            let bar = fr.rect(-0.36, 0.36, y0 + 0.12, y0 + 0.18);
            scene.solid(out, &bar, &bar, base + r * 2.6, base + r * 2.6 + 0.04, Color32::from_rgb(70, 70, 72), Color32::from_rgb(80, 80, 82));
            for y in [y0, y1] {
                wheel(scene, out, &fr, -0.07, 0.07, y, base, r);
            }
            if on_ground {
                shadow(scene, shadows, &fr.rect(-0.25, 0.25, -hd + 0.1, hd - 0.1), 0.15, 50);
            }
        }
        VehicleShape::Trailer | VehicleShape::Boat => {
            let boat = matches!(shape, VehicleShape::Boat);
            let r = 0.26;
            let metal = Color32::from_rgb(96, 98, 102);
            // Tongue to the hitch, then the deck.
            let tongue = fr.rect(-0.05, 0.05, -hd, -hd + d * 0.25);
            scene.solid(out, &tongue, &tongue, base + 0.38, base + 0.46, metal, metal);
            let deck_y0 = -hd + d * 0.2;
            let deck = fr.rect(-hw + 0.02, hw - 0.02, deck_y0, hd);
            let top = base + 0.62;
            scene.solid(out, &deck, &deck, base + 0.42, top, if boat { metal } else { paint }, shade(if boat { metal } else { paint }, 1.04));
            if boat {
                // A hull: pointed bow, narrower keel.
                let (by0, by1) = (-hd + d * 0.05, hd - 0.05);
                let hull_top = vec![fr.at(0.0, by0), fr.at(hw - 0.02, by0 + d * 0.3), fr.at(hw - 0.05, by1), fr.at(-hw + 0.05, by1), fr.at(-hw + 0.02, by0 + d * 0.3)];
                let hull_bottom = vec![fr.at(0.0, by0 + d * 0.22), fr.at(hw * 0.35, by0 + d * 0.4), fr.at(hw * 0.4, by1 - 0.05), fr.at(-hw * 0.4, by1 - 0.05), fr.at(-hw * 0.35, by0 + d * 0.4)];
                scene.solid(out, &hull_bottom, &hull_top, top, top + 0.75, paint, Color32::from_rgb(232, 230, 222));
            } else {
                // Low sides.
                let t = 0.05;
                for rect in [
                    fr.rect(-hw + 0.02, -hw + 0.02 + t, deck_y0, hd),
                    fr.rect(hw - 0.02 - t, hw - 0.02, deck_y0, hd),
                    fr.rect(-hw + 0.02, hw - 0.02, deck_y0, deck_y0 + t),
                    fr.rect(-hw + 0.02, hw - 0.02, hd - t, hd),
                ] {
                    scene.solid(out, &rect, &rect, top, top + 0.3, paint, shade(paint, 1.05));
                }
            }
            let y = deck_y0 + (hd - deck_y0) * 0.55;
            for (x0, x1) in [(-hw - 0.04, -hw + 0.14), (hw - 0.14, hw + 0.04)] {
                wheel(scene, out, &fr, x0, x1, y, base, r);
            }
            if on_ground {
                shadow(scene, shadows, &deck, 0.15, 50);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Decks and fences

/// Where stairs meet a deck's edge: (from, to) distances around its outline,
/// so the railing can stop there.
fn stair_gaps(outline: &[Vec2], stairs: &[Vec<Vec2>]) -> Vec<(f64, f64)> {
    let n = outline.len();
    let mut gaps = Vec::new();
    let mut run = 0.0;
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let len = a.distance(b);
        if len < 1e-6 {
            continue;
        }
        let dir = (b - a) * (1.0 / len);
        for s in stairs {
            let near: Vec<f64> = s
                .iter()
                .filter(|p| {
                    let v = **p - a;
                    (v.x * dir.y - v.y * dir.x).abs() < 0.35
                })
                .map(|p| (*p - a).dot(dir))
                .filter(|t| *t > -0.2 && *t < len + 0.2)
                .collect();
            if near.len() >= 2 {
                let lo = near.iter().cloned().fold(f64::MAX, f64::min).max(0.0);
                let hi = near.iter().cloned().fold(f64::MIN, f64::max).min(len);
                if hi - lo > 0.2 {
                    gaps.push((run + lo, run + hi));
                }
            }
        }
        run += len;
    }
    gaps
}

fn railing_color(material: &str, deck: Color32) -> Color32 {
    let m = material.to_lowercase();
    if m.contains("black") {
        Color32::from_rgb(46, 47, 50)
    } else if m.contains("white") || m.contains("vinyl") {
        Color32::from_rgb(238, 236, 230)
    } else if m.contains("wood") || m.contains("cedar") || m.contains("pine") {
        shade(deck, 0.92)
    } else if m.contains("alum") || m.contains("metal") || m.contains("steel") {
        Color32::from_rgb(124, 126, 130)
    } else {
        Color32::from_rgb(76, 76, 78)
    }
}

/// A square post centred on `p`.
fn post(scene: &Scene, out: &mut Vec<Face>, p: Vec2, s: f64, lo: f64, hi: f64, color: Color32) {
    let sq = [p + Vec2::new(-s, -s), p + Vec2::new(s, -s), p + Vec2::new(s, s), p + Vec2::new(-s, s)];
    scene.solid(out, &sq, &sq, lo, hi, color, shade(color, 1.05));
}

/// Points every `step` (or closer) along a polyline, ends included.
fn every(pts: &[Vec2], step: f64) -> Vec<Vec2> {
    let mut out = Vec::new();
    for w in pts.windows(2) {
        let n = (w[0].distance(w[1]) / step).ceil().max(1.0) as usize;
        for i in 0..n {
            out.push(w[0].lerp(w[1], i as f64 / n as f64));
        }
    }
    out.extend(pts.last().copied());
    out
}

pub(super) fn deck(scene: &Scene, out: &mut Vec<Face>, f: &Feature, col: Color32, stairs: &[&PlacedObject]) {
    let h = f.height.max(0.1);
    let n = f.points.len();
    if n < 3 {
        return;
    }
    // The boards and the rim joist; posts down to the ground if it's high.
    let rim = 0.22_f64.min(h);
    scene.solid(out, &f.points, &f.points, h - rim, h, shade(col, 0.9), col);
    if h - rim > 0.08 {
        let c = f.points.iter().fold(Vec2::ZERO, |a, p| a + *p) * (1.0 / n as f64);
        let mut ring_pts = f.points.clone();
        ring_pts.push(f.points[0]);
        let inset: Vec<Vec2> = every(&ring_pts, 1.8)
            .into_iter()
            .map(|p| {
                let v = c - p;
                let l = v.length();
                if l < 1e-9 { p } else { p + v * (0.15 / l) }
            })
            .collect();
        for p in inset {
            post(scene, out, p, 0.05, 0.0, h - rim, shade(col, 0.72));
        }
    }

    if f.railing {
        let rail = railing_color(&f.railing_material, col);
        let glass = f.railing_material.to_lowercase().contains("glass");
        let infill = if glass {
            rgba(196, 220, 228, 70)
        } else {
            rgba(rail.r(), rail.g(), rail.b(), 46)
        };
        let mut loop_pts = f.points.clone();
        loop_pts.push(f.points[0]);
        let outlines: Vec<Vec<Vec2>> = stairs.iter().map(|s| s.world_outline()).collect();
        let gaps = stair_gaps(&f.points, &outlines);
        let (top, cap) = (h + 0.92, 0.035);
        for piece in cut_polyline(&loop_pts, &gaps) {
            for s in piece.windows(2) {
                let (a, b) = (s[0], s[1]);
                if a.distance(b) < 0.05 {
                    continue;
                }
                scene.wall(out, a, b, h + 0.06, top - 0.04, infill, Stroke::NONE);
                let nrm = (b - a).normalized().perp() * cap;
                let bar = [a + nrm, b + nrm, b - nrm, a - nrm];
                scene.solid(out, &bar, &bar, top - 0.05, top, rail, shade(rail, 1.08));
            }
            for p in every(&piece, 1.5) {
                post(scene, out, p, 0.04, h, top, rail);
            }
        }
    }
}

pub(super) fn fence(scene: &Scene, out: &mut Vec<Face>, f: &Feature, col: Color32) {
    let cuts: Vec<(f64, f64)> = f.gates.iter().map(|g| (g.offset - g.width / 2.0, g.offset + g.width / 2.0)).collect();
    let post_col = shade(col, 0.82);
    for piece in cut_polyline(&f.points, &cuts) {
        // Panels between posts: short faces also sort better against trees.
        let posts = every(&piece, 2.4);
        for s in posts.windows(2) {
            scene.wall(out, s[0], s[1], 0.0, f.height, col, scene.edge);
        }
        for p in posts {
            post(scene, out, p, 0.055, 0.0, f.height + 0.06, post_col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(name: &str, material: &str, height: f64) -> Feature {
        let mut f = Feature::new(FeatureKind::Tree, vec![Vec2::ZERO]);
        f.name = name.into();
        f.material = material.into();
        f.height = height;
        f
    }

    #[test]
    fn trees_get_a_shape_from_their_name() {
        assert_eq!(tree_form(&tree("Blue spruce", "", 6.0)), TreeForm::Conifer);
        assert_eq!(tree_form(&tree("Big tall one", "Birch", 8.0)), TreeForm::Columnar);
        assert_eq!(tree_form(&tree("Front bush", "Unknown", 1.6)), TreeForm::Shrub);
        assert_eq!(tree_form(&tree("Sugar maple", "", 7.0)), TreeForm::Round);
        // Small and unnamed: a shrub.
        assert_eq!(tree_form(&tree("", "", 1.5)), TreeForm::Shrub);
    }

    #[test]
    fn random_is_repeatable() {
        let (mut a, mut b) = (Rng::new(42u32), Rng::new(42u32));
        for _ in 0..10 {
            let x = a.f();
            assert_eq!(x, b.f());
            assert!((0.0..1.0).contains(&x));
        }
    }

    #[test]
    fn railing_stops_at_stairs() {
        let outline = rect_points(Vec2::ZERO, 4.0, 2.0);
        // 1 m wide stairs coming off the long side at y = 0.
        let stairs = rect_points(Vec2::new(1.5, -1.0), 1.0, 1.0);
        let gaps = stair_gaps(&outline, &[stairs]);
        assert_eq!(gaps.len(), 1, "{gaps:?}");
        assert!((gaps[0].1 - gaps[0].0 - 1.0).abs() < 1e-6);
    }
}

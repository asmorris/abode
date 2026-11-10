//! Read-only 3D view: floors extruded into walls, roofs, and everything
//! outside (decks, fences, trees, outbuildings), with an orbit camera.
//!
//! Drawn with egui's 2D painter (projected polygons, painter's-algorithm
//! sorted per storey) so it needs no GPU pipeline. Good enough for a house
//! model; when the plumbing/electrical work needs real 3D, swap this for a
//! wgpu scene.

use super::{Env, Insets};
use crate::theme::{self, c, icon};
use abode_core::geometry::*;
use abode_core::*;
use eframe::egui::{self, Color32, PointerButton, Pos2, Sense, Shape as EShape, Stroke, Ui, vec2};
use std::collections::BTreeMap;

mod models;

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-6 { scale(a, 1.0 / l) } else { a }
}

/// Plan (x, y) at height h → 3D (x, h, y). Y is up.
fn p3(p: Vec2, h: f64) -> V3 {
    [p.x as f32, h as f32, p.y as f32]
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub struct View3dState {
    yaw: f32,
    pitch: f32,
    dist: f32,
    target: V3,
    fitted: bool,
    pub hide_upper_levels: bool,
    pub show_items: bool,
    pub show_roofs: bool,
    pub show_outside: bool,
    /// Debug screenshots: zoom factor and an optional point (plan x, y) to look at.
    debug_view: Option<(f32, Option<[f32; 2]>)>,
}

impl Default for View3dState {
    fn default() -> Self {
        Self {
            yaw: 0.7,
            pitch: 0.75,
            dist: 20.0,
            target: [0.0; 3],
            fitted: false,
            hide_upper_levels: false,
            show_items: true,
            show_roofs: true,
            show_outside: true,
            debug_view: None,
        }
    }
}

impl View3dState {
    /// Point the camera (debug screenshots).
    pub fn set_angle(&mut self, yaw: f32, pitch: f32) {
        self.yaw = yaw;
        self.pitch = pitch.clamp(0.08, 1.5);
    }

    /// Zoom in (debug screenshots), optionally looking at a plan point.
    pub fn set_zoom(&mut self, zoom: f32, at: Option<[f32; 2]>) {
        self.debug_view = Some((zoom, at));
    }
}

struct Cam {
    eye: V3,
    right: V3,
    up: V3,
    fwd: V3,
    f: f32,
    center: Pos2,
}

impl Cam {
    fn project(&self, p: V3) -> Option<(Pos2, f32)> {
        let d = sub(p, self.eye);
        let z = dot(d, self.fwd);
        if z < 0.1 {
            return None;
        }
        let x = dot(d, self.right);
        let y = dot(d, self.up);
        Some((self.center + vec2(x / z * self.f, -y / z * self.f), z))
    }

    fn project_all(&self, pts: &[V3]) -> Option<(Vec<Pos2>, f32)> {
        let mut out = Vec::with_capacity(pts.len());
        let mut depth = 0.0;
        for p in pts {
            let (s, z) = self.project(*p)?;
            out.push(s);
            depth += z;
        }
        Some((out, depth / pts.len() as f32))
    }
}

struct Face {
    pts: Vec<Pos2>,
    depth: f32,
    fill: Color32,
    stroke: Stroke,
}

fn shade(c: Color32, k: f32) -> Color32 {
    let f = |v: u8| (v as f32 * k).clamp(0.0, 255.0) as u8;
    // Channels are already premultiplied, so scaling them keeps the alpha right.
    Color32::from_rgba_premultiplied(f(c.r()), f(c.g()), f(c.b()), c.a())
}

/// A see-through colour. egui's own constructor premultiplies in linear
/// space, which comes out lighter than intended when painted in gamma space.
fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    let m = |v: u8| (v as u16 * a as u16 / 255) as u8;
    Color32::from_rgba_premultiplied(m(r), m(g), m(b), a)
}

/// One storey's worth of geometry: floors are drawn first, then the rest
/// sorted back to front.
#[derive(Default)]
struct Group {
    floors: Vec<(f32, egui::Mesh)>,
    faces: Vec<Face>,
}

struct Scene<'a> {
    cam: &'a Cam,
    light: V3,
    edge: Stroke,
}

impl Scene<'_> {
    fn lit(&self, pts: &[V3]) -> f32 {
        if pts.len() < 3 {
            return 1.0;
        }
        let n = norm(cross(sub(pts[1], pts[0]), sub(pts[2], pts[0])));
        0.8 + 0.2 * dot(n, self.light).abs()
    }

    /// A planar polygon. Convex ones become one face; others are triangulated.
    fn poly(&self, out: &mut Vec<Face>, pts: &[V3], color: Color32, stroke: Stroke) {
        if pts.len() < 3 {
            return;
        }
        let k = self.lit(pts);
        let plan: Vec<Vec2> = pts.iter().map(|p| Vec2::new(p[0] as f64, p[2] as f64)).collect();
        let vertical = area(&plan) < 1e-6;
        if vertical || is_convex(&plan) {
            if let Some((s, depth)) = self.cam.project_all(pts) {
                out.push(Face { pts: s, depth, fill: shade(color, k), stroke });
            }
            return;
        }
        for t in triangulate(&plan) {
            let tri = [pts[t[0]], pts[t[1]], pts[t[2]]];
            if let Some((s, depth)) = self.cam.project_all(&tri) {
                out.push(Face { pts: s, depth, fill: shade(color, k), stroke: Stroke::NONE });
            }
        }
    }

    /// A flat horizontal polygon as a mesh (for floors and ground cover).
    fn flat(&self, pts: &[Vec2], h: f64, color: Color32) -> Option<(f32, egui::Mesh)> {
        let (screen, depth) = self.cam.project_all(&pts.iter().map(|p| p3(*p, h)).collect::<Vec<_>>())?;
        let mut mesh = egui::Mesh::default();
        for s in &screen {
            mesh.colored_vertex(*s, color);
        }
        for t in triangulate(pts) {
            mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
        }
        Some((depth, mesh))
    }

    /// Vertical strip along a plan segment.
    #[allow(clippy::too_many_arguments)]
    fn wall(&self, out: &mut Vec<Face>, a: Vec2, b: Vec2, lo: f64, hi: f64, color: Color32, stroke: Stroke) {
        self.poly(out, &[p3(a, lo), p3(b, lo), p3(b, hi), p3(a, hi)], color, stroke);
    }

    /// A prism: vertical sides plus a top. Sides facing away are skipped
    /// (they'd be hidden anyway, and leaving them out avoids sorting glitches).
    fn prism(&self, out: &mut Vec<Face>, outline: &[Vec2], lo: f64, hi: f64, color: Color32) {
        let n = outline.len();
        if n < 3 {
            return;
        }
        let ccw = signed_area(outline) > 0.0;
        for i in 0..n {
            let (a, b) = (outline[i], outline[(i + 1) % n]);
            let d = b - a;
            let out_n = if ccw { Vec2::new(d.y, -d.x) } else { Vec2::new(-d.y, d.x) };
            let mid = a.lerp(b, 0.5);
            if out_n.dot(Vec2::new(self.cam.eye[0] as f64 - mid.x, self.cam.eye[2] as f64 - mid.y)) < 0.0 {
                continue;
            }
            self.wall(out, a, b, lo, hi, color, Stroke::NONE);
        }
        if (self.cam.eye[1] as f64) < hi {
            return;
        }
        let top: Vec<V3> = outline.iter().map(|p| p3(*p, hi)).collect();
        let before = out.len();
        self.poly(out, &top, shade(color, 1.08), self.edge);
        for f in &mut out[before..] {
            f.depth -= 0.01;
        }
    }

    /// One face of a closed model, with its outward normal: skipped when it
    /// faces away from the camera, and lit by the sun.
    fn face(&self, out: &mut Vec<Face>, pts: &[V3], n: V3, color: Color32, stroke: Stroke) {
        if pts.len() < 3 {
            return;
        }
        let c = scale(pts.iter().fold([0.0; 3], |a, p| add(a, *p)), 1.0 / pts.len() as f32);
        if dot(n, sub(self.cam.eye, c)) <= 0.0 {
            return;
        }
        let k = 0.76 + 0.3 * dot(n, self.light).max(0.0);
        if let Some((s, depth)) = self.cam.project_all(pts) {
            out.push(Face { pts: s, depth, fill: shade(color, k), stroke });
        }
    }

    /// A solid between two outlines with the same number of points: a
    /// prism when they match, tapered (a cabin, a cone) when the top is smaller.
    #[allow(clippy::too_many_arguments)]
    fn solid(&self, out: &mut Vec<Face>, bottom: &[Vec2], top: &[Vec2], lo: f64, hi: f64, side: Color32, cap: Color32) {
        let n = bottom.len();
        if n < 3 || top.len() != n {
            return;
        }
        let avg = |v: &[V3]| scale(v.iter().fold([0.0; 3], |a, p| add(a, *p)), 1.0 / v.len() as f32);
        let ring_b: Vec<V3> = bottom.iter().map(|p| p3(*p, lo)).collect();
        let ring_t: Vec<V3> = top.iter().map(|p| p3(*p, hi)).collect();
        let mid = scale(add(avg(&ring_b), avg(&ring_t)), 0.5);
        for i in 0..n {
            let j = (i + 1) % n;
            let q = [ring_b[i], ring_b[j], ring_t[j], ring_t[i]];
            let mut nn = norm(cross(sub(q[1], q[0]), sub(q[3], q[0])));
            if dot(nn, nn) < 0.5 {
                nn = norm(cross(sub(q[2], q[1]), sub(q[0], q[1])));
            }
            if dot(nn, sub(avg(&q), mid)) < 0.0 {
                nn = scale(nn, -1.0);
            }
            self.face(out, &q, nn, side, Stroke::NONE);
        }
        self.face(out, &ring_t, [0.0, 1.0, 0.0], cap, Stroke::NONE);
    }

    /// How far a shadow falls from something `h` up, on the ground.
    fn sun_offset(&self, h: f64) -> Vec2 {
        let l = self.light;
        Vec2::new(-(l[0] / l[1]) as f64 * h, -(l[2] / l[1]) as f64 * h)
    }
}

fn signed_area(pts: &[Vec2]) -> f64 {
    let n = pts.len();
    (0..n).map(|i| pts[i].x * pts[(i + 1) % n].y - pts[(i + 1) % n].x * pts[i].y).sum::<f64>() / 2.0
}

fn fit(st: &mut View3dState, project: &Project) {
    let mut pts: Vec<Vec2> = project
        .levels
        .iter()
        .filter(|l| !l.is_site())
        .flat_map(|l| l.nodes.values().map(|n| n.pos))
        .collect();
    if pts.is_empty() {
        if let Some(site) = project.site() {
            pts = site.extent_points();
        }
    }
    let top = project
        .levels
        .iter()
        .map(|l| l.elevation + l.wall_height)
        .fold(0.0, f64::max);
    if let Some((lo, hi)) = bounds(&pts) {
        let c = (lo + hi) / 2.0;
        st.target = [c.x as f32, (top / 3.0) as f32, c.y as f32];
        st.dist = ((hi - lo).length() as f32 * 1.4).max(8.0);
    } else {
        st.target = [0.0, 1.0, 0.0];
        st.dist = 20.0;
    }
    if let Some((zoom, at)) = st.debug_view {
        st.dist /= zoom.max(0.05);
        if let Some([x, y]) = at {
            st.target = [x, 0.8, y];
        }
    }
    st.fitted = true;
}

fn toolbar(ctx: &egui::Context, st: &mut View3dState, area: egui::Rect) {
    egui::Area::new(egui::Id::new("view3d-toolbar"))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::CENTER_TOP)
        .fixed_pos(egui::pos2(area.center().x, area.top() + theme::MARGIN))
        .show(ctx, |ui| {
            theme::pill().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if theme::icon_button(ui, icon::STACK, "Hide levels (and roofs) above the one you're viewing", st.hide_upper_levels).clicked() {
                        st.hide_upper_levels = !st.hide_upper_levels;
                    }
                    if theme::icon_button(ui, icon::TRIANGLE, "Show roofs", st.show_roofs).clicked() {
                        st.show_roofs = !st.show_roofs;
                    }
                    if theme::icon_button(ui, icon::TREE, "Show outside: decks, fences, trees", st.show_outside).clicked() {
                        st.show_outside = !st.show_outside;
                    }
                    if theme::icon_button(ui, icon::PACKAGE, "Show items", st.show_items).clicked() {
                        st.show_items = !st.show_items;
                    }
                    theme::vdivider(ui);
                    if theme::icon_button(ui, icon::CORNERS_OUT, "Reset view", false).clicked() {
                        st.fitted = false;
                    }
                    ui.label(egui::RichText::new("Drag to orbit · right-drag to pan · scroll to zoom").size(12.5).color(c::muted()));
                    ui.add_space(6.0);
                });
            });
        });
}

/// Stairs as a run of steps. "Up" stairs climb from this floor; "Down"
/// stairs come up to it from the floor below.
/// Outside, stairs always stand on the ground: "Down" ones just start at
/// the deck end.
fn stair_steps(scene: &Scene, out: &mut Vec<Face>, obj: &PlacedObject, info: &stairs::StairInfo, elev: f64, color: Color32, outside: bool) {
    let n = info.steps.max(1);
    let total = info.rise.unwrap_or(n as f64 * stairs::STEP_RISE);
    let rise = total / n as f64;
    let down = obj.stairs.as_ref().is_some_and(|s| s.direction == stairs::StairDirection::Down);
    let base = if down && !outside { elev - total } else { elev };
    for (i, step) in obj.step_outlines(n).iter().enumerate() {
        // Down stairs: you start at the top (at -y), so heights count down.
        let k = if down { n as usize - i } else { i + 1 };
        scene.prism(out, step, base, base + rise * k as f64, color);
    }
}

fn item_height(obj: &PlacedObject) -> f64 {
    match obj.category {
        ObjectCategory::Hvac | ObjectCategory::Appliance => 0.9,
        ObjectCategory::Electrical => 0.3,
        ObjectCategory::Furniture => 0.75,
        ObjectCategory::Roof => 0.4,
        ObjectCategory::Outdoor => 0.9,
        // Motorcycles and small trailers are lower than cars.
        ObjectCategory::Vehicle if obj.shape.depth.max(obj.shape.width) < 3.0 => 1.1,
        ObjectCategory::Vehicle => 1.55,
        _ => 0.6,
    }
}

pub fn view3d_ui(ui: &mut Ui, project: &Project, env: &mut Env, st: &mut View3dState, insets: Insets) {
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = resp.rect;
    let area = insets.apply(rect);
    if insets.overlays {
        toolbar(ui.ctx(), st, area);
    }
    painter.rect_filled(rect, 0.0, c::canvas());
    if !st.fitted {
        fit(st, project);
    }

    // Camera control
    if resp.dragged_by(PointerButton::Primary) {
        let d = resp.drag_delta();
        st.yaw -= d.x * 0.008;
        st.pitch = (st.pitch + d.y * 0.008).clamp(0.08, 1.5);
    }
    let eye_dir = [st.pitch.cos() * st.yaw.sin(), st.pitch.sin(), st.pitch.cos() * st.yaw.cos()];
    let fwd = norm(scale(eye_dir, -1.0));
    let right = norm(cross(fwd, [0.0, 1.0, 0.0]));
    let up = cross(right, fwd);
    if resp.dragged_by(PointerButton::Secondary) || resp.dragged_by(PointerButton::Middle) {
        let d = resp.drag_delta();
        let k = st.dist / rect.height().max(1.0);
        let ground_fwd = norm([fwd[0], 0.0, fwd[2]]);
        st.target = add(st.target, add(scale(right, -d.x * k), scale(ground_fwd, d.y * k)));
    }
    if resp.hovered() {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        st.dist = (st.dist / zoom * (-scroll * 0.002).exp()).clamp(1.0, 500.0);
    }
    let eye = add(st.target, scale(eye_dir, st.dist));
    let cam = Cam { eye, right, up, fwd, f: rect.height() * 1.1, center: area.center() };
    let scene = Scene { cam: &cam, light: norm([0.4, 0.9, 0.3]), edge: Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(36, 42, 43, 70)) };
    let sel = *env.selection;

    // Which floors to show. "Hide upper levels" applies to the building you're viewing.
    let active = project.level(*env.active_level);
    let cutoff = active.filter(|l| !l.is_site()).map(|l| (l.building, l.elevation));
    let visible = |l: &Level| match (st.hide_upper_levels, cutoff) {
        (true, Some((b, e))) if l.building == b => l.elevation <= e + 1e-6,
        _ => true,
    };
    let roof_visible = |l: &Level| match (st.hide_upper_levels, cutoff) {
        (true, Some((b, e))) if l.building == b => l.elevation < e - 1e-6,
        _ => true,
    };

    let wall_base = Color32::from_rgb(250, 247, 240);
    let exterior = Color32::from_rgb(242, 237, 227);
    let floor = Color32::from_rgb(236, 228, 214);
    let trim = Color32::from_rgb(246, 244, 238);
    let room_paint = |r: RoomId| {
        project.paints.iter().find_map(|p| match (p.target, p.surface, p.swatch) {
            (PaintTarget::Room(x), PaintSurface::Walls, Some(c)) if x == r => Some(c),
            _ => None,
        })
    };
    let face_paint = |w: WallId, r: Option<RoomId>| {
        project.paints.iter().find_map(|p| match (p.target, p.swatch) {
            (PaintTarget::WallFace { wall, room }, Some(c)) if wall == w && room == r => Some(c),
            _ => None,
        })
    };
    let building_paint = |b: Option<BuildingId>| {
        b.and_then(|b| {
            project.paints.iter().find_map(|p| match (p.target, p.swatch) {
                (PaintTarget::Building(x), Some(c)) if x == b => Some(rgb(c)),
                _ => None,
            })
        })
    };

    let mut groups: BTreeMap<i64, Group> = BTreeMap::new();
    // Soft shadows on the ground, drawn over lawns and paths but under everything else.
    let mut shadows: Vec<egui::Mesh> = Vec::new();
    let key = |e: f64| (e * 100.0).round() as i64;

    for level in project.levels.iter().filter(|l| !l.is_site() && visible(l)) {
        let elev = level.elevation;
        let building_selected = level.building.is_some_and(|b| sel == Some(EntityRef::Building(b)));
        {
            let g = groups.entry(key(elev)).or_default();
            // Biggest first: floors on one level are flat together, so a
            // room within a room has to go on top of the one it's in.
            for room in level.rooms_outside_in() {
                let pts = level.room_outline(room);
                let selected = sel == Some(EntityRef::Room(room.id)) || building_selected;
                if let Some(m) = scene.flat(&pts, elev, if selected { c::teal() } else { floor }) {
                    g.floors.push(m);
                }
            }

            let outside = building_paint(level.building).unwrap_or(exterior);
            let around = level.rooms_around();
            for wall in level.walls.values() {
                let Some(line) = level.wall_polyline(wall) else { continue };
                let h = level.wall_height(wall);
                let rooms = level.rooms_of_wall(wall);
                // Which side each room is on (the same all along a curve).
                let mut sides: Vec<(RoomId, f64)> = rooms.iter().filter_map(|r| Some((*r, level.wall_side_of(wall, *r)?))).collect();
                // A room within a room has the room it's in round the outside.
                if let [(r, side)] = sides.as_slice() {
                    if let Some(outer) = around.get(r) {
                        sides.push((*outer, -side));
                    }
                }
                let selected = sel == Some(EntityRef::Wall(wall.id)) || building_selected;
                let openings: Vec<&Opening> = level.openings.values().filter(|o| o.wall == wall.id).collect();
                // A curved wall is drawn as a run of flat panels.
                let mut run = 0.0;
                for seg in line.windows(2) {
                    let (a, b) = (seg[0], seg[1]);
                    let seg_len = a.distance(b);
                    let quad = [p3(a, elev), p3(b, elev), p3(b, elev + h), p3(a, elev + h)];
                    let Some((pts, depth)) = cam.project_all(&quad) else {
                        run += seg_len;
                        continue;
                    };
                    let d = b - a;
                    let n3 = norm([d.y as f32, 0.0, -d.x as f32]);
                    let k = 0.86 + 0.14 * dot(n3, scene.light).abs();
                    // Which side is the camera on?
                    let mid = a.lerp(b, 0.5);
                    let to_eye = Vec2::new(eye[0] as f64 - mid.x, eye[2] as f64 - mid.y);
                    let n = d.perp();
                    let facing = sides.iter().find(|(_, side)| (n * *side).dot(to_eye) > 0.0).map(|(r, _)| *r);
                    let color = match facing {
                        Some(r) => face_paint(wall.id, Some(r)).or_else(|| room_paint(r)).map(rgb).unwrap_or(wall_base),
                        None => face_paint(wall.id, None).map(rgb).unwrap_or(outside),
                    };
                    let fill = if selected { c::teal() } else { shade(color, k) };
                    // A curve's panels are outlined in their own colour, so it reads as one smooth wall.
                    let stroke = if line.len() > 2 { Stroke::new(1.0_f32, fill) } else { scene.edge };
                    g.faces.push(Face { pts, depth, fill, stroke });
                    // The part of each door or window on this panel.
                    for o in &openings {
                        let (o0, o1) = ((o.offset - o.width / 2.0).max(run), (o.offset + o.width / 2.0).min(run + seg_len));
                        if o1 - o0 < 1e-6 {
                            continue;
                        }
                        let dir = d / seg_len;
                        let (s, e) = (a + dir * (o0 - run), a + dir * (o1 - run));
                        let (lo, hi) = (elev + o.sill_height, elev + (o.sill_height + o.height).min(h));
                        let q = [p3(s, lo), p3(e, lo), p3(e, hi), p3(s, hi)];
                        let Some((pts, _)) = cam.project_all(&q) else { continue };
                        let col = if sel == Some(EntityRef::Opening(o.id)) {
                            c::teal()
                        } else if o.kind.is_door() {
                            Color32::from_rgb(169, 122, 84)
                        } else {
                            Color32::from_rgb(196, 222, 228)
                        };
                        let fill = shade(col, k);
                        g.faces.push(Face { pts, depth: depth - 0.001, fill, stroke: if line.len() > 2 { Stroke::new(1.0_f32, fill) } else { scene.edge } });
                    }
                    run += seg_len;
                }
            }

            if st.show_items {
                for obj in level.objects.values().filter(|o| o.roof.is_none()) {
                    let color = if sel == Some(EntityRef::Object(obj.id)) {
                        c::teal()
                    } else {
                        theme::mix(Color32::from_rgb(244, 241, 234), rgb(obj.display_color()), 0.45)
                    };
                    if let Some(info) = project.stair_info(level.id, obj) {
                        stair_steps(&scene, &mut g.faces, obj, &info, elev, color, false);
                        continue;
                    }
                    if obj.category == ObjectCategory::Vehicle {
                        models::vehicle(&scene, &mut g.faces, &mut shadows, obj, elev, sel == Some(EntityRef::Object(obj.id)));
                        continue;
                    }
                    scene.prism(&mut g.faces, &obj.world_outline(), elev, elev + item_height(obj), color);
                }
            }
        }

        // Roofs sit on this level's walls; draw them with the storey above.
        if st.show_roofs && roof_visible(level) {
            let rg = groups.entry(key(elev + level.wall_height)).or_default();
            for roof in level.roofs.values() {
                let selected = sel == Some(EntityRef::RoofSection(roof.id));
                let swatch = roof.top_layer().and_then(|l| l.swatch).map(rgb).unwrap_or(Color32::from_rgb(122, 118, 112));
                let color = if selected { c::teal() } else { swatch };
                let mesh = roof.mesh(level.elevation, level.wall_height);
                for s in &mesh.surfaces {
                    let pts: Vec<V3> = s.iter().map(|(p, h)| p3(*p, *h)).collect();
                    scene.poly(&mut rg.faces, &pts, color, Stroke::new(1.0_f32, shade(color, 0.7)));
                }
                for f in &mesh.fascia {
                    let pts: Vec<V3> = f.iter().map(|(p, h)| p3(*p, *h)).collect();
                    scene.poly(&mut rg.faces, &pts, trim, Stroke::NONE);
                }
                for w in &mesh.gables {
                    let pts: Vec<V3> = w.iter().map(|(p, h)| p3(*p, *h)).collect();
                    scene.poly(&mut rg.faces, &pts, building_paint(level.building).unwrap_or(exterior), scene.edge);
                }
                if st.show_items {
                    let base = roof.eave_height.unwrap_or(level.wall_height) + level.elevation;
                    for obj in level.objects.values().filter(|o| o.roof == Some(roof.id)) {
                        let outline = obj.world_outline();
                        let z = outline.iter().map(|p| roof.height_at(base, *p)).fold(f64::MIN, f64::max);
                        let h = if obj.label.to_lowercase().contains("chimney") { 1.0 } else { 0.15 };
                        let color = if sel == Some(EntityRef::Object(obj.id)) {
                            c::teal()
                        } else {
                            theme::mix(Color32::from_rgb(200, 196, 190), rgb(obj.display_color()), 0.5)
                        };
                        scene.prism(&mut rg.faces, &outline, z - 0.2, z + h, color);
                    }
                }
            }
        }
    }

    // Outside
    let mut ground: Vec<(i32, f32, egui::Mesh)> = Vec::new();
    let mut lines: Vec<EShape> = Vec::new();
    if st.show_outside {
        if let Some(site) = project.site() {
            let g = groups.entry(0).or_default();
            for f in site.features.values() {
                let selected = sel == Some(EntityRef::Feature(f.id));
                let col = if selected { c::teal() } else { rgb(f.display_color()) };
                match f.kind {
                    FeatureKind::PropertyLine => {
                        let n = f.points.len();
                        for i in 0..n {
                            let (a, b) = (f.points[i], f.points[(i + 1) % n]);
                            if let (Some((sa, _)), Some((sb, _))) = (cam.project(p3(a, 0.02)), cam.project(p3(b, 0.02))) {
                                lines.extend(EShape::dashed_line(&[sa, sb], Stroke::new(1.5_f32, c::muted()), 8.0, 5.0));
                            }
                        }
                    }
                    FeatureKind::Deck => {
                        let stairs: Vec<&PlacedObject> = site.objects.values().filter(|o| o.feature == Some(f.id) && o.is_stairs()).collect();
                        models::deck(&scene, &mut g.faces, f, col, &stairs);
                    }
                    FeatureKind::GardenBed => scene.prism(&mut g.faces, &f.points, 0.0, f.height.max(0.05), col),
                    FeatureKind::Carport => {
                        // A roof on posts: a post at each corner.
                        let h = f.height.max(1.8);
                        let post = Color32::from_rgb(150, 150, 150);
                        for p in &f.points {
                            let s = 0.06;
                            let sq = [*p + Vec2::new(-s, -s), *p + Vec2::new(s, -s), *p + Vec2::new(s, s), *p + Vec2::new(-s, s)];
                            scene.prism(&mut g.faces, &sq, 0.0, h, post);
                        }
                        scene.prism(&mut g.faces, &f.points, h, h + 0.12, col);
                    }
                    _ if f.kind.geom() == FeatureGeom::Area => {
                        let order = match f.kind {
                            FeatureKind::Lawn => 0,
                            FeatureKind::Driveway => 1,
                            FeatureKind::Path => 2,
                            FeatureKind::Patio => 3,
                            _ => 4,
                        };
                        let fill = if f.kind == FeatureKind::Pool && !selected { Color32::from_rgb(120, 190, 214) } else { col };
                        // Pure black asphalt reads as a hole in the ground.
                        let fill = models::lift_dark(fill, Color32::from_rgb(70, 71, 74));
                        if let Some((d, m)) = scene.flat(&f.points, 0.01 + order as f64 * 0.004, fill) {
                            ground.push((order, d, m));
                        }
                    }
                    FeatureKind::Fence => models::fence(&scene, &mut g.faces, f, col),
                    FeatureKind::RetainingWall | FeatureKind::Hedge => {
                        let cuts: Vec<(f64, f64)> = f.gates.iter().map(|g| (g.offset - g.width / 2.0, g.offset + g.width / 2.0)).collect();
                        let w = f.line_width();
                        for piece in cut_polyline(&f.points, &cuts) {
                            for s in piece.windows(2) {
                                let (a, b) = (s[0], s[1]);
                                let n = (b - a).normalized().perp() * (w / 2.0);
                                let outline = [a + n, b + n, b - n, a - n];
                                scene.prism(&mut g.faces, &outline, 0.0, f.height, col);
                            }
                        }
                    }
                    FeatureKind::Tree => models::tree(&scene, &mut g.faces, &mut shadows, f, selected),
                    _ => {}
                }
            }
            if st.show_items {
                for obj in site.objects.values() {
                    let on = obj.feature.and_then(|fid| site.features.get(&fid));
                    let base = on.map(|f| if f.kind == FeatureKind::Deck { f.height } else { 0.02 }).unwrap_or(0.0);
                    let selected = sel == Some(EntityRef::Object(obj.id));
                    let color = if selected {
                        c::teal()
                    } else {
                        theme::mix(Color32::from_rgb(244, 241, 234), rgb(obj.display_color()), 0.45)
                    };
                    if let Some(info) = project.stair_info(site.id, obj) {
                        // Deck stairs are built from the deck's own boards.
                        let wood = on.filter(|f| f.kind == FeatureKind::Deck && !selected).map(|f| rgb(f.display_color()));
                        stair_steps(&scene, &mut g.faces, obj, &info, 0.0, wood.unwrap_or(color), true);
                        continue;
                    }
                    if obj.category == ObjectCategory::Vehicle {
                        models::vehicle(&scene, &mut g.faces, &mut shadows, obj, base, selected);
                        continue;
                    }
                    scene.prism(&mut g.faces, &obj.world_outline(), base, base + item_height(obj), color);
                }
            }
        }
    }
    let draw_group = |mut g: Group| {
        // In the order they were added (they're all flat at one height).
        for (_, m) in g.floors {
            painter.add(EShape::mesh(m));
        }
        g.faces.sort_by(|a, b| b.depth.total_cmp(&a.depth));
        for f in g.faces {
            // egui's anti-aliasing turns repeated points, needle-thin corners
            // and edge-on slivers into long spikes, so tidy each face first.
            let pts = clean_polygon(f.pts);
            if pts.len() < 3 || screen_thickness(&pts) < 1.0 {
                continue;
            }
            painter.add(EShape::convex_polygon(pts, f.fill, f.stroke));
        }
    };

    // Basements first, then the ground over them (only faintly see-through,
    // unless it's the basement you're looking at), then everything above.
    let above = groups.split_off(&0);
    for (_, g) in groups {
        draw_group(g);
    }
    let (lo, hi) = grid_extent(project);
    let looking_underground = st.hide_upper_levels && cutoff.is_some_and(|(_, e)| e < -0.05);
    if !looking_underground {
        let cover = [lo, Vec2::new(hi.x, lo.y), hi, Vec2::new(lo.x, hi.y)];
        let canvas = c::canvas();
        if let Some((_, m)) = scene.flat(&cover, 0.0, rgba(canvas.r(), canvas.g(), canvas.b(), 232)) {
            painter.add(EShape::mesh(m));
        }
    }
    ground_grid(&painter, &cam, lo, hi);
    ground.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
    for (_, _, m) in ground {
        painter.add(EShape::mesh(m));
    }
    for m in shadows {
        painter.add(EShape::mesh(m));
    }
    painter.extend(lines);
    for (_, g) in above {
        draw_group(g);
    }

    let empty = project.levels.iter().all(|l| l.rooms.is_empty() && l.features.is_empty());
    if empty {
        painter.text(
            area.center(),
            egui::Align2::CENTER_CENTER,
            "Draw some rooms on the plan to see them in 3D.",
            egui::FontId::proportional(15.0),
            c::muted(),
        );
    }
}

/// Drop repeated points and corners so sharp they're really a straight line.
fn clean_polygon(mut pts: Vec<Pos2>) -> Vec<Pos2> {
    loop {
        let n = pts.len();
        if n < 3 {
            return pts;
        }
        let bad = (0..n).find(|&i| {
            let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let (u, v) = (b - a, c - b);
            let (lu, lv) = (u.length(), v.length());
            if lu < 0.5 || lv < 0.5 {
                return true;
            }
            // Nearly straight (or folding back on itself): sin(angle) tiny.
            (u.x * v.y - u.y * v.x).abs() / (lu * lv) < 0.03 && u.dot(v) < 0.0
                || (u.x * v.y - u.y * v.x).abs() / (lu * lv) < 0.004
        });
        match bad {
            Some(i) => {
                pts.remove(i);
            }
            None => return pts,
        }
    }
}

fn screen_area(pts: &[Pos2]) -> f32 {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        .abs()
        / 2.0
}

/// How thick a polygon is on screen: area over its longest edge. Faces
/// thinner than a pixel are invisible but spike under anti-aliasing.
fn screen_thickness(pts: &[Pos2]) -> f32 {
    let n = pts.len();
    let longest = (0..n).map(|i| pts[i].distance(pts[(i + 1) % n])).fold(0.0, f32::max);
    if longest < 1e-3 { 0.0 } else { screen_area(pts) / longest }
}

/// A polyline with gaps (gates) cut out, as separate pieces.
fn cut_polyline(pts: &[Vec2], cuts: &[(f64, f64)]) -> Vec<Vec<Vec2>> {
    let total = polyline_length(pts);
    let mut stops: Vec<f64> = vec![0.0];
    let mut run = 0.0;
    for w in pts.windows(2) {
        run += w[0].distance(w[1]);
        stops.push(run);
    }
    for (a, b) in cuts {
        stops.push(a.clamp(0.0, total));
        stops.push(b.clamp(0.0, total));
    }
    stops.sort_by(f64::total_cmp);
    stops.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let mut pieces = Vec::new();
    let mut cur: Vec<Vec2> = Vec::new();
    for w in stops.windows(2) {
        let mid = (w[0] + w[1]) / 2.0;
        if cuts.iter().any(|(a, b)| mid > *a && mid < *b) {
            if cur.len() >= 2 {
                pieces.push(std::mem::take(&mut cur));
            }
            cur.clear();
            continue;
        }
        if cur.is_empty() {
            cur.extend(point_along(pts, w[0]).map(|(p, _)| p));
        }
        cur.extend(point_along(pts, w[1]).map(|(p, _)| p));
    }
    if cur.len() >= 2 {
        pieces.push(cur);
    }
    pieces
}

/// The ground around the house: its walls plus 6 m, to whole metres.
fn grid_extent(project: &Project) -> (Vec2, Vec2) {
    let pts: Vec<Vec2> = project
        .levels
        .iter()
        .filter(|l| !l.is_site())
        .flat_map(|l| l.nodes.values().map(|n| n.pos))
        .collect();
    let (lo, hi) = bounds(&pts).unwrap_or((Vec2::new(-5.0, -5.0), Vec2::new(5.0, 5.0)));
    (Vec2::new((lo.x - 6.0).floor(), (lo.y - 6.0).floor()), Vec2::new((hi.x + 6.0).ceil(), (hi.y + 6.0).ceil()))
}

/// Faint ground grid (1 m).
fn ground_grid(painter: &egui::Painter, cam: &Cam, lo: Vec2, hi: Vec2) {
    let (x0, x1) = (lo.x as i32, hi.x as i32);
    let (z0, z1) = (lo.y as i32, hi.y as i32);
    let stroke = Stroke::new(1.0_f32, c::grid_major());
    for x in x0..=x1 {
        if let (Some((a, _)), Some((b, _))) = (cam.project([x as f32, 0.0, z0 as f32]), cam.project([x as f32, 0.0, z1 as f32])) {
            painter.line_segment([a, b], stroke);
        }
    }
    for z in z0..=z1 {
        if let (Some((a, _)), Some((b, _))) = (cam.project([x0 as f32, 0.0, z as f32]), cam.project([x1 as f32, 0.0, z as f32])) {
            painter.line_segment([a, b], stroke);
        }
    }
}

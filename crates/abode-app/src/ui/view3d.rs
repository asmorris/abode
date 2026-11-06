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
        }
    }
}

impl View3dState {
    /// Point the camera (debug screenshots).
    pub fn set_angle(&mut self, yaw: f32, pitch: f32) {
        self.yaw = yaw;
        self.pitch = pitch.clamp(0.08, 1.5);
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
    Color32::from_rgba_unmultiplied(f(c.r()), f(c.g()), f(c.b()), c.a())
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

    /// A prism: vertical sides plus a top.
    fn prism(&self, out: &mut Vec<Face>, outline: &[Vec2], lo: f64, hi: f64, color: Color32) {
        let n = outline.len();
        for i in 0..n {
            self.wall(out, outline[i], outline[(i + 1) % n], lo, hi, color, Stroke::NONE);
        }
        let top: Vec<V3> = outline.iter().map(|p| p3(*p, hi)).collect();
        let before = out.len();
        self.poly(out, &top, shade(color, 1.08), self.edge);
        for f in &mut out[before..] {
            f.depth -= 0.01;
        }
    }
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
    ground_grid(&painter, &cam, project);
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
    let key = |e: f64| (e * 100.0).round() as i64;

    for level in project.levels.iter().filter(|l| !l.is_site() && visible(l)) {
        let elev = level.elevation;
        let building_selected = level.building.is_some_and(|b| sel == Some(EntityRef::Building(b)));
        {
            let g = groups.entry(key(elev)).or_default();
            for room in level.rooms.values() {
                let pts = level.room_points(room);
                let selected = sel == Some(EntityRef::Room(room.id)) || building_selected;
                if let Some(m) = scene.flat(&pts, elev, if selected { c::teal() } else { floor }) {
                    g.floors.push(m);
                }
            }

            let outside = building_paint(level.building).unwrap_or(exterior);
            for wall in level.walls.values() {
                let Some((a, b)) = level.wall_points(wall) else { continue };
                let h = level.wall_height(wall);
                let quad = [p3(a, elev), p3(b, elev), p3(b, elev + h), p3(a, elev + h)];
                let Some((pts, depth)) = cam.project_all(&quad) else { continue };
                let d = b - a;
                let n3 = norm([d.y as f32, 0.0, -d.x as f32]);
                let k = 0.86 + 0.14 * dot(n3, scene.light).abs();
                // Which side is the camera on?
                let mid = a.lerp(b, 0.5);
                let to_eye = Vec2::new(eye[0] as f64 - mid.x, eye[2] as f64 - mid.y);
                let rooms = level.rooms_of_wall(wall);
                let facing = rooms
                    .iter()
                    .copied()
                    .find(|r| level.wall_normal_into(wall, *r).is_some_and(|n| n.dot(to_eye) > 0.0));
                let color = match facing {
                    Some(r) => face_paint(wall.id, Some(r)).or_else(|| room_paint(r)).map(rgb).unwrap_or(wall_base),
                    None => face_paint(wall.id, None).map(rgb).unwrap_or(outside),
                };
                let selected = sel == Some(EntityRef::Wall(wall.id)) || building_selected;
                g.faces.push(Face { pts, depth, fill: if selected { c::teal() } else { shade(color, k) }, stroke: scene.edge });
                for o in level.openings.values().filter(|o| o.wall == wall.id) {
                    let Some((s, e, _)) = level.opening_segment(o) else { continue };
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
                    g.faces.push(Face { pts, depth: depth - 0.001, fill: shade(col, k), stroke: scene.edge });
                }
            }

            if st.show_items {
                for obj in level.objects.values().filter(|o| o.roof.is_none()) {
                    let color = if sel == Some(EntityRef::Object(obj.id)) {
                        c::teal()
                    } else {
                        theme::mix(Color32::from_rgb(244, 241, 234), rgb(obj.display_color()), 0.45)
                    };
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
                                painter.extend(EShape::dashed_line(&[sa, sb], Stroke::new(1.5_f32, c::muted()), 8.0, 5.0));
                            }
                        }
                    }
                    FeatureKind::Deck => {
                        let h = f.height.max(0.1);
                        scene.prism(&mut g.faces, &f.points, (h - 0.3).max(0.0), h, col);
                        if f.railing {
                            let n = f.points.len();
                            let rail = Color32::from_rgba_unmultiplied(70, 70, 70, 90);
                            for i in 0..n {
                                scene.wall(&mut g.faces, f.points[i], f.points[(i + 1) % n], h, h + 0.9, rail, Stroke::new(1.0_f32, Color32::from_rgb(60, 60, 60)));
                            }
                        }
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
                        if let Some((d, m)) = scene.flat(&f.points, 0.01 + order as f64 * 0.004, fill) {
                            ground.push((order, d, m));
                        }
                    }
                    FeatureKind::Fence | FeatureKind::RetainingWall | FeatureKind::Hedge => {
                        let cuts: Vec<(f64, f64)> = f.gates.iter().map(|g| (g.offset - g.width / 2.0, g.offset + g.width / 2.0)).collect();
                        let w = f.line_width();
                        for piece in cut_polyline(&f.points, &cuts) {
                            for s in piece.windows(2) {
                                let (a, b) = (s[0], s[1]);
                                if f.kind == FeatureKind::Fence {
                                    scene.wall(&mut g.faces, a, b, 0.0, f.height, col, scene.edge);
                                } else {
                                    let n = (b - a).normalized().perp() * (w / 2.0);
                                    let outline = [a + n, b + n, b - n, a - n];
                                    scene.prism(&mut g.faces, &outline, 0.0, f.height, col);
                                }
                            }
                        }
                    }
                    FeatureKind::Tree => {
                        let Some(center) = f.points.first().copied() else { continue };
                        tree(&scene, &mut g.faces, center, f.height.max(1.0), (f.size / 2.0).max(0.5), col);
                    }
                    _ => {}
                }
            }
            if st.show_items {
                for obj in site.objects.values() {
                    let base = obj
                        .feature
                        .and_then(|fid| site.features.get(&fid))
                        .map(|f| if f.kind == FeatureKind::Deck { f.height } else { 0.02 })
                        .unwrap_or(0.0);
                    let color = if sel == Some(EntityRef::Object(obj.id)) {
                        c::teal()
                    } else {
                        theme::mix(Color32::from_rgb(244, 241, 234), rgb(obj.display_color()), 0.45)
                    };
                    scene.prism(&mut g.faces, &obj.world_outline(), base, base + item_height(obj), color);
                }
            }
        }
    }
    ground.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
    for (_, _, m) in ground {
        painter.add(EShape::mesh(m));
    }

    for (_, mut g) in groups {
        g.floors.sort_by(|a, b| b.0.total_cmp(&a.0));
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

/// A low-poly tree: a trunk and a rounded canopy.
fn tree(scene: &Scene, out: &mut Vec<Face>, center: Vec2, height: f64, radius: f64, color: Color32) {
    let trunk = Color32::from_rgb(120, 92, 66);
    let t = (radius * 0.12).clamp(0.08, 0.25);
    let trunk_outline = [
        center + Vec2::new(-t, -t),
        center + Vec2::new(t, -t),
        center + Vec2::new(t, t),
        center + Vec2::new(-t, t),
    ];
    let canopy_mid = height * 0.62;
    let vr = (height * 0.38).min(radius * 1.1);
    scene.prism(out, &trunk_outline, 0.0, canopy_mid, trunk);
    let (lon, lat) = (10, 5);
    let point = |i: usize, j: usize| {
        let a = i as f64 / lon as f64 * std::f64::consts::TAU;
        let b = -std::f64::consts::FRAC_PI_2 + j as f64 / lat as f64 * std::f64::consts::PI;
        let r = radius * b.cos();
        p3(center + Vec2::new(a.cos() * r, a.sin() * r), canopy_mid + vr * b.sin())
    };
    // Flat caps instead of pointed poles: needle-sharp triangles spike under anti-aliasing.
    for j in [1, lat - 1] {
        let ring: Vec<V3> = (0..lon).map(|i| point(i, j)).collect();
        let k = if j == 1 { 0.7 } else { 1.04 };
        if let Some((s, depth)) = scene.cam.project_all(&ring) {
            out.push(Face { pts: s, depth, fill: shade(color, k), stroke: Stroke::NONE });
        }
    }
    for i in 0..lon {
        for j in 1..lat - 1 {
            let quad = [point(i, j), point(i + 1, j), point(i + 1, j + 1), point(i, j + 1)];
            let n = norm(sub(add(quad[0], quad[2]), scale(p3(center, canopy_mid), 2.0)));
            let k = 0.72 + 0.32 * dot(n, scene.light).max(0.0);
            if let Some((s, depth)) = scene.cam.project_all(&quad) {
                out.push(Face { pts: s, depth, fill: shade(color, k), stroke: Stroke::NONE });
            }
        }
    }
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

/// Faint ground grid (1 m) around the house.
fn ground_grid(painter: &egui::Painter, cam: &Cam, project: &Project) {
    let pts: Vec<Vec2> = project
        .levels
        .iter()
        .filter(|l| !l.is_site())
        .flat_map(|l| l.nodes.values().map(|n| n.pos))
        .collect();
    let (lo, hi) = bounds(&pts).unwrap_or((Vec2::new(-5.0, -5.0), Vec2::new(5.0, 5.0)));
    let (x0, x1) = ((lo.x - 6.0).floor() as i32, (hi.x + 6.0).ceil() as i32);
    let (z0, z1) = ((lo.y - 6.0).floor() as i32, (hi.y + 6.0).ceil() as i32);
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

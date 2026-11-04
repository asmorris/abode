//! Read-only 3D view: the plan extruded into walls, with an orbit camera.
//!
//! Drawn with egui's 2D painter (projected polygons, painter's-algorithm
//! sorted) so it needs no GPU pipeline. Good enough for a house model; when
//! the plumbing/electrical work needs real 3D, swap this for a wgpu scene.

use super::Env;
use abode_core::geometry::triangulate;
use abode_core::*;
use eframe::egui::{self, Color32, PointerButton, Pos2, Sense, Shape as EShape, Stroke, Ui, vec2};

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

pub struct View3dState {
    yaw: f32,
    pitch: f32,
    dist: f32,
    target: V3,
    fitted: bool,
    pub hide_upper_levels: bool,
    pub show_items: bool,
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
        }
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
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

fn fit(st: &mut View3dState, project: &Project) {
    let pts: Vec<Vec2> = project.levels.iter().flat_map(|l| l.nodes.values().map(|n| n.pos)).collect();
    let top = project
        .levels
        .iter()
        .map(|l| l.elevation + l.wall_height)
        .fold(0.0, f64::max);
    if let Some((lo, hi)) = abode_core::geometry::bounds(&pts) {
        let c = (lo + hi) / 2.0;
        st.target = [c.x as f32, (top / 3.0) as f32, c.y as f32];
        st.dist = ((hi - lo).length() as f32 * 1.4).max(8.0);
    } else {
        st.target = [0.0, 1.0, 0.0];
        st.dist = 20.0;
    }
    st.fitted = true;
}

pub fn view3d_ui(ui: &mut Ui, project: &Project, env: &mut Env, st: &mut View3dState) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut st.hide_upper_levels, "Hide levels above the current one");
        ui.checkbox(&mut st.show_items, "Show items");
        if ui.button("Reset view").clicked() {
            st.fitted = false;
        }
        ui.weak("Drag to orbit · right-drag to pan · scroll to zoom");
    });
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = resp.rect;
    let dark = ui.visuals().dark_mode;
    painter.rect_filled(
        rect,
        0.0,
        if dark { Color32::from_rgb(24, 26, 30) } else { Color32::from_rgb(236, 240, 245) },
    );
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
    let cam = Cam { eye, right, up, fwd, f: rect.height() * 1.1, center: rect.center() };
    let light = norm([0.4, 0.9, 0.3]);

    let active_elev = project.level(*env.active_level).map(|l| l.elevation).unwrap_or(0.0);
    let mut levels: Vec<&Level> = project
        .levels
        .iter()
        .filter(|l| !st.hide_upper_levels || l.elevation <= active_elev + 1e-6)
        .collect();
    levels.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));

    let wall_base = if dark { Color32::from_rgb(190, 192, 198) } else { Color32::from_rgb(226, 222, 214) };
    let exterior = if dark { Color32::from_rgb(150, 140, 128) } else { Color32::from_rgb(196, 182, 160) };
    let floor = if dark { Color32::from_rgb(70, 74, 82) } else { Color32::from_rgb(200, 190, 172) };
    let edge = Stroke::new(1.0, Color32::from_black_alpha(90));

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

    for level in levels {
        let elev = level.elevation;

        // Floors
        let mut floors: Vec<(f32, egui::Mesh)> = Vec::new();
        for room in level.rooms.values() {
            let pts = level.room_points(room);
            let Some((screen, depth)) = cam.project_all(&pts.iter().map(|p| p3(*p, elev)).collect::<Vec<_>>()) else {
                continue;
            };
            let selected = *env.selection == Some(EntityRef::Room(room.id));
            let color = if selected { ui.visuals().selection.bg_fill } else { floor };
            let mut mesh = egui::Mesh::default();
            for s in &screen {
                mesh.colored_vertex(*s, color);
            }
            for t in triangulate(&pts) {
                mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
            }
            floors.push((depth, mesh));
        }
        floors.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, m) in floors {
            painter.add(EShape::mesh(m));
        }

        let mut faces: Vec<Face> = Vec::new();

        // Walls and their openings
        for wall in level.walls.values() {
            let Some((a, b)) = level.wall_points(wall) else { continue };
            let h = level.wall_height(wall);
            let quad = [p3(a, elev), p3(b, elev), p3(b, elev + h), p3(a, elev + h)];
            let Some((pts, depth)) = cam.project_all(&quad) else { continue };
            let d = b - a;
            let n3 = norm([d.y as f32, 0.0, -d.x as f32]);
            let k = 0.55 + 0.45 * dot(n3, light).abs();
            // Which side is the camera on?
            let mid = a.lerp(b, 0.5);
            let to_eye = Vec2::new(eye[0] as f64 - mid.x, eye[2] as f64 - mid.y);
            let rooms = level.rooms_of_wall(wall);
            let facing = rooms
                .iter()
                .copied()
                .find(|r| level.wall_normal_into(wall, *r).is_some_and(|n| n.dot(to_eye) > 0.0));
            let color = match facing {
                Some(r) => face_paint(wall.id, Some(r)).or_else(|| room_paint(r)).map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(wall_base),
                None => face_paint(wall.id, None).map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(exterior),
            };
            let selected = *env.selection == Some(EntityRef::Wall(wall.id));
            faces.push(Face {
                pts,
                depth,
                fill: if selected { ui.visuals().selection.bg_fill } else { shade(color, k) },
                stroke: edge,
            });
            for o in level.openings.values().filter(|o| o.wall == wall.id) {
                let Some((s, e, _)) = level.opening_segment(o) else { continue };
                let (lo, hi) = (elev + o.sill_height, elev + (o.sill_height + o.height).min(h));
                let q = [p3(s, lo), p3(e, lo), p3(e, hi), p3(s, hi)];
                let Some((pts, _)) = cam.project_all(&q) else { continue };
                let c = if o.kind.is_door() {
                    Color32::from_rgb(120, 86, 60)
                } else {
                    Color32::from_rgb(150, 196, 230)
                };
                faces.push(Face { pts, depth: depth - 0.001, fill: shade(c, k), stroke: edge });
            }
        }

        // Items as simple prisms
        if st.show_items {
            for obj in level.objects.values() {
                let outline = obj.world_outline();
                let h = match obj.category {
                    ObjectCategory::Hvac | ObjectCategory::Appliance => 0.9,
                    ObjectCategory::Electrical => 0.3,
                    ObjectCategory::Furniture => 0.75,
                    _ => 0.6,
                };
                let base = obj.display_color();
                let base = Color32::from_rgb(base[0], base[1], base[2]);
                let n = outline.len();
                for i in 0..n {
                    let (a, b) = (outline[i], outline[(i + 1) % n]);
                    let q = [p3(a, elev), p3(b, elev), p3(b, elev + h), p3(a, elev + h)];
                    let Some((pts, depth)) = cam.project_all(&q) else { continue };
                    let d = b - a;
                    let k = 0.55 + 0.45 * dot(norm([d.y as f32, 0.0, -d.x as f32]), light).abs();
                    faces.push(Face { pts, depth, fill: shade(base, k), stroke: Stroke::NONE });
                }
                let top: Vec<V3> = outline.iter().map(|p| p3(*p, elev + h)).collect();
                if let Some((pts, depth)) = cam.project_all(&top) {
                    faces.push(Face { pts, depth: depth - 0.01, fill: shade(base, 1.1), stroke: edge });
                }
            }
        }

        faces.sort_by(|a, b| b.depth.total_cmp(&a.depth));
        for f in faces {
            painter.add(EShape::convex_polygon(f.pts, f.fill, f.stroke));
        }
    }

    if project.levels.iter().all(|l| l.rooms.is_empty()) {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Draw some rooms on the plan to see them in 3D.",
            egui::FontId::proportional(15.0),
            ui.visuals().weak_text_color(),
        );
    }
}

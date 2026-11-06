//! The 2D plan editor.
//!
//! World coordinates are metres (+y down). `View` maps them to screen pixels.
//! There are three contexts:
//! - a floor: rooms, doors, windows and items;
//! - that floor's roof layer: roof sections and what's on them (skylights,
//!   chimneys, gutters...);
//! - Outside: the lot, decks, patios, fences, trees, outbuildings, with the
//!   house's ground floor drawn for reference.
//!
//! Each frame: handle zoom/pan, run the active tool, draw the plan, then the
//! floating toolbars on top.

use super::widgets::length_edit_sized;
use super::{Action, CalibrateDialog, Env, Insets};
use crate::theme::{self, c, icon};
use abode_core::geometry::*;
use abode_core::roof::format_pitch;
use abode_core::units::{UnitSystem, format_area, format_length};
use abode_core::*;
use eframe::egui::{
    self, Align2, Color32, FontId, Key, Painter, PointerButton, Pos2, Rect, Sense, Shape as EShape, Stroke, Ui,
    pos2, vec2,
};
use std::collections::HashMap;

/// Which layer of a floor you're editing.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Layer {
    #[default]
    Floor,
    Roof,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ctx {
    Floor,
    Roof,
    Site,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    RectRoom,
    PolyRoom,
    Door,
    Window,
    Object,
    RectRoof,
    PolyRoof,
    RectArea,
    PolyArea,
    Line,
    Tree,
    Gate,
    Building,
    PropertyLine,
    Calibrate,
    MoveUnderlay,
}

const FLOOR_TOOLS: &[&[Tool]] = &[&[Tool::Select], &[Tool::RectRoom, Tool::PolyRoom], &[Tool::Door, Tool::Window, Tool::Object]];
const ROOF_TOOLS: &[&[Tool]] = &[&[Tool::Select], &[Tool::RectRoof, Tool::PolyRoof], &[Tool::Object]];
const SITE_TOOLS: &[&[Tool]] = &[
    &[Tool::Select],
    &[Tool::RectArea, Tool::PolyArea, Tool::Line, Tool::Tree],
    &[Tool::Building, Tool::Gate, Tool::Object],
    &[Tool::PropertyLine],
];

impl Tool {
    fn groups(ctx: Ctx) -> &'static [&'static [Tool]] {
        match ctx {
            Ctx::Floor => FLOOR_TOOLS,
            Ctx::Roof => ROOF_TOOLS,
            Ctx::Site => SITE_TOOLS,
        }
    }

    fn fits(self, ctx: Ctx) -> bool {
        matches!(self, Tool::Calibrate | Tool::MoveUnderlay)
            || Tool::groups(ctx).iter().any(|g| g.contains(&self))
    }

    fn icon(self) -> &'static str {
        match self {
            Tool::Select => icon::CURSOR,
            Tool::RectRoom | Tool::RectRoof | Tool::RectArea => icon::SQUARE,
            Tool::PolyRoom | Tool::PolyRoof | Tool::PolyArea => icon::POLYGON,
            Tool::Door => icon::DOOR,
            Tool::Window => icon::APP_WINDOW,
            Tool::Object => icon::PACKAGE,
            Tool::Line => icon::LINE_SEGMENTS,
            Tool::Tree => icon::TREE,
            Tool::Gate => icon::DOOR_OPEN,
            Tool::Building => icon::WAREHOUSE,
            Tool::PropertyLine => icon::BOUNDING_BOX,
            Tool::Calibrate => icon::RULER,
            Tool::MoveUnderlay => icon::HAND_GRABBING,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::RectRoom => "Room",
            Tool::PolyRoom => "Shaped room",
            Tool::Door => "Door",
            Tool::Window => "Window",
            Tool::Object => "Item",
            Tool::RectRoof => "Roof section",
            Tool::PolyRoof => "Shaped roof section",
            Tool::RectArea => "Deck, patio, driveway…",
            Tool::PolyArea => "Shaped deck, patio…",
            Tool::Line => "Fence, wall or hedge",
            Tool::Tree => "Tree",
            Tool::Gate => "Gate",
            Tool::Building => "Shed or garage",
            Tool::PropertyLine => "Property line",
            Tool::Calibrate => "Calibrate blueprint",
            Tool::MoveUnderlay => "Move blueprint",
        }
    }

    fn key(self) -> Option<Key> {
        Some(match self {
            Tool::Select => Key::V,
            Tool::RectRoom | Tool::RectRoof | Tool::RectArea => Key::R,
            Tool::PolyRoom | Tool::PolyRoof | Tool::PolyArea => Key::P,
            Tool::Door => Key::D,
            Tool::Window => Key::W,
            Tool::Object => Key::O,
            Tool::Line => Key::L,
            Tool::Tree => Key::T,
            Tool::Gate => Key::G,
            Tool::Building => Key::B,
            _ => return None,
        })
    }

    fn hint(self, ctx: Ctx) -> &'static str {
        match (self, ctx) {
            (Tool::Select, Ctx::Site) => "Click to select · drag a selected thing to move it · double-click a building to open its plan",
            (Tool::Select, Ctx::Roof) => "Click a roof section to select it · drag its corners, or drag it to move it",
            (Tool::Select, _) => "Click to select · drag a selected room, items, doors and windows to move them · drag empty space to pan",
            (Tool::RectRoom, _) => "Drag to draw a room · snaps to corners and walls · hold Alt to disable snapping",
            (Tool::PolyRoom | Tool::PolyRoof | Tool::PolyArea | Tool::PropertyLine, _) => {
                "Click to add corners · click the first corner or press Enter to finish · Shift for straight lines"
            }
            (Tool::Door, _) => "Click on a wall to add a door",
            (Tool::Window, _) => "Click on a wall to add a window",
            (Tool::Object, Ctx::Roof) => "Click on a roof section to place it, or drag to size it",
            (Tool::Object, Ctx::Site) => "Click anywhere outside to place it; on a deck or patio it sits on it",
            (Tool::Object, _) => "Click inside a room to place it, or drag to size it",
            (Tool::RectRoof, _) => "Drag over the walls a roof section covers · the overhang is added for you",
            (Tool::RectArea, _) => "Drag to draw it · snaps to the house walls",
            (Tool::Line, _) => "Click to add posts · double-click or press Enter to finish · Shift for straight lines",
            (Tool::Tree, _) => "Click to plant a tree",
            (Tool::Gate, _) => "Click on a fence to add a gate",
            (Tool::Building, _) => "Drag out the building's footprint",
            (Tool::Calibrate, _) => "Click two points on the blueprint that you know the real distance between",
            (Tool::MoveUnderlay, _) => "Drag to line the blueprint up",
        }
    }

    fn closes_shape(self) -> bool {
        matches!(self, Tool::PolyRoom | Tool::PolyRoof | Tool::PolyArea | Tool::PropertyLine)
    }
}

#[derive(Clone, Copy)]
struct View {
    offset: egui::Vec2,
    zoom: f32,
}

impl View {
    fn to_screen(self, rect: Rect, p: Vec2) -> Pos2 {
        rect.min + self.offset + vec2(p.x as f32, p.y as f32) * self.zoom
    }
    fn to_world(self, rect: Rect, s: Pos2) -> Vec2 {
        let v = (s - rect.min - self.offset) / self.zoom;
        Vec2::new(v.x as f64, v.y as f64)
    }
    /// Size of one screen pixel in metres.
    fn px(self) -> f64 {
        1.0 / self.zoom as f64
    }
}

enum Draft {
    None,
    Rect { start: Vec2 },
    Poly { points: Vec<Vec2> },
    Object { start: Vec2 },
    Calibrate { first: Vec2 },
}

#[derive(Clone, Copy)]
enum DragOp {
    Pan,
    Object { id: ObjectId, grab: Vec2 },
    Opening(OpeningId),
    /// Dragging a corner of a room (from where it started).
    Node { node: NodeId, room: RoomId, start: Vec2 },
    RoofVertex(RoofId, usize),
    FeatureVertex(FeatureId, usize),
    Gate(FeatureId, GateId),
    MoveRoof(RoofId, Vec2),
    MoveFeature(FeatureId, Vec2),
    /// Moving a whole room: where the pointer grabbed it, and where its first
    /// corner started.
    MoveRoom { id: RoomId, grab: Vec2, anchor: Vec2 },
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Hit {
    Opening(OpeningId),
    Object(ObjectId),
    Node(NodeId),
    Wall(WallId),
    Room(RoomId),
    Roof(RoofId),
    RoofVertex(RoofId, usize),
    Feature(FeatureId),
    FeatureVertex(FeatureId, usize),
    Gate(FeatureId, GateId),
    Building(BuildingId),
    House,
}

impl Hit {
    fn entity(self) -> Option<EntityRef> {
        Some(match self {
            Hit::Opening(id) => EntityRef::Opening(id),
            Hit::Object(id) => EntityRef::Object(id),
            Hit::Wall(id) => EntityRef::Wall(id),
            Hit::Room(id) => EntityRef::Room(id),
            Hit::Roof(id) => EntityRef::RoofSection(id),
            Hit::Feature(id) | Hit::Gate(id, _) => EntityRef::Feature(id),
            Hit::Building(id) => EntityRef::Building(id),
            Hit::House => EntityRef::General,
            Hit::Node(_) | Hit::RoofVertex(..) | Hit::FeatureVertex(..) => return None,
        })
    }
}

pub struct PlanState {
    pub tool: Tool,
    pub layer: Layer,
    pub preset: usize,
    pub door_style: DoorStyle,
    pub window_style: WindowStyle,
    pub roof_shape: RoofShape,
    pub area_kind: FeatureKind,
    pub line_kind: FeatureKind,
    pub building_kind: usize,
    pub snap: bool,
    pub show_dims: bool,
    /// Which other floors to show faintly under each floor, once the person
    /// has picked (otherwise the floor next to it; see `reference_levels`).
    pub under: HashMap<LevelId, Vec<LevelId>>,
    pub fit_request: bool,
    view: View,
    fitted: bool,
    fitted_level: Option<LevelId>,
    /// Canvas size the last automatic fit was for.
    fit_size: Option<egui::Vec2>,
    /// Set once the person pans/zooms, so we stop auto-fitting.
    touched: bool,
    pending_center: Option<Vec2>,
    zoom_request: Option<f32>,
    draft: Draft,
    drag: Option<DragOp>,
    last_world: Option<Vec2>,
    /// Alignment guides the pointer is snapped to this frame (point, direction).
    guides: Vec<(Vec2, Vec2)>,
}

impl Default for PlanState {
    fn default() -> Self {
        Self {
            tool: Tool::Select,
            layer: Layer::Floor,
            preset: 0,
            door_style: DoorStyle::Hinged,
            window_style: WindowStyle::DoubleHung,
            roof_shape: RoofShape::Gable,
            area_kind: FeatureKind::Deck,
            line_kind: FeatureKind::Fence,
            building_kind: 0,
            snap: true,
            show_dims: false,
            under: HashMap::new(),
            fit_request: false,
            view: View { offset: egui::Vec2::ZERO, zoom: 50.0 },
            fitted: false,
            fitted_level: None,
            fit_size: None,
            touched: false,
            pending_center: None,
            zoom_request: None,
            draft: Draft::None,
            drag: None,
            last_world: None,
            guides: Vec::new(),
        }
    }
}

impl PlanState {
    /// Scroll the view so `p` is centred.
    pub fn reveal(&mut self, p: Vec2) {
        self.pending_center = Some(p);
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.draft = Draft::None;
        self.drag = None;
    }
}

fn place_of(ctx: Ctx) -> Place {
    match ctx {
        Ctx::Floor => Place::Indoor,
        Ctx::Roof => Place::Roof,
        Ctx::Site => Place::Outdoor,
    }
}

/// Whether vehicles can be parked on this floor: an outbuilding (a garage,
/// say) or a floor with a room that looks like a garage or carport.
fn has_parking(project: &Project, level: &Level) -> bool {
    let garage = |s: &str| {
        let s = s.to_lowercase();
        s.contains("garage") || s.contains("carport") || s.contains("parking")
    };
    level.building.is_some_and(|b| project.buildings.iter().any(|x| x.id == b && (garage(&x.kind) || garage(&x.name))))
        || level.rooms.values().any(|r| garage(&r.kind) || garage(&r.name))
}

// ---------------------------------------------------------------------------
// Helpers

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn grid_step(units: UnitSystem, zoom: f32) -> f64 {
    let steps = units.grid_steps();
    steps
        .iter()
        .copied()
        .find(|s| *s as f32 * zoom >= 14.0)
        .unwrap_or(*steps.last().unwrap())
}

fn major_step(units: UnitSystem, minor: f64) -> f64 {
    units
        .grid_steps()
        .iter()
        .copied()
        .find(|s| *s >= minor * 3.9)
        .unwrap_or(minor * 10.0)
}

fn snap_grid(p: Vec2, step: f64) -> Vec2 {
    Vec2::new((p.x / step).round() * step, (p.y / step).round() * step)
}

/// How far to move a room so it lines up with its neighbours: the nearest of
/// its corners snaps onto another room's corner (or failing that, onto a
/// wall) within `radius`; otherwise its first corner snaps to the grid.
/// `below` are the floors shown underneath: their corners and walls count
/// too, so an upstairs room can be lined up with the floor below.
fn snap_room_move(level: &Level, below: &[&Level], id: RoomId, base: &[Vec2], raw: Vec2, radius: f64, grid: Option<f64>) -> Vec2 {
    let Some(room) = level.rooms.get(&id) else { return raw };
    let consider = |best: &mut Option<(f64, Vec2)>, d: f64, fix: Vec2| {
        if d < radius && best.is_none_or(|(bd, _)| d < bd) {
            *best = Some((d, fix));
        }
    };
    let mut best: Option<(f64, Vec2)> = None;
    for c in base {
        let q = *c + raw;
        for n in level.nodes.values().filter(|n| !room.nodes.contains(&n.id)) {
            consider(&mut best, q.distance(n.pos), n.pos - q);
        }
        for n in below.iter().flat_map(|l| l.nodes.values()) {
            consider(&mut best, q.distance(n.pos), n.pos - q);
        }
    }
    if best.is_none() {
        let own = level.walls.values().filter(|w| !room.nodes.contains(&w.a)).filter_map(|w| level.wall_points(w));
        let under = below.iter().flat_map(|l| l.walls.values().filter_map(|w| l.wall_points(w)));
        for (a, b) in own.chain(under) {
            for c in base {
                let q = *c + raw;
                let (p, _) = closest_point_on_segment(q, a, b);
                consider(&mut best, q.distance(p), p - q);
            }
        }
    }
    match (best, grid, base.first()) {
        (Some((_, fix)), _, _) => raw + fix,
        (None, Some(step), Some(c0)) => snap_grid(*c0 + raw, step) - *c0,
        _ => raw,
    }
}

/// Ground floors of the house and each outbuilding: drawn for reference on
/// the Outside plan.
fn context_levels(project: &Project) -> Vec<usize> {
    let mut ids: Vec<LevelId> = project.ground_level(None).map(|l| l.id).into_iter().collect();
    for b in &project.buildings {
        ids.extend(project.ground_level(Some(b.id)).map(|l| l.id));
    }
    ids.iter()
        .filter_map(|id| project.levels.iter().position(|l| l.id == *id))
        .collect()
}

/// Other floors of the same building that have rooms, top floor first:
/// the ones that can be shown underneath this one.
fn reference_candidates(project: &Project, li: usize) -> Vec<&Level> {
    let level = &project.levels[li];
    if level.is_site() {
        return Vec::new();
    }
    let mut v: Vec<&Level> = project.floor_stack(level.building).into_iter().filter(|l| l.id != level.id && !l.rooms.is_empty()).collect();
    v.reverse();
    v
}

/// Floors drawn faintly under this one so its rooms can be lined up with
/// them, farthest first. Unless the person has chosen, that's the floor
/// directly below (or, for a basement, the one above).
fn reference_levels(project: &Project, li: usize, st: &PlanState) -> Vec<usize> {
    let level = &project.levels[li];
    if level.is_site() {
        return Vec::new();
    }
    let ids: Vec<LevelId> = match st.under.get(&level.id) {
        Some(chosen) => chosen.clone(),
        None => {
            // A basement (bottom floor below ground) shows the floor above it.
            let (below, above) = project.neighbour_levels(level.id);
            let above = above.filter(|_| level.elevation < -0.01);
            below.or(above).filter(|l| !l.rooms.is_empty()).map(|l| l.id).into_iter().collect()
        }
    };
    let gap = |l: &Level| (l.elevation - level.elevation).abs();
    let mut v: Vec<&Level> = ids.iter().filter_map(|id| project.level(*id)).filter(|l| l.building == level.building && !l.is_site()).collect();
    v.sort_by(|a, b| gap(b).total_cmp(&gap(a)));
    v.iter().filter_map(|l| project.levels.iter().position(|x| x.id == l.id)).collect()
}

/// The "show other floors underneath" button and its menu of floors.
fn reference_menu(ui: &mut Ui, project: &Project, li: usize, st: &mut PlanState) {
    let candidates = reference_candidates(project, li);
    if candidates.is_empty() {
        return;
    }
    let level = &project.levels[li];
    let shown: Vec<LevelId> = reference_levels(project, li, st).iter().map(|i| project.levels[*i].id).collect();
    let resp = theme::icon_button(ui, icon::STACK, "", !shown.is_empty());
    let popup = ui.make_persistent_id("plan-under-menu");
    let resp = if ui.memory(|m| m.is_popup_open(popup)) { resp } else { resp.on_hover_text("Show other floors underneath, to line rooms up with them") };
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    egui::popup::popup_above_or_below_widget(ui, popup, &resp, egui::AboveOrBelow::Above, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(230.0);
        ui.label(theme::caps("Show underneath"));
        ui.add_space(2.0);
        let mut picked = shown.clone();
        let mut changed = false;
        for l in &candidates {
            let mut on = picked.contains(&l.id);
            let side = if l.elevation > level.elevation { "above" } else { "below" };
            let text = format!("{}  ·  {side}", l.name);
            if ui.checkbox(&mut on, text).changed() {
                changed = true;
                if on {
                    picked.push(l.id);
                } else {
                    picked.retain(|x| *x != l.id);
                }
            }
        }
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            if ui.small_button("All below").clicked() {
                picked = candidates.iter().filter(|l| l.elevation < level.elevation).map(|l| l.id).collect();
                changed = true;
            }
            if ui.small_button("None").clicked() {
                picked.clear();
                changed = true;
            }
        });
        if changed {
            st.under.insert(level.id, picked);
        }
    });
}

/// Corners and edges to snap to.
#[derive(Default)]
struct SnapSet {
    points: Vec<Vec2>,
    segs: Vec<(Vec2, Vec2)>,
}

impl SnapSet {
    fn add_walls(&mut self, level: &Level) {
        self.points.extend(level.nodes.values().map(|n| n.pos));
        self.segs.extend(level.walls.values().filter_map(|w| level.wall_points(w)));
    }

    fn add_path(&mut self, pts: &[Vec2], closed: bool) {
        self.points.extend(pts.iter().copied());
        let n = pts.len();
        if n < 2 {
            return;
        }
        let count = if closed { n } else { n - 1 };
        for i in 0..count {
            self.segs.push((pts[i], pts[(i + 1) % n]));
        }
    }
}

fn snap_set(project: &Project, li: usize, ctx: Ctx, exclude: Option<EntityRef>, below: &[usize]) -> SnapSet {
    let level = &project.levels[li];
    let mut s = SnapSet::default();
    match ctx {
        Ctx::Floor => {
            s.add_walls(level);
            for bi in below {
                s.add_walls(&project.levels[*bi]);
            }
        }
        Ctx::Roof => {
            s.add_walls(level);
            for r in level.roofs.values().filter(|r| exclude != Some(EntityRef::RoofSection(r.id))) {
                s.add_path(&r.outline, true);
            }
        }
        Ctx::Site => {
            for ci in context_levels(project) {
                s.add_walls(&project.levels[ci]);
            }
            for f in level.features.values().filter(|f| exclude != Some(EntityRef::Feature(f.id))) {
                if f.kind.geom() != FeatureGeom::Point {
                    s.add_path(&f.points, f.kind.geom() == FeatureGeom::Area);
                }
            }
        }
    }
    s
}

/// Snap to a nearby corner, then a nearby edge, then the grid.
fn snap(set: &SnapSet, view: &View, rect: Rect, s: Pos2, grid_on: bool, step: f64) -> Vec2 {
    snap_with_guides(set, view, rect, s, grid_on, step).0
}

/// Snap to, in order: a nearby corner; a spot lined up with two things (e.g.
/// straight across from one corner and straight down from another); a nearby
/// wall; a spot lined up with one thing; the grid. Also returns the alignment
/// guides used, to draw.
fn snap_with_guides(set: &SnapSet, view: &View, rect: Rect, s: Pos2, grid_on: bool, step: f64) -> (Vec2, Vec<(Vec2, Vec2)>) {
    let w = view.to_world(rect, s);
    let near = set
        .points
        .iter()
        .map(|p| (p.distance(w), *p))
        .filter(|(d, _)| *d < 10.0 * view.px())
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, p)) = near {
        return (p, Vec::new());
    }
    let aligned = grid_on.then(|| guided(set, None, w, view.px(), Some(step)));
    if let Some((p, g)) = &aligned {
        if g.len() >= 2 {
            return (*p, g.clone());
        }
    }
    let g = if grid_on { snap_grid(w, step) } else { w };
    let seg = set
        .segs
        .iter()
        .map(|(a, b)| (distance_to_segment(w, *a, *b), *a, *b))
        .filter(|(d, _, _)| *d < 7.0 * view.px())
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, a, b)) = seg {
        let len = a.distance(b);
        if len > 1e-9 {
            let dir = (b - a) / len;
            let along = if grid_on { (g - a).dot(dir) } else { (w - a).dot(dir) };
            return (a + dir * along.clamp(0.0, len), Vec::new());
        }
    }
    if let Some((p, guides)) = aligned {
        if !guides.is_empty() {
            return (p, guides);
        }
    }
    (g, Vec::new())
}


/// An infinite guide line: a point on it and its unit direction.
type Guide = (Vec2, Vec2);

/// Line up with existing walls and corners.
///
/// - With a previous point (`from`), the new segment locks to the nearest
///   direction already used by a wall (or horizontal/vertical), so you can
///   carry on from an angled wall. Along that direction it snaps to where it
///   crosses a wall's extension or a corner's horizontal/vertical line, else
///   to a grid length.
/// - Without one, it snaps to where two guide lines cross, or onto the
///   nearest one.
///
/// Returns the point and the guide lines (point, unit direction) it used.
fn guided(set: &SnapSet, from: Option<Vec2>, raw: Vec2, px: f64, grid: Option<f64>) -> (Vec2, Vec<(Vec2, Vec2)>) {
    // Guide lines: every wall's extension, plus horizontal and vertical
    // through every corner.
    let mut lines: Vec<(Vec2, Vec2)> = Vec::new();
    let mut dirs: Vec<Vec2> = vec![Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)];
    let add_dir = |dirs: &mut Vec<Vec2>, d: Vec2| {
        if !dirs.iter().any(|e| e.cross(d).abs() < 0.01) {
            dirs.push(d);
        }
    };
    for (a, b) in &set.segs {
        let len = a.distance(*b);
        if len < 1e-6 {
            continue;
        }
        let d = (*b - *a) / len;
        lines.push((*a, d));
        add_dir(&mut dirs, d);
        add_dir(&mut dirs, Vec2::new(-d.y, d.x));
    }
    let mut seen: Vec<Vec2> = Vec::new();
    for p in &set.points {
        if seen.iter().any(|q| q.distance(*p) < 1e-6) {
            continue;
        }
        seen.push(*p);
        lines.push((*p, Vec2::new(1.0, 0.0)));
        lines.push((*p, Vec2::new(0.0, 1.0)));
    }
    let reach = 10.0 * px;
    // Where line (p, d) meets line (q, e).
    let meet = |p: Vec2, d: Vec2, q: Vec2, e: Vec2| -> Option<Vec2> {
        let den = d.cross(e);
        if den.abs() < 1e-9 {
            return None;
        }
        Some(p + d * ((q - p).cross(e) / den))
    };
    let dist_to_line = |x: Vec2, (q, e): (Vec2, Vec2)| (x - q).cross(e).abs();

    if let Some(from) = from {
        let v = raw - from;
        if v.length() < 1e-9 {
            return (raw, Vec::new());
        }
        let dir = dirs
            .iter()
            .copied()
            .max_by(|a, b| a.dot(v).abs().total_cmp(&b.dot(v).abs()))
            .unwrap_or(Vec2::new(1.0, 0.0));
        let dir = if dir.dot(v) < 0.0 { dir * -1.0 } else { dir };
        let along = v.dot(dir);
        let here = from + dir * along;
        let ray = (from, dir);
        // The nearest crossing with a guide line, if it's close to the cursor.
        let hit = lines
            .iter()
            .filter(|l| l.1.cross(dir).abs() > 0.01)
            .filter_map(|l| meet(from, dir, l.0, l.1).map(|x| (x.distance(here), x, *l)))
            .filter(|(d, x, _)| *d < reach && (*x - from).dot(dir) > 1e-6)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, x, l)) = hit {
            return (x, vec![ray, l]);
        }
        let t = match grid {
            Some(step) => (along / step).round() * step,
            None => along,
        };
        return (from + dir * t, vec![ray]);
    }

    // No previous point: two guides crossing near the cursor, else one.
    let near: Vec<(Vec2, Vec2)> = lines.iter().copied().filter(|l| dist_to_line(raw, *l) < reach).collect();
    let mut best: Option<(f64, Vec2, Guide, Guide)> = None;
    for (i, a) in near.iter().enumerate() {
        for b in &near[i + 1..] {
            if let Some(x) = meet(a.0, a.1, b.0, b.1) {
                let d = x.distance(raw);
                if d < reach && best.is_none_or(|(bd, ..)| d < bd) {
                    best = Some((d, x, *a, *b));
                }
            }
        }
    }
    if let Some((_, x, a, b)) = best {
        return (x, vec![a, b]);
    }
    if let Some(l) = near
        .iter()
        .copied()
        .filter(|l| dist_to_line(raw, *l) < 6.0 * px)
        .min_by(|a, b| dist_to_line(raw, *a).total_cmp(&dist_to_line(raw, *b)))
    {
        // Slide along the guide in grid steps (for a horizontal or vertical
        // guide this keeps the other coordinate on the grid).
        let g = grid.map_or(raw, |step| snap_grid(raw, step));
        let x = l.0 + l.1 * (g - l.0).dot(l.1);
        return (x, vec![l]);
    }
    (raw, Vec::new())
}

fn hit_vertices(pts: &[Vec2], w: Vec2, px: f64) -> Option<usize> {
    pts.iter().position(|p| p.distance(w) < 9.0 * px)
}

fn hit_test(project: &Project, li: usize, ctx: Ctx, sel: Option<EntityRef>, view: &View, rect: Rect, s: Pos2) -> Option<Hit> {
    let level = &project.levels[li];
    let w = view.to_world(rect, s);
    let px = view.px();
    match ctx {
        Ctx::Floor => {
            if let Some(EntityRef::Room(rid)) = sel {
                if let Some(room) = level.rooms.get(&rid) {
                    if !room.shape_locked {
                        for n in &room.nodes {
                            if level.node_pos(*n).is_some_and(|p| p.distance(w) < 9.0 * px) {
                                return Some(Hit::Node(*n));
                            }
                        }
                    }
                }
            }
            for o in level.openings.values() {
                if let Some((a, b, _)) = level.opening_segment(o) {
                    let t = level.walls.get(&o.wall).map(|w| w.thickness).unwrap_or(0.1);
                    if distance_to_segment(w, a, b) < (t / 2.0).max(6.0 * px) {
                        return Some(Hit::Opening(o.id));
                    }
                }
            }
            for o in level.objects.values().rev().filter(|o| o.roof.is_none()) {
                if point_in_polygon(w, &o.world_outline()) {
                    return Some(Hit::Object(o.id));
                }
            }
            let wall = level
                .walls
                .values()
                .filter_map(|wl| {
                    let (a, b) = level.wall_points(wl)?;
                    let d = distance_to_segment(w, a, b);
                    (d < (wl.thickness / 2.0).max(5.0 * px)).then_some((d, wl.id))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, id)) = wall {
                return Some(Hit::Wall(id));
            }
            level.room_at(w).map(Hit::Room)
        }
        Ctx::Roof => {
            if let Some(EntityRef::RoofSection(rid)) = sel {
                if let Some(i) = level.roofs.get(&rid).and_then(|r| hit_vertices(&r.outline, w, px)) {
                    return Some(Hit::RoofVertex(rid, i));
                }
            }
            for o in level.objects.values().rev().filter(|o| o.roof.is_some()) {
                if point_in_polygon(w, &o.world_outline()) {
                    return Some(Hit::Object(o.id));
                }
            }
            level.roof_at(w).map(Hit::Roof)
        }
        Ctx::Site => {
            if let Some(EntityRef::Feature(fid)) = sel {
                if let Some(f) = level.features.get(&fid) {
                    if f.kind.geom() != FeatureGeom::Point {
                        if let Some(i) = hit_vertices(&f.points, w, px) {
                            return Some(Hit::FeatureVertex(fid, i));
                        }
                    }
                }
            }
            for o in level.objects.values().rev() {
                if point_in_polygon(w, &o.world_outline()) {
                    return Some(Hit::Object(o.id));
                }
            }
            for f in level.features.values() {
                for g in &f.gates {
                    if let Some((p, _)) = point_along(&f.points, g.offset) {
                        if p.distance(w) < (g.width / 2.0).max(6.0 * px) && distance_to_polyline(&f.points, w) < 8.0 * px {
                            return Some(Hit::Gate(f.id, g.id));
                        }
                    }
                }
            }
            for f in level.features.values().filter(|f| f.kind.geom() == FeatureGeom::Point) {
                let r = (f.size / 2.0).max(8.0 * px);
                if f.points.first().is_some_and(|c| c.distance(w) <= r) {
                    return Some(Hit::Feature(f.id));
                }
            }
            for f in level.features.values().filter(|f| f.kind.geom() == FeatureGeom::Line) {
                if distance_to_polyline(&f.points, w) < (f.line_width() / 2.0).max(6.0 * px) {
                    return Some(Hit::Feature(f.id));
                }
            }
            for b in &project.buildings {
                if project.ground_level(Some(b.id)).is_some_and(|l| l.room_at(w).is_some()) {
                    return Some(Hit::Building(b.id));
                }
            }
            if project.ground_level(None).is_some_and(|l| l.room_at(w).is_some()) {
                return Some(Hit::House);
            }
            let area = level
                .features
                .values()
                .filter(|f| f.kind.geom() == FeatureGeom::Area && f.kind != FeatureKind::PropertyLine && f.contains(w))
                .min_by(|a, b| a.area().total_cmp(&b.area()));
            if let Some(f) = area {
                return Some(Hit::Feature(f.id));
            }
            level
                .features
                .values()
                .filter(|f| f.kind == FeatureKind::PropertyLine)
                .find(|f| {
                    let mut closed = f.points.clone();
                    closed.extend(f.points.first().copied());
                    distance_to_polyline(&closed, w) < 6.0 * px
                })
                .map(|f| Hit::Feature(f.id))
        }
    }
}

fn fit_points(project: &Project, li: usize, ctx: Ctx) -> Vec<Vec2> {
    let level = &project.levels[li];
    let mut pts = level.extent_points();
    if ctx == Ctx::Site {
        for ci in context_levels(project) {
            pts.extend(project.levels[ci].nodes.values().map(|n| n.pos));
        }
    }
    if let Some(u) = &level.underlay {
        let [w, h] = u.pixel_size;
        for (x, y) in [(0, 0), (w, 0), (w, h), (0, h)] {
            pts.push(u.pixel_to_world(Vec2::new(x as f64, y as f64)));
        }
    }
    pts
}

fn fit_view(st: &mut PlanState, area: Rect, canvas: Rect, pts: &[Vec2]) {
    let center_px = area.center() - canvas.min;
    match bounds(pts) {
        Some((lo, hi)) => {
            let size = (hi - lo).max(Vec2::new(1.0, 1.0));
            // Leave room for the floating toolbars above and below.
            let zoom = ((area.width() - 96.0) / size.x as f32)
                .min((area.height() - 190.0) / size.y as f32)
                .clamp(4.0, 400.0);
            let c = (lo + hi) / 2.0;
            st.view.zoom = zoom;
            st.view.offset = center_px - vec2(c.x as f32, c.y as f32) * zoom;
        }
        None => {
            st.view.zoom = 50.0;
            st.view.offset = center_px;
        }
    }
}

/// Text centred on `pos`, rotated to follow a line at `angle` (kept upright).
fn text_along(painter: &Painter, pos: Pos2, angle: f32, text: String, font: FontId, color: Color32) {
    use std::f32::consts::{FRAC_PI_2, PI};
    let mut a = angle;
    if a > FRAC_PI_2 {
        a -= PI;
    } else if a < -FRAC_PI_2 {
        a += PI;
    }
    let galley = painter.layout_no_wrap(text, font, color);
    let half = galley.size() / 2.0;
    let offset = egui::emath::Rot2::from_angle(a) * vec2(-half.x, -half.y);
    painter.add(egui::epaint::TextShape::new(pos + offset, galley, color).with_angle(a));
}

/// Parallel lines filling a polygon (deck boards, hatching).
fn hatch(poly: &[Vec2], dir: Vec2, spacing: f64) -> Vec<(Vec2, Vec2)> {
    let dir = dir.normalized();
    let n = dir.perp();
    let (lo, hi) = poly
        .iter()
        .map(|p| n.dot(*p))
        .fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v), b.max(v)));
    let mut out = Vec::new();
    if spacing <= 0.0 || (hi - lo) / spacing > 1500.0 {
        return out;
    }
    let mut t = (lo / spacing).ceil() * spacing;
    let len = poly.len();
    while t < hi {
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..len {
            let (a, b) = (poly[i], poly[(i + 1) % len]);
            let (fa, fb) = (n.dot(a) - t, n.dot(b) - t);
            if (fa < 0.0) != (fb < 0.0) {
                xs.push(dir.dot(a.lerp(b, fa / (fa - fb))));
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.chunks(2) {
            if let [x0, x1] = pair {
                out.push((n * t + dir * *x0, n * t + dir * *x1));
            }
        }
        t += spacing;
    }
    out
}

/// A polyline with gaps cut out at `cuts` (ranges of distance along it).
fn polyline_pieces(pts: &[Vec2], cuts: &[(f64, f64)]) -> Vec<Vec<Vec2>> {
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

// ---------------------------------------------------------------------------
// Floating UI

fn float_area(id: &str) -> egui::Area {
    egui::Area::new(egui::Id::new(id)).order(egui::Order::Middle).constrain(true)
}

pub fn level_icon(l: &Level) -> &'static str {
    if l.is_site() {
        icon::TREE
    } else if l.building.is_some() {
        icon::WAREHOUSE
    } else {
        icon::STACK
    }
}

fn level_picker(ui: &mut Ui, project: &Project, env: &mut Env) {
    let current = project
        .level(*env.active_level)
        .map(|l| format!("{}  {}", level_icon(l), project.entity_name(EntityRef::Level(l.id))))
        .unwrap_or_default();
    theme::combo_box("plan-level").selected_text(current).show_ui(ui, |ui| {
        let mut row = |ui: &mut Ui, l: &Level| {
            let text = format!("{}  {}", level_icon(l), project.entity_name(EntityRef::Level(l.id)));
            ui.selectable_value(env.active_level, l.id, text);
        };
        for l in project.floor_stack(None).into_iter().rev() {
            row(ui, l);
        }
        for b in &project.buildings {
            for l in project.floor_stack(Some(b.id)).into_iter().rev() {
                row(ui, l);
            }
        }
        if let Some(l) = project.site() {
            ui.separator();
            row(ui, l);
        }
    });
}

fn toolbar(ctx: &egui::Context, project: &Project, env: &mut Env, st: &mut PlanState, area: Rect, pctx: Ctx) {
    float_area("plan-toolbar")
        .pivot(Align2::CENTER_TOP)
        .fixed_pos(pos2(area.center().x, area.top() + theme::MARGIN))
        .show(ctx, |ui| {
            theme::pill().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if pctx != Ctx::Site {
                        let before = st.layer;
                        theme::segmented(ui, &mut st.layer, &[(Layer::Floor, "Floor"), (Layer::Roof, "Roof")]);
                        if st.layer != before {
                            st.set_tool(Tool::Select);
                            *env.selection = None;
                        }
                        theme::vdivider(ui);
                    }
                    for (gi, group) in Tool::groups(pctx).iter().enumerate() {
                        if gi > 0 {
                            theme::vdivider(ui);
                        }
                        for tool in *group {
                            let key = tool.key().map(|k| format!("  ({})", k.name())).unwrap_or_default();
                            let tip = format!("{}{key}\n{}", tool.name(), tool.hint(pctx));
                            if theme::icon_button(ui, tool.icon(), &tip, st.tool == *tool).clicked() {
                                st.set_tool(*tool);
                            }
                        }
                    }
                    if project.level(*env.active_level).is_some_and(|l| l.underlay.is_some()) {
                        theme::vdivider(ui);
                        for tool in [Tool::Calibrate, Tool::MoveUnderlay] {
                            let tip = format!("{}\n{}", tool.name(), tool.hint(pctx));
                            if theme::icon_button(ui, tool.icon(), &tip, st.tool == tool).clicked() {
                                st.set_tool(tool);
                            }
                        }
                    }
                    theme::vdivider(ui);
                    level_picker(ui, project, env);
                });
            });
        });

    // Contextual options for placement tools, like a "mode" chip.
    let has_options = matches!(
        st.tool,
        Tool::Object | Tool::Door | Tool::Window | Tool::RectRoof | Tool::PolyRoof | Tool::RectArea | Tool::PolyArea | Tool::Line | Tool::Building
    );
    if !has_options {
        return;
    }
    float_area("plan-tool-options")
        .pivot(Align2::CENTER_TOP)
        .fixed_pos(pos2(area.center().x, area.top() + theme::MARGIN + 60.0))
        .show(ctx, |ui| {
            theme::pill().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(st.tool.icon()).color(c::forest()).size(16.0));
                    match st.tool {
                        Tool::Object => {
                            let place = place_of(pctx);
                            let spot = project.level(*env.active_level).map_or(Spot::default(), |l| Spot {
                                parking: has_parking(project, l),
                                outbuilding: l.building.is_some(),
                            });
                            if !OBJECT_PRESETS[st.preset].fits(place, spot) {
                                st.preset = 0;
                            }
                            ui.label(theme::muted("Place"));
                            theme::combo_box("plan-preset")
                                .selected_text(OBJECT_PRESETS[st.preset].label)
                                .height(420.0)
                                .show_ui(ui, |ui| {
                                    for (i, p) in OBJECT_PRESETS.iter().enumerate().filter(|(_, p)| p.fits(place, spot)) {
                                        ui.selectable_value(&mut st.preset, i, p.label);
                                    }
                                });
                        }
                        Tool::Door => {
                            ui.label(theme::muted("Door style"));
                            super::widgets::combo(ui, "plan-door", &mut st.door_style, &DoorStyle::ALL, DoorStyle::label);
                        }
                        Tool::Window => {
                            ui.label(theme::muted("Window style"));
                            super::widgets::combo(ui, "plan-window", &mut st.window_style, &WindowStyle::ALL, WindowStyle::label);
                        }
                        Tool::RectRoof | Tool::PolyRoof => {
                            ui.label(theme::muted("Shape"));
                            let opts: Vec<(RoofShape, &str)> = RoofShape::ALL.iter().map(|s| (*s, s.label())).collect();
                            theme::segmented(ui, &mut st.roof_shape, &opts);
                        }
                        Tool::RectArea | Tool::PolyArea => {
                            ui.label(theme::muted("Draw a"));
                            super::widgets::combo(ui, "plan-area", &mut st.area_kind, &FeatureKind::AREAS, FeatureKind::label);
                        }
                        Tool::Line => {
                            ui.label(theme::muted("Draw a"));
                            super::widgets::combo(ui, "plan-line", &mut st.line_kind, &FeatureKind::LINES, FeatureKind::label);
                        }
                        Tool::Building => {
                            ui.label(theme::muted("Building"));
                            theme::combo_box("plan-building")
                                .selected_text(BUILDING_KIND_SUGGESTIONS[st.building_kind])
                                .show_ui(ui, |ui| {
                                    for (i, k) in BUILDING_KIND_SUGGESTIONS.iter().enumerate() {
                                        ui.selectable_value(&mut st.building_kind, i, *k);
                                    }
                                });
                        }
                        _ => {}
                    }
                    if theme::icon_button_sized(ui, icon::X, "Done (Esc)", false, 28.0).clicked() {
                        st.set_tool(Tool::Select);
                    }
                });
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn bottom_bars(
    ctx: &egui::Context,
    project: &mut Project,
    env: &mut Env,
    st: &mut PlanState,
    area: Rect,
    li: usize,
    pctx: Ctx,
    hover_world: Option<Vec2>,
) {
    let units = project.info.units;
    // Left: snapping and dimensions
    float_area("plan-snap")
        .pivot(Align2::LEFT_BOTTOM)
        .fixed_pos(pos2(area.left() + theme::MARGIN, area.bottom() - theme::MARGIN))
        .show(ctx, |ui| {
            theme::pill().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if theme::icon_button(ui, icon::GRID_FOUR, "Snap to grid, corners and walls (hold Alt to override)", st.snap).clicked() {
                        st.snap = !st.snap;
                    }
                    if theme::icon_button(ui, icon::RULER, "Show all dimensions", st.show_dims).clicked() {
                        st.show_dims = !st.show_dims;
                    }
                    if pctx == Ctx::Floor {
                        reference_menu(ui, project, li, st);
                    }
                });
            });
        });

    // Centre: context info (editable size for an unlocked rectangular room)
    float_area("plan-info")
        .pivot(Align2::CENTER_BOTTOM)
        .fixed_pos(pos2(area.center().x, area.bottom() - theme::MARGIN))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Fixed size so it doesn't jump around as the coordinates change,
                // and the same height as the pills beside it.
                // Snap pill on the left and zoom pill on the right are each ~200 px.
                let pill_w = (area.width() - 2.0 * (200.0 + theme::MARGIN + 12.0)).clamp(240.0, 560.0);
                let field_w = if pill_w < 300.0 { 64.0 } else { 84.0 };
                theme::pill().show(ui, |ui| {
                    let layout = egui::Layout::left_to_right(egui::Align::Center);
                    ui.allocate_ui_with_layout(vec2(pill_w, 34.0), layout, |ui| {
                        ui.set_min_size(vec2(pill_w, 34.0));
                        ui.add_space(6.0);
                        let editing = match (*env.selection, pctx) {
                            (Some(EntityRef::Room(rid)), Ctx::Floor) => {
                                let level = &project.levels[li];
                                level
                                    .rooms
                                    .get(&rid)
                                    .filter(|r| !r.shape_locked)
                                    .and_then(|r| level.rect_dims(r))
                                    .map(|(_, w, d)| (rid, w, d))
                            }
                            _ => None,
                        };
                        if let Some((rid, w, d)) = editing {
                            let (mut w2, mut d2) = (w, d);
                            ui.label(theme::muted("Width"));
                            let a = length_edit_sized(ui, ("pill-w", rid), &mut w2, units, 1e-6, field_w);
                            theme::vdivider(ui);
                            ui.label(theme::muted("Depth"));
                            let b = length_edit_sized(ui, ("pill-d", rid), &mut d2, units, 1e-6, field_w);
                            if a || b {
                                let level_id = project.levels[li].id;
                                if let Err(e) = project.set_room_size(level_id, rid, w2, d2) {
                                    env.toasts.error(e.to_string());
                                }
                            }
                        } else {
                            let hint = st.tool.hint(pctx);
                            let text = match hover_world {
                                Some(w) => format!("{}, {}   ·   {hint}", format_length(w.x, units), format_length(w.y, units)),
                                None => hint.to_string(),
                            };
                            let max_w = pill_w - 18.0;
                            ui.scope(|ui| {
                                ui.set_max_width(max_w);
                                ui.add(egui::Label::new(egui::RichText::new(text).size(12.5).color(c::muted())).truncate());
                            });
                        }
                        ui.add_space(6.0);
                    });
                });
            });
        });

    // Right: zoom
    float_area("plan-zoom")
        .pivot(Align2::RIGHT_BOTTOM)
        .fixed_pos(pos2(area.right() - theme::MARGIN, area.bottom() - theme::MARGIN))
        .show(ctx, |ui| {
            theme::pill().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if theme::icon_button(ui, icon::MINUS, "Zoom out", false).clicked() {
                        st.zoom_request = Some(1.0 / 1.25);
                    }
                    ui.add_sized(
                        [44.0, 30.0],
                        egui::Label::new(egui::RichText::new(format!("{:.0}%", st.view.zoom / 50.0 * 100.0)).size(12.5).color(c::muted())),
                    );
                    if theme::icon_button(ui, icon::PLUS, "Zoom in", false).clicked() {
                        st.zoom_request = Some(1.25);
                    }
                    if theme::icon_button(ui, icon::CORNERS_OUT, "Fit to window", false).clicked() {
                        st.fit_request = true;
                    }
                });
            });
        });
}

fn empty_state(ctx: &egui::Context, env: &mut Env, st: &mut PlanState, area: Rect, level: LevelId, pctx: Ctx) {
    float_area("plan-empty")
        .pivot(Align2::CENTER_CENTER)
        .fixed_pos(area.center())
        .show(ctx, |ui| {
            theme::panel().inner_margin(egui::Margin::same(28)).show(ui, |ui| {
                ui.set_width(380.0);
                let (i, title, body) = match pctx {
                    Ctx::Floor => (
                        icon::HOUSE_LINE,
                        "Start with a room",
                        "Drag out rectangles for each room. Rooms that touch share their walls, so the house builds itself up as you go.",
                    ),
                    Ctx::Roof => (
                        icon::TRIANGLE,
                        "Add the roof",
                        "Draw each roof section over the walls it covers: the main roof, a porch roof, a garage roof. Each gets its own shape, pitch and layers of roofing.",
                    ),
                    Ctx::Site => (
                        icon::TREE,
                        "Map the outside",
                        "Draw the property line, then add decks, patios, fences, trees and outbuildings around the house.",
                    ),
                };
                theme::icon_tile(ui, i, c::coral(), c::coral_soft());
                ui.add_space(6.0);
                ui.label(theme::title(title));
                ui.label(theme::muted(body));
                ui.add_space(8.0);
                ui.horizontal(|ui| match pctx {
                    Ctx::Floor => {
                        if theme::primary_button(ui, format!("{}  Draw a room", icon::SQUARE)).clicked() {
                            st.set_tool(Tool::RectRoom);
                        }
                        if theme::secondary_button(ui, format!("{}  Trace a blueprint", icon::IMAGE_SQUARE)).clicked() {
                            env.actions.push(Action::ImportUnderlay(level));
                        }
                    }
                    Ctx::Roof => {
                        if theme::primary_button(ui, format!("{}  Draw a roof section", icon::SQUARE)).clicked() {
                            st.set_tool(Tool::RectRoof);
                        }
                    }
                    Ctx::Site => {
                        if theme::primary_button(ui, format!("{}  Property line", icon::BOUNDING_BOX)).clicked() {
                            st.set_tool(Tool::PropertyLine);
                        }
                        if theme::secondary_button(ui, format!("{}  Add a deck", icon::SQUARE)).clicked() {
                            st.area_kind = FeatureKind::Deck;
                            st.set_tool(Tool::RectArea);
                        }
                    }
                });
            });
        });
}

// ---------------------------------------------------------------------------

pub fn plan_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, st: &mut PlanState, insets: Insets) {
    if project.level(*env.active_level).is_none() {
        *env.active_level = project.first_house_level();
    }
    let level_id = *env.active_level;
    let li = project.levels.iter().position(|l| l.id == level_id).unwrap();
    let pctx = if project.levels[li].is_site() {
        Ctx::Site
    } else if st.layer == Layer::Roof {
        Ctx::Roof
    } else {
        Ctx::Floor
    };
    let no_underlay = project.levels[li].underlay.is_none();
    if !st.tool.fits(pctx) || (matches!(st.tool, Tool::Calibrate | Tool::MoveUnderlay) && no_underlay) {
        st.set_tool(Tool::Select);
    }
    if st.fitted_level != Some(level_id) {
        let first = st.fitted_level.is_none();
        st.fitted_level = Some(level_id);
        if !first {
            st.fitted = false;
            st.touched = false;
        }
        st.draft = Draft::None;
    }

    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = resp.rect;
    let area = insets.apply(rect);
    let resized = st.fit_size.is_none_or(|s| (s - area.size()).length() > 1.0);
    if !st.fitted || st.fit_request || (resized && !st.touched) {
        let mut pts = fit_points(project, li, pctx);
        // Upstairs, frame the floor underneath too, so it's all in view.
        if pctx == Ctx::Floor {
            for bi in reference_levels(project, li, st) {
                pts.extend(fit_points(project, bi, Ctx::Floor));
            }
        }
        fit_view(st, area, rect, &pts);
        st.fitted = true;
        st.fit_request = false;
        st.fit_size = Some(area.size());
    }
    if let Some(c) = st.pending_center.take() {
        st.touched = true;
        st.view.offset = (area.center() - rect.min) - vec2(c.x as f32, c.y as f32) * st.view.zoom;
    }
    if let Some(f) = st.zoom_request.take() {
        st.touched = true;
        let anchor = area.center();
        let before = st.view.to_world(rect, anchor);
        st.view.zoom = (st.view.zoom * f).clamp(4.0, 1500.0);
        st.view.offset += anchor - st.view.to_screen(rect, before);
    }
    let units = project.info.units;

    // ---- zoom & pan
    let hover = resp.hover_pos();
    let space = ui.input(|i| i.key_down(Key::Space));
    if let Some(h) = hover {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
        let factor = zoom * (scroll.y * 0.0025).exp();
        if (factor - 1.0).abs() > 1e-4 {
            st.touched = true;
            let before = st.view.to_world(rect, h);
            st.view.zoom = (st.view.zoom * factor).clamp(4.0, 1500.0);
            let after = st.view.to_screen(rect, before);
            st.view.offset += h - after;
        }
    }
    if resp.dragged_by(PointerButton::Secondary)
        || resp.dragged_by(PointerButton::Middle)
        || (space && resp.dragged_by(PointerButton::Primary))
    {
        st.touched = true;
        st.view.offset += resp.drag_delta();
    }

    let grid = grid_step(units, st.view.zoom);
    let (alt, shift) = ui.input(|i| (i.modifiers.alt, i.modifiers.shift));
    let snap_on = st.snap && !alt;
    let dragging = match st.drag {
        Some(DragOp::RoofVertex(r, _)) => Some(EntityRef::RoofSection(r)),
        Some(DragOp::FeatureVertex(f, _)) => Some(EntityRef::Feature(f)),
        _ => None,
    };
    let below = if pctx == Ctx::Floor { reference_levels(project, li, st) } else { Vec::new() };
    let mut snaps = snap_set(project, li, pctx, dragging, &below);
    // A corner being dragged shouldn't snap to itself or its own walls.
    if let Some(DragOp::Node { node, .. }) = st.drag {
        if let Some(np) = project.levels[li].node_pos(node) {
            snaps.points.retain(|p| p.distance(np) > 1e-9);
            snaps.segs.retain(|(a, b)| a.distance(np) > 1e-9 && b.distance(np) > 1e-9);
        }
    }
    // The shape being drawn: line up with its own earlier corners too.
    let draft_last = match &st.draft {
        Draft::Poly { points } => {
            snaps.points.extend(points.iter().copied());
            snaps.segs.extend(points.windows(2).map(|w| (w[0], w[1])));
            points.last().copied()
        }
        _ => None,
    };
    st.guides.clear();
    let mut hover_snapped = None;
    if let Some(h) = hover {
        let (mut p, mut g) = snap_with_guides(&snaps, &st.view, rect, h, snap_on, grid);
        // Shift: straight on from the last corner, in any wall's direction.
        if let (true, Some(last)) = (shift, draft_last) {
            (p, g) = guided(&snaps, Some(last), st.view.to_world(rect, h), st.view.px(), snap_on.then_some(grid));
        }
        hover_snapped = Some(p);
        st.guides = g;
    }
    if let Some(w) = hover_snapped {
        st.last_world = Some(w);
    }
    let latest = ui.input(|i| i.pointer.latest_pos());
    let pointer_world = latest.map(|p| st.view.to_world(rect, p));
    let pointer_snapped = latest.map(|p| {
        let (p, g) = snap_with_guides(&snaps, &st.view, rect, p, snap_on, grid);
        if hover.is_none() {
            st.guides = g;
        }
        p
    });
    let press_origin = ui.input(|i| i.pointer.press_origin());
    let primary_start = resp.drag_started_by(PointerButton::Primary) && !space;
    let primary_drag = resp.dragged_by(PointerButton::Primary) && !space;
    let drag_end = resp.drag_stopped();
    let clicked = resp.clicked();

    // ---- tools
    match st.tool {
        Tool::Select => {
            if primary_start {
                if let Some(origin) = press_origin {
                    let hit = hit_test(project, li, pctx, *env.selection, &st.view, rect, origin);
                    let origin_world = st.view.to_world(rect, origin);
                    let sel = *env.selection;
                    st.drag = Some(match hit {
                        Some(Hit::Opening(id)) => {
                            env.select(EntityRef::Opening(id));
                            DragOp::Opening(id)
                        }
                        Some(Hit::Object(id)) => {
                            env.select(EntityRef::Object(id));
                            let pos = project.levels[li].objects[&id].position;
                            DragOp::Object { id, grab: origin_world - pos }
                        }
                        Some(Hit::Node(n)) => match (sel, project.levels[li].node_pos(n)) {
                            (Some(EntityRef::Room(room)), Some(start)) => {
                                // Locked neighbours keep their corner; we drag our own copy.
                                let level_id = project.levels[li].id;
                                let node = project.begin_corner_move(level_id, room, n);
                                DragOp::Node { node, room, start }
                            }
                            _ => DragOp::Pan,
                        },
                        Some(Hit::RoofVertex(r, i)) => DragOp::RoofVertex(r, i),
                        Some(Hit::FeatureVertex(f, i)) => DragOp::FeatureVertex(f, i),
                        Some(Hit::Gate(f, g)) => {
                            env.select(EntityRef::Feature(f));
                            DragOp::Gate(f, g)
                        }
                        Some(Hit::Roof(r)) if sel == Some(EntityRef::RoofSection(r)) => DragOp::MoveRoof(r, origin_world),
                        Some(Hit::Feature(f)) if sel == Some(EntityRef::Feature(f)) => DragOp::MoveFeature(f, origin_world),
                        Some(Hit::Room(r)) if sel == Some(EntityRef::Room(r)) => {
                            let level = &project.levels[li];
                            match level.rooms[&r].nodes.first().and_then(|n| level.node_pos(*n)) {
                                Some(anchor) if level.is_freestanding(r) => DragOp::MoveRoom { id: r, grab: origin_world, anchor },
                                _ => {
                                    env.toasts.info(
                                        "This room shares walls with other rooms, so it can't move on its own. Unlock it to drag its corners instead.",
                                    );
                                    DragOp::Pan
                                }
                            }
                        }
                        _ => DragOp::Pan,
                    });
                }
            }
            if primary_drag {
                match (st.drag, pointer_world) {
                    (Some(DragOp::Pan), _) => {
                        st.touched = true;
                        st.view.offset += resp.drag_delta();
                    }
                    (Some(DragOp::Object { id, grab }), Some(p)) => {
                        let mut target = p - grab;
                        if snap_on {
                            target = snap_grid(target, grid / 2.0);
                        }
                        if let Some(o) = project.levels[li].objects.get_mut(&id) {
                            o.position = target;
                        }
                    }
                    (Some(DragOp::Opening(id)), Some(p)) => {
                        let level = &project.levels[li];
                        let offset = level.openings.get(&id).and_then(|o| {
                            let (a, b) = level.wall_points(level.walls.get(&o.wall)?)?;
                            let (_, t) = closest_point_on_segment(p, a, b);
                            let off = t * a.distance(b);
                            Some(if snap_on { (off / grid).round() * grid } else { off })
                        });
                        if let Some(off) = offset {
                            let level = &mut project.levels[li];
                            level.openings.get_mut(&id).unwrap().offset = off;
                            level.clamp_openings();
                        }
                    }
                    (Some(DragOp::Node { node: n, .. }), Some(p)) => {
                        let p = if snap_on { pointer_snapped.unwrap_or(p) } else { p };
                        project.levels[li].move_node(n, p);
                    }
                    (Some(DragOp::RoofVertex(r, i)), Some(_)) => {
                        if let (Some(roof), Some(p)) = (project.levels[li].roofs.get_mut(&r), pointer_snapped) {
                            if let Some(v) = roof.outline.get_mut(i) {
                                *v = p;
                            }
                        }
                    }
                    (Some(DragOp::FeatureVertex(f, i)), Some(_)) => {
                        if let (Some(feat), Some(p)) = (project.levels[li].features.get_mut(&f), pointer_snapped) {
                            if let Some(v) = feat.points.get_mut(i) {
                                *v = p;
                            }
                            feat.clamp_gates();
                        }
                    }
                    (Some(DragOp::Gate(f, g)), Some(p)) => {
                        if let Some(feat) = project.levels[li].features.get_mut(&f) {
                            if let Some((s, _)) = nearest_on_polyline(&feat.points, p) {
                                let s = if snap_on { (s / grid).round() * grid } else { s };
                                if let Some(gate) = feat.gates.iter_mut().find(|x| x.id == g) {
                                    gate.offset = s;
                                }
                                feat.clamp_gates();
                            }
                        }
                    }
                    (Some(DragOp::MoveRoof(r, last)), Some(p)) => {
                        project.translate_roof(r, p - last);
                        st.drag = Some(DragOp::MoveRoof(r, p));
                    }
                    (Some(DragOp::MoveFeature(f, last)), Some(p)) => {
                        project.translate_feature(f, p - last);
                        st.drag = Some(DragOp::MoveFeature(f, p));
                    }
                    (Some(DragOp::MoveRoom { id, grab, anchor }), Some(p)) => {
                        let level = &project.levels[li];
                        if let Some(room) = level.rooms.get(&id) {
                            let now = room.nodes.first().and_then(|n| level.node_pos(*n)).unwrap_or(anchor) - anchor;
                            let base: Vec<Vec2> = level.room_points(room).into_iter().map(|c| c - now).collect();
                            let radius = 10.0 * st.view.px();
                            let want = snap_room_move(level, &below.iter().map(|bi| &project.levels[*bi]).collect::<Vec<_>>(), id, &base, p - grab, radius, snap_on.then_some(grid));
                            let _ = project.levels[li].translate_room(id, want - now);
                        }
                    }
                    _ => {}
                }
            }
            if drag_end {
                match st.drag {
                    Some(DragOp::Node { node, room, start }) => {
                        let level_id = project.levels[li].id;
                        if let Err(e) = project.finish_corner_move(level_id, room, node, start) {
                            env.toasts.info(format!("That corner went back: {e}."));
                        }
                    }
                    Some(DragOp::MoveRoom { id, anchor, .. }) => {
                        let level = &project.levels[li];
                        let moved = level.rooms.get(&id).and_then(|r| r.nodes.first()).and_then(|n| level.node_pos(*n));
                        if let Some(now) = moved {
                            let level_id = level.id;
                            if let Err(e) = project.settle_room(level_id, id, anchor - now) {
                                env.toasts.info(e.to_string());
                            }
                        }
                    }
                    Some(DragOp::Object { id, .. }) => {
                        if let Some(pos) = project.levels[li].objects.get(&id).map(|o| o.position) {
                            project.move_object(id, pos);
                        }
                    }
                    Some(DragOp::RoofVertex(r, _)) => {
                        if let Some(roof) = project.levels[li].roofs.get_mut(&r) {
                            roof.outline = positive_winding(&roof.outline);
                        }
                    }
                    Some(DragOp::FeatureVertex(f, _)) => {
                        if let Some(feat) = project.levels[li].features.get_mut(&f) {
                            if feat.kind.geom() == FeatureGeom::Area {
                                feat.points = positive_winding(&feat.points);
                            }
                        }
                    }
                    _ => {}
                }
                st.drag = None;
            }
            if clicked {
                let hit = hover.and_then(|h| hit_test(project, li, pctx, *env.selection, &st.view, rect, h));
                match hit {
                    Some(Hit::Node(_) | Hit::RoofVertex(..) | Hit::FeatureVertex(..)) => {}
                    Some(h) => *env.selection = h.entity(),
                    None => *env.selection = None,
                }
            }
            if resp.double_clicked() && pctx == Ctx::Site {
                let hit = hover.and_then(|h| hit_test(project, li, pctx, *env.selection, &st.view, rect, h));
                let open = match hit {
                    Some(Hit::Building(b)) => project.ground_level(Some(b)).map(|l| l.id),
                    Some(Hit::House) => project.ground_level(None).map(|l| l.id),
                    _ => None,
                };
                if let Some(l) = open {
                    *env.active_level = l;
                    st.layer = Layer::Floor;
                }
            }
        }
        Tool::RectRoom | Tool::RectRoof | Tool::RectArea | Tool::Building => {
            if primary_start {
                if let Some(o) = press_origin {
                    st.draft = Draft::Rect { start: snap(&snaps, &st.view, rect, o, snap_on, grid) };
                }
            }
            if drag_end {
                if let Draft::Rect { start } = st.draft {
                    st.draft = Draft::None;
                    let end = st.last_world.unwrap_or(start);
                    let (lo, hi) = (start.min(end), start.max(end));
                    let (w, h) = (hi.x - lo.x, hi.y - lo.y);
                    if w < 0.3 || h < 0.3 {
                        env.toasts.info("Drag out a bigger rectangle.");
                    } else {
                        finish_shape(project, env, st, level_id, rect_points(lo, w, h));
                    }
                }
            }
        }
        Tool::PolyRoom | Tool::PolyRoof | Tool::PolyArea | Tool::PropertyLine | Tool::Line => {
            if clicked {
                if let Some(p) = hover_snapped {
                    let mut finish = false;
                    let closes = st.tool.closes_shape();
                    match &mut st.draft {
                        Draft::Poly { points } => {
                            let first = st.view.to_screen(rect, points[0]);
                            let last = st.view.to_screen(rect, *points.last().unwrap());
                            let here = st.view.to_screen(rect, p);
                            let closing = closes && points.len() >= 3 && first.distance(here) < 10.0;
                            let ending = !closes && points.len() >= 2 && last.distance(here) < 6.0;
                            if closing || ending {
                                finish = true;
                            } else {
                                points.push(p);
                            }
                        }
                        _ => st.draft = Draft::Poly { points: vec![p] },
                    }
                    if finish {
                        finish_poly(project, env, st, level_id);
                    }
                }
            }
            if resp.double_clicked() {
                finish_poly(project, env, st, level_id);
            }
        }
        Tool::Door | Tool::Window => {
            if clicked {
                let candidate = hover.and_then(|h| {
                    project.levels[li].nearest_wall(st.view.to_world(rect, h), 14.0 * st.view.px())
                });
                match candidate {
                    Some((wid, off, _)) => {
                        let kind = if st.tool == Tool::Door {
                            OpeningKind::Door(st.door_style)
                        } else {
                            OpeningKind::Window(st.window_style)
                        };
                        let off = if snap_on { (off / grid).round() * grid } else { off };
                        if let Some(id) = project.add_opening(level_id, wid, kind, off) {
                            env.select(EntityRef::Opening(id));
                        }
                    }
                    None => env.toasts.info("Click on a wall to place it."),
                }
            }
        }
        Tool::Tree => {
            if clicked {
                if let Some(p) = hover_snapped {
                    if let Some(id) = project.add_feature(FeatureKind::Tree, vec![p]) {
                        env.select(EntityRef::Feature(id));
                    }
                }
            }
        }
        Tool::Gate => {
            if clicked {
                let target = hover.and_then(|h| {
                    let w = st.view.to_world(rect, h);
                    project.levels[li]
                        .features
                        .values()
                        .filter(|f| f.kind.geom() == FeatureGeom::Line)
                        .filter_map(|f| nearest_on_polyline(&f.points, w).map(|(s, d)| (d, s, f.id)))
                        .filter(|(d, _, _)| *d < 14.0 * st.view.px())
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                });
                match target {
                    Some((_, s, fid)) => {
                        let s = if snap_on { (s / grid).round() * grid } else { s };
                        if let Some(f) = project.levels[li].features.get_mut(&fid) {
                            f.gates.push(Gate::new(s));
                            f.clamp_gates();
                        }
                        env.select(EntityRef::Feature(fid));
                    }
                    None => env.toasts.info("Click on a fence (or wall, or hedge) to add a gate."),
                }
            }
        }
        Tool::Object => {
            if primary_start {
                if let Some(o) = press_origin {
                    st.draft = Draft::Object { start: snap(&snaps, &st.view, rect, o, snap_on, grid) };
                }
            }
            if drag_end {
                if let Draft::Object { start } = st.draft {
                    st.draft = Draft::None;
                    let end = st.last_world.unwrap_or(start);
                    let (lo, hi) = (start.min(end), start.max(end));
                    let (w, d) = (hi.x - lo.x, hi.y - lo.y);
                    let preset = &OBJECT_PRESETS[st.preset];
                    let (w, d) = if w < 0.1 || d < 0.1 { (preset.width, preset.depth) } else { (w, d) };
                    place_object(project, env, st, level_id, pctx, (lo + hi) / 2.0, w, d);
                }
            }
            if clicked {
                if let Some(p) = hover_snapped {
                    let preset = &OBJECT_PRESETS[st.preset];
                    place_object(project, env, st, level_id, pctx, p, preset.width, preset.depth);
                }
            }
        }
        Tool::Calibrate => {
            if clicked {
                if let Some(p) = hover.map(|h| st.view.to_world(rect, h)) {
                    match st.draft {
                        Draft::Calibrate { first } => {
                            env.dialogs.calibrate = Some(CalibrateDialog {
                                level: level_id,
                                a: first,
                                b: p,
                                text: String::new(),
                                error: None,
                            });
                            st.set_tool(Tool::Select);
                        }
                        _ => st.draft = Draft::Calibrate { first: p },
                    }
                }
            }
        }
        Tool::MoveUnderlay => {
            if primary_drag {
                if let Some(u) = &mut project.levels[li].underlay {
                    let d = resp.drag_delta() / st.view.zoom;
                    u.origin += Vec2::new(d.x as f64, d.y as f64);
                }
            }
        }
    }

    // ---- keyboard
    if !ui.ctx().wants_keyboard_input() {
        let (esc, enter, backspace, delete, no_mods) = ui.input(|i| {
            (
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Backspace),
                i.key_pressed(Key::Delete),
                !i.modifiers.command && !i.modifiers.ctrl && !i.modifiers.alt,
            )
        });
        if esc {
            if !matches!(st.draft, Draft::None) {
                st.draft = Draft::None;
            } else if st.tool != Tool::Select {
                st.set_tool(Tool::Select);
            } else {
                *env.selection = None;
            }
        }
        if enter && matches!(st.draft, Draft::Poly { .. }) {
            finish_poly(project, env, st, level_id);
        }
        if backspace {
            if let Draft::Poly { points } = &mut st.draft {
                points.pop();
                if points.is_empty() {
                    st.draft = Draft::None;
                }
            }
        }
        if (delete || backspace) && matches!(st.draft, Draft::None) && resp.hovered() {
            match *env.selection {
                Some(r @ (EntityRef::Opening(_) | EntityRef::Object(_) | EntityRef::Feature(_) | EntityRef::RoofSection(_))) => {
                    env.actions.push(Action::Delete(r))
                }
                Some(r @ (EntityRef::Room(_) | EntityRef::Building(_))) => env.dialogs.delete = Some(r),
                _ => {}
            }
        }
        if no_mods {
            for tool in Tool::groups(pctx).iter().flat_map(|g| g.iter()) {
                if let Some(k) = tool.key() {
                    if ui.input(|i| i.key_pressed(k)) {
                        st.set_tool(*tool);
                    }
                }
            }
        }
    }

    // ---- draw
    let hover_hit = if st.tool == Tool::Select && st.drag.is_none() {
        hover.and_then(|h| hit_test(project, li, pctx, *env.selection, &st.view, rect, h))
    } else {
        None
    };
    draw_background(ui, &painter, rect, project, li, env, st, grid);
    let d = Draw { painter: &painter, rect, view: st.view, units, sel: *env.selection, hover: hover_hit, show_dims: st.show_dims };
    match pctx {
        Ctx::Floor => {
            let over_blueprint = project.levels[li].underlay.as_ref().is_some_and(|u| u.visible);
            for bi in &below {
                d.floor(project, &project.levels[*bi], FloorStyle::Faded);
            }
            d.floor(project, &project.levels[li], FloorStyle::Edit { over_blueprint });
            for bi in &below {
                d.outline_below(&project.levels[*bi]);
            }
            d.items(&project.levels[li], |o| o.roof.is_none());
            d.room_selection(&project.levels[li]);
            d.room_labels(&project.levels[li]);
        }
        Ctx::Roof => {
            d.floor(project, &project.levels[li], FloorStyle::Faded);
            d.roofs(&project.levels[li]);
            d.items(&project.levels[li], |o| o.roof.is_some());
        }
        Ctx::Site => d.site(project, li),
    }
    draw_overlay(&painter, rect, project, li, st, hover, hover_snapped, units);
    if hover.is_some() {
        let selected_body = match (hover_hit, *env.selection) {
            (Some(Hit::Roof(r)), Some(EntityRef::RoofSection(s))) => r == s,
            (Some(Hit::Feature(f)), Some(EntityRef::Feature(s))) => f == s,
            (Some(Hit::Room(r)), Some(EntityRef::Room(s))) => r == s && project.levels[li].is_freestanding(r),
            _ => false,
        };
        resp.on_hover_cursor(match (st.tool, hover_hit) {
            _ if st.drag.is_some_and(|d| matches!(d, DragOp::Pan)) => egui::CursorIcon::Grabbing,
            (Tool::Select, _) if selected_body => egui::CursorIcon::Move,
            (
                Tool::Select,
                Some(Hit::Object(_) | Hit::Opening(_) | Hit::Node(_) | Hit::RoofVertex(..) | Hit::FeatureVertex(..) | Hit::Gate(..)),
            ) => egui::CursorIcon::Grab,
            (Tool::Select, _) => egui::CursorIcon::Default,
            (Tool::MoveUnderlay, _) => egui::CursorIcon::Move,
            _ => egui::CursorIcon::Crosshair,
        });
    }

    // ---- floating UI
    if !insets.overlays {
        return;
    }
    let ctx = ui.ctx().clone();
    toolbar(&ctx, project, env, st, area, pctx);
    let hover_world = hover.map(|h| st.view.to_world(rect, h));
    bottom_bars(&ctx, project, env, st, area, li, pctx, hover_world);
    let level = &project.levels[li];
    let empty = match pctx {
        Ctx::Floor => level.rooms.is_empty() && level.underlay.is_none(),
        Ctx::Roof => level.roofs.is_empty() && !level.rooms.is_empty(),
        Ctx::Site => level.features.is_empty() && level.underlay.is_none(),
    };
    if empty && st.tool == Tool::Select {
        empty_state(&ctx, env, st, area, level_id, pctx);
    }
}

fn unique_building_name(project: &Project, base: &str) -> String {
    let taken = |n: &str| project.buildings.iter().any(|b| b.name == n);
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|i| format!("{base} {i}")).find(|n| !taken(n)).unwrap()
}

/// Finish a rectangle or closed polygon for the active tool.
fn finish_shape(project: &mut Project, env: &mut Env, st: &mut PlanState, level: LevelId, points: Vec<Vec2>) {
    match st.tool {
        Tool::RectRoom | Tool::PolyRoom => {
            let name = project.next_room_name();
            match project.add_room(level, &name, &points) {
                Ok(id) => env.select(EntityRef::Room(id)),
                Err(e) => env.toasts.error(format!("Couldn't add room: {e}")),
            }
        }
        Tool::RectRoof | Tool::PolyRoof => match project.add_roof(level, &points, st.roof_shape) {
            Some(id) => env.select(EntityRef::RoofSection(id)),
            None => env.toasts.error("Couldn't add that roof section. Check the outline doesn't cross itself."),
        },
        Tool::RectArea | Tool::PolyArea | Tool::PropertyLine => {
            let kind = if st.tool == Tool::PropertyLine { FeatureKind::PropertyLine } else { st.area_kind };
            match project.add_feature(kind, points) {
                Some(id) => {
                    env.select(EntityRef::Feature(id));
                    if kind == FeatureKind::PropertyLine {
                        st.set_tool(Tool::Select);
                    }
                }
                None => env.toasts.error("Couldn't add that. Check the outline doesn't cross itself."),
            }
        }
        Tool::Building => {
            let kind = BUILDING_KIND_SUGGESTIONS[st.building_kind];
            let name = unique_building_name(project, kind);
            match project.add_building(&name, kind, &points) {
                Ok((b, _)) => {
                    env.select(EntityRef::Building(b));
                    env.toasts.info(format!("Added {name}. Double-click it to add doors, windows and what's stored inside."));
                    st.set_tool(Tool::Select);
                }
                Err(e) => env.toasts.error(format!("Couldn't add the building: {e}")),
            }
        }
        _ => {}
    }
}

fn finish_poly(project: &mut Project, env: &mut Env, st: &mut PlanState, level: LevelId) {
    let Draft::Poly { points } = std::mem::replace(&mut st.draft, Draft::None) else {
        return;
    };
    if st.tool == Tool::Line {
        if points.len() < 2 {
            env.toasts.info("Click at least two points.");
            return;
        }
        match project.add_feature(st.line_kind, points) {
            Some(id) => env.select(EntityRef::Feature(id)),
            None => env.toasts.info("That line is too short."),
        }
        return;
    }
    if points.len() < 3 {
        env.toasts.info("A shape needs at least three corners.");
        return;
    }
    finish_shape(project, env, st, level, points);
}

#[allow(clippy::too_many_arguments)]
fn place_object(project: &mut Project, env: &mut Env, st: &mut PlanState, level: LevelId, pctx: Ctx, center: Vec2, w: f64, d: f64) {
    let preset = &OBJECT_PRESETS[st.preset];
    let label = if preset.category == ObjectCategory::Other { "" } else { preset.label };
    let shape = Shape::new(preset.shape, w, d);
    let placed = match pctx {
        Ctx::Roof => project.add_roof_object(level, label, preset.category, shape, center),
        _ => project.add_object(level, label, preset.category, shape, center),
    };
    match placed {
        Some(id) => {
            env.select(EntityRef::Object(id));
            st.set_tool(Tool::Select);
        }
        None => env.toasts.info(match pctx {
            Ctx::Roof => "Roof items go on a roof section. Draw the roof first.",
            _ => "Items go inside a room. Draw the room first.",
        }),
    }
}

// ---------------------------------------------------------------------------
// Drawing

fn fill_polygon(painter: &Painter, world: &[Vec2], screen: &[Pos2], color: Color32) {
    let mut mesh = egui::Mesh::default();
    for p in screen {
        mesh.colored_vertex(*p, color);
    }
    for t in triangulate(world) {
        mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
    }
    painter.add(EShape::mesh(mesh));
}

fn grid_lines(painter: &Painter, rect: Rect, view: &View, step: f64, color: Color32) {
    let w0 = view.to_world(rect, rect.min);
    let w1 = view.to_world(rect, rect.max);
    let (x0, x1) = ((w0.x / step).floor() as i64, (w1.x / step).ceil() as i64);
    let (y0, y1) = ((w0.y / step).floor() as i64, (w1.y / step).ceil() as i64);
    if x1 - x0 > 400 || y1 - y0 > 400 {
        return;
    }
    let stroke = Stroke::new(1.0_f32, color);
    for i in x0..=x1 {
        let x = view.to_screen(rect, Vec2::new(i as f64 * step, 0.0)).x;
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke);
    }
    for i in y0..=y1 {
        let y = view.to_screen(rect, Vec2::new(0.0, i as f64 * step)).y;
        painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], stroke);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_background(ui: &Ui, painter: &Painter, rect: Rect, project: &Project, li: usize, env: &mut Env, st: &PlanState, grid: f64) {
    let level = &project.levels[li];
    let view = &st.view;
    let units = project.info.units;
    painter.rect_filled(rect, 0.0, c::canvas());
    grid_lines(painter, rect, view, grid, c::grid_minor());
    grid_lines(painter, rect, view, major_step(units, grid), c::grid_major());

    // Blueprint underlay
    if let Some(u) = &level.underlay {
        if u.visible {
            if let Some(tex) = env.textures.get(ui.ctx(), env.store, u.attachment, 4096) {
                let [w, h] = u.pixel_size;
                let corners = [(0, 0), (w, 0), (w, h), (0, h)]
                    .map(|(x, y)| view.to_screen(rect, u.pixel_to_world(Vec2::new(x as f64, y as f64))));
                let uvs = [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)];
                let color = Color32::from_white_alpha((u.opacity.clamp(0.0, 1.0) * 255.0) as u8);
                let mut mesh = egui::Mesh::with_texture(tex.id());
                for i in 0..4 {
                    mesh.vertices.push(egui::epaint::Vertex { pos: corners[i], uv: uvs[i], color });
                }
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                painter.add(EShape::mesh(mesh));
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum FloorStyle {
    /// The level you're editing.
    Edit { over_blueprint: bool },
    /// Under the roof layer: rooms and walls only, quiet.
    Faded,
    /// The house/an outbuilding on the Outside plan.
    Context { selected: bool, hovered: bool },
}

struct Draw<'a> {
    painter: &'a Painter,
    rect: Rect,
    view: View,
    units: UnitSystem,
    sel: Option<EntityRef>,
    hover: Option<Hit>,
    show_dims: bool,
}

impl Draw<'_> {
    fn to(&self, p: Vec2) -> Pos2 {
        self.view.to_screen(self.rect, p)
    }

    fn screen(&self, pts: &[Vec2]) -> Vec<Pos2> {
        pts.iter().map(|p| self.to(*p)).collect()
    }

    fn px(&self) -> f64 {
        self.view.px()
    }

    /// Lengths written along each edge, outside the shape.
    fn edge_dims(&self, pts: &[Vec2], closed: bool, color: Color32) {
        let n = pts.len();
        if n < 2 {
            return;
        }
        let count = if closed { n } else { n - 1 };
        for i in 0..count {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let len = a.distance(b);
            if len as f32 * self.view.zoom < 40.0 {
                continue;
            }
            let outward = -(b - a).perp().normalized();
            let pos = self.to(a.lerp(b, 0.5) + outward * (16.0 * self.px()));
            let (sa, sb) = (self.to(a), self.to(b));
            let angle = (sb.y - sa.y).atan2(sb.x - sa.x);
            text_along(self.painter, pos, angle, format_length(len, self.units), FontId::new(11.5, theme::medium()), color);
        }
    }

    fn handles(&self, pts: &[Vec2]) {
        for p in pts {
            let s = self.to(*p);
            self.painter.circle_filled(s, 5.5, c::panel());
            self.painter.circle_stroke(s, 5.5, Stroke::new(2.0_f32, c::teal()));
        }
    }

    /// Rooms, walls (cut at openings), paint, doors and windows.
    /// Dashed room outlines of the floor underneath, drawn over this floor's
    /// rooms so you can still see where its walls are.
    fn outline_below(&self, level: &Level) {
        let stroke = Stroke::new(1.5_f32, c::faint());
        for room in level.rooms.values() {
            let mut pts = self.screen(&level.room_points(room));
            if pts.len() < 3 {
                continue;
            }
            pts.push(pts[0]);
            for shape in EShape::dashed_line(&pts, stroke, 6.0, 4.0) {
                self.painter.add(shape);
            }
        }
    }

    fn floor(&self, project: &Project, level: &Level, style: FloorStyle) {
        let painter = self.painter;
        let px = self.px();
        let editing = matches!(style, FloorStyle::Edit { .. });

        let mut tint: HashMap<RoomId, [u8; 3]> = HashMap::new();
        if editing {
            for p in &project.paints {
                if let (PaintTarget::Room(r), PaintSurface::Walls, Some(c)) = (p.target, p.surface, p.swatch) {
                    tint.entry(r).or_insert(c);
                }
            }
        }

        for room in level.rooms.values() {
            let pts = level.room_points(room);
            if pts.len() < 3 {
                continue;
            }
            let fill = match style {
                FloorStyle::Edit { over_blueprint } => {
                    let f = if self.sel == Some(EntityRef::Room(room.id)) {
                        c::room_selected()
                    } else if self.hover == Some(Hit::Room(room.id)) {
                        c::room_hover()
                    } else if let Some(col) = tint.get(&room.id) {
                        theme::mix(c::room_fill(), rgb(*col), 0.16)
                    } else {
                        c::room_fill()
                    };
                    if over_blueprint { f.gamma_multiply(0.55) } else { f }
                }
                FloorStyle::Faded => c::context_fill(),
                FloorStyle::Context { selected, hovered } => {
                    if selected {
                        c::room_selected()
                    } else if hovered {
                        c::room_hover()
                    } else {
                        c::room_fill()
                    }
                }
            };
            fill_polygon(painter, &pts, &self.screen(&pts), fill);
        }

        for wall in level.walls.values() {
            let Some((a, b)) = level.wall_points(wall) else { continue };
            let len = a.distance(b);
            let dir = (b - a).normalized();
            let n = dir.perp();
            let ht = (wall.thickness / 2.0).max(px);
            let color = match style {
                FloorStyle::Edit { .. } => {
                    if self.sel == Some(EntityRef::Wall(wall.id)) {
                        c::teal()
                    } else if self.hover == Some(Hit::Wall(wall.id)) {
                        c::wall_hover()
                    } else {
                        c::wall()
                    }
                }
                FloorStyle::Faded => c::context_wall(),
                FloorStyle::Context { selected: true, .. } => c::teal(),
                FloorStyle::Context { .. } => c::wall(),
            };
            let mut cuts: Vec<(f64, f64)> = level
                .openings
                .values()
                .filter(|o| o.wall == wall.id)
                .map(|o| ((o.offset - o.width / 2.0).max(0.0), (o.offset + o.width / 2.0).min(len)))
                .collect();
            cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
            let piece = |s0: f64, s1: f64| {
                if s1 - s0 > 1e-6 {
                    let (p0, p1) = (a + dir * s0, a + dir * s1);
                    let quad = vec![self.to(p0 + n * ht), self.to(p1 + n * ht), self.to(p1 - n * ht), self.to(p0 - n * ht)];
                    painter.add(EShape::convex_polygon(quad, color, Stroke::NONE));
                }
            };
            let mut start = -ht;
            for (c0, c1) in cuts {
                if c0 > start {
                    piece(start, c0);
                }
                start = start.max(c1);
            }
            piece(start, len + ht);
        }

        if editing {
            for p in &project.paints {
                let PaintTarget::WallFace { wall, room } = p.target else { continue };
                let (Some(col), Some(w)) = (p.swatch, level.walls.get(&wall)) else { continue };
                let Some((a, b)) = level.wall_points(w) else { continue };
                let rooms = level.rooms_of_wall(w);
                let normal = match room {
                    Some(r) => level.wall_normal_into(w, r),
                    None => rooms.first().and_then(|r| level.wall_normal_into(w, *r)).map(|n| -n),
                };
                let Some(n) = normal else { continue };
                let off = n * (w.thickness / 2.0 + 3.0 * px);
                painter.line_segment([self.to(a + off), self.to(b + off)], Stroke::new(4.0_f32, rgb(col)));
            }
        }

        for o in level.openings.values() {
            let Some((s, e, dir)) = level.opening_segment(o) else { continue };
            let t = level.walls.get(&o.wall).map(|w| w.thickness).unwrap_or(0.1) / 2.0;
            let n = dir.perp();
            let gap = vec![self.to(s + n * t), self.to(e + n * t), self.to(e - n * t), self.to(s - n * t)];
            let selected = editing && self.sel == Some(EntityRef::Opening(o.id));
            let hovered = editing && self.hover == Some(Hit::Opening(o.id));
            let color = match style {
                FloorStyle::Faded => c::context_wall(),
                _ if selected => c::teal(),
                _ if hovered => c::wall_hover(),
                _ => c::wall(),
            };
            let thin = Stroke::new(if selected { 2.0 } else { 1.0_f32 }, color);
            match o.kind {
                OpeningKind::Window(_) => {
                    let glass = if style == FloorStyle::Faded { c::context_fill() } else { c::window_glass() };
                    painter.add(EShape::convex_polygon(gap, glass, thin));
                    painter.line_segment([self.to(s), self.to(e)], thin);
                }
                OpeningKind::Door(door) => {
                    painter.add(EShape::convex_polygon(gap, c::threshold(), Stroke::NONE));
                    if style == FloorStyle::Faded {
                        continue;
                    }
                    let swing = if o.flip_swing { -n } else { n };
                    match door {
                        DoorStyle::Hinged | DoorStyle::Other => {
                            if o.flip_hinge {
                                self.swing(e, s, swing, color, thin.width)
                            } else {
                                self.swing(s, e, swing, color, thin.width)
                            }
                        }
                        DoorStyle::Double => {
                            let mid = s.lerp(e, 0.5);
                            self.swing(s, mid, swing, color, thin.width);
                            self.swing(e, mid, swing, color, thin.width);
                        }
                        _ => {
                            let k = t * 0.45;
                            let mid = s.lerp(e, 0.5);
                            painter.line_segment([self.to(s + n * k), self.to(mid.lerp(e, 0.15) + n * k)], Stroke::new(2.0_f32, color));
                            painter.line_segment([self.to(mid.lerp(s, 0.15) - n * k), self.to(e - n * k)], Stroke::new(2.0_f32, color));
                        }
                    }
                }
            }
        }
    }

    /// A door or gate leaf with its swing arc.
    fn swing(&self, hinge: Vec2, latch: Vec2, swing: Vec2, color: Color32, width: f32) {
        let v0 = latch - hinge;
        let len = v0.length();
        let v1 = swing * len;
        let sign = if v0.cross(v1) >= 0.0 { 1.0 } else { -1.0 };
        let arc: Vec<Pos2> = (0..=24)
            .map(|k| self.to(hinge + v0.rotated(sign * std::f64::consts::FRAC_PI_2 * k as f64 / 24.0)))
            .collect();
        self.painter.add(EShape::line(arc, Stroke::new(1.0_f32, color.gamma_multiply(0.55))));
        self.painter.line_segment([self.to(hinge), self.to(hinge + v1)], Stroke::new(width + 1.0, color));
    }

    /// Items: line-art footprints with a category dot.
    fn items(&self, level: &Level, filter: impl Fn(&PlacedObject) -> bool) {
        for obj in level.objects.values().filter(|o| filter(o)) {
            let outline = self.screen(&obj.world_outline());
            let selected = self.sel == Some(EntityRef::Object(obj.id));
            let hovered = self.hover == Some(Hit::Object(obj.id));
            let (fill, stroke) = if selected {
                (c::room_selected(), Stroke::new(2.0_f32, c::teal()))
            } else if hovered {
                (c::room_fill(), Stroke::new(1.5_f32, c::ink()))
            } else {
                (c::item_fill(), Stroke::new(1.2_f32, c::item_stroke()))
            };
            self.painter.add(EShape::convex_polygon(outline, fill, stroke));
            let min_side = obj.shape.width.min(obj.shape.depth) as f32 * self.view.zoom;
            let long_side = obj.shape.width.max(obj.shape.depth) as f32 * self.view.zoom;
            if min_side > 24.0 || selected || long_side > 140.0 {
                let font = FontId::new(11.0, theme::medium());
                let galley = self.painter.layout_no_wrap(obj.display_name(), font, c::muted());
                let center = self.to(obj.position);
                let total = galley.size().x + 10.0;
                let dot = center - vec2(total / 2.0 - 3.0, 0.0);
                self.painter.circle_filled(dot, 3.0, rgb(obj.display_color()));
                self.painter.galley(pos2(dot.x + 7.0, center.y - galley.size().y / 2.0), galley, c::muted());
            }
        }
    }

    /// Selected room outline + handles.
    fn room_selection(&self, level: &Level) {
        if let Some(EntityRef::Room(rid)) = self.sel {
            if let Some(room) = level.rooms.get(&rid) {
                let world = level.room_points(room);
                self.painter.add(EShape::closed_line(self.screen(&world), Stroke::new(2.0_f32, c::teal())));
                if !room.shape_locked {
                    self.handles(&world);
                }
            }
        }
    }

    fn room_labels(&self, level: &Level) {
        for room in level.rooms.values() {
            let pts = level.room_points(room);
            if pts.len() < 3 {
                continue;
            }
            let Some((lo, hi)) = bounds(&pts) else { continue };
            let size_px = ((hi.x - lo.x).min(hi.y - lo.y)) as f32 * self.view.zoom;
            if size_px > 36.0 {
                let c = self.to(label_point(&pts));
                self.painter.text(c - vec2(0.0, 8.0), Align2::CENTER_CENTER, room.display_name(), FontId::new(13.5, theme::semibold()), c::ink());
                self.painter.text(
                    c + vec2(0.0, 9.0),
                    Align2::CENTER_CENTER,
                    format_area(area(&pts), self.units),
                    FontId::proportional(11.5),
                    c::muted(),
                );
            }
            let selected = self.sel == Some(EntityRef::Room(room.id));
            if self.show_dims || selected {
                self.edge_dims(&pts, true, if selected { c::teal() } else { c::muted() });
            }
        }
        for wall in level.walls.values() {
            if wall.label.trim().is_empty() {
                continue;
            }
            let Some((a, b)) = level.wall_points(wall) else { continue };
            let n = (b - a).perp().normalized();
            let (sa, sb) = (self.to(a), self.to(b));
            text_along(
                self.painter,
                self.to(a.lerp(b, 0.5) + n * (wall.thickness / 2.0 + 10.0 * self.px())),
                (sb.y - sa.y).atan2(sb.x - sa.x),
                wall.label.clone(),
                FontId::proportional(10.5),
                c::faint(),
            );
        }
    }

    // ---- roofs

    fn roofs(&self, level: &Level) {
        let painter = self.painter;
        let mut roofs: Vec<&RoofSection> = level.roofs.values().collect();
        // Bigger roofs first so porch roofs etc. draw on top.
        roofs.sort_by(|a, b| b.plan_area().total_cmp(&a.plan_area()));
        let light = Vec2::new(-0.6, -0.8).normalized();
        for roof in &roofs {
            let selected = self.sel == Some(EntityRef::RoofSection(roof.id));
            let hovered = self.hover == Some(Hit::Roof(roof.id));
            let swatch = roof.top_layer().and_then(|l| l.swatch).map(rgb).unwrap_or(Color32::from_rgb(132, 128, 122));
            let mut base = theme::mix(c::canvas(), swatch, 0.55);
            if selected {
                base = theme::mix(base, c::teal(), 0.3);
            } else if hovered {
                base = theme::mix(base, c::ink(), 0.06);
            }
            for facet in roof.facets() {
                // Shade each facet by which way it faces.
                let mid = centroid(&facet);
                let e = 0.05;
                let g = Vec2::new(
                    roof.height_at(0.0, mid + Vec2::new(e, 0.0)) - roof.height_at(0.0, mid - Vec2::new(e, 0.0)),
                    roof.height_at(0.0, mid + Vec2::new(0.0, e)) - roof.height_at(0.0, mid - Vec2::new(0.0, e)),
                );
                let k = if g.length() < 1e-9 { 0.0 } else { (-g.normalized()).dot(light) as f32 };
                let fill = if k >= 0.0 {
                    theme::mix(base, Color32::WHITE, 0.18 * k)
                } else {
                    theme::mix(base, Color32::BLACK, 0.14 * -k)
                };
                fill_polygon(painter, &facet, &self.screen(&facet), fill);
            }
            let edge = if selected { c::teal() } else { c::wall() };
            for (a, b) in roof.ridge_lines() {
                painter.line_segment([self.to(a), self.to(b)], Stroke::new(1.2_f32, edge.gamma_multiply(0.8)));
            }
            painter.add(EShape::closed_line(self.screen(&roof.footprint()), Stroke::new(if selected { 2.0 } else { 1.5_f32 }, edge)));
            // The walls under it.
            let mut wall_line = self.screen(&roof.outline);
            wall_line.extend(wall_line.first().copied());
            painter.extend(EShape::dashed_line(&wall_line, Stroke::new(1.0_f32, edge.gamma_multiply(0.45)), 5.0, 4.0));
        }
        for roof in &roofs {
            let fp = roof.footprint();
            let Some((lo, hi)) = bounds(&fp) else { continue };
            let size_px = ((hi.x - lo.x).min(hi.y - lo.y)) as f32 * self.view.zoom;
            if size_px > 50.0 {
                let p = self.to(label_point(&fp));
                let sub = if roof.shape.is_sloped() {
                    format!("{} · {}", roof.shape.label(), format_pitch(roof.pitch, self.units))
                } else {
                    roof.shape.label().to_string()
                };
                let galley = painter.layout_no_wrap(roof.display_name(), FontId::new(13.0, theme::semibold()), c::ink());
                let sub_g = painter.layout_no_wrap(sub, FontId::proportional(11.0), c::muted());
                let w = galley.size().x.max(sub_g.size().x) + 16.0;
                let r = Rect::from_center_size(p, vec2(w, 38.0));
                painter.rect_filled(r, 10.0, c::panel().gamma_multiply(0.92));
                painter.galley(pos2(p.x - galley.size().x / 2.0, r.top() + 4.0), galley, c::ink());
                painter.galley(pos2(p.x - sub_g.size().x / 2.0, r.top() + 21.0), sub_g, c::muted());
            }
            let selected = self.sel == Some(EntityRef::RoofSection(roof.id));
            if selected || self.show_dims {
                self.edge_dims(&roof.outline, true, if selected { c::teal() } else { c::muted() });
            }
            if selected {
                self.handles(&roof.outline);
            }
        }
    }

    // ---- outside

    fn site(&self, project: &Project, li: usize) {
        let painter = self.painter;
        let level = &project.levels[li];
        let zoom = self.view.zoom;
        let strength = c::tint_strength();

        // Property lines first, under everything.
        for f in level.features.values().filter(|f| f.kind == FeatureKind::PropertyLine) {
            let selected = self.sel == Some(EntityRef::Feature(f.id));
            let hovered = self.hover == Some(Hit::Feature(f.id));
            let mut pts = self.screen(&f.points);
            pts.extend(pts.first().copied());
            let col = if selected { c::teal() } else if hovered { c::ink() } else { c::muted() };
            painter.extend(EShape::dashed_line(&pts, Stroke::new(if selected { 2.5 } else { 1.6_f32 }, col), 12.0, 5.0));
            if let Some((lo, _)) = bounds(&f.points) {
                let text = format!("{}  ·  {}", f.display_name(), format_area(f.area(), self.units));
                painter.text(self.to(lo) + vec2(10.0, 10.0), Align2::LEFT_TOP, text, FontId::new(12.0, theme::medium()), c::muted());
            }
        }

        // Areas: lawn under paving under decks.
        let order = |k: FeatureKind| match k {
            FeatureKind::Lawn => 0,
            FeatureKind::GardenBed => 1,
            FeatureKind::Driveway => 2,
            FeatureKind::Path => 3,
            FeatureKind::Patio => 4,
            FeatureKind::Pool => 5,
            _ => 6,
        };
        let mut areas: Vec<&Feature> = level
            .features
            .values()
            .filter(|f| f.kind.geom() == FeatureGeom::Area && f.kind != FeatureKind::PropertyLine)
            .collect();
        areas.sort_by(|a, b| order(a.kind).cmp(&order(b.kind)).then(b.area().total_cmp(&a.area())));
        for f in &areas {
            let selected = self.sel == Some(EntityRef::Feature(f.id));
            let hovered = self.hover == Some(Hit::Feature(f.id));
            let col = rgb(f.display_color());
            let mut fill = theme::mix(c::canvas(), col, strength);
            if hovered {
                fill = theme::mix(fill, c::ink(), 0.05);
            }
            let line = theme::mix(c::canvas(), col, 0.9);
            fill_polygon(painter, &f.points, &self.screen(&f.points), fill);
            // Texture: deck boards, paving joints, bed hatching.
            let longest = {
                let n = f.points.len();
                (0..n)
                    .map(|i| (f.points[i], f.points[(i + 1) % n]))
                    .max_by(|x, y| x.0.distance(x.1).total_cmp(&y.0.distance(y.1)))
                    .map(|(a, b)| b - a)
                    .unwrap_or(Vec2::new(1.0, 0.0))
            };
            let pattern = match f.kind {
                FeatureKind::Deck => Some((longest, 0.14, 0.35)),
                FeatureKind::Patio | FeatureKind::Path => Some((longest, 0.6, 0.25)),
                FeatureKind::GardenBed => Some((Vec2::new(1.0, -1.0), 0.3, 0.25)),
                _ => None,
            };
            if let Some((dir, spacing, alpha)) = pattern {
                if spacing as f32 * zoom > 5.0 {
                    let stroke = Stroke::new(1.0_f32, line.gamma_multiply(alpha));
                    for (a, b) in hatch(&f.points, dir, spacing) {
                        painter.line_segment([self.to(a), self.to(b)], stroke);
                    }
                }
            }
            if f.kind == FeatureKind::Pool {
                let inner = offset_polygon(&f.points, -0.3);
                painter.add(EShape::closed_line(self.screen(&inner), Stroke::new(1.0_f32, line)));
            }
            let width = if f.kind == FeatureKind::Deck && f.railing { 2.5 } else { 1.2_f32 };
            let stroke = if selected { Stroke::new(2.0_f32, c::teal()) } else { Stroke::new(width, line) };
            painter.add(EShape::closed_line(self.screen(&f.points), stroke));
        }

        // The house and outbuildings.
        for ci in context_levels(project) {
            let l = &project.levels[ci];
            let (hovered, selected) = match l.building {
                Some(b) => (self.hover == Some(Hit::Building(b)), self.sel == Some(EntityRef::Building(b))),
                None => (self.hover == Some(Hit::House), self.sel == Some(EntityRef::General)),
            };
            self.floor(project, l, FloorStyle::Context { selected, hovered });
            let pts: Vec<Vec2> = l.nodes.values().map(|n| n.pos).collect();
            if let Some((lo, hi)) = bounds(&pts) {
                let size_px = ((hi.x - lo.x).min(hi.y - lo.y)) as f32 * zoom;
                if size_px > 30.0 {
                    let name = match l.building.and_then(|b| project.building(b)) {
                        Some(b) => b.display_name(),
                        None => project.entity_name(EntityRef::General),
                    };
                    let i = if l.building.is_some() { icon::WAREHOUSE } else { icon::HOUSE_LINE };
                    let font = if l.building.is_some() { FontId::new(13.0, theme::semibold()) } else { FontId::new(22.0, theme::serif()) };
                    painter.text(self.to((lo + hi) / 2.0), Align2::CENTER_CENTER, format!("{i}  {name}"), font, c::ink());
                }
            }
        }

        // Fences, walls and hedges, with gates.
        for f in level.features.values().filter(|f| f.kind.geom() == FeatureGeom::Line) {
            let selected = self.sel == Some(EntityRef::Feature(f.id));
            let hovered = self.hover == Some(Hit::Feature(f.id));
            let col = rgb(f.display_color());
            let cuts: Vec<(f64, f64)> = f.gates.iter().map(|g| (g.offset - g.width / 2.0, g.offset + g.width / 2.0)).collect();
            let pieces = polyline_pieces(&f.points, &cuts);
            let (width, color) = match f.kind {
                FeatureKind::Hedge => ((f.line_width() as f32 * zoom).max(4.0), theme::mix(c::canvas(), col, 0.7)),
                FeatureKind::RetainingWall => ((f.line_width() as f32 * zoom).max(3.0), theme::mix(c::canvas(), col, 0.9)),
                _ => (2.2, theme::mix(c::canvas(), col, 0.95)),
            };
            for piece in &pieces {
                let pts = self.screen(piece);
                if selected || hovered {
                    painter.add(EShape::line(pts.clone(), Stroke::new(width + 4.0, if selected { c::teal().gamma_multiply(0.5) } else { c::hover() })));
                }
                painter.add(EShape::line(pts, Stroke::new(width, color)));
            }
            if f.kind == FeatureKind::Fence {
                // Posts about every 2.4 m.
                let len = f.length();
                let n = (len / 2.4).ceil().max(1.0) as usize;
                if (len / n as f64) as f32 * zoom > 10.0 {
                    for i in 0..=n {
                        let s = len * i as f64 / n as f64;
                        if cuts.iter().any(|(a, b)| s > *a && s < *b) {
                            continue;
                        }
                        if let Some((p, _)) = point_along(&f.points, s) {
                            painter.rect_filled(Rect::from_center_size(self.to(p), vec2(5.0, 5.0)), 1.0, color);
                        }
                    }
                }
            }
            for g in &f.gates {
                let (Some((a, _)), Some((b, dir))) =
                    (point_along(&f.points, g.offset - g.width / 2.0), point_along(&f.points, g.offset + g.width / 2.0))
                else {
                    continue;
                };
                self.swing(a, b, dir.perp(), if selected { c::teal() } else { color }, 1.0);
                if zoom > 30.0 && !g.label.is_empty() {
                    painter.text(
                        self.to(a.lerp(b, 0.5) - dir.perp() * (14.0 * self.px())),
                        Align2::CENTER_CENTER,
                        &g.label,
                        FontId::proportional(10.5),
                        c::muted(),
                    );
                }
            }
            if selected || self.show_dims {
                self.edge_dims(&f.points, false, if selected { c::teal() } else { c::muted() });
            }
        }

        // Trees
        for f in level.features.values().filter(|f| f.kind == FeatureKind::Tree) {
            let Some(center) = f.points.first().copied() else { continue };
            let selected = self.sel == Some(EntityRef::Feature(f.id));
            let hovered = self.hover == Some(Hit::Feature(f.id));
            let r = (f.size / 2.0).max(0.3);
            let col = rgb(f.display_color());
            let pts: Vec<Pos2> = (0..72)
                .map(|i| {
                    let t = i as f64 / 72.0 * std::f64::consts::TAU;
                    let rr = r * (0.93 + 0.07 * (9.0 * t).cos());
                    self.to(center + Vec2::new(t.cos(), t.sin()) * rr)
                })
                .collect();
            let fill = theme::mix(c::canvas(), col, if hovered { 0.5 } else { 0.38 }).gamma_multiply(0.85);
            let stroke = if selected { Stroke::new(2.0_f32, c::teal()) } else { Stroke::new(1.2_f32, theme::mix(c::canvas(), col, 0.9)) };
            let disc: Vec<Pos2> = (0..36)
                .map(|i| {
                    let t = i as f64 / 36.0 * std::f64::consts::TAU;
                    self.to(center + Vec2::new(t.cos(), t.sin()) * (r * 0.93))
                })
                .collect();
            painter.add(EShape::convex_polygon(disc, fill, Stroke::NONE));
            painter.add(EShape::closed_line(pts, stroke));
            painter.circle_filled(self.to(center), 3.0, theme::mix(c::canvas(), Color32::from_rgb(110, 80, 50), 0.9));
            if r as f32 * zoom > 28.0 {
                painter.text(self.to(center) + vec2(0.0, 8.0), Align2::CENTER_TOP, f.display_name(), FontId::proportional(11.0), c::muted());
            }
        }

        self.items(level, |_| true);

        // Area labels and the selected shape's handles/dimensions.
        for f in &areas {
            let Some((lo, hi)) = bounds(&f.points) else { continue };
            let size_px = ((hi.x - lo.x).min(hi.y - lo.y)) as f32 * zoom;
            let selected = self.sel == Some(EntityRef::Feature(f.id));
            if size_px > 44.0 {
                let has_items = level.objects.values().any(|o| o.feature == Some(f.id));
                let p = self.to(label_point(&f.points));
                let p = if has_items { pos2(p.x, self.to(lo).y + 14.0) } else { p };
                painter.text(p, Align2::CENTER_CENTER, f.display_name(), FontId::new(12.5, theme::semibold()), c::ink());
                if !has_items {
                    painter.text(p + vec2(0.0, 15.0), Align2::CENTER_CENTER, format_area(f.area(), self.units), FontId::proportional(11.0), c::muted());
                }
            }
            if selected || self.show_dims {
                self.edge_dims(&f.points, true, if selected { c::teal() } else { c::muted() });
            }
        }
        if let Some(EntityRef::Feature(fid)) = self.sel {
            if let Some(f) = level.features.get(&fid) {
                if f.kind.geom() != FeatureGeom::Point {
                    self.handles(&f.points);
                }
                if f.kind == FeatureKind::PropertyLine {
                    self.edge_dims(&f.points, true, c::teal());
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_overlay(
    painter: &Painter,
    rect: Rect,
    project: &Project,
    li: usize,
    st: &PlanState,
    hover: Option<Pos2>,
    hover_snapped: Option<Vec2>,
    units: UnitSystem,
) {
    let level = &project.levels[li];
    let view = &st.view;
    let to = |p: Vec2| view.to_screen(rect, p);
    let accent = Stroke::new(2.0_f32, c::teal());

    // Alignment guides: dashed lines right across the canvas.
    let reach = (rect.width() + rect.height()) as f64 * view.px();
    let guide = Stroke::new(1.0_f32, c::coral().gamma_multiply(0.8));
    for (p, d) in &st.guides {
        let (a, b) = (to(*p - *d * reach), to(*p + *d * reach));
        painter.with_clip_rect(rect).extend(EShape::dashed_line(&[a, b], guide, 5.0, 4.0));
    }
    if let (false, Some(h)) = (st.guides.is_empty(), hover_snapped) {
        painter.circle_filled(to(h), 3.5, c::coral());
    }
    let label = |p: Pos2, s: String| {
        let galley = painter.layout_no_wrap(s, FontId::new(12.0, theme::medium()), Color32::WHITE);
        let r = Rect::from_center_size(p, galley.size() + vec2(14.0, 6.0));
        painter.rect_filled(r, 8.0, c::teal());
        painter.galley(r.center() - galley.size() / 2.0, galley, Color32::WHITE);
    };

    match &st.draft {
        Draft::Rect { start } | Draft::Object { start } => {
            if let Some(end) = st.last_world {
                let (lo, hi) = (start.min(end), start.max(end));
                let world = rect_points(lo, hi.x - lo.x, hi.y - lo.y);
                let pts: Vec<Pos2> = world.iter().map(|p| to(*p)).collect();
                painter.add(EShape::convex_polygon(pts, c::teal().gamma_multiply(0.08), accent));
                if st.tool == Tool::RectRoof && hi.x - lo.x > 0.3 && hi.y - lo.y > 0.3 {
                    let r = RoofSection::new(&world, st.roof_shape);
                    for (a, b) in r.ridge_lines() {
                        painter.line_segment([to(a), to(b)], Stroke::new(1.0_f32, c::teal()));
                    }
                    let fp: Vec<Pos2> = r.footprint().iter().map(|p| to(*p)).collect();
                    painter.add(EShape::closed_line(fp, Stroke::new(1.0_f32, c::teal().gamma_multiply(0.6))));
                }
                label(
                    to((lo + hi) / 2.0),
                    format!("{} × {}", format_length(hi.x - lo.x, units), format_length(hi.y - lo.y, units)),
                );
            }
        }
        Draft::Poly { points } => {
            let mut pts: Vec<Pos2> = points.iter().map(|p| to(*p)).collect();
            if let Some(h) = hover_snapped {
                if let Some(&last) = points.last() {
                    label(to(last.lerp(h, 0.5)) + vec2(0.0, -16.0), format_length(last.distance(h), units));
                }
                pts.push(to(h));
            }
            painter.add(EShape::line(pts, accent));
            for (i, p) in points.iter().enumerate() {
                let r = if i == 0 && st.tool.closes_shape() { 6.0 } else { 4.5 };
                painter.circle_filled(to(*p), r, c::panel());
                painter.circle_stroke(to(*p), r, accent);
            }
        }
        Draft::Calibrate { first } => {
            painter.circle_filled(to(*first), 5.0, c::teal());
            if let Some(h) = hover {
                painter.line_segment([to(*first), h], accent);
            }
        }
        Draft::None => {}
    }

    let Some(h) = hover else { return };
    match st.tool {
        Tool::Door | Tool::Window => {
            let w = view.to_world(rect, h);
            if let Some((wid, off, _)) = level.nearest_wall(w, 14.0 * view.px()) {
                if let Some((a, b)) = level.wall_points(&level.walls[&wid]) {
                    let width = if st.tool == Tool::Door { 0.81 } else { 0.91 };
                    let dir = (b - a).normalized();
                    let c = a + dir * off;
                    painter.line_segment(
                        [to(c - dir * (width / 2.0)), to(c + dir * (width / 2.0))],
                        Stroke::new(7.0_f32, c::teal().gamma_multiply(0.8)),
                    );
                }
            }
        }
        Tool::Gate => {
            let w = view.to_world(rect, h);
            let best = level
                .features
                .values()
                .filter(|f| f.kind.geom() == FeatureGeom::Line)
                .filter_map(|f| nearest_on_polyline(&f.points, w).map(|(s, d)| (d, s, f)))
                .filter(|(d, _, _)| *d < 14.0 * view.px())
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, s, f)) = best {
                if let (Some((a, _)), Some((b, _))) = (point_along(&f.points, s - 0.5), point_along(&f.points, s + 0.5)) {
                    painter.line_segment([to(a), to(b)], Stroke::new(7.0_f32, c::teal().gamma_multiply(0.8)));
                }
            }
        }
        Tool::Tree => {
            if let Some(p) = hover_snapped {
                painter.circle_stroke(to(p), 2.5 * view.zoom, Stroke::new(1.5_f32, c::teal()));
            }
        }
        Tool::Object if matches!(st.draft, Draft::None) => {
            if let Some(p) = hover_snapped {
                let preset = &OBJECT_PRESETS[st.preset];
                let outline: Vec<Pos2> = Shape::new(preset.shape, preset.width, preset.depth)
                    .outline()
                    .into_iter()
                    .map(|q| to(p + q))
                    .collect();
                painter.add(EShape::closed_line(outline, Stroke::new(1.5_f32, c::teal())));
            }
        }
        Tool::RectRoom
        | Tool::PolyRoom
        | Tool::RectRoof
        | Tool::PolyRoof
        | Tool::RectArea
        | Tool::PolyArea
        | Tool::Line
        | Tool::Building
        | Tool::PropertyLine
            if matches!(st.draft, Draft::None) =>
        {
            if let Some(p) = hover_snapped {
                painter.circle_stroke(to(p), 5.0, Stroke::new(1.5_f32, c::teal()));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hatching_fills_a_rectangle() {
        let lines = hatch(&rect_points(Vec2::ZERO, 4.0, 2.0), Vec2::new(1.0, 0.0), 0.5);
        assert_eq!(lines.len(), 3);
        for (a, b) in lines {
            assert!((a.distance(b) - 4.0).abs() < 1e-9);
        }
    }

    #[test]
    fn gates_cut_the_fence() {
        let pts = vec![Vec2::ZERO, Vec2::new(10.0, 0.0)];
        let pieces = polyline_pieces(&pts, &[(4.0, 5.0)]);
        assert_eq!(pieces.len(), 2);
        assert!((polyline_length(&pieces[0]) - 4.0).abs() < 1e-9);
        assert!((polyline_length(&pieces[1]) - 5.0).abs() < 1e-9);
    }
}

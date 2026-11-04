//! The 2D floor-plan editor.
//!
//! World coordinates are metres (+y down). `View` maps them to screen pixels.
//! Each frame: handle zoom/pan, run the active tool's interaction, draw the
//! plan, then the floating toolbars on top.

use super::widgets::length_edit_sized;
use super::{Action, CalibrateDialog, Env, Insets};
use crate::theme::{self, c, icon};
use abode_core::geometry::*;
use abode_core::units::{UnitSystem, format_area, format_length};
use abode_core::*;
use eframe::egui::{
    self, Align2, Color32, FontId, Key, Painter, PointerButton, Pos2, Rect, Sense, Shape as EShape, Stroke, Ui,
    pos2, vec2,
};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    RectRoom,
    PolyRoom,
    Door,
    Window,
    Object,
    Calibrate,
    MoveUnderlay,
}

impl Tool {
    fn icon(self) -> &'static str {
        match self {
            Tool::Select => icon::CURSOR,
            Tool::RectRoom => icon::SQUARE,
            Tool::PolyRoom => icon::POLYGON,
            Tool::Door => icon::DOOR,
            Tool::Window => icon::APP_WINDOW,
            Tool::Object => icon::PACKAGE,
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
            Tool::Calibrate => "Calibrate blueprint",
            Tool::MoveUnderlay => "Move blueprint",
        }
    }

    fn key(self) -> Option<Key> {
        Some(match self {
            Tool::Select => Key::V,
            Tool::RectRoom => Key::R,
            Tool::PolyRoom => Key::P,
            Tool::Door => Key::D,
            Tool::Window => Key::W,
            Tool::Object => Key::O,
            _ => return None,
        })
    }

    fn hint(self) -> &'static str {
        match self {
            Tool::Select => "Click to select · drag items, doors and windows · drag empty space to pan",
            Tool::RectRoom => "Drag to draw a room · snaps to corners and walls · hold Alt to disable snapping",
            Tool::PolyRoom => "Click to add corners · click the first corner or press Enter to finish · Shift for straight lines",
            Tool::Door => "Click on a wall to add a door",
            Tool::Window => "Click on a wall to add a window",
            Tool::Object => "Click inside a room to place it, or drag to size it",
            Tool::Calibrate => "Click two points on the blueprint that you know the real distance between",
            Tool::MoveUnderlay => "Drag to line the blueprint up",
        }
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
    Node(NodeId),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Hit {
    Opening(OpeningId),
    Object(ObjectId),
    Node(NodeId),
    Wall(WallId),
    Room(RoomId),
}

impl Hit {
    fn entity(self) -> Option<EntityRef> {
        Some(match self {
            Hit::Opening(id) => EntityRef::Opening(id),
            Hit::Object(id) => EntityRef::Object(id),
            Hit::Wall(id) => EntityRef::Wall(id),
            Hit::Room(id) => EntityRef::Room(id),
            Hit::Node(_) => return None,
        })
    }
}

pub struct PlanState {
    pub tool: Tool,
    pub preset: usize,
    pub door_style: DoorStyle,
    pub window_style: WindowStyle,
    pub snap: bool,
    pub show_dims: bool,
    pub fit_request: bool,
    view: View,
    fitted: bool,
    /// Canvas size the last automatic fit was for.
    fit_size: Option<egui::Vec2>,
    /// Set once the person pans/zooms, so we stop auto-fitting.
    touched: bool,
    pending_center: Option<Vec2>,
    zoom_request: Option<f32>,
    draft: Draft,
    drag: Option<DragOp>,
    last_world: Option<Vec2>,
}

impl Default for PlanState {
    fn default() -> Self {
        Self {
            tool: Tool::Select,
            preset: 0,
            door_style: DoorStyle::Hinged,
            window_style: WindowStyle::DoubleHung,
            snap: true,
            show_dims: false,
            fit_request: false,
            view: View { offset: egui::Vec2::ZERO, zoom: 50.0 },
            fitted: false,
            fit_size: None,
            touched: false,
            pending_center: None,
            zoom_request: None,
            draft: Draft::None,
            drag: None,
            last_world: None,
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

// ---------------------------------------------------------------------------
// Colours for the drawing

const ROOM_FILL: Color32 = Color32::WHITE;
const ROOM_HOVER: Color32 = Color32::from_rgb(251, 250, 246);
const ROOM_SELECTED: Color32 = Color32::from_rgb(232, 243, 239);
const WALL_HOVER: Color32 = Color32::from_rgb(70, 80, 80);
const THRESHOLD: Color32 = Color32::from_rgb(233, 230, 222);
const ITEM_FILL: Color32 = Color32::from_rgb(245, 243, 238);
const ITEM_STROKE: Color32 = Color32::from_rgb(140, 138, 130);

// ---------------------------------------------------------------------------
// Helpers

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

/// Snap to a nearby corner, then a nearby wall, then the grid.
fn snap(level: &Level, view: &View, rect: Rect, s: Pos2, grid_on: bool, step: f64) -> Vec2 {
    let w = view.to_world(rect, s);
    if let Some(n) = level.find_node_near(w, 10.0 * view.px()) {
        return level.nodes[&n].pos;
    }
    let g = if grid_on { snap_grid(w, step) } else { w };
    if let Some((wid, off, _)) = level.nearest_wall(w, 7.0 * view.px()) {
        if let Some((a, b)) = level.wall_points(&level.walls[&wid]) {
            let dir = (b - a).normalized();
            let along = if grid_on { (g - a).dot(dir).clamp(0.0, a.distance(b)) } else { off };
            return a + dir * along;
        }
    }
    g
}

fn ortho(from: Vec2, to: Vec2) -> Vec2 {
    let d = to - from;
    if d.x.abs() > d.y.abs() {
        Vec2::new(to.x, from.y)
    } else {
        Vec2::new(from.x, to.y)
    }
}

fn hit_test(level: &Level, sel: Option<EntityRef>, view: &View, rect: Rect, s: Pos2) -> Option<Hit> {
    let w = view.to_world(rect, s);
    let px = view.px();
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
    for o in level.objects.values().rev() {
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

fn fit_view(st: &mut PlanState, area: Rect, canvas: Rect, level: &Level) {
    let mut pts: Vec<Vec2> = level.nodes.values().map(|n| n.pos).collect();
    if let Some(u) = &level.underlay {
        let [w, h] = u.pixel_size;
        for (x, y) in [(0, 0), (w, 0), (w, h), (0, h)] {
            pts.push(u.pixel_to_world(Vec2::new(x as f64, y as f64)));
        }
    }
    let center_px = area.center() - canvas.min;
    match bounds(&pts) {
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

// ---------------------------------------------------------------------------
// Floating UI

fn float_area(id: &str) -> egui::Area {
    egui::Area::new(egui::Id::new(id)).order(egui::Order::Middle).constrain(true)
}

fn toolbar(ctx: &egui::Context, project: &Project, env: &mut Env, st: &mut PlanState, area: Rect) {
    float_area("plan-toolbar")
        .pivot(Align2::CENTER_TOP)
        .fixed_pos(pos2(area.center().x, area.top() + theme::MARGIN))
        .show(ctx, |ui| {
            theme::pill().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let groups: [&[Tool]; 3] = [&[Tool::Select], &[Tool::RectRoom, Tool::PolyRoom], &[Tool::Door, Tool::Window, Tool::Object]];
                    for (gi, group) in groups.iter().enumerate() {
                        if gi > 0 {
                            theme::vdivider(ui);
                        }
                        for tool in *group {
                            let key = tool.key().map(|k| format!("  ({})", k.name())).unwrap_or_default();
                            let tip = format!("{}{key}\n{}", tool.name(), tool.hint());
                            if theme::icon_button(ui, tool.icon(), &tip, st.tool == *tool).clicked() {
                                st.set_tool(*tool);
                            }
                        }
                    }
                    if project.level(*env.active_level).is_some_and(|l| l.underlay.is_some()) {
                        theme::vdivider(ui);
                        for tool in [Tool::Calibrate, Tool::MoveUnderlay] {
                            let tip = format!("{}\n{}", tool.name(), tool.hint());
                            if theme::icon_button(ui, tool.icon(), &tip, st.tool == tool).clicked() {
                                st.set_tool(tool);
                            }
                        }
                    }
                    theme::vdivider(ui);
                    let current = project.level(*env.active_level).map(|l| l.name.clone()).unwrap_or_default();
                    egui::ComboBox::from_id_salt("plan-level")
                        .selected_text(format!("{}  {current}", icon::STACK))
                        .show_ui(ui, |ui| {
                            for l in &project.levels {
                                ui.selectable_value(env.active_level, l.id, &l.name);
                            }
                        });
                });
            });
        });

    // Contextual options for placement tools, like a "mode" chip.
    if matches!(st.tool, Tool::Object | Tool::Door | Tool::Window) {
        float_area("plan-tool-options")
            .pivot(Align2::CENTER_TOP)
            .fixed_pos(pos2(area.center().x, area.top() + theme::MARGIN + 60.0))
            .show(ctx, |ui| {
                theme::pill().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(st.tool.icon()).color(c::FOREST).size(16.0));
                        match st.tool {
                            Tool::Object => {
                                ui.label(theme::muted("Place"));
                                egui::ComboBox::from_id_salt("plan-preset")
                                    .selected_text(OBJECT_PRESETS[st.preset].label)
                                    .height(420.0)
                                    .show_ui(ui, |ui| {
                                        for (i, p) in OBJECT_PRESETS.iter().enumerate() {
                                            ui.selectable_value(&mut st.preset, i, p.label);
                                        }
                                    });
                            }
                            Tool::Door => {
                                ui.label(theme::muted("Door style"));
                                super::widgets::combo(ui, "plan-door", &mut st.door_style, &DoorStyle::ALL, DoorStyle::label);
                            }
                            _ => {
                                ui.label(theme::muted("Window style"));
                                super::widgets::combo(ui, "plan-window", &mut st.window_style, &WindowStyle::ALL, WindowStyle::label);
                            }
                        }
                        if theme::icon_button_sized(ui, icon::X, "Done (Esc)", false, 28.0).clicked() {
                            st.set_tool(Tool::Select);
                        }
                    });
                });
            });
    }
}

#[allow(clippy::too_many_arguments)]
fn bottom_bars(
    ctx: &egui::Context,
    project: &mut Project,
    env: &mut Env,
    st: &mut PlanState,
    area: Rect,
    li: usize,
    hover_world: Option<Vec2>,
) {
    let units = project.info.units;
    // Centre: context info (editable size for an unlocked rectangular room)
    float_area("plan-info")
        .pivot(Align2::CENTER_BOTTOM)
        .fixed_pos(pos2(area.center().x, area.bottom() - theme::MARGIN))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                theme::pill().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(6.0);
                        let editing = match *env.selection {
                            Some(EntityRef::Room(rid)) => {
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
                            let a = length_edit_sized(ui, ("pill-w", rid), &mut w2, units, 1e-6, 84.0);
                            theme::vdivider(ui);
                            ui.label(theme::muted("Depth"));
                            let b = length_edit_sized(ui, ("pill-d", rid), &mut d2, units, 1e-6, 84.0);
                            if a || b {
                                if let Err(e) = project.levels[li].set_rect_size(rid, w2, d2) {
                                    env.toasts.error(e.to_string());
                                }
                            }
                        } else {
                            let text = match hover_world {
                                Some(w) => format!("{}, {}   ·   {}", format_length(w.x, units), format_length(w.y, units), st.tool.hint()),
                                None => st.tool.hint().to_string(),
                            };
                            ui.label(egui::RichText::new(text).size(12.5).color(c::MUTED));
                        }
                        ui.add_space(6.0);
                    });
                });
                theme::pill().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if theme::icon_button(ui, icon::GRID_FOUR, "Snap to grid, corners and walls (hold Alt to override)", st.snap).clicked() {
                            st.snap = !st.snap;
                        }
                        if theme::icon_button(ui, icon::RULER, "Show all dimensions", st.show_dims).clicked() {
                            st.show_dims = !st.show_dims;
                        }
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
                        egui::Label::new(egui::RichText::new(format!("{:.0}%", st.view.zoom / 50.0 * 100.0)).size(12.5).color(c::MUTED)),
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

fn empty_state(ctx: &egui::Context, env: &mut Env, st: &mut PlanState, area: Rect, level: LevelId) {
    float_area("plan-empty")
        .pivot(Align2::CENTER_CENTER)
        .fixed_pos(area.center())
        .show(ctx, |ui| {
            theme::panel().inner_margin(egui::Margin::same(28)).show(ui, |ui| {
                ui.set_width(380.0);
                theme::icon_tile(ui, icon::HOUSE_LINE, c::CORAL, c::CORAL_SOFT);
                ui.add_space(6.0);
                ui.label(theme::title("Start with a room"));
                ui.label(theme::muted(
                    "Drag out rectangles for each room. Rooms that touch share their walls, so the house builds itself up as you go.",
                ));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if theme::primary_button(ui, format!("{}  Draw a room", icon::SQUARE)).clicked() {
                        st.set_tool(Tool::RectRoom);
                    }
                    if theme::secondary_button(ui, format!("{}  Trace a blueprint", icon::IMAGE_SQUARE)).clicked() {
                        env.actions.push(Action::ImportUnderlay(level));
                    }
                });
            });
        });
}

// ---------------------------------------------------------------------------

pub fn plan_ui(ui: &mut Ui, project: &mut Project, env: &mut Env, st: &mut PlanState, insets: Insets) {
    if project.level(*env.active_level).is_none() {
        *env.active_level = project.levels[0].id;
    }
    let level_id = *env.active_level;
    if matches!(st.tool, Tool::Calibrate | Tool::MoveUnderlay)
        && project.level(level_id).is_some_and(|l| l.underlay.is_none())
    {
        st.set_tool(Tool::Select);
    }

    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = resp.rect;
    let area = insets.apply(rect);
    let li = project.levels.iter().position(|l| l.id == level_id).unwrap();
    let resized = st.fit_size.is_none_or(|s| (s - area.size()).length() > 1.0);
    if !st.fitted || st.fit_request || (resized && !st.touched) {
        fit_view(st, area, rect, &project.levels[li]);
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
    let hover_snapped = hover.map(|h| snap(&project.levels[li], &st.view, rect, h, snap_on, grid));
    if let Some(w) = hover_snapped {
        st.last_world = Some(w);
    }
    let pointer_world = ui
        .input(|i| i.pointer.latest_pos())
        .map(|p| st.view.to_world(rect, p));
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
                    let hit = hit_test(&project.levels[li], *env.selection, &st.view, rect, origin);
                    let origin_world = st.view.to_world(rect, origin);
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
                        Some(Hit::Node(n)) => DragOp::Node(n),
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
                    (Some(DragOp::Node(n)), Some(p)) => {
                        let p = if snap_on { snap_grid(p, grid) } else { p };
                        project.levels[li].move_node(n, p);
                    }
                    _ => {}
                }
            }
            if drag_end {
                if let Some(DragOp::Object { id, .. }) = st.drag {
                    if let Some(pos) = project.levels[li].objects.get(&id).map(|o| o.position) {
                        project.move_object(id, pos);
                    }
                }
                st.drag = None;
            }
            if clicked {
                let hit = hover.and_then(|h| hit_test(&project.levels[li], *env.selection, &st.view, rect, h));
                match hit {
                    Some(Hit::Node(_)) => {}
                    Some(h) => *env.selection = h.entity(),
                    None => *env.selection = None,
                }
            }
        }
        Tool::RectRoom => {
            if primary_start {
                if let Some(o) = press_origin {
                    st.draft = Draft::Rect { start: snap(&project.levels[li], &st.view, rect, o, snap_on, grid) };
                }
            }
            if drag_end {
                if let Draft::Rect { start } = st.draft {
                    st.draft = Draft::None;
                    let end = st.last_world.unwrap_or(start);
                    let (lo, hi) = (start.min(end), start.max(end));
                    let (w, h) = (hi.x - lo.x, hi.y - lo.y);
                    if w < 0.3 || h < 0.3 {
                        env.toasts.info("Drag out a bigger rectangle to make a room.");
                    } else {
                        let name = project.next_room_name();
                        match project.add_room(level_id, &name, &rect_points(lo, w, h)) {
                            Ok(id) => env.select(EntityRef::Room(id)),
                            Err(e) => env.toasts.error(format!("Couldn't add room: {e}")),
                        }
                    }
                }
            }
        }
        Tool::PolyRoom => {
            if clicked {
                if let Some(mut p) = hover_snapped {
                    let mut finish = false;
                    match &mut st.draft {
                        Draft::Poly { points } => {
                            if shift {
                                if let Some(&last) = points.last() {
                                    p = ortho(last, p);
                                }
                            }
                            let first = st.view.to_screen(rect, points[0]);
                            if points.len() >= 3 && first.distance(st.view.to_screen(rect, p)) < 10.0 {
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
        Tool::Object => {
            if primary_start {
                if let Some(o) = press_origin {
                    st.draft = Draft::Object { start: snap(&project.levels[li], &st.view, rect, o, snap_on, grid) };
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
                    place_object(project, env, st, level_id, (lo + hi) / 2.0, w, d);
                }
            }
            if clicked {
                if let Some(p) = hover_snapped {
                    let preset = &OBJECT_PRESETS[st.preset];
                    place_object(project, env, st, level_id, p, preset.width, preset.depth);
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
                Some(r @ (EntityRef::Opening(_) | EntityRef::Object(_))) => env.actions.push(Action::Delete(r)),
                Some(r @ EntityRef::Room(_)) => env.dialogs.delete = Some(r),
                _ => {}
            }
        }
        if no_mods {
            for tool in Tool::MAIN_KEYS {
                if let Some(k) = tool.key() {
                    if ui.input(|i| i.key_pressed(k)) {
                        st.set_tool(tool);
                    }
                }
            }
        }
    }

    // ---- draw
    let hover_hit = if st.tool == Tool::Select && st.drag.is_none() {
        hover.and_then(|h| hit_test(&project.levels[li], *env.selection, &st.view, rect, h))
    } else {
        None
    };
    draw_plan(ui, &painter, rect, project, li, env, st, grid, hover_hit);
    draw_overlay(&painter, rect, project, li, st, hover, hover_snapped, shift, units);
    if let Some(h) = hover {
        resp.on_hover_cursor(match (st.tool, hover_hit) {
            _ if st.drag.is_some_and(|d| matches!(d, DragOp::Pan)) => egui::CursorIcon::Grabbing,
            (Tool::Select, Some(Hit::Object(_) | Hit::Opening(_) | Hit::Node(_))) => egui::CursorIcon::Grab,
            (Tool::Select, _) => egui::CursorIcon::Default,
            (Tool::MoveUnderlay, _) => egui::CursorIcon::Move,
            _ => egui::CursorIcon::Crosshair,
        });
        let _ = h;
    }

    // ---- floating UI
    if !insets.overlays {
        return;
    }
    let ctx = ui.ctx().clone();
    toolbar(&ctx, project, env, st, area);
    let hover_world = hover.map(|h| st.view.to_world(rect, h));
    bottom_bars(&ctx, project, env, st, area, li, hover_world);
    let level = &project.levels[li];
    if level.rooms.is_empty() && level.underlay.is_none() && st.tool == Tool::Select {
        empty_state(&ctx, env, st, area, level_id);
    }
}

impl Tool {
    const MAIN_KEYS: [Tool; 6] = [
        Tool::Select,
        Tool::RectRoom,
        Tool::PolyRoom,
        Tool::Door,
        Tool::Window,
        Tool::Object,
    ];
}

fn finish_poly(project: &mut Project, env: &mut Env, st: &mut PlanState, level: LevelId) {
    let Draft::Poly { points } = std::mem::replace(&mut st.draft, Draft::None) else {
        return;
    };
    if points.len() < 3 {
        env.toasts.info("A room needs at least three corners.");
        return;
    }
    let name = project.next_room_name();
    match project.add_room(level, &name, &points) {
        Ok(id) => env.select(EntityRef::Room(id)),
        Err(e) => env.toasts.error(format!("Couldn't add room: {e}")),
    }
}

fn place_object(project: &mut Project, env: &mut Env, st: &mut PlanState, level: LevelId, center: Vec2, w: f64, d: f64) {
    let preset = &OBJECT_PRESETS[st.preset];
    let label = if preset.category == ObjectCategory::Other { "" } else { preset.label };
    let shape = Shape::new(preset.shape, w, d);
    match project.add_object(level, label, preset.category, shape, center) {
        Some(id) => {
            env.select(EntityRef::Object(id));
            st.set_tool(Tool::Select);
        }
        None => env.toasts.info("Items go inside a room. Draw the room first."),
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
    let stroke = Stroke::new(1.0, color);
    for i in x0..=x1 {
        let x = view.to_screen(rect, Vec2::new(i as f64 * step, 0.0)).x;
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke);
    }
    for i in y0..=y1 {
        let y = view.to_screen(rect, Vec2::new(0.0, i as f64 * step)).y;
        painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], stroke);
    }
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

#[allow(clippy::too_many_arguments)]
fn draw_plan(
    ui: &Ui,
    painter: &Painter,
    rect: Rect,
    project: &Project,
    li: usize,
    env: &mut Env,
    st: &PlanState,
    grid: f64,
    hover_hit: Option<Hit>,
) {
    let level = &project.levels[li];
    let view = &st.view;
    let units = project.info.units;
    let sel = *env.selection;
    let to = |p: Vec2| view.to_screen(rect, p);

    painter.rect_filled(rect, 0.0, c::CANVAS);
    grid_lines(painter, rect, view, grid, c::GRID_MINOR);
    grid_lines(painter, rect, view, major_step(units, grid), c::GRID_MAJOR);

    // Blueprint underlay
    if let Some(u) = &level.underlay {
        if u.visible {
            if let Some(tex) = env.textures.get(ui.ctx(), env.store, u.attachment, 4096) {
                let [w, h] = u.pixel_size;
                let corners = [(0, 0), (w, 0), (w, h), (0, h)].map(|(x, y)| to(u.pixel_to_world(Vec2::new(x as f64, y as f64))));
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
    let over_blueprint = level.underlay.as_ref().is_some_and(|u| u.visible);

    // Room paint tints
    let mut tint: HashMap<RoomId, [u8; 3]> = HashMap::new();
    for p in &project.paints {
        if let (PaintTarget::Room(r), PaintSurface::Walls, Some(c)) = (p.target, p.surface, p.swatch) {
            tint.entry(r).or_insert(c);
        }
    }

    // Room fills
    for room in level.rooms.values() {
        let pts = level.room_points(room);
        if pts.len() < 3 {
            continue;
        }
        let screen: Vec<Pos2> = pts.iter().map(|p| to(*p)).collect();
        let mut fill = if sel == Some(EntityRef::Room(room.id)) {
            ROOM_SELECTED
        } else if hover_hit == Some(Hit::Room(room.id)) {
            ROOM_HOVER
        } else if let Some(c) = tint.get(&room.id) {
            theme::mix(ROOM_FILL, rgb(*c), 0.16)
        } else {
            ROOM_FILL
        };
        if over_blueprint {
            fill = fill.gamma_multiply(0.55);
        }
        fill_polygon(painter, &pts, &screen, fill);
    }

    // Walls, cut where doors and windows are
    for wall in level.walls.values() {
        let Some((a, b)) = level.wall_points(wall) else { continue };
        let len = a.distance(b);
        let dir = (b - a).normalized();
        let n = dir.perp();
        let ht = (wall.thickness / 2.0).max(1.0 * view.px());
        let color = if sel == Some(EntityRef::Wall(wall.id)) {
            c::TEAL
        } else if hover_hit == Some(Hit::Wall(wall.id)) {
            WALL_HOVER
        } else {
            c::WALL
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
                let quad = vec![to(p0 + n * ht), to(p1 + n * ht), to(p1 - n * ht), to(p0 - n * ht)];
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

    // Wall-face paint stripes
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
        let off = n * (w.thickness / 2.0 + 3.0 * view.px());
        painter.line_segment([to(a + off), to(b + off)], Stroke::new(4.0, rgb(col)));
    }

    // Doors & windows
    for o in level.openings.values() {
        let Some((s, e, dir)) = level.opening_segment(o) else { continue };
        let t = level.walls.get(&o.wall).map(|w| w.thickness).unwrap_or(0.1) / 2.0;
        let n = dir.perp();
        let gap = vec![to(s + n * t), to(e + n * t), to(e - n * t), to(s - n * t)];
        let selected = sel == Some(EntityRef::Opening(o.id));
        let hovered = hover_hit == Some(Hit::Opening(o.id));
        let color = if selected { c::TEAL } else if hovered { WALL_HOVER } else { c::WALL };
        let thin = Stroke::new(if selected { 2.0 } else { 1.0 }, color);
        match o.kind {
            OpeningKind::Window(_) => {
                painter.add(EShape::convex_polygon(gap, Color32::WHITE, thin));
                painter.line_segment([to(s), to(e)], thin);
            }
            OpeningKind::Door(style) => {
                painter.add(EShape::convex_polygon(gap, THRESHOLD, Stroke::NONE));
                let swing = if o.flip_swing { -n } else { n };
                let leaf = |hinge: Vec2, latch: Vec2| {
                    let v0 = latch - hinge;
                    let len = v0.length();
                    let v1 = swing * len;
                    let sign = if v0.cross(v1) >= 0.0 { 1.0 } else { -1.0 };
                    let arc: Vec<Pos2> = (0..=24)
                        .map(|k| to(hinge + v0.rotated(sign * std::f64::consts::FRAC_PI_2 * k as f64 / 24.0)))
                        .collect();
                    painter.add(EShape::line(arc, Stroke::new(1.0, color.gamma_multiply(0.55))));
                    painter.line_segment([to(hinge), to(hinge + v1)], Stroke::new(thin.width + 1.0, color));
                };
                match style {
                    DoorStyle::Hinged | DoorStyle::Other => {
                        if o.flip_hinge {
                            leaf(e, s)
                        } else {
                            leaf(s, e)
                        }
                    }
                    DoorStyle::Double => {
                        let mid = s.lerp(e, 0.5);
                        leaf(s, mid);
                        leaf(e, mid);
                    }
                    _ => {
                        let k = t * 0.45;
                        let mid = s.lerp(e, 0.5);
                        painter.line_segment([to(s + n * k), to(mid.lerp(e, 0.15) + n * k)], Stroke::new(2.0, color));
                        painter.line_segment([to(mid.lerp(s, 0.15) - n * k), to(e - n * k)], Stroke::new(2.0, color));
                    }
                }
            }
        }
    }

    // Items: line-art footprints with a category dot
    for obj in level.objects.values() {
        let outline: Vec<Pos2> = obj.world_outline().into_iter().map(to).collect();
        let selected = sel == Some(EntityRef::Object(obj.id));
        let hovered = hover_hit == Some(Hit::Object(obj.id));
        let (fill, stroke) = if selected {
            (ROOM_SELECTED, Stroke::new(2.0, c::TEAL))
        } else if hovered {
            (Color32::WHITE, Stroke::new(1.5, c::INK))
        } else {
            (ITEM_FILL, Stroke::new(1.2, ITEM_STROKE))
        };
        painter.add(EShape::convex_polygon(outline, fill, stroke));
        let min_side = obj.shape.width.min(obj.shape.depth) as f32 * view.zoom;
        if min_side > 24.0 || selected {
            let font = FontId::new(11.0, theme::medium());
            let galley = painter.layout_no_wrap(obj.display_name(), font, c::MUTED);
            let center = to(obj.position);
            let total = galley.size().x + 10.0;
            let dot = center - vec2(total / 2.0 - 3.0, 0.0);
            painter.circle_filled(dot, 3.0, rgb(obj.display_color()));
            painter.galley(
                pos2(dot.x + 7.0, center.y - galley.size().y / 2.0),
                galley,
                c::MUTED,
            );
        }
    }

    // Selected room outline + handles
    if let Some(EntityRef::Room(rid)) = sel {
        if let Some(room) = level.rooms.get(&rid) {
            let pts: Vec<Pos2> = level.room_points(room).into_iter().map(to).collect();
            painter.add(EShape::closed_line(pts.clone(), Stroke::new(2.0, c::TEAL)));
            if !room.shape_locked {
                for p in pts {
                    painter.circle_filled(p, 5.5, Color32::WHITE);
                    painter.circle_stroke(p, 5.5, Stroke::new(2.0, c::TEAL));
                }
            }
        }
    }

    // Labels & dimensions
    for room in level.rooms.values() {
        let pts = level.room_points(room);
        if pts.len() < 3 {
            continue;
        }
        let Some((lo, hi)) = bounds(&pts) else { continue };
        let size_px = ((hi.x - lo.x).min(hi.y - lo.y)) as f32 * view.zoom;
        if size_px > 36.0 {
            let c = to(label_point(&pts));
            painter.text(c - vec2(0.0, 8.0), Align2::CENTER_CENTER, room.display_name(), FontId::new(13.5, theme::semibold()), c::INK);
            painter.text(
                c + vec2(0.0, 9.0),
                Align2::CENTER_CENTER,
                format_area(area(&pts), units),
                FontId::proportional(11.5),
                c::MUTED,
            );
        }
        let selected = sel == Some(EntityRef::Room(room.id));
        if st.show_dims || selected {
            let n = pts.len();
            for i in 0..n {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                let len = a.distance(b);
                if len as f32 * view.zoom < 40.0 {
                    continue;
                }
                let outward = -(b - a).perp().normalized();
                let pos = to(a.lerp(b, 0.5) + outward * (16.0 * view.px()));
                let (sa, sb) = (to(a), to(b));
                let angle = (sb.y - sa.y).atan2(sb.x - sa.x);
                let color = if selected { c::TEAL } else { c::MUTED };
                text_along(painter, pos, angle, format_length(len, units), FontId::new(11.5, theme::medium()), color);
            }
        }
    }
    for wall in level.walls.values() {
        if wall.label.trim().is_empty() {
            continue;
        }
        let Some((a, b)) = level.wall_points(wall) else { continue };
        let n = (b - a).perp().normalized();
        let (sa, sb) = (to(a), to(b));
        text_along(
            painter,
            to(a.lerp(b, 0.5) + n * (wall.thickness / 2.0 + 10.0 * view.px())),
            (sb.y - sa.y).atan2(sb.x - sa.x),
            wall.label.clone(),
            FontId::proportional(10.5),
            c::FAINT,
        );
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
    shift: bool,
    units: UnitSystem,
) {
    let level = &project.levels[li];
    let view = &st.view;
    let to = |p: Vec2| view.to_screen(rect, p);
    let accent = Stroke::new(2.0, c::TEAL);
    let label = |p: Pos2, s: String| {
        let galley = painter.layout_no_wrap(s, FontId::new(12.0, theme::medium()), Color32::WHITE);
        let r = Rect::from_center_size(p, galley.size() + vec2(14.0, 6.0));
        painter.rect_filled(r, 8.0, c::TEAL);
        painter.galley(r.center() - galley.size() / 2.0, galley, Color32::WHITE);
    };

    match &st.draft {
        Draft::Rect { start } | Draft::Object { start } => {
            if let Some(end) = st.last_world {
                let (lo, hi) = (start.min(end), start.max(end));
                let pts: Vec<Pos2> = rect_points(lo, hi.x - lo.x, hi.y - lo.y).into_iter().map(to).collect();
                painter.add(EShape::convex_polygon(pts, c::TEAL.gamma_multiply(0.08), accent));
                label(
                    to((lo + hi) / 2.0),
                    format!("{} × {}", format_length(hi.x - lo.x, units), format_length(hi.y - lo.y, units)),
                );
            }
        }
        Draft::Poly { points } => {
            let mut pts: Vec<Pos2> = points.iter().map(|p| to(*p)).collect();
            if let Some(mut h) = hover_snapped {
                if shift {
                    if let Some(&last) = points.last() {
                        h = ortho(last, h);
                    }
                }
                if let Some(&last) = points.last() {
                    label(to(last.lerp(h, 0.5)) + vec2(0.0, -16.0), format_length(last.distance(h), units));
                }
                pts.push(to(h));
            }
            painter.add(EShape::line(pts, accent));
            for (i, p) in points.iter().enumerate() {
                painter.circle_filled(to(*p), if i == 0 { 6.0 } else { 4.5 }, Color32::WHITE);
                painter.circle_stroke(to(*p), if i == 0 { 6.0 } else { 4.5 }, accent);
            }
        }
        Draft::Calibrate { first } => {
            painter.circle_filled(to(*first), 5.0, c::TEAL);
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
                        Stroke::new(7.0, c::TEAL.gamma_multiply(0.8)),
                    );
                }
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
                painter.add(EShape::closed_line(outline, Stroke::new(1.5, c::TEAL)));
            }
        }
        Tool::RectRoom | Tool::PolyRoom if matches!(st.draft, Draft::None) => {
            if let Some(p) = hover_snapped {
                painter.circle_stroke(to(p), 5.0, Stroke::new(1.5, c::TEAL));
            }
        }
        _ => {}
    }
}

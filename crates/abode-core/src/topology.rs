//! Floor-plan topology: rooms are loops of shared nodes, walls are the
//! unique edges between them. Adding a room that touches an existing one
//! splits walls as needed so the touching part becomes a single shared wall.

use crate::geometry::*;
use crate::ids::*;
use crate::model::*;
use crate::outline::{self, CutError, Outline, split_bulge};
use std::collections::BTreeSet;

/// Corners closer than this (metres) are merged.
pub const SNAP_TOLERANCE: f64 = 0.02;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TopologyError {
    #[error("a room needs at least three corners")]
    TooFewPoints,
    #[error("that shape is too small to be a room")]
    Degenerate,
    #[error("room outlines can't cross themselves")]
    SelfIntersecting,
    #[error("room not found")]
    NotFound,
    #[error("this room isn't a simple rectangle, drag its corners instead")]
    NotRectangle,
    #[error("can't shrink past a corner shared with another room")]
    BlockedByJunction,
    #[error("this room shares walls with another room, so it can't move on its own")]
    Attached,
    #[error("rooms can't overlap")]
    Overlaps,
    #[error("both sides are against locked rooms; unlock one of them to make this room wider or deeper")]
    BetweenLocked,
    #[error("that's all inside another room")]
    Inside,
    #[error("another room would be left inside it, like a hole")]
    Surrounds,
    #[error("the room's shape is locked; unlock it first")]
    Locked,
    #[error("that corner is where other walls join, so it has to stay")]
    Junction,
    #[error("that would cut the room it's in into separate pieces")]
    SplitsRoom,
    #[error("that would take up the whole of the room it's in")]
    FillsRoom,
}

/// Side effects of a topology edit, so the project can fix up references.
#[derive(Debug, Default, Clone)]
pub struct TopologyChanges {
    /// (original wall, new second half)
    pub split_walls: Vec<(WallId, WallId)>,
    pub removed_walls: Vec<WallId>,
    pub removed_openings: Vec<OpeningId>,
    pub removed_objects: Vec<ObjectId>,
    /// (removed duplicate wall, the wall it became) when two rooms joined up.
    pub merged_walls: Vec<(WallId, WallId)>,
}

fn edge_key(a: NodeId, b: NodeId) -> (NodeId, NodeId) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Area shared by two simple polygons (triangulate, clip triangle pairs).
fn overlap_area(a: &[Vec2], b: &[Vec2]) -> f64 {
    let tris = |p: &[Vec2]| -> Vec<[Vec2; 3]> {
        let p = positive_winding(p);
        triangulate(&p).into_iter().map(|[i, j, k]| [p[i], p[j], p[k]]).collect()
    };
    let (ta, tb) = (tris(a), tris(b));
    let mut total = 0.0;
    for x in &ta {
        for y in &tb {
            let mut poly = x.to_vec();
            for i in 0..3 {
                let (p, q) = (y[i], y[(i + 1) % 3]);
                let d = q - p;
                let n = Vec2::new(d.y, -d.x);
                poly = clip_half_plane(&poly, n, n.dot(p));
                if poly.len() < 3 {
                    break;
                }
            }
            if poly.len() >= 3 {
                total += area(&poly);
            }
        }
    }
    total
}

/// True if `inner` is wholly inside `outer` without touching it, and more
/// than `gap` clear of its walls.
fn clear_inside(inner: &[Vec2], outer: &[Vec2], gap: f64) -> bool {
    let (Some((lo, hi)), Some((olo, ohi))) = (bounds(inner), bounds(outer)) else { return false };
    if lo.x <= olo.x + gap || lo.y <= olo.y + gap || hi.x >= ohi.x - gap || hi.y >= ohi.y - gap {
        return false;
    }
    if !inner.iter().all(|p| point_in_polygon(*p, outer)) {
        return false;
    }
    let (n, m) = (inner.len(), outer.len());
    for i in 0..n {
        let (p, q) = (inner[i], inner[(i + 1) % n]);
        for j in 0..m {
            let (r, t) = (outer[j], outer[(j + 1) % m]);
            if segments_cross(p, q, r, t) || distance_to_segment(r, p, q) <= gap || distance_to_segment(p, r, t) <= gap {
                return false;
            }
        }
    }
    true
}

/// The outline going anticlockwise.
fn anticlockwise(o: &[(Vec2, f64)]) -> Outline {
    if outline::signed_area_of(o) < 0.0 { outline::reverse(o) } else { o.to_vec() }
}

fn extend_changes(into: &mut TopologyChanges, more: TopologyChanges) {
    into.split_walls.extend(more.split_walls);
    into.removed_walls.extend(more.removed_walls);
    into.removed_openings.extend(more.removed_openings);
    into.removed_objects.extend(more.removed_objects);
    into.merged_walls.extend(more.merged_walls);
}

fn has_edge(nodes: &[NodeId], a: NodeId, b: NodeId) -> bool {
    let n = nodes.len();
    (0..n).any(|i| {
        let (x, y) = (nodes[i], nodes[(i + 1) % n]);
        (x == a && y == b) || (x == b && y == a)
    })
}

impl Level {
    pub fn node_pos(&self, id: NodeId) -> Option<Vec2> {
        self.nodes.get(&id).map(|n| n.pos)
    }

    pub fn room_points(&self, room: &Room) -> Vec<Vec2> {
        room.nodes.iter().filter_map(|n| self.node_pos(*n)).collect()
    }

    pub fn wall_points(&self, wall: &Wall) -> Option<(Vec2, Vec2)> {
        Some((self.node_pos(wall.a)?, self.node_pos(wall.b)?))
    }

    /// Length along the wall (around the curve, for a curved wall).
    pub fn wall_length(&self, wall: &Wall) -> f64 {
        self.wall_points(wall).map(|(a, b)| arc_length(a, b, wall.bulge)).unwrap_or(0.0)
    }

    /// The wall's centre line from `a` to `b`: two points, or many for a curve.
    pub fn wall_polyline(&self, wall: &Wall) -> Option<Vec<Vec2>> {
        let (a, b) = self.wall_points(wall)?;
        Some(arc_polyline(a, b, wall.bulge))
    }

    /// Halfway along the wall.
    pub fn wall_mid(&self, wall: &Wall) -> Option<Vec2> {
        let (a, b) = self.wall_points(wall)?;
        Some(arc_mid(a, b, wall.bulge))
    }

    /// Point and direction `s` metres along the wall from `a`.
    pub fn wall_point_at(&self, wall: &Wall, s: f64) -> Option<(Vec2, Vec2)> {
        let (a, b) = self.wall_points(wall)?;
        Some(arc_point(a, b, wall.bulge, s))
    }

    /// Points and directions along the wall from `s0` to `s1` metres (either
    /// may run past the ends), for drawing a stretch of it.
    pub fn wall_samples(&self, wall: &Wall, s0: f64, s1: f64) -> Vec<(Vec2, Vec2)> {
        match self.wall_points(wall) {
            Some((a, b)) => arc_samples(a, b, wall.bulge, s0, s1),
            None => Vec::new(),
        }
    }

    /// The room's floor outline, following curved walls. Use this for areas,
    /// drawing and hit tests; [`Level::room_points`] is just the corners.
    pub fn room_outline(&self, room: &Room) -> Vec<Vec2> {
        let n = room.nodes.len();
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let (u, v) = (room.nodes[i], room.nodes[(i + 1) % n]);
            let Some(pu) = self.node_pos(u) else { continue };
            out.push(pu);
            let curved = self.wall_between(u, v).and_then(|w| self.walls.get(&w)).filter(|w| w.is_curved());
            if let (Some(w), Some(pv)) = (curved, self.node_pos(v)) {
                let bulge = if w.a == u { w.bulge } else { -w.bulge };
                let arc = arc_polyline(pu, pv, bulge);
                out.extend_from_slice(&arc[1..arc.len() - 1]);
            }
        }
        out
    }

    /// True if any of the room's walls is curved.
    pub fn room_has_curves(&self, room: &Room) -> bool {
        self.walls_of_room(room).iter().any(|w| self.walls.get(w).is_some_and(|w| w.is_curved()))
    }

    pub fn wall_height(&self, wall: &Wall) -> f64 {
        wall.height.unwrap_or(self.wall_height)
    }

    pub fn wall_between(&self, a: NodeId, b: NodeId) -> Option<WallId> {
        let k = edge_key(a, b);
        self.walls.values().find(|w| edge_key(w.a, w.b) == k).map(|w| w.id)
    }

    /// Rooms on either side of a wall (one for exterior walls, two for interior).
    pub fn rooms_of_wall(&self, wall: &Wall) -> Vec<RoomId> {
        self.rooms
            .values()
            .filter(|r| has_edge(&r.nodes, wall.a, wall.b))
            .map(|r| r.id)
            .collect()
    }

    /// Walls around a room, in outline order.
    pub fn walls_of_room(&self, room: &Room) -> Vec<WallId> {
        let n = room.nodes.len();
        (0..n)
            .filter_map(|i| self.wall_between(room.nodes[i], room.nodes[(i + 1) % n]))
            .collect()
    }

    /// The smallest room containing `p`.
    pub fn room_at(&self, p: Vec2) -> Option<RoomId> {
        self.rooms
            .values()
            .filter_map(|r| {
                let pts = self.room_outline(r);
                point_in_polygon(p, &pts).then(|| (area(&pts), r.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }

    /// True if `inner` is a room within `outer`: wholly inside it, not
    /// touching its walls (a room that reaches them is cut out of it instead).
    pub fn room_inside(&self, inner: RoomId, outer: RoomId) -> bool {
        let (Some(a), Some(b)) = (self.rooms.get(&inner), self.rooms.get(&outer)) else { return false };
        inner != outer && !a.nodes.iter().any(|n| b.nodes.contains(n)) && clear_inside(&self.room_outline(a), &self.room_outline(b), 0.0)
    }

    /// For each room within a room, the room it's directly inside.
    pub fn rooms_around(&self) -> std::collections::BTreeMap<RoomId, RoomId> {
        let rooms: Vec<(RoomId, Vec<Vec2>, f64)> = self
            .rooms
            .values()
            .map(|r| {
                let pts = self.room_outline(r);
                let a = area(&pts);
                (r.id, pts, a)
            })
            .collect();
        let mut out = std::collections::BTreeMap::new();
        for (id, pts, _) in &rooms {
            let nodes = &self.rooms[id].nodes;
            let around = rooms
                .iter()
                .filter(|(o, opts, _)| o != id && !self.rooms[o].nodes.iter().any(|n| nodes.contains(n)) && clear_inside(pts, opts, 0.0))
                .min_by(|a, b| a.2.total_cmp(&b.2));
            if let Some((o, _, _)) = around {
                out.insert(*id, *o);
            }
        }
        out
    }

    /// The room this one is inside, if it's a room within a room.
    pub fn room_around(&self, room: RoomId) -> Option<RoomId> {
        self.rooms_around().get(&room).copied()
    }

    /// Every room inside this one, however deep.
    pub fn rooms_inside(&self, room: RoomId) -> Vec<RoomId> {
        self.rooms.keys().copied().filter(|r| self.room_inside(*r, room)).collect()
    }

    /// Floor area of a room, less any rooms within it.
    pub fn room_area(&self, room: &Room) -> f64 {
        self.room_area_with(room, &self.rooms_around())
    }

    /// [`Level::room_area`] with [`Level::rooms_around`] already worked out.
    pub fn room_area_with(&self, room: &Room, around: &std::collections::BTreeMap<RoomId, RoomId>) -> f64 {
        let within: f64 = around.iter().filter(|(_, o)| **o == room.id).map(|(r, _)| area(&self.room_outline(&self.rooms[r]))).sum();
        (area(&self.room_outline(room)) - within).max(0.0)
    }

    /// Rooms biggest first, so drawn in this order a room within a room ends
    /// up on top.
    pub fn rooms_outside_in(&self) -> Vec<&Room> {
        let mut rooms: Vec<(f64, &Room)> = self.rooms.values().map(|r| (area(&self.room_outline(r)), r)).collect();
        rooms.sort_by(|a, b| b.0.total_cmp(&a.0));
        rooms.into_iter().map(|(_, r)| r).collect()
    }

    /// Where to put a room's name: in the middle of its floor, clear of any
    /// rooms within it.
    pub fn room_label_point(&self, room: &Room) -> Vec2 {
        let pts = self.room_outline(room);
        let holes: Vec<Vec<Vec2>> =
            self.rooms_around().iter().filter(|(_, o)| **o == room.id).map(|(r, _)| self.room_outline(&self.rooms[r])).collect();
        let plain = label_point(&pts);
        if holes.is_empty() {
            return plain;
        }
        // The open floor point furthest from any wall.
        let Some((lo, hi)) = bounds(&pts) else { return plain };
        let clearance = |p: Vec2| {
            std::iter::once(&pts).chain(holes.iter()).map(|poly| distance_to_polyline(&[poly.as_slice(), &poly[..1]].concat(), p)).fold(f64::MAX, f64::min)
        };
        let steps = 24;
        let mut best: Option<(f64, Vec2)> = None;
        for i in 0..=steps {
            for j in 0..=steps {
                let p = Vec2::new(lo.x + (hi.x - lo.x) * i as f64 / steps as f64, lo.y + (hi.y - lo.y) * j as f64 / steps as f64);
                if !point_in_polygon(p, &pts) || holes.iter().any(|h| point_in_polygon(p, h)) {
                    continue;
                }
                let c = clearance(p);
                if best.is_none_or(|b| c > b.0) {
                    best = Some((c, p));
                }
            }
        }
        best.map_or(plain, |b| b.1)
    }

    /// Rooms on either side of a wall. A room within a room has the room
    /// it's in on the other side of its outer walls.
    pub fn rooms_beside_wall(&self, wall: &Wall) -> Vec<RoomId> {
        let mut rooms = self.rooms_of_wall(wall);
        if rooms.len() == 1 {
            rooms.extend(self.room_around(rooms[0]));
        }
        rooms
    }

    pub fn find_node_near(&self, p: Vec2, tol: f64) -> Option<NodeId> {
        self.nodes
            .values()
            .map(|n| (n.pos.distance(p), n.id))
            .filter(|(d, _)| *d <= tol)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }

    /// Nearest wall to `p` within `max_dist`: (wall, offset from `a`, distance).
    pub fn nearest_wall(&self, p: Vec2, max_dist: f64) -> Option<(WallId, f64, f64)> {
        self.walls
            .values()
            .filter_map(|w| {
                let (a, b) = self.wall_points(w)?;
                let (s, d) = arc_nearest(a, b, w.bulge, p);
                (d <= max_dist).then_some((w.id, s, d))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
    }

    /// Endpoints of an opening along its wall, plus the direction from one
    /// to the other. On a curved wall the ends are on the curve and the
    /// direction is straight across.
    pub fn opening_segment(&self, o: &Opening) -> Option<(Vec2, Vec2, Vec2)> {
        let wall = self.walls.get(&o.wall)?;
        let (s, _) = self.wall_point_at(wall, o.offset - o.width / 2.0)?;
        let (e, dir) = self.wall_point_at(wall, o.offset + o.width / 2.0)?;
        let across = (e - s).normalized();
        Some((s, e, if across == Vec2::ZERO { dir } else { across }))
    }

    /// Which side of `wall` `room` is on: +1 if it's towards the wall's
    /// direction turned by [`Vec2::perp`], -1 if the other way. The same
    /// all along a curved wall.
    pub fn wall_side_of(&self, wall: &Wall, room: RoomId) -> Option<f64> {
        let pts = self.room_outline(self.rooms.get(&room)?);
        let len = self.wall_length(wall);
        let (mid, dir) = self.wall_point_at(wall, len / 2.0)?;
        let probe = (len * 0.01).clamp(0.005, 0.05);
        Some(if point_in_polygon(mid + dir.perp() * probe, &pts) { 1.0 } else { -1.0 })
    }

    /// Unit normal of `wall` pointing into `room` (at its middle, for a curve).
    pub fn wall_normal_into(&self, wall: &Wall, room: RoomId) -> Option<Vec2> {
        let side = self.wall_side_of(wall, room)?;
        let (_, dir) = self.wall_point_at(wall, self.wall_length(wall) / 2.0)?;
        Some(dir.perp() * side)
    }

    /// Bend a wall: `bulge` 0 straightens it (see [`Wall::bulge`]). Fails,
    /// changing nothing, if a room would then cross itself or overlap
    /// another. Doors and windows keep their place along the wall.
    pub fn set_wall_bulge(&mut self, wall_id: WallId, bulge: f64) -> Result<(), TopologyError> {
        let wall = self.walls.get(&wall_id).ok_or(TopologyError::NotFound)?;
        let bulge = bulge.clamp(-MAX_BULGE, MAX_BULGE);
        let old_len = self.wall_length(wall);
        let old = wall.bulge;
        let rooms: BTreeSet<RoomId> = self.rooms_of_wall(wall).into_iter().collect();
        let fractions: Vec<(OpeningId, f64)> = self
            .openings
            .values()
            .filter(|o| o.wall == wall_id)
            .map(|o| (o.id, if old_len > 0.0 { o.offset / old_len } else { 0.5 }))
            .collect();
        self.walls.get_mut(&wall_id).unwrap().bulge = bulge;
        if let Err(e) = self.check_rooms(&rooms) {
            self.walls.get_mut(&wall_id).unwrap().bulge = old;
            return Err(e);
        }
        let new_len = self.wall_length(&self.walls[&wall_id]);
        for (id, f) in fractions {
            if let Some(o) = self.openings.get_mut(&id) {
                o.offset = f * new_len;
            }
        }
        self.clamp_openings();
        Ok(())
    }

    pub fn bounds(&self) -> Option<(Vec2, Vec2)> {
        let mids: Vec<Vec2> = self.walls.values().filter(|w| w.is_curved()).filter_map(|w| self.wall_mid(w)).collect();
        bounds(self.nodes.values().map(|n| &n.pos).chain(mids.iter()))
    }

    /// Add a room from an outline. Corners snap to existing corners within
    /// `tol`, and walls it touches are split so they're shared.
    pub fn add_room(
        &mut self,
        name: &str,
        points: &[Vec2],
        tol: f64,
        thickness: f64,
    ) -> Result<(RoomId, TopologyChanges), TopologyError> {
        let shape: Outline = points.iter().map(|p| (*p, 0.0)).collect();
        self.add_room_shape(name, &shape, tol, thickness)
    }

    /// Add a room whose sides may be curved (see [`crate::outline`]).
    pub fn add_room_shape(
        &mut self,
        name: &str,
        shape: &[(Vec2, f64)],
        tol: f64,
        thickness: f64,
    ) -> Result<(RoomId, TopologyChanges), TopologyError> {
        let backup = self.clone();
        let room = Room::new(name, Vec::new());
        let id = room.id;
        self.rooms.insert(id, room);
        match self.place_room(id, shape, tol, thickness) {
            Ok(changes) => Ok((id, changes)),
            Err(e) => {
                *self = backup;
                Err(e)
            }
        }
    }

    /// Like [`Level::add_room_shape`], but where it overlaps other rooms it's
    /// trimmed back to their walls, which it then shares. Drawn (mostly)
    /// inside another room, it's a room within that room: kept inside it, and
    /// where it reaches that room's walls it's cut out of it instead.
    pub fn add_room_fitted(
        &mut self,
        name: &str,
        shape: &[(Vec2, f64)],
        tol: f64,
        thickness: f64,
    ) -> Result<(RoomId, TopologyChanges), TopologyError> {
        let host = self.host_for(shape, None);
        let (fitted, carve) = self.make_room_for(shape, None, host, tol)?;
        let backup = self.clone();
        let result = self.add_room_carving(name, &fitted, carve, tol, thickness);
        if result.is_err() {
            *self = backup;
        }
        result
    }

    fn add_room_carving(
        &mut self,
        name: &str,
        shape: &[(Vec2, f64)],
        carve: Option<(RoomId, Outline)>,
        tol: f64,
        thickness: f64,
    ) -> Result<(RoomId, TopologyChanges), TopologyError> {
        let (id, mut changes) = self.add_room_shape(name, shape, tol, thickness)?;
        let mut check: BTreeSet<RoomId> = [id].into();
        if let Some((host, rest)) = carve {
            extend_changes(&mut changes, self.place_room(host, &rest, tol, thickness)?);
            self.move_items(host, id);
            check.insert(host);
        }
        self.check_rooms(&check)?;
        Ok((id, changes))
    }

    /// Items of `from` that now stand in `to` belong to it.
    fn move_items(&mut self, from: RoomId, to: RoomId) {
        let Some(room) = self.rooms.get(&to) else { return };
        let pts = self.room_outline(room);
        for o in self.objects.values_mut().filter(|o| o.room == Some(from) && o.roof.is_none()) {
            if point_in_polygon(o.position, &pts) {
                o.room = Some(to);
            }
        }
    }

    /// The room `shape` is mostly inside (the smallest, if they're nested),
    /// leaving out `me` and the rooms within it.
    fn host_for(&self, shape: &[(Vec2, f64)], me: Option<RoomId>) -> Option<RoomId> {
        let flat = outline::tessellate(shape);
        let size = area(&flat);
        self.rooms
            .values()
            .filter(|r| Some(r.id) != me && !me.is_some_and(|m| self.room_inside(r.id, m)))
            .filter_map(|r| {
                let pts = self.room_outline(r);
                (overlap_area(&flat, &pts) > 0.5 * size).then(|| (area(&pts), r.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }

    /// Work out the outline for a room (`me`, or a new one) shaped like
    /// `shape`. Inside `host` it's kept inside it; where it then reaches the
    /// host's walls, the second part of the answer is what's left of the host
    /// once it's cut out. Rooms it overlaps are cut out of it; rooms it goes
    /// right round stay as rooms within it.
    fn make_room_for(
        &self,
        shape: &[(Vec2, f64)],
        me: Option<RoomId>,
        host: Option<RoomId>,
        tol: f64,
    ) -> Result<(Outline, Option<(RoomId, Outline)>), TopologyError> {
        let shape = anticlockwise(shape);
        let Some(h) = host else { return Ok((self.fit_outline(&shape, me, None)?, None)) };
        let theirs = anticlockwise(&self.room_shape(&self.rooms[&h]));
        let inside = outline::intersect(&shape, &theirs).into_iter().next().ok_or(TopologyError::Inside)?;
        let inside = self.snap_to_room(&inside, h, tol);
        let mut fitted = self.fit_outline(&inside, me, Some(h))?;
        if clear_inside(&outline::tessellate(&fitted), &self.room_outline(&self.rooms[&h]), tol) {
            return Ok((fitted, None));
        }
        let mut rest = match outline::subtract(&theirs, &fitted) {
            Ok(parts) => parts,
            Err(CutError::Covered) => return Err(TopologyError::FillsRoom),
            // Close to its walls but not touching them.
            Err(CutError::Hole) => return Ok((fitted, None)),
        };
        let keep = rest.remove(0);
        if !rest.is_empty() {
            // Slivers left between it and the host's walls (say a flat side
            // drawn against a curve) are part of it; anything bigger would be
            // a separate room.
            let mine = outline::signed_area_of(&fitted);
            if rest.iter().any(|p| outline::signed_area_of(p) > 0.5 * mine) {
                return Err(TopologyError::SplitsRoom);
            }
            fitted = match outline::subtract(&theirs, &keep) {
                Ok(mut p) if p.len() == 1 => p.remove(0),
                _ => return Err(TopologyError::SplitsRoom),
            };
        }
        Ok((fitted, Some((h, keep))))
    }

    /// Corners of `shape` within `tol` of `room`'s corners or walls move onto them.
    fn snap_to_room(&self, shape: &[(Vec2, f64)], room: RoomId, tol: f64) -> Outline {
        let r = &self.rooms[&room];
        let corners: Vec<Vec2> = r.nodes.iter().filter_map(|n| self.node_pos(*n)).collect();
        let walls: Vec<&Wall> = self.walls_of_room(r).iter().filter_map(|w| self.walls.get(w)).collect();
        shape
            .iter()
            .map(|&(p, bulge)| {
                if let Some(c) = corners.iter().find(|c| c.distance(p) <= tol) {
                    return (*c, bulge);
                }
                for w in &walls {
                    let Some((a, b)) = self.wall_points(w) else { continue };
                    let q = if w.is_curved() {
                        let (s, _) = arc_nearest(a, b, w.bulge, p);
                        arc_point(a, b, w.bulge, s).0
                    } else {
                        closest_point_on_segment(p, a, b).0
                    };
                    if q.distance(p) <= tol {
                        return (q, bulge);
                    }
                }
                (p, bulge)
            })
            .collect()
    }

    /// A room's outline, with its curves.
    pub fn room_shape(&self, room: &Room) -> Outline {
        let n = room.nodes.len();
        (0..n)
            .filter_map(|i| {
                let (u, v) = (room.nodes[i], room.nodes[(i + 1) % n]);
                let bulge = self.wall_between(u, v).and_then(|w| self.walls.get(&w)).map_or(0.0, |w| if w.a == u { w.bulge } else { -w.bulge });
                Some((self.node_pos(u)?, bulge))
            })
            .collect()
    }

    /// `shape` with every room it overlaps (other than `me`, and `host` and
    /// the rooms that's in) cut out of it, so it fits up against them. If
    /// that splits it in two, the bigger part. Rooms it goes right round are
    /// left alone, as rooms within it.
    pub fn fit_outline(&self, shape: &[(Vec2, f64)], me: Option<RoomId>, host: Option<RoomId>) -> Result<Outline, TopologyError> {
        let mut out = anticlockwise(shape);
        for r in self.rooms.values().filter(|r| Some(r.id) != me) {
            if host.is_some_and(|h| h == r.id || self.room_inside(h, r.id)) {
                continue;
            }
            let theirs = positive_winding(&self.room_outline(r));
            if overlap_area(&outline::tessellate(&out), &theirs) <= 0.01 {
                continue;
            }
            let cutter = anticlockwise(&self.room_shape(r));
            out = match outline::subtract(&out, &cutter) {
                Ok(mut parts) => parts.swap_remove(0),
                Err(CutError::Covered) => return Err(TopologyError::Inside),
                Err(CutError::Hole) => continue,
            };
        }
        Ok(out)
    }

    /// If `p` lies partway along `wall` (within `tol`): how far along, 0..1.
    fn point_on_wall(&self, wall: &Wall, p: Vec2, tol: f64) -> Option<f64> {
        let (a, b) = self.wall_points(wall)?;
        let t = if wall.is_curved() {
            let (s, d) = arc_nearest(a, b, wall.bulge, p);
            (d <= tol).then(|| s / arc_length(a, b, wall.bulge))?
        } else {
            let (c, t) = closest_point_on_segment(p, a, b);
            (c.distance(p) <= tol).then_some(t)?
        };
        (t > 1e-6 && t < 1.0 - 1e-6 && p.distance(a) > tol && p.distance(b) > tol).then_some(t)
    }

    /// Give `room` the outline `shape`: corners snap to nearby corners, walls
    /// they land on are split, and corners lying along its sides are picked
    /// up, so it shares walls with whatever it touches. Its own walls get the
    /// shape's curves; walls it shares keep theirs.
    fn place_room(&mut self, room_id: RoomId, shape: &[(Vec2, f64)], tol: f64, thickness: f64) -> Result<TopologyChanges, TopologyError> {
        let mut pts: Outline = Vec::new();
        for &(p, bulge) in shape {
            if pts.last().is_none_or(|l| l.0.distance(p) > tol) {
                pts.push((p, bulge));
            }
        }
        while pts.len() > 1 && pts[0].0.distance(pts.last().unwrap().0) <= tol {
            pts.pop();
        }
        if pts.len() < 3 {
            return Err(TopologyError::TooFewPoints);
        }
        let flat = outline::tessellate(&pts);
        if !is_simple(&flat) {
            return Err(TopologyError::SelfIntersecting);
        }
        if area(&flat) < 0.05 {
            return Err(TopologyError::Degenerate);
        }
        if signed_area(&flat) < 0.0 {
            pts = outline::reverse(&pts);
        }

        let mut changes = TopologyChanges::default();
        let mut ids: Vec<NodeId> = Vec::with_capacity(pts.len());
        for (p, _) in &pts {
            let id = match self.find_node_near(*p, tol) {
                Some(id) => id,
                None => {
                    let node = Node { id: NodeId::new(), pos: *p };
                    let id = node.id;
                    self.nodes.insert(id, node);
                    id
                }
            };
            if ids.contains(&id) {
                self.remove_orphan_nodes();
                return Err(TopologyError::Degenerate);
            }
            ids.push(id);
        }

        // Split existing walls where one of our corners lands mid-wall.
        for &nid in &ids {
            let p = self.nodes[&nid].pos;
            let hits: Vec<WallId> = self
                .walls
                .values()
                .filter(|w| w.a != nid && w.b != nid)
                .filter(|w| self.point_on_wall(w, p, tol).is_some())
                .map(|w| w.id)
                .collect();
            for w in hits {
                self.split_wall_at(w, nid, &mut changes);
            }
        }

        // Pick up existing corners that lie along our new sides.
        let n = ids.len();
        let mut corners: Vec<(NodeId, f64)> = Vec::with_capacity(n);
        for i in 0..n {
            let (u, v) = (ids[i], ids[(i + 1) % n]);
            let bulge = pts[i].1;
            let (pu, pv) = (self.nodes[&u].pos, self.nodes[&v].pos);
            let side = Wall { bulge, ..Wall::new(u, v, thickness) };
            let mut on_edge: Vec<(f64, NodeId)> = self
                .nodes
                .values()
                .filter(|nd| !ids.contains(&nd.id) && nd.pos.distance(pu) > tol && nd.pos.distance(pv) > tol)
                .filter_map(|nd| self.point_on_wall(&side, nd.pos, tol).map(|t| (t, nd.id)))
                .collect();
            on_edge.sort_by(|a, b| a.0.total_cmp(&b.0));
            on_edge.retain(|(_, id)| !corners.iter().any(|c| c.0 == *id));
            let mut prev = 0.0;
            corners.push((u, 0.0));
            for (t, id) in on_edge {
                corners.last_mut().unwrap().1 = split_bulge(bulge, t - prev);
                corners.push((id, 0.0));
                prev = t;
            }
            corners.last_mut().unwrap().1 = split_bulge(bulge, 1.0 - prev);
        }

        self.rooms.get_mut(&room_id).ok_or(TopologyError::NotFound)?.nodes = corners.iter().map(|c| c.0).collect();
        self.rebuild_walls(thickness, &mut changes);
        // Our own walls take our curves; shared ones keep the neighbour's.
        let m = corners.len();
        for i in 0..m {
            let (u, v, bulge) = (corners[i].0, corners[(i + 1) % m].0, corners[i].1);
            let Some(wid) = self.wall_between(u, v) else { continue };
            let shared = self.rooms.values().any(|r| r.id != room_id && has_edge(&r.nodes, u, v));
            if !shared {
                let w = self.walls.get_mut(&wid).unwrap();
                w.bulge = if w.a == u { bulge } else { -bulge };
            }
        }
        self.clamp_openings();
        Ok(changes)
    }

    /// Add a corner partway along a wall, `offset` metres from its start,
    /// splitting it in two (each keeps its part of any curve). The shape
    /// doesn't change, so locked rooms are fine.
    pub fn add_corner(&mut self, wall_id: WallId, offset: f64) -> Result<(NodeId, TopologyChanges), TopologyError> {
        let wall = self.walls.get(&wall_id).ok_or(TopologyError::NotFound)?;
        let len = self.wall_length(wall);
        if offset < 0.05 || offset > len - 0.05 {
            return Err(TopologyError::Degenerate);
        }
        let (pos, _) = self.wall_point_at(wall, offset).ok_or(TopologyError::NotFound)?;
        let node = Node { id: NodeId::new(), pos };
        let id = node.id;
        self.nodes.insert(id, node);
        let mut changes = TopologyChanges::default();
        self.split_wall_at(wall_id, id, &mut changes);
        Ok((id, changes))
    }

    fn split_wall_at(&mut self, wall_id: WallId, node: NodeId, changes: &mut TopologyChanges) {
        let Some(wall) = self.walls.remove(&wall_id) else {
            return;
        };
        let (a, b) = (wall.a, wall.b);
        for room in self.rooms.values_mut() {
            let len = room.nodes.len();
            if let Some(i) = (0..len).find(|&i| {
                let (x, y) = (room.nodes[i], room.nodes[(i + 1) % len]);
                (x == a && y == b) || (x == b && y == a)
            }) {
                room.nodes.insert(i + 1, node);
            }
        }
        // Where along the wall the new corner is, for sharing out a curve.
        let (pa, pb, p) = (self.nodes[&a].pos, self.nodes[&b].pos, self.nodes[&node].pos);
        let t = if wall.is_curved() {
            arc_nearest(pa, pb, wall.bulge, p).0 / arc_length(pa, pb, wall.bulge)
        } else {
            closest_point_on_segment(p, pa, pb).1
        };
        let second_id = WallId::new();
        let mut first = wall.clone();
        first.b = node;
        first.bulge = split_bulge(wall.bulge, t);
        let mut second = wall;
        second.id = second_id;
        second.a = node;
        second.bulge = split_bulge(second.bulge, 1.0 - t);
        let first_len = arc_length(pa, p, first.bulge);
        for o in self.openings.values_mut().filter(|o| o.wall == wall_id) {
            if o.offset > first_len {
                o.wall = second_id;
                o.offset -= first_len;
            }
        }
        self.walls.insert(first.id, first);
        self.walls.insert(second_id, second);
        changes.split_walls.push((wall_id, second_id));
    }

    /// The two walls meeting at `node`, if just two do: (wall into it, its
    /// far corner), (wall out of it, its far corner).
    fn corner_walls(&self, node: NodeId) -> Option<[(WallId, NodeId); 2]> {
        let ws: Vec<&Wall> = self.walls.values().filter(|w| w.a == node || w.b == node).collect();
        let [w1, w2] = ws[..] else { return None };
        let far = |w: &Wall| if w.a == node { w.b } else { w.a };
        Some([(w1.id, far(w1)), (w2.id, far(w2))])
    }

    /// A corner partway along what is really one wall: the walls either side
    /// carry straight on, or follow the same curve. Removing it leaves the
    /// shape as it is (it's the kind of corner [`Level::add_corner`] makes).
    pub fn is_extra_corner(&self, node: NodeId) -> bool {
        self.extra_corner_sweep(node).is_some()
    }

    /// For an extra corner: the angle the joined-up wall would sweep (0 for
    /// a straight one).
    pub fn extra_corner_sweep(&self, node: NodeId) -> Option<f64> {
        let [(w1, p), (w2, q)] = self.corner_walls(node)?;
        let (pp, pn, pq) = (self.node_pos(p)?, self.node_pos(node)?, self.node_pos(q)?);
        let bulge_of = |w: WallId, from: NodeId| {
            let w = &self.walls[&w];
            if w.a == from { w.bulge } else { -w.bulge }
        };
        let (b1, b2) = (bulge_of(w1, p), bulge_of(w2, node));
        if b1 == 0.0 && b2 == 0.0 {
            let (d1, d2) = ((pn - pp).normalized(), (pq - pn).normalized());
            return (d1.cross(d2).abs() < 1e-3 && d1.dot(d2) > 0.0).then_some(0.0);
        }
        // Same circle: the arc through all three matches both halves.
        let whole = bulge_through(pp, pn, pq);
        let t = arc_length(pp, pn, b1) / (arc_length(pp, pn, b1) + arc_length(pn, pq, b2));
        let same = b1 != 0.0
            && b2 != 0.0
            && (outline::split_bulge(whole, t) - b1).abs() < 1e-3
            && (outline::split_bulge(whole, 1.0 - t) - b2).abs() < 1e-3;
        same.then(|| 4.0 * whole.atan().abs())
    }

    /// Take a corner out, joining the two walls that meet there into one
    /// (straight if both were straight, otherwise a curve through where the
    /// corner was). Doors and windows stay where they are. A corner where
    /// more walls meet has to stay. Taking out a real corner reshapes the
    /// room, so its room must be unlocked; an extra corner (see
    /// [`Level::is_extra_corner`]) can go from a locked room too.
    pub fn remove_corner(&mut self, node: NodeId) -> Result<TopologyChanges, TopologyError> {
        let [(w1, p), (w2, q)] = self.corner_walls(node).ok_or(TopologyError::Junction)?;
        let rooms: BTreeSet<RoomId> = self.rooms.values().filter(|r| r.nodes.contains(&node)).map(|r| r.id).collect();
        if !self.is_extra_corner(node) && rooms.iter().any(|r| self.rooms[r].shape_locked) {
            return Err(TopologyError::Locked);
        }
        if p == q || self.wall_between(p, q).is_some() {
            return Err(TopologyError::Degenerate);
        }
        let (pp, pn, pq) = (self.nodes[&p].pos, self.nodes[&node].pos, self.nodes[&q].pos);
        let straight = !self.walls[&w1].is_curved() && !self.walls[&w2].is_curved();
        let bulge = if straight { 0.0 } else { bulge_through(pp, pn, pq) };
        let backup = self.clone();
        // Where each door and window is now, to put it back on the new wall.
        let spots: Vec<(OpeningId, Vec2)> = self
            .openings
            .values()
            .filter(|o| o.wall == w1 || o.wall == w2)
            .filter_map(|o| Some((o.id, self.wall_point_at(&self.walls[&o.wall], o.offset)?.0)))
            .collect();
        for r in self.rooms.values_mut() {
            r.nodes.retain(|n| *n != node);
        }
        let mut changes = TopologyChanges::default();
        self.walls.remove(&w2);
        changes.merged_walls.push((w2, w1));
        let w = self.walls.get_mut(&w1).unwrap();
        (w.a, w.b, w.bulge) = (p, q, bulge);
        self.nodes.remove(&node);
        for (id, at) in spots {
            let o = self.openings.get_mut(&id).unwrap();
            o.wall = w1;
            o.offset = arc_nearest(pp, pq, bulge, at).0;
        }
        if let Err(e) = self.check_rooms(&rooms) {
            *self = backup;
            return Err(e);
        }
        self.clamp_openings();
        Ok(changes)
    }

    /// Take out a room's extra corners: ones partway along a straight wall or
    /// a curve, which don't change its shape. Straight ones go first, then
    /// the smallest curves; a curved room keeps at least four corners (a
    /// round room's own corners are extra too). Returns how many went.
    pub fn remove_extra_corners(&mut self, room: RoomId) -> (usize, TopologyChanges) {
        let mut all = TopologyChanges::default();
        let mut removed = 0;
        let next = |l: &Level| {
            let r = l.rooms.get(&room)?;
            let corners = r.nodes.len();
            r.nodes
                .iter()
                .filter_map(|n| Some((l.extra_corner_sweep(*n)?, *n)))
                .filter(|(sweep, _)| *sweep == 0.0 || corners > 4)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, n)| n)
        };
        while let Some(n) = next(self) {
            match self.remove_corner(n) {
                Ok(c) => {
                    all.merged_walls.extend(c.merged_walls);
                    removed += 1;
                }
                Err(_) => break,
            }
        }
        (removed, all)
    }

    /// Make the wall set match the room outlines exactly.
    pub fn rebuild_walls(&mut self, thickness: f64, changes: &mut TopologyChanges) {
        let mut edges = BTreeSet::new();
        for room in self.rooms.values() {
            let n = room.nodes.len();
            for i in 0..n {
                edges.insert(edge_key(room.nodes[i], room.nodes[(i + 1) % n]));
            }
        }
        let mut existing = BTreeSet::new();
        let mut remove = Vec::new();
        for w in self.walls.values() {
            let k = edge_key(w.a, w.b);
            if !edges.contains(&k) || !existing.insert(k) {
                remove.push(w.id);
            }
        }
        for id in remove {
            self.walls.remove(&id);
            changes.removed_walls.push(id);
        }
        let dead: Vec<OpeningId> = self
            .openings
            .values()
            .filter(|o| !self.walls.contains_key(&o.wall))
            .map(|o| o.id)
            .collect();
        for id in dead {
            self.openings.remove(&id);
            changes.removed_openings.push(id);
        }
        for k in edges {
            if !existing.contains(&k) {
                let w = Wall::new(k.0, k.1, thickness);
                self.walls.insert(w.id, w);
            }
        }
        self.remove_orphan_nodes();
    }

    fn remove_orphan_nodes(&mut self) {
        let used: BTreeSet<NodeId> = self.rooms.values().flat_map(|r| r.nodes.iter().copied()).collect();
        self.nodes.retain(|id, _| used.contains(id));
    }

    /// Remove a room and the objects in it. Walls only it used go too.
    pub fn remove_room(&mut self, id: RoomId, thickness: f64) -> TopologyChanges {
        let mut changes = TopologyChanges::default();
        if self.rooms.remove(&id).is_none() {
            return changes;
        }
        let objects: Vec<ObjectId> = self.objects.values().filter(|o| o.room == Some(id)).map(|o| o.id).collect();
        for o in objects {
            self.objects.remove(&o);
            changes.removed_objects.push(o);
        }
        self.rebuild_walls(thickness, &mut changes);
        changes
    }

    pub fn move_node(&mut self, id: NodeId, pos: Vec2) {
        if let Some(n) = self.nodes.get_mut(&id) {
            n.pos = pos;
        }
        self.clamp_openings();
    }

    /// Keep doors/windows within their walls after walls change length.
    pub fn clamp_openings(&mut self) {
        let lengths: Vec<(OpeningId, f64)> = self
            .openings
            .values()
            .map(|o| (o.id, self.walls.get(&o.wall).map(|w| self.wall_length(w)).unwrap_or(0.0)))
            .collect();
        for (id, len) in lengths {
            if let Some(o) = self.openings.get_mut(&id) {
                let half = o.width / 2.0;
                let lo = half.min(len / 2.0);
                let hi = (len - half).max(len / 2.0);
                o.offset = o.offset.clamp(lo, hi);
            }
        }
    }

    /// If the room is an axis-aligned rectangle: (top-left, width, depth).
    /// Extra points along the edges (junctions with neighbours) are allowed.
    pub fn rect_dims(&self, room: &Room) -> Option<(Vec2, f64, f64)> {
        if self.room_has_curves(room) {
            return None;
        }
        let pts = self.room_points(room);
        let n = pts.len();
        if n < 4 {
            return None;
        }
        let corners: Vec<Vec2> = (0..n)
            .filter(|&i| {
                let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
                (b - a).normalized().cross((c - b).normalized()).abs() > 1e-6
            })
            .map(|i| pts[i])
            .collect();
        if corners.len() != 4 {
            return None;
        }
        let axis_aligned = (0..4).all(|i| {
            let d = corners[(i + 1) % 4] - corners[i];
            d.x.abs() < 1e-6 || d.y.abs() < 1e-6
        });
        if !axis_aligned {
            return None;
        }
        let (lo, hi) = bounds(&corners)?;
        Some((lo, hi.x - lo.x, hi.y - lo.y))
    }

    /// True if `node` is a corner of a locked room other than `room`.
    pub fn pinned_by_locked(&self, room: RoomId, node: NodeId) -> bool {
        self.rooms.values().any(|r| r.id != room && r.shape_locked && r.nodes.contains(&node))
    }

    /// True if the edge `a`–`b` of `room` is also a wall of a locked room.
    fn edge_pinned(&self, room: RoomId, a: NodeId, b: NodeId) -> bool {
        self.rooms.values().any(|r| r.id != room && r.shape_locked && has_edge(&r.nodes, a, b))
    }

    /// Give `room` its own copy of a corner it shares with other rooms, so
    /// moving it leaves them alone. Walls only `room` uses switch over to the
    /// copy (keeping their doors and windows); shared walls stay with the
    /// other rooms. Call [`Level::weld_room`] afterwards to rejoin.
    pub fn detach_node(&mut self, room: RoomId, node: NodeId) -> Option<NodeId> {
        let pos = self.nodes.get(&node)?.pos;
        let copy = Node { id: NodeId::new(), pos };
        let new = copy.id;
        self.nodes.insert(new, copy);
        let others: Vec<Vec<NodeId>> =
            self.rooms.values().filter(|r| r.id != room).map(|r| r.nodes.clone()).collect();
        for w in self.walls.values_mut() {
            if (w.a == node || w.b == node) && !others.iter().any(|o| has_edge(o, w.a, w.b)) {
                if w.a == node {
                    w.a = new;
                }
                if w.b == node {
                    w.b = new;
                }
            }
        }
        for n in self.rooms.get_mut(&room)?.nodes.iter_mut() {
            if *n == node {
                *n = new;
            }
        }
        Some(new)
    }

    /// Move some of `room`'s corners. Corners shared with a locked room are
    /// detached first, so locked rooms never change shape; corners shared only
    /// with unlocked rooms move for both. `drop` lists in-between corners
    /// (junctions along an edge) that the room should let go of. Afterwards
    /// the room is rejoined to whatever it touches. If any room ends up
    /// crossing itself or overlapping another, nothing changes.
    pub fn reshape_room(
        &mut self,
        room: RoomId,
        targets: &[(NodeId, Vec2)],
        drop: &[NodeId],
        tol: f64,
        thickness: f64,
    ) -> Result<TopologyChanges, TopologyError> {
        if !self.rooms.contains_key(&room) {
            return Err(TopologyError::NotFound);
        }
        let backup = self.clone();
        let result = self.reshape_inner(room, targets, drop, tol, thickness);
        if result.is_err() {
            *self = backup;
        }
        result
    }

    fn reshape_inner(
        &mut self,
        room: RoomId,
        targets: &[(NodeId, Vec2)],
        drop: &[NodeId],
        tol: f64,
        thickness: f64,
    ) -> Result<TopologyChanges, TopologyError> {
        let mut changes = TopologyChanges::default();
        if !drop.is_empty() {
            self.rooms.get_mut(&room).unwrap().nodes.retain(|n| !drop.contains(n));
            self.rebuild_walls(thickness, &mut changes);
        }
        let mut touched = BTreeSet::from([room]);
        for &(node, pos) in targets {
            if !self.nodes.contains_key(&node) {
                continue;
            }
            if self.nodes[&node].pos.distance(pos) < 1e-9 {
                continue;
            }
            let id = if self.pinned_by_locked(room, node) {
                self.detach_node(room, node).ok_or(TopologyError::NotFound)?
            } else {
                touched.extend(self.rooms.values().filter(|r| r.nodes.contains(&node)).map(|r| r.id));
                node
            };
            self.nodes.get_mut(&id).unwrap().pos = pos;
        }
        self.rebuild_walls(thickness, &mut changes);
        self.check_rooms(&touched)?;
        let more = self.weld_room(room, tol, thickness);
        changes.split_walls.extend(more.split_walls);
        changes.removed_walls.extend(more.removed_walls);
        changes.removed_openings.extend(more.removed_openings);
        changes.merged_walls.extend(more.merged_walls);
        self.clamp_openings();
        Ok(changes)
    }

    /// After dragging `node` (a corner of `room`) away from `start`: if a room
    /// is now a bad shape or overlaps another, put the corner back. Then
    /// rejoin the room to whatever it touches.
    pub fn finish_corner_move(
        &mut self,
        room: RoomId,
        node: NodeId,
        start: Vec2,
        tol: f64,
        thickness: f64,
    ) -> Result<TopologyChanges, TopologyError> {
        let touched: BTreeSet<RoomId> = self.rooms.values().filter(|r| r.nodes.contains(&node)).map(|r| r.id).collect();
        let mut bad = self.check_rooms(&touched).err();
        let mut fitted = TopologyChanges::default();
        if bad == Some(TopologyError::Overlaps) {
            // Pushed into a neighbour: take its shape there instead.
            let backup = self.clone();
            // The room it was within before the move, if any, keeps it.
            let host = self.nodes.get(&node).map(|n| n.pos).and_then(|now| {
                self.nodes.get_mut(&node).unwrap().pos = start;
                let host = self.room_around(room);
                self.nodes.get_mut(&node).unwrap().pos = now;
                host
            });
            let attempt = self
                .rooms
                .get(&room)
                .map(|r| self.room_shape(r))
                .ok_or(TopologyError::NotFound)
                .and_then(|shape| self.make_room_for(&shape, Some(room), host, tol))
                .and_then(|(shape, carve)| {
                    let mut c = self.place_room(room, &shape, tol, thickness)?;
                    if let Some((h, rest)) = carve {
                        extend_changes(&mut c, self.place_room(h, &rest, tol, thickness)?);
                    }
                    Ok(c)
                });
            let now: BTreeSet<RoomId> = touched.iter().copied().chain(host).filter(|r| self.rooms.contains_key(r)).collect();
            match attempt {
                Ok(c) if self.check_rooms(&now).is_ok() => {
                    fitted = c;
                    bad = None;
                }
                _ => *self = backup,
            }
        }
        if bad.is_some() {
            if let Some(n) = self.nodes.get_mut(&node) {
                n.pos = start;
            }
        }
        let mut changes = self.weld_room(room, tol, thickness);
        changes.split_walls.extend(fitted.split_walls);
        changes.removed_walls.extend(fitted.removed_walls);
        changes.removed_openings.extend(fitted.removed_openings);
        changes.merged_walls.extend(fitted.merged_walls);
        self.clamp_openings();
        match bad {
            Some(e) => Err(e),
            None => Ok(changes),
        }
    }

    /// Tidy a room drawn slightly off: corners within `within` of another
    /// room's corner (across or down) line up with it exactly, and walls that
    /// are nearly horizontal or vertical become exactly so. Locked rooms
    /// never change.
    pub fn straighten_room(&mut self, room: RoomId, within: f64, tol: f64, thickness: f64) -> Result<TopologyChanges, TopologyError> {
        let r = self.rooms.get(&room).ok_or(TopologyError::NotFound)?;
        let ids = r.nodes.clone();
        let n = ids.len();
        let pts: Vec<Vec2> = ids.iter().map(|id| self.nodes[id].pos).collect();
        let others: Vec<Vec2> = self.nodes.values().filter(|nd| !ids.contains(&nd.id)).map(|nd| nd.pos).collect();
        let mut out = pts.clone();
        for axis in [0usize, 1] {
            let get = |p: Vec2| if axis == 0 { p.x } else { p.y };
            // Group corners joined by a nearly-straight wall along this axis
            // (a nearly vertical wall for x, nearly horizontal for y).
            let mut group: Vec<usize> = (0..n).collect();
            fn find(g: &mut [usize], i: usize) -> usize {
                let mut i = i;
                while g[i] != i {
                    g[i] = g[g[i]];
                    i = g[i];
                }
                i
            }
            for i in 0..n {
                let j = (i + 1) % n;
                if (get(pts[i]) - get(pts[j])).abs() < within {
                    let (a, b) = (find(&mut group, i), find(&mut group, j));
                    group[a] = b;
                }
            }
            let mut roots: Vec<usize> = (0..n).map(|i| find(&mut group, i)).collect();
            roots.sort();
            roots.dedup();
            for root in roots {
                let members: Vec<usize> = (0..n).filter(|&i| find(&mut group, i) == root).collect();
                let mean = members.iter().map(|&i| get(pts[i])).sum::<f64>() / members.len() as f64;
                // Prefer lining up with a neighbour's corner.
                let target = others
                    .iter()
                    .map(|p| get(*p))
                    .filter(|v| members.iter().any(|&i| (get(pts[i]) - v).abs() < within))
                    .min_by(|a, b| (a - mean).abs().total_cmp(&(b - mean).abs()))
                    .unwrap_or(mean);
                for i in members {
                    if axis == 0 {
                        out[i].x = target;
                    } else {
                        out[i].y = target;
                    }
                }
            }
        }
        let targets: Vec<(NodeId, Vec2)> = ids.iter().copied().zip(out).collect();
        self.reshape_room(room, &targets, &[], tol, thickness)
    }

    /// Every room in `rooms` must be a proper shape and not overlap another.
    fn check_rooms(&self, rooms: &BTreeSet<RoomId>) -> Result<(), TopologyError> {
        for id in rooms {
            let Some(r) = self.rooms.get(id) else { continue };
            let pts = self.room_outline(r);
            if pts.len() < 3 || area(&pts) < 0.05 {
                return Err(TopologyError::Degenerate);
            }
            if !is_simple(&pts) {
                return Err(TopologyError::SelfIntersecting);
            }
            if self.room_overlaps_others(*id) {
                return Err(TopologyError::Overlaps);
            }
        }
        Ok(())
    }

    /// Resize a rectangular room. Each side that's against a locked room stays
    /// put and the opposite side moves; otherwise the right and bottom sides
    /// move. Locked neighbours never change; unlocked ones sharing the moving
    /// side follow it.
    pub fn set_rect_size(
        &mut self,
        room_id: RoomId,
        width: f64,
        depth: f64,
        tol: f64,
        thickness: f64,
    ) -> Result<TopologyChanges, TopologyError> {
        let room = self.rooms.get(&room_id).ok_or(TopologyError::NotFound)?;
        let (lo, ow, od) = self.rect_dims(room).ok_or(TopologyError::NotRectangle)?;
        if width < 0.1 || depth < 0.1 {
            return Err(TopologyError::Degenerate);
        }
        let eps = 1e-6;
        let nodes = room.nodes.clone();
        let n = nodes.len();
        let pos = |id: &NodeId| self.nodes[id].pos;
        // Is the side at x == v (or y == v) against a locked room?
        let side_pinned = |vertical: bool, v: f64| {
            (0..n).any(|i| {
                let (a, b) = (nodes[i], nodes[(i + 1) % n]);
                let (pa, pb) = (pos(&a), pos(&b));
                let on = if vertical {
                    (pa.x - v).abs() < eps && (pb.x - v).abs() < eps
                } else {
                    (pa.y - v).abs() < eps && (pb.y - v).abs() < eps
                };
                on && self.edge_pinned(room_id, a, b)
            })
        };
        // (old coordinate of the moving side, its new coordinate, new min, new max)
        let axis = |vertical: bool, start: f64, old: f64, new: f64| -> Result<Option<(f64, f64, f64, f64)>, TopologyError> {
            if (new - old).abs() < eps {
                return Ok(None);
            }
            let (low, high) = (side_pinned(vertical, start), side_pinned(vertical, start + old));
            match (low, high) {
                (true, true) => Err(TopologyError::BetweenLocked),
                (false, true) => Ok(Some((start, start + old - new, start + old - new, start + old))),
                _ => Ok(Some((start + old, start + new, start, start + new))),
            }
        };
        let ax = axis(true, lo.x, ow, width)?;
        let ay = axis(false, lo.y, od, depth)?;
        let mut targets = Vec::new();
        let mut drop = Vec::new();
        for id in &nodes {
            let p = pos(id);
            let mut q = p;
            let mut moving = false;
            if let Some((from, to, _, _)) = ax {
                if (p.x - from).abs() < eps {
                    q.x = to;
                    moving = true;
                }
            }
            if let Some((from, to, _, _)) = ay {
                if (p.y - from).abs() < eps {
                    q.y = to;
                    moving = true;
                }
            }
            let outside = ax.is_some_and(|(_, _, a, b)| q.x < a - eps || q.x > b + eps)
                || ay.is_some_and(|(_, _, a, b)| q.y < a - eps || q.y > b + eps);
            if !moving && outside {
                // A junction the shrinking room no longer reaches.
                drop.push(*id);
            } else if moving {
                targets.push((*id, q));
            }
        }
        self.reshape_room(room_id, &targets, &drop, tol, thickness)
    }

    /// Change one of a room's walls to `length` by sliding one of its corners
    /// along the wall's direction; the other corner stays put. We'd rather
    /// move a corner that isn't held by a locked room, then the one with fewer
    /// walls (a busy junction disturbs more neighbours), then the next corner
    /// going round the room.
    pub fn set_wall_length(
        &mut self,
        room_id: RoomId,
        wall_id: WallId,
        length: f64,
        tol: f64,
        thickness: f64,
    ) -> Result<TopologyChanges, TopologyError> {
        let room = self.rooms.get(&room_id).ok_or(TopologyError::NotFound)?;
        let wall = self.walls.get(&wall_id).ok_or(TopologyError::NotFound)?.clone();
        let (pa, pb) = self.wall_points(&wall).ok_or(TopologyError::NotFound)?;
        let old = pa.distance(pb);
        if length < 0.1 || old < 1e-9 {
            return Err(TopologyError::Degenerate);
        }
        // A curve keeps its shape, so its length scales with the straight
        // distance between its ends: work in that distance.
        let around = self.wall_length(&wall) / old;
        let length = length / around;
        let n = room.nodes.len();
        let ia = room.nodes.iter().position(|&x| x == wall.a).ok_or(TopologyError::NotFound)?;
        let (start, end) = if room.nodes[(ia + 1) % n] == wall.b { (wall.a, wall.b) } else { (wall.b, wall.a) };
        let cost = |node: NodeId| {
            let degree = self.walls.values().filter(|w| w.a == node || w.b == node).count();
            (self.pinned_by_locked(room_id, node), degree)
        };
        let (fixed, moving) = if cost(end) > cost(start) { (end, start) } else { (start, end) };
        let from = self.nodes[&fixed].pos;
        let dir = (self.nodes[&moving].pos - from) * (1.0 / old);
        // Openings measure from node `a`; if `a` moves, keep them where they are.
        // (Unless the wall stays behind with a locked neighbour.)
        let wall_shared = self.rooms.values().any(|r| r.id != room_id && has_edge(&r.nodes, wall.a, wall.b));
        let stays = self.pinned_by_locked(room_id, moving) && wall_shared;
        let shift = if moving == wall.a && !stays { (length - old) * around } else { 0.0 };
        let backup = self.clone();
        for o in self.openings.values_mut().filter(|o| o.wall == wall_id) {
            o.offset += shift;
        }
        let result = self.reshape_room(room_id, &[(moving, from + dir * length)], &[], tol, thickness);
        if result.is_err() {
            *self = backup;
        }
        result
    }

    /// True if no corner of the room is shared with another room.
    pub fn is_freestanding(&self, room_id: RoomId) -> bool {
        let Some(room) = self.rooms.get(&room_id) else { return false };
        !self.rooms.values().any(|r| r.id != room_id && r.nodes.iter().any(|n| room.nodes.contains(n)))
    }

    /// Slide a freestanding room (and the items and rooms in it) by `delta`. Doors and
    /// windows ride along because they're measured along their walls. Call
    /// [`Level::settle_room`] once the move is finished.
    pub fn translate_room(&mut self, room_id: RoomId, delta: Vec2) -> Result<(), TopologyError> {
        if !self.is_freestanding(room_id) {
            return Err(TopologyError::Attached);
        }
        // Rooms within it come along too.
        let rooms: BTreeSet<RoomId> = std::iter::once(room_id).chain(self.rooms_inside(room_id)).collect();
        let nodes: BTreeSet<NodeId> = rooms.iter().flat_map(|r| self.rooms[r].nodes.iter().copied()).collect();
        for n in nodes {
            if let Some(node) = self.nodes.get_mut(&n) {
                node.pos += delta;
            }
        }
        for o in self.objects.values_mut().filter(|o| o.room.is_some_and(|r| rooms.contains(&r)) && o.roof.is_none()) {
            o.position += delta;
        }
        Ok(())
    }

    /// True if the room's floor overlaps another room's (touching is fine, and
    /// so is one being a room within the other).
    pub fn room_overlaps_others(&self, room_id: RoomId) -> bool {
        let Some(room) = self.rooms.get(&room_id) else { return false };
        let mine = self.room_outline(room);
        self.rooms
            .values()
            .filter(|r| r.id != room_id)
            .filter(|r| overlap_area(&mine, &self.room_outline(r)) > 0.01)
            .any(|r| !self.room_inside(room_id, r.id) && !self.room_inside(r.id, room_id))
    }

    /// After a room has moved: if it now overlaps another room, move it back by
    /// `undo`. Otherwise join it to any rooms it touches.
    pub fn settle_room(
        &mut self,
        room_id: RoomId,
        undo: Vec2,
        tol: f64,
        thickness: f64,
    ) -> Result<TopologyChanges, TopologyError> {
        if !self.rooms.contains_key(&room_id) {
            return Err(TopologyError::NotFound);
        }
        if self.room_overlaps_others(room_id) {
            self.translate_room(room_id, undo)?;
            return Err(TopologyError::Overlaps);
        }
        let changes = self.weld_room(room_id, tol, thickness);
        self.clamp_openings();
        Ok(changes)
    }

    /// Join `room` to the rooms it touches, so touching walls become shared:
    /// corners within `tol` of another corner merge, corners landing partway
    /// along a wall split it, and two walls along the same edge become one
    /// (keeping the one that was there first, plus the doors and windows of
    /// both).
    pub fn weld_room(&mut self, room_id: RoomId, tol: f64, thickness: f64) -> TopologyChanges {
        let mut changes = TopologyChanges::default();
        let Some(room) = self.rooms.get(&room_id) else { return changes };

        // A room within a room stays clear of the room it's in (and of the
        // rooms within it).
        let apart: BTreeSet<NodeId> = self
            .room_around(room_id)
            .into_iter()
            .chain(self.rooms_inside(room_id))
            .flat_map(|r| self.rooms[&r].nodes.clone())
            .collect();

        // 1. Merge our corners into nearby corners of other rooms.
        let mine = room.nodes.clone();
        for n in &mine {
            let Some(p) = self.nodes.get(n).map(|x| x.pos) else { continue };
            let other = self
                .nodes
                .values()
                .filter(|m| !mine.contains(&m.id) && !apart.contains(&m.id) && m.pos.distance(p) <= tol)
                .min_by(|a, b| a.pos.distance(p).total_cmp(&b.pos.distance(p)))
                .map(|m| m.id);
            let Some(m) = other else { continue };
            // Never fold a room onto itself.
            if self.rooms.values().any(|r| r.nodes.contains(n) && r.nodes.contains(&m)) {
                continue;
            }
            for r in self.rooms.values_mut() {
                for id in r.nodes.iter_mut() {
                    if *id == *n {
                        *id = m;
                    }
                }
            }
            for w in self.walls.values_mut() {
                if w.a == *n {
                    w.a = m;
                }
                if w.b == *n {
                    w.b = m;
                }
            }
            self.nodes.remove(n);
        }

        // 2. Corners that land partway along a wall split it.
        let mine: BTreeSet<NodeId> = self.rooms[&room_id].nodes.iter().copied().collect();
        loop {
            let mut split = None;
            'find: for w in self.walls.values() {
                let apart_wall = apart.contains(&w.a) && apart.contains(&w.b);
                let my_wall = mine.contains(&w.a) && mine.contains(&w.b);
                for nd in self.nodes.values() {
                    if nd.id == w.a || nd.id == w.b {
                        continue;
                    }
                    if (apart_wall && mine.contains(&nd.id)) || (my_wall && apart.contains(&nd.id)) {
                        continue;
                    }
                    if self.point_on_wall(w, nd.pos, tol).is_some() {
                        split = Some((w.id, nd.id));
                        break 'find;
                    }
                }
            }
            let Some((w, n)) = split else { break };
            self.split_wall_at(w, n, &mut changes);
        }

        // 3. Two walls along one edge become one shared wall.
        let mut by_edge: std::collections::BTreeMap<(NodeId, NodeId), Vec<WallId>> = Default::default();
        for w in self.walls.values() {
            by_edge.entry(edge_key(w.a, w.b)).or_default().push(w.id);
        }
        let ours: BTreeSet<WallId> = {
            let room = &self.rooms[&room_id];
            let others: Vec<&Room> = self.rooms.values().filter(|r| r.id != room_id).collect();
            self.walls
                .values()
                .filter(|w| has_edge(&room.nodes, w.a, w.b) && !others.iter().any(|o| has_edge(&o.nodes, w.a, w.b)))
                .map(|w| w.id)
                .collect()
        };
        for ids in by_edge.values().filter(|ids| ids.len() > 1) {
            let keep = ids.iter().copied().find(|id| !ours.contains(id)).unwrap_or(ids[0]);
            for &gone in ids.iter().filter(|&&id| id != keep) {
                let (kw, gw) = (self.walls[&keep].clone(), self.walls[&gone].clone());
                let len = self.wall_length(&kw);
                for o in self.openings.values_mut().filter(|o| o.wall == gone) {
                    o.wall = keep;
                    if gw.a != kw.a {
                        o.offset = len - o.offset;
                    }
                }
                self.walls.remove(&gone);
                changes.merged_walls.push((gone, keep));
            }
        }
        self.rebuild_walls(thickness, &mut changes);
        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level() -> Level {
        Level::new("Main", 0.0)
    }

    fn add(l: &mut Level, x: f64, y: f64, w: f64, h: f64) -> RoomId {
        l.add_room("r", &rect_points(Vec2::new(x, y), w, h), SNAP_TOLERANCE, 0.15)
            .unwrap()
            .0
    }

    #[test]
    fn single_room_has_four_walls() {
        let mut l = level();
        add(&mut l, 0.0, 0.0, 4.0, 3.0);
        assert_eq!(l.walls.len(), 4);
        assert_eq!(l.nodes.len(), 4);
    }

    #[test]
    fn adjacent_rooms_share_a_wall() {
        let mut l = level();
        let a = add(&mut l, 0.0, 0.0, 4.0, 3.0);
        let b = add(&mut l, 4.0, 0.0, 3.0, 3.0);
        assert_eq!(l.walls.len(), 7);
        assert_eq!(l.nodes.len(), 6);
        let shared: Vec<_> = l.walls.values().filter(|w| l.rooms_of_wall(w).len() == 2).collect();
        assert_eq!(shared.len(), 1);
        let rooms = l.rooms_of_wall(shared[0]);
        assert!(rooms.contains(&a) && rooms.contains(&b));
    }

    #[test]
    fn partial_overlap_splits_walls() {
        // A is 4x3; B is a 2x2 room against A's right wall, so A's right wall
        // must split into a shared part (0..2) and an exterior part (2..3).
        let mut l = level();
        let a = add(&mut l, 0.0, 0.0, 4.0, 3.0);
        let b = add(&mut l, 4.0, 0.0, 2.0, 2.0);
        assert_eq!(l.rooms[&a].nodes.len(), 5);
        assert_eq!(l.walls.len(), 4 + 1 + 3); // A's 5 edges + B's 3 non-shared edges
        let shared: Vec<_> = l.walls.values().filter(|w| l.rooms_of_wall(w).len() == 2).collect();
        assert_eq!(shared.len(), 1);
        assert!((l.wall_length(shared[0]) - 2.0).abs() < 1e-9);
        assert!(l.rooms_of_wall(shared[0]).contains(&b));
    }

    #[test]
    fn existing_corner_on_new_edge_is_shared() {
        // Two small rooms side by side, then a long hallway under both.
        let mut l = level();
        add(&mut l, 0.0, 0.0, 2.0, 2.0);
        add(&mut l, 2.0, 0.0, 2.0, 2.0);
        let hall = add(&mut l, 0.0, 2.0, 4.0, 1.0);
        assert_eq!(l.rooms[&hall].nodes.len(), 5);
        let shared = l.walls.values().filter(|w| l.rooms_of_wall(w).len() == 2).count();
        assert_eq!(shared, 3);
    }

    #[test]
    fn opening_moves_to_second_half_on_split() {
        let mut l = level();
        add(&mut l, 0.0, 0.0, 4.0, 4.0);
        let (wid, offset, _) = l.nearest_wall(Vec2::new(4.0, 3.0), 0.1).unwrap();
        let o = Opening::new(wid, OpeningKind::Window(WindowStyle::Fixed), offset);
        let oid = o.id;
        l.openings.insert(oid, o);
        add(&mut l, 4.0, 0.0, 2.0, 2.0);
        let o = &l.openings[&oid];
        let (s, e, _) = l.opening_segment(o).unwrap();
        let mid = s.lerp(e, 0.5);
        assert!(mid.distance(Vec2::new(4.0, 3.0)) < 1e-9, "{mid:?}");
    }

    #[test]
    fn remove_room_keeps_neighbour_intact() {
        let mut l = level();
        let a = add(&mut l, 0.0, 0.0, 4.0, 3.0);
        let b = add(&mut l, 4.0, 0.0, 3.0, 3.0);
        l.remove_room(b, 0.15);
        assert_eq!(l.walls.len(), 4);
        assert_eq!(l.rooms.len(), 1);
        assert!(l.rooms.contains_key(&a));
    }

    #[test]
    fn resize_rectangle() {
        let mut l = level();
        let a = add(&mut l, 0.0, 0.0, 4.0, 3.0);
        l.set_rect_size(a, 5.0, 3.5, SNAP_TOLERANCE, 0.15).unwrap();
        let (_, w, d) = l.rect_dims(&l.rooms[&a]).unwrap();
        assert!((w - 5.0).abs() < 1e-9 && (d - 3.5).abs() < 1e-9);
    }

    #[test]
    fn set_one_wall_length() {
        let mut l = level();
        let a = add(&mut l, 0.0, 0.0, 4.0, 3.0);
        let walls = l.walls_of_room(&l.rooms[&a]);
        let w = walls[0];
        let other: Vec<f64> = walls[1..].iter().map(|w| l.wall_length(&l.walls[w])).collect();
        l.set_wall_length(a, w, 5.0, SNAP_TOLERANCE, 0.15).unwrap();
        assert!((l.wall_length(&l.walls[&w]) - 5.0).abs() < 1e-9);
        // The wall before it is untouched; only the one after the moved corner changes.
        let changed = walls[1..]
            .iter()
            .zip(&other)
            .filter(|(w, len)| (l.wall_length(&l.walls[w]) - **len).abs() > 1e-9)
            .count();
        assert_eq!(changed, 1);
        // Too short, or folding the room over itself, is refused and undone.
        assert!(l.set_wall_length(a, w, 0.01, SNAP_TOLERANCE, 0.15).is_err());
        assert!((l.wall_length(&l.walls[&w]) - 5.0).abs() < 1e-9);
    }

    fn rect_of(l: &Level, r: RoomId) -> (f64, f64, f64, f64) {
        let (lo, w, d) = l.rect_dims(&l.rooms[&r]).unwrap();
        (lo.x, lo.y, w, d)
    }

    fn close4(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9 && (a.2 - b.2).abs() < 1e-9 && (a.3 - b.3).abs() < 1e-9
    }

    /// An unlocked room sitting on top of a locked one.
    fn dining_over_living() -> (Level, RoomId, RoomId) {
        let mut l = level();
        let dining = add(&mut l, 0.0, 0.0, 3.2, 4.35);
        let living = add(&mut l, 0.0, 4.35, 3.2, 3.35);
        l.rooms.get_mut(&living).unwrap().shape_locked = true;
        (l, dining, living)
    }

    #[test]
    fn locked_neighbour_keeps_its_shape_when_narrowing() {
        let (mut l, dining, living) = dining_over_living();
        let before = rect_of(&l, living);
        l.set_rect_size(dining, 2.5, 4.35, SNAP_TOLERANCE, 0.15).unwrap();
        assert!(close4(rect_of(&l, living), before));
        assert!(close4(rect_of(&l, dining), (0.0, 0.0, 2.5, 4.35)));
        // Still sharing the part of the wall they have in common.
        assert!(!l.is_freestanding(dining));
        assert_eq!(l.walls.len(), 4 + 4 + 1 - 1, "living's top wall split; one piece shared");
    }

    #[test]
    fn locked_neighbour_keeps_its_shape_when_widening() {
        let (mut l, dining, living) = dining_over_living();
        let before = rect_of(&l, living);
        l.set_rect_size(dining, 4.0, 4.35, SNAP_TOLERANCE, 0.15).unwrap();
        assert!(close4(rect_of(&l, living), before));
        assert!(close4(rect_of(&l, dining), (0.0, 0.0, 4.0, 4.35)));
        assert!(!l.is_freestanding(dining));
    }

    #[test]
    fn depth_grows_away_from_a_locked_neighbour() {
        let (mut l, dining, living) = dining_over_living();
        let before = rect_of(&l, living);
        l.set_rect_size(dining, 3.2, 3.0, SNAP_TOLERANCE, 0.15).unwrap();
        assert!(close4(rect_of(&l, living), before));
        // The bottom (against the living room) stayed; the top moved down.
        assert!(close4(rect_of(&l, dining), (0.0, 1.35, 3.2, 3.0)));
        assert_eq!(l.walls.len(), 7, "the whole bottom wall is still shared");
    }

    #[test]
    fn wall_length_leaves_locked_neighbour_alone() {
        let (mut l, dining, living) = dining_over_living();
        let before = rect_of(&l, living);
        let walls = l.walls_of_room(&l.rooms[&dining]);
        for w in walls {
            let len = l.wall_length(&l.walls[&w]);
            l.set_wall_length(dining, w, len - 0.3, SNAP_TOLERANCE, 0.15).ok();
            assert!(close4(rect_of(&l, living), before));
        }
    }

    #[test]
    fn between_two_locked_rooms() {
        let mut l = level();
        let left = add(&mut l, 0.0, 0.0, 3.0, 3.0);
        let mid = add(&mut l, 3.0, 0.0, 3.0, 3.0);
        let right = add(&mut l, 6.0, 0.0, 3.0, 3.0);
        for r in [left, right] {
            l.rooms.get_mut(&r).unwrap().shape_locked = true;
        }
        assert_eq!(l.set_rect_size(mid, 2.0, 3.0, SNAP_TOLERANCE, 0.15).unwrap_err(), TopologyError::BetweenLocked);
        // Depth is fine: top and bottom are free.
        l.set_rect_size(mid, 3.0, 2.0, SNAP_TOLERANCE, 0.15).unwrap();
        assert!(close4(rect_of(&l, left), (0.0, 0.0, 3.0, 3.0)));
        assert!(close4(rect_of(&l, right), (6.0, 0.0, 3.0, 3.0)));
    }

    #[test]
    fn unlocked_neighbour_still_follows() {
        let mut l = level();
        let a = add(&mut l, 0.0, 0.0, 3.0, 3.0);
        let b = add(&mut l, 3.0, 0.0, 3.0, 3.0);
        l.set_rect_size(a, 4.0, 3.0, SNAP_TOLERANCE, 0.15).unwrap();
        assert!(close4(rect_of(&l, b), (4.0, 0.0, 2.0, 3.0)));
    }

    #[test]
    fn straighten_lines_up_with_neighbour() {
        let mut l = level();
        let a = add(&mut l, 0.0, 0.0, 3.0, 3.0);
        l.rooms.get_mut(&a).unwrap().shape_locked = true;
        // Drawn a little off: right side 1 cm short of A's, bottom-left a bit crooked.
        let pts = [Vec2::new(0.012, 3.0), Vec2::new(2.99, 3.0), Vec2::new(2.99, 5.0), Vec2::new(0.0, 5.015)];
        let (b, _) = l.add_room("b", &pts, SNAP_TOLERANCE, 0.15).unwrap();
        l.straighten_room(b, 0.05, SNAP_TOLERANCE, 0.15).unwrap();
        // Bottom corners had nothing to line up with, so they meet halfway.
        assert!(close4(rect_of(&l, b), (0.0, 3.0, 3.0, 2.0075)), "{:?}", rect_of(&l, b));
        assert!(close4(rect_of(&l, a), (0.0, 0.0, 3.0, 3.0)));
        assert_eq!(l.walls.len(), 7, "the whole top wall is shared");
    }

    #[test]
    fn move_room_and_join_neighbour() {
        let mut l = level();
        let a = add(&mut l, 0.0, 0.0, 4.0, 3.0);
        let b = add(&mut l, 10.0, 0.0, 3.0, 3.0);
        assert_eq!(l.walls.len(), 8);
        // Slide B so its left side lies along A's right side.
        let d = Vec2::new(-6.0, 0.0);
        l.translate_room(b, d).unwrap();
        l.settle_room(b, d * -1.0, SNAP_TOLERANCE, 0.15).unwrap();
        assert_eq!(l.walls.len(), 7, "the touching side is one shared wall");
        assert!(!l.is_freestanding(a) && !l.is_freestanding(b));
        assert_eq!(l.translate_room(b, Vec2::new(1.0, 0.0)), Err(TopologyError::Attached));
    }

    #[test]
    fn join_partway_along_a_wall_keeps_doors() {
        let mut l = level();
        add(&mut l, 0.0, 0.0, 4.0, 4.0);
        let b = add(&mut l, 10.0, 0.0, 3.0, 2.0);
        // A door on B's left wall (x = 10).
        let left = l
            .walls
            .values()
            .find(|w| {
                let (p, q) = l.wall_points(w).unwrap();
                (p.x - 10.0).abs() < 1e-9 && (q.x - 10.0).abs() < 1e-9
            })
            .unwrap()
            .id;
        let door = Opening::new(left, OpeningKind::Door(DoorStyle::Hinged), 1.0);
        let did = door.id;
        l.openings.insert(did, door);
        let d = Vec2::new(-6.0, 1.0);
        l.translate_room(b, d).unwrap();
        l.settle_room(b, d * -1.0, SNAP_TOLERANCE, 0.15).unwrap();
        // A's right wall split at y=1 and y=3; the middle piece is shared.
        assert_eq!(l.walls.len(), 4 + 2 + 3);
        let o = &l.openings[&did];
        let (s, e, _) = l.opening_segment(o).unwrap();
        let mid = (s + e) * 0.5;
        assert!((mid.x - 4.0).abs() < 1e-6 && (mid.y - 2.0).abs() < 1e-6, "door stayed put: {mid:?}");
    }

    #[test]
    fn overlapping_move_is_undone() {
        let mut l = level();
        add(&mut l, 0.0, 0.0, 4.0, 4.0);
        let b = add(&mut l, 10.0, 0.0, 3.0, 3.0);
        let d = Vec2::new(-8.0, 0.0);
        l.translate_room(b, d).unwrap();
        assert_eq!(l.settle_room(b, d * -1.0, SNAP_TOLERANCE, 0.15).unwrap_err(), TopologyError::Overlaps);
        let min_x = l.room_points(&l.rooms[&b]).iter().map(|p| p.x).fold(f64::MAX, f64::min);
        assert!((min_x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_bad_shapes() {
        let mut l = level();
        let bowtie = [
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 2.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(0.0, 2.0),
        ];
        assert_eq!(
            l.add_room("x", &bowtie, SNAP_TOLERANCE, 0.15).unwrap_err(),
            TopologyError::SelfIntersecting
        );
        assert!(l.nodes.is_empty());
    }
}

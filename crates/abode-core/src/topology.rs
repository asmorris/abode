//! Floor-plan topology: rooms are loops of shared nodes, walls are the
//! unique edges between them. Adding a room that touches an existing one
//! splits walls as needed so the touching part becomes a single shared wall.

use crate::geometry::*;
use crate::ids::*;
use crate::model::*;
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

    pub fn wall_length(&self, wall: &Wall) -> f64 {
        self.wall_points(wall).map(|(a, b)| a.distance(b)).unwrap_or(0.0)
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
                let pts = self.room_points(r);
                point_in_polygon(p, &pts).then(|| (area(&pts), r.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
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
                let (c, t) = closest_point_on_segment(p, a, b);
                let d = c.distance(p);
                (d <= max_dist).then(|| (w.id, t * a.distance(b), d))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
    }

    /// Endpoints of an opening along its wall, plus the wall direction.
    pub fn opening_segment(&self, o: &Opening) -> Option<(Vec2, Vec2, Vec2)> {
        let wall = self.walls.get(&o.wall)?;
        let (a, b) = self.wall_points(wall)?;
        let dir = (b - a).normalized();
        let c = a + dir * o.offset;
        Some((c - dir * (o.width / 2.0), c + dir * (o.width / 2.0), dir))
    }

    /// Unit normal of `wall` pointing into `room`.
    pub fn wall_normal_into(&self, wall: &Wall, room: RoomId) -> Option<Vec2> {
        let (a, b) = self.wall_points(wall)?;
        let pts = self.room_points(self.rooms.get(&room)?);
        let n = (b - a).perp().normalized();
        let mid = a.lerp(b, 0.5);
        let probe = (a.distance(b) * 0.01).clamp(0.005, 0.05);
        Some(if point_in_polygon(mid + n * probe, &pts) { n } else { -n })
    }

    pub fn bounds(&self) -> Option<(Vec2, Vec2)> {
        bounds(self.nodes.values().map(|n| &n.pos))
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
        let mut pts: Vec<Vec2> = Vec::new();
        for &p in points {
            if pts.last().is_none_or(|l: &Vec2| l.distance(p) > tol) {
                pts.push(p);
            }
        }
        while pts.len() > 1 && pts[0].distance(*pts.last().unwrap()) <= tol {
            pts.pop();
        }
        if pts.len() < 3 {
            return Err(TopologyError::TooFewPoints);
        }
        if !is_simple(&pts) {
            return Err(TopologyError::SelfIntersecting);
        }
        if area(&pts) < 0.05 {
            return Err(TopologyError::Degenerate);
        }
        if signed_area(&pts) < 0.0 {
            pts.reverse();
        }

        let mut changes = TopologyChanges::default();
        let mut ids: Vec<NodeId> = Vec::with_capacity(pts.len());
        for p in &pts {
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
                .filter(|w| {
                    let (a, b) = (self.nodes[&w.a].pos, self.nodes[&w.b].pos);
                    let (c, t) = closest_point_on_segment(p, a, b);
                    t > 1e-6 && t < 1.0 - 1e-6 && c.distance(p) <= tol
                })
                .map(|w| w.id)
                .collect();
            for w in hits {
                self.split_wall_at(w, nid, &mut changes);
            }
        }

        // Pick up existing corners that lie along our new edges.
        let n = ids.len();
        let mut outline = Vec::with_capacity(n);
        for i in 0..n {
            let (u, v) = (ids[i], ids[(i + 1) % n]);
            outline.push(u);
            let (pu, pv) = (self.nodes[&u].pos, self.nodes[&v].pos);
            let mut on_edge: Vec<(f64, NodeId)> = self
                .nodes
                .values()
                .filter(|nd| !ids.contains(&nd.id))
                .filter_map(|nd| {
                    let (c, t) = closest_point_on_segment(nd.pos, pu, pv);
                    (t > 1e-6 && t < 1.0 - 1e-6 && c.distance(nd.pos) <= tol).then_some((t, nd.id))
                })
                .collect();
            on_edge.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, id) in on_edge {
                if !outline.contains(&id) {
                    outline.push(id);
                }
            }
        }

        let room = Room::new(name, outline);
        let id = room.id;
        self.rooms.insert(id, room);
        self.rebuild_walls(thickness, &mut changes);
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
        let first_len = self.nodes[&a].pos.distance(self.nodes[&node].pos);
        let second_id = WallId::new();
        let mut first = wall.clone();
        first.b = node;
        let mut second = wall;
        second.id = second_id;
        second.a = node;
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
        let bad = self.check_rooms(&touched).err();
        if bad.is_some() {
            if let Some(n) = self.nodes.get_mut(&node) {
                n.pos = start;
            }
        }
        let changes = self.weld_room(room, tol, thickness);
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
            let pts = self.room_points(r);
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
        let shift = if moving == wall.a && !stays { length - old } else { 0.0 };
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

    /// Slide a freestanding room (and the items in it) by `delta`. Doors and
    /// windows ride along because they're measured along their walls. Call
    /// [`Level::settle_room`] once the move is finished.
    pub fn translate_room(&mut self, room_id: RoomId, delta: Vec2) -> Result<(), TopologyError> {
        if !self.is_freestanding(room_id) {
            return Err(TopologyError::Attached);
        }
        let nodes = self.rooms[&room_id].nodes.clone();
        for n in nodes {
            if let Some(node) = self.nodes.get_mut(&n) {
                node.pos += delta;
            }
        }
        for o in self.objects.values_mut().filter(|o| o.room == Some(room_id) && o.roof.is_none()) {
            o.position += delta;
        }
        Ok(())
    }

    /// True if the room's floor overlaps another room's (touching is fine).
    pub fn room_overlaps_others(&self, room_id: RoomId) -> bool {
        let Some(room) = self.rooms.get(&room_id) else { return false };
        let mine = self.room_points(room);
        self.rooms
            .values()
            .filter(|r| r.id != room_id)
            .any(|r| overlap_area(&mine, &self.room_points(r)) > 0.01)
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

        // 1. Merge our corners into nearby corners of other rooms.
        let mine = room.nodes.clone();
        for n in &mine {
            let Some(p) = self.nodes.get(n).map(|x| x.pos) else { continue };
            let other = self
                .nodes
                .values()
                .filter(|m| !mine.contains(&m.id) && m.pos.distance(p) <= tol)
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
        loop {
            let mut split = None;
            'find: for w in self.walls.values() {
                let (a, b) = (self.nodes[&w.a].pos, self.nodes[&w.b].pos);
                for nd in self.nodes.values() {
                    if nd.id == w.a || nd.id == w.b {
                        continue;
                    }
                    let (c, t) = closest_point_on_segment(nd.pos, a, b);
                    if t > 1e-6 && t < 1.0 - 1e-6 && c.distance(nd.pos) <= tol {
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

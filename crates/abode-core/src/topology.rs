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
}

/// Side effects of a topology edit, so the project can fix up references.
#[derive(Debug, Default, Clone)]
pub struct TopologyChanges {
    /// (original wall, new second half)
    pub split_walls: Vec<(WallId, WallId)>,
    pub removed_walls: Vec<WallId>,
    pub removed_openings: Vec<OpeningId>,
    pub removed_objects: Vec<ObjectId>,
}

fn edge_key(a: NodeId, b: NodeId) -> (NodeId, NodeId) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
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

    /// Resize a rectangular room by moving its right and bottom edges.
    /// Shared corners move too, so neighbouring rooms follow.
    pub fn set_rect_size(&mut self, room_id: RoomId, width: f64, depth: f64) -> Result<(), TopologyError> {
        let room = self.rooms.get(&room_id).ok_or(TopologyError::NotFound)?;
        let (lo, ow, od) = self.rect_dims(room).ok_or(TopologyError::NotRectangle)?;
        if width < 0.1 || depth < 0.1 {
            return Err(TopologyError::Degenerate);
        }
        let eps = 1e-6;
        let (old_x, old_y) = (lo.x + ow, lo.y + od);
        let (new_x, new_y) = (lo.x + width, lo.y + depth);
        let nodes = room.nodes.clone();
        for nid in &nodes {
            let p = self.nodes[nid].pos;
            if (p.x - old_x).abs() > eps && p.x >= new_x - eps {
                return Err(TopologyError::BlockedByJunction);
            }
            if (p.y - old_y).abs() > eps && p.y >= new_y - eps {
                return Err(TopologyError::BlockedByJunction);
            }
        }
        for nid in &nodes {
            let node = self.nodes.get_mut(nid).unwrap();
            if (node.pos.x - old_x).abs() < eps {
                node.pos.x = new_x;
            }
            if (node.pos.y - old_y).abs() < eps {
                node.pos.y = new_y;
            }
        }
        self.clamp_openings();
        Ok(())
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
        l.set_rect_size(a, 5.0, 3.5).unwrap();
        let (_, w, d) = l.rect_dims(&l.rooms[&a]).unwrap();
        assert!((w - 5.0).abs() < 1e-9 && (d - 3.5).abs() < 1e-9);
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

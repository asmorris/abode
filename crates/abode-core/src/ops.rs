//! Project-level operations and lookups. UI code should prefer these over
//! poking at levels directly, because they keep references (paint, notes,
//! etc.) consistent.

use crate::geometry::*;
use crate::ids::*;
use crate::model::*;
use crate::topology::{SNAP_TOLERANCE, TopologyChanges, TopologyError};

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        let info = HouseInfo { name: name.into(), ..Default::default() };
        Self {
            id: ProjectId::new(),
            info,
            roof: Roof::default(),
            levels: vec![Level::new("Main floor", 0.0)],
            notes: Vec::new(),
            service_records: Vec::new(),
            reminders: Vec::new(),
            paints: Vec::new(),
            changelog: Vec::new(),
        }
    }

    // ---- lookups -----------------------------------------------------------

    pub fn level(&self, id: LevelId) -> Option<&Level> {
        self.levels.iter().find(|l| l.id == id)
    }

    pub fn level_mut(&mut self, id: LevelId) -> Option<&mut Level> {
        self.levels.iter_mut().find(|l| l.id == id)
    }

    pub fn find_room(&self, id: RoomId) -> Option<(&Level, &Room)> {
        self.levels.iter().find_map(|l| l.rooms.get(&id).map(|r| (l, r)))
    }

    pub fn find_wall(&self, id: WallId) -> Option<(&Level, &Wall)> {
        self.levels.iter().find_map(|l| l.walls.get(&id).map(|w| (l, w)))
    }

    pub fn find_opening(&self, id: OpeningId) -> Option<(&Level, &Opening)> {
        self.levels.iter().find_map(|l| l.openings.get(&id).map(|o| (l, o)))
    }

    pub fn find_object(&self, id: ObjectId) -> Option<(&Level, &PlacedObject)> {
        self.levels.iter().find_map(|l| l.objects.get(&id).map(|o| (l, o)))
    }

    /// Index of the level an entity lives on (if it lives on one).
    pub fn level_index_of(&self, r: EntityRef) -> Option<usize> {
        self.levels.iter().position(|l| match r {
            EntityRef::Level(id) => l.id == id,
            EntityRef::Room(id) => l.rooms.contains_key(&id),
            EntityRef::Wall(id) => l.walls.contains_key(&id),
            EntityRef::Opening(id) => l.openings.contains_key(&id),
            EntityRef::Object(id) => l.objects.contains_key(&id),
            EntityRef::General | EntityRef::Roof => false,
        })
    }

    pub fn exists(&self, r: EntityRef) -> bool {
        match r {
            EntityRef::General | EntityRef::Roof => true,
            _ => self.level_index_of(r).is_some(),
        }
    }

    pub fn paint_target_exists(&self, t: PaintTarget) -> bool {
        match t {
            PaintTarget::Room(r) => self.exists(EntityRef::Room(r)),
            PaintTarget::WallFace { wall, room } => {
                self.exists(EntityRef::Wall(wall)) && room.is_none_or(|r| self.exists(EntityRef::Room(r)))
            }
        }
    }

    /// A human name for anything in the house.
    pub fn entity_name(&self, r: EntityRef) -> String {
        match r {
            EntityRef::General => {
                if self.info.name.trim().is_empty() {
                    "House".into()
                } else {
                    self.info.name.clone()
                }
            }
            EntityRef::Roof => "Roof".into(),
            EntityRef::Level(id) => self.level(id).map(|l| l.name.clone()).unwrap_or_else(|| "Level".into()),
            EntityRef::Room(id) => self
                .find_room(id)
                .map(|(_, r)| r.display_name())
                .unwrap_or_else(|| "Room".into()),
            EntityRef::Wall(id) => match self.find_wall(id) {
                Some((_, w)) if !w.label.trim().is_empty() => w.label.clone(),
                Some((l, w)) => {
                    let rooms: Vec<String> = l
                        .rooms_of_wall(w)
                        .into_iter()
                        .filter_map(|r| l.rooms.get(&r).map(|r| r.display_name()))
                        .collect();
                    match rooms.len() {
                        0 => "Wall".into(),
                        1 => format!("{} exterior wall", rooms[0]),
                        _ => format!("Wall {}", rooms.join(" / ")),
                    }
                }
                None => "Wall".into(),
            },
            EntityRef::Opening(id) => match self.find_opening(id) {
                Some((_, o)) if !o.label.trim().is_empty() => o.label.clone(),
                Some((l, o)) => {
                    let room = l
                        .walls
                        .get(&o.wall)
                        .and_then(|w| l.rooms_of_wall(w).first().copied())
                        .and_then(|r| l.rooms.get(&r))
                        .map(|r| r.display_name());
                    match room {
                        Some(r) => format!("{} ({r})", o.kind.title()),
                        None => o.kind.title().into(),
                    }
                }
                None => "Opening".into(),
            },
            EntityRef::Object(id) => self
                .find_object(id)
                .map(|(_, o)| o.display_name())
                .unwrap_or_else(|| "Object".into()),
        }
    }

    pub fn notes_for(&self, target: EntityRef) -> impl Iterator<Item = &Note> {
        self.notes.iter().filter(move |n| n.target == target)
    }

    pub fn service_for(&self, target: EntityRef) -> impl Iterator<Item = &ServiceRecord> {
        self.service_records.iter().filter(move |s| s.target == target)
    }

    pub fn reminders_for(&self, target: EntityRef) -> impl Iterator<Item = &Reminder> {
        self.reminders.iter().filter(move |r| r.target == target)
    }

    // ---- edits -------------------------------------------------------------

    pub fn add_level(&mut self, name: impl Into<String>) -> LevelId {
        let elevation = self
            .levels
            .iter()
            .map(|l| l.elevation + l.wall_height)
            .fold(0.0, f64::max);
        let level = Level::new(name, elevation);
        let id = level.id;
        self.levels.push(level);
        id
    }

    pub fn add_room(&mut self, level: LevelId, name: &str, points: &[Vec2]) -> Result<RoomId, TopologyError> {
        let thickness = self.info.default_wall_thickness;
        let lvl = self.level_mut(level).ok_or(TopologyError::NotFound)?;
        let (id, changes) = lvl.add_room(name, points, SNAP_TOLERANCE, thickness)?;
        self.apply_topology_changes(&changes);
        Ok(id)
    }

    /// A sensible default name like "Room 3".
    pub fn next_room_name(&self) -> String {
        let count: usize = self.levels.iter().map(|l| l.rooms.len()).sum();
        format!("Room {}", count + 1)
    }

    pub fn add_opening(&mut self, level: LevelId, wall: WallId, kind: OpeningKind, offset: f64) -> Option<OpeningId> {
        let lvl = self.level_mut(level)?;
        lvl.walls.get(&wall)?;
        let o = Opening::new(wall, kind, offset);
        let id = o.id;
        lvl.openings.insert(id, o);
        lvl.clamp_openings();
        Some(id)
    }

    /// Place an object. It must land inside a room, which it then belongs to.
    pub fn add_object(
        &mut self,
        level: LevelId,
        label: &str,
        category: ObjectCategory,
        shape: Shape,
        position: Vec2,
    ) -> Option<ObjectId> {
        let lvl = self.level_mut(level)?;
        let room = lvl.room_at(position)?;
        let obj = PlacedObject {
            id: ObjectId::new(),
            room,
            label: label.to_string(),
            category,
            shape,
            position,
            rotation: 0.0,
            color: None,
            details: ItemDetails::default(),
        };
        let id = obj.id;
        lvl.objects.insert(id, obj);
        Some(id)
    }

    /// Move an object; if it's dropped into another room it moves there too.
    pub fn move_object(&mut self, id: ObjectId, position: Vec2) {
        for lvl in &mut self.levels {
            if lvl.objects.contains_key(&id) {
                let room = lvl.room_at(position);
                let obj = lvl.objects.get_mut(&id).unwrap();
                obj.position = position;
                if let Some(r) = room {
                    obj.room = r;
                }
                return;
            }
        }
    }

    /// Delete something (and everything that hangs off it). Returns false if
    /// it can't be deleted directly (walls belong to rooms).
    pub fn delete_entity(&mut self, r: EntityRef) -> bool {
        let thickness = self.info.default_wall_thickness;
        let ok = match r {
            EntityRef::General | EntityRef::Roof | EntityRef::Wall(_) => false,
            EntityRef::Level(id) => {
                if self.levels.len() > 1 {
                    self.levels.retain(|l| l.id != id);
                    true
                } else {
                    false
                }
            }
            EntityRef::Room(id) => {
                let changes = self
                    .levels
                    .iter_mut()
                    .find(|l| l.rooms.contains_key(&id))
                    .map(|l| l.remove_room(id, thickness));
                match changes {
                    Some(c) => {
                        self.apply_topology_changes(&c);
                        true
                    }
                    None => false,
                }
            }
            EntityRef::Opening(id) => self.levels.iter_mut().any(|l| l.openings.remove(&id).is_some()),
            EntityRef::Object(id) => self.levels.iter_mut().any(|l| l.objects.remove(&id).is_some()),
        };
        if ok {
            self.cleanup_orphans();
        }
        ok
    }

    pub(crate) fn apply_topology_changes(&mut self, changes: &TopologyChanges) {
        // Paint on a wall that got split applies to both halves.
        for (old, new) in &changes.split_walls {
            let copies: Vec<Paint> = self
                .paints
                .iter()
                .filter_map(|p| match p.target {
                    PaintTarget::WallFace { wall, room } if wall == *old => {
                        let mut c = p.clone();
                        c.id = PaintId::new();
                        c.target = PaintTarget::WallFace { wall: *new, room };
                        Some(c)
                    }
                    _ => None,
                })
                .collect();
            self.paints.extend(copies);
        }
        self.cleanup_orphans();
    }

    /// Drop notes/history/paint whose subject no longer exists.
    pub fn cleanup_orphans(&mut self) {
        let notes = std::mem::take(&mut self.notes);
        self.notes = notes.into_iter().filter(|n| self.exists(n.target)).collect();
        let svc = std::mem::take(&mut self.service_records);
        self.service_records = svc.into_iter().filter(|s| self.exists(s.target)).collect();
        let rem = std::mem::take(&mut self.reminders);
        self.reminders = rem.into_iter().filter(|r| self.exists(r.target)).collect();
        let paints = std::mem::take(&mut self.paints);
        self.paints = paints.into_iter().filter(|p| self.paint_target_exists(p.target)).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleting_room_removes_its_notes_and_objects() {
        let mut p = Project::new("Test");
        let l = p.levels[0].id;
        let room = p.add_room(l, "Kitchen", &rect_points(Vec2::ZERO, 4.0, 3.0)).unwrap();
        let obj = p
            .add_object(l, "Fridge", ObjectCategory::Appliance, Shape::new(ShapeKind::Rectangle, 0.9, 0.7), Vec2::new(1.0, 1.0))
            .unwrap();
        p.notes.push(Note::new(EntityRef::Room(room)));
        p.notes.push(Note::new(EntityRef::Object(obj)));
        p.notes.push(Note::new(EntityRef::General));
        assert!(p.delete_entity(EntityRef::Room(room)));
        assert_eq!(p.notes.len(), 1);
        assert!(p.find_object(obj).is_none());
    }

    #[test]
    fn objects_must_be_in_a_room() {
        let mut p = Project::new("Test");
        let l = p.levels[0].id;
        p.add_room(l, "Kitchen", &rect_points(Vec2::ZERO, 4.0, 3.0)).unwrap();
        let shape = Shape::new(ShapeKind::Circle, 0.5, 0.5);
        assert!(p.add_object(l, "x", ObjectCategory::Other, shape, Vec2::new(10.0, 10.0)).is_none());
        assert!(p.add_object(l, "x", ObjectCategory::Other, shape, Vec2::new(1.0, 1.0)).is_some());
    }

    #[test]
    fn wall_paint_follows_split() {
        let mut p = Project::new("Test");
        let l = p.levels[0].id;
        let a = p.add_room(l, "A", &rect_points(Vec2::ZERO, 4.0, 4.0)).unwrap();
        let (wall, _, _) = p.levels[0].nearest_wall(Vec2::new(4.0, 2.0), 0.1).unwrap();
        p.paints.push(Paint::new(PaintTarget::WallFace { wall, room: Some(a) }));
        p.add_room(l, "B", &rect_points(Vec2::new(4.0, 0.0), 2.0, 2.0)).unwrap();
        assert_eq!(p.paints.len(), 2);
    }
}

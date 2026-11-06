//! Project-level operations and lookups. UI code should prefer these over
//! poking at levels directly, because they keep references (paint, notes,
//! etc.) consistent.

use crate::geometry::{self, *};
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
            levels: vec![Level::new("Main floor", 0.0), Level::new_site()],
            buildings: Vec::new(),
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

    pub fn find_roof(&self, id: RoofId) -> Option<(&Level, &RoofSection)> {
        self.levels.iter().find_map(|l| l.roofs.get(&id).map(|r| (l, r)))
    }

    pub fn find_feature(&self, id: FeatureId) -> Option<(&Level, &Feature)> {
        self.levels.iter().find_map(|l| l.features.get(&id).map(|f| (l, f)))
    }

    pub fn building(&self, id: BuildingId) -> Option<&Building> {
        self.buildings.iter().find(|b| b.id == id)
    }

    pub fn building_mut(&mut self, id: BuildingId) -> Option<&mut Building> {
        self.buildings.iter_mut().find(|b| b.id == id)
    }

    // ---- levels: house, outbuildings, outside -------------------------------

    /// Make sure there's an "Outside" level (older files don't have one).
    pub fn ensure_site(&mut self) {
        if !self.levels.iter().any(|l| l.is_site()) {
            self.levels.push(Level::new_site());
        }
        if !self.levels.iter().any(|l| !l.is_site() && l.building.is_none()) {
            self.levels.insert(0, Level::new("Main floor", 0.0));
        }
    }

    pub fn site(&self) -> Option<&Level> {
        self.levels.iter().find(|l| l.is_site())
    }

    pub fn site_id(&self) -> Option<LevelId> {
        self.site().map(|l| l.id)
    }

    /// The house's own floors (not outbuildings, not outside).
    pub fn house_levels(&self) -> impl Iterator<Item = &Level> {
        self.levels.iter().filter(|l| !l.is_site() && l.building.is_none())
    }

    pub fn building_levels(&self, b: BuildingId) -> impl Iterator<Item = &Level> {
        self.levels.iter().filter(move |l| l.building == Some(b))
    }

    /// Floor area of the house: every room on its floors (not outbuildings,
    /// not outside), measured to wall centrelines like the plan's room labels.
    /// Returns the total and each floor that has rooms.
    pub fn house_floor_area(&self) -> (f64, Vec<(LevelId, f64)>) {
        let floors: Vec<(LevelId, f64)> = self
            .house_levels()
            .map(|l| (l.id, l.rooms.values().map(|r| geometry::area(&l.room_points(r))).sum::<f64>()))
            .filter(|(_, a)| *a > 0.0)
            .collect();
        (floors.iter().map(|f| f.1).sum(), floors)
    }

    pub fn first_house_level(&self) -> LevelId {
        self.ground_level(None).or(self.house_levels().next()).or(self.levels.first()).map(|l| l.id).unwrap_or_default()
    }

    /// The floors of the house (`None`) or an outbuilding, bottom to top.
    pub fn floor_stack(&self, building: Option<BuildingId>) -> Vec<&Level> {
        let mut v: Vec<&Level> = self.levels.iter().filter(|l| !l.is_site() && l.building == building).collect();
        v.sort_by(|a, b| a.elevation.total_cmp(&b.elevation));
        v
    }

    /// The floors directly below and above `level` in its building.
    pub fn neighbour_levels(&self, level: LevelId) -> (Option<&Level>, Option<&Level>) {
        let Some(l) = self.level(level).filter(|l| !l.is_site()) else { return (None, None) };
        let stack = self.floor_stack(l.building);
        let Some(i) = stack.iter().position(|x| x.id == level) else { return (None, None) };
        (i.checked_sub(1).map(|j| stack[j]), stack.get(i + 1).copied())
    }

    /// Keep `levels` in stacking order (bottom floor first), with the site
    /// last. Call after changing a floor's height.
    pub fn sort_levels(&mut self) {
        self.levels.sort_by(|a, b| a.is_site().cmp(&b.is_site()).then(a.elevation.total_cmp(&b.elevation)));
    }

    /// The ground floor of the house (`None`) or of an outbuilding: the floor
    /// nearest ground level, preferring ones at or above it.
    pub fn ground_level(&self, building: Option<BuildingId>) -> Option<&Level> {
        self.levels
            .iter()
            .filter(|l| !l.is_site() && l.building == building)
            .min_by(|a, b| {
                let key = |l: &Level| (l.elevation < -0.01, (l.elevation.abs() * 1000.0) as i64);
                key(a).cmp(&key(b))
            })
    }

    /// Index of the level an entity lives on (if it lives on one).
    pub fn level_index_of(&self, r: EntityRef) -> Option<usize> {
        self.levels.iter().position(|l| match r {
            EntityRef::Level(id) => l.id == id,
            EntityRef::Room(id) => l.rooms.contains_key(&id),
            EntityRef::Wall(id) => l.walls.contains_key(&id),
            EntityRef::Opening(id) => l.openings.contains_key(&id),
            EntityRef::Object(id) => l.objects.contains_key(&id),
            EntityRef::RoofSection(id) => l.roofs.contains_key(&id),
            EntityRef::Feature(id) => l.features.contains_key(&id),
            EntityRef::General | EntityRef::Roof | EntityRef::Building(_) => false,
        })
    }

    pub fn exists(&self, r: EntityRef) -> bool {
        match r {
            EntityRef::General | EntityRef::Roof => true,
            EntityRef::Building(id) => self.building(id).is_some(),
            _ => self.level_index_of(r).is_some(),
        }
    }

    pub fn paint_target_exists(&self, t: PaintTarget) -> bool {
        match t {
            PaintTarget::Room(r) => self.exists(EntityRef::Room(r)),
            PaintTarget::WallFace { wall, room } => {
                self.exists(EntityRef::Wall(wall)) && room.is_none_or(|r| self.exists(EntityRef::Room(r)))
            }
            PaintTarget::Feature(f) => self.exists(EntityRef::Feature(f)),
            PaintTarget::Building(b) => self.exists(EntityRef::Building(b)),
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
            EntityRef::Level(id) => match self.level(id) {
                Some(l) => match l.building.and_then(|b| self.building(b)) {
                    Some(b) => format!("{} · {}", b.display_name(), l.name),
                    None => l.name.clone(),
                },
                None => "Level".into(),
            },
            EntityRef::RoofSection(id) => self
                .find_roof(id)
                .map(|(_, r)| r.display_name())
                .unwrap_or_else(|| "Roof".into()),
            EntityRef::Feature(id) => self
                .find_feature(id)
                .map(|(_, f)| f.display_name())
                .unwrap_or_else(|| "Outside".into()),
            EntityRef::Building(id) => self
                .building(id)
                .map(|b| b.display_name())
                .unwrap_or_else(|| "Building".into()),
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

    /// Add a floor on top of the house (`None`) or an outbuilding.
    pub fn add_level(&mut self, building: Option<BuildingId>, name: impl Into<String>) -> LevelId {
        let elevation = self
            .levels
            .iter()
            .filter(|l| !l.is_site() && l.building == building)
            .map(|l| l.elevation + l.wall_height)
            .fold(0.0, f64::max);
        let mut level = Level::new(name, elevation);
        level.building = building;
        let id = level.id;
        // Keep the site last so it reads as "below" the floors in lists.
        self.levels.push(level);
        self.sort_levels();
        id
    }

    /// A new outbuilding with one floor and one room drawn to `outline`.
    pub fn add_building(&mut self, name: &str, kind: &str, outline: &[Vec2]) -> Result<(BuildingId, LevelId), TopologyError> {
        let b = Building { id: BuildingId::new(), name: name.to_string(), kind: kind.to_string(), details: ItemDetails::default() };
        let bid = b.id;
        self.buildings.push(b);
        let lid = self.add_level(Some(bid), "Ground floor");
        if let Some(l) = self.level_mut(lid) {
            l.wall_height = 2.4;
        }
        let thickness = 0.1;
        let lvl = self.level_mut(lid).ok_or(TopologyError::NotFound)?;
        match lvl.add_room(name, outline, SNAP_TOLERANCE, thickness) {
            Ok((rid, changes)) => {
                if let Some(r) = self.level_mut(lid).and_then(|l| l.rooms.get_mut(&rid)) {
                    r.kind = kind.to_string();
                    r.shape_locked = true;
                }
                self.apply_topology_changes(&changes);
                Ok((bid, lid))
            }
            Err(e) => {
                self.levels.retain(|l| l.id != lid);
                self.buildings.retain(|b| b.id != bid);
                Err(e)
            }
        }
    }

    /// Add a floor underneath the house's (or outbuilding's) lowest floor.
    pub fn add_basement(&mut self, building: Option<BuildingId>, name: impl Into<String>) -> LevelId {
        let bottom = self
            .levels
            .iter()
            .filter(|l| !l.is_site() && l.building == building)
            .map(|l| l.elevation)
            .fold(0.0, f64::min);
        let mut level = Level::new(name, bottom - DEFAULT_WALL_HEIGHT);
        level.building = building;
        let id = level.id;
        self.levels.push(level);
        self.sort_levels();
        id
    }

    /// Move a floor one place up (`up`) or down its building's stack. The
    /// floors are re-stacked so none overlap: each keeps its storey height
    /// (the gap to the floor above it, or its wall height if it's on top),
    /// and the ground floor stays where it is, so moving a floor below it
    /// makes that floor a basement. Returns false if it's already at the
    /// top/bottom.
    pub fn move_level(&mut self, id: LevelId, up: bool) -> bool {
        let Some(level) = self.level(id).filter(|l| !l.is_site()) else { return false };
        let building = level.building;
        let Some(ground) = self.ground_level(building).map(|g| (g.id, g.elevation)) else { return false };
        let stack: Vec<(LevelId, f64, f64)> = {
            let s = self.floor_stack(building);
            s.iter()
                .enumerate()
                .map(|(i, l)| {
                    let storey = s.get(i + 1).map(|n| n.elevation - l.elevation).filter(|h| *h > 0.5).unwrap_or(l.wall_height);
                    (l.id, l.elevation, storey)
                })
                .collect()
        };
        let Some(i) = stack.iter().position(|x| x.0 == id) else { return false };
        let j = if up { i + 1 } else { match i.checked_sub(1) { Some(j) => j, None => return false } };
        if j >= stack.len() {
            return false;
        }
        let mut order = stack;
        order.swap(i, j);
        // Stack upwards and downwards from the ground floor.
        let g = order.iter().position(|x| x.0 == ground.0).unwrap_or(0);
        let mut elev = vec![0.0; order.len()];
        elev[g] = ground.1;
        for k in g + 1..order.len() {
            elev[k] = elev[k - 1] + order[k - 1].2;
        }
        for k in (0..g).rev() {
            elev[k] = elev[k + 1] - order[k].2;
        }
        for (k, (lid, _, _)) in order.iter().enumerate() {
            if let Some(l) = self.level_mut(*lid) {
                l.elevation = elev[k];
            }
        }
        self.sort_levels();
        true
    }

    pub fn add_roof(&mut self, level: LevelId, outline: &[Vec2], shape: RoofShape) -> Option<RoofId> {
        if outline.len() < 3 || geometry::area(outline) < 0.25 || !geometry::is_simple(outline) {
            return None;
        }
        let lvl = self.level_mut(level)?;
        if lvl.is_site() {
            return None;
        }
        let r = RoofSection::new(outline, shape);
        let id = r.id;
        lvl.roofs.insert(id, r);
        Some(id)
    }

    pub fn add_feature(&mut self, kind: FeatureKind, points: Vec<Vec2>) -> Option<FeatureId> {
        let ok = match kind.geom() {
            FeatureGeom::Area => points.len() >= 3 && geometry::area(&points) > 0.05 && geometry::is_simple(&points),
            FeatureGeom::Line => points.len() >= 2 && geometry::polyline_length(&points) > 0.2,
            FeatureGeom::Point => points.len() == 1,
        };
        if !ok {
            return None;
        }
        self.ensure_site();
        let site = self.levels.iter_mut().find(|l| l.is_site())?;
        let f = Feature::new(kind, points);
        let id = f.id;
        site.features.insert(id, f);
        Some(id)
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

    /// Place an object. Indoors it must land inside a room, which it then
    /// belongs to; outside it can go anywhere (and sits on a deck/patio if
    /// dropped on one).
    pub fn add_object(
        &mut self,
        level: LevelId,
        label: &str,
        category: ObjectCategory,
        shape: Shape,
        position: Vec2,
    ) -> Option<ObjectId> {
        let lvl = self.level_mut(level)?;
        let (room, feature) = if lvl.is_site() {
            (None, lvl.feature_at(position))
        } else {
            (Some(lvl.room_at(position)?), None)
        };
        Self::insert_object(lvl, room, feature, None, label, category, shape, position)
    }

    /// Place something on a roof section (skylight, chimney, solar panels...).
    pub fn add_roof_object(
        &mut self,
        level: LevelId,
        label: &str,
        category: ObjectCategory,
        shape: Shape,
        position: Vec2,
    ) -> Option<ObjectId> {
        let lvl = self.level_mut(level)?;
        let roof = lvl.roof_at(position)?;
        Self::insert_object(lvl, None, None, Some(roof), label, category, shape, position)
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_object(
        lvl: &mut Level,
        room: Option<RoomId>,
        feature: Option<FeatureId>,
        roof: Option<RoofId>,
        label: &str,
        category: ObjectCategory,
        shape: Shape,
        position: Vec2,
    ) -> Option<ObjectId> {
        // A vehicle is named by its year/make/model once those are filled in;
        // the preset ("SUV / crossover") is its body style.
        let is_vehicle = category == ObjectCategory::Vehicle;
        let obj = PlacedObject {
            id: ObjectId::new(),
            room,
            feature,
            roof,
            label: if is_vehicle { String::new() } else { label.to_string() },
            category,
            shape,
            position,
            rotation: 0.0,
            color: None,
            details: ItemDetails {
                custom_fields: starter_fields(label).iter().map(|k| CustomField { key: k.to_string(), ..Default::default() }).collect(),
                ..Default::default()
            },
            vehicle: is_vehicle.then(|| Box::new(crate::vehicle::Vehicle { body: label.to_string(), ..Default::default() })),
        };
        let id = obj.id;
        lvl.objects.insert(id, obj);
        Some(id)
    }

    /// Move an object; if it's dropped into another room (or onto another
    /// deck, or roof section) it moves there too.
    pub fn move_object(&mut self, id: ObjectId, position: Vec2) {
        for lvl in &mut self.levels {
            let Some(obj) = lvl.objects.get(&id) else { continue };
            let on_roof = obj.roof.is_some();
            let (room, feature, roof) = (lvl.room_at(position), lvl.feature_at(position), lvl.roof_at(position));
            let site = lvl.is_site();
            let obj = lvl.objects.get_mut(&id).unwrap();
            obj.position = position;
            if on_roof {
                if roof.is_some() {
                    obj.roof = roof;
                }
            } else if site {
                obj.feature = feature;
            } else if room.is_some() {
                obj.room = room;
            }
            return;
        }
    }

    /// Move an area/line feature (and anything sitting on it) by `delta`.
    pub fn translate_feature(&mut self, id: FeatureId, delta: Vec2) {
        for lvl in &mut self.levels {
            if let Some(f) = lvl.features.get_mut(&id) {
                for p in &mut f.points {
                    *p += delta;
                }
                for o in lvl.objects.values_mut().filter(|o| o.feature == Some(id)) {
                    o.position += delta;
                }
                return;
            }
        }
    }

    /// Move a roof section (and what's on it) by `delta`.
    pub fn translate_roof(&mut self, id: RoofId, delta: Vec2) {
        for lvl in &mut self.levels {
            if let Some(r) = lvl.roofs.get_mut(&id) {
                for p in &mut r.outline {
                    *p += delta;
                }
                for o in lvl.objects.values_mut().filter(|o| o.roof == Some(id)) {
                    o.position += delta;
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
                let Some(l) = self.level(id) else { return false };
                let siblings = self
                    .levels
                    .iter()
                    .filter(|x| !x.is_site() && x.building == l.building)
                    .count();
                if !l.is_site() && siblings > 1 {
                    self.levels.retain(|l| l.id != id);
                    true
                } else {
                    false
                }
            }
            EntityRef::Building(id) => {
                let before = self.buildings.len();
                self.buildings.retain(|b| b.id != id);
                self.levels.retain(|l| l.building != Some(id));
                self.buildings.len() != before
            }
            EntityRef::RoofSection(id) => self.levels.iter_mut().any(|l| {
                let found = l.roofs.remove(&id).is_some();
                if found {
                    l.objects.retain(|_, o| o.roof != Some(id));
                }
                found
            }),
            EntityRef::Feature(id) => self.levels.iter_mut().any(|l| {
                let found = l.features.remove(&id).is_some();
                if found {
                    l.objects.retain(|_, o| o.feature != Some(id));
                }
                found
            }),
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
        // Two walls that became one: notes, history and paint follow along.
        for (gone, keep) in &changes.merged_walls {
            let fix = |t: &mut EntityRef| {
                if *t == EntityRef::Wall(*gone) {
                    *t = EntityRef::Wall(*keep);
                }
            };
            self.notes.iter_mut().for_each(|n| fix(&mut n.target));
            self.service_records.iter_mut().for_each(|s| fix(&mut s.target));
            self.reminders.iter_mut().for_each(|r| fix(&mut r.target));
            for p in &mut self.paints {
                if let PaintTarget::WallFace { wall, .. } = &mut p.target {
                    if *wall == *gone {
                        *wall = *keep;
                    }
                }
            }
        }
        self.cleanup_orphans();
    }

    /// Resize a rectangular room (see [`Level::set_rect_size`]).
    pub fn set_room_size(&mut self, level: LevelId, room: RoomId, width: f64, depth: f64) -> Result<(), TopologyError> {
        let thickness = self.info.default_wall_thickness;
        let lvl = self.level_mut(level).ok_or(TopologyError::NotFound)?;
        let changes = lvl.set_rect_size(room, width, depth, SNAP_TOLERANCE, thickness)?;
        self.apply_topology_changes(&changes);
        Ok(())
    }

    /// Line a slightly-off room up with its neighbours (see [`Level::straighten_room`]).
    /// Corners within 5 cm (2") are pulled into line.
    pub fn straighten_room(&mut self, level: LevelId, room: RoomId) -> Result<(), TopologyError> {
        let thickness = self.info.default_wall_thickness;
        let lvl = self.level_mut(level).ok_or(TopologyError::NotFound)?;
        let changes = lvl.straighten_room(room, 0.05, SNAP_TOLERANCE, thickness)?;
        self.apply_topology_changes(&changes);
        Ok(())
    }

    /// Change one wall of a room (see [`Level::set_wall_length`]).
    pub fn set_wall_length(&mut self, level: LevelId, room: RoomId, wall: WallId, length: f64) -> Result<(), TopologyError> {
        let thickness = self.info.default_wall_thickness;
        let lvl = self.level_mut(level).ok_or(TopologyError::NotFound)?;
        let changes = lvl.set_wall_length(room, wall, length, SNAP_TOLERANCE, thickness)?;
        self.apply_topology_changes(&changes);
        Ok(())
    }

    /// Before dragging a corner of `room`: if a locked room shares it, give
    /// `room` its own copy to drag. Returns the corner to move.
    pub fn begin_corner_move(&mut self, level: LevelId, room: RoomId, node: NodeId) -> NodeId {
        let thickness = self.info.default_wall_thickness;
        let Some(lvl) = self.level_mut(level) else { return node };
        if !lvl.pinned_by_locked(room, node) {
            return node;
        }
        let Some(copy) = lvl.detach_node(room, node) else { return node };
        let mut changes = TopologyChanges::default();
        lvl.rebuild_walls(thickness, &mut changes);
        self.apply_topology_changes(&changes);
        copy
    }

    /// After dragging a corner (see [`Level::finish_corner_move`]).
    pub fn finish_corner_move(&mut self, level: LevelId, room: RoomId, node: NodeId, start: Vec2) -> Result<(), TopologyError> {
        let thickness = self.info.default_wall_thickness;
        let lvl = self.level_mut(level).ok_or(TopologyError::NotFound)?;
        let result = lvl.finish_corner_move(room, node, start, SNAP_TOLERANCE, thickness);
        if let Ok(changes) = &result {
            self.apply_topology_changes(changes);
        }
        result.map(|_| ())
    }

    /// Finish moving a room: put it back if it overlaps, else join it to
    /// rooms it now touches.
    pub fn settle_room(&mut self, level: LevelId, room: RoomId, undo: Vec2) -> Result<(), TopologyError> {
        let thickness = self.info.default_wall_thickness;
        let lvl = self.level_mut(level).ok_or(TopologyError::NotFound)?;
        let changes = lvl.settle_room(room, undo, SNAP_TOLERANCE, thickness)?;
        self.apply_topology_changes(&changes);
        Ok(())
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
    fn house_floor_area_adds_up_rooms_on_house_floors() {
        let mut p = Project::new("Test");
        let l = p.levels[0].id;
        p.add_room(l, "Kitchen", &rect_points(Vec2::ZERO, 4.0, 3.0)).unwrap();
        p.add_room(l, "Dining", &rect_points(Vec2::new(4.0, 0.0), 2.0, 3.0)).unwrap();
        let (total, floors) = p.house_floor_area();
        assert!((total - 18.0).abs() < 1e-9);
        assert_eq!(floors.len(), 1);
    }

    #[test]
    fn objects_must_be_in_a_room() {
        let mut p = Project::new("Test");
        let l = p.levels[0].id;
        p.add_room(l, "Kitchen", &rect_points(Vec2::ZERO, 4.0, 3.0)).unwrap();
        let shape = Shape::new(ShapeKind::Circle, 0.5, 0.5);
        // ...but outside, anything goes.
        let site = p.site_id().unwrap();
        assert!(p.add_object(site, "BBQ", ObjectCategory::Outdoor, shape, Vec2::new(10.0, 10.0)).is_some());
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

    #[test]
    fn items_follow_their_deck_and_go_with_it() {
        let mut p = Project::new("Test");
        let deck = p.add_feature(FeatureKind::Deck, rect_points(Vec2::ZERO, 4.0, 3.0)).unwrap();
        let site = p.site_id().unwrap();
        let shape = Shape::new(ShapeKind::Rectangle, 1.0, 0.5);
        let bbq = p.add_object(site, "BBQ", ObjectCategory::Outdoor, shape, Vec2::new(1.0, 1.0)).unwrap();
        assert_eq!(p.find_object(bbq).unwrap().1.feature, Some(deck));
        p.translate_feature(deck, Vec2::new(1.0, 0.0));
        assert!((p.find_object(bbq).unwrap().1.position.x - 2.0).abs() < 1e-9);
        p.move_object(bbq, Vec2::new(20.0, 20.0));
        assert_eq!(p.find_object(bbq).unwrap().1.feature, None);
        p.move_object(bbq, Vec2::new(2.0, 1.0));
        p.delete_entity(EntityRef::Feature(deck));
        assert!(p.find_object(bbq).is_none());
    }

    #[test]
    fn ev_charger_goes_outside_and_in_outbuildings() {
        let charger = OBJECT_PRESETS.iter().find(|p| p.label == "EV charger").unwrap();
        let none = Spot::default();
        assert!(charger.fits(Place::Outdoor, none));
        assert!(!charger.fits(Place::Indoor, none));
        assert!(charger.fits(Place::Indoor, Spot { outbuilding: true, ..none }));
        assert!(charger.fits(Place::Indoor, Spot { parking: true, ..none }));
        assert!(!charger.fits(Place::Roof, none));
        let car = OBJECT_PRESETS.iter().find(|p| p.label == "Car").unwrap();
        assert!(!car.fits(Place::Indoor, Spot { outbuilding: true, ..none }), "no cars in the shed");
        // It starts with prompts for what's worth recording.
        let mut p = Project::new("T");
        let site = p.site_id().unwrap();
        let id = p.add_object(site, "EV charger", ObjectCategory::Electrical, Shape::new(ShapeKind::Rectangle, 0.35, 0.2), Vec2::new(0.0, 0.0)).unwrap();
        let keys: Vec<String> = p.find_object(id).unwrap().1.details.custom_fields.iter().map(|f| f.key.clone()).collect();
        assert_eq!(keys, ["Level", "Power", "Connector", "Breaker", "App / Wi-Fi"]);
    }

    #[test]
    fn levels_restack_when_moved() {
        let mut p = Project::new("Test");
        let main = p.first_house_level();
        let upper = p.add_level(None, "Upper");
        let third = p.add_level(None, "Third");
        let elev = |p: &Project, id| p.level(id).unwrap().elevation;
        let h = DEFAULT_WALL_HEIGHT;
        assert!((elev(&p, third) - 2.0 * h).abs() < 1e-9);
        assert!(!p.move_level(third, true));
        // Two moves down puts it under the main floor: a basement.
        assert!(p.move_level(third, false));
        assert!((elev(&p, third) - h).abs() < 1e-9);
        assert!((elev(&p, upper) - 2.0 * h).abs() < 1e-9);
        assert!(p.move_level(third, false));
        assert!((elev(&p, third) + h).abs() < 1e-9);
        assert!(elev(&p, main).abs() < 1e-9);
        assert!((elev(&p, upper) - h).abs() < 1e-9);
        assert!(!p.move_level(third, false));
        assert_eq!(p.first_house_level(), main);
        let order: Vec<LevelId> = p.house_levels().map(|l| l.id).collect();
        assert_eq!(order, vec![third, main, upper]);
        assert!(p.levels.last().unwrap().is_site());
        assert_eq!(p.neighbour_levels(main), (p.level(third), p.level(upper)));
        // A basement goes under the lowest floor.
        let sub = p.add_basement(None, "Sub-basement");
        assert!((elev(&p, sub) + 2.0 * h).abs() < 1e-9);
        assert_eq!(p.house_levels().next().unwrap().id, sub);
    }

    #[test]
    fn buildings_have_their_own_levels() {
        let mut p = Project::new("Test");
        let (b, l) = p.add_building("Shed", "Shed", &rect_points(Vec2::new(10.0, 0.0), 3.0, 2.0)).unwrap();
        assert_eq!(p.building_levels(b).count(), 1);
        assert_eq!(p.house_levels().count(), 1);
        assert_eq!(p.level(l).unwrap().rooms.len(), 1);
        assert!(p.levels.last().unwrap().is_site());
        // Can't delete a building's only level, or the site.
        assert!(!p.delete_entity(EntityRef::Level(l)));
        assert!(!p.delete_entity(EntityRef::Level(p.site_id().unwrap())));
        p.notes.push(Note::new(EntityRef::Building(b)));
        assert!(p.delete_entity(EntityRef::Building(b)));
        assert!(p.level(l).is_none());
        assert!(p.notes.is_empty());
    }

    #[test]
    fn roof_items_stay_on_roofs() {
        let mut p = Project::new("Test");
        let l = p.levels[0].id;
        p.add_room(l, "A", &rect_points(Vec2::ZERO, 8.0, 6.0)).unwrap();
        let roof = p.add_roof(l, &rect_points(Vec2::ZERO, 8.0, 6.0), RoofShape::Gable).unwrap();
        let shape = Shape::new(ShapeKind::Rectangle, 0.6, 1.2);
        let sky = p.add_roof_object(l, "Skylight", ObjectCategory::Roof, shape, Vec2::new(2.0, 2.0)).unwrap();
        let o = p.find_object(sky).unwrap().1;
        assert_eq!((o.roof, o.room), (Some(roof), None));
        assert!(p.add_roof_object(l, "x", ObjectCategory::Roof, shape, Vec2::new(30.0, 2.0)).is_none());
        p.delete_entity(EntityRef::RoofSection(roof));
        assert!(p.find_object(sky).is_none());
    }
}

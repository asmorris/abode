//! Automatic change log.
//!
//! Each committed edit is diffed against the previous state, record by record
//! (rooms, objects, notes...), and turned into readable entries like
//! "Updated item “Furnace” on Utility: model, serial". Rapid edits to the same
//! thing by the same person are merged into one entry so the log stays tidy.

use crate::ids::*;
use crate::model::*;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Edits to the same record within this window are merged.
pub const MERGE_MINUTES: i64 = 15;

struct Record {
    kind: String,
    name: String,
    refs: Vec<EntityRef>,
    owner: Option<String>,
    /// If the owner is deleted, the removal of this record isn't logged separately.
    owner_ref: Option<EntityRef>,
    value: Value,
    private: bool,
    /// Whether to log Added/Removed (walls are managed automatically, so not).
    log_lifecycle: bool,
}

fn val<T: Serialize>(v: &T, strip: &[&str]) -> Value {
    let mut v = serde_json::to_value(v).unwrap_or(Value::Null);
    if let Value::Object(m) = &mut v {
        for k in strip {
            m.remove(*k);
        }
    }
    v
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

fn short(s: &str) -> String {
    let line = s.lines().next().unwrap_or("").trim();
    if line.chars().count() > 40 {
        format!("{}…", line.chars().take(40).collect::<String>())
    } else {
        line.to_string()
    }
}

fn collect(p: &Project) -> BTreeMap<String, Record> {
    let mut out = BTreeMap::new();
    // Vehicles leave with their owner, so everything logged about one is
    // private (and stripped from a copy for a new owner), even after the
    // vehicle itself is gone.
    let cars: std::collections::BTreeSet<EntityRef> = p.vehicles().into_iter().map(|(_, id)| EntityRef::Object(id)).collect();
    // Pets are private the same way.
    let about_car = |t: &EntityRef| cars.contains(t) || matches!(t, EntityRef::Pet(_));
    let rec = |kind: &str, name: String, refs: Vec<EntityRef>, value: Value| Record {
        kind: kind.to_string(),
        name,
        refs,
        owner: None,
        owner_ref: None,
        value,
        private: false,
        log_lifecycle: true,
    };

    out.insert(
        "house".into(),
        Record {
            log_lifecycle: false,
            ..rec("house details", String::new(), vec![EntityRef::General], val(&p.info, &["created"]))
        },
    );
    out.insert(
        "roof".into(),
        Record {
            log_lifecycle: false,
            ..rec("roof details", String::new(), vec![EntityRef::Roof], val(&p.roof, &[]))
        },
    );

    for level in &p.levels {
        let lref = EntityRef::Level(level.id);
        out.insert(
            lref.key(),
            rec(
                "level",
                level.name.clone(),
                vec![lref],
                val(level, &["nodes", "walls", "rooms", "openings", "objects", "roofs", "features"]),
            ),
        );
        for room in level.rooms.values() {
            let mut v = val(room, &["nodes", "shape_locked"]);
            let pts: Vec<[f64; 2]> = level
                .room_points(room)
                .iter()
                .map(|q| [round4(q.x), round4(q.y)])
                .collect();
            if let Value::Object(m) = &mut v {
                m.insert("shape".into(), serde_json::to_value(pts).unwrap_or(Value::Null));
            }
            let r = EntityRef::Room(room.id);
            out.insert(
                r.key(),
                Record {
                    owner: Some(level.name.clone()),
                    owner_ref: Some(lref),
                    ..rec("room", room.display_name(), vec![r], v)
                },
            );
        }
        for wall in level.walls.values() {
            let rooms = level.rooms_of_wall(wall);
            let owner = rooms
                .iter()
                .filter_map(|r| level.rooms.get(r).map(|r| r.display_name()))
                .collect::<Vec<_>>()
                .join(" / ");
            let mut refs = vec![EntityRef::Wall(wall.id)];
            refs.extend(rooms.iter().map(|r| EntityRef::Room(*r)));
            out.insert(
                EntityRef::Wall(wall.id).key(),
                Record {
                    owner: (!owner.is_empty()).then_some(owner),
                    log_lifecycle: false,
                    ..rec("wall", wall.label.clone(), refs, val(wall, &["a", "b"]))
                },
            );
        }
        for o in level.openings.values() {
            let room = level
                .walls
                .get(&o.wall)
                .and_then(|w| level.rooms_of_wall(w).first().copied());
            let mut refs = vec![EntityRef::Opening(o.id)];
            refs.extend(room.map(EntityRef::Room));
            out.insert(
                EntityRef::Opening(o.id).key(),
                Record {
                    owner: room.and_then(|r| level.rooms.get(&r)).map(|r| r.display_name()),
                    owner_ref: room.map(EntityRef::Room),
                    ..rec(o.kind.noun(), o.label.clone(), refs, val(o, &["wall"]))
                },
            );
        }
        for obj in level.objects.values() {
            let parent = obj
                .roof
                .map(EntityRef::RoofSection)
                .or(obj.feature.map(EntityRef::Feature))
                .or(obj.room.map(EntityRef::Room));
            let owner = match parent {
                Some(r) => Some(p.entity_name(r)),
                None => Some(level.name.clone()),
            };
            let mut refs = vec![EntityRef::Object(obj.id)];
            refs.extend(parent);
            let vehicle = obj.vehicle.is_some();
            out.insert(
                EntityRef::Object(obj.id).key(),
                Record {
                    owner,
                    owner_ref: parent.or(Some(lref)),
                    private: vehicle,
                    ..rec(if vehicle { "vehicle" } else { "item" }, obj.display_name(), refs, val(obj, &[]))
                },
            );
        }
        for roof in level.roofs.values() {
            let r = EntityRef::RoofSection(roof.id);
            out.insert(
                r.key(),
                Record {
                    owner: Some(p.entity_name(lref)),
                    owner_ref: Some(lref),
                    ..rec("roof section", roof.display_name(), vec![r, EntityRef::Roof], val(roof, &[]))
                },
            );
        }
        for f in level.features.values() {
            let r = EntityRef::Feature(f.id);
            out.insert(
                r.key(),
                Record {
                    owner: Some("outside".into()),
                    owner_ref: Some(lref),
                    ..rec(&f.kind_label().to_lowercase(), f.display_name(), vec![r], val(f, &[]))
                },
            );
        }
    }

    for pl in &p.plantings {
        let r = EntityRef::Planting(pl.id);
        let garden = EntityRef::Feature(pl.garden);
        out.insert(
            r.key(),
            Record {
                owner: Some(p.entity_name(garden)),
                owner_ref: Some(garden),
                ..rec("planting", pl.display_name(), vec![r, garden], val(pl, &[]))
            },
        );
    }
    // Seed packets go with the owner (see sharing.rs), so their history does too.
    for s in &p.seeds {
        out.insert(
            format!("seed:{}", s.id),
            Record { private: true, ..rec("seed packet", s.display_name(), Vec::new(), val(s, &[])) },
        );
    }

    for pet in &p.pets {
        let r = EntityRef::Pet(pet.id);
        out.insert(r.key(), Record { private: true, ..rec("pet", pet.display_name(), vec![r], val(pet, &[])) });
    }

    for b in &p.buildings {
        let r = EntityRef::Building(b.id);
        out.insert(r.key(), rec("building", b.display_name(), vec![r], val(b, &[])));
    }

    for n in &p.notes {
        out.insert(
            format!("note:{}", n.id),
            Record {
                owner: Some(p.entity_name(n.target)),
                owner_ref: Some(n.target),
                private: n.private || about_car(&n.target),
                ..rec("note", n.title.clone(), vec![n.target], val(n, &["created", "updated"]))
            },
        );
    }
    for s in &p.service_records {
        let name = if s.description.trim().is_empty() {
            s.kind_label()
        } else {
            short(&s.description)
        };
        out.insert(
            format!("service:{}", s.id),
            Record {
                owner: Some(p.entity_name(s.target)),
                owner_ref: Some(s.target),
                private: s.private || about_car(&s.target),
                ..rec("service record", name, vec![s.target], val(s, &[]))
            },
        );
    }
    for r in &p.reminders {
        out.insert(
            format!("reminder:{}", r.id),
            Record {
                owner: Some(p.entity_name(r.target)),
                owner_ref: Some(r.target),
                private: about_car(&r.target),
                ..rec("reminder", r.title.clone(), vec![r.target], val(r, &[]))
            },
        );
    }
    for paint in &p.paints {
        let target = paint.target.entity();
        let name = if paint.color_name.trim().is_empty() {
            paint.brand.clone()
        } else {
            paint.color_name.clone()
        };
        let mut refs = vec![target];
        if let PaintTarget::WallFace { room: Some(r), .. } = paint.target {
            refs.push(EntityRef::Room(r));
        }
        out.insert(
            format!("paint:{}", paint.id),
            Record {
                owner: Some(p.entity_name(target)),
                owner_ref: Some(target),
                ..rec("paint", name, refs, val(paint, &[]))
            },
        );
    }
    out
}

fn humanize(key: &str) -> String {
    match key {
        "area_m2" => "area".into(),
        "kind" => "type".into(),
        k => k.replace('_', " "),
    }
}

fn changed_fields(a: &Value, b: &Value) -> Vec<String> {
    let (Value::Object(ma), Value::Object(mb)) = (a, b) else {
        return vec!["value".into()];
    };
    let mut keys: Vec<&String> = ma.keys().chain(mb.keys()).collect();
    keys.sort();
    keys.dedup();
    let mut out = Vec::new();
    for k in keys {
        let (va, vb) = (ma.get(k), mb.get(k));
        if va == vb {
            continue;
        }
        match (va, vb) {
            (Some(x @ Value::Object(_)), Some(y @ Value::Object(_))) if k == "details" => {
                out.extend(changed_fields(x, y));
            }
            _ => out.push(humanize(k)),
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityChange {
    pub key: String,
    pub kind: ChangeKind,
    pub subject: String,
    pub owner: Option<String>,
    pub refs: Vec<EntityRef>,
    pub fields: Vec<String>,
    pub private: bool,
}

fn subject(kind: &str, name: &str) -> String {
    if name.trim().is_empty() {
        kind.to_string()
    } else {
        format!("{kind} “{}”", name.trim())
    }
}

fn to_change(key: &str, kind: ChangeKind, r: &Record, fields: Vec<String>) -> EntityChange {
    EntityChange {
        key: key.to_string(),
        kind,
        subject: subject(&r.kind, &r.name),
        owner: r.owner.clone(),
        refs: r.refs.clone(),
        fields,
        private: r.private,
    }
}

/// Everything that differs between two versions of a project.
pub fn diff(old: &Project, new: &Project) -> Vec<EntityChange> {
    let a = collect(old);
    let b = collect(new);
    let mut out = Vec::new();
    for (k, rb) in &b {
        match a.get(k) {
            None if rb.log_lifecycle => out.push(to_change(k, ChangeKind::Added, rb, vec![])),
            None => {}
            Some(ra) if ra.value != rb.value => {
                let fields = changed_fields(&ra.value, &rb.value);
                if !fields.is_empty() {
                    out.push(to_change(k, ChangeKind::Modified, rb, fields));
                }
            }
            Some(_) => {}
        }
    }
    for (k, ra) in &a {
        if b.contains_key(k) || !ra.log_lifecycle {
            continue;
        }
        if let Some(owner) = ra.owner_ref {
            if !new.exists(owner) {
                continue; // covered by the owner's own "Removed" entry
            }
        }
        out.push(to_change(k, ChangeKind::Removed, ra, vec![]));
    }
    out
}

pub fn compose_summary(kind: ChangeKind, subject: &str, owner: Option<&str>, fields: &[String]) -> String {
    match kind {
        ChangeKind::Added => match owner {
            Some(o) => format!("Added {subject} to {o}"),
            None => format!("Added {subject}"),
        },
        ChangeKind::Removed => match owner {
            Some(o) => format!("Removed {subject} from {o}"),
            None => format!("Removed {subject}"),
        },
        ChangeKind::Modified => {
            let on = owner.map(|o| format!(" on {o}")).unwrap_or_default();
            if fields.is_empty() {
                format!("Updated {subject}{on}")
            } else {
                format!("Updated {subject}{on}: {}", fields.join(", "))
            }
        }
        ChangeKind::Manual => subject.to_string(),
    }
}

/// Diff `old` against `project` and append (or merge) entries into
/// `project.changelog`. Returns how many changes were found.
pub fn record_changes(project: &mut Project, old: &Project, author: &str, now: DateTime<Utc>) -> usize {
    let changes = diff(old, project);
    let count = changes.len();
    let window = Duration::minutes(MERGE_MINUTES);
    for ch in changes {
        let len = project.changelog.len();
        let start = len.saturating_sub(12);
        let found = (start..len)
            .rev()
            .find(|&i| project.changelog[i].record.as_deref() == Some(ch.key.as_str()));
        if let Some(i) = found {
            let e = &mut project.changelog[i];
            let mergeable = ch.kind == ChangeKind::Modified
                && matches!(e.kind, ChangeKind::Added | ChangeKind::Modified)
                && e.author == author
                && now - e.timestamp <= window;
            if mergeable {
                if e.kind == ChangeKind::Modified {
                    for f in ch.fields {
                        if !e.fields.contains(&f) {
                            e.fields.push(f);
                        }
                    }
                }
                e.subject = ch.subject;
                e.owner = ch.owner;
                e.private |= ch.private;
                e.timestamp = now;
                e.summary = compose_summary(e.kind, &e.subject, e.owner.as_deref(), &e.fields);
                continue;
            }
        }
        project.changelog.push(ChangeEntry {
            id: ChangeId::new(),
            timestamp: now,
            occurred: None,
            author: author.to_string(),
            kind: ch.kind,
            summary: compose_summary(ch.kind, &ch.subject, ch.owner.as_deref(), &ch.fields),
            details: String::new(),
            targets: ch.refs,
            record: Some(ch.key),
            subject: ch.subject,
            owner: ch.owner,
            fields: ch.fields,
            private: ch.private,
        });
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Vec2, rect_points};

    #[test]
    fn logs_add_then_merges_rename() {
        let mut p = Project::new("T");
        let l = p.levels[0].id;
        let old = p.clone();
        let room = p.add_room(l, "Room 1", &rect_points(Vec2::ZERO, 3.0, 3.0)).unwrap();
        let now = Utc::now();
        record_changes(&mut p, &old, "me", now);
        assert_eq!(p.changelog.len(), 1);
        assert_eq!(p.changelog[0].summary, "Added room “Room 1” to Main floor");

        let old = p.clone();
        p.levels[0].rooms.get_mut(&room).unwrap().name = "Kitchen".into();
        record_changes(&mut p, &old, "me", now + Duration::minutes(1));
        assert_eq!(p.changelog.len(), 1, "rename right after adding is merged");
        assert_eq!(p.changelog[0].summary, "Added room “Kitchen” to Main floor");
    }

    #[test]
    fn logs_field_changes_and_respects_window() {
        let mut p = Project::new("T");
        let l = p.levels[0].id;
        p.add_room(l, "Utility", &rect_points(Vec2::ZERO, 3.0, 3.0)).unwrap();
        let obj = p
            .add_object(l, "Furnace", ObjectCategory::Hvac, Shape::new(ShapeKind::Rectangle, 0.5, 0.7), Vec2::new(1.0, 1.0))
            .unwrap();
        let t0 = Utc::now();
        let old = p.clone();
        p.levels[0].objects.get_mut(&obj).unwrap().details.model = "X1".into();
        record_changes(&mut p, &old, "me", t0);
        let old = p.clone();
        p.levels[0].objects.get_mut(&obj).unwrap().details.serial = "123".into();
        record_changes(&mut p, &old, "me", t0 + Duration::minutes(2));
        assert_eq!(p.changelog.len(), 1);
        assert_eq!(p.changelog[0].summary, "Updated item “Furnace” on Utility: model, serial");

        let old = p.clone();
        p.levels[0].objects.get_mut(&obj).unwrap().details.vendor = "Shop".into();
        record_changes(&mut p, &old, "me", t0 + Duration::hours(2));
        assert_eq!(p.changelog.len(), 2, "outside the merge window");
    }

    #[test]
    fn private_notes_produce_private_entries() {
        let mut p = Project::new("T");
        let old = p.clone();
        let mut n = Note::new(EntityRef::General);
        n.title = "Alarm code".into();
        n.private = true;
        p.notes.push(n);
        record_changes(&mut p, &old, "me", Utc::now());
        assert!(p.changelog[0].private);
    }

    #[test]
    fn room_delete_does_not_spam_child_removals() {
        let mut p = Project::new("T");
        let l = p.levels[0].id;
        let room = p.add_room(l, "Den", &rect_points(Vec2::ZERO, 3.0, 3.0)).unwrap();
        p.notes.push(Note::new(EntityRef::Room(room)));
        let old = p.clone();
        p.delete_entity(EntityRef::Room(room));
        let changes = diff(&old, &p);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Removed);
    }
}

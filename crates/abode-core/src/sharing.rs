//! Preparing a copy of the house file for someone else (e.g. a buyer).

use crate::ids::*;
use crate::model::*;
use std::collections::BTreeSet;

fn strip_private(fields: &mut Vec<CustomField>) {
    fields.retain(|f| !f.private);
}

impl Project {
    /// A copy without anything marked private: private notes, private service
    /// records, private custom fields, and change-log entries about them.
    /// (Save-history snapshots are dropped separately by the storage layer.)
    pub fn sanitized_for_sharing(&self) -> Project {
        let mut p = self.clone();
        // Vehicles leave with their owners: drop them and everything about them.
        let cars: BTreeSet<EntityRef> = p.vehicles().into_iter().map(|(_, id)| EntityRef::Object(id)).collect();
        for l in &mut p.levels {
            l.objects.retain(|_, o| o.vehicle.is_none());
        }
        p.notes.retain(|n| !cars.contains(&n.target));
        p.service_records.retain(|s| !cars.contains(&s.target));
        p.reminders.retain(|r| !cars.contains(&r.target));
        p.changelog.retain(|c| !c.targets.iter().any(|t| cars.contains(t)));
        p.notes.retain(|n| !n.private);
        p.service_records.retain(|s| !s.private);
        p.changelog.retain(|c| !c.private);
        strip_private(&mut p.info.custom_fields);
        strip_private(&mut p.roof.custom_fields);
        for b in &mut p.buildings {
            strip_private(&mut b.details.custom_fields);
        }
        for level in &mut p.levels {
            for w in level.walls.values_mut() {
                strip_private(&mut w.custom_fields);
            }
            for r in level.rooms.values_mut() {
                strip_private(&mut r.custom_fields);
            }
            for o in level.openings.values_mut() {
                strip_private(&mut o.details.custom_fields);
            }
            for o in level.objects.values_mut() {
                strip_private(&mut o.details.custom_fields);
            }
            for r in level.roofs.values_mut() {
                strip_private(&mut r.custom_fields);
            }
            for f in level.features.values_mut() {
                strip_private(&mut f.details.custom_fields);
            }
        }
        p
    }

    /// Every attachment something in the project still points at.
    pub fn referenced_attachments(&self) -> BTreeSet<AttachmentId> {
        let mut out = BTreeSet::new();
        for n in &self.notes {
            out.extend(n.attachments.iter().copied());
        }
        for s in &self.service_records {
            out.extend(s.attachments.iter().copied());
        }
        for l in &self.levels {
            if let Some(u) = &l.underlay {
                out.insert(u.attachment);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_private_things() {
        let mut p = Project::new("T");
        let mut public = Note::new(EntityRef::General);
        public.attachments.push(AttachmentId::new());
        let mut secret = Note::new(EntityRef::General);
        secret.private = true;
        let secret_att = AttachmentId::new();
        secret.attachments.push(secret_att);
        p.notes.push(public);
        p.notes.push(secret);
        p.info.custom_fields.push(CustomField { key: "Alarm".into(), value: "1234".into(), private: true });
        p.info.custom_fields.push(CustomField { key: "Lot".into(), value: "12".into(), private: false });

        let s = p.sanitized_for_sharing();
        assert_eq!(s.notes.len(), 1);
        assert_eq!(s.info.custom_fields.len(), 1);
        assert!(!s.referenced_attachments().contains(&secret_att));
        assert!(p.referenced_attachments().contains(&secret_att));
    }

    #[test]
    fn vehicles_are_left_out() {
        let mut p = Project::new("T");
        let site = p.site_id().unwrap();
        let shape = Shape::new(ShapeKind::Rectangle, 1.8, 4.6);
        let car = p.add_object(site, "Car", ObjectCategory::Vehicle, shape, crate::Vec2::new(0.0, 0.0)).unwrap();
        let bbq = p.add_object(site, "BBQ", ObjectCategory::Outdoor, shape, crate::Vec2::new(9.0, 9.0)).unwrap();
        p.notes.push(Note::new(EntityRef::Object(car)));
        p.reminders.push(Reminder::new(EntityRef::Object(car), "Oil change", chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()));
        let s = p.sanitized_for_sharing();
        assert!(s.find_object(car).is_none());
        assert!(s.find_object(bbq).is_some());
        assert!(s.notes.is_empty() && s.reminders.is_empty());
    }
}

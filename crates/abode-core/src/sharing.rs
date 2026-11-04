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
}

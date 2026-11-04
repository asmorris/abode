//! Strongly-typed identifiers.
//!
//! Every entity gets a UUIDv7 (time ordered, globally unique). That makes
//! files mergeable/syncable later without id collisions, and the separate
//! types stop us mixing up e.g. a `RoomId` with a `WallId` at compile time.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_ids {
    ($($name:ident),* $(,)?) => {$(
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            pub fn parse(s: &str) -> Option<Self> {
                Uuid::parse_str(s).ok().map(Self)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    )*};
}

define_ids!(
    ProjectId,
    LevelId,
    NodeId,
    WallId,
    RoomId,
    OpeningId,
    ObjectId,
    NoteId,
    AttachmentId,
    ServiceId,
    ReminderId,
    PaintId,
    ChangeId,
    RevisionId,
    BuildingId,
    RoofId,
    FeatureId,
    GateId,
);

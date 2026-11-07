//! Core of Abode: the house model, geometry, history and file format.
//!
//! Nothing in here knows about the UI, so it can be reused by a future web
//! viewer, mobile app or sync service.

pub mod blueprint;
pub mod diff;
pub mod export;
pub mod garden;
pub mod geometry;
pub mod ids;
pub mod model;
pub mod ops;
pub mod pet;
pub mod reminders;
pub mod roof;
pub mod sample;
pub mod sharing;
pub mod stairs;
pub mod site;
pub mod storage;
pub mod topology;
pub mod units;
pub mod vehicle;

pub use garden::{GardenSettings, Harvest, Layout, Method, Planting, SeedPacket};
pub use geometry::Vec2;
pub use ids::*;
pub use model::*;
pub use storage::Store;
pub use topology::TopologyError;
pub use units::UnitSystem;
pub use vehicle::{Vehicle, VehiclePart, VehicleWarranty, TireSet};

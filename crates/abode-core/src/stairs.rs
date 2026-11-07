//! Stairs: an item that joins one floor to the next. It climbs along its
//! depth (local +y), from the end at -y to the end at +y.

use crate::geometry::Vec2;
use crate::ids::*;
use crate::model::*;
use serde::{Deserialize, Serialize};

/// A comfortable step: about 18 cm up, 25 cm deep.
pub const STEP_RISE: f64 = 0.18;
pub const STEP_RUN: f64 = 0.25;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StairDirection {
    /// To the floor above.
    #[default]
    Up,
    /// To the floor below (basement stairs, say).
    Down,
}

impl StairDirection {
    pub fn label(self) -> &'static str {
        match self {
            StairDirection::Up => "Up",
            StairDirection::Down => "Down",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stairs {
    pub direction: StairDirection,
    /// Number of steps (blank: worked out from the height to climb).
    pub steps: Option<u32>,
}

pub fn is_stairs_label(label: &str) -> bool {
    label.to_lowercase().contains("stair")
}

/// Stairs as seen from a floor: its own, or ones arriving from the floor
/// above or below.
pub struct StairInfo {
    pub steps: u32,
    /// Total height climbed (m), when it's known.
    pub rise: Option<f64>,
    /// The floor at the other end.
    pub to: Option<LevelId>,
}

impl PlacedObject {
    pub fn is_stairs(&self) -> bool {
        self.stairs.is_some()
    }

    /// Where the climb starts and ends (world), for drawing the arrow.
    pub fn stair_line(&self) -> (Vec2, Vec2) {
        let d = self.shape.depth / 2.0;
        (self.position + Vec2::new(0.0, -d).rotated(self.rotation), self.position + Vec2::new(0.0, d).rotated(self.rotation))
    }

    /// Each step's outline (world), from the bottom of the climb.
    pub fn step_outlines(&self, steps: u32) -> Vec<Vec<Vec2>> {
        let n = steps.max(1);
        let (w, d) = (self.shape.width / 2.0, self.shape.depth);
        (0..n)
            .map(|i| {
                let y0 = -d / 2.0 + d * i as f64 / n as f64;
                let y1 = -d / 2.0 + d * (i + 1) as f64 / n as f64;
                [Vec2::new(-w, y0), Vec2::new(w, y0), Vec2::new(w, y1), Vec2::new(-w, y1)]
                    .into_iter()
                    .map(|p| self.position + p.rotated(self.rotation))
                    .collect()
            })
            .collect()
    }
}

impl Project {
    /// Steps, height and destination for stairs on `level`.
    pub fn stair_info(&self, level: LevelId, obj: &PlacedObject) -> Option<StairInfo> {
        let s = obj.stairs.as_ref()?;
        let l = self.level(level)?;
        let (below, above) = self.neighbour_levels(level);
        let to = match s.direction {
            StairDirection::Up => above,
            StairDirection::Down => below,
        };
        let rise = match (to, obj.feature) {
            (Some(t), _) => Some((t.elevation - l.elevation).abs()),
            // Outside: up onto the deck it's next to (or on).
            (None, Some(f)) => l.features.get(&f).map(|f| f.height).filter(|h| *h > 0.05),
            (None, None) => None,
        };
        let steps = s.steps.unwrap_or_else(|| match rise {
            Some(r) => ((r / STEP_RISE).round() as u32).max(1),
            None => ((obj.shape.depth / STEP_RUN).round() as u32).max(1),
        });
        Some(StairInfo { steps, rise, to: to.map(|t| t.id) })
    }

    /// Stairs on the floors next to `level` that lead to it: (their floor,
    /// the stairs).
    pub fn stairs_arriving(&self, level: LevelId) -> Vec<(&Level, &PlacedObject)> {
        let (below, above) = self.neighbour_levels(level);
        let mut out = Vec::new();
        for (l, dir) in [(below, StairDirection::Up), (above, StairDirection::Down)] {
            let Some(l) = l else { continue };
            for o in l.objects.values().filter(|o| o.stairs.as_ref().is_some_and(|s| s.direction == dir)) {
                out.push((l, o));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::rect_points;

    #[test]
    fn stairs_join_floors() {
        let mut p = Project::new("T");
        let main = p.levels[0].id;
        p.add_room(main, "Hall", &rect_points(Vec2::ZERO, 4.0, 4.0)).unwrap();
        let up = p.add_level(None, "Upper");
        let shape = Shape::new(ShapeKind::Rectangle, 0.9, 3.0);
        let id = p.add_object(main, "Stairs", ObjectCategory::Fixture, shape, Vec2::new(1.0, 2.0)).unwrap();
        let (_, o) = p.find_object(id).unwrap();
        assert!(o.is_stairs());
        let info = p.stair_info(main, o).unwrap();
        assert_eq!(info.to, Some(up));
        // 2.44 m walls + floor: about 15 steps of 18 cm.
        let rise = info.rise.unwrap();
        assert_eq!(info.steps, (rise / STEP_RISE).round() as u32);
        assert_eq!(o.step_outlines(info.steps).len(), info.steps as usize);
        assert_eq!(p.stairs_arriving(up).len(), 1);
        assert!(p.stairs_arriving(main).is_empty());
        let (a, b) = o.stair_line();
        assert!((a.distance(b) - 3.0).abs() < 1e-9);
    }
}


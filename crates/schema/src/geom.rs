//! Geometry carried on the wire.
//!
//! These types are plain data. The simulation may use a faster vector library
//! internally, but anything that crosses the wire converts to these first.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A point or a vector in world coordinates. Origin sits at the top-left corner of
/// the arena, x grows east, y grows south.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn distance_squared(self, other: Vec2) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    pub fn distance(self, other: Vec2) -> f32 {
        self.distance_squared(other).sqrt()
    }
}

/// A region of the arena. Spawn foci, objective zones, and belief patches all use
/// this. Bases are rectangles, the nest is a disc.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "shape", rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum Region {
    Disc {
        center: Vec2,
        radius: f32,
    },
    Rect {
        min: Vec2,
        max: Vec2,
    },
    Annulus {
        center: Vec2,
        inner: f32,
        outer: f32,
    },
    WholeArena,
}

impl Region {
    /// Point containment. `WholeArena` accepts every point; bounds checking against
    /// the arena square is the caller's job.
    pub fn contains(&self, p: Vec2, arena_side: f32) -> bool {
        match *self {
            Region::Disc { center, radius } => p.distance_squared(center) <= radius * radius,
            Region::Rect { min, max } => {
                p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y
            }
            Region::Annulus {
                center,
                inner,
                outer,
            } => {
                let d2 = p.distance_squared(center);
                d2 >= inner * inner && d2 <= outer * outer
            }
            Region::WholeArena => {
                p.x >= 0.0 && p.y >= 0.0 && p.x <= arena_side && p.y <= arena_side
            }
        }
    }
}

//! A focus: one place shapes come from, and the rules for that place.

use rand::rngs::StdRng;
use rand::RngExt;
use serde::{Deserialize, Serialize};

use schema::{FocusId, Region, ShapeTier, Vec2};

/// A shape the spawner wants to exist. The caller applies it; the spawner does not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpawnRequest {
    pub tier: ShapeTier,
    pub pos: Vec2,
    pub vel: Vec2,
    /// The focus that asked for it. Recorded on the entity and in the event log,
    /// which is what makes "which farming area did the team prioritise" a query
    /// rather than a guess.
    pub focus: FocusId,
}

/// What a focus counts when it asks whether it is full.
///
/// The distinction matters because shapes drift. A focus that counts by origin is
/// maintaining a *population*: the shapes it made, wherever they have wandered to.
/// A focus that counts residents is maintaining a *place*: the shapes it made that
/// are still standing in it.
///
/// The nest is a place. It is the richest ground on the map and is supposed to stay
/// that way, so a pentagon that drifts out must be replaced. Counting by origin
/// there produces a nest that reports itself full while the disc empties.
///
/// A third rule — count every shape standing in the region, whoever made it — was
/// tried and rejected. Drifting squares from the scatter focus wander through the
/// nest, count against its band, and starve pentagon respawn; the disc then fills
/// with the cheapest shapes on the map, which is the opposite of the intent.
/// Whether a place is physically full is the placement budget's question, not the
/// population band's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CountBy {
    /// Shapes carrying this focus identifier, anywhere in the arena.
    Origin,
    /// Shapes carrying this focus identifier that are still inside its region.
    Resident,
}

/// How a shape moves once it exists.
///
/// Drift is decided at spawn and never revisited. `sim-core` reflects a shape off
/// the arena wall and never re-steers it, so a velocity chosen here is a velocity
/// for the shape's whole life.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DriftModel {
    /// Sits still. Useful for making a run easier to read.
    Static,
    /// One direction drawn uniformly at spawn, held at this speed.
    Wander { speed: f32 },
}

impl DriftModel {
    pub fn velocity(&self, rng: &mut StdRng) -> Vec2 {
        match *self {
            DriftModel::Static => Vec2::ZERO,
            DriftModel::Wander { speed } => {
                let a = rng.random_range(0.0..std::f32::consts::TAU);
                Vec2::new(speed * a.cos(), speed * a.sin())
            }
        }
    }
}

/// One farming area.
///
/// `num_slow` and `num_max` are the per-focus population band. `docs/DESIGN.md`
/// calls the upper bound `capacity`; it is named for the constant it defaults to so
/// that the two are recognisably the same knob. Both counts are of shapes alive
/// from *this* focus, never of shapes in the arena, so one area filling up does not
/// suppress spawning in another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Focus {
    pub id: FocusId,
    /// Where shapes may appear. Sampled uniformly by area.
    pub region: Region,
    /// Regions inside `region` that are nevertheless off limits. The uniform
    /// scatter focus excludes the nest this way, which is what keeps pentagons a
    /// reason to go to the centre.
    pub exclude: Vec<Region>,
    /// Relative weights. Drawn from in configuration order; weights need not sum
    /// to one.
    pub tier_weights: Vec<(ShapeTier, f32)>,
    /// Alive count at which the respawn rate starts falling.
    pub num_slow: usize,
    /// Alive count at which respawning stops.
    pub num_max: usize,
    /// Whether the population band counts shapes made here or shapes standing
    /// here. See [`CountBy`].
    pub count_by: CountBy,
    /// Shapes per second at full rate.
    pub respawn_per_sec: f32,
    /// How many to place at match start, before the first tick. An arena that
    /// starts empty and fills over three minutes is a different experiment from
    /// one that starts populated, and the populated one is the intended baseline.
    pub prefill: usize,
    pub drift: DriftModel,
    /// Off means the focus contributes nothing. Wings ship disabled.
    pub enabled: bool,
}

impl Focus {
    /// The fraction of `respawn_per_sec` this focus is currently entitled to.
    ///
    /// One at or below `num_slow`, zero at or above `num_max`, and a straight line
    /// between them. The ramp is what turns a filling area into a diminishing
    /// return, so exploring elsewhere starts to pay before the area is full.
    pub fn throttle(&self, alive: usize) -> f32 {
        if alive >= self.num_max {
            return 0.0;
        }
        if alive <= self.num_slow || self.num_max <= self.num_slow {
            return 1.0;
        }
        (self.num_max - alive) as f32 / (self.num_max - self.num_slow) as f32
    }

    /// Draw a tier. Returns `None` only if the weights are empty or all
    /// non-positive, which is a configuration error rather than a runtime one.
    pub fn draw_tier(&self, rng: &mut StdRng) -> Option<ShapeTier> {
        let total: f32 = self.tier_weights.iter().map(|(_, w)| w.max(0.0)).sum();
        if total <= 0.0 {
            return None;
        }
        let mut roll = rng.random_range(0.0..total);
        for (tier, w) in &self.tier_weights {
            let w = w.max(0.0);
            if roll < w {
                return Some(*tier);
            }
            roll -= w;
        }
        // Floating point can walk off the end of the last bucket. Take it.
        self.tier_weights.last().map(|(t, _)| *t)
    }
}

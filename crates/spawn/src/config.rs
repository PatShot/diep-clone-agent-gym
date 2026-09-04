//! The default arrangement of foci.
//!
//! Plain typed structures, no file input. `config/spawn.toml` and its loader land
//! with the crate that has a binary to load them; until then [`SpawnConfig`] is the
//! configuration and [`SpawnConfig::default`] is the shipped arrangement.

use serde::{Deserialize, Serialize};

use schema::{FocusId, Region, ShapeTier, Vec2};
use sim_core::constants::{
    ARENA_SIDE, NEST_ALPHA_RATE, NEST_MINOR_RATE_MULT, NEST_PENTAGON_RATE_MULT, NEST_RADIUS,
    SHAPE_DRIFT_SPEED, SHAPE_NUM_MAX, SHAPE_NUM_SLOW, SHAPE_SPAWN_BASE_KEEPOUT, SHAPE_SPAWN_RATE,
    WING_SPAWN_RATE,
};

use crate::focus::{CountBy, DriftModel, Focus};

/// Identifiers for the shipped foci. Stable, because they are written into the
/// event log and a recorded match is read back by them.
pub const FOCUS_SCATTER: FocusId = FocusId(0);
pub const FOCUS_NEST: FocusId = FocusId(1);
pub const FOCUS_WING_NE: FocusId = FocusId(2);
pub const FOCUS_WING_SW: FocusId = FocusId(3);
/// Alpha pentagons. A focus of its own, sharing the nest disc, because a focus
/// carries one population band and the alphas need their own: four of them against
/// the nest's forty-five, at a twentieth of the rate.
pub const FOCUS_NEST_ALPHA: FocusId = FocusId(4);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnConfig {
    /// Walked in order every tick. Order is part of the determinism contract.
    pub foci: Vec<Focus>,
    /// How far from a base rectangle a shape must appear.
    pub base_keepout: f32,
    /// Rejection sampling budget per shape. Over budget, the focus spawns nothing
    /// this tick and keeps its credit for the next one.
    pub place_attempts: u32,
}

impl SpawnConfig {
    pub fn focus(&self, id: FocusId) -> Option<&Focus> {
        self.foci.iter().find(|f| f.id == id)
    }
}

/// Relative mix of the two common shapes, wherever both appear.
const SQUARE_SHARE: f32 = 0.85;
const TRIANGLE_SHARE: f32 = 0.15;

/// Alpha pentagons are placed only within this fraction of the nest radius.
///
/// Contact separation slowly evicts anything from a crowded region, and the arena
/// at `SHAPE_NUM_MAX` is crowded everywhere, so an alpha placed near the rim drifts
/// out of the nest within minutes. Starting them near the centre is the only lever
/// the spawner has; the real fix is physics that does not let a square shove a
/// four-hundred-health shape, and that is a `core` decision.
const ALPHA_PLACEMENT_INSET: f32 = 0.35;

/// The nest region, as `ArenaSpec` builds it. Repeated here so that a focus can
/// name it without reaching into a `World` that does not exist yet.
fn nest_region() -> Region {
    Region::Disc {
        center: Vec2::new(ARENA_SIDE * 0.5, ARENA_SIDE * 0.5),
        radius: NEST_RADIUS,
    }
}

impl Default for SpawnConfig {
    /// Uniform scatter, the nest, and two wings that ship disabled.
    ///
    /// The population bands differ per focus and that is deliberate. The scatter
    /// focus covers the whole arena and carries the constants literally, at 1200
    /// and 2000. The nest is bounded by its own geometry long before those
    /// numbers: a disc of radius 150 packs to about forty shapes, not two
    /// thousand. The ramp shape is the same everywhere; only the scale moves.
    ///
    /// The scatter focus does **not** exclude the nest. Scatter shapes fall inside
    /// the disc like anywhere else, and the nest focus piles its own on top, so
    /// the densest square of ground on the map is the centre. That is the point:
    /// the richest source of points is also the hardest place to move through.
    fn default() -> Self {
        let centre = Vec2::new(ARENA_SIDE * 0.5, ARENA_SIDE * 0.5);

        // The wings sit halfway from the centre toward the two corners the bases do
        // not occupy, which keeps them equidistant from both teams. Bases hold the
        // northwest and southeast corners, so the wings take northeast and
        // southwest.
        let wing_offset = ARENA_SIDE * 0.25;
        let wing_ne = Vec2::new(centre.x + wing_offset, centre.y - wing_offset);
        let wing_sw = Vec2::new(centre.x - wing_offset, centre.y + wing_offset);
        let wing_radius = 100.0;

        let scatter = Focus {
            id: FOCUS_SCATTER,
            region: Region::WholeArena,
            // Nothing is excluded. Scatter shapes may land in the nest, which is
            // what lets the centre become the densest ground on the map rather
            // than a clearing with pentagons in it. Pentagons remain the nest's
            // alone because no other focus draws that tier.
            exclude: Vec::new(),
            tier_weights: vec![
                (ShapeTier::Square, SQUARE_SHARE),
                (ShapeTier::Triangle, TRIANGLE_SHARE),
            ],
            num_slow: SHAPE_NUM_SLOW,
            num_max: SHAPE_NUM_MAX,
            // The scatter focus maintains a population, not a place. Its region is
            // the whole arena, so the two would mean nearly the same thing anyway.
            count_by: CountBy::Origin,
            respawn_per_sec: SHAPE_SPAWN_RATE,
            prefill: 600,
            drift: DriftModel::Wander {
                speed: SHAPE_DRIFT_SPEED,
            },
            enabled: true,
        };

        let nest = Focus {
            id: FOCUS_NEST,
            region: nest_region(),
            exclude: Vec::new(),
            // Weights are the rates. Pentagons at five times the baseline, the two
            // common shapes at a fifth of it, split in the same proportion they
            // take everywhere else. Dividing through by the focus rate below
            // recovers exactly those multiples.
            tier_weights: vec![
                (ShapeTier::Pentagon, NEST_PENTAGON_RATE_MULT),
                (ShapeTier::Square, NEST_MINOR_RATE_MULT * SQUARE_SHARE),
                (ShapeTier::Triangle, NEST_MINOR_RATE_MULT * TRIANGLE_SHARE),
            ],
            // Measured, not guessed: a radius 150 disc takes 39 pentagons before
            // rejection sampling starts failing more often than it succeeds, and
            // the smaller shapes fill the gaps between them. The band sits at that
            // edge so the nest stays packed and refills the instant it is farmed.
            num_slow: 30,
            num_max: 45,
            // The nest is a place. A pentagon that drifts out of the disc stops
            // counting and is replaced, so the centre stays packed instead of
            // slowly bleeding its shapes into the arena.
            count_by: CountBy::Resident,
            respawn_per_sec: SHAPE_SPAWN_RATE * (NEST_PENTAGON_RATE_MULT + NEST_MINOR_RATE_MULT),
            prefill: 45,
            // Pentagons do not drift. They are the map's fixed prize, and a
            // prize that wanders is not a place worth contesting. It also closes
            // a leak: drifting nest shapes escape into the arena, where nothing
            // caps them, and the total shape count then climbs without bound.
            // Squares and triangles still drift, and they are 98% of shapes, so
            // the stale-belief pressure `SHAPE_DRIFT_SPEED` exists for is intact.
            drift: DriftModel::Static,
            enabled: true,
        };

        let wing = |id: FocusId, at: Vec2| Focus {
            id,
            region: Region::Disc {
                center: at,
                radius: wing_radius,
            },
            exclude: Vec::new(),
            tier_weights: vec![(ShapeTier::Triangle, 0.7), (ShapeTier::Square, 0.3)],
            num_slow: 18,
            num_max: 30,
            // A wing is a place, for the same reason the nest is.
            count_by: CountBy::Resident,
            respawn_per_sec: WING_SPAWN_RATE,
            prefill: 20,
            drift: DriftModel::Wander {
                speed: SHAPE_DRIFT_SPEED,
            },
            // Off by default, as `docs/DESIGN.md` specifies. They exist so a
            // researcher can add a second and third contested area and watch how
            // team allocation changes.
            enabled: false,
        };

        // Alpha pentagons occupy the same disc as the nest focus and are counted
        // separately from it. Four alphas is 3200 points standing on 7% of the map,
        // which is what a team that holds the centre is holding it for.
        let alpha = Focus {
            id: FOCUS_NEST_ALPHA,
            region: nest_region(),
            // Placed in the inner third of the disc, counted across the whole of
            // it. `exclude` applies to placement only, which is what lets those two
            // differ. An alpha therefore starts far from the rim and has to be
            // shoved a long way before it stops being a nest shape. Measured over
            // thirty simulated minutes, this roughly halves how many escape.
            exclude: vec![Region::Annulus {
                center: Vec2::new(ARENA_SIDE * 0.5, ARENA_SIDE * 0.5),
                inner: NEST_RADIUS * ALPHA_PLACEMENT_INSET,
                outer: NEST_RADIUS,
            }],
            tier_weights: vec![(ShapeTier::AlphaPentagon, 1.0)],
            num_slow: 2,
            num_max: 4,
            count_by: CountBy::Resident,
            respawn_per_sec: NEST_ALPHA_RATE,
            prefill: 4,
            // Static, for the same reason pentagons are. An alpha that wandered out
            // of the nest would be the single most valuable thing on the map sitting
            // where nobody has to fight for it.
            drift: DriftModel::Static,
            enabled: true,
        };

        // Order is load-bearing, not cosmetic. Foci are walked in this order both
        // at prefill and every tick, and placement is first-come: whoever asks
        // first gets the ground. The alpha focus is confined to the inner third of
        // the nest, the smallest and most contested region on the map, so it asks
        // first. Behind the scatter focus it finds the centre already full and
        // places almost nothing.
        Self {
            foci: vec![
                alpha,
                nest,
                scatter,
                wing(FOCUS_WING_NE, wing_ne),
                wing(FOCUS_WING_SW, wing_sw),
            ],
            base_keepout: SHAPE_SPAWN_BASE_KEEPOUT,
            place_attempts: 32,
        }
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// What went wrong reading a spawn configuration.
#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(toml::de::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "reading spawn config: {e}"),
            ConfigError::Parse(e) => write!(f, "parsing spawn config: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io(e) => Some(e),
            ConfigError::Parse(e) => Some(e),
        }
    }
}

impl SpawnConfig {
    /// Parse a configuration from TOML.
    ///
    /// [`Self::default`] remains the authority on what the shipped values are;
    /// `config/spawn.toml` mirrors it, and a test asserts the two agree. That test
    /// is the reason this loader can be trusted: a file and a default that drift
    /// apart silently would be worse than having no file at all.
    pub fn from_toml_str(s: &str) -> Result<Self, ConfigError> {
        toml::from_str(s).map_err(ConfigError::Parse)
    }

    /// Read a configuration from a file.
    pub fn from_path(path: impl AsRef<std::path::Path>) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
        Self::from_toml_str(&text)
    }

    /// Render this configuration as TOML. Used to generate `config/spawn.toml` and
    /// to keep it honest.
    pub fn to_toml_string(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}

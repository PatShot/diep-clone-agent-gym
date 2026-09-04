//! Where a shape is allowed to appear, and how a candidate point is drawn.
//!
//! Placement is rejection sampling. Draw a point in the focus region, test it
//! against every rule, and try again on a rejection up to a fixed budget. The
//! budget is what keeps a saturated or badly configured focus from spinning: over
//! budget, the focus simply spawns nothing this tick and keeps its credit.

use rand::rngs::StdRng;
use rand::RngExt;
use schema::{EntityId, Region, Vec2};
use sim_core::arena::Rect;
use sim_core::{Grid, World};

/// Draw a point uniformly by area from a region, inset so that a circle of
/// `radius` centred there still fits inside the arena.
///
/// Uniform *by area* matters for discs and annuli: drawing the radius uniformly
/// would pile shapes at the centre, and the nest would grow a bald ring.
pub fn sample(region: &Region, arena_side: f32, radius: f32, rng: &mut StdRng) -> Vec2 {
    let lo = radius;
    let hi = (arena_side - radius).max(radius);
    match *region {
        Region::WholeArena => Vec2::new(rng.random_range(lo..hi), rng.random_range(lo..hi)),
        Region::Rect { min, max } => Vec2::new(
            rng.random_range(min.x.max(lo)..max.x.min(hi).max(min.x.max(lo))),
            rng.random_range(min.y.max(lo)..max.y.min(hi).max(min.y.max(lo))),
        ),
        Region::Disc { center, radius: r } => {
            let a = rng.random_range(0.0..std::f32::consts::TAU);
            let d = r * rng.random_range(0.0f32..1.0).sqrt();
            Vec2::new(center.x + d * a.cos(), center.y + d * a.sin())
        }
        Region::Annulus {
            center,
            inner,
            outer,
        } => {
            let a = rng.random_range(0.0..std::f32::consts::TAU);
            let u: f32 = rng.random_range(0.0f32..1.0);
            let d = (inner * inner + u * (outer * outer - inner * inner)).sqrt();
            Vec2::new(center.x + d * a.cos(), center.y + d * a.sin())
        }
    }
}

/// Distance from a point to the nearest edge of a rectangle. Zero inside it.
pub fn distance_to_rect(p: Vec2, r: &Rect) -> f32 {
    let dx = (r.min.x - p.x).max(p.x - r.max.x).max(0.0);
    let dy = (r.min.y - p.y).max(p.y - r.max.y).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// A spatial index of everything currently alive, rebuilt on the ticks that spawn.
///
/// Reuses the broadphase from `sim-core` rather than growing a second one. The
/// bucket allocations survive `rebuild`, so the cost of a spawning tick is a walk
/// over the entity table and nothing else.
pub struct Occupancy {
    grid: Grid,
    scratch: Vec<EntityId>,
    /// Shapes requested earlier in this same tick. They are not in the world yet,
    /// so the grid cannot know about them, and without this two requests could be
    /// placed on top of each other.
    pending: Vec<(Vec2, f32)>,
}

impl Occupancy {
    pub fn new(arena_side: f32) -> Self {
        Self {
            grid: Grid::new(arena_side),
            scratch: Vec::new(),
            pending: Vec::new(),
        }
    }

    /// Load every live entity. Called once on a tick that spawns, never otherwise.
    pub fn rebuild(&mut self, world: &World) {
        self.grid.clear();
        self.pending.clear();
        for (id, e) in world.iter_entities() {
            self.grid.insert(id, e.pos);
        }
    }

    /// Record a placement decided this tick.
    pub fn note(&mut self, pos: Vec2, radius: f32) {
        self.pending.push((pos, radius));
    }

    /// True when a circle of `radius` at `p` touches nothing alive and nothing
    /// already promised this tick.
    pub fn is_free(&mut self, world: &World, p: Vec2, radius: f32) -> bool {
        self.grid.query_radius(p, radius, &mut self.scratch);
        for &id in &self.scratch {
            let Some(e) = world.entity(id) else { continue };
            let reach = radius + e.radius;
            if p.distance_squared(e.pos) < reach * reach {
                return false;
            }
        }
        for &(q, r) in &self.pending {
            let reach = radius + r;
            if p.distance_squared(q) < reach * reach {
                return false;
            }
        }
        true
    }
}

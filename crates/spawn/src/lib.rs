//! Shape spawning.
//!
//! A spawner is a composition of weighted foci, not a monolith. Each focus owns a
//! region, a tier mix, a population band, and a drift model. Uniform scatter, the
//! nest, and the wings are three configurations of the same code.
//!
//! The spawner never touches world state. [`Spawner::tick`] takes the world by
//! shared reference and returns [`SpawnRequest`]s; the caller applies them, which
//! [`apply`] does in one line. That keeps the invariant that only the tick mutates
//! the world.
//!
//! Determinism holds the same way it does in `sim-core`: foci are walked in
//! configuration order, placement draws come from one generator the caller passes
//! in, and the per-focus rate accumulators are plain floating point arithmetic
//! performed in a fixed order.
//!
//! ```
//! use rand::{rngs::StdRng, SeedableRng};
//! use sim_core::{World, WorldSpec};
//! use spawn::{CompositeSpawner, SpawnConfig, Spawner};
//!
//! let mut world = World::new(WorldSpec::default());
//! let mut spawner = CompositeSpawner::new(SpawnConfig::default());
//! let mut rng = StdRng::seed_from_u64(7);
//!
//! let requests = spawner.tick(&world, world.tick(), &mut rng);
//! spawn::apply(&mut world, &requests);
//! ```

pub mod composite;
pub mod config;
pub mod focus;
pub mod place;

pub use composite::{CompositeSpawner, FocusCensus};
pub use config::SpawnConfig;
pub use focus::{CountBy, DriftModel, Focus, SpawnRequest};

use rand::rngs::StdRng;
use schema::{EntityId, Tick};
use sim_core::World;

/// What a spawner is.
///
/// `docs/DESIGN.md` writes the generator as `&mut Rng`. It is `&mut StdRng` here
/// because that is the concrete generator `sim-core` seeds from `WorldSpec::seed`,
/// and a type parameter would buy nothing a research sandbox uses.
pub trait Spawner: Send {
    /// Decide what should come into existence this tick.
    ///
    /// Called once per tick, before physics, alongside the command drain. Returning
    /// an empty vector is the common case.
    fn tick(&mut self, world: &World, now: Tick, rng: &mut StdRng) -> Vec<SpawnRequest>;
}

/// Apply spawn requests to the world.
///
/// A free function rather than a method on the trait, so that the boundary between
/// deciding and mutating stays visible at the call site.
pub fn apply(world: &mut World, requests: &[SpawnRequest]) -> Vec<EntityId> {
    requests
        .iter()
        .map(|r| world.spawn_shape(r.tier, r.pos, r.vel, Some(r.focus)))
        .collect()
}

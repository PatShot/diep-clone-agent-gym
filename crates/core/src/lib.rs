//! The headless simulation.
//!
//! Entities, fixed-step physics, circle collision, arena bounds, and the combat
//! that makes those worth having. No I/O, no sockets, no clock. `World::step`
//! advances one tick and appends to an event buffer the caller drains.
//!
//! Depends on `schema` and nothing else structural, so the wire contract stays the
//! one definition of what an event is.
//!
//! ```
//! use sim_core::{Inputs, World, WorldSpec};
//!
//! let mut world = World::new(WorldSpec::default());
//! world.step(&Inputs::default());
//! assert_eq!(world.tick().0, 1);
//! ```

pub mod arena;
pub mod bitmap;
pub mod constants;
pub mod entity;
pub mod grid;
pub mod store;
pub mod world;

pub use arena::{ArenaSpec, Rect};
pub use bitmap::BitMap;
pub use entity::{shape_drag, shape_mass, shape_radius, tank_max_hp, Entity};
pub use grid::Grid;
pub use store::{EntityKey, Store};
pub use world::{AgentState, Inputs, World, WorldSpec};

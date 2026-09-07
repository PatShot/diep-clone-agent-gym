//! Belief. What an agent holds instead of the world.
//!
//! `docs/DESIGN.md` §World Model: the representation is pluggable, and a quadtree,
//! a dense grid, a landmark graph and a particle set must all satisfy one trait.
//! [`WorldModel`] is that trait, verbatim, plus one method v0 needs.
//!
//! `ingest_scan` and `ingest_belief` are separate on purpose. A scan is your own
//! evidence. A belief is hearsay with an age and a source. A model that cannot tell
//! them apart lets teammates echo stale information back and forth and inflate each
//! other's confidence — rumour propagation — and the sandbox must be able to exhibit
//! that failure, which means the interface must permit avoiding it.
//!
//! v0 has no scan. `World::observe` hands a policy the entities inside its sense
//! radius, ground truth but local, and `ingest_visible` is the path for that. It
//! stays until the planar LIDAR lands at v0.5 and `ingest_scan` takes over.
//!
//! The first implementation is [`DenseGrid`]: a fixed grid of cells and a flat track
//! list. The design names the quadtree as its first implementation; the quadtree's
//! advantages are all in `encode` — graceful truncation for a byte budget — which is
//! comms work and deferred with it. The trait is what makes swapping it in a local
//! change.

pub mod grid;
pub mod tracks;

pub use grid::DenseGrid;
pub use tracks::{TrackEntry, TrackList};

use schema::{
    AgentId, BeliefMsg, CellState, Interest, Observation, Region, Scan, Tick, Track, Vec2,
};

/// The per-tank memory ceiling `docs/DESIGN.md` §Budgets starts at. The runtime
/// does not enforce it yet. Every model's tests assert against it.
pub const MEMORY_CEILING_BYTES: usize = 16 * 1024;

pub trait WorldModel: Send {
    /// Own sensor evidence. Nothing calls this until v0.5.
    fn ingest_scan(&mut self, scan: &Scan);

    /// v0's evidence: the entities inside sense radius, as `World::observe` gives
    /// them. Ground truth, but only locally.
    fn ingest_visible(&mut self, obs: &Observation);

    /// Hearsay. `from` is who said it; the payload carries when.
    fn ingest_belief(&mut self, msg: &BeliefMsg, from: AgentId);

    /// Advance the clock: age tracks, forget what has decayed past use, hold the
    /// memory ceiling.
    fn tick(&mut self, now: Tick);

    fn occupancy(&self, p: Vec2) -> CellState;

    /// Zero to one. How far to trust `occupancy(p)`.
    fn confidence(&self, p: Vec2) -> f32;

    /// Up to `k` places worth looking, nearest to `near` first.
    ///
    /// Ordered by ignorance: unknown cells on the edge of the known first, then known
    /// cells nobody has looked at lately. A prior over where the things worth finding
    /// probably are — a rescuer's last known addresses — would weight this ordering,
    /// and is the intended extension. The signature does not preclude it.
    fn frontiers(&self, near: Vec2, k: usize) -> Vec<Region>;

    /// Tracks whose last known position lies within `r` of `p`, nearest first, with
    /// their staleness attached. A track four seconds old with a 200-unit
    /// uncertainty disc is a hypothesis, not a target.
    fn tracks_near(&self, p: Vec2, r: f32) -> Vec<Track>;

    /// The most useful message that fits `budget` bytes on the wire, for a receiver
    /// who cares about `interest`. Where a world model earns or loses its keep.
    fn encode(&self, budget: usize, interest: Interest) -> BeliefMsg;

    /// Bytes held. What the per-tank ceiling is measured against.
    fn footprint(&self) -> usize;
}

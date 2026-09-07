//! Agents: the policy interface, scripted baselines, and the match runner.
//!
//! An agent never touches world state. It receives an [`Observation`] and returns
//! an [`Action`], and that pair — defined in `schema` so an out-of-process policy
//! sees exactly what an in-process one sees — is the whole contract. [`Policy`] is
//! the in-process form of it.
//!
//! The [`Runner`] is the match loop: spawner, observations, decisions, physics,
//! event bus, in that order, once per tick. It is the piece `record_match` did by
//! hand at step 4 of the build order, made reusable so every later step drives a
//! match the same way.
//!
//! Decisions run at [`DECISION_HZ`] and physics at `sim_core::constants::TICK_HZ`.
//! Between decision ticks an agent's last action is held, which is what makes a
//! replay of scripted policies mostly repeat markers.
//!
//! ```
//! use agent::{NearestShape, Runner, RunnerConfig};
//! use events::Bus;
//! use sim_core::constants::ARENA_SIDE;
//! use sim_core::WorldSpec;
//! use spawn::SpawnConfig;
//!
//! let mut runner = Runner::new(
//!     WorldSpec::default(),
//!     SpawnConfig::default(),
//!     Bus::new(),
//!     RunnerConfig::default(),
//! );
//! for (agent, _, is_cc) in runner.seats().collect::<Vec<_>>() {
//!     if !is_cc {
//!         runner.set_policy(agent, Box::new(NearestShape::new(ARENA_SIDE)));
//!     }
//! }
//! let summary = runner.run(25);
//! assert_eq!(summary.ticks, 25);
//! ```

pub mod baseline;
pub mod bridge;
pub mod doctrine;
pub(crate) mod geom;
pub mod model;
pub mod runner;

pub use baseline::{Idle, NearestKnownShape, NearestShape};
pub use bridge::{Link, LinkStats, SocketPolicy};
pub use doctrine::{Doctrine, DoctrinePolicy};
pub use model::{DenseGrid, WorldModel};
pub use runner::{MatchSummary, Runner, RunnerConfig};

use schema::{Action, Observation, Tick};

/// Decision rate in hertz. `docs/DESIGN.md` §Sensing: physics 25, decision 5.
pub const DECISION_HZ: u32 = 5;

/// What an agent is, from the simulation's side.
///
/// `decide` is called once per decision tick with everything the agent may know.
/// Any randomness a policy wants comes from a generator it owns, seeded from the
/// match seed and its own identity, so two runs on one seed decide identically. No
/// clock, no world, no other agent.
pub trait Policy: Send {
    fn decide(&mut self, obs: &Observation, now: Tick) -> Action;

    /// Bytes of memory the policy holds between decisions. The per-tank ceiling in
    /// `docs/DESIGN.md` §Budgets is enforced against this, later; for now the
    /// runner reports it. A policy that holds nothing reports nothing.
    fn footprint(&self) -> usize {
        0
    }

    /// The named mode the policy is in, if it has one. A discrete fact the runner
    /// counts, so "what was tank three doing when it died" is a query.
    fn stance(&self) -> Option<&str> {
        None
    }
}

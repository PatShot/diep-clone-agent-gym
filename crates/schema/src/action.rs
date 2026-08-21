//! The agent interface, as data.
//!
//! An agent receives an `Observation` and returns an `Action`. It never touches
//! world state. This pair is the whole contract, and it is defined in `schema` so
//! an out-of-process policy written in Python sees exactly what an in-process Rust
//! policy sees.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::belief::{Inbound, Outbound};
use crate::command::Command;
use crate::entity::{Class, EntityView, Stats};
use crate::geom::Vec2;
use crate::ids::{AgentId, EntityId, TeamId, Tick};
use crate::sense::Scan;

/// What an agent knows about itself. Exact, unlike everything else it holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct SelfView {
    pub agent: AgentId,
    pub team: TeamId,
    /// Absent while dead and awaiting respawn.
    pub entity: Option<EntityId>,
    pub pos: Vec2,
    pub vel: Vec2,
    pub heading: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub score: u32,
    pub level: u8,
    pub class: Option<Class>,
    pub stats: Stats,
    /// Unspent stat points.
    pub points: u8,
    pub sense_radius: f32,
    pub comms_radius: f32,
    pub reload_ready: bool,
    /// Tick this agent respawns on. Present only while dead.
    pub respawn_at: Option<Tick>,
}

/// Everything an agent gets for one decision. Nothing outside this struct is
/// visible to a policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct Observation {
    pub tick: Tick,
    pub own: SelfView,
    /// Entities inside sense radius this tick. Ground truth, but only locally.
    pub visible: Vec<EntityView>,
    /// Sensor sweep. Empty in v0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan: Option<Scan>,
    /// Belief messages delivered this tick. Each carries the tick it was sent, so
    /// the contents can be aged rather than trusted as current.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inbox: Vec<Inbound>,
    /// Commands from the control center [CC] that reached this agent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<Command>,
    /// Score for both teams, indexed by team identifier. Public information.
    pub scores: Vec<u32>,
}

/// Continuous control output. `thrust` is a direction and magnitude in world
/// coordinates, clamped to unit length by the simulation. `aim` is an absolute
/// heading in radians.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct Control {
    pub thrust: Vec2,
    pub aim: f32,
    pub fire: bool,
}

/// One-shot decisions an agent may attach to a tick. Both are irreversible.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "choice", rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum Choice {
    /// Commit to a class. Permitted once, at the level threshold.
    Class { class: Class },
    /// Spend one stat point.
    Stat { stat: StatKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum StatKind {
    Damage,
    Reload,
    Speed,
    Health,
}

/// What an agent returns. A control center [CC] leaves `control` at its default
/// and populates `commands`; a tank does the reverse.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct Action {
    pub control: Control,
    /// Messages handed to the comms broker. Ignored in v0.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub send: Vec<Outbound>,
    /// Commands issued. Only a CC agent may populate this.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<Command>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<Choice>,
}

impl Action {
    /// The action a tank takes when its policy misses its time budget: nothing new.
    pub fn idle() -> Self {
        Self::default()
    }
}

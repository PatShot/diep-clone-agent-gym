//! Commands. The control center's [CC] output, and the viewer's only upward channel.
//!
//! A human operating a CC and an automated CC policy emit this same type. That is
//! what makes human-in-the-loop free and lets a researcher swap a person for an
//! algorithm and compare the runs directly.
//!
//! A command is subject to the comms rules like any other message. A human at the
//! CC cannot reach a tank the CC cannot reach.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::geom::{Region, Vec2};
use crate::ids::{AgentId, EntityId};

/// Who issued a command. Recorded so a replay distinguishes human decisions from
/// policy decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum Origin {
    Human,
    Policy,
}

/// A role a CC can assign. The set is deliberately small. Role entropy across a
/// team is one of the metrics, and it only means something if the vocabulary is
/// fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum Role {
    /// Destroy shapes for points.
    Farm,
    /// Protect a farming teammate.
    Screen,
    /// Gather information, avoid engagements.
    Scout,
    /// Hold position between two clusters to carry messages.
    Relay,
    /// Attack enemy tanks.
    Push,
    /// Hold the area around the friendly base.
    Defend,
    /// Return toward the base sanctuary.
    Regroup,
}

/// A coarse statement of team priority. Deliberately vague: it constrains without
/// dictating, which leaves the tanks something to work out.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "objective", rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum ObjectiveHint {
    /// Contest the nest.
    HoldNest,
    /// Farm where the enemy is not.
    FarmSafe,
    /// Attack toward the enemy base.
    Pressure,
    /// Hold the friendly half.
    Defend,
    /// Concentrate somewhere.
    Rally { pos: Vec2 },
    /// Look here.
    Explore { region: Region },
}

/// Everything a CC can say. Nothing here touches world state directly; a command
/// is a request delivered to an agent, and the agent's policy decides what to do
/// with it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "cmd", rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum Command {
    SetObjective { hint: ObjectiveHint },
    AssignRole { agent: AgentId, role: Role },
    DesignateTarget { agent: AgentId, target: EntityId },
    SetRally { agent: AgentId, pos: Vec2 },
    RequestReport { agent: AgentId },
}

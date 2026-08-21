//! Belief messages. What one agent tells another.
//!
//! An agent never sees world state. It holds a belief, and the gap between belief
//! and truth is the thing under study. These types are the currency of that gap.
//!
//! The union carries a small set of standard payloads plus a `Raw` escape hatch.
//! Standard payloads let two teams running different world models still talk, which
//! keeps mixed-model experiments available. `Raw` lets a researcher invent a
//! representation without touching the schema.
//!
//! Nothing here is emitted in v0. The comms broker arrives later. The types exist
//! now so the wire contract does not move under a client that already depends on it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::geom::{Region, Vec2};
use crate::ids::{AgentId, EntityId, TeamId, Tick};

/// Who a message is addressed to. Range rules still apply; addressing a team does
/// not reach a teammate outside comms radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "to", rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum Recipient {
    Agent {
        agent: AgentId,
    },
    Team {
        team: TeamId,
    },
    /// Everyone in range, including enemies who can receive it.
    Broadcast,
}

/// Occupancy state of a region as one agent believes it to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum CellState {
    Unknown,
    Free,
    Occupied,
}

/// One node of a truncated occupancy tree. Depth is the bandwidth dial: cut the
/// tree shallow for a coarse map of everywhere, deep for detail somewhere.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct MapCell {
    pub region: Region,
    pub state: CellState,
    /// When this cell was last observed. Oldest of its children when aggregated.
    pub updated: Tick,
    /// Zero to one. How much the sender trusts this cell.
    pub confidence: f32,
}

/// A hypothesis about a moving entity, not a target.
///
/// `uncertainty` grows with age at the maximum speed the entity could manage. A
/// four second old track with a 200 unit disc is worth re-confirming, not shooting
/// at. Whether an agent understands that difference is the behaviour under study.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct Track {
    /// Absent when the observer could not identify which entity it saw.
    pub id: Option<EntityId>,
    pub team: Option<TeamId>,
    pub last_pos: Vec2,
    pub vel_estimate: Vec2,
    pub last_seen: Tick,
    /// Radius of the disc the entity is believed to be inside.
    pub uncertainty: f32,
}

/// What an agent intends to do. Cheap to send and the highest value per byte on a
/// capped link, because it lets a teammate plan against it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "intent", rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum Intent {
    MovingTo { pos: Vec2, eta: Tick },
    Engaging { target: EntityId },
    Farming { region: Region },
    Relaying { between: [AgentId; 2] },
    Retreating { to: Vec2 },
}

/// What a receiver cares about. Passed to `WorldModel::encode` so the sender can
/// spend its byte budget where it will be read.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "interest", rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum Interest {
    /// Detail near a point, coarse elsewhere.
    Near { pos: Vec2, radius: f32 },
    /// Enemy positions above all.
    Threats,
    /// Where nobody has looked recently.
    Frontiers,
    /// No preference stated.
    Any,
}

/// A message between agents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "msg", rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum BeliefMsg {
    /// A truncated slice of an occupancy model.
    MapPatch { cells: Vec<MapCell> },
    /// Enemy and teammate tracks with their staleness attached.
    TrackSet { tracks: Vec<Track> },
    /// What the sender plans to do.
    Intent { intent: Intent },
    /// Ask for what the sender lacks.
    Request { interest: Interest },
    /// Anything else. Only a matching world model can decode it.
    Raw {
        codec: String,
        #[serde(with = "crate::b64")]
        #[ts(type = "string")]
        bytes: Vec<u8>,
    },
}

/// A belief message an agent hands to the broker. The broker decides whether it
/// arrives, when, and whether it fits the bandwidth cap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct Outbound {
    pub to: Recipient,
    pub payload: BeliefMsg,
}

/// A belief message that arrived. `sent` is the tick it left the sender, so the
/// receiver can age the contents rather than treating them as current.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct Inbound {
    pub from: AgentId,
    pub sent: Tick,
    pub payload: BeliefMsg,
}

/// One agent's belief, rendered for the viewer's belief mode. Optional on every
/// frame and off by default, because it is large.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct BeliefOverlay {
    pub agent: AgentId,
    pub cells: Vec<MapCell>,
    pub tracks: Vec<Track>,
    /// Agents this agent can currently reach. The comms graph, per node.
    pub links: Vec<AgentId>,
}

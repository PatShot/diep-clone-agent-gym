//! The wire protocol.
//!
//! WebSocket carrying newline-delimited JSON [NDJSON]. One message per line. Binary
//! comes later, and later than you expect.
//!
//! Server to client: a `Snapshot` first, then per-tick frames. v0 sends a full
//! snapshot every tick because full state is trivial at ten tanks. `Delta` exists
//! now so switching is a server change, not a schema change.
//!
//! Client to server: `Command` and view control. The viewer has no other authority.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::belief::BeliefOverlay;
use crate::command::Command;
use crate::entity::{EntityDelta, EntityView};
use crate::event::Event;
use crate::geom::{Region, Vec2};
use crate::ids::{AgentId, EntityId, TeamId, Tick};

/// Protocol version. Bump on any breaking change to the types in this crate. The
/// client refuses a mismatch rather than misreading a frame.
pub const PROTOCOL_VERSION: u32 = 1;

/// Static arena geometry. Sent once, in the hello. Never changes during a match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct ArenaInfo {
    pub side: f32,
    pub bases: Vec<BaseInfo>,
    pub nest: Region,
    pub wings: Vec<Region>,
    pub tick_hz: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct BaseInfo {
    pub team: TeamId,
    pub region: Region,
    pub cc_pos: Vec2,
}

/// Match parameters the viewer needs in order to draw meaningful things.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct MatchInfo {
    pub match_id: String,
    #[serde(with = "crate::u64str")]
    #[ts(type = "string")]
    pub seed: u64,
    pub point_target: u32,
    pub teams: Vec<TeamInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct TeamInfo {
    pub team: TeamId,
    pub name: String,
    pub agents: Vec<AgentId>,
    /// The control center agent for this team.
    pub cc: AgentId,
}

/// Frames from server to client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "t", rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum ServerMsg {
    /// First frame on every connection.
    Hello {
        protocol: u32,
        arena: ArenaInfo,
        match_info: MatchInfo,
    },
    /// Complete world state. Sent on connect, after a gap, and every tick in v0.
    Snapshot {
        tick: Tick,
        entities: Vec<EntityView>,
        scores: Vec<u32>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        events: Vec<Event>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        beliefs: Option<Vec<BeliefOverlay>>,
    },
    /// Changed field groups only. Not emitted in v0.
    Delta {
        tick: Tick,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        changed: Vec<EntityDelta>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        removed: Vec<EntityId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scores: Option<Vec<u32>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        events: Vec<Event>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        beliefs: Option<Vec<BeliefOverlay>>,
    },
    MatchOver {
        winner: Option<TeamId>,
        tick: Tick,
        scores: Vec<u32>,
    },
    Error {
        message: String,
    },
}

/// Frames from client to server. A viewer is a viewer; the only authority here is
/// the command channel, and that exists solely so a human can operate a control
/// center [CC].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "t", rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum ClientMsg {
    /// Issue a command as the human operator of one team's CC.
    Command { team: TeamId, cmd: Command },
    /// Ask for the belief overlay of one agent, or none to switch it off.
    WatchBelief { agent: Option<AgentId> },
    /// Ask for a full snapshot, after a dropped frame or a late join.
    Resync,
}

/// One line of a replay file. The file is the event stream plus the frames derived
/// from it, so the viewer decodes a replay with the same decoder it uses live.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "t", rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum ReplayLine {
    Header {
        protocol: u32,
        arena: ArenaInfo,
        match_info: MatchInfo,
    },
    Frame {
        frame: ServerMsg,
    },
}

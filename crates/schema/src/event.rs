//! The event enum. The only source of truth.
//!
//! Wire frames, the replay file, and the database are all derived from this one
//! stream. If the viewer and the database could disagree, someone would eventually
//! chase a bug that does not exist.
//!
//! The serialized form is adjacently tagged: `{"kind": "...", "payload": {...}}`.
//! That maps straight onto the `events` table, which stores `kind` and `payload`
//! in separate columns so a query can filter by kind without parsing JSON.
//!
//! Position updates are not events. Ten tanks, two hundred shapes, and bullets at
//! 30 Hz is roughly ten thousand rows per second, which would drown this table.
//! Kinematics go to their own table.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::action::Action;
use crate::belief::Recipient;
use crate::command::{Command, Origin};
use crate::entity::{Cause, Kind, ScoreReason};
use crate::geom::Vec2;
use crate::ids::{AgentId, EntityId, FocusId, TeamId, Tick};

/// Why the broker discarded a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum DropReason {
    /// Receiver outside comms radius at submit time.
    OutOfRange,
    /// Lost to the per-message drop probability.
    Loss,
    /// Over the per-link byte budget.
    Bandwidth,
    /// Receiver was dead when delivery came due.
    NoReceiver,
}

/// Which ceiling an agent exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum BudgetKind {
    /// World model footprint over the memory ceiling.
    Memory,
    /// Decision took longer than the per-tick time budget.
    Time,
}

/// Everything worth recording. Every sink consumes this type.
///
/// Adding variants later is cheap. Migrating a database of a hundred recorded
/// matches is not. The comms and budget variants are reserved now and not emitted
/// in v0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum Event {
    /// First event of every match. `config_hash` pins the configuration that
    /// produced the run, so a replay that does not reproduce is detectable.
    MatchStart {
        #[serde(with = "crate::u64str")]
        #[ts(type = "string")]
        seed: u64,
        #[serde(with = "crate::u64str")]
        #[ts(type = "string")]
        config_hash: u64,
        match_id: String,
    },
    TickBegin {
        tick: Tick,
    },
    Spawned {
        id: EntityId,
        kind: Kind,
        pos: Vec2,
        team: Option<TeamId>,
        focus: Option<FocusId>,
    },
    Despawned {
        id: EntityId,
        cause: Cause,
    },
    Damaged {
        target: EntityId,
        source: EntityId,
        amount: f32,
        remaining: f32,
    },
    Killed {
        target: EntityId,
        killer: EntityId,
    },
    Scored {
        team: TeamId,
        delta: u32,
        total: u32,
        reason: ScoreReason,
    },
    /// A command drained at the top of a tick, stamped with the tick it landed on.
    CommandIssued {
        team: TeamId,
        origin: Origin,
        cmd: Command,
    },
    /// One agent's decision, with the wall-clock time the policy took. The latency
    /// is diagnostic only and never feeds back into the simulation.
    ActionSubmitted {
        agent: AgentId,
        action: Action,
        latency_us: u32,
    },
    MatchEnd {
        winner: Option<TeamId>,
        tick: Tick,
    },

    // -- Reserved. Not emitted in v0. ---------------------------------------
    MessageSent {
        from: AgentId,
        to: Recipient,
        bytes: u32,
        tick: Tick,
    },
    MessageDelivered {
        from: AgentId,
        to: AgentId,
        bytes: u32,
        latency_ticks: u32,
    },
    MessageDropped {
        from: AgentId,
        to: Recipient,
        reason: DropReason,
    },
    BudgetExceeded {
        agent: AgentId,
        kind: BudgetKind,
        /// Bytes over the memory ceiling, or microseconds over the time budget.
        #[ts(type = "number")]
        overage: u64,
    },
}

impl Event {
    /// The `kind` column value. Must match the serde tag exactly, so the database
    /// and the wire agree on the name of a thing.
    pub fn kind_str(&self) -> &'static str {
        match self {
            Event::MatchStart { .. } => "match_start",
            Event::TickBegin { .. } => "tick_begin",
            Event::Spawned { .. } => "spawned",
            Event::Despawned { .. } => "despawned",
            Event::Damaged { .. } => "damaged",
            Event::Killed { .. } => "killed",
            Event::Scored { .. } => "scored",
            Event::CommandIssued { .. } => "command_issued",
            Event::ActionSubmitted { .. } => "action_submitted",
            Event::MatchEnd { .. } => "match_end",
            Event::MessageSent { .. } => "message_sent",
            Event::MessageDelivered { .. } => "message_delivered",
            Event::MessageDropped { .. } => "message_dropped",
            Event::BudgetExceeded { .. } => "budget_exceeded",
        }
    }

    /// True for variants reserved for the comms and budget work. A v0 sink can
    /// assert it never sees one.
    pub fn is_reserved(&self) -> bool {
        matches!(
            self,
            Event::MessageSent { .. }
                | Event::MessageDelivered { .. }
                | Event::MessageDropped { .. }
                | Event::BudgetExceeded { .. }
        )
    }
}

/// One row of the kinematics table. Written in batched transactions, one per tick,
/// separate from the discrete event stream.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct Kinematic {
    pub tick: Tick,
    pub entity: EntityId,
    pub pos: Vec2,
    pub vel: Vec2,
    pub heading: f32,
}

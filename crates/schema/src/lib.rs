//! Shared types for the tank coordination sandbox.
//!
//! This crate depends on nothing structural. Every other crate depends on it. That
//! asymmetry is what keeps the wire contract honest: there is one definition of an
//! event, one definition of an action, and the TypeScript the viewer compiles
//! against is generated from these types rather than written by hand.
//!
//! Regenerate the TypeScript with `cargo test -p schema`. The output lands in
//! `client/src/gen/schema.ts` and is checked in, so a drifted type fails the build
//! rather than the runtime.

pub mod action;
pub mod b64;
pub mod belief;
pub mod command;
pub mod entity;
pub mod event;
pub mod geom;
pub mod ids;
pub mod sense;
pub mod u64str;
pub mod wire;

pub use action::{Action, Choice, Control, Inputs, Observation, SelfView, StatKind};
pub use belief::{
    BeliefMsg, BeliefOverlay, CellState, Inbound, Intent, Interest, MapCell, Outbound, Recipient,
    Track,
};
pub use command::{Command, DoctrineId, Knob, ObjectiveHint, Origin, Role};
pub use entity::{
    Cause, Class, EntityDelta, EntityView, HealthGroup, Kind, PhysicsGroup, PositionGroup,
    ScoreGroup, ScoreReason, ShapeTier, Stats, StyleGroup, TeamGroup,
};
pub use event::{BudgetKind, DropReason, Event, Kinematic};
pub use geom::{Region, Vec2};
pub use ids::{AgentId, EntityId, FocusId, TeamId, Tick};
pub use sense::{Pose, Scan};
pub use wire::{
    ArenaInfo, BaseInfo, ClientMsg, MatchInfo, ReplayLine, ServerMsg, TeamInfo, PROTOCOL_VERSION,
};

/// Encode one newline-delimited JSON [NDJSON] line, newline included.
pub fn to_line<T: serde::Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut s = serde_json::to_string(value)?;
    s.push('\n');
    Ok(s)
}

/// Decode one newline-delimited JSON [NDJSON] line. Trailing newline optional.
pub fn from_line<T: serde::de::DeserializeOwned>(line: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(line.trim_end())
}

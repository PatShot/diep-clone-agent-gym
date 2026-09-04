//! Entity taxonomy and the field groups the delta scheme sends.
//!
//! Properties are grouped rather than flat. A delta names the groups that changed
//! and omits the rest. The grouping comes from diep.io and is a solved answer to
//! the problem of streaming many entities cheaply.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::geom::Vec2;
use crate::ids::{AgentId, EntityId, FocusId, TeamId};

/// What an entity is. Determines which field groups are meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum Kind {
    Tank,
    Shape,
    Bullet,
    ControlCenter,
}

/// Shape tier, ordered by worth. Squares and triangles scatter across the arena.
/// Pentagons and alpha pentagons spawn only in the nest, which is what makes the
/// arena centre worth contesting.
///
/// Named after diep.io's polygons because the viewer draws them as polygons and a
/// tier called `High` would have to be translated at the boundary anyway. The
/// ordering is deliberate: `Square < Triangle < Pentagon`, so a policy can compare
/// tiers without a lookup table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum ShapeTier {
    Square,
    Triangle,
    Pentagon,
    /// The nest's prize. Barely larger than a pentagon and worth many times more,
    /// so it raises the points a defender earns per unit of ground held rather
    /// than dominating the map by size.
    AlphaPentagon,
}

/// Tank class. Chosen once and irreversible. The sense radius difference is the
/// point: a siege tank is nearly blind and depends on teammates to see for it.
///
/// Not used in v0. Every tank is identical, with the same sense radius and the same
/// base stats, so that range-limited communication is the only asymmetry under study.
/// The type is defined now because it is carried in `StyleGroup` and `SelfView`, and
/// adding it later would be a wire change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum Class {
    Scout,
    Line,
    Siege,
}

/// The four upgradable stats. Four, not diep.io's eight, because build
/// optimisation is not the problem under study.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct Stats {
    pub damage: u8,
    pub reload: u8,
    pub speed: u8,
    pub health: u8,
}

/// Why an entity left the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum Cause {
    /// Health reached zero.
    Killed,
    /// A bullet outlived its lifetime.
    Expired,
    /// Destroyed at a sanctuary base boundary.
    Absorbed,
    /// Removed because the match ended.
    MatchEnd,
}

/// Why a team's score changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "schema.ts")]
pub enum ScoreReason {
    ShapeDestroyed,
    TankKilled,
}

// ---------------------------------------------------------------------------
// Field groups
// ---------------------------------------------------------------------------

/// Where the entity is and which way it faces. Changes every tick for anything
/// that moves.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct PositionGroup {
    pub pos: Vec2,
    /// Heading in radians. Zero points east, positive turns toward south.
    pub heading: f32,
}

/// Motion and collision extent.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct PhysicsGroup {
    pub vel: Vec2,
    pub radius: f32,
}

/// Health and regeneration state. Regeneration is what forces focus fire: one tank
/// cannot out-damage a healthy enemy's recovery, two can.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct HealthGroup {
    pub hp: f32,
    pub max_hp: f32,
    /// Tick of the last damage taken. Regeneration resumes a fixed delay after it.
    pub last_damaged: Option<crate::ids::Tick>,
}

/// Team ownership. Absent on neutral entities such as shapes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct TeamGroup {
    pub team: Option<TeamId>,
    /// The agent driving this entity, if any.
    pub agent: Option<AgentId>,
    /// The entity that fired this bullet, if any.
    pub owner: Option<EntityId>,
}

/// Accumulated score and level. A tank's own score sets its kill value, so a fed
/// tank is a target worth coordinating on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct ScoreGroup {
    pub score: u32,
    pub level: u8,
}

/// Rendering and classification. Near-static, so it rarely appears in a delta.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct StyleGroup {
    pub kind: Kind,
    pub tier: Option<ShapeTier>,
    pub class: Option<Class>,
    pub stats: Option<Stats>,
    /// The spawn focus that produced this entity.
    pub focus: Option<FocusId>,
}

/// A complete entity as carried in a snapshot. Every group present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct EntityView {
    pub id: EntityId,
    pub position: PositionGroup,
    pub physics: PhysicsGroup,
    pub style: StyleGroup,
    pub team: TeamGroup,
    pub health: Option<HealthGroup>,
    pub score: Option<ScoreGroup>,
}

/// An entity as carried in a delta. Only the groups that changed this tick are
/// present. A `None` group means unchanged, not absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct EntityDelta {
    pub id: EntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<PositionGroup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physics: Option<PhysicsGroup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<StyleGroup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<TeamGroup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<HealthGroup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<ScoreGroup>,
}

impl EntityDelta {
    /// A delta that changes nothing. Fill the groups that moved.
    pub fn new(id: EntityId) -> Self {
        Self {
            id,
            position: None,
            physics: None,
            style: None,
            team: None,
            health: None,
            score: None,
        }
    }

    /// True when no group changed. The encoder drops these rather than sending them.
    pub fn is_empty(&self) -> bool {
        self.position.is_none()
            && self.physics.is_none()
            && self.style.is_none()
            && self.team.is_none()
            && self.health.is_none()
            && self.score.is_none()
    }
}

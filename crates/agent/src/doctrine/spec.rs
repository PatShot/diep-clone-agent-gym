//! The doctrine vocabulary, as data.
//!
//! Everything a researcher — or, later, a language model at the control center —
//! can say about how a team fights is one of these structures. The set is small on
//! purpose: every name here is a line in a prompt, and a value nobody can read is a
//! value nobody can reason about.
//!
//! Three layers. A [`Doctrine`] is a composition of roles. A [`RoleBehaviour`] is
//! an ordered list of [`Stance`]s with a target scorer, a fire block and cohesion
//! rules. A stance is a predicate and a set of weighted drives; the first stance
//! whose predicate holds is the one in force.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use schema::{Region, Role, ShapeTier};

/// A team's way of fighting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Doctrine {
    pub name: String,
    /// Roles handed to a team's tanks in seat order. Once the counts run out, the
    /// remaining seats take the last role.
    pub roles: Vec<RoleCount>,
    /// How each role behaves. Every role named in `roles` needs an entry.
    pub behaviour: BTreeMap<Role, RoleBehaviour>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleCount {
    pub role: Role,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleBehaviour {
    #[serde(default)]
    pub cohesion: Cohesion,
    #[serde(default)]
    pub target: TargetSpec,
    #[serde(default)]
    pub fire: FireSpec,
    /// Walked in order. The first whose `when` holds is the stance in force. The
    /// last must be `always`.
    pub stances: Vec<Stance>,
}

// ---------------------------------------------------------------------------
// Stances and predicates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stance {
    pub name: String,
    pub when: Predicate,
    #[serde(default)]
    pub drives: Drives,
    /// Overrides the role's fire block while this stance is in force.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fire: Option<FireSpec>,
}

/// What a stance waits for. Distances are in world units; health is a fraction.
///
/// "Enemy" means an enemy tank whose track is recent enough to act on: its
/// uncertainty disc is no wider than a sense radius. A stale track is not an enemy
/// within anything.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Predicate {
    Always,
    HpBelow(f32),
    HpAbove(f32),
    EnemyWithin(f32),
    NoEnemyWithin(f32),
    TeammatesWithin {
        radius: f32,
        at_least: u32,
    },
    InRegion(Region),
    /// The target scorer found something.
    HasTarget,
    TargetValueAbove(f32),
    /// The control center designated a target and it is seen or remembered.
    HasDesignatedTarget,
    ObjectiveIs(ObjectiveKind),
    /// This team trails by at least this many points.
    ScoreBehindBy(u32),
    AllOf(Vec<Predicate>),
    AnyOf(Vec<Predicate>),
    Not(Box<Predicate>),
}

/// `schema::ObjectiveHint` by variant, for a predicate that does not care about
/// the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveKind {
    HoldNest,
    FarmSafe,
    Pressure,
    Defend,
    Rally,
    Explore,
}

// ---------------------------------------------------------------------------
// Drives
// ---------------------------------------------------------------------------

/// Weighted steering terms. Each yields a direction; the weighted sum, normalised,
/// is the thrust. A weight of zero is a drive not in use.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Drives {
    /// Toward the scored target.
    #[serde(skip_serializing_if = "is_zero")]
    pub seek_target: f32,
    /// Away from recent enemy tracks, nearer ones harder.
    #[serde(skip_serializing_if = "is_zero")]
    pub avoid_enemy: f32,
    /// Toward the leash anchor when beyond the leash; away when inside its minimum.
    /// Nothing without a leash.
    #[serde(skip_serializing_if = "is_zero")]
    pub cohere: f32,
    /// Away from teammates closer than the spacing.
    #[serde(skip_serializing_if = "is_zero")]
    pub separate: f32,
    /// Toward the nearest frontier of the world model.
    #[serde(skip_serializing_if = "is_zero")]
    pub explore: f32,
    /// Toward the friendly control center.
    #[serde(skip_serializing_if = "is_zero")]
    pub home: f32,
    /// Toward the control center's rally point, if one was set.
    #[serde(skip_serializing_if = "is_zero")]
    pub rally: f32,
    /// A direction from the agent's own seeded generator, held for a couple of
    /// seconds at a time. Breaks ties so a tank never sits still by accident.
    #[serde(skip_serializing_if = "is_zero")]
    pub wander: f32,
    /// Hold a distance from the nearest enemy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_range: Option<KeepRange>,
    /// Toward a region's centre from outside it; nothing inside.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold: Option<Hold>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeepRange {
    pub standoff: f32,
    pub weight: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hold {
    pub region: Region,
    pub weight: f32,
}

// ---------------------------------------------------------------------------
// Cohesion
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Cohesion {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leash: Option<Leash>,
    /// Teammates closer than this push each other apart.
    pub spacing: f32,
}

impl Default for Cohesion {
    fn default() -> Self {
        Self {
            leash: None,
            spacing: 25.0,
        }
    }
}

/// A band of distance from an anchor. A scout "beyond eyesight" has a minimum
/// larger than the sense radius.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Leash {
    pub to: Anchor,
    #[serde(default)]
    pub min: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    /// The centroid of the teammates this tank knows the position of. The
    /// friendly control center when it knows of none.
    TeamCentroid,
    Cc,
    /// The control center's rally point, else the control center.
    Rally,
}

// ---------------------------------------------------------------------------
// Target selection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TargetSpec {
    pub weights: TargetWeights,
    /// Shape tiers that may be targeted. All when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tiers: Option<Vec<ShapeTier>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_distance: Option<f32>,
    /// Never target anything standing in these.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub avoid_regions: Vec<Region>,
    /// Whether remembered shapes are candidates, or only seen ones.
    pub remembered: bool,
    /// Whether enemy tanks are candidates at all.
    pub enemies: bool,
}

impl Default for TargetSpec {
    fn default() -> Self {
        Self {
            weights: TargetWeights::default(),
            tiers: None,
            max_distance: None,
            avoid_regions: Vec::new(),
            remembered: true,
            enemies: true,
        }
    }
}

/// Linear scoring. Score is the weighted sum of the features; the highest wins.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TargetWeights {
    /// Points for destroying it.
    pub value: f32,
    /// Distance in units. Negative to prefer near.
    pub distance: f32,
    /// Remaining health. Negative to prefer wounded.
    pub hp: f32,
    /// Added when the candidate is an enemy tank.
    pub enemy_tank: f32,
    /// Per recent enemy within a hundred units of the candidate.
    pub threat: f32,
    /// Added when the control center designated this candidate.
    pub designated: f32,
    /// Per second since last seen. Negative to prefer the fresh.
    pub staleness: f32,
}

impl Default for TargetWeights {
    fn default() -> Self {
        Self {
            value: 1.0,
            distance: -0.2,
            hp: -0.05,
            enemy_tank: 0.0,
            threat: -20.0,
            designated: 1000.0,
            staleness: -0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// Fire discipline
// ---------------------------------------------------------------------------

/// When to shoot. Defined now, priced at v0.5: sensing in v0 is a radius query, so
/// a shot reveals nothing it did not already reveal, and silence costs tempo for
/// nothing measurable. The vocabulary is here so doctrines written today still
/// mean the same thing when a shot becomes an emission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FireSpec {
    pub mode: FireMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burst: Option<Burst>,
    pub require: FireRequire,
    /// Never fire while this holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silent_when: Option<Predicate>,
    /// Aim where the target will be, not where it is. Bullets do not inherit the
    /// firer's velocity, so this is a real computation.
    pub lead: bool,
}

impl Default for FireSpec {
    fn default() -> Self {
        Self {
            mode: FireMode::Free,
            burst: None,
            require: FireRequire::default(),
            silent_when: None,
            lead: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FireMode {
    /// Whenever the requirements hold.
    #[default]
    Free,
    /// `rounds` decision windows of fire, then `gap_ticks` of silence.
    Burst,
    /// Never.
    Hold,
}

/// A round is a decision window with fire held on, not a bullet. At the default
/// rates a window is five ticks and a reload is fifteen, so one round is one shot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Burst {
    pub rounds: u32,
    pub gap_ticks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FireRequire {
    /// Target no further than this. Bullet range when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub within: Option<f32>,
    /// Current heading within this many radians of the aim. No requirement when
    /// absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aim_error_below: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_value_above: Option<f32>,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

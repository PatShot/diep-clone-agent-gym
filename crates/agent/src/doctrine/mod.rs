//! Doctrine: a team's way of fighting, as data a runtime interprets.
//!
//! A doctrine is not a program. It selects and weights behaviours the engine
//! already knows how to execute — stances, drives, a target scorer, fire
//! discipline — and it can say nothing the engine cannot do. That is the point.
//! The file is small enough to read, bounded enough to validate, and flat enough
//! for a language model to edit one field of; anything the vocabulary cannot say
//! is written as a policy on the other end of the socket bridge instead.
//!
//! The runtime reads TOML for hand-written files and JSON as the canonical form,
//! both through serde. Validation lives here, at the runtime boundary, because a
//! doctrine written at runtime by a model never passes through an offline
//! compiler: whatever a configuration language would have checked, this must
//! check anyway.

pub mod spec;

mod ctx;
mod drive;
mod engine;
mod fire;
mod library;
mod predicate;
mod target;

pub use ctx::{Contact, Landmarks, Orders, Reassign};
pub use engine::DoctrinePolicy;
pub use fire::BULLET_RANGE;
pub use library::DoctrineLibrary;
pub use spec::*;
pub use target::{Target, TANK_KILL_VALUE};

use std::fmt;
use std::path::Path;

use schema::{AgentId, Role, TeamId};

use crate::runner::Runner;

/// One thing wrong with a doctrine, with where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub path: String,
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

#[derive(Debug)]
pub enum DoctrineError {
    Io(std::io::Error),
    Toml(toml::de::Error),
    Json(serde_json::Error),
    Invalid(Vec<Issue>),
    NoBehaviour(Role),
    UnknownDoctrine(String),
}

impl fmt::Display for DoctrineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DoctrineError::Io(e) => write!(f, "reading doctrine: {e}"),
            DoctrineError::Toml(e) => write!(f, "parsing doctrine: {e}"),
            DoctrineError::Json(e) => write!(f, "parsing doctrine: {e}"),
            DoctrineError::Invalid(issues) => {
                write!(f, "invalid doctrine:")?;
                for i in issues {
                    write!(f, "\n  {i}")?;
                }
                Ok(())
            }
            DoctrineError::NoBehaviour(role) => {
                write!(f, "doctrine has no behaviour for role {}", role_name(*role))
            }
            DoctrineError::UnknownDoctrine(name) => {
                write!(f, "no doctrine named {name:?} in the library")
            }
        }
    }
}

impl std::error::Error for DoctrineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DoctrineError::Io(e) => Some(e),
            DoctrineError::Toml(e) => Some(e),
            DoctrineError::Json(e) => Some(e),
            DoctrineError::Invalid(_)
            | DoctrineError::NoBehaviour(_)
            | DoctrineError::UnknownDoctrine(_) => None,
        }
    }
}

/// Largest weight a drive may carry, either sign. Beyond this the mix is meaningless.
pub const MAX_WEIGHT: f32 = 10.0;

pub fn role_name(role: Role) -> &'static str {
    match role {
        Role::Farm => "farm",
        Role::Screen => "screen",
        Role::Scout => "scout",
        Role::Relay => "relay",
        Role::Push => "push",
        Role::Defend => "defend",
        Role::Regroup => "regroup",
    }
}

impl Doctrine {
    /// Parse and validate TOML.
    pub fn from_toml_str(s: &str) -> Result<Self, DoctrineError> {
        let d: Doctrine = toml::from_str(s).map_err(DoctrineError::Toml)?;
        d.validate().map_err(DoctrineError::Invalid)?;
        Ok(d)
    }

    /// Parse and validate JSON. The form a language model reads and writes.
    pub fn from_json_str(s: &str) -> Result<Self, DoctrineError> {
        let d: Doctrine = serde_json::from_str(s).map_err(DoctrineError::Json)?;
        d.validate().map_err(DoctrineError::Invalid)?;
        Ok(d)
    }

    /// Read a file. JSON by extension, TOML otherwise.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, DoctrineError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(DoctrineError::Io)?;
        if path.extension().is_some_and(|e| e == "json") {
            Self::from_json_str(&text)
        } else {
            Self::from_toml_str(&text)
        }
    }

    pub fn to_toml_string(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn behaviour(&self, role: Role) -> Option<&RoleBehaviour> {
        self.behaviour.get(&role)
    }

    /// Every issue at once, so an author fixes a file in one pass.
    pub fn validate(&self) -> Result<(), Vec<Issue>> {
        let mut out = Vec::new();
        let mut issue = |path: String, message: &str| {
            out.push(Issue {
                path,
                message: message.to_string(),
            })
        };

        if self.name.trim().is_empty() {
            issue("name".into(), "must not be empty");
        }
        if self.roles.is_empty() {
            issue("roles".into(), "must name at least one role");
        }
        for (i, rc) in self.roles.iter().enumerate() {
            if rc.count == 0 {
                issue(format!("roles[{i}]"), "count must be at least one");
            }
            if !self.behaviour.contains_key(&rc.role) {
                issue(
                    format!("roles[{i}]"),
                    &format!("no behaviour for role {}", role_name(rc.role)),
                );
            }
        }

        for (role, b) in &self.behaviour {
            let base = format!("behaviour.{}", role_name(*role));
            validate_behaviour(&base, b, &mut issue);
        }

        if out.is_empty() {
            Ok(())
        } else {
            Err(out)
        }
    }

    /// Roles for `seats`, in order: each `RoleCount` fills its count, and any seat
    /// past the last count takes the last role.
    pub fn assign(&self, seats: &[AgentId]) -> Vec<(AgentId, Role)> {
        let mut out = Vec::with_capacity(seats.len());
        let mut counts = self.roles.iter();
        let mut current = counts.next().copied();
        let mut filled = 0u32;
        for &seat in seats {
            let Some(rc) = current else {
                break;
            };
            out.push((seat, rc.role));
            filled += 1;
            if filled >= rc.count {
                if let Some(next) = counts.next() {
                    current = Some(*next);
                    filled = 0;
                }
            }
        }
        out
    }

    /// Seat this doctrine on every tank of `team` in the runner. Returns who got
    /// which role.
    pub fn seat(
        &self,
        runner: &mut Runner,
        team: TeamId,
    ) -> Result<Vec<(AgentId, Role)>, DoctrineError> {
        let seats: Vec<AgentId> = runner
            .seats()
            .filter(|(_, t, is_cc)| *t == team && !is_cc)
            .map(|(a, _, _)| a)
            .collect();
        let arena = runner.world().arena.clone();
        let seed = runner.world().spec().seed;
        let assigned = self.assign(&seats);
        let library = std::sync::Arc::new(DoctrineLibrary::of(self.clone()));
        for (agent, role) in &assigned {
            let policy = DoctrinePolicy::with_library(
                library.clone(),
                &self.name,
                *role,
                team,
                &arena,
                seed,
                *agent,
            )?;
            runner.set_policy(*agent, Box::new(policy));
        }
        Ok(assigned)
    }
}

fn validate_behaviour(base: &str, b: &RoleBehaviour, issue: &mut impl FnMut(String, &str)) {
    if b.stances.is_empty() {
        issue(format!("{base}.stances"), "must have at least one stance");
    }
    if let Some(last) = b.stances.last() {
        if last.when != Predicate::Always {
            issue(
                format!("{base}.stances[{}]", b.stances.len() - 1),
                "the last stance must be `always`, so the tank is never without one",
            );
        }
    }
    for (i, s) in b.stances.iter().enumerate() {
        let path = format!("{base}.stances[{i}]");
        if s.name.trim().is_empty() {
            issue(format!("{path}.name"), "must not be empty");
        }
        if b.stances[..i].iter().any(|o| o.name == s.name) {
            issue(format!("{path}.name"), "duplicate stance name");
        }
        validate_predicate(&format!("{path}.when"), &s.when, issue);
        validate_drives(&format!("{path}.drives"), &s.drives, issue);
        if let Some(f) = &s.fire {
            validate_fire(&format!("{path}.fire"), f, issue);
        }
    }

    let c = &b.cohesion;
    if negative(c.spacing) {
        issue(format!("{base}.cohesion.spacing"), "must be at least zero");
    }
    if let Some(l) = c.leash {
        if negative(l.min) {
            issue(
                format!("{base}.cohesion.leash.min"),
                "must be at least zero",
            );
        }
        if l.max.is_some_and(|m| m.is_nan() || m < l.min) {
            issue(
                format!("{base}.cohesion.leash.max"),
                "must be at least the minimum",
            );
        }
    }

    let t = &b.target;
    let w = t.weights;
    for (name, v) in [
        ("value", w.value),
        ("distance", w.distance),
        ("hp", w.hp),
        ("enemy_tank", w.enemy_tank),
        ("threat", w.threat),
        ("designated", w.designated),
        ("staleness", w.staleness),
    ] {
        if !v.is_finite() {
            issue(format!("{base}.target.weights.{name}"), "must be finite");
        }
    }
    if t.max_distance.is_some_and(negative) {
        issue(
            format!("{base}.target.max_distance"),
            "must be at least zero",
        );
    }
    if t.tiers.as_ref().is_some_and(|ts| ts.is_empty()) {
        issue(
            format!("{base}.target.tiers"),
            "an empty list targets nothing; omit it to target every tier",
        );
    }

    validate_fire(&format!("{base}.fire"), &b.fire, issue);
}

fn validate_drives(path: &str, d: &Drives, issue: &mut impl FnMut(String, &str)) {
    for (name, v) in [
        ("seek_target", d.seek_target),
        ("avoid_enemy", d.avoid_enemy),
        ("cohere", d.cohere),
        ("separate", d.separate),
        ("explore", d.explore),
        ("home", d.home),
        ("rally", d.rally),
        ("wander", d.wander),
    ] {
        check_weight(&format!("{path}.{name}"), v, issue);
    }
    if let Some(k) = d.keep_range {
        check_weight(&format!("{path}.keep_range.weight"), k.weight, issue);
        if negative(k.standoff) {
            issue(
                format!("{path}.keep_range.standoff"),
                "must be at least zero",
            );
        }
    }
    if let Some(h) = d.hold {
        check_weight(&format!("{path}.hold.weight"), h.weight, issue);
        if h.region == schema::Region::WholeArena {
            issue(
                format!("{path}.hold.region"),
                "the whole arena has no centre to hold",
            );
        }
    }
}

/// Below zero, or not a number at all. A NaN fails every check, which is the point.
fn negative(v: f32) -> bool {
    v.is_nan() || v < 0.0
}

fn check_weight(path: &str, v: f32, issue: &mut impl FnMut(String, &str)) {
    if !v.is_finite() || v.abs() > MAX_WEIGHT {
        issue(
            path.to_string(),
            &format!("must be a finite weight within ±{MAX_WEIGHT}"),
        );
    }
}

fn validate_fire(path: &str, f: &FireSpec, issue: &mut impl FnMut(String, &str)) {
    match (f.mode, f.burst) {
        (FireMode::Burst, None) => issue(
            format!("{path}.burst"),
            "burst mode needs a burst: rounds and gap_ticks",
        ),
        (FireMode::Burst, Some(b)) if b.rounds == 0 => {
            issue(format!("{path}.burst.rounds"), "must be at least one")
        }
        (m, Some(_)) if m != FireMode::Burst => issue(
            format!("{path}.burst"),
            "a burst is given but the mode is not `burst`",
        ),
        _ => {}
    }
    if f.require.within.is_some_and(negative) {
        issue(format!("{path}.require.within"), "must be at least zero");
    }
    if f.require
        .aim_error_below
        .is_some_and(|e| !(0.0..=std::f32::consts::PI).contains(&e))
    {
        issue(
            format!("{path}.require.aim_error_below"),
            "must be between zero and π radians",
        );
    }
    if let Some(p) = &f.silent_when {
        validate_predicate(&format!("{path}.silent_when"), p, issue);
    }
}

fn validate_predicate(path: &str, p: &Predicate, issue: &mut impl FnMut(String, &str)) {
    match p {
        Predicate::HpBelow(f) | Predicate::HpAbove(f) => {
            if !(0.0..=1.0).contains(f) {
                issue(
                    path.to_string(),
                    "health is a fraction between zero and one",
                );
            }
        }
        Predicate::EnemyWithin(r) | Predicate::NoEnemyWithin(r) => {
            if negative(*r) {
                issue(path.to_string(), "distance must be at least zero");
            }
        }
        Predicate::TeammatesWithin { radius, at_least } => {
            if negative(*radius) {
                issue(format!("{path}.radius"), "must be at least zero");
            }
            if *at_least == 0 {
                issue(format!("{path}.at_least"), "must be at least one");
            }
        }
        Predicate::TargetValueAbove(v) => {
            if !v.is_finite() {
                issue(path.to_string(), "must be finite");
            }
        }
        Predicate::AllOf(ps) | Predicate::AnyOf(ps) => {
            if ps.is_empty() {
                issue(path.to_string(), "must contain at least one predicate");
            }
            for (i, q) in ps.iter().enumerate() {
                validate_predicate(&format!("{path}[{i}]"), q, issue);
            }
        }
        Predicate::Not(q) => validate_predicate(&format!("{path}.not"), q, issue),
        Predicate::Always
        | Predicate::InRegion(_)
        | Predicate::HasTarget
        | Predicate::HasDesignatedTarget
        | Predicate::ObjectiveIs(_)
        | Predicate::ScoreBehindBy(_) => {}
    }
}

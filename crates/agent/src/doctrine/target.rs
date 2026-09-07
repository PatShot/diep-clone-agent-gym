//! Which thing to go after.
//!
//! A linear score over a few named features. Not clever, and readable: the weights
//! are the whole explanation of why a tank picked what it picked.

use schema::{EntityId, Kind, ShapeTier, Vec2};
use sim_core::constants::{
    SHAPE_VALUE_ALPHA, SHAPE_VALUE_PENTAGON, SHAPE_VALUE_SQUARE, SHAPE_VALUE_TRIANGLE,
    TICKS_PER_SEC,
};

use super::ctx::Contact;
use super::spec::TargetSpec;

/// PROVISIONAL. Points for killing a tank, for scoring purposes only. Nothing
/// scores yet; the objective crate will say what a tank is worth.
pub const TANK_KILL_VALUE: f32 = 100.0;

/// Enemies closer than this to a candidate count as threats around it.
pub const THREAT_RADIUS: f32 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub contact: Contact,
    pub value: f32,
    pub score: f32,
}

/// Points for destroying a contact.
pub fn value_of(c: &Contact) -> f32 {
    match c.kind {
        Kind::Tank => TANK_KILL_VALUE,
        Kind::Shape => match c.tier {
            Some(ShapeTier::Square) | None => SHAPE_VALUE_SQUARE as f32,
            Some(ShapeTier::Triangle) => SHAPE_VALUE_TRIANGLE as f32,
            Some(ShapeTier::Pentagon) => SHAPE_VALUE_PENTAGON as f32,
            Some(ShapeTier::AlphaPentagon) => SHAPE_VALUE_ALPHA as f32,
        },
        Kind::Bullet | Kind::ControlCenter => 0.0,
    }
}

/// The best candidate by the spec's weights, or none. Ties go to the nearer, then
/// to the lower entity identity, so the choice is the same on every run.
pub fn choose(
    candidates: &[Contact],
    enemies: &[Contact],
    me: Vec2,
    designated: Option<EntityId>,
    spec: &TargetSpec,
    arena_side: f32,
) -> Option<Target> {
    let w = spec.weights;
    let mut best: Option<Target> = None;

    for c in candidates {
        if let (Kind::Shape, Some(tiers)) = (c.kind, &spec.tiers) {
            if !c.tier.is_some_and(|t| tiers.contains(&t)) {
                continue;
            }
        }
        let distance = c.pos.distance(me);
        if spec.max_distance.is_some_and(|m| distance > m) {
            continue;
        }
        if spec
            .avoid_regions
            .iter()
            .any(|r| r.contains(c.pos, arena_side))
        {
            continue;
        }

        let value = value_of(c);
        let threats = enemies
            .iter()
            .filter(|e| e.id != c.id && e.pos.distance(c.pos) <= THREAT_RADIUS)
            .count() as f32;
        let score = w.value * value
            + w.distance * distance
            + w.hp * c.hp.unwrap_or(0.0)
            + if c.kind == Kind::Tank {
                w.enemy_tank
            } else {
                0.0
            }
            + w.threat * threats
            + if designated == Some(c.id) {
                w.designated
            } else {
                0.0
            }
            + w.staleness * (c.age_ticks as f32 / TICKS_PER_SEC);

        let candidate = Target {
            contact: *c,
            value,
            score,
        };
        best = match best {
            None => Some(candidate),
            Some(b) => Some(if better(&candidate, &b, me) {
                candidate
            } else {
                b
            }),
        };
    }
    best
}

fn better(a: &Target, b: &Target, me: Vec2) -> bool {
    if a.score != b.score {
        return a.score > b.score;
    }
    let (da, db) = (
        a.contact.pos.distance_squared(me),
        b.contact.pos.distance_squared(me),
    );
    if da != db {
        return da < db;
    }
    (a.contact.id.index, a.contact.id.generation) < (b.contact.id.index, b.contact.id.generation)
}

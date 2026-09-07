//! Evaluating a stance's guard.

use schema::ObjectiveHint;

use super::ctx::Ctx;
use super::spec::{ObjectiveKind, Predicate};

pub fn holds(p: &Predicate, ctx: &Ctx<'_>) -> bool {
    match p {
        Predicate::Always => true,
        Predicate::HpBelow(f) => ctx.hp_frac < *f,
        Predicate::HpAbove(f) => ctx.hp_frac > *f,
        Predicate::EnemyWithin(r) => ctx.enemies.iter().any(|e| e.pos.distance(ctx.me) <= *r),
        Predicate::NoEnemyWithin(r) => !ctx.enemies.iter().any(|e| e.pos.distance(ctx.me) <= *r),
        Predicate::TeammatesWithin { radius, at_least } => {
            let n = ctx
                .teammates
                .iter()
                .filter(|t| t.pos.distance(ctx.me) <= *radius)
                .count();
            n as u32 >= *at_least
        }
        Predicate::InRegion(region) => region.contains(ctx.me, ctx.landmarks.side),
        Predicate::HasTarget => ctx.target.is_some(),
        Predicate::TargetValueAbove(v) => ctx.target.is_some_and(|t| t.value > *v),
        Predicate::HasDesignatedTarget => ctx
            .orders
            .designated
            .is_some_and(|d| ctx.candidates.iter().any(|c| c.id == d)),
        Predicate::ObjectiveIs(kind) => ctx.orders.objective.is_some_and(|o| kind_of(&o) == *kind),
        Predicate::ScoreBehindBy(n) => ctx.their_score() >= ctx.my_score().saturating_add(*n),
        Predicate::AllOf(ps) => ps.iter().all(|p| holds(p, ctx)),
        Predicate::AnyOf(ps) => ps.iter().any(|p| holds(p, ctx)),
        Predicate::Not(p) => !holds(p, ctx),
    }
}

pub fn kind_of(hint: &ObjectiveHint) -> ObjectiveKind {
    match hint {
        ObjectiveHint::HoldNest => ObjectiveKind::HoldNest,
        ObjectiveHint::FarmSafe => ObjectiveKind::FarmSafe,
        ObjectiveHint::Pressure => ObjectiveKind::Pressure,
        ObjectiveHint::Defend => ObjectiveKind::Defend,
        ObjectiveHint::Rally { .. } => ObjectiveKind::Rally,
        ObjectiveHint::Explore { .. } => ObjectiveKind::Explore,
    }
}

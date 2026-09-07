//! Whether to shoot, and where to point.
//!
//! Only a visible target can be shot. A remembered one is a place to go.

use schema::{Tick, Vec2};
use sim_core::constants::{BULLET_LIFETIME_TICKS, BULLET_SPEED, DT};

use super::ctx::Ctx;
use super::predicate;
use super::spec::{FireMode, FireSpec};
use crate::geom::{heading, sub};

/// How far a bullet travels before it expires. The default `require.within`.
pub const BULLET_RANGE: f32 = BULLET_SPEED * BULLET_LIFETIME_TICKS as f32 * DT;

/// Where a burst is. Lives on the policy, across decisions.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BurstState {
    rounds_left: u32,
    gap_until: Tick,
}

impl BurstState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Fire this decision, and the heading to aim if there is a visible target.
pub fn decide(spec: &FireSpec, ctx: &Ctx<'_>, burst: &mut BurstState) -> (bool, Option<f32>) {
    let Some(target) = ctx.target.filter(|t| t.contact.visible) else {
        return (false, None);
    };
    let c = target.contact;

    let aim = if spec.lead {
        let d = c.pos.distance(ctx.me);
        let t = d / BULLET_SPEED;
        let predicted = Vec2::new(c.pos.x + c.vel.x * t, c.pos.y + c.vel.y * t);
        heading(sub(predicted, ctx.me))
    } else {
        heading(sub(c.pos, ctx.me))
    };

    let allowed = ctx.reload_ready
        && spec.mode != FireMode::Hold
        && c.pos.distance(ctx.me) <= spec.require.within.unwrap_or(BULLET_RANGE)
        && spec
            .require
            .aim_error_below
            .map_or(true, |e| angle_between(aim, ctx.heading) <= e)
        && spec
            .require
            .target_value_above
            .map_or(true, |v| target.value > v)
        && !spec
            .silent_when
            .as_ref()
            .is_some_and(|p| predicate::holds(p, ctx));

    if !allowed {
        return (false, Some(aim));
    }

    let fire = match (spec.mode, spec.burst) {
        (FireMode::Burst, Some(b)) => {
            if ctx.now < burst.gap_until {
                false
            } else {
                if burst.rounds_left == 0 {
                    burst.rounds_left = b.rounds.max(1);
                }
                burst.rounds_left -= 1;
                if burst.rounds_left == 0 {
                    burst.gap_until = Tick(ctx.now.0.saturating_add(b.gap_ticks));
                }
                true
            }
        }
        _ => true,
    };
    (fire, Some(aim))
}

/// Unsigned angle between two headings, in radians, in `[0, π]`.
pub fn angle_between(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(std::f32::consts::TAU);
    if d > std::f32::consts::PI {
        std::f32::consts::TAU - d
    } else {
        d
    }
}

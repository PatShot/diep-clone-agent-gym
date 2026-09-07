//! Drives: named steering terms, summed by weight into a thrust.
//!
//! Each drive yields a direction of length one or less, or zero when it has
//! nothing to say. The weighted sum is normalised, so weights set the mix and not
//! the speed. This is the behaviour-based tradition — Arkin's motor schemas,
//! Reynolds's flocking rules — with the terms named so a doctrine can be read.

use schema::{Region, Vec2};

use super::ctx::Ctx;
use super::spec::{Anchor, Cohesion, Drives};
use crate::geom::{normalized, scale, sub, toward};
use crate::model::WorldModel;

/// Closer than this to a destination counts as there.
const ARRIVED: f32 = 20.0;

pub fn thrust(drives: &Drives, cohesion: &Cohesion, ctx: &Ctx<'_>, wander: Vec2) -> Vec2 {
    let mut sum = Vec2::ZERO;
    let mut add = |w: f32, d: Vec2| {
        if w != 0.0 {
            sum = Vec2::new(sum.x + w * d.x, sum.y + w * d.y);
        }
    };

    add(drives.seek_target, seek_target(ctx));
    if let Some(k) = drives.keep_range {
        add(k.weight, keep_range(ctx, k.standoff));
    }
    add(drives.avoid_enemy, avoid_enemy(ctx));
    add(drives.cohere, cohere(ctx, cohesion));
    add(drives.separate, separate(ctx, cohesion.spacing));
    add(drives.explore, explore(ctx));
    add(drives.home, toward_unless_there(ctx.me, ctx.landmarks.home));
    if let Some(h) = drives.hold {
        add(h.weight, hold(ctx, &h.region));
    }
    if let Some(r) = ctx.orders.rally {
        add(drives.rally, toward_unless_there(ctx.me, r));
    }
    add(drives.wander, wander);

    normalized(sum)
}

fn seek_target(ctx: &Ctx<'_>) -> Vec2 {
    ctx.target
        .map(|t| toward(ctx.me, t.contact.pos))
        .unwrap_or(Vec2::ZERO)
}

/// Toward the nearest enemy when further than the standoff, away when nearer,
/// with strength growing with the error. Zero at the standoff itself.
fn keep_range(ctx: &Ctx<'_>, standoff: f32) -> Vec2 {
    let Some(e) = ctx.nearest_enemy() else {
        return Vec2::ZERO;
    };
    if standoff <= 0.0 {
        return toward(ctx.me, e.pos);
    }
    let d = e.pos.distance(ctx.me);
    let error = ((d - standoff) / standoff).clamp(-1.0, 1.0);
    scale(toward(ctx.me, e.pos), error)
}

/// Away from every recent enemy within two sense radii, the near ones harder.
fn avoid_enemy(ctx: &Ctx<'_>) -> Vec2 {
    let reach = ctx.sense_radius * 2.0;
    let mut sum = Vec2::ZERO;
    for e in &ctx.enemies {
        let d = e.pos.distance(ctx.me);
        if d < reach {
            let away = toward(e.pos, ctx.me);
            let w = 1.0 - d / reach;
            sum = Vec2::new(sum.x + away.x * w, sum.y + away.y * w);
        }
    }
    normalized(sum)
}

/// Toward the anchor beyond the leash's maximum, away inside its minimum. Nothing
/// between, and nothing without a leash. The team centroid is where the teammates
/// are, or where they last were seen: without comms, sight is the only way to
/// know, and a leash that snapped every time a teammate stepped out of view would
/// hold nothing.
fn cohere(ctx: &Ctx<'_>, cohesion: &Cohesion) -> Vec2 {
    let Some(leash) = cohesion.leash else {
        return Vec2::ZERO;
    };
    let anchor = match leash.to {
        Anchor::Cc => ctx.landmarks.home,
        Anchor::Rally => ctx.orders.rally.unwrap_or(ctx.landmarks.home),
        Anchor::TeamCentroid => ctx.group.unwrap_or(ctx.landmarks.home),
    };
    let d = anchor.distance(ctx.me);
    if leash.max.is_some_and(|max| d > max) {
        toward(ctx.me, anchor)
    } else if d < leash.min {
        toward(anchor, ctx.me)
    } else {
        Vec2::ZERO
    }
}

/// Away from teammates inside the spacing, the near ones harder.
fn separate(ctx: &Ctx<'_>, spacing: f32) -> Vec2 {
    if spacing <= 0.0 {
        return Vec2::ZERO;
    }
    let mut sum = Vec2::ZERO;
    for t in &ctx.teammates {
        let d = t.pos.distance(ctx.me);
        if d < spacing {
            let away = toward(t.pos, ctx.me);
            let w = 1.0 - d / spacing;
            sum = Vec2::new(sum.x + away.x * w, sum.y + away.y * w);
        }
    }
    normalized(sum)
}

/// Toward the nearest frontier of the world model.
fn explore(ctx: &Ctx<'_>) -> Vec2 {
    ctx.model
        .frontiers(ctx.me, 1)
        .first()
        .and_then(region_centre)
        .map(|c| toward_unless_there(ctx.me, c))
        .unwrap_or(Vec2::ZERO)
}

/// Toward a region's centre from outside it. Nothing inside.
fn hold(ctx: &Ctx<'_>, region: &Region) -> Vec2 {
    if region.contains(ctx.me, ctx.landmarks.side) {
        return Vec2::ZERO;
    }
    region_centre(region)
        .map(|c| toward(ctx.me, c))
        .unwrap_or(Vec2::ZERO)
}

fn toward_unless_there(me: Vec2, goal: Vec2) -> Vec2 {
    if sub(goal, me).length() <= ARRIVED {
        Vec2::ZERO
    } else {
        toward(me, goal)
    }
}

pub fn region_centre(r: &Region) -> Option<Vec2> {
    match *r {
        Region::Disc { center, .. } | Region::Annulus { center, .. } => Some(center),
        Region::Rect { min, max } => Some(Vec2::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5)),
        Region::WholeArena => None,
    }
}

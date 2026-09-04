//! Arena geometry and the sanctuary rule.
//!
//! The arena is a square with a base at two opposite corners. A base is a
//! sanctuary: enemy tanks cannot enter it and enemy fire is destroyed at its
//! boundary. That removes spawn camping and gives a losing team somewhere to
//! regroup, so one bad engagement does not decide a match.

use schema::{ArenaInfo, BaseInfo, Region, TeamId, Vec2};

use crate::constants::{ARENA_SIDE, BASE_SIDE, CC_RADIUS, NEST_RADIUS, TICK_HZ};

/// Static arena geometry. Fixed for the whole match.
#[derive(Debug, Clone, PartialEq)]
pub struct ArenaSpec {
    pub side: f32,
    /// Indexed by team identifier. Team A is northwest, team B is southeast.
    pub bases: [Rect; 2],
    pub nest: Region,
    /// Off by default. Present so a researcher can add contested areas and watch
    /// how team allocation changes.
    pub wings: Vec<Region>,
}

/// An axis-aligned rectangle. Bases are rectangles; the nest is a disc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
        )
    }

    /// True when a circle of `radius` at `p` overlaps this rectangle at all.
    pub fn overlaps_circle(&self, p: Vec2, radius: f32) -> bool {
        let nearest = Vec2::new(
            p.x.clamp(self.min.x, self.max.x),
            p.y.clamp(self.min.y, self.max.y),
        );
        p.distance_squared(nearest) < radius * radius
    }

    fn to_region(self) -> Region {
        Region::Rect {
            min: self.min,
            max: self.max,
        }
    }
}

impl Default for ArenaSpec {
    fn default() -> Self {
        Self::new(ARENA_SIDE, BASE_SIDE, NEST_RADIUS)
    }
}

impl ArenaSpec {
    /// Bases at the northwest and southeast corners, nest at the centre, no wings.
    /// This is the original diep.io Team Deathmatch layout.
    pub fn new(side: f32, base_side: f32, nest_radius: f32) -> Self {
        let a = Rect {
            min: Vec2::ZERO,
            max: Vec2::new(base_side, base_side),
        };
        let b = Rect {
            min: Vec2::new(side - base_side, side - base_side),
            max: Vec2::new(side, side),
        };
        Self {
            side,
            bases: [a, b],
            nest: Region::Disc {
                center: Vec2::new(side * 0.5, side * 0.5),
                radius: nest_radius,
            },
            wings: Vec::new(),
        }
    }

    pub fn base(&self, team: TeamId) -> &Rect {
        &self.bases[team.0 as usize]
    }

    /// Where a team's control center [CC] stands: the centre of its base.
    pub fn cc_pos(&self, team: TeamId) -> Vec2 {
        self.base(team).center()
    }

    /// The base a point lies inside, if any.
    pub fn base_at(&self, p: Vec2) -> Option<TeamId> {
        if self.bases[0].contains(p) {
            Some(TeamId::A)
        } else if self.bases[1].contains(p) {
            Some(TeamId::B)
        } else {
            None
        }
    }

    /// Clamp a circle's centre so the circle stays wholly inside the arena.
    /// Returns the clamped point and which axes were hit, so the caller can zero
    /// the velocity component that drove into the wall.
    pub fn clamp_circle(&self, p: Vec2, radius: f32) -> (Vec2, bool, bool) {
        let lo = radius;
        let hi = self.side - radius;
        let x = p.x.clamp(lo, hi);
        let y = p.y.clamp(lo, hi);
        (Vec2::new(x, y), x != p.x, y != p.y)
    }

    /// Push a circle out of `base` by the shortest move that stays inside the
    /// arena. Returns the corrected centre and which axis moved.
    ///
    /// Least-penetration resolution alone is wrong here. Bases sit flush in the
    /// arena corners, so for the northwest base the shortest way out of the
    /// rectangle is very often north or west — through the arena wall and out of
    /// the world. A shape drifting into a corner leaves the map entirely, and
    /// because it is then outside every collision path it never comes back.
    ///
    /// So each of the four directions is tested for whether it leaves the circle
    /// inside the arena, and only the survivors compete on distance.
    pub fn eject_from_base(&self, base_of: TeamId, p: Vec2, radius: f32) -> (Vec2, bool, bool) {
        let r = self.bases[base_of.0 as usize];
        if !r.overlaps_circle(p, radius) {
            return (p, false, false);
        }

        let lo = radius;
        let hi = self.side - radius;

        // (target coordinate, distance moved, is this the x axis)
        let candidates = [
            (r.min.x - radius, p.x - (r.min.x - radius), true),
            (r.max.x + radius, (r.max.x + radius) - p.x, true),
            (r.min.y - radius, p.y - (r.min.y - radius), false),
            (r.max.y + radius, (r.max.y + radius) - p.y, false),
        ];

        let mut best: Option<(f32, f32, bool)> = None;
        for (target, dist, is_x) in candidates {
            if target < lo || target > hi {
                continue;
            }
            // `is_none_or` would read better but is stable only from 1.82.
            let better = match best {
                None => true,
                Some((_, d, _)) => dist < d,
            };
            if better {
                best = Some((target, dist, is_x));
            }
        }

        match best {
            Some((target, _, true)) => (Vec2::new(target, p.y), true, false),
            Some((target, _, false)) => (Vec2::new(p.x, target), false, true),
            // A base wider than the arena leaves nowhere to go. Not reachable with
            // any sane configuration, and clamping afterwards keeps it in bounds.
            None => (p, false, false),
        }
    }

    /// The wire form, sent once in the hello frame.
    pub fn to_info(&self) -> ArenaInfo {
        ArenaInfo {
            side: self.side,
            bases: [TeamId::A, TeamId::B]
                .into_iter()
                .map(|team| BaseInfo {
                    team,
                    region: self.bases[team.0 as usize].to_region(),
                    cc_pos: self.cc_pos(team),
                })
                .collect(),
            nest: self.nest,
            wings: self.wings.clone(),
            tick_hz: TICK_HZ,
        }
    }

    /// Radius of the control center marker. Rendering and sensing only.
    pub const fn cc_radius(&self) -> f32 {
        CC_RADIUS
    }
}

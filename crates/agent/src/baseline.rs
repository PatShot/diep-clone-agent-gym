//! Scripted baselines. Trivial on purpose.
//!
//! `AGENTS.md`: "Scripted policies stay trivial throughout: drive to nearest shape,
//! shoot it. They exist to make the viewer show something, not to be good."

use schema::{Action, Control, Kind, Observation, Region, Tick, Vec2};

use crate::geom::{heading, toward};
use crate::model::{DenseGrid, WorldModel};
use crate::Policy;

/// Does nothing. The control center's policy until a control center has one, and
/// the runner's default for any seat nothing else claims.
pub struct Idle;

impl Policy for Idle {
    fn decide(&mut self, _obs: &Observation, _now: Tick) -> Action {
        Action::idle()
    }
}

/// Drive to the nearest visible shape and shoot it.
///
/// With nothing in sight it drives toward the arena centre, which is where the
/// shapes are. That case is not hypothetical: shapes keep clear of the bases by a
/// sense radius, so a tank on its spawn point sees nothing at all.
///
/// Ties are broken by observation order, which is slot order, so the choice is
/// the same on every run of one seed.
pub struct NearestShape {
    centre: Vec2,
}

impl NearestShape {
    pub fn new(arena_side: f32) -> Self {
        Self {
            centre: Vec2::new(arena_side * 0.5, arena_side * 0.5),
        }
    }
}

impl Policy for NearestShape {
    fn decide(&mut self, obs: &Observation, _now: Tick) -> Action {
        // Dead. Nothing to drive.
        if obs.own.entity.is_none() {
            return Action::idle();
        }
        let me = obs.own.pos;

        let target = obs
            .visible
            .iter()
            .filter(|v| v.style.kind == Kind::Shape)
            .min_by(|a, b| {
                let da = a.position.pos.distance_squared(me);
                let db = b.position.pos.distance_squared(me);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });

        let (dir, fire) = match target {
            Some(t) => (toward(me, t.position.pos), obs.own.reload_ready),
            None => (toward(me, self.centre), false),
        };

        Action {
            control: Control {
                thrust: dir,
                aim: heading(dir),
                fire,
            },
            ..Action::default()
        }
    }
}

/// Drive to the nearest shape it *knows of*, seen or remembered, and shoot what it
/// can see. With nothing remembered it heads for the nearest frontier of its map,
/// and with no map yet, for the centre.
///
/// The first policy with a belief. Where [`NearestShape`] forgets everything
/// between decisions, this one carries a [`DenseGrid`] and farms from memory: a
/// shape seen a minute ago is still a place worth going back to. It is still
/// trivial. It exists so the world model has a caller.
pub struct NearestKnownShape {
    centre: Vec2,
    model: DenseGrid,
}

impl NearestKnownShape {
    pub fn new(arena_side: f32) -> Self {
        Self {
            centre: Vec2::new(arena_side * 0.5, arena_side * 0.5),
            model: DenseGrid::new(arena_side),
        }
    }

    pub fn model(&self) -> &DenseGrid {
        &self.model
    }

    fn drive(dir: Vec2, fire: bool) -> Action {
        Action {
            control: Control {
                thrust: dir,
                aim: heading(dir),
                fire,
            },
            ..Action::default()
        }
    }
}

impl Policy for NearestKnownShape {
    fn decide(&mut self, obs: &Observation, now: Tick) -> Action {
        self.model.ingest_visible(obs);
        self.model.tick(now);

        if obs.own.entity.is_none() {
            return Action::idle();
        }
        let me = obs.own.pos;

        // Seen: same as the forgetful baseline.
        let seen = obs
            .visible
            .iter()
            .filter(|v| v.style.kind == Kind::Shape)
            .min_by(|a, b| {
                let da = a.position.pos.distance_squared(me);
                let db = b.position.pos.distance_squared(me);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(t) = seen {
            return Self::drive(toward(me, t.position.pos), obs.own.reload_ready);
        }

        // Remembered: nearest first, so the first shape track is the one.
        let side = self.centre.x * 2.0;
        if let Some(t) = self
            .model
            .tracks_near(me, side * 2.0)
            .into_iter()
            .find(|t| t.kind == Some(Kind::Shape))
        {
            return Self::drive(toward(me, t.last_pos), false);
        }

        // Unknown: the nearest edge of the map, else the middle of it.
        let goal = self
            .model
            .frontiers(me, 1)
            .first()
            .and_then(region_centre)
            .unwrap_or(self.centre);
        Self::drive(toward(me, goal), false)
    }

    fn footprint(&self) -> usize {
        self.model.footprint()
    }
}

fn region_centre(r: &Region) -> Option<Vec2> {
    match *r {
        Region::Disc { center, .. } | Region::Annulus { center, .. } => Some(center),
        Region::Rect { min, max } => Some(Vec2::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5)),
        Region::WholeArena => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schema::{
        AgentId, EntityId, EntityView, PhysicsGroup, PositionGroup, SelfView, StyleGroup,
        TeamGroup, TeamId,
    };

    fn own_at(pos: Vec2, alive: bool) -> SelfView {
        SelfView {
            agent: AgentId(0),
            team: TeamId::A,
            entity: alive.then_some(EntityId {
                index: 1,
                generation: 0,
            }),
            pos,
            vel: Vec2::ZERO,
            heading: 0.0,
            hp: 50.0,
            max_hp: 50.0,
            score: 0,
            level: 1,
            class: None,
            stats: Default::default(),
            points: 0,
            sense_radius: 120.0,
            comms_radius: 200.0,
            reload_ready: true,
            respawn_at: None,
        }
    }

    fn shape_at(index: u32, pos: Vec2) -> EntityView {
        EntityView {
            id: EntityId {
                index,
                generation: 0,
            },
            position: PositionGroup { pos, heading: 0.0 },
            physics: PhysicsGroup {
                vel: Vec2::ZERO,
                radius: 7.0,
            },
            style: StyleGroup {
                kind: Kind::Shape,
                tier: None,
                class: None,
                stats: None,
                focus: None,
            },
            team: TeamGroup {
                team: None,
                agent: None,
                owner: None,
            },
            health: None,
            score: None,
        }
    }

    fn obs(own: SelfView, visible: Vec<EntityView>) -> Observation {
        Observation {
            tick: Tick::ZERO,
            own,
            visible,
            scan: None,
            inbox: Vec::new(),
            commands: Vec::new(),
            scores: vec![0, 0],
        }
    }

    #[test]
    fn drives_at_the_nearest_shape_and_fires() {
        let me = Vec2::new(100.0, 100.0);
        let o = obs(
            own_at(me, true),
            vec![
                shape_at(2, Vec2::new(200.0, 100.0)),
                shape_at(3, Vec2::new(100.0, 130.0)),
            ],
        );
        let a = NearestShape::new(1000.0).decide(&o, Tick::ZERO);
        assert!(a.control.thrust.y > 0.99, "nearest shape is due south");
        assert!(a.control.fire);
    }

    #[test]
    fn heads_for_the_centre_when_nothing_is_visible() {
        let o = obs(own_at(Vec2::new(100.0, 100.0), true), Vec::new());
        let a = NearestShape::new(1000.0).decide(&o, Tick::ZERO);
        assert!(a.control.thrust.x > 0.7 && a.control.thrust.y > 0.7);
        assert!(!a.control.fire);
    }

    #[test]
    fn known_shape_is_remembered_after_it_leaves_view() {
        let me = Vec2::new(300.0, 300.0);
        let mut p = NearestKnownShape::new(1000.0);
        // Tick 0: a shape due east, in view.
        let o = obs(own_at(me, true), vec![shape_at(2, Vec2::new(400.0, 300.0))]);
        let a = p.decide(&o, Tick::ZERO);
        assert!(a.control.thrust.x > 0.99 && a.control.fire);

        // Tick 5: moved west, the shape is now out of sense radius and unseen.
        let far = Vec2::new(150.0, 300.0);
        let o = obs(own_at(far, true), Vec::new());
        let a = p.decide(&o, Tick(5));
        assert!(
            a.control.thrust.x > 0.99,
            "drives back toward the remembered shape"
        );
        assert!(!a.control.fire, "cannot shoot a memory");
        assert!(p.footprint() > 0);
    }

    #[test]
    fn idles_while_dead() {
        let o = obs(
            own_at(Vec2::new(100.0, 100.0), false),
            vec![shape_at(2, Vec2::new(150.0, 100.0))],
        );
        let a = NearestShape::new(1000.0).decide(&o, Tick::ZERO);
        assert_eq!(a, Action::idle());
    }
}

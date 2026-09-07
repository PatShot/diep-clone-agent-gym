//! What one decision is made from.
//!
//! Built once per decision from the observation and the world model, then read by
//! every predicate, drive and fire rule. Nothing here reaches past the observation.

use schema::{
    AgentId, Command, EntityId, Kind, ObjectiveHint, Observation, Role, ShapeTier, TeamId, Tick,
    Vec2,
};
use sim_core::constants::TANK_SENSE_RADIUS;
use sim_core::ArenaSpec;

use super::spec::TargetSpec;
use super::target::{self, Target};
use crate::model::{DenseGrid, WorldModel};

/// A seen or remembered entity, as a decision sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    pub id: EntityId,
    pub pos: Vec2,
    pub vel: Vec2,
    pub kind: Kind,
    pub tier: Option<ShapeTier>,
    pub team: Option<TeamId>,
    pub hp: Option<f32>,
    /// Seen this very decision. Only a visible contact can be shot.
    pub visible: bool,
    pub uncertainty: f32,
    pub age_ticks: u32,
}

/// Fixed places, worked out once from the arena.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landmarks {
    pub side: f32,
    pub centre: Vec2,
    /// The friendly control center. Home.
    pub home: Vec2,
    pub enemy_home: Vec2,
}

impl Landmarks {
    pub fn from_arena(arena: &ArenaSpec, team: TeamId) -> Self {
        Self {
            side: arena.side,
            centre: Vec2::new(arena.side * 0.5, arena.side * 0.5),
            home: arena.cc_pos(team),
            enemy_home: arena.cc_pos(team.other()),
        }
    }
}

/// What the control center has said, as far as this tank heard it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orders {
    pub role: Role,
    pub objective: Option<ObjectiveHint>,
    pub designated: Option<EntityId>,
    pub rally: Option<Vec2>,
}

impl Orders {
    pub fn new(role: Role) -> Self {
        Self {
            role,
            objective: None,
            designated: None,
            rally: None,
        }
    }

    /// Take in a command. Objective, designation and rally are held here; a
    /// reassignment of role or doctrine is returned for the policy to act on,
    /// and a reserved knob is reported so the policy can count it.
    pub fn take(&mut self, cmd: &Command, me: AgentId) -> Option<Reassign> {
        match cmd {
            Command::SetObjective { hint } => {
                self.objective = Some(*hint);
                None
            }
            Command::AssignRole { agent, role } if *agent == me => Some(Reassign::Role(*role)),
            Command::DesignateTarget { agent, target } if *agent == me => {
                self.designated = Some(*target);
                None
            }
            Command::SetRally { agent, pos } if *agent == me => {
                self.rally = Some(*pos);
                None
            }
            Command::SetDoctrine { agent, doctrine } if *agent == me => {
                Some(Reassign::Doctrine(doctrine.0.clone()))
            }
            Command::TuneDoctrine { agent, .. } if *agent == me => Some(Reassign::Reserved),
            _ => None,
        }
    }
}

/// What a command asks the policy itself to change.
#[derive(Debug, Clone, PartialEq)]
pub enum Reassign {
    Role(Role),
    Doctrine(String),
    /// A reserved command: seen, counted, not acted on.
    Reserved,
}

pub struct Ctx<'a> {
    pub now: Tick,
    pub me: Vec2,
    pub team: TeamId,
    pub heading: f32,
    pub hp_frac: f32,
    pub reload_ready: bool,
    pub sense_radius: f32,
    pub scores: [u32; 2],
    /// Enemy tanks recent enough to act on.
    pub enemies: Vec<Contact>,
    /// Friendly tanks whose position is known recently enough to steer by.
    pub teammates: Vec<Contact>,
    /// Everything the target scorer was allowed to consider.
    pub candidates: Vec<Contact>,
    pub target: Option<Target>,
    /// Where the teammates are, as a centroid — or where they last were, when none
    /// is in sight. Absent until a teammate has ever been seen.
    pub group: Option<Vec2>,
    pub orders: &'a Orders,
    pub landmarks: &'a Landmarks,
    pub model: &'a DenseGrid,
}

impl<'a> Ctx<'a> {
    pub fn build(
        obs: &Observation,
        model: &'a DenseGrid,
        orders: &'a Orders,
        landmarks: &'a Landmarks,
        spec: &TargetSpec,
        remembered_group: Option<Vec2>,
    ) -> Self {
        let now = obs.tick;
        let me = obs.own.pos;
        let team = obs.own.team;

        let mut enemies = Vec::new();
        let mut teammates = Vec::new();
        let mut candidates = Vec::new();

        for t in model.tracks_near(me, landmarks.side * 2.0) {
            let (Some(id), Some(kind)) = (t.id, t.kind) else {
                continue;
            };
            let age_ticks = now.0.saturating_sub(t.last_seen.0);
            let c = Contact {
                id,
                pos: t.last_pos,
                vel: t.vel_estimate,
                kind,
                tier: t.tier,
                team: t.team,
                hp: t.hp,
                visible: age_ticks == 0,
                uncertainty: t.uncertainty,
                age_ticks,
            };
            // A track whose disc has grown past a sense radius says where the
            // entity was, not where it is.
            let recent = c.uncertainty <= TANK_SENSE_RADIUS;
            match kind {
                Kind::Tank if c.team == Some(team) => {
                    if recent {
                        teammates.push(c);
                    }
                }
                Kind::Tank => {
                    if recent {
                        enemies.push(c);
                        if spec.enemies {
                            candidates.push(c);
                        }
                    }
                }
                Kind::Shape => {
                    if spec.remembered || c.visible {
                        candidates.push(c);
                    }
                }
                Kind::Bullet | Kind::ControlCenter => {}
            }
        }

        let scores = [
            obs.scores.first().copied().unwrap_or(0),
            obs.scores.get(1).copied().unwrap_or(0),
        ];
        let target = target::choose(
            &candidates,
            &enemies,
            me,
            orders.designated,
            spec,
            landmarks.side,
        );

        let group = if teammates.is_empty() {
            remembered_group
        } else {
            let n = teammates.len() as f32;
            let (x, y) = teammates
                .iter()
                .fold((0.0, 0.0), |(x, y), t| (x + t.pos.x, y + t.pos.y));
            Some(Vec2::new(x / n, y / n))
        };

        Self {
            now,
            me,
            team,
            heading: obs.own.heading,
            hp_frac: if obs.own.max_hp > 0.0 {
                (obs.own.hp / obs.own.max_hp).clamp(0.0, 1.0)
            } else {
                0.0
            },
            reload_ready: obs.own.reload_ready,
            sense_radius: obs.own.sense_radius,
            scores,
            enemies,
            teammates,
            candidates,
            target,
            group,
            orders,
            landmarks,
            model,
        }
    }

    pub fn nearest_enemy(&self) -> Option<&Contact> {
        self.enemies.iter().min_by(|a, b| {
            a.pos
                .distance_squared(self.me)
                .partial_cmp(&b.pos.distance_squared(self.me))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub fn my_score(&self) -> u32 {
        self.scores[self.team.0 as usize & 1]
    }

    pub fn their_score(&self) -> u32 {
        self.scores[self.team.other().0 as usize & 1]
    }
}

//! The doctrine policy: a `Policy` that reads a doctrine instead of being one.

use std::mem::size_of;
use std::sync::Arc;

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use schema::{Action, AgentId, Control, Observation, Role, TeamId, Tick, Vec2};
use sim_core::ArenaSpec;

use super::ctx::{Ctx, Landmarks, Orders, Reassign};
use super::fire::{self, BurstState};
use super::library::DoctrineLibrary;
use super::spec::{Doctrine, RoleBehaviour};
use super::{drive, predicate, DoctrineError};
use crate::geom::heading;
use crate::model::{DenseGrid, WorldModel};
use crate::Policy;

/// Decisions a wander direction is held for. Ten at 5 Hz is two seconds.
const WANDER_HOLD: u32 = 10;

pub struct DoctrinePolicy {
    library: Arc<DoctrineLibrary>,
    doctrine: Arc<Doctrine>,
    behaviour: RoleBehaviour,
    orders: Orders,
    landmarks: Landmarks,
    agent: AgentId,
    team: TeamId,
    model: DenseGrid,
    rng: StdRng,
    wander: Vec2,
    wander_left: u32,
    burst: BurstState,
    stance: Option<usize>,
    last_aim: f32,
    /// Where the teammates were last known to be, as a centroid.
    group: Option<Vec2>,
    /// Reassignments refused: a role this doctrine has no behaviour for, or a
    /// doctrine the library does not hold.
    refused: u32,
    /// Reserved commands seen and not acted on.
    reserved: u32,
}

impl DoctrinePolicy {
    /// A tank of `team` in `role`, following `doctrine` alone: a library of one,
    /// so `SetDoctrine` can only ever name this doctrine.
    pub fn new(
        doctrine: &Doctrine,
        role: Role,
        team: TeamId,
        arena: &ArenaSpec,
        seed: u64,
        agent: AgentId,
    ) -> Result<Self, DoctrineError> {
        let library = Arc::new(DoctrineLibrary::of(doctrine.clone()));
        Self::with_library(library, &doctrine.name, role, team, arena, seed, agent)
    }

    /// A tank of `team` in `role`, starting on the doctrine named `initial`,
    /// with the whole library available to `SetDoctrine`. The generator is
    /// seeded from the match seed and the agent, so two runs on one seed wander
    /// alike.
    pub fn with_library(
        library: Arc<DoctrineLibrary>,
        initial: &str,
        role: Role,
        team: TeamId,
        arena: &ArenaSpec,
        seed: u64,
        agent: AgentId,
    ) -> Result<Self, DoctrineError> {
        let doctrine = library
            .get(initial)
            .ok_or_else(|| DoctrineError::UnknownDoctrine(initial.to_string()))?
            .clone();
        let behaviour = doctrine
            .behaviour(role)
            .cloned()
            .ok_or(DoctrineError::NoBehaviour(role))?;
        let mix = (agent.0 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        Ok(Self {
            library,
            doctrine,
            behaviour,
            orders: Orders::new(role),
            landmarks: Landmarks::from_arena(arena, team),
            agent,
            team,
            model: DenseGrid::new(arena.side),
            rng: StdRng::seed_from_u64(seed ^ mix),
            wander: Vec2::ZERO,
            wander_left: 0,
            burst: BurstState::default(),
            stance: None,
            last_aim: 0.0,
            group: None,
            refused: 0,
            reserved: 0,
        })
    }

    pub fn role(&self) -> Role {
        self.orders.role
    }

    pub fn team(&self) -> TeamId {
        self.team
    }

    pub fn orders(&self) -> &Orders {
        &self.orders
    }

    pub fn model(&self) -> &DenseGrid {
        &self.model
    }

    pub fn doctrine(&self) -> &Doctrine {
        &self.doctrine
    }

    pub fn library(&self) -> &DoctrineLibrary {
        &self.library
    }

    /// Reassignments refused because the doctrine has no behaviour for the role,
    /// or the library has no doctrine of the name.
    pub fn refused(&self) -> u32 {
        self.refused
    }

    /// Reserved commands seen and not acted on.
    pub fn reserved(&self) -> u32 {
        self.reserved
    }

    /// Change role. The behaviour swaps, the burst and stance reset, and orders
    /// other than the role are kept: a rally point survives a reassignment.
    pub fn set_role(&mut self, role: Role) -> Result<(), DoctrineError> {
        let behaviour = self
            .doctrine
            .behaviour(role)
            .cloned()
            .ok_or(DoctrineError::NoBehaviour(role))?;
        self.behaviour = behaviour;
        self.orders.role = role;
        self.burst.reset();
        self.stance = None;
        Ok(())
    }

    /// Change doctrine, keeping the role. Refused when the library has no such
    /// doctrine or it has no behaviour for the current role, in which case
    /// nothing changes.
    pub fn set_doctrine(&mut self, name: &str) -> Result<(), DoctrineError> {
        let doctrine = self
            .library
            .get(name)
            .ok_or_else(|| DoctrineError::UnknownDoctrine(name.to_string()))?
            .clone();
        let behaviour = doctrine
            .behaviour(self.orders.role)
            .cloned()
            .ok_or(DoctrineError::NoBehaviour(self.orders.role))?;
        self.doctrine = doctrine;
        self.behaviour = behaviour;
        self.burst.reset();
        self.stance = None;
        Ok(())
    }

    fn take_commands(&mut self, obs: &Observation) {
        for cmd in &obs.commands {
            match self.orders.take(cmd, self.agent) {
                Some(Reassign::Role(role)) => {
                    if self.set_role(role).is_err() {
                        self.refused += 1;
                    }
                }
                Some(Reassign::Doctrine(name)) => {
                    if self.set_doctrine(&name).is_err() {
                        self.refused += 1;
                    }
                }
                Some(Reassign::Reserved) => self.reserved += 1,
                None => {}
            }
        }
    }
}

impl Policy for DoctrinePolicy {
    fn decide(&mut self, obs: &Observation, now: Tick) -> Action {
        self.take_commands(obs);
        self.model.ingest_visible(obs);
        self.model.tick(now);

        if obs.own.entity.is_none() {
            self.stance = None;
            return Action::idle();
        }

        if self.wander_left == 0 {
            let a: f32 = self.rng.random_range(0.0..std::f32::consts::TAU);
            self.wander = Vec2::new(a.cos(), a.sin());
            self.wander_left = WANDER_HOLD;
        }
        self.wander_left -= 1;

        let ctx = Ctx::build(
            obs,
            &self.model,
            &self.orders,
            &self.landmarks,
            &self.behaviour.target,
            self.group,
        );
        let group = ctx.group;

        let idx = self
            .behaviour
            .stances
            .iter()
            .position(|s| predicate::holds(&s.when, &ctx));
        self.stance = idx;
        let Some(idx) = idx else {
            return Action::idle();
        };
        let stance = &self.behaviour.stances[idx];

        let thrust = drive::thrust(&stance.drives, &self.behaviour.cohesion, &ctx, self.wander);
        let fire_spec = stance.fire.as_ref().unwrap_or(&self.behaviour.fire);
        let (fire, aim_at) = fire::decide(fire_spec, &ctx, &mut self.burst);
        let aim = aim_at.unwrap_or(if thrust == Vec2::ZERO {
            self.last_aim
        } else {
            heading(thrust)
        });
        self.last_aim = aim;
        self.group = group;

        Action {
            control: Control { thrust, aim, fire },
            ..Action::default()
        }
    }

    fn footprint(&self) -> usize {
        size_of::<Self>() + self.model.footprint()
    }

    fn stance(&self) -> Option<&str> {
        self.stance.map(|i| self.behaviour.stances[i].name.as_str())
    }
}

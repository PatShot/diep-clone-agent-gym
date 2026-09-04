//! The world and its tick.
//!
//! `World::step` is the whole simulation. It takes the actions agents returned and
//! the commands that arrived, advances one fixed step, and appends to an event
//! buffer the caller drains. It opens no files, reads no clock, and holds no
//! channels: the `events` crate consumes the buffer, the `server` crate owns the
//! transport, and neither concern reaches in here.
//!
//! # Tick order
//!
//! Fixed, and the order is load-bearing.
//!
//! 1. Commands drain. At the top of the tick, before physics, never mid-step.
//! 2. Actions apply: thrust, aim, fire.
//! 3. Integrate velocity, then position.
//! 4. Arena bounds and the base sanctuary rule.
//! 5. Broadphase, then contacts: separation and damage, collected not applied.
//! 6. Damage applies in collection order.
//! 7. Bullets past their lifetime expire.
//! 8. Regeneration, for anything undamaged long enough.
//! 9. The dead are reaped.
//! 10. Agents whose respawn has come due return.
//!
//! Damage is collected during step 5 and applied in step 6 rather than as contacts
//! are found. Otherwise an entity killed early in the sweep would stop dealing the
//! contact damage it was owed, and which of two simultaneous killers got credit
//! would depend on grid traversal order.
//!
//! # Determinism
//!
//! No wall-clock reads. One seeded generator. Every container walked here is
//! ordered by slot index or by key, never hashed. The agent table is a `BTreeMap`
//! for exactly that reason.

use std::collections::BTreeMap;

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use schema::{
    Action, AgentId, Cause, Command, EntityId, EntityView, Event, Kind, Kinematic, Observation,
    Origin, SelfView, ShapeTier, Stats, TeamId, Tick, Vec2,
};

use crate::arena::ArenaSpec;
use crate::constants::{
    BODY_DAMAGE_VS_PROJECTILE, BULLET_SPEED, CC_COMMS_RADIUS, DT, RECOIL_IMPULSE,
    REGEN_DELAY_TICKS, REGEN_FRACTION_PER_SEC, RELOAD_TICKS, RESPAWN_DELAY_TICKS,
    SEPARATION_STRENGTH, TANK_ACCEL, TANK_COMMS_RADIUS, TANK_RADIUS, TANK_SENSE_RADIUS,
};
use crate::entity::Entity;
use crate::grid::Grid;
use crate::store::Store;

/// How a match is set up. Everything a world needs to exist.
#[derive(Debug, Clone)]
pub struct WorldSpec {
    pub seed: u64,
    pub arena: ArenaSpec,
    /// Tanks per team. Five in the default configuration.
    pub tanks_per_team: usize,
    pub match_id: String,
    /// Pins the configuration that produced this run, so a replay that does not
    /// reproduce is detectable rather than merely suspected.
    pub config_hash: u64,
}

impl Default for WorldSpec {
    fn default() -> Self {
        Self {
            seed: 0,
            arena: ArenaSpec::default(),
            tanks_per_team: 5,
            match_id: "match".to_string(),
            config_hash: 0,
        }
    }
}

/// What an agent is and where it stands in the death-respawn cycle.
#[derive(Debug, Clone)]
pub struct AgentState {
    pub team: TeamId,
    /// The entity this agent drives. `None` while dead and awaiting respawn.
    pub entity: Option<EntityId>,
    pub respawn_at: Option<Tick>,
    pub is_cc: bool,
}

/// One tick's input. Actions from policies, commands from control centers.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    pub actions: Vec<(AgentId, Action)>,
    pub commands: Vec<(TeamId, Origin, Command)>,
}

/// A damaging contact, resolved but not yet applied.
#[derive(Debug, Clone, Copy)]
struct Hit {
    target: EntityId,
    /// The entity that made contact. A bullet, a tank, or a shape.
    source: EntityId,
    /// Who gets credit for a kill. A bullet's owner, otherwise the source itself.
    attributed: EntityId,
    amount: f32,
    /// One-shot exchanges mark the bullet so it cannot re-hit next tick.
    bullet: Option<EntityId>,
}

pub struct World {
    pub arena: ArenaSpec,
    store: Store,
    grid: Grid,
    tick: Tick,
    rng: StdRng,
    events: Vec<Event>,
    kinematics: Vec<Kinematic>,
    agents: BTreeMap<AgentId, AgentState>,
    scores: [u32; 2],
    spec: WorldSpec,
    /// Reused between ticks so the sweep does not allocate.
    hits: Vec<Hit>,
    scratch: Vec<EntityId>,
}

impl World {
    /// Build a world and populate it: a control center per team, then tanks.
    ///
    /// Shapes are not placed here. That is the spawner's job.
    pub fn new(spec: WorldSpec) -> Self {
        let mut w = Self {
            grid: Grid::new(spec.arena.side),
            arena: spec.arena.clone(),
            store: Store::new(),
            tick: Tick::ZERO,
            rng: StdRng::seed_from_u64(spec.seed),
            events: Vec::new(),
            kinematics: Vec::new(),
            agents: BTreeMap::new(),
            scores: [0, 0],
            hits: Vec::new(),
            scratch: Vec::new(),
            spec: spec.clone(),
        };

        w.events.push(Event::MatchStart {
            seed: spec.seed,
            config_hash: spec.config_hash,
            match_id: spec.match_id.clone(),
        });

        // Agent numbering is positional and stable: each team's tanks first, then
        // its control center. A policy assignment in config is an index into this.
        let mut next = 0u32;
        for team in [TeamId::A, TeamId::B] {
            for _ in 0..spec.tanks_per_team {
                let agent = AgentId(next);
                next += 1;
                w.add_tank(agent, team);
            }
            let agent = AgentId(next);
            next += 1;
            w.add_control_center(agent, team);
        }

        w.rebuild_grid();
        w
    }

    // -- Accessors ----------------------------------------------------------

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn scores(&self) -> [u32; 2] {
        self.scores
    }

    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.store.get(id)
    }

    pub fn entity_count(&self) -> usize {
        self.store.len()
    }

    pub fn agents(&self) -> &BTreeMap<AgentId, AgentState> {
        &self.agents
    }

    pub fn iter_entities(&self) -> impl Iterator<Item = (EntityId, &Entity)> {
        self.store.iter()
    }

    /// Complete world state, for a snapshot frame or a test.
    pub fn snapshot(&self) -> Vec<EntityView> {
        self.store.iter().map(|(id, e)| e.to_view(id)).collect()
    }

    /// Take the events accumulated since the last drain. The caller owns them from
    /// here; this crate keeps no copy.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Take this tick's kinematics rows. Kept apart from the event stream because
    /// position updates at 25 Hz across hundreds of entities would drown it.
    pub fn drain_kinematics(&mut self) -> Vec<Kinematic> {
        std::mem::take(&mut self.kinematics)
    }

    /// Add an entity and record the spawn.
    pub fn spawn(&mut self, e: Entity) -> EntityId {
        let (kind, pos, team, focus) = (e.kind, e.pos, e.team, e.focus);
        let id = self.store.insert(e);
        self.events.push(Event::Spawned {
            id,
            kind,
            pos,
            team,
            focus,
        });
        id
    }

    /// Mutable access to an entity.
    ///
    /// The objective crate writes score through here, and the spawner adjusts
    /// drift. Physics is this crate's business; ownership of an entity's meaning
    /// is not.
    pub fn entity_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.store.get_mut(id)
    }

    /// Add a tank for `agent`, placed at a scattered position in its own base.
    pub fn add_tank(&mut self, agent: AgentId, team: TeamId) -> EntityId {
        let (pos, heading) = self.respawn_pose(team);
        self.add_tank_at(agent, team, pos, heading)
    }

    /// Add a tank for `agent` at an exact pose, and register the agent.
    ///
    /// Registration is what makes an entity drivable: an action naming an
    /// unregistered agent is discarded, so a tank nobody registered simply drifts.
    pub fn add_tank_at(
        &mut self,
        agent: AgentId,
        team: TeamId,
        pos: Vec2,
        heading: f32,
    ) -> EntityId {
        let id = self.spawn(Entity::tank(agent, team, pos, heading));
        self.agents.insert(
            agent,
            AgentState {
                team,
                entity: Some(id),
                respawn_at: None,
                is_cc: false,
            },
        );
        id
    }

    /// Add a team's control center [CC] at the centre of its base.
    pub fn add_control_center(&mut self, agent: AgentId, team: TeamId) -> EntityId {
        let pos = self.arena.cc_pos(team);
        let id = self.spawn(Entity::control_center(agent, team, pos));
        self.agents.insert(
            agent,
            AgentState {
                team,
                entity: Some(id),
                respawn_at: None,
                is_cc: true,
            },
        );
        id
    }

    /// Score accrual lives in the objective crate. This is the hook it writes
    /// through, so that team totals stay on the entity that owns them.
    pub fn add_score(&mut self, team: TeamId, delta: u32) -> u32 {
        let t = &mut self.scores[team.0 as usize];
        *t = t.saturating_add(delta);
        *t
    }

    // -- The tick -----------------------------------------------------------

    pub fn step(&mut self, inputs: &Inputs) {
        self.events.push(Event::TickBegin { tick: self.tick });

        self.drain_commands(inputs);
        self.apply_actions(inputs);
        self.integrate();
        self.enforce_bounds();
        self.rebuild_grid();
        self.collect_contacts();
        self.apply_damage();
        self.expire_bullets();
        self.regenerate();
        self.reap();
        self.respawn_due();
        self.record_kinematics();

        self.tick = self.tick.next();
    }

    /// Step 1. Commands land on the tick they were drained on, not the tick they
    /// were sent, so a replay attributes them to the state they actually saw.
    fn drain_commands(&mut self, inputs: &Inputs) {
        for (team, origin, cmd) in &inputs.commands {
            self.events.push(Event::CommandIssued {
                team: *team,
                origin: *origin,
                cmd: *cmd,
            });
        }
    }

    /// Step 2. Thrust, aim, fire. Class and stat choices are ignored in v0: every
    /// tank is identical.
    fn apply_actions(&mut self, inputs: &Inputs) {
        for (agent, action) in &inputs.actions {
            self.events.push(Event::ActionSubmitted {
                agent: *agent,
                action: action.clone(),
                latency_us: 0,
            });

            let Some(state) = self.agents.get(agent) else {
                continue;
            };
            let (Some(id), false) = (state.entity, state.is_cc) else {
                continue;
            };
            let Some(e) = self.store.get_mut(id) else {
                continue;
            };

            // Thrust is a direction with magnitude, clamped to unit length. A
            // policy cannot buy speed by returning a longer vector.
            let t = action.control.thrust;
            let len = t.length();
            if len > 1e-6 {
                let s = if len > 1.0 { 1.0 / len } else { 1.0 };
                e.vel.x += t.x * s * TANK_ACCEL * DT;
                e.vel.y += t.y * s * TANK_ACCEL * DT;
            }
            e.heading = action.control.aim;

            if action.control.fire {
                self.try_fire(id);
            }
        }
    }

    fn try_fire(&mut self, id: EntityId) {
        let Some(e) = self.store.get(id) else { return };
        let ready = match e.last_fired {
            None => true,
            Some(t) => self.tick.age_since(t) >= RELOAD_TICKS,
        };
        if !ready {
            return;
        }
        let (Some(team), heading, pos, radius) = (e.team, e.heading, e.pos, e.radius) else {
            return;
        };

        let dir = Vec2::new(heading.cos(), heading.sin());
        // Muzzle sits clear of the hull so a bullet never starts inside its owner.
        let muzzle = Vec2::new(
            pos.x + dir.x * (radius + crate::constants::BULLET_RADIUS + 1.0),
            pos.y + dir.y * (radius + crate::constants::BULLET_RADIUS + 1.0),
        );
        // No velocity inheritance. PROVISIONAL: diep.io adds the firer's velocity,
        // but a fixed muzzle speed keeps bullet range a constant, which both the
        // scripted baselines and the after-match analysis rely on.
        let vel = Vec2::new(dir.x * BULLET_SPEED, dir.y * BULLET_SPEED);
        let bullet = Entity::bullet(id, team, muzzle, vel, self.tick);
        self.spawn(bullet);

        if let Some(e) = self.store.get_mut(id) {
            e.last_fired = Some(self.tick);
            e.vel.x -= dir.x * RECOIL_IMPULSE;
            e.vel.y -= dir.y * RECOIL_IMPULSE;
        }
    }

    /// Step 3. Velocity first, then position. Drag applies to tanks only: a bullet
    /// flies straight at muzzle speed for its whole life, and a shape drifts.
    fn integrate(&mut self) {
        for (_, e) in self.store.iter_mut() {
            if e.is_inert() {
                continue;
            }
            if e.is_tank() {
                e.vel.x *= crate::constants::DRAG;
                e.vel.y *= crate::constants::DRAG;
            }
            e.pos.x += e.vel.x * DT;
            e.pos.y += e.vel.y * DT;
        }
    }

    /// Step 4. Arena walls, then the sanctuary rule.
    ///
    /// A base is a sanctuary in two senses. An enemy tank is pushed back out at the
    /// boundary, and enemy fire is destroyed there rather than passing through.
    /// Shapes are kept out too, so a base never becomes a private farm.
    fn enforce_bounds(&mut self) {
        let arena = self.arena.clone();
        let mut absorbed: Vec<EntityId> = Vec::new();

        for (id, e) in self.store.iter_mut() {
            if e.is_inert() {
                continue;
            }

            for team in [TeamId::A, TeamId::B] {
                let foreign = match e.kind {
                    // A shape belongs to nobody, so every base is foreign to it.
                    Kind::Shape => true,
                    _ => e.team != Some(team),
                };
                if !foreign {
                    continue;
                }
                if e.is_bullet() {
                    if arena.base(team).overlaps_circle(e.pos, e.radius) {
                        absorbed.push(id);
                    }
                    continue;
                }
                let (p, moved_x, moved_y) = arena.eject_from_base(team, e.pos, e.radius);
                e.pos = p;
                if moved_x {
                    e.vel.x = 0.0;
                }
                if moved_y {
                    e.vel.y = 0.0;
                }
            }

            // The arena wall has the last word, after the sanctuary rule has had
            // its say. Ejection is already constrained to stay in bounds, so this
            // is the backstop for a configuration that puts a base somewhere the
            // default layout does not.
            let (p, hit_x, hit_y) = arena.clamp_circle(e.pos, e.radius);
            e.pos = p;
            if hit_x || hit_y {
                // A shape reflects. Anything else stops dead against the wall.
                //
                // Shapes have no steering, so a shape that stopped at a wall would
                // stay there for the rest of the match. Over ninety minutes every
                // shape ends up lining the edges, the arena centre empties, and the
                // contested nest quietly stops being contested.
                let bounce = if e.is_shape() { -1.0 } else { 0.0 };
                if hit_x {
                    e.vel.x *= bounce;
                }
                if hit_y {
                    e.vel.y *= bounce;
                }
            }
        }

        for id in absorbed {
            self.despawn(id, Cause::Absorbed);
        }
    }

    fn rebuild_grid(&mut self) {
        self.grid.clear();
        for (id, e) in self.store.iter() {
            if e.is_inert() {
                continue;
            }
            self.grid.insert(id, e.pos);
        }
    }

    /// Step 5. Separation and damage, collected rather than applied.
    fn collect_contacts(&mut self) {
        self.hits.clear();

        let mut pairs: Vec<(EntityId, EntityId)> = Vec::new();
        self.grid.for_each_pair(|a, b| pairs.push((a, b)));

        for (a, b) in pairs {
            let (Some(ea), Some(eb)) = (self.store.get(a), self.store.get(b)) else {
                continue;
            };

            let sum = ea.radius + eb.radius;
            let d2 = ea.pos.distance_squared(eb.pos);
            if d2 >= sum * sum {
                continue;
            }

            let same_team = ea.team.is_some() && ea.team == eb.team;
            let owns = ea.owner == Some(b) || eb.owner == Some(a);
            let bullets = (ea.is_bullet(), eb.is_bullet());
            let dmg_a = ea.contact_damage_against(eb);
            let dmg_b = eb.contact_damage_against(ea);
            let (owner_a, owner_b) = (ea.owner, eb.owner);

            // Friendly fire is off, and a bullet never touches its own firer.
            // Both cases pass straight through, with no separation either.
            if owns || (same_team && (bullets.0 || bullets.1)) {
                continue;
            }

            if !bullets.0 && !bullets.1 {
                self.separate(a, b, d2, sum);
            }

            if same_team {
                continue;
            }

            let one_shot = bullets.0 || bullets.1;

            // A one-shot exchange involves a bullet, and the bullet's memory of its
            // last target is what stops it charging full damage every tick it
            // spends inside the same hull.
            let bullet_id = if bullets.0 {
                Some(a)
            } else if bullets.1 {
                Some(b)
            } else {
                None
            };
            if let Some(bid) = bullet_id {
                let other = if bid == a { b } else { a };
                let bullet = self.store.get(bid).expect("checked above");
                if bullet.last_hit == Some(other) {
                    continue;
                }
            }

            let scale = if one_shot { 1.0 } else { DT };

            if dmg_a > 0.0 {
                self.hits.push(Hit {
                    target: b,
                    source: a,
                    attributed: owner_a.unwrap_or(a),
                    amount: dmg_a * scale,
                    bullet: bullet_id,
                });
            }
            if dmg_b > 0.0 {
                self.hits.push(Hit {
                    target: a,
                    source: b,
                    attributed: owner_b.unwrap_or(b),
                    amount: dmg_b * scale,
                    bullet: bullet_id,
                });
            }
        }
    }

    /// Push two overlapping circles apart, each by half the penetration, damped so
    /// contacts settle over a few ticks instead of snapping.
    fn separate(&mut self, a: EntityId, b: EntityId, d2: f32, sum: f32) {
        let d = d2.sqrt();
        let Some((ea, eb)) = self.store.get_pair_mut(a, b) else {
            return;
        };
        let (nx, ny) = if d > 1e-6 {
            ((eb.pos.x - ea.pos.x) / d, (eb.pos.y - ea.pos.y) / d)
        } else {
            // Exactly coincident. Any axis will do; pick one deterministically
            // rather than reaching for the generator.
            (1.0, 0.0)
        };
        let push = (sum - d) * 0.5 * SEPARATION_STRENGTH;
        ea.pos.x -= nx * push;
        ea.pos.y -= ny * push;
        eb.pos.x += nx * push;
        eb.pos.y += ny * push;
    }

    /// Step 6. Apply in collection order, skipping anything already dead. Order is
    /// a function of grid traversal and slot index, both stable.
    fn apply_damage(&mut self) {
        let hits = std::mem::take(&mut self.hits);
        for h in &hits {
            let Some(target) = self.store.get_mut(h.target) else {
                continue;
            };
            if target.is_inert() || target.hp <= 0.0 {
                continue;
            }
            target.hp -= h.amount;
            target.last_damaged = Some(self.tick);
            let remaining = target.hp.max(0.0);

            self.events.push(Event::Damaged {
                target: h.target,
                source: h.source,
                amount: h.amount,
                remaining,
            });

            if remaining <= 0.0 {
                self.events.push(Event::Killed {
                    target: h.target,
                    killer: h.attributed,
                });
            }

            if let Some(bid) = h.bullet {
                let other = if bid == h.target { h.source } else { h.target };
                if let Some(bullet) = self.store.get_mut(bid) {
                    bullet.last_hit = Some(other);
                }
            }
        }
        self.hits = hits;
        self.hits.clear();
    }

    /// Step 7. A bullet that outlives its lifetime is spent.
    fn expire_bullets(&mut self) {
        let now = self.tick;
        let expired: Vec<EntityId> = self
            .store
            .iter()
            .filter(|(_, e)| e.expires_at.is_some_and(|t| now >= t))
            .map(|(id, _)| id)
            .collect();
        for id in expired {
            self.despawn(id, Cause::Expired);
        }
    }

    /// Step 8. Regeneration, gated on a delay shorter than one reload interval.
    /// That relationship is what makes focus fire a threshold rather than a sum;
    /// see `constants::REGEN_DELAY_TICKS`.
    fn regenerate(&mut self) {
        let now = self.tick;
        for (_, e) in self.store.iter_mut() {
            if !e.is_tank() || e.hp <= 0.0 || e.hp >= e.max_hp {
                continue;
            }
            let quiet = match e.last_damaged {
                None => true,
                Some(t) => now.age_since(t) >= REGEN_DELAY_TICKS,
            };
            if !quiet {
                continue;
            }
            e.hp = (e.hp + e.max_hp * REGEN_FRACTION_PER_SEC * DT).min(e.max_hp);
        }
    }

    /// Step 9. Anything at zero health leaves, and an agent driving it starts its
    /// respawn timer.
    fn reap(&mut self) {
        let dead: Vec<EntityId> = self
            .store
            .iter()
            .filter(|(_, e)| !e.is_inert() && e.hp <= 0.0)
            .map(|(id, _)| id)
            .collect();
        for id in dead {
            self.despawn(id, Cause::Killed);
        }
    }

    fn despawn(&mut self, id: EntityId, cause: Cause) {
        let Some(e) = self.store.remove(id) else {
            return;
        };
        self.events.push(Event::Despawned { id, cause });

        if let Some(agent) = e.agent {
            if let Some(state) = self.agents.get_mut(&agent) {
                state.entity = None;
                state.respawn_at = Some(Tick(self.tick.0 + RESPAWN_DELAY_TICKS));
            }
        }
    }

    /// Step 10. Dead agents return to their own base.
    ///
    /// `docs/DESIGN.md` leaves open whether respawn should cost team score instead
    /// of time. A fixed delay is the placeholder.
    fn respawn_due(&mut self) {
        let now = self.tick;
        let due: Vec<(AgentId, TeamId)> = self
            .agents
            .iter()
            .filter(|(_, s)| {
                !s.is_cc && s.entity.is_none() && s.respawn_at.is_some_and(|t| now >= t)
            })
            .map(|(a, s)| (*a, s.team))
            .collect();

        for (agent, team) in due {
            let (pos, heading) = self.respawn_pose(team);
            let id = self.spawn(Entity::tank(agent, team, pos, heading));
            if let Some(state) = self.agents.get_mut(&agent) {
                state.entity = Some(id);
                state.respawn_at = None;
            }
        }
    }

    /// A scattered position inside a team's base, facing the arena centre.
    fn respawn_pose(&mut self, team: TeamId) -> (Vec2, f32) {
        let base = *self.arena.base(team);
        let pad = TANK_RADIUS + 2.0;
        let x = self
            .rng
            .random_range((base.min.x + pad)..(base.max.x - pad));
        let y = self
            .rng
            .random_range((base.min.y + pad)..(base.max.y - pad));
        let center = Vec2::new(self.arena.side * 0.5, self.arena.side * 0.5);
        let heading = (center.y - y).atan2(center.x - x);
        (Vec2::new(x, y), heading)
    }

    fn record_kinematics(&mut self) {
        let tick = self.tick;
        self.kinematics
            .extend(self.store.iter().filter_map(|(id, e)| {
                if e.is_inert() {
                    return None;
                }
                Some(Kinematic {
                    tick,
                    entity: id,
                    pos: e.pos,
                    vel: e.vel,
                    heading: e.heading,
                })
            }));
    }

    // -- Observation --------------------------------------------------------

    /// What one agent sees this tick. Ground truth, but only inside its sense
    /// radius. Nothing outside this struct reaches a policy.
    pub fn observe(&mut self, agent: AgentId) -> Option<Observation> {
        let state = self.agents.get(&agent)?.clone();
        let own = self.self_view(agent, &state);

        let (origin, radius) = match state.entity.and_then(|id| self.store.get(id)) {
            Some(e) if !state.is_cc => (e.pos, TANK_SENSE_RADIUS),
            // A control center sees its own base and no further. It coordinates on
            // what it is told, which is the point of having one.
            _ => (self.arena.cc_pos(state.team), 0.0),
        };

        let mut visible = Vec::new();
        if radius > 0.0 {
            self.grid.query_radius(origin, radius, &mut self.scratch);
            for &id in &self.scratch {
                let Some(e) = self.store.get(id) else {
                    continue;
                };
                if e.pos.distance_squared(origin) <= (radius + e.radius) * (radius + e.radius) {
                    visible.push(e.to_view(id));
                }
            }
            // Slot order, so two runs on one seed produce identical observations.
            visible.sort_by_key(|v| (v.id.index, v.id.generation));
        }

        Some(Observation {
            tick: self.tick,
            own,
            visible,
            scan: None,
            inbox: Vec::new(),
            commands: Vec::new(),
            scores: self.scores.to_vec(),
        })
    }

    fn self_view(&self, agent: AgentId, state: &AgentState) -> SelfView {
        let comms = if state.is_cc {
            CC_COMMS_RADIUS
        } else {
            TANK_COMMS_RADIUS
        };
        let sense = if state.is_cc { 0.0 } else { TANK_SENSE_RADIUS };

        match state.entity.and_then(|id| self.store.get(id)) {
            Some(e) => SelfView {
                agent,
                team: state.team,
                entity: state.entity,
                pos: e.pos,
                vel: e.vel,
                heading: e.heading,
                hp: e.hp,
                max_hp: e.max_hp,
                score: e.score,
                level: e.level,
                class: e.class,
                stats: e.stats,
                points: 0,
                sense_radius: sense,
                comms_radius: comms,
                // `is_none_or` would read better but is stable only from 1.82,
                // and the workspace pins 1.80.
                reload_ready: !e
                    .last_fired
                    .is_some_and(|t| self.tick.age_since(t) < RELOAD_TICKS),
                respawn_at: None,
            },
            // Dead. The agent still gets a view, so a policy can plan its return.
            None => SelfView {
                agent,
                team: state.team,
                entity: None,
                pos: self.arena.cc_pos(state.team),
                vel: Vec2::ZERO,
                heading: 0.0,
                hp: 0.0,
                max_hp: crate::entity::tank_max_hp(1),
                score: 0,
                level: 1,
                class: None,
                stats: Stats::default(),
                points: 0,
                sense_radius: sense,
                comms_radius: comms,
                reload_ready: false,
                respawn_at: state.respawn_at,
            },
        }
    }

    /// Place a shape. The spawner calls this; the world does not decide where
    /// shapes belong.
    pub fn spawn_shape(
        &mut self,
        tier: ShapeTier,
        pos: Vec2,
        vel: Vec2,
        focus: Option<schema::FocusId>,
    ) -> EntityId {
        self.spawn(Entity::shape(tier, pos, vel, focus))
    }

    /// The generator driving world randomness. Agent policies get their own
    /// streams; this one is the world's.
    pub fn rng(&mut self) -> &mut StdRng {
        &mut self.rng
    }

    pub fn spec(&self) -> &WorldSpec {
        &self.spec
    }
}

/// Body damage a tank deals to a bullet. Exposed for tests that assert the
/// penetration arithmetic in `docs/STATS.md`.
pub const TANK_VS_BULLET: f32 = BODY_DAMAGE_VS_PROJECTILE;

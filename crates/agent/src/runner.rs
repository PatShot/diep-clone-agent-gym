//! The match loop.
//!
//! One tick: spawner, then (on a decision tick) observe and decide for every
//! seat, then physics, then the event bus. Commands drain at the top of the tick
//! inside `World::step`; here they are collected from control center policies and
//! delivered to teammates on the following decision tick.
//!
//! Two things this loop measures but does not enforce, on purpose.
//!
//! The decision time budget. `docs/DESIGN.md` §Budgets wants an overrun to cost
//! the tank its action. Enforcing that in lockstep would make a match's outcome
//! depend on the speed of the machine running it, which is the one thing a
//! recorded match must not do. The wall time is kept per seat for the summary and
//! nothing reads it in simulation.
//!
//! Command range. Commands reach every teammate. The comms model owns the question
//! of who is in range and it is deferred, so the runner does not pre-empt it with a
//! radius check of its own.

use std::collections::BTreeMap;
use std::time::Instant;

use events::{Bus, TickRecord};
use rand::rngs::StdRng;
use rand::SeedableRng;
use schema::{Action, AgentId, Command, EntityId, Event, Inputs, Origin, TeamId, Tick};
use sim_core::constants::TICK_HZ;
use sim_core::{World, WorldSpec};
use spawn::{CompositeSpawner, SpawnConfig, Spawner};

use crate::baseline::Idle;
use crate::{Policy, DECISION_HZ};

#[derive(Debug, Clone)]
pub struct RunnerConfig {
    /// Physics ticks per decision. Five at the default rates.
    pub decision_every: u32,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            decision_every: TICK_HZ / DECISION_HZ,
        }
    }
}

/// What a run produced, for the caller that started it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MatchSummary {
    pub ticks: u32,
    pub scores: [u32; 2],
    /// Kills credited to an agent of each team, shapes and tanks alike.
    pub kills: [u32; 2],
    pub events: u64,
    pub decisions: u64,
    pub latency_mean_us: u64,
    pub latency_max_us: u32,
    /// The largest memory footprint any seat reported at the end of the run.
    pub footprint_max: usize,
}

struct Seat {
    team: TeamId,
    is_cc: bool,
    policy: Box<dyn Policy>,
    /// The action in force until the next decision tick.
    held: Action,
    /// The stance last reported, so a change is an event and a repeat is not.
    last_stance: Option<String>,
    decisions: u64,
    latency_total_us: u64,
    latency_max_us: u32,
}

pub struct Runner {
    world: World,
    spawner: Box<dyn Spawner>,
    rng: StdRng,
    bus: Bus,
    seats: BTreeMap<AgentId, Seat>,
    /// Commands issued on the last decision tick, delivered on the next.
    pending: Vec<(TeamId, Command)>,
    decision_every: u32,
    kills: [u32; 2],
    events: u64,
    /// Decisions spent in each named stance, per seat. A policy that names no
    /// stance appears nowhere.
    stances: BTreeMap<AgentId, BTreeMap<String, u32>>,
    /// Events the runner itself raises on a decision tick, published ahead of the
    /// world's events for that tick because the decision came first.
    raised: Vec<Event>,
}

impl Runner {
    /// Build the world, prefill it, and seat an [`Idle`] policy on every agent.
    ///
    /// The spawner's generator is seeded from the match seed, as `record_match`
    /// did, so the arena a policy meets is a function of the seed alone.
    pub fn new(spec: WorldSpec, spawn: SpawnConfig, bus: Bus, config: RunnerConfig) -> Self {
        let seed = spec.seed;
        let mut world = World::new(spec);
        let mut spawner = CompositeSpawner::new(spawn);
        let mut rng = StdRng::seed_from_u64(seed);
        let seeded = spawner.prefill(&world, &mut rng);
        spawn::apply(&mut world, &seeded);

        let seats = world
            .agents()
            .iter()
            .map(|(agent, state)| {
                (
                    *agent,
                    Seat {
                        team: state.team,
                        is_cc: state.is_cc,
                        policy: Box::new(Idle),
                        held: Action::idle(),
                        last_stance: None,
                        decisions: 0,
                        latency_total_us: 0,
                        latency_max_us: 0,
                    },
                )
            })
            .collect();

        Self {
            world,
            spawner: Box::new(spawner),
            rng,
            bus,
            seats,
            pending: Vec::new(),
            decision_every: config.decision_every.max(1),
            kills: [0, 0],
            events: 0,
            stances: BTreeMap::new(),
            raised: Vec::new(),
        }
    }

    /// Decisions spent in each named stance, per seat.
    pub fn stance_histogram(&self) -> &BTreeMap<AgentId, BTreeMap<String, u32>> {
        &self.stances
    }

    /// Seat a policy. Unknown agents are ignored.
    pub fn set_policy(&mut self, agent: AgentId, policy: Box<dyn Policy>) {
        if let Some(seat) = self.seats.get_mut(&agent) {
            seat.policy = policy;
        }
    }

    /// Every seat: agent, team, and whether it is a control center.
    pub fn seats(&self) -> impl Iterator<Item = (AgentId, TeamId, bool)> + '_ {
        self.seats.iter().map(|(a, s)| (*a, s.team, s.is_cc))
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn tick(&self) -> Tick {
        self.world.tick()
    }

    /// Advance one physics tick.
    pub fn step(&mut self) {
        self.advance(false);
    }

    /// Run `ticks` ticks, emit `MatchEnd` on the last, and flush the bus.
    ///
    /// The winner is whichever team leads on score, which is a placeholder until
    /// the `objective` crate decides what ending a match means.
    pub fn run(&mut self, ticks: u32) -> MatchSummary {
        for i in 0..ticks {
            self.advance(i + 1 == ticks);
        }
        self.bus.flush();
        self.summary(ticks)
    }

    pub fn summary(&self, ticks: u32) -> MatchSummary {
        let decisions: u64 = self.seats.values().map(|s| s.decisions).sum();
        let total: u64 = self.seats.values().map(|s| s.latency_total_us).sum();
        MatchSummary {
            ticks,
            scores: self.world.scores(),
            kills: self.kills,
            events: self.events,
            decisions,
            latency_mean_us: if decisions == 0 { 0 } else { total / decisions },
            latency_max_us: self
                .seats
                .values()
                .map(|s| s.latency_max_us)
                .max()
                .unwrap_or(0),
            footprint_max: self
                .seats
                .values()
                .map(|s| s.policy.footprint())
                .max()
                .unwrap_or(0),
        }
    }

    fn advance(&mut self, last: bool) {
        let now = self.world.tick();

        let requests = self.spawner.tick(&self.world, now, &mut self.rng);
        spawn::apply(&mut self.world, &requests);

        let commands = if now.0 % self.decision_every == 0 {
            self.decide_all(now)
        } else {
            Vec::new()
        };

        let inputs = Inputs {
            actions: self
                .seats
                .iter()
                .map(|(a, s)| (*a, s.held.clone()))
                .collect(),
            commands,
        };
        self.world.step(&inputs);

        let mut ev = std::mem::take(&mut self.raised);
        ev.extend(self.world.drain_events());
        if last {
            ev.push(Event::MatchEnd {
                winner: self.leader(),
                tick: self.world.tick(),
            });
        }
        self.tally(&ev);
        self.bus.publish(&TickRecord {
            tick: now,
            events: &ev,
            inputs: &inputs,
            scores: self.world.scores(),
        });
    }

    /// Observe and decide for every seat, in agent order. Returns the commands
    /// control centers issued, for the record.
    fn decide_all(&mut self, now: Tick) -> Vec<(TeamId, Origin, Command)> {
        let pending = std::mem::take(&mut self.pending);
        let mut issued = Vec::new();
        let agents: Vec<AgentId> = self.seats.keys().copied().collect();

        for agent in agents {
            let Some(mut obs) = self.world.observe(agent) else {
                continue;
            };
            let seat = self.seats.get_mut(&agent).expect("seat exists");

            if !seat.is_cc {
                obs.commands = pending
                    .iter()
                    .filter(|(team, cmd)| {
                        *team == seat.team && addressed_to(cmd).map_or(true, |a| a == agent)
                    })
                    .map(|(_, cmd)| cmd.clone())
                    .collect();
            }

            let start = Instant::now();
            let action = seat.policy.decide(&obs, now);
            let us = start.elapsed().as_micros().min(u32::MAX as u128) as u32;
            seat.decisions += 1;
            seat.latency_total_us += us as u64;
            seat.latency_max_us = seat.latency_max_us.max(us);

            let stance = seat.policy.stance();
            if let Some(name) = stance {
                let per_seat = self.stances.entry(agent).or_default();
                match per_seat.get_mut(name) {
                    Some(n) => *n += 1,
                    None => {
                        per_seat.insert(name.to_string(), 1);
                    }
                }
            }
            if stance != seat.last_stance.as_deref() {
                if let Some(name) = stance {
                    self.raised.push(Event::StanceChanged {
                        agent,
                        stance: name.to_string(),
                        tick: now,
                    });
                }
                seat.last_stance = stance.map(str::to_owned);
            }

            // Only a control center may command. A tank's commands are dropped.
            if seat.is_cc {
                for cmd in &action.commands {
                    issued.push((seat.team, Origin::Policy, cmd.clone()));
                    self.pending.push((seat.team, cmd.clone()));
                }
            }
            seat.held = action;
        }
        issued
    }

    fn tally(&mut self, ev: &[Event]) {
        self.events += ev.len() as u64;
        for e in ev {
            if let Event::Killed { killer, .. } = e {
                if let Some(team) = self.team_of_entity(*killer) {
                    self.kills[team.0 as usize] += 1;
                }
            }
        }
    }

    fn team_of_entity(&self, id: EntityId) -> Option<TeamId> {
        self.world
            .agents()
            .values()
            .find(|s| s.entity == Some(id))
            .map(|s| s.team)
    }

    fn leader(&self) -> Option<TeamId> {
        let [a, b] = self.world.scores();
        match a.cmp(&b) {
            std::cmp::Ordering::Greater => Some(TeamId::A),
            std::cmp::Ordering::Less => Some(TeamId::B),
            std::cmp::Ordering::Equal => None,
        }
    }
}

/// The agent a command names, or `None` for a team-wide command.
fn addressed_to(cmd: &Command) -> Option<AgentId> {
    match cmd {
        Command::SetObjective { .. } => None,
        Command::AssignRole { agent, .. }
        | Command::DesignateTarget { agent, .. }
        | Command::SetRally { agent, .. }
        | Command::RequestReport { agent }
        | Command::SetDoctrine { agent, .. }
        | Command::TuneDoctrine { agent, .. } => Some(*agent),
    }
}

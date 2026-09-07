//! The runner, end to end: determinism, cadence, command delivery, and the first
//! kill.

use std::sync::{Arc, Mutex};

use agent::{Idle, NearestShape, Policy, Runner, RunnerConfig};
use events::{Bus, Sink, TickRecord};
use schema::{Action, AgentId, Command, Event, Observation, Role, Tick};
use sim_core::constants::ARENA_SIDE;
use sim_core::WorldSpec;
use spawn::SpawnConfig;

/// Keeps every event, for comparison.
struct Collect(Arc<Mutex<Vec<Event>>>);

impl Sink for Collect {
    fn accept(&mut self, rec: &TickRecord<'_>) {
        self.0.lock().unwrap().extend(rec.events.iter().cloned());
    }
    fn flush(&mut self) {}
}

fn runner(seed: u64) -> (Runner, Arc<Mutex<Vec<Event>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let bus = Bus::new().with(Box::new(Collect(log.clone())));
    let spec = WorldSpec {
        seed,
        ..WorldSpec::default()
    };
    let r = Runner::new(spec, SpawnConfig::default(), bus, RunnerConfig::default());
    (r, log)
}

fn farm_everywhere(r: &mut Runner) {
    let seats: Vec<_> = r.seats().collect();
    for (agent, _, is_cc) in seats {
        if !is_cc {
            r.set_policy(agent, Box::new(NearestShape::new(ARENA_SIDE)));
        }
    }
}

#[test]
fn one_seed_two_runs_one_event_stream() {
    let mut streams = Vec::new();
    for _ in 0..2 {
        let (mut r, log) = runner(11);
        farm_everywhere(&mut r);
        r.run(300);
        let events = log.lock().unwrap().clone();
        streams.push(schema::to_line(&events).unwrap());
    }
    assert_eq!(streams[0], streams[1]);
    assert!(streams[0].len() > 1000, "a run produces events");
}

#[test]
fn actions_are_held_between_decision_ticks() {
    let (mut r, log) = runner(3);
    farm_everywhere(&mut r);
    r.run(10);
    let events = log.lock().unwrap().clone();

    // Agent 0's submitted action, tick by tick.
    let submitted: Vec<Action> = events
        .iter()
        .filter_map(|e| match e {
            Event::ActionSubmitted { agent, action, .. } if *agent == AgentId(0) => {
                Some(action.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(submitted.len(), 10);
    for t in 1..5 {
        assert_eq!(submitted[t], submitted[0], "held through tick {t}");
    }
    // A new decision at tick 5 may differ, and from then on holds again.
    for t in 6..10 {
        assert_eq!(submitted[t], submitted[5], "held through tick {t}");
    }
}

/// A control center that issues one command on its first decision.
struct Once(Option<Command>);

impl Policy for Once {
    fn decide(&mut self, _obs: &Observation, _now: Tick) -> Action {
        Action {
            commands: self.0.take().into_iter().collect(),
            ..Action::default()
        }
    }
}

/// What a recording tank was handed, decision by decision.
type Seen = Arc<Mutex<Vec<(Tick, Vec<Command>)>>>;

/// A tank that writes down every command it is handed.
struct Recorder(Seen);

impl Policy for Recorder {
    fn decide(&mut self, obs: &Observation, now: Tick) -> Action {
        self.0.lock().unwrap().push((now, obs.commands.clone()));
        Action::idle()
    }
}

#[test]
fn control_center_commands_reach_teammates_on_the_next_decision_tick() {
    let (mut r, log) = runner(5);
    let seats: Vec<_> = r.seats().collect();
    let cc_a = seats
        .iter()
        .find(|(_, team, is_cc)| *is_cc && *team == schema::TeamId::A)
        .map(|(a, _, _)| *a)
        .unwrap();
    let cmd = Command::AssignRole {
        agent: AgentId(0),
        role: Role::Scout,
    };
    r.set_policy(cc_a, Box::new(Once(Some(cmd.clone()))));

    let seen_by_0 = Arc::new(Mutex::new(Vec::new()));
    let seen_by_1 = Arc::new(Mutex::new(Vec::new()));
    let seen_by_6 = Arc::new(Mutex::new(Vec::new()));
    r.set_policy(AgentId(0), Box::new(Recorder(seen_by_0.clone())));
    r.set_policy(AgentId(1), Box::new(Recorder(seen_by_1.clone())));
    // Agent 6 is team B's first tank.
    r.set_policy(AgentId(6), Box::new(Recorder(seen_by_6.clone())));
    r.run(11);

    let got = |log: &Seen| -> Vec<(Tick, Vec<Command>)> {
        log.lock()
            .unwrap()
            .iter()
            .filter(|(_, c)| !c.is_empty())
            .cloned()
            .collect()
    };
    assert_eq!(got(&seen_by_0), vec![(Tick(5), vec![cmd.clone()])]);
    assert!(got(&seen_by_1).is_empty(), "addressed to agent 0 only");
    assert!(got(&seen_by_6).is_empty(), "other team");

    // And it was recorded at the tick it was issued.
    let issued = log
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, Event::CommandIssued { cmd: c, .. } if *c == cmd));
    assert!(issued);
}

#[test]
fn idle_tanks_kill_nothing_and_farmers_kill_something() {
    let (mut idle, _) = runner(7);
    let s = idle.run(500);
    assert_eq!(s.kills, [0, 0]);

    let (mut r, _) = runner(7);
    farm_everywhere(&mut r);
    let s = r.run(1500);
    assert!(
        s.kills[0] + s.kills[1] > 0,
        "sixty seconds of farming kills something"
    );
    assert_eq!(s.decisions, 12 * 300);
}

#[test]
fn default_policy_is_idle() {
    let own = schema::SelfView {
        agent: AgentId(0),
        team: schema::TeamId::A,
        entity: None,
        pos: schema::Vec2::ZERO,
        vel: schema::Vec2::ZERO,
        heading: 0.0,
        hp: 0.0,
        max_hp: 50.0,
        score: 0,
        level: 1,
        class: None,
        stats: Default::default(),
        points: 0,
        sense_radius: 120.0,
        comms_radius: 200.0,
        reload_ready: false,
        respawn_at: None,
    };
    let obs = Observation {
        tick: Tick::ZERO,
        own,
        visible: Vec::new(),
        scan: None,
        inbox: Vec::new(),
        commands: Vec::new(),
        scores: vec![0, 0],
    };
    assert_eq!(Idle.decide(&obs, Tick::ZERO), Action::idle());

    let (mut r, _) = runner(1);
    r.step();
    assert_eq!(r.tick(), Tick(1));
}

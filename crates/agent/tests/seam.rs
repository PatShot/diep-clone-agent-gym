//! The command seam a control center — later a language model — steers through:
//! `SetDoctrine` switches, `TuneDoctrine` is seen and held, and every stance
//! change is a row.

use std::sync::{Arc, Mutex};

use agent::doctrine::DoctrineLibrary;
use agent::{Doctrine, DoctrinePolicy, Policy, Runner, RunnerConfig};
use events::{Bus, Sink, TickRecord};
use schema::{
    Action, AgentId, Command, DoctrineId, EntityId, Event, Knob, Observation, Role, SelfView,
    TeamId, Tick, Vec2,
};
use sim_core::{ArenaSpec, WorldSpec};
use spawn::SpawnConfig;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/doctrine");

fn library() -> Arc<DoctrineLibrary> {
    Arc::new(DoctrineLibrary::from_dir(DIR).expect("shipped doctrines"))
}

fn observation(tick: u32, commands: Vec<Command>) -> Observation {
    Observation {
        tick: Tick(tick),
        own: SelfView {
            agent: AgentId(4),
            team: TeamId::A,
            entity: Some(EntityId {
                index: 4,
                generation: 0,
            }),
            pos: Vec2::new(300.0, 300.0),
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
        },
        visible: Vec::new(),
        scan: None,
        inbox: Vec::new(),
        commands,
        scores: vec![0, 0],
    }
}

#[test]
fn the_library_holds_the_shipped_doctrines_by_name() {
    let lib = library();
    let names: Vec<&str> = lib.names().collect();
    assert_eq!(names, vec!["aggressive", "baseline", "defensive"]);
    assert!(lib.get("aggressive").is_some());
    assert!(lib.get("nope").is_none());
}

#[test]
fn set_doctrine_switches_when_the_role_exists_and_refuses_when_it_does_not() {
    let lib = library();
    let arena = ArenaSpec::default();
    // A scout exists in both shipped doctrines; a screen only in defensive.
    let mut scout = DoctrinePolicy::with_library(
        lib.clone(),
        "defensive",
        Role::Scout,
        TeamId::A,
        &arena,
        1,
        AgentId(4),
    )
    .unwrap();
    assert_eq!(scout.doctrine().name, "defensive");

    scout.decide(
        &observation(
            5,
            vec![Command::SetDoctrine {
                agent: AgentId(4),
                doctrine: DoctrineId::from("aggressive"),
            }],
        ),
        Tick(5),
    );
    assert_eq!(scout.doctrine().name, "aggressive");
    assert_eq!(scout.role(), Role::Scout, "the role survives the switch");
    assert_eq!(scout.refused(), 0);

    // Addressed to someone else: ignored.
    scout.decide(
        &observation(
            10,
            vec![Command::SetDoctrine {
                agent: AgentId(0),
                doctrine: DoctrineId::from("defensive"),
            }],
        ),
        Tick(10),
    );
    assert_eq!(scout.doctrine().name, "aggressive");

    // Unknown name: refused and counted, nothing changes.
    scout.decide(
        &observation(
            15,
            vec![Command::SetDoctrine {
                agent: AgentId(4),
                doctrine: DoctrineId::from("cautious"),
            }],
        ),
        Tick(15),
    );
    assert_eq!(scout.doctrine().name, "aggressive");
    assert_eq!(scout.refused(), 1);

    // A screen cannot become aggressive: that doctrine has no screen.
    let mut screen = DoctrinePolicy::with_library(
        lib,
        "defensive",
        Role::Screen,
        TeamId::A,
        &arena,
        1,
        AgentId(4),
    )
    .unwrap();
    screen.decide(
        &observation(
            5,
            vec![Command::SetDoctrine {
                agent: AgentId(4),
                doctrine: DoctrineId::from("aggressive"),
            }],
        ),
        Tick(5),
    );
    assert_eq!(screen.doctrine().name, "defensive");
    assert_eq!(screen.refused(), 1);
}

#[test]
fn tune_doctrine_is_seen_and_held() {
    let lib = library();
    let arena = ArenaSpec::default();
    let mut p = DoctrinePolicy::with_library(
        lib,
        "defensive",
        Role::Screen,
        TeamId::A,
        &arena,
        1,
        AgentId(4),
    )
    .unwrap();
    let before = p.doctrine().clone();
    p.decide(
        &observation(
            5,
            vec![Command::TuneDoctrine {
                agent: AgentId(4),
                knob: Knob::Cohesion,
                value: 0.9,
            }],
        ),
        Tick(5),
    );
    assert_eq!(p.reserved(), 1);
    assert_eq!(p.doctrine(), &before, "reserved means nothing changed");
}

#[test]
fn the_new_variants_survive_the_wire() {
    for cmd in [
        Command::SetDoctrine {
            agent: AgentId(3),
            doctrine: DoctrineId::from("aggressive"),
        },
        Command::TuneDoctrine {
            agent: AgentId(3),
            knob: Knob::ExploreBias,
            value: 0.25,
        },
    ] {
        let line = schema::to_line(&cmd).unwrap();
        let back: Command = schema::from_line(&line).unwrap();
        assert_eq!(back, cmd);
    }
    let line = schema::to_line(&Command::SetDoctrine {
        agent: AgentId(3),
        doctrine: DoctrineId::from("aggressive"),
    })
    .unwrap();
    assert!(line.contains(r#""doctrine":"aggressive""#), "{line}");

    let ev = Event::StanceChanged {
        agent: AgentId(2),
        stance: "engage".into(),
        tick: Tick(40),
    };
    let line = schema::to_line(&ev).unwrap();
    let back: Event = schema::from_line(&line).unwrap();
    assert_eq!(back, ev);
    assert_eq!(ev.kind_str(), "stance_changed");
}

struct Collect(Arc<Mutex<Vec<(Tick, Event)>>>);

impl Sink for Collect {
    fn accept(&mut self, rec: &TickRecord<'_>) {
        let mut log = self.0.lock().unwrap();
        for e in rec.events {
            log.push((rec.tick, e.clone()));
        }
    }
    fn flush(&mut self) {}
}

#[test]
fn a_stance_change_is_an_event_and_a_repeat_is_not() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let bus = Bus::new().with(Box::new(Collect(log.clone())));
    let spec = WorldSpec {
        seed: 33,
        ..WorldSpec::default()
    };
    let mut r = Runner::new(spec, SpawnConfig::default(), bus, RunnerConfig::default());
    let lib = library();
    let a = lib.seat(&mut r, TeamId::A, "defensive").unwrap();
    lib.seat(&mut r, TeamId::B, "aggressive").unwrap();
    r.run(750);

    let events = log.lock().unwrap().clone();
    let screen = a[0].0;
    let changes: Vec<(Tick, String)> = events
        .iter()
        .filter_map(|(t, e)| match e {
            Event::StanceChanged { agent, stance, .. } if *agent == screen => {
                Some((*t, stance.clone()))
            }
            _ => None,
        })
        .collect();
    assert!(!changes.is_empty());
    assert_eq!(
        changes[0].0,
        Tick(0),
        "the first stance is a change from nothing"
    );
    for w in changes.windows(2) {
        assert_ne!(w[0].1, w[1].1, "consecutive rows differ: {changes:?}");
    }

    // The runner's count and the event stream agree on what stances existed.
    let counted: std::collections::BTreeSet<&String> =
        r.stance_histogram()[&screen].keys().collect();
    let emitted: std::collections::BTreeSet<&String> = changes.iter().map(|(_, s)| s).collect();
    assert_eq!(counted, emitted);

    // And the plain baseline names no stance, so it raises none.
    let log2 = Arc::new(Mutex::new(Vec::new()));
    let bus = Bus::new().with(Box::new(Collect(log2.clone())));
    let mut r = Runner::new(
        WorldSpec {
            seed: 33,
            ..WorldSpec::default()
        },
        SpawnConfig::default(),
        bus,
        RunnerConfig::default(),
    );
    r.run(50);
    assert!(!log2
        .lock()
        .unwrap()
        .iter()
        .any(|(_, e)| matches!(e, Event::StanceChanged { .. })));
    let _ = Doctrine::from_path(format!("{DIR}/baseline.toml")).unwrap();
    let _ = Action::idle();
}

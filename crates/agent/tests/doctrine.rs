//! Doctrines, from the outside: they load, they validate, the trivial one is the
//! trivial baseline, and the two shipped ones behave as their comments claim.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agent::doctrine::{DoctrineError, Predicate};
use agent::{Doctrine, DoctrinePolicy, NearestShape, Policy, Runner, RunnerConfig};
use events::{Bus, Sink, TickRecord};
use rand::rngs::StdRng;
use rand::SeedableRng;
use schema::{
    Action, AgentId, Command, Event, Inputs, ObjectiveHint, Observation, Role, TeamId, Tick,
};
use sim_core::constants::ARENA_SIDE;
use sim_core::{World, WorldSpec};
use spawn::{CompositeSpawner, SpawnConfig, Spawner};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/doctrine");

fn shipped(name: &str) -> Doctrine {
    Doctrine::from_path(format!("{DIR}/{name}.toml")).unwrap_or_else(|e| panic!("{name}: {e}"))
}

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
    (
        Runner::new(spec, SpawnConfig::default(), bus, RunnerConfig::default()),
        log,
    )
}

// ---------------------------------------------------------------------------
// Loading and validation
// ---------------------------------------------------------------------------

#[test]
fn the_shipped_doctrines_load_and_round_trip() {
    for name in ["baseline", "aggressive", "defensive"] {
        let d = shipped(name);
        assert_eq!(d.name, name);
        let toml = d.to_toml_string().unwrap();
        let back = Doctrine::from_toml_str(&toml).unwrap();
        assert_eq!(back, d, "{name} survives TOML");
        let json = d.to_json_string().unwrap();
        let back = Doctrine::from_json_str(&json).unwrap();
        assert_eq!(back, d, "{name} survives JSON");
    }
}

#[test]
fn composition_fills_counts_in_order_and_the_last_role_absorbs_the_rest() {
    let d = shipped("defensive");
    let seats: Vec<AgentId> = (0..5).map(AgentId).collect();
    let roles: Vec<Role> = d.assign(&seats).into_iter().map(|(_, r)| r).collect();
    assert_eq!(
        roles,
        vec![
            Role::Screen,
            Role::Screen,
            Role::Screen,
            Role::Screen,
            Role::Scout
        ]
    );
    let seats: Vec<AgentId> = (0..7).map(AgentId).collect();
    let roles: Vec<Role> = d.assign(&seats).into_iter().map(|(_, r)| r).collect();
    assert_eq!(roles[4..], [Role::Scout, Role::Scout, Role::Scout]);
}

fn issues_of(toml: &str) -> Vec<String> {
    match Doctrine::from_toml_str(toml) {
        Err(DoctrineError::Invalid(issues)) => issues.iter().map(|i| i.path.clone()).collect(),
        Err(e) => panic!("expected validation issues, got {e}"),
        Ok(_) => panic!("expected validation issues, got a doctrine"),
    }
}

#[test]
fn validation_names_every_problem_by_path() {
    let paths = issues_of(
        r#"
name = ""
roles = [{ role = "farm", count = 0 }, { role = "scout", count = 1 }]

[behaviour.farm]
cohesion = { leash = { to = "cc", min = 50.0, max = 10.0 } }
fire = { mode = "burst" }
stances = [
  { name = "a", when = { hp_below = 1.5 }, drives = { seek_target = 99.0 } },
  { name = "a", when = { enemy_within = 10.0 }, drives = { hold = { region = { shape = "whole_arena" }, weight = 1.0 } } },
]
"#,
    );
    for expected in [
        "name",
        "roles[0]",
        "roles[1]",
        "behaviour.farm.stances[1]",
        "behaviour.farm.stances[0].when",
        "behaviour.farm.stances[0].drives.seek_target",
        "behaviour.farm.stances[1].name",
        "behaviour.farm.stances[1].drives.hold.region",
        "behaviour.farm.cohesion.leash.max",
        "behaviour.farm.fire.burst",
    ] {
        assert!(
            paths.contains(&expected.to_string()),
            "missing {expected} in {paths:?}"
        );
    }
}

#[test]
fn an_unknown_field_is_a_parse_error_not_a_silent_ignore() {
    let toml = r#"
name = "x"
roles = [{ role = "farm", count = 1 }]
[behaviour.farm]
stances = [{ name = "s", when = "always", drives = { seek_targt = 1.0 } }]
"#;
    assert!(matches!(
        Doctrine::from_toml_str(toml),
        Err(DoctrineError::Toml(_))
    ));
}

#[test]
fn a_role_without_behaviour_cannot_be_seated() {
    let d = shipped("baseline");
    let arena = sim_core::ArenaSpec::default();
    let err = DoctrinePolicy::new(&d, Role::Scout, TeamId::A, &arena, 1, AgentId(0)).err();
    assert!(matches!(err, Some(DoctrineError::NoBehaviour(Role::Scout))));
}

#[test]
fn predicates_serialise_the_way_the_files_are_written() {
    let p: Predicate = toml::from_str::<BTreeMap<String, Predicate>>(
        r#"w = { all_of = [{ hp_below = 0.5 }, { not = "has_target" }, "always"] }"#,
    )
    .unwrap()
    .remove("w")
    .unwrap();
    assert_eq!(
        p,
        Predicate::AllOf(vec![
            Predicate::HpBelow(0.5),
            Predicate::Not(Box::new(Predicate::HasTarget)),
            Predicate::Always,
        ])
    );
}

// ---------------------------------------------------------------------------
// The baseline doctrine is the baseline policy
// ---------------------------------------------------------------------------

#[test]
fn the_baseline_doctrine_decides_what_nearest_shape_decides() {
    let seed = 42;
    let mut world = World::new(WorldSpec {
        seed,
        ..WorldSpec::default()
    });
    let mut spawner = CompositeSpawner::new(SpawnConfig::default());
    let mut rng = StdRng::seed_from_u64(seed);
    let seeded = spawner.prefill(&world, &mut rng);
    spawn::apply(&mut world, &seeded);

    let doctrine = shipped("baseline");
    let arena = world.arena.clone();
    let tanks: Vec<(AgentId, TeamId)> = world
        .agents()
        .iter()
        .filter(|(_, s)| !s.is_cc)
        .map(|(a, s)| (*a, s.team))
        .collect();
    let mut reference: Vec<NearestShape> = tanks
        .iter()
        .map(|_| NearestShape::new(ARENA_SIDE))
        .collect();
    let mut under_test: Vec<DoctrinePolicy> = tanks
        .iter()
        .map(|(a, t)| DoctrinePolicy::new(&doctrine, Role::Farm, *t, &arena, seed, *a).unwrap())
        .collect();

    let mut held: Vec<(AgentId, Action)> =
        tanks.iter().map(|(a, _)| (*a, Action::idle())).collect();
    let mut compared = 0;
    for t in 0..600u32 {
        let requests = spawner.tick(&world, world.tick(), &mut rng);
        spawn::apply(&mut world, &requests);
        if t % 5 == 0 {
            for (i, (agent, _)) in tanks.iter().enumerate() {
                let obs: Observation = world.observe(*agent).unwrap();
                let now = world.tick();
                let a = reference[i].decide(&obs, now);
                let b = under_test[i].decide(&obs, now);
                assert_eq!(
                    a.control.fire, b.control.fire,
                    "fire, agent {agent:?} tick {t}"
                );
                assert!(
                    (a.control.thrust.x - b.control.thrust.x).abs() < 1e-4
                        && (a.control.thrust.y - b.control.thrust.y).abs() < 1e-4,
                    "thrust, agent {agent:?} tick {t}: {:?} vs {:?}",
                    a.control.thrust,
                    b.control.thrust
                );
                assert!(
                    (a.control.aim - b.control.aim).abs() < 1e-4,
                    "aim, agent {agent:?} tick {t}: {} vs {}",
                    a.control.aim,
                    b.control.aim
                );
                held[i].1 = a;
                compared += 1;
            }
        }
        world.step(&Inputs {
            actions: held.clone(),
            commands: Vec::new(),
        });
        world.drain_events();
    }
    assert_eq!(compared, 10 * 120);
}

// ---------------------------------------------------------------------------
// The shipped doctrines behave as claimed
// ---------------------------------------------------------------------------

fn fired_by(events: &[Event], agent: AgentId) -> usize {
    events
        .iter()
        .filter(|e| {
            matches!(e, Event::ActionSubmitted { agent: a, action, .. } if *a == agent && action.control.fire)
        })
        .count()
}

#[test]
fn the_scout_never_fires_and_the_screen_does() {
    let (mut r, log) = runner(9);
    let assigned = shipped("defensive").seat(&mut r, TeamId::A).unwrap();
    shipped("aggressive").seat(&mut r, TeamId::B).unwrap();
    r.run(1500);
    let events = log.lock().unwrap().clone();

    let scout = assigned
        .iter()
        .find(|(_, role)| *role == Role::Scout)
        .unwrap()
        .0;
    let screen = assigned
        .iter()
        .find(|(_, role)| *role == Role::Screen)
        .unwrap()
        .0;
    assert_eq!(fired_by(&events, scout), 0, "scout holds fire");
    assert!(fired_by(&events, screen) > 0, "screen fires");

    let h = r.stance_histogram();
    assert!(h[&scout].contains_key("scout"), "{:?}", h[&scout]);
    assert!(h[&screen].contains_key("farm"), "{:?}", h[&screen]);
}

/// Mean distance of the living tanks among `agents` from their centroid.
fn spread(r: &Runner, agents: &[AgentId]) -> f32 {
    let world = r.world();
    let pos: Vec<schema::Vec2> = agents
        .iter()
        .filter_map(|a| world.agents().get(a))
        .filter_map(|s| s.entity.and_then(|id| world.entity(id)).map(|e| e.pos))
        .collect();
    if pos.len() < 2 {
        return 0.0;
    }
    let n = pos.len() as f32;
    let c = pos.iter().fold(schema::Vec2::ZERO, |c, p| {
        schema::Vec2::new(c.x + p.x / n, c.y + p.y / n)
    });
    pos.iter().map(|p| p.distance(c)).sum::<f32>() / n
}

#[test]
fn a_leash_keeps_the_screen_tighter_than_pushers_with_none() {
    // Measured while the group is in sight of itself. Without comms, cohesion is
    // cohesion by sight: a screen that loses view of the others drives to where
    // the group last was and farms there while the group moves on, and nothing
    // can tell it otherwise. Later in a match both teams fragment. The leash is
    // real for as long as sight is, and the `TrackSet` belief message is what
    // will make it real for longer.
    let mut tight = Vec::new();
    let mut loose = Vec::new();
    let (mut r, _) = runner(21);
    let a = shipped("defensive").seat(&mut r, TeamId::A).unwrap();
    let b = shipped("aggressive").seat(&mut r, TeamId::B).unwrap();
    let screens: Vec<AgentId> = a
        .iter()
        .filter(|(_, role)| *role == Role::Screen)
        .map(|(a, _)| *a)
        .collect();
    let pushers: Vec<AgentId> = b
        .iter()
        .filter(|(_, role)| *role == Role::Push)
        .map(|(a, _)| *a)
        .collect();
    for _ in 0..4 {
        for _ in 0..100 {
            r.step();
        }
        tight.push(spread(&r, &screens));
        loose.push(spread(&r, &pushers));
    }
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
    assert!(
        mean(&tight) < mean(&loose),
        "screens {tight:?} should be tighter than pushers {loose:?}"
    );
    assert!(mean(&tight) < 80.0, "a sixty-unit leash: {tight:?}");
}

/// A control center that issues its commands on its first decision.
struct Once(Vec<Command>);

impl Policy for Once {
    fn decide(&mut self, _obs: &Observation, _now: Tick) -> Action {
        Action {
            commands: std::mem::take(&mut self.0),
            ..Action::default()
        }
    }
}

#[test]
fn commands_change_roles_and_objectives() {
    let (mut r, _) = runner(5);
    let assigned = shipped("defensive").seat(&mut r, TeamId::A).unwrap();
    let cc = r
        .seats()
        .find(|(_, t, is_cc)| *is_cc && *t == TeamId::A)
        .map(|(a, _, _)| a)
        .unwrap();
    let first_screen = assigned[0].0;
    r.set_policy(
        cc,
        Box::new(Once(vec![
            Command::AssignRole {
                agent: first_screen,
                role: Role::Scout,
            },
            Command::SetObjective {
                hint: ObjectiveHint::HoldNest,
            },
        ])),
    );
    r.run(1000);
    let h = r.stance_histogram();

    // The reassigned tank spent the match in scout stances, bar the one decision
    // it made before the order arrived.
    let reassigned = &h[&first_screen];
    assert!(
        reassigned.get("scout").copied().unwrap_or(0) > 100,
        "{reassigned:?}"
    );
    assert!(
        reassigned.get("farm").copied().unwrap_or(0) <= 1,
        "{reassigned:?}"
    );

    // The screens heard the objective and held the nest.
    let held_nest = assigned[1..4]
        .iter()
        .any(|(a, _)| h[a].contains_key("hold_nest"));
    assert!(held_nest, "{h:?}");
}

#[test]
fn doctrines_are_deterministic() {
    let mut streams = Vec::new();
    for _ in 0..2 {
        let (mut r, log) = runner(77);
        shipped("aggressive").seat(&mut r, TeamId::A).unwrap();
        shipped("defensive").seat(&mut r, TeamId::B).unwrap();
        let s = r.run(750);
        assert!(s.kills[0] + s.kills[1] > 0);
        streams.push(schema::to_line(&*log.lock().unwrap()).unwrap());
    }
    assert_eq!(streams[0], streams[1]);
}

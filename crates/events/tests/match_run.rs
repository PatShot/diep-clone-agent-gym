//! End-to-end: a real match, recorded, replayed, and queried.

use rand::rngs::StdRng;
use rand::SeedableRng;
use rusqlite::Connection;
use schema::{Action, AgentId, Control, Event, Inputs, TeamId, Tick, Vec2};
use sim_core::constants::TICK_HZ;
use sim_core::{World, WorldSpec};
use spawn::{CompositeSpawner, SpawnConfig, Spawner};

use events::{Bus, ReplayReader, ReplaySink, Sink, SqliteSink, TickRecord};

const SEED: u64 = 20260904;
const TICKS: u32 = 20 * TICK_HZ;

fn spec() -> WorldSpec {
    WorldSpec {
        seed: SEED,
        match_id: "e2e".into(),
        config_hash: 99,
        ..WorldSpec::default()
    }
}

/// A scripted policy. Deterministic, and varied enough that the log is not one long
/// run of repeats.
fn actions(agents: &[AgentId], t: u32) -> Vec<(AgentId, Action)> {
    agents
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let th = t as f32 * 0.05 + i as f32 * 1.1;
            (
                *a,
                Action {
                    control: Control {
                        thrust: Vec2::new(th.cos(), th.sin()),
                        aim: th,
                        fire: (t / 10 + i as u32) % 3 == 0,
                    },
                    ..Action::default()
                },
            )
        })
        .collect()
}

/// Run a match, handing each tick to `observe`. Returns the whole event stream.
fn run(mut observe: impl FnMut(&TickRecord<'_>)) -> Vec<Event> {
    let mut w = World::new(spec());
    let mut spawner = CompositeSpawner::new(SpawnConfig::default());
    let mut rng = StdRng::seed_from_u64(SEED);
    let seeded = spawner.prefill(&w, &mut rng);
    spawn::apply(&mut w, &seeded);
    let agents: Vec<AgentId> = w.agents().keys().copied().collect();

    let mut all = Vec::new();
    for t in 0..TICKS {
        let requests = spawner.tick(&w, w.tick(), &mut rng);
        spawn::apply(&mut w, &requests);

        let inputs = Inputs {
            actions: actions(&agents, t),
            commands: Vec::new(),
        };
        w.step(&inputs);

        let mut events = w.drain_events();
        if t == TICKS - 1 {
            events.push(Event::MatchEnd {
                winner: Some(TeamId::A),
                tick: w.tick(),
            });
        }
        observe(&TickRecord {
            tick: Tick(t),
            events: &events,
            inputs: &inputs,
            scores: w.scores(),
        });
        all.extend(events);
    }
    all
}

/// Replay a recorded log against a fresh world and return its event stream.
///
/// The spawner is re-run from the same seed rather than recorded. Its decisions are
/// a function of the configuration and the world state, both of which the seed and
/// the inputs already determine — which is exactly what `config_hash` pins.
fn replay(log: &[(Tick, Inputs)]) -> Vec<Event> {
    let mut w = World::new(spec());
    let mut spawner = CompositeSpawner::new(SpawnConfig::default());
    let mut rng = StdRng::seed_from_u64(SEED);
    let seeded = spawner.prefill(&w, &mut rng);
    spawn::apply(&mut w, &seeded);

    let mut all = Vec::new();
    for (i, (_, inputs)) in log.iter().enumerate() {
        let requests = spawner.tick(&w, w.tick(), &mut rng);
        spawn::apply(&mut w, &requests);
        w.step(inputs);
        let mut events = w.drain_events();
        if i == log.len() - 1 {
            events.push(Event::MatchEnd {
                winner: Some(TeamId::A),
                tick: w.tick(),
            });
        }
        all.extend(events);
    }
    all
}

/// The property the whole design rests on: a log of inputs plus a seed reproduces
/// the match. If this fails, the replay file is a record of nothing.
#[test]
fn a_recorded_log_replays_the_match_exactly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("m.replay.gz");

    let arena = World::new(spec()).arena.to_info();
    let info = schema::MatchInfo {
        match_id: "e2e".into(),
        seed: SEED,
        point_target: 1000,
        teams: Vec::new(),
    };

    let original = {
        let mut sink = ReplaySink::create(&path, arena, info).expect("create");
        let events = run(|rec| sink.accept(rec));
        sink.flush();
        events
    };

    let (_, reader) = ReplayReader::open(&path).expect("open");
    let log = reader.read_all().expect("read");
    assert_eq!(log.len(), TICKS as usize);

    let replayed = replay(&log);

    assert_eq!(
        original.len(),
        replayed.len(),
        "replay produced a different number of events"
    );
    let a = serde_json::to_string(&original).expect("serialize");
    let b = serde_json::to_string(&replayed).expect("serialize");
    assert_eq!(a, b, "replay diverged from the recorded match");
}

/// A replay is orders of magnitude smaller than the match it reproduces. That is the
/// entire reason for recording inputs rather than frames.
#[test]
fn a_replay_log_is_small() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("m.replay.gz");
    let arena = World::new(spec()).arena.to_info();
    let info = schema::MatchInfo {
        match_id: "e2e".into(),
        seed: SEED,
        point_target: 1000,
        teams: Vec::new(),
    };

    let entities;
    {
        let mut sink = ReplaySink::create(&path, arena, info).expect("create");
        let mut last = 0;
        run(|rec| {
            last = rec.events.len();
            sink.accept(rec);
        });
        sink.flush();
        entities = last;
    }
    let _ = entities;

    let bytes = std::fs::metadata(&path).expect("stat").len();
    let per_tick = bytes as f64 / TICKS as f64;
    // Twelve agents at ~13 bytes of decision is the floor; a snapshot of two
    // thousand entities would be a hundred times this.
    assert!(
        per_tick < 400.0,
        "replay costs {per_tick:.0} bytes a tick, which is snapshot territory"
    );
}

#[test]
fn the_database_answers_questions_about_the_match() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("m.db");

    {
        let mut bus = Bus::new().with(Box::new(SqliteSink::create(&db).expect("create")));
        run(|rec| bus.publish(rec));
        bus.flush();
    }

    let conn = Connection::open(&db).expect("open");

    // Every event kind that fired, and how often.
    let mut stmt = conn
        .prepare("SELECT kind, count(*) FROM events GROUP BY kind")
        .expect("prepare");
    let counts: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    assert!(!counts.is_empty(), "no events recorded");

    let by = |k: &str| counts.iter().find(|(n, _)| n == k).map(|(_, c)| *c);
    assert!(by("spawned").unwrap_or(0) > 0, "no spawns recorded");
    assert_eq!(by("tick_begin"), None, "tick_begin should not be persisted");
    assert_eq!(
        by("action_submitted"),
        None,
        "actions belong to the replay log, not to the database"
    );
    assert_eq!(by("match_end"), Some(1));

    // DESIGN.md's "which farming area did the team prioritise", answered from the
    // database rather than from a test fixture.
    let mut stmt = conn
        .prepare(
            "SELECT payload->>'focus', count(*) FROM events \
             WHERE kind = 'spawned' AND payload->>'focus' IS NOT NULL \
             GROUP BY 1 ORDER BY 2 DESC",
        )
        .expect("prepare");
    let foci: Vec<(i64, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    assert!(
        foci.len() >= 2,
        "expected several foci to have spawned shapes, got {foci:?}"
    );

    // The match row closes.
    let winner: Option<i64> = conn
        .query_row("SELECT winner FROM match WHERE id = 'e2e'", [], |r| {
            r.get(0)
        })
        .expect("match row");
    assert_eq!(winner, Some(0));
    let ended: Option<String> = conn
        .query_row("SELECT ended_at FROM match WHERE id = 'e2e'", [], |r| {
            r.get(0)
        })
        .expect("match row");
    assert!(ended.is_some(), "match never closed");

    // No kinematics table: trajectories are regenerated, never stored.
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .expect("prepare")
        .query_map([], |r| r.get(0))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    assert!(
        !tables.iter().any(|t| t == "kinematics"),
        "kinematics are not persisted; found {tables:?}"
    );
}

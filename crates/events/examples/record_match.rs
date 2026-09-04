//! Record a full-length match and report what it cost to store.
//!
//! `cargo run -p events --example record_match --release`
//!
//! The point of the run is the last two lines. A replay records what twelve agents
//! decided, not what two thousand entities did, so a ninety-minute match should
//! cost tens of megabytes rather than tens of gigabytes.

use rand::rngs::StdRng;
use rand::SeedableRng;
use schema::{Action, AgentId, Control, Event, Inputs, MatchInfo, TeamId, Tick, Vec2};
use sim_core::constants::TICK_HZ;
use sim_core::{World, WorldSpec};
use spawn::{CompositeSpawner, SpawnConfig, Spawner};
use std::time::Instant;

use events::{Bus, ReplaySink, SqliteSink};

const SEED: u64 = 20260904;
const MINUTES: u32 = 90;

fn main() {
    let dir = std::env::temp_dir();
    let replay_path = dir.join("match.replay.gz");
    let db_path = dir.join("match.db");
    let _ = std::fs::remove_file(&db_path);

    let mut w = World::new(WorldSpec {
        seed: SEED,
        match_id: "record".into(),
        config_hash: 1,
        ..WorldSpec::default()
    });
    let mut spawner = CompositeSpawner::new(SpawnConfig::default());
    let mut rng = StdRng::seed_from_u64(SEED);
    let seeded = spawner.prefill(&w, &mut rng);
    spawn::apply(&mut w, &seeded);

    let info = MatchInfo {
        match_id: "record".into(),
        seed: SEED,
        point_target: 1000,
        teams: Vec::new(),
    };
    let mut bus = Bus::new()
        .with(Box::new(
            ReplaySink::create(&replay_path, w.arena.to_info(), info).expect("replay"),
        ))
        .with(Box::new(SqliteSink::create(&db_path).expect("db")));

    let agents: Vec<AgentId> = w.agents().keys().copied().collect();
    let total = MINUTES * 60 * TICK_HZ;
    let start = Instant::now();
    let mut events_seen = 0u64;

    for t in 0..total {
        let requests = spawner.tick(&w, w.tick(), &mut rng);
        spawn::apply(&mut w, &requests);

        let th = t as f32 * 0.013;
        let inputs = Inputs {
            actions: agents
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let a_th = th + i as f32 * 1.257;
                    (
                        *a,
                        Action {
                            control: Control {
                                thrust: Vec2::new(a_th.cos(), a_th.sin()),
                                aim: -a_th,
                                fire: t % 7 == 0,
                            },
                            ..Action::default()
                        },
                    )
                })
                .collect(),
            commands: Vec::new(),
        };
        w.step(&inputs);

        let mut ev = w.drain_events();
        if t == total - 1 {
            ev.push(Event::MatchEnd {
                winner: Some(TeamId::A),
                tick: w.tick(),
            });
        }
        events_seen += ev.len() as u64;
        bus.publish(&events::TickRecord {
            tick: Tick(t),
            events: &ev,
            inputs: &inputs,
            scores: w.scores(),
        });
    }
    bus.flush();
    drop(bus);

    let elapsed = start.elapsed();
    let replay_bytes = std::fs::metadata(&replay_path)
        .map(|m| m.len())
        .unwrap_or(0);
    let db_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
    let snapshot_bytes = 2_000u64 * 120 * total as u64;

    println!("{MINUTES}-minute match, {total} ticks, {events_seen} events");
    println!("simulated and written in {:.1}s\n", elapsed.as_secs_f64());
    println!(
        "  replay log   {:>10}   {}",
        mb(replay_bytes),
        replay_path.display()
    );
    println!(
        "  database     {:>10}   {}",
        mb(db_bytes),
        db_path.display()
    );
    println!("  ---");
    println!(
        "  a snapshot-per-tick replay would have been {:>10}",
        mb(snapshot_bytes)
    );
    println!(
        "  inputs are {:.0}x smaller",
        snapshot_bytes as f64 / replay_bytes.max(1) as f64
    );
}

fn mb(bytes: u64) -> String {
    let mb = bytes as f64 / 1_048_576.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{mb:.1} MB")
    }
}

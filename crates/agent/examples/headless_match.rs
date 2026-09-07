//! Run a match headless with the trivial baseline on every tank.
//!
//! `cargo run -p agent --example headless_match --release -- [minutes] [seed] [policy]`
//!
//! `policy` is `doctrine` (the default: team A runs `config/doctrine/aggressive.toml`
//! against team B on `defensive.toml`), `known` (every tank farms from memory),
//! `nearest` (every tank forgets everything between decisions), or `bridge`
//! (team A is driven by a policy process over a socket, team B is defensive).
//!
//! In `bridge` mode the example listens on 127.0.0.1:7777 — `bridge:HOST:PORT` to
//! choose — and waits for one client. `python3 scripts/policy_client.py` is one.
//!
//! Writes a replay log and a match database to the temp directory and prints what
//! happened. The first thing worth looking at is the kill count: this is the first
//! time a tank has killed anything.

use agent::{
    Doctrine, Link, NearestKnownShape, NearestShape, Policy, Runner, RunnerConfig, SocketPolicy,
};
use events::{Bus, ReplaySink, SqliteSink};
use schema::MatchInfo;
use schema::TeamId;
use sim_core::constants::{ARENA_SIDE, TICK_HZ};
use sim_core::{ArenaSpec, WorldSpec};
use spawn::SpawnConfig;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let minutes: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(10);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(20260907);
    let policy = args.next().unwrap_or_else(|| "doctrine".to_string());

    let dir = std::env::temp_dir();
    let replay_path = dir.join("headless.replay.gz");
    let db_path = dir.join("headless.db");
    let _ = std::fs::remove_file(&db_path);

    let spec = WorldSpec {
        seed,
        match_id: "headless".into(),
        config_hash: 1,
        ..WorldSpec::default()
    };
    let info = MatchInfo {
        match_id: spec.match_id.clone(),
        seed,
        point_target: 1000,
        teams: Vec::new(),
    };
    let bus = Bus::new()
        .with(Box::new(
            ReplaySink::create(&replay_path, ArenaSpec::default().to_info(), info).expect("replay"),
        ))
        .with(Box::new(SqliteSink::create(&db_path).expect("db")));

    let mut runner = Runner::new(spec, SpawnConfig::default(), bus, RunnerConfig::default());
    let seats: Vec<_> = runner.seats().collect();
    let mut link: Option<Arc<Mutex<Link>>> = None;
    if let Some(rest) = policy.strip_prefix("bridge") {
        let addr = rest.strip_prefix(':').unwrap_or("127.0.0.1:7777");
        println!("listening on {addr} for a policy process to drive team A ...");
        let l = Link::listen(addr, Duration::from_secs(2)).expect("listen");
        println!("connected\n");
        let l = Arc::new(Mutex::new(l));
        for (agent, team, is_cc) in &seats {
            if !is_cc && *team == TeamId::A {
                runner.set_policy(*agent, Box::new(SocketPolicy::new(l.clone())));
            }
        }
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/doctrine");
        let b = Doctrine::from_path(format!("{dir}/defensive.toml")).expect("defensive.toml");
        b.seat(&mut runner, TeamId::B).expect("seat B");
        link = Some(l);
    } else if policy == "doctrine" {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/doctrine");
        let a = Doctrine::from_path(format!("{dir}/aggressive.toml")).expect("aggressive.toml");
        let b = Doctrine::from_path(format!("{dir}/defensive.toml")).expect("defensive.toml");
        a.seat(&mut runner, TeamId::A).expect("seat A");
        b.seat(&mut runner, TeamId::B).expect("seat B");
        println!("team A: {}   team B: {}\n", a.name, b.name);
    } else {
        for (agent, _, is_cc) in &seats {
            if !is_cc {
                let p: Box<dyn Policy> = match policy.as_str() {
                    "nearest" => Box::new(NearestShape::new(ARENA_SIDE)),
                    _ => Box::new(NearestKnownShape::new(ARENA_SIDE)),
                };
                runner.set_policy(*agent, p);
            }
        }
    }

    let total = minutes * 60 * TICK_HZ;
    let start = Instant::now();
    let s = runner.run(total);
    let elapsed = start.elapsed();

    println!(
        "{minutes}-minute match, seed {seed}, policy {policy}, {} ticks, {} events",
        s.ticks, s.events
    );
    println!("simulated in {:.1}s\n", elapsed.as_secs_f64());
    println!("  kills     A {:>5}   B {:>5}", s.kills[0], s.kills[1]);
    println!(
        "  scores    A {:>5}   B {:>5}   (nothing scores until objective)",
        s.scores[0], s.scores[1]
    );
    println!(
        "  decisions {:>7}   mean {} us   max {} us",
        s.decisions, s.latency_mean_us, s.latency_max_us
    );
    println!(
        "  footprint {:>7} bytes per tank, against a ceiling of {}",
        s.footprint_max,
        agent::model::MEMORY_CEILING_BYTES
    );

    if let Some(l) = &link {
        let s = l.lock().unwrap().stats();
        println!(
            "  bridge    {} decided   {} timeouts   {} malformed   {}",
            s.decided,
            s.timeouts,
            s.malformed,
            if s.dead { "hung up" } else { "connected" }
        );
    }

    // Decisions per stance, summed over each team's tanks.
    let mut per_team: BTreeMap<TeamId, BTreeMap<&str, u32>> = BTreeMap::new();
    for (agent, team, _) in &seats {
        if let Some(h) = runner.stance_histogram().get(agent) {
            let entry = per_team.entry(*team).or_default();
            for (name, n) in h {
                *entry.entry(name.as_str()).or_default() += n;
            }
        }
    }
    for (team, h) in &per_team {
        let total: u32 = h.values().sum();
        print!("  stances {:?}", team);
        for (name, n) in h {
            print!("   {name} {:.0}%", 100.0 * *n as f32 / total.max(1) as f32);
        }
        println!();
    }

    println!("\n  replay    {}", replay_path.display());
    println!("  database  {}", db_path.display());
    println!(
        "\n  sqlite3 {} \"select kind, count(*) from events group by kind\"",
        db_path.display()
    );
    println!(
        "  sqlite3 {} \"select payload->>'stance', count(*) from events where kind='stance_changed' group by 1\"",
        db_path.display()
    );
}

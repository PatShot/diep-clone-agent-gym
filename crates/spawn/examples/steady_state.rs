//! Drive a world and a composite spawner together and watch the population settle.
//!
//! `cargo run -p spawn --example steady_state --release`
//!
//! Prints alive shapes per focus every thirty seconds of simulated time. What the
//! table should show: the scatter focus climbing quickly to its slow threshold at
//! 1200, then decelerating as the ramp takes hold, and the nest sitting flat at its
//! own much smaller ceiling. That deceleration is the farming pressure.

use rand::rngs::StdRng;
use rand::SeedableRng;
use schema::ShapeTier;
use sim_core::constants::TICK_HZ;
use sim_core::{Inputs, World, WorldSpec};
use spawn::{CompositeSpawner, SpawnConfig, Spawner};
use std::path::PathBuf;

const MINUTES: u32 = 5;
const REPORT_EVERY_SEC: u32 = 30;

fn main() {
    let spec = WorldSpec {
        seed: 20260904,
        ..WorldSpec::default()
    };
    let mut world = World::new(spec);
    let mut spawner = CompositeSpawner::new(load_config());
    let mut rng = StdRng::seed_from_u64(20260904);

    let seeded = spawner.prefill(&world, &mut rng);
    spawn::apply(&mut world, &seeded);
    println!("prefill placed {} shapes\n", seeded.len());

    let ids: Vec<_> = spawner.config().foci.iter().map(|f| f.id).collect();
    print!("{:>7}", "time");
    for f in spawner.config().foci.iter() {
        let label = format!("{:?}{}", f.id.0, if f.enabled { "" } else { "*" });
        print!("{label:>8}");
    }
    println!("{:>9}{:>9}{:>9}", "total", "spawned", "points");

    let total_ticks = MINUTES * 60 * TICK_HZ;
    let mut spawned_since = 0usize;

    for t in 0..total_ticks {
        let requests = spawner.tick(&world, world.tick(), &mut rng);
        spawned_since += requests.len();
        spawn::apply(&mut world, &requests);
        world.step(&Inputs::default());

        if (t + 1) % (REPORT_EVERY_SEC * TICK_HZ) == 0 {
            let census = spawner.last_census();
            print!("{:>6}s", (t + 1) / TICK_HZ);
            let mut total = 0;
            for id in &ids {
                let n = census
                    .iter()
                    .find(|c| c.focus == *id)
                    .map(|c| c.alive)
                    .unwrap_or(0);
                total += n;
                print!("{n:>8}");
            }
            println!("{total:>9}{spawned_since:>9}{:>9}", points(&world));
            spawned_since = 0;
        }
    }

    println!("\n* focus is disabled in the default configuration");
    let (sq, tri, pent, alpha) = mix(&world);
    println!(
        "tier mix at the end: {sq} squares, {tri} triangles, {pent} pentagons, \
         {alpha} alpha pentagons"
    );
    println!("points on the board: {}", points(&world));
}

/// Read the shipped configuration, so that editing `config/spawn.toml` changes what
/// this prints. Falls back to the compiled defaults if the file is not where the
/// manifest says it should be; a test asserts the two are identical anyway.
fn load_config() -> SpawnConfig {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/spawn.toml");
    match SpawnConfig::from_path(&path) {
        Ok(c) => {
            println!("configuration: {}", path.display());
            c
        }
        Err(e) => {
            println!("configuration: compiled defaults ({e})");
            SpawnConfig::default()
        }
    }
}

/// Total score sitting in the arena as unfarmed shapes.
fn points(world: &World) -> u32 {
    world
        .iter_entities()
        .filter(|(_, e)| e.is_shape())
        .map(|(_, e)| e.value())
        .sum()
}

fn mix(world: &World) -> (usize, usize, usize, usize) {
    let mut out = (0, 0, 0, 0);
    for (_, e) in world.iter_entities() {
        match e.tier {
            Some(ShapeTier::Square) => out.0 += 1,
            Some(ShapeTier::Triangle) => out.1 += 1,
            Some(ShapeTier::Pentagon) => out.2 += 1,
            Some(ShapeTier::AlphaPentagon) => out.3 += 1,
            None => {}
        }
    }
    out
}

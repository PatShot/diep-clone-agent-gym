//! Nest radius against yield.
//!
//! `cargo run -p spawn --example nest_geometry --release`
//!
//! Holds every population band at its shipped value and grows only the nest disc,
//! at one, 1.2 and 1.5 times its radius. Reports what a region is worth standing
//! still — points per unit area — and what it is worth being farmed, by pointing a
//! team's worth of damage at it and counting what falls out.
//!
//! Wings are enabled here. They ship disabled, and the comparison needs them.

use rand::rngs::StdRng;
use rand::SeedableRng;
use schema::{Region, ShapeTier, Vec2};
use sim_core::constants::{BULLET_DAMAGE, DT, NEST_RADIUS, RELOAD_TICKS, TICK_HZ};
use sim_core::{Inputs, World, WorldSpec};
use spawn::config::{FOCUS_NEST, FOCUS_NEST_ALPHA};
use spawn::{CompositeSpawner, SpawnConfig, Spawner};

/// Damage per second from five tanks firing without pause. The figure a farming
/// team is limited by, and therefore the one that decides where farming pays.
fn team_dps() -> f32 {
    let per_tank = BULLET_DAMAGE / (RELOAD_TICKS as f32 / TICK_HZ as f32);
    per_tank * 5.0
}

/// A team's damage, pointed at whatever is worth most inside one region.
struct Harvester {
    region: Region,
    dps: f32,
    points: u64,
    kills: u32,
}

impl Harvester {
    fn new(region: Region, dps: f32) -> Self {
        Self {
            region,
            dps,
            points: 0,
            kills: 0,
        }
    }

    fn tick(&mut self, w: &mut World, side: f32) {
        let target = w
            .iter_entities()
            .filter(|(_, e)| e.is_shape() && e.hp > 0.0 && self.region.contains(e.pos, side))
            .max_by_key(|(_, e)| e.value())
            .map(|(id, e)| (id, e.value()));
        let Some((id, value)) = target else { return };
        let Some(e) = w.entity_mut(id) else { return };
        e.hp -= self.dps * DT;
        if e.hp <= 0.0 {
            self.points += value as u64;
            self.kills += 1;
        }
    }
}

fn area(r: &Region) -> f32 {
    match *r {
        Region::Disc { radius, .. } => std::f32::consts::PI * radius * radius,
        _ => unreachable!("only discs are measured here"),
    }
}

fn main() {
    let dps = team_dps();
    println!("team damage per second: {dps:.1}\n");

    println!(
        "{:>6} {:>7} {:>6} {:>6} {:>7} {:>8} {:>9} {:>9} {:>10}",
        "region",
        "radius",
        "pents",
        "alpha",
        "escapd",
        "points",
        "pts/area",
        "farm/s",
        "farm/s/area"
    );

    for mult in [1.0f32, 1.2, 1.5] {
        run_nest(mult, dps);
    }
    run_wing(dps);

    println!(
        "\nescapd: alpha pentagons that ended up outside the nest disc.\n\
         pts/area and farm/s/area are scaled by 10,000 square units."
    );
}

fn nest_disc(mult: f32) -> Region {
    Region::Disc {
        center: Vec2::new(500.0, 500.0),
        radius: NEST_RADIUS * mult,
    }
}

fn run_nest(mult: f32, dps: f32) {
    let disc = nest_disc(mult);
    let mut config = SpawnConfig::default();
    for f in &mut config.foci {
        if f.id == FOCUS_NEST || f.id == FOCUS_NEST_ALPHA {
            f.region = disc;
        }
        // The alpha placement inset is a fraction of the nest radius, so it has to
        // grow with the disc or the experiment measures two changes at once.
        if f.id == FOCUS_NEST_ALPHA {
            for r in &mut f.exclude {
                if let Region::Annulus { inner, outer, .. } = r {
                    *inner *= mult;
                    *outer *= mult;
                }
            }
        }
        f.enabled = true;
    }
    let (pents, alphas, escaped, points, farmed) = measure(config, disc, dps, true);
    report(
        &format!("nest x{mult}"),
        NEST_RADIUS * mult,
        pents,
        alphas,
        escaped,
        points,
        farmed,
        area(&disc),
    );
}

fn run_wing(dps: f32) {
    let mut config = SpawnConfig::default();
    for f in &mut config.foci {
        f.enabled = true;
    }
    let wing = config
        .foci
        .iter()
        .find(|f| f.id == spawn::config::FOCUS_WING_NE)
        .expect("wing")
        .region;
    let radius = match wing {
        Region::Disc { radius, .. } => radius,
        _ => unreachable!(),
    };
    let (pents, alphas, escaped, points, farmed) = measure(config, wing, dps, false);
    report(
        "wing",
        radius,
        pents,
        alphas,
        escaped,
        points,
        farmed,
        area(&wing),
    );
}

/// Fill the arena, measure the standing stock, then farm the region for ten
/// minutes and measure the flow.
fn measure(
    config: SpawnConfig,
    region: Region,
    dps: f32,
    count_nest: bool,
) -> (usize, usize, usize, u32, f32) {
    let mut w = World::new(WorldSpec::default());
    let side = w.spec().arena.side;
    let mut s = CompositeSpawner::new(config);
    let mut rng = StdRng::seed_from_u64(20260904);
    let seeded = s.prefill(&w, &mut rng);
    spawn::apply(&mut w, &seeded);

    // Settle, unfarmed.
    for _ in 0..(30 * 60 * TICK_HZ) {
        let r = s.tick(&w, w.tick(), &mut rng);
        spawn::apply(&mut w, &r);
        w.step(&Inputs::default());
    }

    let mut pents = 0;
    let mut alphas = 0;
    let mut escaped = 0;
    let mut points = 0u32;
    for (_, e) in w.iter_entities() {
        if !e.is_shape() {
            continue;
        }
        let inside = region.contains(e.pos, side);
        if inside {
            points += e.value();
            match e.tier {
                Some(ShapeTier::Pentagon) => pents += 1,
                Some(ShapeTier::AlphaPentagon) => alphas += 1,
                _ => {}
            }
        } else if count_nest && e.tier == Some(ShapeTier::AlphaPentagon) {
            escaped += 1;
        }
    }

    // Farm it.
    let minutes = 10;
    let mut h = Harvester::new(region, dps);
    for _ in 0..(minutes * 60 * TICK_HZ) {
        h.tick(&mut w, side);
        let r = s.tick(&w, w.tick(), &mut rng);
        spawn::apply(&mut w, &r);
        w.step(&Inputs::default());
    }
    let farmed = h.points as f32 / (minutes * 60) as f32;
    (pents, alphas, escaped, points, farmed)
}

#[allow(clippy::too_many_arguments)]
fn report(
    label: &str,
    radius: f32,
    pents: usize,
    alphas: usize,
    escaped: usize,
    points: u32,
    farmed: f32,
    a: f32,
) {
    println!(
        "{label:>6} {radius:>7.0} {pents:>6} {alphas:>6} {escaped:>7} {points:>8} \
         {:>9.1} {farmed:>9.1} {:>10.2}",
        points as f32 / a * 10_000.0,
        farmed / a * 10_000.0
    );
}

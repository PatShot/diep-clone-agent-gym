//! Behavioural tests for the composite spawner.

use std::collections::BTreeMap;

use rand::rngs::StdRng;
use rand::SeedableRng;
use schema::{Region, ShapeTier, Vec2};
use sim_core::constants::{
    NEST_ALPHA_RATE, NEST_MINOR_RATE_MULT, NEST_PENTAGON_RATE_MULT, SHAPE_SPAWN_BASE_KEEPOUT,
    SHAPE_VALUE_ALPHA, SHAPE_VALUE_SQUARE, SHAPE_VALUE_TRIANGLE, TICK_HZ, WING_SPAWN_RATE,
};
use sim_core::{shape_radius, Inputs, World, WorldSpec};
use spawn::config::{FOCUS_NEST, FOCUS_NEST_ALPHA, FOCUS_SCATTER, FOCUS_WING_NE, FOCUS_WING_SW};
use spawn::place::distance_to_rect;
use spawn::{CompositeSpawner, SpawnConfig, SpawnRequest, Spawner};

fn world() -> World {
    World::new(WorldSpec::default())
}

/// A resident count may sit slightly above its ceiling without anything being
/// wrong. The band governs spawning, not membership: contact separation can push a
/// shape out of its region on one tick and back in on another, and a shape that
/// re-enters was never spawned. What must hold exactly is that no tick *requests*
/// more than the headroom, which `no_tick_requests_more_than_the_headroom` checks.
const REENTRY_SLACK: usize = 8;

/// Run the spawner and the world together for `ticks`, applying every request.
fn run(w: &mut World, s: &mut CompositeSpawner, rng: &mut StdRng, ticks: u32) {
    for _ in 0..ticks {
        let reqs = s.tick(w, w.tick(), rng);
        spawn::apply(w, &reqs);
        w.step(&Inputs::default());
    }
}

fn alive(w: &World, id: schema::FocusId) -> usize {
    w.iter_entities()
        .filter(|(_, e)| e.is_shape() && e.focus == Some(id))
        .count()
}

#[test]
fn same_seed_same_requests() {
    let mut requests: Vec<Vec<SpawnRequest>> = Vec::new();
    for _ in 0..2 {
        let mut w = world();
        let mut s = CompositeSpawner::new(SpawnConfig::default());
        let mut rng = StdRng::seed_from_u64(99);
        let mut seen = Vec::new();
        seen.extend(s.prefill(&w, &mut rng));
        spawn::apply(&mut w, &seen.clone());
        for _ in 0..600 {
            let reqs = s.tick(&w, w.tick(), &mut rng);
            seen.extend(reqs.iter().copied());
            spawn::apply(&mut w, &reqs);
            w.step(&Inputs::default());
        }
        requests.push(seen);
    }
    assert!(!requests[0].is_empty(), "the run must actually spawn");
    assert_eq!(
        requests[0], requests[1],
        "the same seed must produce the same request stream"
    );
}

#[test]
fn population_stays_inside_the_band() {
    let mut w = world();
    let mut s = CompositeSpawner::new(SpawnConfig::default());
    let mut rng = StdRng::seed_from_u64(1);
    let seeded = s.prefill(&w, &mut rng);
    spawn::apply(&mut w, &seeded);

    run(&mut w, &mut s, &mut rng, 60 * TICK_HZ);

    let census = s.last_census();
    for focus in &s.config().foci {
        let n = census
            .iter()
            .find(|c| c.focus == focus.id)
            .map(|c| c.alive)
            .expect("every focus is censused");
        if !focus.enabled {
            assert_eq!(
                alive(&w, focus.id),
                0,
                "a disabled focus must spawn nothing"
            );
            continue;
        }
        assert!(
            n <= focus.num_max + REENTRY_SLACK,
            "focus {:?} holds {n}, above its ceiling of {}",
            focus.id,
            focus.num_max
        );
    }
}

#[test]
fn nothing_spawns_near_a_base() {
    let mut w = world();
    let mut s = CompositeSpawner::new(SpawnConfig::default());
    let mut rng = StdRng::seed_from_u64(4);
    let seeded = s.prefill(&w, &mut rng);
    spawn::apply(&mut w, &seeded);
    run(&mut w, &mut s, &mut rng, 30 * TICK_HZ);

    let bases = w.spec().arena.bases;
    let mut checked = 0;
    for (_, e) in w.iter_entities() {
        if !e.is_shape() {
            continue;
        }
        checked += 1;
        for base in &bases {
            // Shapes drift, so a shape may have wandered closer since. The spawn
            // rule is about where it was placed; allow for the drift travelled.
            assert!(
                distance_to_rect(e.pos, base) > 0.0,
                "a shape reached the inside of a base at {:?}",
                e.pos
            );
        }
    }
    assert!(checked > 100, "expected a populated arena, saw {checked}");
}

#[test]
fn placement_respects_the_keepout_exactly() {
    let w = world();
    let mut s = CompositeSpawner::new(SpawnConfig::default());
    let mut rng = StdRng::seed_from_u64(5);

    let reqs = s.prefill(&w, &mut rng);
    let bases = w.spec().arena.bases;
    for r in &reqs {
        let radius = shape_radius(r.tier);
        for base in &bases {
            assert!(
                distance_to_rect(r.pos, base) >= SHAPE_SPAWN_BASE_KEEPOUT + radius,
                "placed {:?} within the keep-out of a base",
                r.pos
            );
        }
    }
}

#[test]
fn pentagons_are_the_nest_and_the_nest_only() {
    let mut w = world();
    let mut s = CompositeSpawner::new(SpawnConfig::default());
    let mut rng = StdRng::seed_from_u64(11);

    let mut reqs = s.prefill(&w, &mut rng);
    spawn::apply(&mut w, &reqs);
    for _ in 0..(120 * TICK_HZ) {
        let more = s.tick(&w, w.tick(), &mut rng);
        reqs.extend(more.iter().copied());
        spawn::apply(&mut w, &more);
        w.step(&Inputs::default());
    }

    let nest = w.spec().arena.nest;
    let side = w.spec().arena.side;
    let mut pentagons = 0;
    let mut alphas = 0;
    let mut scatter_in_nest = 0;
    for r in &reqs {
        if r.tier == ShapeTier::Pentagon {
            pentagons += 1;
            assert_eq!(r.focus, FOCUS_NEST, "a pentagon came from outside the nest");
            assert!(
                nest.contains(r.pos, side),
                "a pentagon was placed outside the nest disc at {:?}",
                r.pos
            );
        }
        if r.tier == ShapeTier::AlphaPentagon {
            alphas += 1;
            assert_eq!(
                r.focus, FOCUS_NEST_ALPHA,
                "an alpha pentagon came from outside the nest"
            );
            assert!(
                nest.contains(r.pos, side),
                "an alpha pentagon was placed outside the nest disc at {:?}",
                r.pos
            );
        }
        if r.focus == FOCUS_SCATTER && nest.contains(r.pos, side) {
            scatter_in_nest += 1;
        }
    }
    assert!(pentagons > 0, "the nest never produced a pentagon");
    assert!(alphas > 0, "the nest never produced an alpha pentagon");
    // The scatter focus deliberately does not exclude the nest: the centre is meant
    // to be the densest ground on the map, not a clearing with pentagons in it.
    assert!(
        scatter_in_nest > 0,
        "the scatter focus should be free to place inside the nest"
    );
}

#[test]
fn the_nest_pays_five_times_the_rate_elsewhere() {
    // Lift every band out of the way so nothing throttles, then count what each
    // focus asks for over a minute of simulated time.
    let mut config = SpawnConfig::default();
    for f in &mut config.foci {
        f.prefill = 0;
        f.num_slow = usize::MAX;
        f.num_max = usize::MAX;
    }
    let w = world();
    let mut s = CompositeSpawner::new(config);
    let mut rng = StdRng::seed_from_u64(77);

    let (mut pentagons, mut nest_minor, mut scatter) = (0.0f32, 0.0f32, 0.0f32);
    let secs = 120;
    for _ in 0..(secs * TICK_HZ) {
        // Not applied: this measures the rate, not the packing.
        for r in s.tick(&w, w.tick(), &mut rng) {
            match (r.focus, r.tier) {
                (FOCUS_NEST, ShapeTier::Pentagon) => pentagons += 1.0,
                (FOCUS_NEST, _) => nest_minor += 1.0,
                (FOCUS_SCATTER, _) => scatter += 1.0,
                _ => {}
            }
        }
    }

    let ratio = pentagons / scatter;
    assert!(
        (ratio - NEST_PENTAGON_RATE_MULT).abs() < 0.25,
        "pentagons should spawn at {NEST_PENTAGON_RATE_MULT} times the baseline, got {ratio}"
    );
    let minor = nest_minor / scatter;
    assert!(
        (minor - NEST_MINOR_RATE_MULT).abs() < 0.1,
        "the nest's common shapes should spawn at {NEST_MINOR_RATE_MULT} times the \
         baseline, got {minor}"
    );
}

/// The reason alpha pentagons exist, stated as arithmetic.
///
/// A team that holds the nest has to out-earn a team farming both wings without
/// contest. A wing is spawn-limited, so its yield is its spawn rate times the
/// average value of what it spawns. The alphas alone must cover that.
#[test]
fn alphas_cover_the_yield_of_both_wings() {
    let config = SpawnConfig::default();
    let wing = config.focus(FOCUS_WING_NE).expect("wing");

    let total: f32 = wing.tier_weights.iter().map(|(_, w)| w).sum();
    let wing_value: f32 = wing
        .tier_weights
        .iter()
        .map(|(tier, w)| {
            let v = match tier {
                ShapeTier::Square => SHAPE_VALUE_SQUARE,
                ShapeTier::Triangle => SHAPE_VALUE_TRIANGLE,
                other => panic!("a wing should not spawn {other:?}"),
            };
            v as f32 * w / total
        })
        .sum();
    let both_wings = 2.0 * WING_SPAWN_RATE * wing_value;
    let alphas = NEST_ALPHA_RATE * SHAPE_VALUE_ALPHA as f32;

    assert!(
        alphas >= both_wings,
        "alphas yield {alphas:.1} points per second, two uncontested wings yield \
         {both_wings:.1}. The nest must at least match them."
    );
    // And not by so much that contesting the centre is the only sane play.
    assert!(
        alphas <= both_wings * 1.5,
        "alphas yield {alphas:.1} against the wings' {both_wings:.1}, which makes \
         the wings pointless rather than a trade-off"
    );
}

#[test]
fn a_resident_focus_replaces_what_leaves_it() {
    let mut w = world();
    let mut s = CompositeSpawner::new(SpawnConfig::default());
    let mut rng = StdRng::seed_from_u64(83);
    let seeded = s.prefill(&w, &mut rng);
    spawn::apply(&mut w, &seeded);

    let nest = w.spec().arena.nest;
    let side = w.spec().arena.side;
    let resident = |w: &World| {
        w.iter_entities()
            .filter(|(_, e)| {
                e.is_shape() && e.focus == Some(FOCUS_NEST) && nest.contains(e.pos, side)
            })
            .count()
    };
    let before = resident(&w);
    assert!(before > 0, "the nest should start populated");

    // Teleport every nest shape to the far corner. Nothing died, so a focus that
    // counted by origin would see no change and never respawn.
    let ids: Vec<_> = w
        .iter_entities()
        .filter(|(_, e)| e.is_shape() && e.focus == Some(FOCUS_NEST))
        .map(|(id, _)| id)
        .collect();
    for (n, id) in ids.iter().enumerate() {
        if let Some(e) = w.entity_mut(*id) {
            e.pos = Vec2::new(
                700.0 + (n % 10) as f32 * 20.0,
                60.0 + (n / 10) as f32 * 20.0,
            );
        }
    }
    assert_eq!(resident(&w), 0, "the nest should now be empty of its own");

    run(&mut w, &mut s, &mut rng, 30 * TICK_HZ);
    assert!(
        resident(&w) > before / 2,
        "the nest should have refilled, holds {}",
        resident(&w)
    );
}

#[test]
fn nothing_overlaps_at_the_moment_it_is_placed() {
    let w = world();
    let mut s = CompositeSpawner::new(SpawnConfig::default());
    let mut rng = StdRng::seed_from_u64(21);

    let reqs = s.prefill(&w, &mut rng);
    for (i, a) in reqs.iter().enumerate() {
        let ra = shape_radius(a.tier);
        for b in &reqs[i + 1..] {
            let reach = ra + shape_radius(b.tier);
            assert!(
                a.pos.distance_squared(b.pos) >= reach * reach,
                "two shapes were placed on top of each other"
            );
        }
    }
}

#[test]
fn throttle_is_a_linear_ramp() {
    let config = SpawnConfig::default();
    let f = config.focus(FOCUS_SCATTER).expect("scatter focus");

    assert_eq!(f.throttle(0), 1.0);
    assert_eq!(f.throttle(f.num_slow), 1.0);
    assert_eq!(f.throttle(f.num_max), 0.0);
    assert_eq!(f.throttle(f.num_max + 500), 0.0);

    let mid = (f.num_slow + f.num_max) / 2;
    let half = f.throttle(mid);
    assert!(
        (half - 0.5).abs() < 0.01,
        "halfway up the band the rate should be halved, got {half}"
    );
    assert!(f.throttle(f.num_slow + 1) > f.throttle(f.num_max - 1));
}

#[test]
fn a_culled_focus_refills() {
    let mut w = world();
    let mut s = CompositeSpawner::new(SpawnConfig::default());
    let mut rng = StdRng::seed_from_u64(31);
    let seeded = s.prefill(&w, &mut rng);
    spawn::apply(&mut w, &seeded);

    let ids: Vec<_> = w
        .iter_entities()
        .filter(|(_, e)| e.is_shape() && e.focus == Some(FOCUS_NEST))
        .map(|(id, _)| id)
        .collect();
    assert!(!ids.is_empty(), "the nest should start populated");
    for id in ids {
        if let Some(e) = w.entity_mut(id) {
            e.hp = 0.0;
        }
    }
    w.step(&Inputs::default());
    assert_eq!(alive(&w, FOCUS_NEST), 0, "the cull should have emptied it");

    run(&mut w, &mut s, &mut rng, 60 * TICK_HZ);
    let ceiling = s.config().focus(FOCUS_NEST).expect("nest").num_max;
    let back = s
        .last_census()
        .into_iter()
        .find(|c| c.focus == FOCUS_NEST)
        .map(|c| c.alive)
        .expect("nest is censused");
    assert!(back > 0, "the nest should refill after being cleared");
    assert!(
        back <= ceiling + REENTRY_SLACK,
        "the nest holds {back}, above its ceiling of {ceiling}"
    );
}

#[test]
fn no_tick_requests_more_than_the_headroom() {
    let mut w = world();
    let mut s = CompositeSpawner::new(SpawnConfig::default());
    let mut rng = StdRng::seed_from_u64(97);
    let seeded = s.prefill(&w, &mut rng);
    spawn::apply(&mut w, &seeded);

    for _ in 0..(120 * TICK_HZ) {
        // The census the spawner will act on, taken before it acts.
        s.census(&w);
        let before: Vec<_> = s.last_census();
        let requests = s.tick(&w, w.tick(), &mut rng);

        for focus in &s.config().foci {
            let alive = before
                .iter()
                .find(|c| c.focus == focus.id)
                .map(|c| c.alive)
                .expect("censused");
            let asked = requests.iter().filter(|r| r.focus == focus.id).count();
            assert!(
                alive + asked <= focus.num_max.max(alive),
                "focus {:?} held {alive} and asked for {asked}, ceiling {}",
                focus.id,
                focus.num_max
            );
        }
        spawn::apply(&mut w, &requests);
        w.step(&Inputs::default());
    }
}

#[test]
fn a_disabled_focus_can_be_switched_on() {
    let mut config = SpawnConfig::default();
    for f in &mut config.foci {
        if f.id == FOCUS_WING_NE || f.id == FOCUS_WING_SW {
            f.enabled = true;
        }
    }
    let mut w = world();
    let mut s = CompositeSpawner::new(config);
    let mut rng = StdRng::seed_from_u64(41);
    let reqs = s.prefill(&w, &mut rng);
    spawn::apply(&mut w, &reqs);

    let wings: Vec<_> = s
        .last_census()
        .into_iter()
        .filter(|c| c.focus == FOCUS_WING_NE || c.focus == FOCUS_WING_SW)
        .collect();
    assert_eq!(wings.len(), 2, "two wings");
    for wing in wings {
        assert!(wing.alive > 0, "an enabled wing must populate");
    }
}

#[test]
fn a_region_smaller_than_its_band_does_not_spin() {
    // A disc with room for a handful of shapes, told it may hold two thousand.
    // The placement budget is what has to stop this, not the population band.
    let mut config = SpawnConfig::default();
    config.foci.retain(|f| f.id == FOCUS_SCATTER);
    config.foci[0].region = Region::Disc {
        center: Vec2::new(500.0, 500.0),
        radius: 40.0,
    };
    config.foci[0].exclude.clear();
    config.foci[0].prefill = 2000;

    let w = world();
    let mut s = CompositeSpawner::new(config);
    let mut rng = StdRng::seed_from_u64(51);
    let reqs = s.prefill(&w, &mut rng);

    assert!(!reqs.is_empty(), "it should place what fits");
    assert!(
        reqs.len() < 200,
        "a disc of radius 40 cannot hold {} shapes",
        reqs.len()
    );
}

/// `docs/DESIGN.md`: "Every spawned entity records its originating `FocusId` in the
/// event log. That is what makes 'which farming area did the team prioritise' a
/// query rather than a guess."
///
/// Asserted end to end, through the spawner and out of the world's event stream,
/// rather than by inspecting entities. The event log is what a recorded match is
/// read back from, so it is the log that has to carry the attribution.
#[test]
fn every_spawned_shape_is_attributed_to_its_focus_in_the_event_log() {
    let mut config = SpawnConfig::default();
    for f in &mut config.foci {
        f.enabled = true;
    }
    let enabled: Vec<_> = config.foci.iter().map(|f| f.id).collect();

    let mut w = world();
    let mut s = CompositeSpawner::new(config);
    let mut rng = StdRng::seed_from_u64(1234);

    // Discard the tanks and control centers the world places on construction.
    w.drain_events();

    let requests = s.prefill(&w, &mut rng);
    spawn::apply(&mut w, &requests);
    for _ in 0..(60 * TICK_HZ) {
        let more = s.tick(&w, w.tick(), &mut rng);
        spawn::apply(&mut w, &more);
        w.step(&Inputs::default());
    }

    let mut seen: BTreeMap<schema::FocusId, usize> = BTreeMap::new();
    let mut shapes = 0;
    for event in w.drain_events() {
        let schema::Event::Spawned {
            kind, focus, id, ..
        } = event
        else {
            continue;
        };
        if kind != schema::Kind::Shape {
            continue;
        }
        shapes += 1;
        let focus = focus.unwrap_or_else(|| panic!("shape {id:?} spawned with no focus"));
        *seen.entry(focus).or_default() += 1;
    }

    assert!(shapes > 600, "expected a populated arena, logged {shapes}");
    for id in &enabled {
        assert!(
            seen.get(id).copied().unwrap_or(0) > 0,
            "focus {id:?} spawned nothing, so nothing in the log attributes to it"
        );
    }
    assert_eq!(
        seen.keys().copied().collect::<Vec<_>>(),
        enabled
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
        "the log names a focus that is not in the configuration"
    );
}

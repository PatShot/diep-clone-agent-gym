//! A full-length match, to catch what short tests cannot: drift into NaN,
//! unbounded entity growth, and entities escaping the arena after many thousands
//! of contacts.

use schema::{Action, AgentId, Control, ShapeTier, Vec2};
use sim_core::constants::*;
use sim_core::world::Inputs;
use sim_core::{ArenaSpec, World, WorldSpec};

#[test]
fn a_full_length_match_stays_finite_and_bounded() {
    let mut w = World::new(WorldSpec {
        seed: 20260904,
        arena: ArenaSpec::default(),
        tanks_per_team: 5,
        match_id: "soak".into(),
        config_hash: 1,
        // On here, so the soak keeps exercising the recording path. It is off by
        // default: nothing in v0 consumes the rows, and left on they accumulate
        // until a caller drains them.
        record_kinematics: true,
    });

    // Seed 200 shapes by hand. The spawner is step 3; this only needs traffic.
    let mut st = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        (st >> 11) as f32 / (1u64 << 53) as f32
    };
    for i in 0..200 {
        let tier = if i % 20 == 0 {
            ShapeTier::Pentagon
        } else {
            ShapeTier::Square
        };
        let a = next() * std::f32::consts::TAU;
        w.spawn_shape(
            tier,
            Vec2::new(next() * ARENA_SIDE, next() * ARENA_SIDE),
            Vec2::new(a.cos() * SHAPE_DRIFT_SPEED, a.sin() * SHAPE_DRIFT_SPEED),
            None,
        );
    }

    let agents: Vec<AgentId> = w.agents().keys().copied().collect();
    // 90 minutes at 25 Hz.
    let total = 90 * 60 * TICK_HZ;
    let mut peak = 0usize;

    for t in 0..total {
        let angle = t as f32 * 0.013;
        let actions = agents
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let th = angle + i as f32 * 1.257;
                (
                    *a,
                    Action {
                        control: Control {
                            thrust: Vec2::new(th.cos(), th.sin()),
                            aim: -th,
                            fire: true,
                        },
                        ..Action::default()
                    },
                )
            })
            .collect();
        w.step(&Inputs {
            actions,
            commands: Vec::new(),
        });
        // Drain, as the event and kinematics sinks will. Left to accumulate, the
        // buffers would be the only thing that grew without bound.
        w.drain_events();
        w.drain_kinematics();
        peak = peak.max(w.entity_count());
    }

    assert_eq!(w.tick().0, total);

    for (id, e) in w.iter_entities() {
        assert!(
            e.pos.x.is_finite() && e.pos.y.is_finite(),
            "{id:?} position went non-finite: {:?}",
            e.pos
        );
        assert!(
            e.vel.x.is_finite() && e.vel.y.is_finite(),
            "{id:?} velocity went non-finite: {:?}",
            e.vel
        );
        assert!(
            e.pos.x >= -1.0
                && e.pos.y >= -1.0
                && e.pos.x <= ARENA_SIDE + 1.0
                && e.pos.y <= ARENA_SIDE + 1.0,
            "{id:?} escaped the arena at {:?}",
            e.pos
        );
        assert!(
            e.hp.is_finite() || e.is_inert(),
            "{id:?} health went non-finite"
        );
    }

    // Ten tanks, two control centers, 200 shapes, and bullets in flight. Bullets
    // are the only unbounded term, and their lifetime caps them.
    assert!(
        peak < 400,
        "entity count peaked at {peak}; something is not being reaped"
    );
    // Every agent is alive or waiting on a respawn, none stranded.
    for (agent, s) in w.agents() {
        assert!(
            s.entity.is_some() || s.respawn_at.is_some(),
            "{agent:?} was left neither alive nor scheduled to respawn"
        );
    }
}

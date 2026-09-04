//! Behavioural tests for the headless world.
//!
//! These assert the claims made in `constants.rs` and `docs/DESIGN.md`, not the
//! implementation. If a test here fails, either the physics changed or the
//! documented design did.

use schema::{Action, AgentId, Cause, Control, Event, Kind, ShapeTier, TeamId, Tick, Vec2};
use sim_core::constants::*;
use sim_core::entity::Entity;
use sim_core::store::{to_id, to_key};
use sim_core::world::Inputs;
use sim_core::{ArenaSpec, World, WorldSpec};

/// Step one tick with a single agent thrusting in `dir`.
fn push(w: &mut World, agent: AgentId, dir: Vec2) {
    w.step(&Inputs {
        actions: vec![drive(agent, dir, 0.0, false)],
        commands: Vec::new(),
    });
}

/// Set an entity's health directly, to reach a state without simulating the
/// fight that would produce it.
fn set_hp(w: &mut World, id: schema::EntityId, hp: f32) {
    w.entity_mut(id).expect("entity is alive").hp = hp;
}

fn spec(seed: u64) -> WorldSpec {
    WorldSpec {
        seed,
        arena: ArenaSpec::default(),
        tanks_per_team: 5,
        match_id: "test".into(),
        config_hash: 7,
    }
}

/// A world with no tanks, for isolating one mechanic at a time.
fn empty_world() -> World {
    World::new(WorldSpec {
        tanks_per_team: 0,
        ..spec(1)
    })
}

fn drive(agent: AgentId, thrust: Vec2, aim: f32, fire: bool) -> (AgentId, Action) {
    (
        agent,
        Action {
            control: Control { thrust, aim, fire },
            ..Action::default()
        },
    )
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

#[test]
fn match_starts_with_tanks_and_control_centers() {
    let w = World::new(spec(42));
    // Five tanks and one control center per team.
    assert_eq!(w.entity_count(), 12);
    assert_eq!(w.agents().len(), 12);

    let ccs = w
        .iter_entities()
        .filter(|(_, e)| e.kind == Kind::ControlCenter)
        .count();
    assert_eq!(ccs, 2);

    // Every tank starts inside its own base.
    for (_, e) in w.iter_entities().filter(|(_, e)| e.is_tank()) {
        let team = e.team.expect("a tank has a team");
        assert!(
            w.arena.base(team).contains(e.pos),
            "tank spawned outside its base at {:?}",
            e.pos
        );
    }
}

#[test]
fn match_start_is_the_first_event() {
    let mut w = World::new(spec(42));
    let events = w.drain_events();
    assert!(matches!(
        events.first(),
        Some(Event::MatchStart { config_hash: 7, .. })
    ));
}

#[test]
fn control_centers_are_not_reaped_and_never_move() {
    let mut w = World::new(spec(3));
    let cc = w
        .iter_entities()
        .find(|(_, e)| e.kind == Kind::ControlCenter)
        .map(|(id, e)| (id, e.pos))
        .expect("a control center exists");
    for _ in 0..50 {
        w.step(&Inputs::default());
    }
    let after = w.entity(cc.0).expect("control center survives");
    assert_eq!(after.pos, cc.1);
}

// ---------------------------------------------------------------------------
// Movement
// ---------------------------------------------------------------------------

#[test]
fn full_thrust_converges_on_the_documented_top_speed() {
    let mut w = empty_world();
    let a = AgentId(0);
    let id = w.add_tank_at(a, TeamId::A, Vec2::new(500.0, 500.0), 0.0);
    for _ in 0..200 {
        push(&mut w, a, Vec2::new(1.0, 0.0));
    }
    let speed = w.entity(id).expect("alive").vel.length();
    assert!(
        (speed - TANK_MAX_SPEED).abs() < 0.5,
        "terminal speed was {speed}, expected about {TANK_MAX_SPEED}"
    );
}

#[test]
fn a_tank_cannot_leave_the_arena() {
    let mut w = empty_world();
    let a = AgentId(0);
    let id = w.add_tank_at(a, TeamId::A, Vec2::new(900.0, 500.0), 0.0);
    for _ in 0..300 {
        push(&mut w, a, Vec2::new(1.0, 0.0));
    }
    let e = w.entity(id).expect("alive");
    assert!(e.pos.x <= ARENA_SIDE - TANK_RADIUS + 0.001, "{:?}", e.pos);
    assert!(e.pos.x > ARENA_SIDE - TANK_RADIUS - 1.0, "{:?}", e.pos);
}

#[test]
fn thrust_longer_than_unit_length_buys_no_extra_speed() {
    let mut a = empty_world();
    let mut b = empty_world();
    let ag = AgentId(0);
    let ida = a.add_tank_at(ag, TeamId::A, Vec2::new(500.0, 500.0), 0.0);
    let idb = b.add_tank_at(ag, TeamId::A, Vec2::new(500.0, 500.0), 0.0);
    for _ in 0..100 {
        push(&mut a, ag, Vec2::new(1.0, 0.0));
        push(&mut b, ag, Vec2::new(1000.0, 0.0));
    }
    let (va, vb) = (
        a.entity(ida).expect("alive").vel.length(),
        b.entity(idb).expect("alive").vel.length(),
    );
    assert!((va - vb).abs() < 1e-3, "{va} vs {vb}");
}

// ---------------------------------------------------------------------------
// The base sanctuary
// ---------------------------------------------------------------------------

#[test]
fn an_enemy_tank_is_kept_out_of_a_base() {
    let mut w = empty_world();
    // Team B tank driving northwest, into team A's base.
    let a = AgentId(0);
    let id = w.add_tank_at(a, TeamId::B, Vec2::new(260.0, 100.0), std::f32::consts::PI);
    for _ in 0..300 {
        push(&mut w, a, Vec2::new(-1.0, 0.0));
    }
    let e = w.entity(id).expect("alive");
    assert!(
        !w.arena.base(TeamId::A).overlaps_circle(e.pos, e.radius),
        "enemy tank entered the sanctuary at {:?}",
        e.pos
    );
}

#[test]
fn a_tank_moves_freely_inside_its_own_base() {
    let mut w = empty_world();
    let a = AgentId(0);
    let id = w.add_tank_at(a, TeamId::A, Vec2::new(150.0, 100.0), std::f32::consts::PI);
    for _ in 0..100 {
        push(&mut w, a, Vec2::new(-1.0, 0.0));
    }
    let e = w.entity(id).expect("alive");
    assert!(e.pos.x < 100.0, "own tank was blocked at {:?}", e.pos);
    assert!(w.arena.base(TeamId::A).contains(e.pos));
}

#[test]
fn enemy_fire_is_destroyed_at_the_base_boundary() {
    let mut w = empty_world();
    let owner = w.spawn(Entity::tank(
        AgentId(0),
        TeamId::B,
        Vec2::new(400.0, 100.0),
        0.0,
    ));
    let bullet = w.spawn(Entity::bullet(
        owner,
        TeamId::B,
        Vec2::new(230.0, 100.0),
        Vec2::new(-BULLET_SPEED, 0.0),
        Tick::ZERO,
    ));
    w.drain_events();

    let mut absorbed = false;
    for _ in 0..BULLET_LIFETIME_TICKS {
        w.step(&Inputs::default());
        for ev in w.drain_events() {
            if let Event::Despawned { id, cause } = ev {
                if id == bullet {
                    assert_eq!(cause, Cause::Absorbed);
                    absorbed = true;
                }
            }
        }
        if absorbed {
            break;
        }
    }
    assert!(
        absorbed,
        "enemy bullet was not absorbed at the base boundary"
    );
}

#[test]
fn friendly_fire_crosses_its_own_base_boundary() {
    let mut w = empty_world();
    let owner = w.spawn(Entity::tank(
        AgentId(0),
        TeamId::A,
        Vec2::new(400.0, 100.0),
        0.0,
    ));
    let bullet = w.spawn(Entity::bullet(
        owner,
        TeamId::A,
        Vec2::new(230.0, 100.0),
        Vec2::new(-BULLET_SPEED, 0.0),
        Tick::ZERO,
    ));
    w.step(&Inputs::default());
    let e = w.entity(bullet).expect("own bullet survives its own base");
    assert!(e.pos.x < 230.0);
}

// ---------------------------------------------------------------------------
// Bullets
// ---------------------------------------------------------------------------

#[test]
fn a_bullet_expires_at_its_documented_range() {
    let mut w = empty_world();
    let owner = w.spawn(Entity::tank(
        AgentId(0),
        TeamId::A,
        Vec2::new(100.0, 500.0),
        0.0,
    ));
    let start = Vec2::new(200.0, 500.0);
    let bullet = w.spawn(Entity::bullet(
        owner,
        TeamId::A,
        start,
        Vec2::new(BULLET_SPEED, 0.0),
        Tick::ZERO,
    ));

    let mut travelled = 0.0;
    for _ in 0..BULLET_LIFETIME_TICKS + 2 {
        w.step(&Inputs::default());
        match w.entity(bullet) {
            Some(e) => travelled = e.pos.distance(start),
            None => break,
        }
    }
    assert!(w.entity(bullet).is_none(), "bullet outlived its lifetime");

    // Range is speed times lifetime, and the constants file claims that lands just
    // past the sense radius so a tank can shoot as far as it can see.
    let expected = BULLET_SPEED * BULLET_LIFETIME_TICKS as f32 * DT;
    assert!((travelled - expected).abs() < BULLET_SPEED * DT + 0.5);
    assert!(
        travelled >= TANK_SENSE_RADIUS,
        "range {travelled} fell short of sight"
    );
}

#[test]
fn a_bullet_never_hits_its_owner_or_a_teammate() {
    let mut w = empty_world();
    let owner = w.spawn(Entity::tank(
        AgentId(0),
        TeamId::A,
        Vec2::new(500.0, 500.0),
        0.0,
    ));
    let mate = w.spawn(Entity::tank(
        AgentId(1),
        TeamId::A,
        Vec2::new(520.0, 500.0),
        0.0,
    ));
    w.spawn(Entity::bullet(
        owner,
        TeamId::A,
        Vec2::new(505.0, 500.0),
        Vec2::ZERO,
        Tick::ZERO,
    ));
    for _ in 0..5 {
        w.step(&Inputs::default());
    }
    assert_eq!(w.entity(owner).expect("alive").hp, tank_hp());
    assert_eq!(w.entity(mate).expect("alive").hp, tank_hp());
}

#[test]
fn a_bullet_damages_an_enemy_once_not_once_per_tick() {
    let mut w = empty_world();
    let shooter = w.spawn(Entity::tank(
        AgentId(0),
        TeamId::B,
        Vec2::new(400.0, 500.0),
        0.0,
    ));
    let target = w.spawn(Entity::tank(
        AgentId(1),
        TeamId::A,
        Vec2::new(500.0, 500.0),
        0.0,
    ));
    // A stationary bullet parked inside the target: it stays in contact for many
    // ticks, so anything other than one-shot damage shows up immediately.
    w.spawn(Entity::bullet(
        shooter,
        TeamId::B,
        Vec2::new(500.0, 500.0),
        Vec2::ZERO,
        Tick::ZERO,
    ));

    w.step(&Inputs::default());
    let after_one = w.entity(target).expect("alive").hp;
    assert!((tank_hp() - after_one - BULLET_DAMAGE).abs() < 1e-3);

    for _ in 0..5 {
        w.step(&Inputs::default());
    }
    let later = w.entity(target).expect("alive").hp;
    assert!(
        later <= after_one + 1e-3,
        "target healed while a bullet sat inside it"
    );
    assert!(
        later >= after_one - 1e-3,
        "bullet dealt {} extra damage over five ticks of contact",
        after_one - later
    );
}

#[test]
fn firing_respects_the_reload_gate_and_pushes_the_tank_back() {
    let mut w = World::new(spec(9));
    let agent = AgentId(0);
    let id = w.agents()[&agent].entity.expect("alive");
    let before = w.entity(id).expect("alive").pos;

    // Aim east and hold the trigger for one reload interval plus a tick.
    let mut fired = 0;
    for _ in 0..RELOAD_TICKS + 1 {
        w.drain_events();
        w.step(&Inputs {
            actions: vec![drive(agent, Vec2::ZERO, 0.0, true)],
            commands: Vec::new(),
        });
        fired += w
            .drain_events()
            .into_iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::Spawned {
                        kind: Kind::Bullet,
                        ..
                    }
                )
            })
            .count();
    }
    assert_eq!(
        fired, 2,
        "reload gate let through {fired} shots, expected 2"
    );

    let after = w.entity(id).expect("alive").pos;
    assert!(after.x < before.x, "recoil did not push the tank back");
}

// ---------------------------------------------------------------------------
// Collision
// ---------------------------------------------------------------------------

#[test]
fn overlapping_tanks_push_apart() {
    let mut w = empty_world();
    let a = w.spawn(Entity::tank(
        AgentId(0),
        TeamId::A,
        Vec2::new(500.0, 500.0),
        0.0,
    ));
    let b = w.spawn(Entity::tank(
        AgentId(1),
        TeamId::A,
        Vec2::new(505.0, 500.0),
        0.0,
    ));
    for _ in 0..60 {
        w.step(&Inputs::default());
    }
    let (ea, eb) = (w.entity(a).expect("alive"), w.entity(b).expect("alive"));
    let gap = ea.pos.distance(eb.pos);
    assert!(
        gap >= ea.radius + eb.radius - 0.5,
        "tanks stayed overlapped at {gap}"
    );
}

#[test]
fn shapes_do_not_damage_each_other() {
    let mut w = empty_world();
    let a = w.spawn_shape(ShapeTier::Common, Vec2::new(500.0, 500.0), Vec2::ZERO, None);
    let b = w.spawn_shape(ShapeTier::Common, Vec2::new(504.0, 500.0), Vec2::ZERO, None);
    for _ in 0..30 {
        w.step(&Inputs::default());
    }
    assert_eq!(w.entity(a).expect("alive").hp, SHAPE_HP_COMMON);
    assert_eq!(w.entity(b).expect("alive").hp, SHAPE_HP_COMMON);
}

#[test]
fn a_tank_and_a_shape_damage_each_other_on_contact() {
    let mut w = empty_world();
    let tank = w.spawn(Entity::tank(
        AgentId(0),
        TeamId::A,
        Vec2::new(500.0, 500.0),
        0.0,
    ));
    let shape = w.spawn_shape(ShapeTier::Common, Vec2::new(512.0, 500.0), Vec2::ZERO, None);
    w.step(&Inputs::default());
    assert!(w.entity(tank).expect("alive").hp < tank_hp());
    assert!(w.entity(shape).expect("alive").hp < SHAPE_HP_COMMON);
}

// ---------------------------------------------------------------------------
// Health, and the focus-fire threshold
// ---------------------------------------------------------------------------

#[test]
fn regeneration_waits_for_the_delay_then_heals() {
    let mut w = empty_world();
    let shooter = w.spawn(Entity::tank(
        AgentId(0),
        TeamId::B,
        Vec2::new(400.0, 500.0),
        0.0,
    ));
    let target = w.spawn(Entity::tank(
        AgentId(1),
        TeamId::A,
        Vec2::new(500.0, 500.0),
        0.0,
    ));
    let b = w.spawn(Entity::bullet(
        shooter,
        TeamId::B,
        Vec2::new(500.0, 500.0),
        Vec2::ZERO,
        Tick::ZERO,
    ));
    w.step(&Inputs::default());
    let hurt = w.entity(target).expect("alive").hp;
    assert!(hurt < tank_hp());

    // Remove the bullet so nothing keeps refreshing the damage stamp.
    let _ = b;
    while w.entity(b).is_some() {
        w.step(&Inputs::default());
    }
    let before_regen = w.entity(target).expect("alive").hp;

    for _ in 0..REGEN_DELAY_TICKS + 25 {
        w.step(&Inputs::default());
    }
    let healed = w.entity(target).expect("alive").hp;
    assert!(
        healed > before_regen,
        "no regeneration after the delay elapsed"
    );
    assert!(healed <= tank_hp() + 1e-3, "regenerated past maximum");
}

#[test]
fn regeneration_rate_matches_the_documented_fraction() {
    let mut w = empty_world();
    let target = w.spawn(Entity::tank(
        AgentId(0),
        TeamId::A,
        Vec2::new(500.0, 500.0),
        0.0,
    ));
    // Damage it well clear of the cap so the clamp does not mask the rate.
    let max = tank_hp();
    set_hp(&mut w, target, max * 0.2);

    let ticks = 10u32;
    for _ in 0..ticks {
        w.step(&Inputs::default());
    }
    let gained = w.entity(target).expect("alive").hp - max * 0.2;
    let expected = max * REGEN_FRACTION_PER_SEC * DT * ticks as f32;
    assert!(
        (gained - expected).abs() < 1e-2,
        "regenerated {gained}, expected {expected}"
    );
}

#[test]
fn two_attackers_kill_far_faster_than_one() {
    // The headline claim in `constants::REGEN_DELAY_TICKS`: focus fire is a
    // threshold, not a sum. Two attackers interleaving shots close the gaps below
    // the regeneration delay, so the target never heals between hits.
    let solo = ticks_to_kill(1);
    let pair = ticks_to_kill(2);

    let pair = pair.expect("two attackers must be able to kill a healthy tank");
    match solo {
        None => {} // A lone attacker never got there. Even sharper than claimed.
        Some(solo) => assert!(
            solo > pair * 2,
            "one attacker took {solo} ticks and two took {pair}; \
             the threshold has collapsed into a plain sum"
        ),
    }
}

/// Run a target against `n` attackers each firing on the reload cadence, evenly
/// offset in phase. Returns the tick it died on, or `None` inside 600 ticks.
fn ticks_to_kill(n: u32) -> Option<u32> {
    let mut w = empty_world();
    let shooter = w.spawn(Entity::tank(
        AgentId(0),
        TeamId::B,
        Vec2::new(300.0, 500.0),
        0.0,
    ));
    let target = w.spawn(Entity::tank(
        AgentId(1),
        TeamId::A,
        Vec2::new(500.0, 500.0),
        0.0,
    ));

    for t in 0..600u32 {
        for k in 0..n {
            let phase = (k * RELOAD_TICKS) / n;
            if t % RELOAD_TICKS == phase {
                // A stationary bullet placed on the target: one clean hit each,
                // with the firing geometry taken out of the measurement.
                w.spawn(Entity::bullet(
                    shooter,
                    TeamId::B,
                    Vec2::new(500.0, 500.0),
                    Vec2::ZERO,
                    w.tick(),
                ));
            }
        }
        w.step(&Inputs::default());
        if w.entity(target).is_none() {
            return Some(t);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

#[test]
fn a_killed_tank_respawns_in_its_base_after_the_delay() {
    let mut w = World::new(spec(11));
    let agent = AgentId(0);
    let id = w.agents()[&agent].entity.expect("alive");
    set_hp(&mut w, id, 0.0);
    w.step(&Inputs::default());

    assert!(w.entity(id).is_none(), "dead tank was not reaped");
    assert!(w.agents()[&agent].entity.is_none());
    let due = w.agents()[&agent].respawn_at.expect("respawn scheduled");

    while w.tick() <= due {
        w.step(&Inputs::default());
    }
    let new_id = w.agents()[&agent].entity.expect("respawned");
    assert_ne!(new_id, id, "respawn reused the dead handle");

    let e = w.entity(new_id).expect("alive");
    assert!(w.arena.base(TeamId::A).contains(e.pos));
    assert_eq!(e.hp, e.max_hp);
}

#[test]
fn a_kill_names_the_firing_tank_not_the_bullet() {
    let mut w = empty_world();
    let shooter = w.spawn(Entity::tank(
        AgentId(0),
        TeamId::B,
        Vec2::new(300.0, 500.0),
        0.0,
    ));
    let target = w.spawn(Entity::tank(
        AgentId(1),
        TeamId::A,
        Vec2::new(500.0, 500.0),
        0.0,
    ));
    set_hp(&mut w, target, 1.0);
    w.spawn(Entity::bullet(
        shooter,
        TeamId::B,
        Vec2::new(500.0, 500.0),
        Vec2::ZERO,
        Tick::ZERO,
    ));
    w.drain_events();
    w.step(&Inputs::default());

    let killed = w
        .drain_events()
        .into_iter()
        .find_map(|e| match e {
            Event::Killed { target: t, killer } if t == target => Some(killer),
            _ => None,
        })
        .expect("a Killed event was emitted");
    assert_eq!(killed, shooter, "credit went to the bullet, not the firer");
}

// ---------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------

#[test]
fn a_stale_handle_never_aliases_a_recycled_slot() {
    let mut w = empty_world();
    let first = w.spawn(Entity::tank(
        AgentId(0),
        TeamId::A,
        Vec2::new(500.0, 500.0),
        0.0,
    ));
    set_hp(&mut w, first, 0.0);
    w.step(&Inputs::default());
    assert!(w.entity(first).is_none());

    // Fill the freed slot. Its index may well be reused; its generation must not.
    let mut reused = None;
    for _ in 0..4 {
        let id = w.spawn(Entity::tank(
            AgentId(1),
            TeamId::A,
            Vec2::new(400.0, 400.0),
            0.0,
        ));
        if id.index == first.index {
            reused = Some(id);
        }
    }
    let reused = reused.expect("the freed slot index was reused");
    assert_ne!(reused.generation, first.generation);
    assert!(
        w.entity(first).is_none(),
        "the stale handle resolved to the new occupant"
    );
}

#[test]
fn handles_round_trip_through_the_internal_key() {
    let mut w = empty_world();
    for i in 0..8 {
        let id = w.spawn(Entity::tank(
            AgentId(i),
            TeamId::A,
            Vec2::new(400.0, 400.0),
            0.0,
        ));
        assert_eq!(to_id(to_key(id)), id);
        assert_eq!(schema::EntityId::from_u64(id.to_u64()), id);
    }
}

// ---------------------------------------------------------------------------
// Observation
// ---------------------------------------------------------------------------

#[test]
fn an_agent_sees_only_inside_its_sense_radius() {
    let mut w = empty_world();
    let agent = AgentId(0);
    // Open arena, well clear of either base. A shape spawned inside a sanctuary is
    // ejected from it, which would move the subject of this test rather than the
    // sensing being measured.
    let at = Vec2::new(500.0, 500.0);
    w.add_tank_at(agent, TeamId::A, at, 0.0);

    let near = Vec2::new(at.x + TANK_SENSE_RADIUS * 0.5, at.y);
    let far = Vec2::new(at.x + TANK_SENSE_RADIUS * 3.0, at.y);
    let seen = w.spawn_shape(ShapeTier::Common, near, Vec2::ZERO, None);
    let unseen = w.spawn_shape(ShapeTier::Common, far, Vec2::ZERO, None);
    w.step(&Inputs::default());

    let obs = w.observe(agent).expect("the agent exists");
    let ids: Vec<_> = obs.visible.iter().map(|v| v.id).collect();
    assert!(
        ids.contains(&seen),
        "missed a shape inside the sense radius"
    );
    assert!(
        !ids.contains(&unseen),
        "saw a shape outside the sense radius"
    );
    assert_eq!(obs.own.sense_radius, TANK_SENSE_RADIUS);
    assert_eq!(obs.own.comms_radius, TANK_COMMS_RADIUS);
}

#[test]
fn every_tank_gets_the_same_sense_and_comms_radius() {
    // v0 runs one tank type so that range-limited communication is the only
    // asymmetry under study. See `docs/DESIGN.md`, Progression.
    let mut w = World::new(spec(6));
    let tanks: Vec<AgentId> = w
        .agents()
        .iter()
        .filter(|(_, s)| !s.is_cc)
        .map(|(a, _)| *a)
        .collect();
    for a in tanks {
        let obs = w.observe(a).expect("the agent exists");
        assert_eq!(obs.own.sense_radius, TANK_SENSE_RADIUS);
        assert_eq!(obs.own.comms_radius, TANK_COMMS_RADIUS);
        assert_eq!(obs.own.class, None, "v0 assigns no class");
    }
}

#[test]
fn a_dead_agent_still_receives_an_observation() {
    let mut w = World::new(spec(7));
    let agent = AgentId(0);
    let id = w.agents()[&agent].entity.expect("alive");
    set_hp(&mut w, id, 0.0);
    w.step(&Inputs::default());

    let obs = w.observe(agent).expect("a dead agent still observes");
    assert!(obs.own.entity.is_none());
    assert!(obs.own.respawn_at.is_some());
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

#[test]
fn one_seed_produces_one_event_stream() {
    let run = |seed: u64| -> String {
        let mut w = World::new(spec(seed));
        let agents: Vec<AgentId> = w.agents().keys().copied().collect();
        let mut out = String::new();
        for t in 0..120u32 {
            let actions = agents
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let angle = (t as f32 * 0.05) + i as f32;
                    drive(*a, Vec2::new(angle.cos(), angle.sin()), angle, t % 5 == 0)
                })
                .collect();
            w.step(&Inputs {
                actions,
                commands: Vec::new(),
            });
            for e in w.drain_events() {
                out.push_str(&serde_json::to_string(&e).expect("event serializes"));
                out.push('\n');
            }
        }
        out
    };

    let a = run(1234);
    let b = run(1234);
    assert_eq!(a, b, "the same seed produced two different event streams");
    assert_ne!(a, run(1235), "two seeds produced the same stream");
    assert!(a.len() > 1000, "the run produced almost no events");
}

#[test]
fn kinematics_stay_out_of_the_event_stream() {
    let mut w = World::new(spec(2));
    w.step(&Inputs::default());
    let events = w.drain_events();
    let rows = w.drain_kinematics();

    assert!(!rows.is_empty(), "no kinematics recorded");
    // Control centers never move, so they are not worth a row a tick.
    assert_eq!(rows.len(), w.entity_count() - 2);
    assert!(rows.iter().all(|k| k.tick == Tick::ZERO));
    assert!(
        !events.iter().any(|e| e.kind_str() == "kinematic"),
        "kinematics leaked into the event stream"
    );
}

#[test]
fn v0_emits_no_reserved_event() {
    let mut w = World::new(spec(8));
    let agents: Vec<AgentId> = w.agents().keys().copied().collect();
    for t in 0..200u32 {
        let actions = agents
            .iter()
            .map(|a| drive(*a, Vec2::new(1.0, 0.3), t as f32 * 0.1, true))
            .collect();
        w.step(&Inputs {
            actions,
            commands: Vec::new(),
        });
        for e in w.drain_events() {
            assert!(!e.is_reserved(), "v0 emitted a reserved event: {e:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers used by the assertions above
// ---------------------------------------------------------------------------

fn tank_hp() -> f32 {
    sim_core::tank_max_hp(1)
}

// ---------------------------------------------------------------------------
// Broadphase
// ---------------------------------------------------------------------------

/// The grid must never miss a contact the quadratic sweep would find.
///
/// A broadphase that drops pairs fails silently: bullets pass through tanks
/// occasionally and nothing in the event log says why. Checked against brute
/// force over randomised layouts, including the degenerate ones — coincident
/// points, and entities pressed into the arena corners.
#[test]
fn the_grid_finds_every_pair_brute_force_finds() {
    use sim_core::Grid;

    // A small deterministic generator, so a failure reproduces from the seed.
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f32 / (1u64 << 53) as f32
    };

    for round in 0..40 {
        let mut grid = Grid::new(ARENA_SIDE);
        let mut pts: Vec<(schema::EntityId, Vec2, f32)> = Vec::new();

        for i in 0..120u32 {
            let (x, y) = match round % 4 {
                // Spread across the arena.
                0 => (next() * ARENA_SIDE, next() * ARENA_SIDE),
                // Clustered, so many entities share a cell.
                1 => (400.0 + next() * 40.0, 400.0 + next() * 40.0),
                // Pressed into the corners, where cell clamping applies.
                2 => (
                    if i % 2 == 0 {
                        next() * 8.0
                    } else {
                        ARENA_SIDE - next() * 8.0
                    },
                    if i % 3 == 0 {
                        next() * 8.0
                    } else {
                        ARENA_SIDE - next() * 8.0
                    },
                ),
                // Exactly coincident.
                _ => (500.0, 500.0),
            };
            let r = MAX_ENTITY_RADIUS * (0.25 + 0.75 * next());
            let id = schema::EntityId::new(i, 1);
            grid.insert(id, Vec2::new(x, y));
            pts.push((id, Vec2::new(x, y), r));
        }

        let mut found = std::collections::BTreeSet::new();
        grid.for_each_pair(|a, b| {
            let key = if a.index < b.index {
                (a.index, b.index)
            } else {
                (b.index, a.index)
            };
            // Each candidate pair must be offered exactly once.
            assert!(found.insert(key), "grid offered pair {key:?} twice");
        });

        for i in 0..pts.len() {
            for j in (i + 1)..pts.len() {
                let (ia, pa, ra) = pts[i];
                let (ib, pb, rb) = pts[j];
                if pa.distance_squared(pb) < (ra + rb) * (ra + rb) {
                    let key = if ia.index < ib.index {
                        (ia.index, ib.index)
                    } else {
                        (ib.index, ia.index)
                    };
                    assert!(
                        found.contains(&key),
                        "round {round}: grid missed overlapping pair {key:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn shapes_reflect_off_walls_rather_than_piling_against_them() {
    // A shape that stopped at a wall would stay there for the whole match, and
    // the arena centre would empty out over ninety minutes.
    let mut w = empty_world();
    let s = w.spawn_shape(
        ShapeTier::Common,
        Vec2::new(ARENA_SIDE - 20.0, 500.0),
        Vec2::new(SHAPE_DRIFT_SPEED, 0.0),
        None,
    );
    for _ in 0..400 {
        w.step(&Inputs::default());
    }
    let e = w.entity(s).expect("alive");
    assert!(e.vel.x < 0.0, "shape did not reflect: vel {:?}", e.vel);
    assert!(
        e.pos.x < ARENA_SIDE - SHAPE_RADIUS_COMMON - 1.0,
        "shape stayed pinned to the wall at {:?}",
        e.pos
    );
}

#[test]
fn a_bullet_is_spent_against_a_wall_not_bounced() {
    let mut w = empty_world();
    let owner = w.spawn(Entity::tank(
        AgentId(0),
        TeamId::A,
        Vec2::new(500.0, 500.0),
        0.0,
    ));
    let b = w.spawn(Entity::bullet(
        owner,
        TeamId::A,
        Vec2::new(ARENA_SIDE - 10.0, 500.0),
        Vec2::new(BULLET_SPEED, 0.0),
        Tick::ZERO,
    ));
    w.step(&Inputs::default());
    if let Some(e) = w.entity(b) {
        assert!(e.vel.x <= 0.0 + 1e-6, "bullet bounced off the wall");
    }
}

#[test]
fn base_ejection_never_pushes_anything_out_of_the_arena() {
    // Regression. Bases sit flush in the arena corners, so the shortest way out of
    // the northwest base is often north or west — through the arena wall. An
    // entity pushed out there is outside every collision path and never returns.
    let w = empty_world();
    let side = w.arena.side;

    for team in [TeamId::A, TeamId::B] {
        let base = *w.arena.base(team);
        for radius in [BULLET_RADIUS, TANK_RADIUS, SHAPE_RADIUS_HIGH] {
            // Sweep the whole base, corners included.
            for i in 0..=20 {
                for j in 0..=20 {
                    // Sample positions the world could actually hold: the arena
                    // clamp runs every tick, so a centre nearer the wall than its
                    // own radius never reaches the ejection path.
                    let raw = Vec2::new(
                        base.min.x + (base.max.x - base.min.x) * i as f32 / 20.0,
                        base.min.y + (base.max.y - base.min.y) * j as f32 / 20.0,
                    );
                    let (p, _, _) = w.arena.clamp_circle(raw, radius);
                    let (out, _, _) = w.arena.eject_from_base(team, p, radius);
                    assert!(
                        out.x >= radius - 1e-3
                            && out.y >= radius - 1e-3
                            && out.x <= side - radius + 1e-3
                            && out.y <= side - radius + 1e-3,
                        "{team:?} base ejected radius {radius} from {p:?} to {out:?}, \
                         which is outside the arena"
                    );
                    assert!(
                        !base.overlaps_circle(out, radius),
                        "{team:?} base failed to eject radius {radius} from {p:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_shape_drifting_into_a_corner_base_stays_in_the_world() {
    let mut w = empty_world();
    let s = w.spawn_shape(
        ShapeTier::High,
        Vec2::new(60.0, 60.0),
        Vec2::new(-SHAPE_DRIFT_SPEED, -SHAPE_DRIFT_SPEED),
        None,
    );
    for _ in 0..500 {
        w.step(&Inputs::default());
        let e = w.entity(s).expect("the shape is still alive");
        assert!(
            e.pos.x >= 0.0 && e.pos.y >= 0.0 && e.pos.x <= ARENA_SIDE && e.pos.y <= ARENA_SIDE,
            "shape left the arena at {:?}",
            e.pos
        );
    }
}

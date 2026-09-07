//! The dense grid, from the outside: what it knows, what it forgets, what it will
//! say in a byte budget, and what it weighs.

use agent::model::{DenseGrid, WorldModel, MEMORY_CEILING_BYTES};
use schema::{
    AgentId, BeliefMsg, CellState, EntityId, EntityView, HealthGroup, Interest, Kind, Observation,
    PhysicsGroup, Pose, PositionGroup, Region, Scan, SelfView, StyleGroup, TeamGroup, TeamId, Tick,
    Track, Vec2,
};
use sim_core::constants::{ARENA_SIDE, DT, TANK_MAX_SPEED};

fn own(pos: Vec2) -> SelfView {
    SelfView {
        agent: AgentId(0),
        team: TeamId::A,
        entity: Some(EntityId {
            index: 1,
            generation: 0,
        }),
        pos,
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
    }
}

fn view(index: u32, kind: Kind, team: Option<TeamId>, pos: Vec2) -> EntityView {
    EntityView {
        id: EntityId {
            index,
            generation: 0,
        },
        position: PositionGroup { pos, heading: 0.0 },
        physics: PhysicsGroup {
            vel: Vec2::new(1.0, 0.0),
            radius: 10.0,
        },
        style: StyleGroup {
            kind,
            tier: None,
            class: None,
            stats: None,
            focus: None,
        },
        team: TeamGroup {
            team,
            agent: None,
            owner: None,
        },
        health: Some(HealthGroup {
            hp: 30.0,
            max_hp: 50.0,
            last_damaged: None,
        }),
        score: None,
    }
}

fn observation(tick: u32, at: Vec2, visible: Vec<EntityView>) -> Observation {
    Observation {
        tick: Tick(tick),
        own: own(at),
        visible,
        scan: None,
        inbox: Vec::new(),
        commands: Vec::new(),
        scores: vec![0, 0],
    }
}

fn centre(r: &Region) -> Vec2 {
    match *r {
        Region::Rect { min, max } => Vec2::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5),
        Region::Disc { center, .. } | Region::Annulus { center, .. } => center,
        Region::WholeArena => Vec2::ZERO,
    }
}

#[test]
fn a_sighting_frees_the_disc_and_nothing_beyond_it() {
    let mut m = DenseGrid::new(ARENA_SIDE);
    let me = Vec2::new(500.0, 500.0);
    assert_eq!(m.occupancy(me), CellState::Unknown);
    assert_eq!(m.confidence(me), 0.0);

    m.ingest_visible(&observation(10, me, Vec::new()));
    assert_eq!(m.occupancy(me), CellState::Free);
    assert_eq!(m.occupancy(Vec2::new(580.0, 500.0)), CellState::Free);
    assert_eq!(m.occupancy(Vec2::new(700.0, 500.0)), CellState::Unknown);
    assert!((m.confidence(me) - 1.0).abs() < 1e-6);

    m.tick(Tick(10 + 250));
    assert!(
        (m.confidence(me) - 0.5).abs() < 1e-3,
        "halves in a half-life"
    );
}

#[test]
fn shapes_occupy_cells_and_tanks_do_not() {
    let mut m = DenseGrid::new(ARENA_SIDE);
    let me = Vec2::new(500.0, 500.0);
    let shape_at = Vec2::new(540.0, 500.0);
    let tank_at = Vec2::new(460.0, 500.0);
    m.ingest_visible(&observation(
        1,
        me,
        vec![
            view(2, Kind::Shape, None, shape_at),
            view(3, Kind::Tank, Some(TeamId::B), tank_at),
        ],
    ));
    assert_eq!(m.occupancy(shape_at), CellState::Occupied);
    assert_eq!(
        m.occupancy(tank_at),
        CellState::Free,
        "tanks are tracks, not ground"
    );
    assert_eq!(m.tracks_near(me, 100.0).len(), 2);
}

#[test]
fn frontiers_are_the_edge_of_the_known_nearest_first() {
    let mut m = DenseGrid::new(ARENA_SIDE);
    assert!(
        m.frontiers(Vec2::new(500.0, 500.0), 4).is_empty(),
        "nothing known, no edge"
    );

    let me = Vec2::new(500.0, 500.0);
    m.ingest_visible(&observation(1, me, Vec::new()));
    let f = m.frontiers(me, 8);
    assert_eq!(f.len(), 8);
    for r in &f {
        let c = centre(r);
        assert_eq!(
            m.occupancy(c),
            CellState::Unknown,
            "a frontier cell is unknown"
        );
        let d = c.distance(me);
        assert!(
            d > 100.0 && d < 200.0,
            "on the rim of the sensed disc, at {d}"
        );
    }
    let d0 = centre(&f[0]).distance(me);
    let d7 = centre(&f[7]).distance(me);
    assert!(d0 <= d7, "nearest first");
}

#[test]
fn stale_ground_becomes_a_frontier_again() {
    let mut m = DenseGrid::with_resolution(ARENA_SIDE, 4, 8);
    let me = Vec2::new(500.0, 500.0);
    // A disc as wide as the arena: everything known, no unknown left.
    let mut o = observation(1, me, Vec::new());
    o.own.sense_radius = 2000.0;
    m.ingest_visible(&o);
    assert!(
        m.frontiers(me, 4).is_empty(),
        "everything fresh, nowhere to look"
    );
    m.tick(Tick(1 + 250));
    assert_eq!(m.frontiers(me, 4).len(), 4, "ten seconds later, everywhere");
}

#[test]
fn uncertainty_grows_with_age_and_resets_on_resight() {
    let mut m = DenseGrid::new(ARENA_SIDE);
    let me = Vec2::new(500.0, 500.0);
    let enemy = view(9, Kind::Tank, Some(TeamId::B), Vec2::new(560.0, 500.0));
    m.ingest_visible(&observation(0, me, vec![enemy.clone()]));
    assert_eq!(m.tracks_near(me, 100.0)[0].uncertainty, 0.0);

    m.tick(Tick(50));
    let t = &m.tracks_near(me, 100.0)[0];
    let expected = TANK_MAX_SPEED * 50.0 * DT;
    assert!(
        (t.uncertainty - expected).abs() < 1e-3,
        "{} vs {expected}",
        t.uncertainty
    );
    assert_eq!(t.kind, Some(Kind::Tank));
    assert_eq!(t.hp, Some(30.0));

    m.ingest_visible(&observation(50, Vec2::new(300.0, 300.0), vec![enemy]));
    assert_eq!(
        m.tracks_near(Vec2::new(560.0, 500.0), 1.0)[0].uncertainty,
        0.0
    );
}

#[test]
fn a_track_inside_the_sensed_disc_that_is_not_seen_is_forgotten() {
    let mut m = DenseGrid::new(ARENA_SIDE);
    let me = Vec2::new(500.0, 500.0);
    let shape = view(4, Kind::Shape, None, Vec2::new(530.0, 500.0));
    m.ingest_visible(&observation(0, me, vec![shape]));
    assert_eq!(m.tracks_near(me, 100.0).len(), 1);

    // Look again from the same place: it is gone, and so is the track.
    m.ingest_visible(&observation(5, me, Vec::new()));
    assert!(m.tracks_near(me, 100.0).is_empty());
}

#[test]
fn a_track_that_decays_past_use_is_forgotten() {
    let mut m = DenseGrid::new(ARENA_SIDE);
    let me = Vec2::new(500.0, 500.0);
    m.ingest_visible(&observation(
        0,
        me,
        vec![view(
            9,
            Kind::Tank,
            Some(TeamId::B),
            Vec2::new(560.0, 500.0),
        )],
    ));
    // 60 units per second: the disc spans the arena in about 17 seconds.
    m.tick(Tick(25 * 20));
    assert!(m.tracks_near(me, ARENA_SIDE).is_empty());
}

#[test]
fn the_cap_forgets_the_stalest_first() {
    let mut m = DenseGrid::with_resolution(ARENA_SIDE, 32, 3);
    let me = Vec2::new(500.0, 500.0);
    for (i, t) in [10u32, 20, 30].iter().enumerate() {
        m.ingest_visible(&observation(
            *t,
            me,
            vec![view(
                10 + i as u32,
                Kind::Shape,
                None,
                Vec2::new(520.0 + i as f32, 500.0),
            )],
        ));
    }
    // The re-look at tick 20 and 30 would forget the earlier shapes as unseen
    // inside the disc, so re-observe all three at once, then add a fourth.
    let all: Vec<EntityView> = (0..3)
        .map(|i| {
            view(
                10 + i,
                Kind::Shape,
                None,
                Vec2::new(520.0 + i as f32, 500.0),
            )
        })
        .collect();
    m.ingest_visible(&observation(40, me, all));
    assert_eq!(m.tracks().len(), 3);

    m.ingest_belief(
        &BeliefMsg::TrackSet {
            tracks: vec![Track {
                id: Some(EntityId {
                    index: 99,
                    generation: 0,
                }),
                team: None,
                last_pos: Vec2::new(100.0, 100.0),
                vel_estimate: Vec2::ZERO,
                last_seen: Tick(41),
                uncertainty: 0.0,
                kind: Some(Kind::Shape),
                tier: None,
                hp: None,
            }],
        },
        AgentId(1),
    );
    assert_eq!(m.tracks().len(), 3, "capped");
    assert!(m
        .tracks()
        .get(EntityId {
            index: 99,
            generation: 0
        })
        .is_some());
}

#[test]
fn hearsay_is_taken_only_when_fresher() {
    let mut m = DenseGrid::new(ARENA_SIDE);
    let me = Vec2::new(500.0, 500.0);
    let id = EntityId {
        index: 9,
        generation: 0,
    };
    m.ingest_visible(&observation(
        100,
        me,
        vec![view(
            9,
            Kind::Tank,
            Some(TeamId::B),
            Vec2::new(560.0, 500.0),
        )],
    ));
    let rumour = |seen: u32, pos: Vec2| BeliefMsg::TrackSet {
        tracks: vec![Track {
            id: Some(id),
            team: Some(TeamId::B),
            last_pos: pos,
            vel_estimate: Vec2::ZERO,
            last_seen: Tick(seen),
            uncertainty: 0.0,
            kind: Some(Kind::Tank),
            tier: None,
            hp: None,
        }],
    };
    // Older than what is held: ignored.
    m.ingest_belief(&rumour(50, Vec2::new(100.0, 100.0)), AgentId(1));
    assert_eq!(m.tracks().get(id).unwrap().pos, Vec2::new(560.0, 500.0));
    // Fresher: taken.
    m.ingest_belief(&rumour(120, Vec2::new(700.0, 700.0)), AgentId(1));
    assert_eq!(m.tracks().get(id).unwrap().pos, Vec2::new(700.0, 700.0));
    // The same rumour again adds nothing and changes nothing.
    m.ingest_belief(&rumour(120, Vec2::new(700.0, 700.0)), AgentId(2));
    assert_eq!(m.tracks().len(), 1);
}

#[test]
fn a_map_patch_fills_unknown_cells_and_respects_freshness() {
    let mut m = DenseGrid::new(ARENA_SIDE);
    let far = Vec2::new(900.0, 900.0);
    let patch = |at: u32, state: CellState| BeliefMsg::MapPatch {
        cells: vec![schema::MapCell {
            region: Region::Disc {
                center: far,
                radius: 1.0,
            },
            state,
            updated: Tick(at),
            confidence: 1.0,
        }],
    };
    m.ingest_belief(&patch(10, CellState::Occupied), AgentId(1));
    assert_eq!(m.occupancy(far), CellState::Occupied);
    m.ingest_belief(&patch(5, CellState::Free), AgentId(1));
    assert_eq!(m.occupancy(far), CellState::Occupied, "older patch ignored");
    m.ingest_belief(&patch(20, CellState::Free), AgentId(1));
    assert_eq!(m.occupancy(far), CellState::Free);
}

#[test]
fn encode_fits_the_budget_and_prefers_the_fresh() {
    let mut m = DenseGrid::new(ARENA_SIDE);
    let me = Vec2::new(500.0, 500.0);
    let mut seen = Vec::new();
    for i in 0..20u32 {
        seen.push(view(
            100 + i,
            Kind::Tank,
            Some(TeamId::B),
            Vec2::new(420.0 + i as f32 * 8.0, 500.0),
        ));
    }
    m.ingest_visible(&observation(0, me, seen.clone()));
    // Re-sight the last one later, so it is the freshest. From far enough away
    // that the sensed disc does not cover the other nineteen: that would be
    // negative evidence, and the model would rightly forget them.
    m.ingest_visible(&observation(
        30,
        Vec2::new(572.0, 700.0),
        vec![seen[19].clone()],
    ));

    for budget in [64usize, 300, 2000, 100_000] {
        let msg = m.encode(budget, Interest::Threats);
        let bytes = schema::to_line(&msg).unwrap().len();
        let BeliefMsg::TrackSet { tracks } = &msg else {
            panic!("threats are a track set");
        };
        assert!(bytes <= budget || tracks.is_empty(), "{bytes} > {budget}");
        if let Some(first) = tracks.first() {
            assert_eq!(first.last_seen, Tick(30), "freshest first");
        }
    }
    let BeliefMsg::TrackSet { tracks } = m.encode(100_000, Interest::Threats) else {
        panic!()
    };
    assert_eq!(tracks.len(), 20);

    let BeliefMsg::MapPatch { cells } = m.encode(
        100_000,
        Interest::Near {
            pos: me,
            radius: 60.0,
        },
    ) else {
        panic!("near is a map patch")
    };
    assert!(!cells.is_empty());
    for c in &cells {
        assert!(centre(&c.region).distance(me) <= 60.0);
    }

    let BeliefMsg::MapPatch { cells } = m.encode(500, Interest::Frontiers) else {
        panic!()
    };
    assert!(!cells.is_empty());
    assert!(cells.iter().all(|c| c.state == CellState::Unknown));
}

#[test]
fn a_scan_frees_the_ray_and_occupies_its_end() {
    let mut m = DenseGrid::new(ARENA_SIDE);
    let pose = Pose {
        pos: Vec2::new(500.0, 500.0),
        heading: 0.0,
        tick: Tick(3),
    };
    let scan = Scan {
        pose_start: pose,
        pose_end: pose,
        angle_start: 0.0,
        sweep: std::f32::consts::TAU,
        max_range: 200.0,
        ranges: vec![Some(100.0), None, None, None],
        intensities: None,
    };
    m.ingest_scan(&scan);
    assert_eq!(m.occupancy(Vec2::new(550.0, 500.0)), CellState::Free);
    assert_eq!(m.occupancy(Vec2::new(600.0, 500.0)), CellState::Occupied);
    assert_eq!(
        m.occupancy(Vec2::new(500.0, 400.0)),
        CellState::Unknown,
        "a dropout says nothing"
    );
}

#[test]
fn footprint_is_fixed_and_under_the_ceiling() {
    let mut m = DenseGrid::new(ARENA_SIDE);
    let before = m.footprint();
    assert!(before < MEMORY_CEILING_BYTES, "{before} bytes");
    for t in 0..40u32 {
        let seen: Vec<EntityView> = (0..30)
            .map(|i| {
                view(
                    1000 + t * 30 + i,
                    Kind::Shape,
                    None,
                    Vec2::new(420.0 + i as f32 * 5.0, 480.0 + t as f32),
                )
            })
            .collect();
        m.ingest_visible(&observation(
            t * 5,
            Vec2::new(500.0, 480.0 + t as f32),
            seen,
        ));
        m.tick(Tick(t * 5));
    }
    assert_eq!(m.footprint(), before, "capacity is allocated once");
    assert!(m.tracks().len() <= 128);
}

#[test]
fn one_history_two_models_one_message() {
    let build = || {
        let mut m = DenseGrid::new(ARENA_SIDE);
        for t in 0..10u32 {
            let at = Vec2::new(300.0 + t as f32 * 20.0, 400.0);
            let seen = vec![
                view(50 + t, Kind::Shape, None, Vec2::new(at.x + 30.0, at.y)),
                view(9, Kind::Tank, Some(TeamId::B), Vec2::new(at.x, at.y + 40.0)),
            ];
            m.ingest_visible(&observation(t * 5, at, seen));
            m.tick(Tick(t * 5));
        }
        m
    };
    let a = build();
    let b = build();
    for interest in [Interest::Any, Interest::Threats, Interest::Frontiers] {
        assert_eq!(
            schema::to_line(&a.encode(4000, interest)).unwrap(),
            schema::to_line(&b.encode(4000, interest)).unwrap()
        );
    }
    assert_eq!(a.footprint(), b.footprint());
}

//! Wire contract tests.
//!
//! Every type that crosses the wire must survive an encode and decode unchanged.
//! The serialized shapes are asserted literally where a downstream consumer depends
//! on them: the database reads `kind` and `payload` as columns, and the viewer
//! reads the frame tag `t`.

use schema::*;

fn sample_entity() -> EntityView {
    EntityView {
        id: EntityId::new(7, 3),
        position: PositionGroup {
            pos: Vec2::new(120.5, 44.25),
            heading: 1.5,
        },
        physics: PhysicsGroup {
            vel: Vec2::new(-1.0, 0.5),
            radius: 12.0,
        },
        style: StyleGroup {
            kind: Kind::Tank,
            tier: None,
            class: Some(Class::Scout),
            stats: Some(Stats::default()),
            focus: None,
        },
        team: TeamGroup {
            team: Some(TeamId::A),
            agent: Some(AgentId(2)),
            owner: None,
        },
        health: Some(HealthGroup {
            hp: 40.0,
            max_hp: 50.0,
            last_damaged: Some(Tick(88)),
        }),
        score: Some(ScoreGroup {
            score: 1200,
            level: 9,
        }),
    }
}

#[test]
fn entity_id_packs_and_unpacks() {
    let id = EntityId::new(0xDEAD_BEEF, 0x0000_002A);
    assert_eq!(EntityId::from_u64(id.to_u64()), id);
    // A recycled slot must not compare equal to the handle it replaced.
    assert_ne!(EntityId::new(4, 1), EntityId::new(4, 2));
}

#[test]
fn entity_id_wire_form_is_a_pair() {
    let json = serde_json::to_string(&EntityId::new(7, 3)).unwrap();
    assert_eq!(json, "[7,3]");
    let back: EntityId = serde_json::from_str(&json).unwrap();
    assert_eq!(back, EntityId::new(7, 3));
}

#[test]
fn newtype_ids_are_bare_numbers() {
    assert_eq!(serde_json::to_string(&Tick(42)).unwrap(), "42");
    assert_eq!(serde_json::to_string(&TeamId::B).unwrap(), "1");
    assert_eq!(serde_json::to_string(&AgentId(5)).unwrap(), "5");
    assert_eq!(serde_json::to_string(&FocusId(1)).unwrap(), "1");
}

#[test]
fn event_serializes_as_kind_and_payload() {
    let ev = Event::Scored {
        team: TeamId::A,
        delta: 10,
        total: 310,
        reason: ScoreReason::ShapeDestroyed,
    };
    let value: serde_json::Value = serde_json::to_value(&ev).unwrap();
    assert_eq!(value["kind"], "scored");
    assert_eq!(value["payload"]["total"], 310);
    // The database column and the serde tag must not drift apart.
    assert_eq!(value["kind"].as_str().unwrap(), ev.kind_str());
    let back: Event = serde_json::from_value(value).unwrap();
    assert_eq!(back, ev);
}

#[test]
fn every_event_kind_string_matches_its_tag() {
    let events = vec![
        Event::MatchStart {
            seed: 1,
            config_hash: 2,
            match_id: "m".into(),
        },
        Event::TickBegin { tick: Tick(1) },
        Event::Spawned {
            id: EntityId::new(1, 1),
            kind: Kind::Shape,
            pos: Vec2::ZERO,
            team: None,
            focus: Some(FocusId(0)),
        },
        Event::Despawned {
            id: EntityId::new(1, 1),
            cause: Cause::Killed,
        },
        Event::Damaged {
            target: EntityId::new(1, 1),
            source: EntityId::new(2, 1),
            amount: 5.0,
            remaining: 10.0,
        },
        Event::Killed {
            target: EntityId::new(1, 1),
            killer: EntityId::new(2, 1),
        },
        Event::Scored {
            team: TeamId::A,
            delta: 1,
            total: 1,
            reason: ScoreReason::TankKilled,
        },
        Event::CommandIssued {
            team: TeamId::A,
            origin: Origin::Human,
            cmd: Command::RequestReport { agent: AgentId(1) },
        },
        Event::ActionSubmitted {
            agent: AgentId(1),
            action: Action::idle(),
            latency_us: 40,
        },
        Event::MatchEnd {
            winner: Some(TeamId::B),
            tick: Tick(900),
        },
        Event::MessageSent {
            from: AgentId(1),
            to: Recipient::Broadcast,
            bytes: 64,
            tick: Tick(3),
        },
        Event::MessageDelivered {
            from: AgentId(1),
            to: AgentId(2),
            bytes: 64,
            latency_ticks: 2,
        },
        Event::MessageDropped {
            from: AgentId(1),
            to: Recipient::Team { team: TeamId::A },
            reason: DropReason::Bandwidth,
        },
        Event::BudgetExceeded {
            agent: AgentId(1),
            kind: BudgetKind::Memory,
            overage: 512,
        },
    ];
    for ev in &events {
        let value: serde_json::Value = serde_json::to_value(ev).unwrap();
        assert_eq!(value["kind"].as_str().unwrap(), ev.kind_str(), "{ev:?}");
        let back: Event = serde_json::from_value(value).unwrap();
        assert_eq!(&back, ev);
    }
}

#[test]
fn reserved_variants_are_flagged() {
    assert!(Event::MessageSent {
        from: AgentId(0),
        to: Recipient::Broadcast,
        bytes: 1,
        tick: Tick(0)
    }
    .is_reserved());
    assert!(!Event::TickBegin { tick: Tick(0) }.is_reserved());
}

#[test]
fn snapshot_frame_roundtrips() {
    let frame = ServerMsg::Snapshot {
        tick: Tick(120),
        entities: vec![sample_entity()],
        scores: vec![300, 250],
        events: vec![Event::TickBegin { tick: Tick(120) }],
        beliefs: None,
    };
    let line = to_line(&frame).unwrap();
    assert!(line.ends_with('\n'));
    assert!(line.contains("\"t\":\"snapshot\""));
    let back: ServerMsg = from_line(&line).unwrap();
    assert_eq!(back, frame);
}

#[test]
fn delta_omits_unchanged_groups() {
    let mut d = EntityDelta::new(EntityId::new(4, 1));
    assert!(d.is_empty());
    d.position = Some(PositionGroup {
        pos: Vec2::new(1.0, 2.0),
        heading: 0.0,
    });
    assert!(!d.is_empty());
    let json = serde_json::to_string(&d).unwrap();
    assert!(json.contains("position"));
    assert!(
        !json.contains("physics"),
        "unchanged groups must not be sent"
    );
    let back: EntityDelta = serde_json::from_str(&json).unwrap();
    assert_eq!(back, d);
}

#[test]
fn client_can_only_command() {
    let msg = ClientMsg::Command {
        team: TeamId::A,
        cmd: Command::AssignRole {
            agent: AgentId(3),
            role: Role::Relay,
        },
    };
    let back: ClientMsg = from_line(&to_line(&msg).unwrap()).unwrap();
    assert_eq!(back, msg);
}

#[test]
fn belief_message_roundtrips_including_raw() {
    let msgs = vec![
        BeliefMsg::MapPatch {
            cells: vec![MapCell {
                region: Region::Rect {
                    min: Vec2::ZERO,
                    max: Vec2::new(250.0, 250.0),
                },
                state: CellState::Free,
                updated: Tick(10),
                confidence: 0.8,
            }],
        },
        BeliefMsg::TrackSet {
            tracks: vec![Track {
                id: Some(EntityId::new(9, 1)),
                team: Some(TeamId::B),
                last_pos: Vec2::new(500.0, 500.0),
                vel_estimate: Vec2::new(1.0, 0.0),
                last_seen: Tick(80),
                uncertainty: 200.0,
                kind: Some(Kind::Tank),
                tier: None,
                hp: Some(12.5),
            }],
        },
        BeliefMsg::Intent {
            intent: Intent::Relaying {
                between: [AgentId(1), AgentId(4)],
            },
        },
        BeliefMsg::Request {
            interest: Interest::Near {
                pos: Vec2::new(500.0, 500.0),
                radius: 150.0,
            },
        },
        BeliefMsg::Raw {
            codec: "quadtree/v1".into(),
            bytes: vec![0, 1, 2, 250, 255],
        },
    ];
    for m in &msgs {
        let back: BeliefMsg = from_line(&to_line(m).unwrap()).unwrap();
        assert_eq!(&back, m);
    }
    // Raw payloads travel as base64, not as an array of integers.
    let json = serde_json::to_string(&msgs[4]).unwrap();
    assert!(json.contains("\"bytes\":\"AAEC+v8=\""), "{json}");
}

#[test]
fn observation_and_action_roundtrip() {
    let obs = Observation {
        tick: Tick(5),
        own: SelfView {
            agent: AgentId(1),
            team: TeamId::A,
            entity: Some(EntityId::new(1, 1)),
            pos: Vec2::new(100.0, 100.0),
            vel: Vec2::ZERO,
            heading: 0.0,
            hp: 50.0,
            max_hp: 50.0,
            score: 0,
            level: 1,
            class: None,
            stats: Stats::default(),
            points: 0,
            sense_radius: 120.0,
            comms_radius: 200.0,
            reload_ready: true,
            respawn_at: None,
        },
        visible: vec![sample_entity()],
        scan: None,
        inbox: vec![],
        commands: vec![],
        scores: vec![0, 0],
    };
    let back: Observation = from_line(&to_line(&obs).unwrap()).unwrap();
    assert_eq!(back, obs);

    let action = Action {
        control: Control {
            thrust: Vec2::new(0.0, 1.0),
            aim: 1.57,
            fire: true,
        },
        send: vec![Outbound {
            to: Recipient::Team { team: TeamId::A },
            payload: BeliefMsg::Intent {
                intent: Intent::Farming {
                    region: Region::Disc {
                        center: Vec2::new(500.0, 500.0),
                        radius: 150.0,
                    },
                },
            },
        }],
        commands: vec![],
        choices: vec![Choice::Stat {
            stat: StatKind::Speed,
        }],
    };
    let back: Action = from_line(&to_line(&action).unwrap()).unwrap();
    assert_eq!(back, action);

    // An idle action must stay small on the wire; it is sent on every missed budget.
    assert_eq!(
        serde_json::to_string(&Action::idle()).unwrap(),
        r#"{"control":{"thrust":{"x":0.0,"y":0.0},"aim":0.0,"fire":false}}"#
    );
}

#[test]
fn base64_roundtrips_every_byte_length() {
    for n in 0..64usize {
        let bytes: Vec<u8> = (0..n).map(|i| (i * 7 % 256) as u8).collect();
        let text = schema::b64::encode(&bytes);
        assert_eq!(schema::b64::decode(&text).unwrap(), bytes, "length {n}");
    }
}

#[test]
fn region_containment() {
    let nest = Region::Disc {
        center: Vec2::new(500.0, 500.0),
        radius: 150.0,
    };
    assert!(nest.contains(Vec2::new(500.0, 620.0), 1000.0));
    assert!(!nest.contains(Vec2::new(500.0, 700.0), 1000.0));
    assert!(Region::WholeArena.contains(Vec2::new(0.0, 999.0), 1000.0));
    assert!(!Region::WholeArena.contains(Vec2::new(-1.0, 5.0), 1000.0));
}

#[test]
fn large_seeds_survive_the_javascript_boundary() {
    let ev = Event::MatchStart {
        seed: u64::MAX,
        config_hash: 9_007_199_254_740_993, // 2^53 + 1, unrepresentable as f64
        match_id: "m1".into(),
    };
    let line = to_line(&ev).unwrap();
    assert!(line.contains(r#""seed":"18446744073709551615""#), "{line}");
    let back: Event = from_line(&line).unwrap();
    assert_eq!(back, ev);
}

#[test]
fn tick_age_saturates() {
    assert_eq!(Tick(10).age_since(Tick(4)), 6);
    assert_eq!(Tick(4).age_since(Tick(10)), 0);
}

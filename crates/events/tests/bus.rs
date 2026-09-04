//! The bus, the replay round trip, and the event stream's contract with the
//! database. None of these start a simulation.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use events::{Bus, ReplayReader, ReplaySink, Sink, TickRecord};
use schema::{
    Action, AgentId, ArenaInfo, BaseInfo, Cause, Control, EntityId, Event, Inputs, Kind, MatchInfo,
    Region, TeamId, TeamInfo, Tick, Vec2, PROTOCOL_VERSION,
};

fn arena() -> ArenaInfo {
    ArenaInfo {
        side: 1000.0,
        bases: vec![BaseInfo {
            team: TeamId::A,
            region: Region::Rect {
                min: Vec2::ZERO,
                max: Vec2::new(200.0, 200.0),
            },
            cc_pos: Vec2::new(100.0, 100.0),
        }],
        nest: Region::Disc {
            center: Vec2::new(500.0, 500.0),
            radius: 150.0,
        },
        wings: Vec::new(),
        tick_hz: 25,
    }
}

fn match_info() -> MatchInfo {
    MatchInfo {
        match_id: "test".into(),
        seed: 42,
        point_target: 1000,
        teams: vec![TeamInfo {
            team: TeamId::A,
            name: "A".into(),
            agents: vec![AgentId(0)],
            cc: AgentId(1),
        }],
    }
}

fn drive(x: f32) -> Inputs {
    Inputs {
        actions: vec![(
            AgentId(0),
            Action {
                control: Control {
                    thrust: Vec2::new(x, 0.0),
                    aim: x,
                    fire: x > 0.5,
                },
                ..Action::default()
            },
        )],
        commands: Vec::new(),
    }
}

#[derive(Default)]
struct Counter {
    seen: Arc<AtomicUsize>,
    flushes: Arc<AtomicUsize>,
}

impl Sink for Counter {
    fn accept(&mut self, _rec: &TickRecord<'_>) {
        self.seen.fetch_add(1, Ordering::SeqCst);
    }
    fn flush(&mut self) {
        self.flushes.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn one_record_reaches_every_sink() {
    let a = Arc::new(AtomicUsize::new(0));
    let b = Arc::new(AtomicUsize::new(0));
    let flushes = Arc::new(AtomicUsize::new(0));

    let mut bus = Bus::new()
        .with(Box::new(Counter {
            seen: a.clone(),
            flushes: flushes.clone(),
        }))
        .with(Box::new(Counter {
            seen: b.clone(),
            flushes: flushes.clone(),
        }));
    assert_eq!(bus.len(), 2);

    let inputs = drive(1.0);
    for t in 0..5 {
        bus.publish(&TickRecord {
            tick: Tick(t),
            events: &[],
            inputs: &inputs,
            scores: [0, 0],
        });
    }
    assert_eq!(a.load(Ordering::SeqCst), 5);
    assert_eq!(b.load(Ordering::SeqCst), 5);

    // Flush is safe to call more than once.
    bus.flush();
    bus.flush();
    assert_eq!(flushes.load(Ordering::SeqCst), 4);
}

#[test]
fn a_replay_log_round_trips() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("m.replay.gz");

    // A run of identical ticks, a change, then another run. The repeats are what
    // the log collapses, and expanding them again must restore every tick.
    let mut written: Vec<Inputs> = Vec::new();
    written.extend(std::iter::repeat_n(drive(0.0), 40));
    written.extend(std::iter::repeat_n(drive(1.0), 25));
    written.push(drive(0.5));
    written.extend(std::iter::repeat_n(drive(0.0), 10));

    {
        let mut sink = ReplaySink::create(&path, arena(), match_info()).expect("create");
        for (t, inputs) in written.iter().enumerate() {
            sink.accept(&TickRecord {
                tick: Tick(t as u32),
                events: &[],
                inputs,
                scores: [0, 0],
            });
        }
        sink.flush();
        let (ticks, lines) = sink.counts();
        assert_eq!(ticks, written.len() as u64);
        assert!(
            lines < 12,
            "76 ticks of four distinct actions should collapse to a handful of \
             lines, wrote {lines}"
        );
    }

    let (header, reader) = ReplayReader::open(&path).expect("open");
    assert_eq!(header.protocol, PROTOCOL_VERSION);
    assert_eq!(header.match_info, match_info());
    assert_eq!(header.arena, arena());

    let read = reader.read_all().expect("read");
    assert_eq!(read.len(), written.len(), "every tick must come back");
    for (i, (tick, inputs)) in read.iter().enumerate() {
        assert_eq!(tick.0, i as u32, "ticks must come back in order");
        assert_eq!(inputs, &written[i]);
    }
}

/// `kind` is the only join key the `events` table has. If `kind_str` and the serde
/// tag ever disagree, every query written against the database is silently wrong.
#[test]
fn kind_str_matches_the_serde_tag() {
    let samples = [
        Event::MatchStart {
            seed: 1,
            config_hash: 2,
            match_id: "m".into(),
        },
        Event::TickBegin { tick: Tick(0) },
        Event::Spawned {
            id: EntityId::new(1, 1),
            kind: Kind::Shape,
            pos: Vec2::ZERO,
            team: None,
            focus: None,
        },
        Event::Despawned {
            id: EntityId::new(1, 1),
            cause: Cause::Killed,
        },
        Event::Damaged {
            target: EntityId::new(1, 1),
            source: EntityId::new(2, 1),
            amount: 1.0,
            remaining: 2.0,
        },
        Event::Killed {
            target: EntityId::new(1, 1),
            killer: EntityId::new(2, 1),
        },
        Event::MatchEnd {
            winner: Some(TeamId::A),
            tick: Tick(9),
        },
    ];

    for event in &samples {
        let value = serde_json::to_value(event).expect("serialize");
        let tag = value.get("kind").and_then(|k| k.as_str()).expect("tagged");
        assert_eq!(
            tag,
            event.kind_str(),
            "the database column and the wire disagree about {event:?}"
        );
    }
}

#[test]
fn v0_events_are_never_reserved() {
    let live = Event::Killed {
        target: EntityId::new(1, 1),
        killer: EntityId::new(2, 1),
    };
    assert!(!live.is_reserved());
    assert!(Event::BudgetExceeded {
        agent: AgentId(0),
        kind: schema::BudgetKind::Time,
        overage: 1,
    }
    .is_reserved());
}

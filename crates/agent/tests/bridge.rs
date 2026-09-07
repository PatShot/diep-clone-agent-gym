//! The socket bridge, against a client thread: replies come back, late replies
//! are idles and do not knock the exchange out of step, garbage is an idle, a
//! hangup is idle forever, and a baseline driven over the socket is the baseline.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use agent::{Link, NearestShape, Policy, Runner, RunnerConfig, SocketPolicy};
use events::{Bus, Sink, TickRecord};
use schema::{
    Action, AgentId, Control, EntityId, Event, Observation, SelfView, TeamId, Tick, Vec2,
};
use sim_core::constants::ARENA_SIDE;
use sim_core::WorldSpec;
use spawn::SpawnConfig;

fn observation(tick: u32) -> Observation {
    Observation {
        tick: Tick(tick),
        own: SelfView {
            agent: AgentId(1),
            team: TeamId::A,
            entity: Some(EntityId {
                index: 1,
                generation: 0,
            }),
            pos: Vec2::new(100.0, 100.0),
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
        },
        visible: Vec::new(),
        scan: None,
        inbox: Vec::new(),
        commands: Vec::new(),
        scores: vec![0, 0],
    }
}

fn fixed(x: f32) -> Action {
    Action {
        control: Control {
            thrust: Vec2::new(x, 0.0),
            aim: 1.0,
            fire: true,
        },
        ..Action::default()
    }
}

/// A listener on a free port, and a client thread connected to it that runs
/// `serve` over the connection.
fn pair<F>(serve: F) -> (Link, thread::JoinHandle<()>)
where
    F: FnOnce(BufReader<TcpStream>, TcpStream) + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let client = thread::spawn(move || {
        let stream = TcpStream::connect(addr).unwrap();
        let writer = stream.try_clone().unwrap();
        serve(BufReader::new(stream), writer);
    });
    let link = Link::accept(&listener, Duration::from_millis(300)).unwrap();
    (link, client)
}

#[test]
fn a_reply_comes_back_as_the_action() {
    let (link, client) = pair(|reader, mut writer| {
        for line in reader.lines().map_while(Result::ok) {
            let obs: Observation = schema::from_line(&line).expect("an observation");
            let reply = fixed(obs.tick.0 as f32);
            writer
                .write_all(schema::to_line(&reply).unwrap().as_bytes())
                .unwrap();
        }
    });
    let mut p = SocketPolicy::new(Arc::new(Mutex::new(link)));
    for t in 0..5u32 {
        assert_eq!(p.decide(&observation(t), Tick(t)), fixed(t as f32));
    }
    let s = p.stats();
    assert_eq!(s.decided, 5);
    assert_eq!((s.timeouts, s.malformed, s.dead), (0, 0, false));
    drop(p);
    client.join().unwrap();
}

#[test]
fn a_late_reply_is_an_idle_and_does_not_shift_the_exchange() {
    let (link, client) = pair(|reader, mut writer| {
        let mut lines = reader.lines().map_while(Result::ok);
        // First observation: sit on it past the timeout, then answer.
        let first = lines.next().unwrap();
        let first: Observation = schema::from_line(&first).unwrap();
        thread::sleep(Duration::from_millis(600));
        writer
            .write_all(
                schema::to_line(&fixed(first.tick.0 as f32))
                    .unwrap()
                    .as_bytes(),
            )
            .unwrap();
        // Every later one: answer at once, with its own tick.
        for line in lines {
            let obs: Observation = schema::from_line(&line).unwrap();
            writer
                .write_all(
                    schema::to_line(&fixed(obs.tick.0 as f32))
                        .unwrap()
                        .as_bytes(),
                )
                .unwrap();
        }
    });
    let mut p = SocketPolicy::new(Arc::new(Mutex::new(link)));

    let started = Instant::now();
    assert_eq!(p.decide(&observation(0), Tick(0)), Action::idle());
    assert!(
        started.elapsed() < Duration::from_millis(550),
        "gave up at the timeout"
    );
    assert_eq!(p.stats().timeouts, 1);

    // The late reply to tick 0 is owed; the answer to tick 5 must be tick 5's.
    thread::sleep(Duration::from_millis(400));
    assert_eq!(p.decide(&observation(5), Tick(5)), fixed(5.0));
    assert_eq!(p.decide(&observation(10), Tick(10)), fixed(10.0));
    assert_eq!(p.stats().decided, 2);
    drop(p);
    client.join().unwrap();
}

#[test]
fn garbage_is_an_idle_and_a_hangup_is_idle_forever() {
    let (link, client) = pair(|reader, mut writer| {
        let mut lines = reader.lines().map_while(Result::ok);
        lines.next();
        writer.write_all(b"this is not an action\n").unwrap();
        lines.next();
        writer
            .write_all(schema::to_line(&fixed(2.0)).unwrap().as_bytes())
            .unwrap();
        // Then hang up.
    });
    let mut p = SocketPolicy::new(Arc::new(Mutex::new(link)));
    assert_eq!(p.decide(&observation(0), Tick(0)), Action::idle());
    assert_eq!(p.stats().malformed, 1);
    assert_eq!(p.decide(&observation(1), Tick(1)), fixed(2.0));
    client.join().unwrap();

    let started = Instant::now();
    assert_eq!(p.decide(&observation(2), Tick(2)), Action::idle());
    assert!(p.stats().dead);
    for t in 3..10u32 {
        assert_eq!(p.decide(&observation(t), Tick(t)), Action::idle());
    }
    assert!(
        started.elapsed() < Duration::from_millis(400),
        "dead links do not wait"
    );
}

struct Collect(Arc<Mutex<Vec<Event>>>);

impl Sink for Collect {
    fn accept(&mut self, rec: &TickRecord<'_>) {
        self.0.lock().unwrap().extend(rec.events.iter().cloned());
    }
    fn flush(&mut self) {}
}

fn run(seed: u64, bridged: Option<Arc<Mutex<Link>>>) -> String {
    let log = Arc::new(Mutex::new(Vec::new()));
    let bus = Bus::new().with(Box::new(Collect(log.clone())));
    let mut r = Runner::new(
        WorldSpec {
            seed,
            ..WorldSpec::default()
        },
        SpawnConfig::default(),
        bus,
        RunnerConfig::default(),
    );
    let seats: Vec<_> = r.seats().collect();
    for (agent, team, is_cc) in seats {
        if is_cc {
            continue;
        }
        let p: Box<dyn Policy> = match (&bridged, team) {
            (Some(link), TeamId::A) => Box::new(SocketPolicy::new(link.clone())),
            _ => Box::new(NearestShape::new(ARENA_SIDE)),
        };
        r.set_policy(agent, p);
    }
    r.run(400);
    let events = log.lock().unwrap().clone();
    schema::to_line(&events).unwrap()
}

#[test]
fn a_baseline_over_the_socket_is_the_baseline() {
    let (link, client) = pair(|reader, mut writer| {
        let mut policy = NearestShape::new(ARENA_SIDE);
        for line in reader.lines().map_while(Result::ok) {
            let obs: Observation = schema::from_line(&line).unwrap();
            let action = policy.decide(&obs, obs.tick);
            writer
                .write_all(schema::to_line(&action).unwrap().as_bytes())
                .unwrap();
        }
    });
    let link = Arc::new(Mutex::new(link));
    let bridged = run(5, Some(link.clone()));
    let stats = link.lock().unwrap().stats();
    assert_eq!((stats.timeouts, stats.malformed, stats.dead), (0, 0, false));
    assert_eq!(stats.decided, 5 * 80, "five tanks, eighty decisions each");
    drop(link);
    client.join().unwrap();

    let local = run(5, None);
    assert_eq!(bridged, local, "the socket changes nothing");
}

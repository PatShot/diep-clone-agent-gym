//! The socket bridge: a `Policy` on the other end of a socket.
//!
//! This is what buys the flexibility. A policy can be written in Python, which is
//! where anyone doing learned control will want to be, and the simulation never
//! knows. It sees a [`Policy`] like any other.
//!
//! # Protocol
//!
//! Newline-delimited JSON. The simulation writes one `Observation` per decision
//! and reads one `Action` back. That is the whole of it. The observation carries
//! its tick and the agent it is for, so one connection serves every tank on a team
//! in turn — lockstep asks them one at a time anyway — and there is no envelope
//! and no handshake: the first observation is the hello. The types are exactly
//! `schema::Observation` and `schema::Action`, so the JSON a client sees is the
//! JSON `client/src/gen/schema.ts` describes.
//!
//! # Lockstep with a process that may be late
//!
//! The read has a timeout. A reply that does not arrive in time becomes
//! `Action::idle()`, is counted, and is *owed*: the client will still send it,
//! and it must not be mistaken for the answer to the next observation. So before
//! the next observation goes out, the owed replies are read and discarded. This
//! keeps the exchange in step as long as the client answers every observation
//! once, in order, which is the only thing asked of it.
//!
//! A connection that closes or errors is dead, and every decision after that is
//! idle without waiting. The match goes on; the replay records the idles.
//!
//! Whatever the process does, the replay is exact: it records what came back.
//! Determinism of the *live* run is the client's business, not the bridge's.

use std::io::{self, BufRead, BufReader, ErrorKind, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use schema::{Action, Observation, Tick};

use crate::Policy;

/// What a link has been through.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkStats {
    /// Replies that arrived in time and parsed.
    pub decided: u64,
    /// Replies that did not arrive in time.
    pub timeouts: u32,
    /// Replies that arrived and did not parse as an `Action`.
    pub malformed: u32,
    /// Whether the connection has closed or failed.
    pub dead: bool,
}

/// One connection to a policy process, shared by the seats it serves.
pub struct Link {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    line: String,
    /// Replies owed from earlier timeouts, to be read and discarded.
    owed: u32,
    stats: LinkStats,
}

impl Link {
    /// Bind `addr` and wait for one policy process to connect.
    pub fn listen(addr: impl ToSocketAddrs, timeout: Duration) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        Self::accept(&listener, timeout)
    }

    /// Wait for one policy process to connect to an already-bound listener.
    pub fn accept(listener: &TcpListener, timeout: Duration) -> io::Result<Self> {
        let (stream, _) = listener.accept()?;
        Self::from_stream(stream, timeout)
    }

    /// Connect to a policy process that is already listening.
    pub fn connect(addr: impl ToSocketAddrs, timeout: Duration) -> io::Result<Self> {
        Self::from_stream(TcpStream::connect(addr)?, timeout)
    }

    pub fn from_stream(stream: TcpStream, timeout: Duration) -> io::Result<Self> {
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(timeout.max(Duration::from_millis(1))))?;
        let writer = stream.try_clone()?;
        Ok(Self {
            reader: BufReader::new(stream),
            writer,
            line: String::new(),
            owed: 0,
            stats: LinkStats::default(),
        })
    }

    pub fn stats(&self) -> LinkStats {
        self.stats
    }

    /// Send an observation and wait for the action. Idle on timeout, on a reply
    /// that does not parse, and forever after the connection dies.
    pub fn exchange(&mut self, obs: &Observation) -> Action {
        if self.stats.dead {
            return Action::idle();
        }
        if !self.settle() {
            return Action::idle();
        }

        let Ok(line) = schema::to_line(obs) else {
            return Action::idle();
        };
        if let Err(e) = self
            .writer
            .write_all(line.as_bytes())
            .and_then(|_| self.writer.flush())
        {
            return self.die(e);
        }

        match self.read_reply() {
            Reply::Line => match schema::from_line::<Action>(&self.line) {
                Ok(action) => {
                    self.stats.decided += 1;
                    action
                }
                Err(_) => {
                    self.stats.malformed += 1;
                    Action::idle()
                }
            },
            Reply::Timeout => {
                self.stats.timeouts += 1;
                self.owed += 1;
                Action::idle()
            }
            Reply::Closed(e) => self.die(e),
        }
    }

    /// Read and discard the replies owed from earlier timeouts. False if one is
    /// still not there, in which case the link stays out of step and this
    /// decision is idle too.
    fn settle(&mut self) -> bool {
        while self.owed > 0 {
            match self.read_reply() {
                Reply::Line => self.owed -= 1,
                Reply::Timeout => return false,
                Reply::Closed(e) => {
                    self.die(e);
                    return false;
                }
            }
        }
        true
    }

    fn read_reply(&mut self) -> Reply {
        self.line.clear();
        match self.reader.read_line(&mut self.line) {
            Ok(0) => Reply::Closed(io::Error::new(ErrorKind::UnexpectedEof, "policy hung up")),
            Ok(_) => Reply::Line,
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                Reply::Timeout
            }
            Err(e) => Reply::Closed(e),
        }
    }

    fn die(&mut self, _why: io::Error) -> Action {
        self.stats.dead = true;
        Action::idle()
    }
}

enum Reply {
    Line,
    Timeout,
    Closed(io::Error),
}

/// A seat driven over a [`Link`]. Several seats may share one link; lockstep
/// decides them one at a time, so the lock is never contended.
pub struct SocketPolicy {
    link: Arc<Mutex<Link>>,
}

impl SocketPolicy {
    pub fn new(link: Arc<Mutex<Link>>) -> Self {
        Self { link }
    }

    /// A link of its own, listening on `addr` for one process.
    pub fn listen(addr: impl ToSocketAddrs, timeout: Duration) -> io::Result<Self> {
        Ok(Self::new(Arc::new(Mutex::new(Link::listen(
            addr, timeout,
        )?))))
    }

    pub fn link(&self) -> &Arc<Mutex<Link>> {
        &self.link
    }

    pub fn stats(&self) -> LinkStats {
        self.link.lock().map(|l| l.stats()).unwrap_or_default()
    }
}

impl Policy for SocketPolicy {
    fn decide(&mut self, obs: &Observation, _now: Tick) -> Action {
        match self.link.lock() {
            Ok(mut link) => link.exchange(obs),
            Err(_) => Action::idle(),
        }
    }
}

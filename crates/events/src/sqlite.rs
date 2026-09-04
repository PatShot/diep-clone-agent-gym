//! The match database: discrete events and the score timeline.
//!
//! One file per match, write-ahead logging on, one transaction per tick. No server
//! to run, the file ships alongside the replay, and it opens directly in pandas.
//!
//! # What is here, and what is not
//!
//! Events and scores. **No kinematics table.** Trajectories were the largest thing a
//! match wrote by an order of magnitude, and they are pure derived data: the replay
//! log plus a re-simulation rebuilds them at whatever rate and filter is wanted.
//! `docs/DESIGN.md` specifies the table; it is deliberately absent, and that is
//! recorded as a proposal in the development log rather than as an edit.
//!
//! Two variants are filtered out. `TickBegin` frames the stream; as a row it would
//! be one hundred and thirty-five thousand records of nothing. `ActionSubmitted` is
//! a second copy of the replay log — the same twelve decisions a tick, stored ten
//! times less compactly, and measured at 235 MB of a 305 MB table. The decisions
//! live in the replay; the database holds their consequences.
//!
//! Filtering is not divergence. The bus still carries every variant to every sink;
//! a sink choosing not to store what another already holds is the point of having
//! more than one.
//!
//! The one thing lost with `ActionSubmitted` is `latency_us`, which is diagnostic
//! and which nothing in v0 measures — scripted policies return immediately. When
//! per-agent compute budgets land it wants a narrow table of its own rather than a
//! full copy of every action riding along beside it.

use std::path::Path;

use rusqlite::{params, Connection};
use schema::Event;

use crate::{Sink, TickRecord};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS match(
  id TEXT PRIMARY KEY,
  seed INTEGER,
  config TEXT,
  started_at TEXT,
  ended_at TEXT,
  winner INTEGER
);

-- `kind` is the serde tag, filled from Event::kind_str(), so a query filters by
-- kind without parsing JSON. `payload` is the rest of the variant; SQLite queries
-- it natively with ->> and it is the right shape while the events are still moving.
CREATE TABLE IF NOT EXISTS events(
  tick INTEGER,
  seq INTEGER,
  kind TEXT,
  payload TEXT,
  PRIMARY KEY(tick, seq)
);

CREATE TABLE IF NOT EXISTS scores(tick INTEGER, team INTEGER, total INTEGER);

CREATE INDEX IF NOT EXISTS idx_events_kind ON events(kind);
"#;

/// Writes a match database.
pub struct SqliteSink {
    conn: Connection,
    rows: u64,
}

impl SqliteSink {
    /// Create or open a match database and apply the schema.
    pub fn create(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        // WAL keeps the writer from blocking a reader, so a match can be queried
        // while it is still running.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn, rows: 0 })
    }

    /// Rows written to `events`.
    pub fn rows(&self) -> u64 {
        self.rows
    }

    fn write(&mut self, rec: &TickRecord<'_>) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut insert =
                tx.prepare_cached("INSERT OR REPLACE INTO events VALUES (?1, ?2, ?3, ?4)")?;
            let mut score =
                tx.prepare_cached("INSERT INTO scores (tick, team, total) VALUES (?1, ?2, ?3)")?;

            for (seq, event) in rec.events.iter().enumerate() {
                if matches!(
                    event,
                    Event::TickBegin { .. } | Event::ActionSubmitted { .. }
                ) {
                    continue;
                }
                let payload = payload_json(event);
                insert.execute(params![rec.tick.0, seq as i64, event.kind_str(), payload])?;
                self.rows += 1;

                match event {
                    // `Scored` already carries the running total, so the timeline
                    // falls out of the events rather than needing two rows a tick.
                    Event::Scored { team, total, .. } => {
                        score.execute(params![rec.tick.0, team.0, total])?;
                    }
                    Event::MatchStart {
                        seed,
                        config_hash,
                        match_id,
                    } => {
                        // Wall clock. Not a determinism violation: the invariant
                        // forbids clock reads in simulation code, and a sink is not
                        // simulation code.
                        let mut m = tx.prepare_cached(
                            "INSERT OR REPLACE INTO match (id, seed, config, started_at) \
                             VALUES (?1, ?2, ?3, datetime('now'))",
                        )?;
                        m.execute(params![match_id, *seed as i64, config_hash.to_string()])?;
                    }
                    Event::MatchEnd { winner, .. } => {
                        let mut m = tx.prepare_cached(
                            "UPDATE match SET ended_at = datetime('now'), winner = ?1",
                        )?;
                        m.execute(params![winner.map(|w| w.0)])?;
                    }
                    _ => {}
                }
            }
        }
        tx.commit()
    }
}

impl Sink for SqliteSink {
    fn accept(&mut self, rec: &TickRecord<'_>) {
        // A sink that cannot write is a broken run, but it is not the simulation's
        // problem and it must not take the tick down. Report and carry on.
        if let Err(e) = self.write(rec) {
            eprintln!("sqlite sink: tick {}: {e}", rec.tick.0);
        }
    }

    fn flush(&mut self) {
        // Every tick commits its own transaction, so there is nothing buffered. The
        // checkpoint folds the write-ahead log back into the file, which is what
        // makes the artefact self-contained.
        let _ = self.conn.pragma_update(None, "wal_checkpoint", "TRUNCATE");
    }
}

/// The `payload` half of an adjacently tagged event.
///
/// `Event` serializes as `{"kind": ..., "payload": ...}`, which maps straight onto
/// the two columns. A unit-like variant has no payload; store an empty object so the
/// column is never null and `->>` never surprises anyone.
fn payload_json(event: &Event) -> String {
    match serde_json::to_value(event) {
        Ok(serde_json::Value::Object(mut map)) => map
            .remove("payload")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "{}".to_string()),
        _ => "{}".to_string(),
    }
}

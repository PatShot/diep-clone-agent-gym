//! The replay log: a seed, a configuration hash, and what the agents decided.
//!
//! Newline-delimited JSON through gzip. Gzip rather than something stronger because
//! `DecompressionStream("gzip")` exists in every browser, and the viewer reads these
//! files directly; at this size the thirty per cent zstd would save is not worth a
//! decoder the browser has to ship.
//!
//! A tick whose inputs match the previous tick writes a repeat marker instead of the
//! whole struct. Scripted policies hold an action for many ticks at a time, so runs
//! of repeats are most of a log.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use schema::{ArenaInfo, Inputs, MatchInfo, ReplayLine, Tick, PROTOCOL_VERSION};

use crate::{Sink, TickRecord};

/// Writes an input log.
pub struct ReplaySink {
    out: Option<GzEncoder<File>>,
    /// The last inputs written, for run-length collapsing.
    last: Option<Inputs>,
    /// How many ticks the pending repeat marker covers.
    repeats: u32,
    ticks: u64,
    lines: u64,
}

impl ReplaySink {
    /// Open a log and write its header.
    pub fn create(
        path: impl AsRef<Path>,
        arena: ArenaInfo,
        match_info: MatchInfo,
    ) -> std::io::Result<Self> {
        let file = File::create(path)?;
        let mut out = GzEncoder::new(file, Compression::default());
        let header = ReplayLine::Header {
            protocol: PROTOCOL_VERSION,
            arena,
            match_info,
        };
        write_line(&mut out, &header)?;
        Ok(Self {
            out: Some(out),
            last: None,
            repeats: 0,
            ticks: 0,
            lines: 1,
        })
    }

    /// Ticks recorded, and lines written. The gap between them is what the repeat
    /// markers saved.
    pub fn counts(&self) -> (u64, u64) {
        (self.ticks, self.lines)
    }

    fn emit_pending_repeat(&mut self) {
        if self.repeats == 0 {
            return;
        }
        let ticks = std::mem::take(&mut self.repeats);
        if let Some(out) = self.out.as_mut() {
            let _ = write_line(out, &ReplayLine::Repeat { ticks });
            self.lines += 1;
        }
    }
}

impl Sink for ReplaySink {
    fn accept(&mut self, rec: &TickRecord<'_>) {
        self.ticks += 1;
        if self.last.as_ref() == Some(rec.inputs) {
            self.repeats += 1;
            return;
        }
        self.emit_pending_repeat();
        self.last = Some(rec.inputs.clone());
        if let Some(out) = self.out.as_mut() {
            let line = ReplayLine::Inputs {
                tick: rec.tick,
                inputs: rec.inputs.clone(),
            };
            let _ = write_line(out, &line);
            self.lines += 1;
        }
    }

    fn flush(&mut self) {
        self.emit_pending_repeat();
        if let Some(out) = self.out.as_mut() {
            let _ = out.flush();
        }
    }
}

impl Drop for ReplaySink {
    fn drop(&mut self) {
        self.emit_pending_repeat();
        // `finish` writes the gzip trailer. Without it the file does not decode.
        if let Some(out) = self.out.take() {
            let _ = out.finish();
        }
    }
}

fn write_line<W: Write>(out: &mut W, line: &ReplayLine) -> std::io::Result<()> {
    let s = serde_json::to_string(line).map_err(std::io::Error::other)?;
    out.write_all(s.as_bytes())?;
    out.write_all(b"\n")
}

/// Reads an input log back, expanding repeat markers.
pub struct ReplayReader {
    lines: std::io::Lines<BufReader<GzDecoder<File>>>,
}

/// A log's opening line.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayHeader {
    pub protocol: u32,
    pub arena: ArenaInfo,
    pub match_info: MatchInfo,
}

impl ReplayReader {
    /// Open a log and consume its header.
    pub fn open(path: impl AsRef<Path>) -> std::io::Result<(ReplayHeader, Self)> {
        let file = File::open(path)?;
        let mut lines = BufReader::new(GzDecoder::new(file)).lines();
        let first = lines
            .next()
            .transpose()?
            .ok_or_else(|| std::io::Error::other("replay log is empty"))?;
        let parsed: ReplayLine = serde_json::from_str(&first).map_err(std::io::Error::other)?;
        let ReplayLine::Header {
            protocol,
            arena,
            match_info,
        } = parsed
        else {
            return Err(std::io::Error::other(
                "replay log does not start with a header",
            ));
        };
        Ok((
            ReplayHeader {
                protocol,
                arena,
                match_info,
            },
            Self { lines },
        ))
    }

    /// Every tick's inputs in order, repeats expanded back into whole ticks.
    ///
    /// Collected rather than streamed. A ninety-minute match is under 140,000
    /// entries and the caller almost always wants to feed them straight into a
    /// world, so an iterator would buy nothing but lifetimes.
    pub fn read_all(self) -> std::io::Result<Vec<(Tick, Inputs)>> {
        let mut out: Vec<(Tick, Inputs)> = Vec::new();
        for line in self.lines {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let parsed: ReplayLine = serde_json::from_str(&line).map_err(std::io::Error::other)?;
            match parsed {
                ReplayLine::Inputs { tick, inputs } => out.push((tick, inputs)),
                ReplayLine::Repeat { ticks } => {
                    let (last_tick, last) = out
                        .last()
                        .cloned()
                        .ok_or_else(|| std::io::Error::other("repeat before any inputs"))?;
                    for i in 1..=ticks {
                        out.push((Tick(last_tick.0 + i), last.clone()));
                    }
                }
                ReplayLine::Header { .. } => {
                    return Err(std::io::Error::other("second header inside a replay log"))
                }
                ReplayLine::Frame { .. } => {
                    return Err(std::io::Error::other(
                        "frame in an input log; frames belong to an exported replay",
                    ))
                }
            }
        }
        Ok(out)
    }
}

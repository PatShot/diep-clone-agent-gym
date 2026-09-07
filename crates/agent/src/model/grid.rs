//! A dense occupancy grid with a flat track list. The first world model.
//!
//! Every cell holds a state and the tick it was last observed. Confidence is not
//! stored: it is a function of age, computed when asked, which is one fewer thing to
//! keep consistent and four fewer bytes per cell.
//!
//! Only ground goes in the grid: free space, and the semi-static things that stand
//! on it — shapes and control centers. Tanks are tracks. Bullets are nothing; by the
//! next decision they are somewhere else or gone.

use std::mem::size_of;

use schema::{
    AgentId, BeliefMsg, CellState, Interest, Kind, MapCell, Observation, Region, Scan, Tick, Track,
    Vec2,
};

use super::tracks::TrackList;
use super::WorldModel;

/// Cells per side. Thirty-two over a thousand units is a cell of about thirty-one
/// units, three tank diameters, and a thousand cells in five kilobytes.
pub const CELLS_PER_SIDE: usize = 32;

/// Tracks held at most. Past this, the stalest are forgotten. A tank sees roughly
/// thirty shapes at a time; the cap is what forces it to forget the far ones.
pub const MAX_TRACKS: usize = 128;

/// A known cell nobody has looked at for this long is worth looking at again.
/// Ten seconds.
pub const STALE_TICKS: u32 = 250;

/// Confidence in a cell halves every this many ticks. Ten seconds.
pub const CONFIDENCE_HALF_LIFE_TICKS: f32 = 250.0;

pub struct DenseGrid {
    side: f32,
    n: usize,
    cell: f32,
    state: Vec<CellState>,
    updated: Vec<Tick>,
    tracks: TrackList,
    now: Tick,
}

impl DenseGrid {
    pub fn new(side: f32) -> Self {
        Self::with_resolution(side, CELLS_PER_SIDE, MAX_TRACKS)
    }

    pub fn with_resolution(side: f32, cells_per_side: usize, max_tracks: usize) -> Self {
        let n = cells_per_side.max(1);
        Self {
            side,
            n,
            cell: side / n as f32,
            state: vec![CellState::Unknown; n * n],
            updated: vec![Tick::ZERO; n * n],
            tracks: TrackList::new(max_tracks),
            now: Tick::ZERO,
        }
    }

    pub fn tracks(&self) -> &TrackList {
        &self.tracks
    }

    pub fn now(&self) -> Tick {
        self.now
    }

    pub fn cells_per_side(&self) -> usize {
        self.n
    }

    /// Cells whose state is known.
    pub fn known_cells(&self) -> usize {
        self.state
            .iter()
            .filter(|s| **s != CellState::Unknown)
            .count()
    }

    fn index(&self, p: Vec2) -> Option<usize> {
        if p.x < 0.0 || p.y < 0.0 || p.x >= self.side || p.y >= self.side {
            return None;
        }
        let cx = ((p.x / self.cell) as usize).min(self.n - 1);
        let cy = ((p.y / self.cell) as usize).min(self.n - 1);
        Some(cy * self.n + cx)
    }

    fn centre_of(&self, i: usize) -> Vec2 {
        let cx = (i % self.n) as f32 + 0.5;
        let cy = (i / self.n) as f32 + 0.5;
        Vec2::new(cx * self.cell, cy * self.cell)
    }

    fn region_of(&self, i: usize) -> Region {
        let min = Vec2::new(
            (i % self.n) as f32 * self.cell,
            (i / self.n) as f32 * self.cell,
        );
        Region::Rect {
            min,
            max: Vec2::new(min.x + self.cell, min.y + self.cell),
        }
    }

    /// Ticks since the cell was observed, or `None` if never.
    fn age_of(&self, i: usize) -> Option<u32> {
        match self.state[i] {
            CellState::Unknown => None,
            _ => Some(self.now.0.saturating_sub(self.updated[i].0)),
        }
    }

    fn set(&mut self, i: usize, state: CellState, at: Tick) {
        self.state[i] = state;
        self.updated[i] = at;
    }

    /// Every cell whose centre lies within `radius` of `centre` was seen, and seen
    /// empty unless something is marked into it afterwards.
    fn observe_disc(&mut self, centre: Vec2, radius: f32, at: Tick) {
        let r2 = radius * radius;
        let (cell, n) = (self.cell, self.n);
        let lo = |v: f32| (((v - radius) / cell).floor().max(0.0)) as usize;
        let hi = |v: f32| (((v + radius) / cell).ceil().min(n as f32)) as usize;
        for cy in lo(centre.y)..hi(centre.y).min(n) {
            for cx in lo(centre.x)..hi(centre.x).min(n) {
                let i = cy * n + cx;
                if self.centre_of(i).distance_squared(centre) <= r2 {
                    self.set(i, CellState::Free, at);
                }
            }
        }
    }

    /// An unknown cell with a known four-neighbour: the edge of what is known.
    fn is_frontier(&self, i: usize) -> bool {
        if self.state[i] != CellState::Unknown {
            return false;
        }
        let (cx, cy) = (i % self.n, i / self.n);
        let mut neighbours = Vec::with_capacity(4);
        if cx > 0 {
            neighbours.push(i - 1);
        }
        if cx + 1 < self.n {
            neighbours.push(i + 1);
        }
        if cy > 0 {
            neighbours.push(i - self.n);
        }
        if cy + 1 < self.n {
            neighbours.push(i + self.n);
        }
        neighbours
            .into_iter()
            .any(|j| self.state[j] != CellState::Unknown)
    }

    /// Cells worth looking at, as `(tier, distance², index)`, sorted. Tier zero is
    /// the frontier proper; tier one is known ground gone stale.
    fn candidates(&self, near: Vec2) -> Vec<(u8, f32, usize)> {
        let mut out = Vec::new();
        for i in 0..self.state.len() {
            let tier = if self.is_frontier(i) {
                0
            } else if self.age_of(i).is_some_and(|a| a >= STALE_TICKS) {
                1
            } else {
                continue;
            };
            out.push((tier, self.centre_of(i).distance_squared(near), i));
        }
        out.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.2.cmp(&b.2))
        });
        out
    }

    fn map_cell(&self, i: usize) -> MapCell {
        MapCell {
            region: self.region_of(i),
            state: self.state[i],
            updated: self.updated[i],
            confidence: self.confidence_of(i),
        }
    }

    fn confidence_of(&self, i: usize) -> f32 {
        match self.age_of(i) {
            None => 0.0,
            Some(age) => 0.5f32.powf(age as f32 / CONFIDENCE_HALF_LIFE_TICKS),
        }
    }

    /// Bytes a message takes on the wire. Newline-delimited JSON, as the wire is.
    fn wire_len(msg: &BeliefMsg) -> usize {
        serde_json::to_string(msg)
            .map(|s| s.len() + 1)
            .unwrap_or(usize::MAX)
    }

    /// The longest prefix of `tracks` whose `TrackSet` fits the budget.
    fn fit_tracks(tracks: Vec<Track>, budget: usize) -> BeliefMsg {
        let mut out = Vec::with_capacity(tracks.len());
        for t in tracks {
            out.push(t);
            let msg = BeliefMsg::TrackSet { tracks: out };
            if Self::wire_len(&msg) > budget {
                let BeliefMsg::TrackSet { mut tracks } = msg else {
                    unreachable!()
                };
                tracks.pop();
                return BeliefMsg::TrackSet { tracks };
            }
            let BeliefMsg::TrackSet { tracks } = msg else {
                unreachable!()
            };
            out = tracks;
        }
        BeliefMsg::TrackSet { tracks: out }
    }

    /// The longest prefix of `cells` whose `MapPatch` fits the budget.
    fn fit_cells(cells: Vec<MapCell>, budget: usize) -> BeliefMsg {
        let mut out = Vec::with_capacity(cells.len());
        for c in cells {
            out.push(c);
            let msg = BeliefMsg::MapPatch { cells: out };
            if Self::wire_len(&msg) > budget {
                let BeliefMsg::MapPatch { mut cells } = msg else {
                    unreachable!()
                };
                cells.pop();
                return BeliefMsg::MapPatch { cells };
            }
            let BeliefMsg::MapPatch { cells } = msg else {
                unreachable!()
            };
            out = cells;
        }
        BeliefMsg::MapPatch { cells: out }
    }

    fn region_centre(r: &Region) -> Option<Vec2> {
        match *r {
            Region::Disc { center, .. } | Region::Annulus { center, .. } => Some(center),
            Region::Rect { min, max } => {
                Some(Vec2::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5))
            }
            Region::WholeArena => None,
        }
    }
}

impl WorldModel for DenseGrid {
    /// Each returned ray frees the cells it crossed and occupies the cell it ended
    /// in. A dropout frees nothing: no return is not the same as nothing there, and
    /// the design says dropouts must not be replaced with the maximum.
    fn ingest_scan(&mut self, scan: &Scan) {
        let at = scan.pose_end.tick;
        self.now = Tick(self.now.0.max(at.0));
        let origin = scan.pose_start.pos;
        let step = self.cell * 0.5;
        for (i, range) in scan.ranges.iter().enumerate() {
            let Some(d) = *range else {
                continue;
            };
            let bearing = scan.bearing(i) + scan.pose_start.heading;
            let (dx, dy) = (bearing.cos(), bearing.sin());
            let mut s = 0.0;
            while s < d {
                let p = Vec2::new(origin.x + dx * s, origin.y + dy * s);
                if let Some(idx) = self.index(p) {
                    self.set(idx, CellState::Free, at);
                }
                s += step;
            }
            let end = Vec2::new(origin.x + dx * d, origin.y + dy * d);
            if let Some(idx) = self.index(end) {
                self.set(idx, CellState::Occupied, at);
            }
        }
    }

    fn ingest_visible(&mut self, obs: &Observation) {
        let at = obs.tick;
        self.now = Tick(self.now.0.max(at.0));
        // Dead. Sees nothing, and must not un-see what it knew.
        if obs.own.entity.is_none() {
            return;
        }
        let me = obs.own.pos;
        self.observe_disc(me, obs.own.sense_radius, at);

        let mut seen = Vec::with_capacity(obs.visible.len());
        for v in &obs.visible {
            if Some(v.id) == obs.own.entity {
                continue;
            }
            match v.style.kind {
                Kind::Bullet => continue,
                Kind::Shape | Kind::ControlCenter => {
                    if let Some(i) = self.index(v.position.pos) {
                        self.set(i, CellState::Occupied, at);
                    }
                }
                Kind::Tank => {}
            }
            seen.push(v.id);
            self.tracks.observe(v, at);
        }
        // A little inside the rim: an entity at the edge may be just outside it
        // this tick and back next, and forgetting it twice a second is noise.
        self.tracks
            .forget_unseen(me, obs.own.sense_radius * 0.9, &seen);
    }

    /// Hearsay is taken where it is fresher than what is held, and nowhere else.
    /// Intent and requests are for a policy to read; a map holds neither.
    fn ingest_belief(&mut self, msg: &BeliefMsg, _from: AgentId) {
        match msg {
            BeliefMsg::TrackSet { tracks } => {
                for t in tracks {
                    self.tracks.merge(t);
                }
            }
            BeliefMsg::MapPatch { cells } => {
                for c in cells {
                    let Some(centre) = Self::region_centre(&c.region) else {
                        continue;
                    };
                    let Some(i) = self.index(centre) else {
                        continue;
                    };
                    let fresher = match self.state[i] {
                        CellState::Unknown => c.state != CellState::Unknown,
                        _ => c.updated > self.updated[i] && c.state != CellState::Unknown,
                    };
                    if fresher {
                        self.set(i, c.state, c.updated);
                    }
                }
            }
            BeliefMsg::Intent { .. } | BeliefMsg::Request { .. } | BeliefMsg::Raw { .. } => {}
        }
    }

    fn tick(&mut self, now: Tick) {
        self.now = now;
        // A disc as wide as the arena says nothing about where the entity is.
        self.tracks.age(now, self.side);
    }

    fn occupancy(&self, p: Vec2) -> CellState {
        self.index(p)
            .map(|i| self.state[i])
            .unwrap_or(CellState::Unknown)
    }

    fn confidence(&self, p: Vec2) -> f32 {
        self.index(p).map(|i| self.confidence_of(i)).unwrap_or(0.0)
    }

    fn frontiers(&self, near: Vec2, k: usize) -> Vec<Region> {
        self.candidates(near)
            .into_iter()
            .take(k)
            .map(|(_, _, i)| self.region_of(i))
            .collect()
    }

    fn tracks_near(&self, p: Vec2, r: f32) -> Vec<Track> {
        self.tracks.near(p, r, self.now)
    }

    fn encode(&self, budget: usize, interest: Interest) -> BeliefMsg {
        let now = self.now;
        let by_freshness = |a: &Track, b: &Track| {
            b.last_seen.cmp(&a.last_seen).then_with(|| {
                a.id.map(|i| (i.index, i.generation))
                    .cmp(&b.id.map(|i| (i.index, i.generation)))
            })
        };
        match interest {
            Interest::Threats => {
                let mut tracks: Vec<Track> = self
                    .tracks
                    .iter()
                    .filter(|e| e.kind == Kind::Tank)
                    .map(|e| e.to_track(now))
                    .collect();
                tracks.sort_by(by_freshness);
                Self::fit_tracks(tracks, budget)
            }
            Interest::Any => {
                let mut tracks: Vec<Track> = self.tracks.iter().map(|e| e.to_track(now)).collect();
                tracks.sort_by(by_freshness);
                Self::fit_tracks(tracks, budget)
            }
            Interest::Near { pos, radius } => {
                let r2 = radius * radius;
                let mut cells: Vec<(f32, usize)> = (0..self.state.len())
                    .filter(|&i| self.state[i] != CellState::Unknown)
                    .map(|i| (self.centre_of(i).distance_squared(pos), i))
                    .filter(|(d2, _)| *d2 <= r2)
                    .collect();
                cells.sort_by(|a, b| {
                    a.0.partial_cmp(&b.0)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.1.cmp(&b.1))
                });
                Self::fit_cells(
                    cells.into_iter().map(|(_, i)| self.map_cell(i)).collect(),
                    budget,
                )
            }
            Interest::Frontiers => {
                let centre = Vec2::new(self.side * 0.5, self.side * 0.5);
                let cells = self
                    .candidates(centre)
                    .into_iter()
                    .map(|(_, _, i)| self.map_cell(i))
                    .collect();
                Self::fit_cells(cells, budget)
            }
        }
    }

    fn footprint(&self) -> usize {
        size_of::<Self>()
            + self.state.capacity() * size_of::<CellState>()
            + self.updated.capacity() * size_of::<Tick>()
            + self.tracks.bytes()
    }
}

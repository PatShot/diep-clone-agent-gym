//! The flat track list. Moving entities, kept out of the grid.
//!
//! `docs/DESIGN.md` §What The Quadtree Does Not Hold: forcing tanks into cells
//! wastes the budget re-subdividing cells a tank left two ticks ago. Tracks live in
//! a list; the grid holds ground.
//!
//! Shapes are tracked too, in v0. The design puts them in a shape field inside the
//! occupancy structure, semi-static and shared once. A dense grid has no such field
//! yet, and a farming baseline needs a position to drive at, so a shape is a track
//! with a very low speed bound. The cap decides what to forget.

use std::mem::size_of;

use schema::{EntityId, EntityView, Kind, ShapeTier, TeamId, Tick, Track, Vec2};
use sim_core::constants::{DT, SHAPE_DRIFT_SPEED, TANK_MAX_SPEED};

/// One remembered entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackEntry {
    pub id: EntityId,
    pub kind: Kind,
    pub tier: Option<ShapeTier>,
    pub team: Option<TeamId>,
    pub pos: Vec2,
    pub vel: Vec2,
    pub hp: Option<f32>,
    pub last_seen: Tick,
}

impl TrackEntry {
    pub fn from_view(v: &EntityView, now: Tick) -> Self {
        Self {
            id: v.id,
            kind: v.style.kind,
            tier: v.style.tier,
            team: v.team.team,
            pos: v.position.pos,
            vel: v.physics.vel,
            hp: v.health.map(|h| h.hp),
            last_seen: now,
        }
    }

    /// Hearsay into an entry. `None` when the track names no entity or no kind: an
    /// anonymous sighting has nothing to merge against, and v0 drops it.
    pub fn from_track(t: &Track) -> Option<Self> {
        Some(Self {
            id: t.id?,
            kind: t.kind?,
            tier: t.tier,
            team: t.team,
            pos: t.last_pos,
            vel: t.vel_estimate,
            hp: t.hp,
            last_seen: t.last_seen,
        })
    }

    /// The speed bound that grows the uncertainty disc. A tank can be anywhere
    /// within its top speed times age; a shape barely moves; a control center
    /// never does.
    pub fn speed_bound(&self) -> f32 {
        match self.kind {
            Kind::Tank => TANK_MAX_SPEED,
            Kind::Shape => SHAPE_DRIFT_SPEED,
            Kind::Bullet | Kind::ControlCenter => 0.0,
        }
    }

    pub fn age(&self, now: Tick) -> u32 {
        now.0.saturating_sub(self.last_seen.0)
    }

    /// Radius of the disc the entity is believed to be inside. `docs/DESIGN.md`
    /// §Belief: maximum speed times age.
    pub fn uncertainty(&self, now: Tick) -> f32 {
        self.speed_bound() * self.age(now) as f32 * DT
    }

    pub fn to_track(&self, now: Tick) -> Track {
        Track {
            id: Some(self.id),
            team: self.team,
            last_pos: self.pos,
            vel_estimate: self.vel,
            last_seen: self.last_seen,
            uncertainty: self.uncertainty(now),
            kind: Some(self.kind),
            tier: self.tier,
            hp: self.hp,
        }
    }
}

/// A capped list of tracks. Capacity is allocated once, so the footprint is fixed.
pub struct TrackList {
    entries: Vec<TrackEntry>,
    cap: usize,
}

impl TrackList {
    pub fn new(cap: usize) -> Self {
        Self {
            entries: Vec::with_capacity(cap),
            cap,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &TrackEntry> {
        self.entries.iter()
    }

    pub fn get(&self, id: EntityId) -> Option<&TrackEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Own sighting. Replaces whatever was believed about this entity.
    pub fn observe(&mut self, v: &EntityView, now: Tick) {
        let entry = TrackEntry::from_view(v, now);
        self.put(entry, now);
    }

    /// Hearsay. Taken only when fresher than what is held. A track that merely
    /// repeats what is already known adds nothing: a rumour that comes back is not
    /// news, and this is the line that keeps it from becoming confidence.
    pub fn merge(&mut self, t: &Track) -> bool {
        let Some(entry) = TrackEntry::from_track(t) else {
            return false;
        };
        if let Some(held) = self.get(entry.id) {
            if held.last_seen >= entry.last_seen {
                return false;
            }
        }
        self.put(entry, entry.last_seen);
        true
    }

    /// Negative evidence. An entity believed to be inside the sensed disc that was
    /// not among the entities seen there is no longer there. Forget it.
    pub fn forget_unseen(&mut self, centre: Vec2, radius: f32, seen: &[EntityId]) {
        let r2 = radius * radius;
        self.entries
            .retain(|e| e.pos.distance_squared(centre) > r2 || seen.contains(&e.id));
    }

    /// Forget what has decayed past use, then hold the cap.
    ///
    /// A track whose uncertainty disc has grown to `forget_beyond` says nothing
    /// about where the entity is. Past the cap, the stalest go first.
    pub fn age(&mut self, now: Tick, forget_beyond: f32) {
        self.entries.retain(|e| e.uncertainty(now) < forget_beyond);
        while self.entries.len() > self.cap {
            self.evict_one(now);
        }
    }

    /// Tracks within `r` of `p`, nearest first. Ties break on entity identity so
    /// the order is the same on every run.
    pub fn near(&self, p: Vec2, r: f32, now: Tick) -> Vec<Track> {
        let r2 = r * r;
        let mut hits: Vec<(f32, &TrackEntry)> = self
            .entries
            .iter()
            .filter(|e| e.pos.distance_squared(p) <= r2)
            .map(|e| (e.pos.distance_squared(p), e))
            .collect();
        hits.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    (a.1.id.index, a.1.id.generation).cmp(&(b.1.id.index, b.1.id.generation))
                })
        });
        hits.into_iter().map(|(_, e)| e.to_track(now)).collect()
    }

    /// Bytes held. Capacity, not length: the allocation is the footprint.
    pub fn bytes(&self) -> usize {
        self.entries.capacity() * size_of::<TrackEntry>()
    }

    fn put(&mut self, entry: TrackEntry, now: Tick) {
        if let Some(slot) = self.entries.iter_mut().find(|e| e.id == entry.id) {
            *slot = entry;
            return;
        }
        if self.entries.len() >= self.cap {
            self.evict_one(now);
        }
        self.entries.push(entry);
    }

    /// Remove the stalest track. Among equals, the one remembered longest.
    fn evict_one(&mut self, now: Tick) {
        if let Some(i) = (0..self.entries.len())
            .max_by_key(|&i| (self.entries[i].age(now), std::cmp::Reverse(i)))
        {
            self.entries.remove(i);
        }
    }
}

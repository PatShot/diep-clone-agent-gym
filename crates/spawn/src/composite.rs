//! The composite spawner: a `Vec<Focus>` and the arithmetic that drives it.

use rand::rngs::StdRng;
use schema::{FocusId, Tick, Vec2};
use sim_core::arena::ArenaSpec;
use sim_core::constants::DT;
use sim_core::{shape_radius, World};

use crate::config::SpawnConfig;
use crate::focus::{CountBy, Focus, SpawnRequest};
use crate::place::{distance_to_rect, sample, Occupancy};
use crate::Spawner;

/// How many shapes one focus currently has alive. Reported for instrumentation;
/// the spawner recomputes it every tick regardless.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusCensus {
    pub focus: FocusId,
    pub alive: usize,
}

/// Weighted foci, walked in configuration order.
///
/// Holds no world state. Its only memory between ticks is one fractional spawn
/// credit per focus, which is what lets a rate of 0.25 per second mean anything at
/// a 25 hertz [Hz] tick.
pub struct CompositeSpawner {
    config: SpawnConfig,
    /// Fractional spawn credit carried between ticks, one per focus.
    accum: Vec<f32>,
    /// Alive count per focus. A field rather than a local so the allocation is
    /// made once.
    census: Vec<usize>,
    occupancy: Option<Occupancy>,
}

impl CompositeSpawner {
    pub fn new(config: SpawnConfig) -> Self {
        let n = config.foci.len();
        Self {
            config,
            accum: vec![0.0; n],
            census: vec![0; n],
            occupancy: None,
        }
    }

    pub fn config(&self) -> &SpawnConfig {
        &self.config
    }

    /// Alive shapes per focus, as of the last [`Spawner::tick`] or [`Self::census`]
    /// call.
    pub fn last_census(&self) -> Vec<FocusCensus> {
        self.config
            .foci
            .iter()
            .zip(&self.census)
            .map(|(f, &alive)| FocusCensus { focus: f.id, alive })
            .collect()
    }

    /// Count alive shapes per focus by walking the entity table.
    ///
    /// A scan rather than a cached set of identifiers. At a couple of thousand
    /// entities and a handful of foci it costs nothing, and it cannot drift out of
    /// step with slot recycling the way a cache would. What each focus counts is
    /// its own choice; see [`CountBy`].
    pub fn census(&mut self, world: &World) {
        let side = world.spec().arena.side;
        for c in &mut self.census {
            *c = 0;
        }
        for (_, e) in world.iter_entities() {
            if !e.is_shape() {
                continue;
            }
            for (i, focus) in self.config.foci.iter().enumerate() {
                if e.focus != Some(focus.id) {
                    continue;
                }
                let counts = match focus.count_by {
                    CountBy::Origin => true,
                    CountBy::Resident => focus.region.contains(e.pos, side),
                };
                if counts {
                    self.census[i] += 1;
                }
            }
        }
    }

    /// The shapes an arena should start a match with.
    ///
    /// Called once, before the first tick. Returns requests like [`Spawner::tick`]
    /// does; the caller applies them.
    pub fn prefill(&mut self, world: &World, rng: &mut StdRng) -> Vec<SpawnRequest> {
        let arena = &world.spec().arena;
        self.census(world);
        let occ = self
            .occupancy
            .get_or_insert_with(|| Occupancy::new(arena.side));
        occ.rebuild(world);

        let mut out = Vec::new();
        for (i, focus) in self.config.foci.iter().enumerate() {
            if !focus.enabled || focus.prefill == 0 {
                continue;
            }
            let want = focus
                .prefill
                .min(focus.num_max.saturating_sub(self.census[i]));
            let placed = place_batch(
                focus,
                arena,
                self.config.base_keepout,
                self.config.place_attempts,
                want,
                world,
                occ,
                rng,
                &mut out,
            );
            self.census[i] += placed;
        }
        out
    }
}

impl Spawner for CompositeSpawner {
    fn tick(&mut self, world: &World, _now: Tick, rng: &mut StdRng) -> Vec<SpawnRequest> {
        let arena = &world.spec().arena;
        self.census(world);

        // Decide how many each focus is owed before touching the spatial index, so
        // that a quiet tick — the common case — does no work beyond the census.
        let mut wanted = vec![0usize; self.config.foci.len()];
        let mut total = 0usize;
        for (i, focus) in self.config.foci.iter().enumerate() {
            if !focus.enabled {
                continue;
            }
            let alive = self.census[i];
            let throttle = focus.throttle(alive);
            if throttle <= 0.0 {
                continue;
            }
            self.accum[i] += focus.respawn_per_sec * throttle * DT;
            let earned = self.accum[i].floor();
            if earned < 1.0 {
                continue;
            }
            self.accum[i] -= earned;
            // Credit beyond what the focus has room for is dropped rather than
            // banked. Banking it would make a focus that sat full for a minute
            // dump a minute of shapes the instant one died.
            let n = (earned as usize).min(focus.num_max.saturating_sub(alive));
            wanted[i] = n;
            total += n;
        }
        if total == 0 {
            return Vec::new();
        }

        let occ = self
            .occupancy
            .get_or_insert_with(|| Occupancy::new(arena.side));
        occ.rebuild(world);

        let mut out = Vec::with_capacity(total);
        for (i, focus) in self.config.foci.iter().enumerate() {
            if wanted[i] == 0 {
                continue;
            }
            let placed = place_batch(
                focus,
                arena,
                self.config.base_keepout,
                self.config.place_attempts,
                wanted[i],
                world,
                occ,
                rng,
                &mut out,
            );
            self.census[i] += placed;
            // Give back the credit for anything the arena had no room for, so a
            // temporarily crowded focus catches up rather than losing the shape.
            self.accum[i] += (wanted[i] - placed) as f32;
        }
        out
    }
}

/// Place up to `n` shapes for one focus. Returns how many succeeded.
#[allow(clippy::too_many_arguments)]
fn place_batch(
    focus: &Focus,
    arena: &ArenaSpec,
    keepout: f32,
    attempts: u32,
    n: usize,
    world: &World,
    occ: &mut Occupancy,
    rng: &mut StdRng,
    out: &mut Vec<SpawnRequest>,
) -> usize {
    let mut placed = 0;
    for _ in 0..n {
        let Some(tier) = focus.draw_tier(rng) else {
            break;
        };
        let radius = shape_radius(tier);

        let mut found = None;
        for _ in 0..attempts {
            let p = sample(&focus.region, arena.side, radius, rng);
            if !acceptable(focus, arena, keepout, p, radius) {
                continue;
            }
            if !occ.is_free(world, p, radius) {
                continue;
            }
            found = Some(p);
            break;
        }
        // Out of budget. The region is crowded or misconfigured; stop asking this
        // tick rather than burning the same budget again for the next shape.
        let Some(pos) = found else { break };

        occ.note(pos, radius);
        out.push(SpawnRequest {
            tier,
            pos,
            vel: focus.drift.velocity(rng),
            focus: focus.id,
        });
        placed += 1;
    }
    placed
}

/// Every placement rule that does not need to know what else is alive.
fn acceptable(focus: &Focus, arena: &ArenaSpec, keepout: f32, p: Vec2, radius: f32) -> bool {
    // Inside the arena, with room for the whole circle.
    if p.x < radius || p.y < radius || p.x > arena.side - radius || p.y > arena.side - radius {
        return false;
    }
    // Inside the focus. Discs and annuli can sample outside after the arena inset.
    if !focus.region.contains(p, arena.side) {
        return false;
    }
    for region in &focus.exclude {
        if region.contains(p, arena.side) {
            return false;
        }
    }
    // Clear of both bases by the keep-out distance. A base is a sanctuary, and a
    // shape spawning next to one would feed whoever is camping the exit.
    for base in &arena.bases {
        if distance_to_rect(p, base) < keepout + radius {
            return false;
        }
    }
    true
}

//! Every tunable number, in one file.
//!
//! # Provisional
//!
//! The constants extraction pass against diep.io is still outstanding. Almost
//! everything below is marked PROVISIONAL and carries the reasoning that produced
//! it. That reasoning is the point: our arena is 1000 units on a side against
//! diep.io's ~22300, so the reference physics numbers need rescaling regardless and
//! copying them exactly would not save the tuning pass.
//!
//! Values traceable to `docs/STATS.md` are marked STATS and should not be changed
//! without changing the document.
//!
//! # Units
//!
//! Distance is arena units. The arena is 1000 units on a side. Time is seconds, and
//! one tick is exactly 0.04 of one. Rates named `_PER_SEC` are divided by the tick
//! rate at the point of use, never pre-multiplied here, so reading a rate off this
//! file gives a number in units a human thinks in.

// ---------------------------------------------------------------------------
// Clock
// ---------------------------------------------------------------------------

/// Simulation rate. diep.io's own tick rate, which is why one tick is exactly 0.04
/// seconds and the per-tick tables in `docs/STATS.md` transfer without rounding.
pub const TICK_HZ: u32 = 25;

/// Seconds per tick. Exact in binary: 0.04 is not, but 1/25 computed once is the
/// same value everywhere, which is what determinism needs.
pub const DT: f32 = 1.0 / TICK_HZ as f32;

/// Ticks in one second. Used to convert the `_PER_SEC` rates below.
pub const TICKS_PER_SEC: f32 = TICK_HZ as f32;

// ---------------------------------------------------------------------------
// Arena
// ---------------------------------------------------------------------------

/// Arena side length. Square, origin at the top-left corner.
pub const ARENA_SIDE: f32 = 1000.0;

/// Base side length. Two squares at opposite corners, northwest and southeast.
pub const BASE_SIDE: f32 = 200.0;

/// Nest radius. Disc at the arena centre, the only place high-tier shapes spawn.
pub const NEST_RADIUS: f32 = 150.0;

// ---------------------------------------------------------------------------
// Ranges
// ---------------------------------------------------------------------------

/// How far a tank senses. Uniform across every tank in v0: class differences are
/// deferred so that range-limited communication is the only asymmetry under study.
pub const TANK_SENSE_RADIUS: f32 = 120.0;

/// How far a tank can talk. Exceeds the sense radius, which is the entire reason
/// communication has value: a tank can report what a teammate cannot see. Were it
/// shorter, information would be trapped where it was gathered.
pub const TANK_COMMS_RADIUS: f32 = 200.0;

/// How far a control center [CC] can talk. Larger than a tank's, and still far
/// short of the ~1100 unit base-to-base diagonal.
pub const CC_COMMS_RADIUS: f32 = 350.0;

// ---------------------------------------------------------------------------
// Radii
// ---------------------------------------------------------------------------

/// PROVISIONAL. Tank radius, 1% of the arena side. Twelve tank-widths of sight in
/// every direction, which is near-blind relative to the arena, as intended.
pub const TANK_RADIUS: f32 = 10.0;

/// PROVISIONAL. Common shape radius.
pub const SHAPE_RADIUS_COMMON: f32 = 8.0;

/// PROVISIONAL. High-tier shape radius. Visibly the bigger prize.
pub const SHAPE_RADIUS_HIGH: f32 = 16.0;

/// PROVISIONAL. Bullet radius.
pub const BULLET_RADIUS: f32 = 4.0;

/// PROVISIONAL. Control center radius. Rendering and sensing only. A CC has no
/// collision and cannot be destroyed.
pub const CC_RADIUS: f32 = 14.0;

/// Largest radius any entity may take. The collision grid sizes its cells from
/// this, so a radius above it would let two overlapping circles land in
/// non-adjacent cells and miss each other.
pub const MAX_ENTITY_RADIUS: f32 = SHAPE_RADIUS_HIGH;

// ---------------------------------------------------------------------------
// Movement
// ---------------------------------------------------------------------------

/// PROVISIONAL. Terminal speed under full thrust, units per second. Crosses the
/// 120-unit sense radius in two seconds and the arena diagonal in twenty-four.
pub const TANK_MAX_SPEED: f32 = 60.0;

/// PROVISIONAL. Velocity retained per tick. Ten percent lost per tick is roughly
/// ninety-three percent per second, which is the heavy damping diep.io has and the
/// reason a tank stops nearly as fast as it starts.
pub const DRAG: f32 = 0.9;

/// Acceleration under full thrust, units per second squared.
///
/// Derived rather than chosen, so that editing `TANK_MAX_SPEED` moves the terminal
/// speed and nothing else. Solving `v = (v + a·dt)·drag` for the fixed point gives
/// `a = v·(1 − drag) / (dt·drag)`. Reaches 95% of terminal in about 28 ticks.
pub const TANK_ACCEL: f32 = TANK_MAX_SPEED * (1.0 - DRAG) / (DT * DRAG);

/// PROVISIONAL. How fast shapes drift. Slow enough to be scenery, fast enough that
/// a remembered position goes stale, which is what makes belief decay matter.
pub const SHAPE_DRIFT_SPEED: f32 = 4.0;

/// PROVISIONAL. Backward impulse applied to a tank on firing, units per second.
pub const RECOIL_IMPULSE: f32 = 15.0;

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

/// STATS. Base tank health at level 1. `docs/STATS.md`: `50 + 2·(level − 1)`.
pub const TANK_BASE_HP: f32 = 50.0;

/// STATS. Health added per level, before stat points.
pub const TANK_HP_PER_LEVEL: f32 = 2.0;

/// PROVISIONAL. Health regenerated per second, as a fraction of maximum.
///
/// This number and the one below carry the focus-fire pressure. See
/// [`REGEN_DELAY_TICKS`].
pub const REGEN_FRACTION_PER_SEC: f32 = 0.30;

/// PROVISIONAL. Ticks after taking damage before regeneration resumes.
///
/// Deliberately shorter than [`RELOAD_TICKS`], and that relationship is the whole
/// mechanism. A lone attacker firing every 15 ticks leaves a 15-tick gap, so
/// regeneration runs for 5 of every 15 ticks and claws back part of each shot.
/// Two attackers interleaving shots close the gaps below this threshold and
/// regeneration never starts at all.
///
/// Measured against a healthy 50 HP tank, one attacker to three:
///
/// | Attackers | Time to kill |
/// |---|---|
/// | 1 | 6.60 s |
/// | 2 | 2.08 s |
/// | 3 | 1.40 s |
///
/// Doubling the attackers more than triples the rate. That superlinearity is the
/// point: focus fire is a threshold, not a sum.
///
/// So focus fire is not merely additive damage. It is a threshold, crossed by
/// agreeing on a target inside a window — and neither attacker can see the other's
/// aim, so crossing it requires a message. That is the coordination pressure the
/// sandbox exists to measure, expressed as two numbers.
///
/// Changing this above `RELOAD_TICKS` switches the pressure off. Do it knowingly.
pub const REGEN_DELAY_TICKS: u32 = 10;

// Enforced at compile time rather than in a test, because a test asserting a
// relation between two constants is a tautology the optimiser is entitled to
// delete. Inverting this inequality lets a lone attacker suppress regeneration by
// itself, and coordination stops being the thing under study.
const _: () = assert!(
    REGEN_DELAY_TICKS < RELOAD_TICKS,
    "the regeneration delay must stay below one reload interval"
);

// ---------------------------------------------------------------------------
// Weapons
// ---------------------------------------------------------------------------

/// STATS. Ticks between shots at zero reload points. `docs/STATS.md` gives 0.60
/// seconds per bullet, which is 15 ticks exactly at 25 Hz.
pub const RELOAD_TICKS: u32 = 15;

/// STATS. Bullet damage at zero points, against a tank or a shape.
pub const BULLET_DAMAGE: f32 = 7.0;

/// STATS. Bullet health at zero penetration points, for a basic tank: the table's
/// base of 2, times the basic tank's multiplier of 4. A bullet is an entity with
/// health, so penetration is not a special case — it is the bullet surviving the
/// body damage of what it hits.
pub const BULLET_HP: f32 = 8.0;

/// PROVISIONAL. Bullet speed, units per second. Well above tank top speed, so
/// fleeing does not outrun fire.
pub const BULLET_SPEED: f32 = 220.0;

/// PROVISIONAL. Bullet lifetime in ticks. Chosen so that range is 123 units,
/// just past the 120-unit sense radius: a tank can shoot as far as it can see and
/// no further, so firing blind is never rewarded by the physics.
pub const BULLET_LIFETIME_TICKS: u32 = 14;

/// STATS. Body damage at zero points, against a shape.
pub const BODY_DAMAGE_VS_SHAPE: f32 = 20.0;

/// STATS. Body damage against a tank: 50% above the base rate.
pub const BODY_DAMAGE_VS_TANK: f32 = 30.0;

/// STATS. Body damage against a projectile: 75% below the base rate. This is what
/// wears a bullet down, and the reason a bullet passes through some things.
pub const BODY_DAMAGE_VS_PROJECTILE: f32 = 5.0;

// ---------------------------------------------------------------------------
// Shapes
// ---------------------------------------------------------------------------

/// PROVISIONAL. Common shape health. Two bullets and change.
pub const SHAPE_HP_COMMON: f32 = 20.0;

/// PROVISIONAL. High-tier shape health. Twenty-two bullets, which is deliberately
/// more than one tank can land before the shape's own contact damage becomes a
/// problem. The nest is meant to need company.
pub const SHAPE_HP_HIGH: f32 = 150.0;

/// PROVISIONAL. Body damage a common shape deals on contact.
pub const SHAPE_BODY_DAMAGE_COMMON: f32 = 8.0;

/// PROVISIONAL. Body damage a high-tier shape deals on contact.
pub const SHAPE_BODY_DAMAGE_HIGH: f32 = 12.0;

/// PROVISIONAL. Points for destroying a common shape. Read by the objective crate,
/// not by this one.
pub const SHAPE_VALUE_COMMON: u32 = 10;

/// PROVISIONAL. Points for destroying a high-tier shape. Thirteen times a common
/// shape for roughly eight times the health, which is what makes contesting the
/// nest worth more than farming the edges in safety.
pub const SHAPE_VALUE_HIGH: u32 = 130;

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// PROVISIONAL. Ticks between a tank's death and its respawn.
///
/// `docs/DESIGN.md` leaves open whether respawn should instead cost team score,
/// making death a resource decision. A fixed delay is the placeholder, not the
/// answer.
pub const RESPAWN_DELAY_TICKS: u32 = 75;

/// Separation applied per tick to resolve an overlap, as a fraction of the
/// penetration depth. Below one so that contacts settle over a few ticks instead of
/// snapping apart, which keeps a tank pressed against a wall from jittering.
pub const SEPARATION_STRENGTH: f32 = 0.5;

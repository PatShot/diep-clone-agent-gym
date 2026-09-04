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

/// PROVISIONAL. Square radius. The smallest and most common shape.
pub const SHAPE_RADIUS_SQUARE: f32 = 7.0;

/// PROVISIONAL. Triangle radius.
pub const SHAPE_RADIUS_TRIANGLE: f32 = 10.0;

/// PROVISIONAL. Pentagon radius. Visibly the bigger prize.
pub const SHAPE_RADIUS_PENTAGON: f32 = 16.0;

/// PROVISIONAL. Alpha pentagon radius, and the reason [`MAX_ENTITY_RADIUS`] is what
/// it is.
///
/// Deliberately only two units above a pentagon. diep.io's alpha pentagon is a
/// landmark you can see across the map; this one is not. It earns its place through
/// points per unit of area, not through occupying the nest. A shape large enough to
/// fill the disc would reduce how many prizes fit there, which is the opposite of
/// making the centre rich.
pub const SHAPE_RADIUS_ALPHA: f32 = 18.0;

/// PROVISIONAL. Bullet radius.
pub const BULLET_RADIUS: f32 = 4.0;

/// PROVISIONAL. Control center radius. Rendering and sensing only. A CC has no
/// collision and cannot be destroyed.
pub const CC_RADIUS: f32 = 14.0;

/// Largest radius any entity may take. The collision grid sizes its cells from
/// this, so a radius above it would let two overlapping circles land in
/// non-adjacent cells and miss each other.
pub const MAX_ENTITY_RADIUS: f32 = SHAPE_RADIUS_ALPHA;

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

// The three tiers are ordered Square < Triangle < Pentagon, in health, in contact
// damage, and in points. The ratios are what carry the design; the absolute numbers
// are rescaled to this arena and this bullet, not copied from diep.io.
//
// PROVENANCE. `docs/STATS.md` carries no shape table, so the diep.io values these
// are shaped after (10, 25 and 130 points) come from recall, not from a document in
// this repository. Every number below is PROVISIONAL for that reason. The reference
// pass against diepcustom/src/Const/ would settle them.

/// PROVISIONAL. Square health. Three bullets at [`BULLET_DAMAGE`].
pub const SHAPE_HP_SQUARE: f32 = 20.0;

/// PROVISIONAL. Triangle health. Seven bullets. Farmable alone, but slow enough
/// that a tank doing it is committed and not watching the map.
pub const SHAPE_HP_TRIANGLE: f32 = 45.0;

/// PROVISIONAL. Pentagon health. Twenty-two bullets, which is deliberately more
/// than one tank can land before the shape's own contact damage becomes a problem.
/// The nest is meant to need company.
pub const SHAPE_HP_PENTAGON: f32 = 150.0;

/// PROVISIONAL. Alpha pentagon health. Fifty-eight bullets.
///
/// A team of five firing continuously deals about 58 damage per second, so an alpha
/// takes them seven seconds and takes one tank half a minute — long enough that its
/// contact damage decides the trade. Paired with [`SHAPE_VALUE_ALPHA`] this works
/// out to two points per point of health, against a pentagon's 0.87 and a
/// triangle's 0.56. Points per unit of damage dealt is the number that decides
/// where a team farms, so it is the number the alpha is designed around.
pub const SHAPE_HP_ALPHA: f32 = 400.0;

/// PROVISIONAL. Body damage a square deals on contact.
pub const SHAPE_BODY_DAMAGE_SQUARE: f32 = 8.0;

/// PROVISIONAL. Body damage a triangle deals on contact.
pub const SHAPE_BODY_DAMAGE_TRIANGLE: f32 = 12.0;

/// PROVISIONAL. Body damage a pentagon deals on contact. High enough that a lone
/// tank grinding one down loses the trade.
pub const SHAPE_BODY_DAMAGE_PENTAGON: f32 = 20.0;

/// PROVISIONAL. Body damage an alpha pentagon deals on contact.
pub const SHAPE_BODY_DAMAGE_ALPHA: f32 = 30.0;

/// PROVISIONAL. Points for destroying a square. Read by the objective crate, not
/// by this one.
pub const SHAPE_VALUE_SQUARE: u32 = 10;

/// PROVISIONAL. Points for destroying a triangle. Two and a half squares for a bit
/// over twice the health, so it is a marginal improvement, not a reason to travel.
pub const SHAPE_VALUE_TRIANGLE: u32 = 25;

/// PROVISIONAL. Points for destroying a pentagon. Thirteen squares for seven and a
/// half times the health, which is what makes contesting the nest worth more than
/// farming the edges in safety.
pub const SHAPE_VALUE_PENTAGON: u32 = 130;

/// PROVISIONAL. Points for destroying an alpha pentagon.
///
/// Six pentagons of value in 1.27 pentagons of area. That ratio is the whole design:
/// the alpha raises what a defended nest yields per unit of ground without making
/// the nest physically fuller. See [`NEST_ALPHA_RATE`] for how the figure was set.
pub const SHAPE_VALUE_ALPHA: u32 = 800;

// ---------------------------------------------------------------------------
// Shape spawn rates
// ---------------------------------------------------------------------------

/// PROVISIONAL. Baseline spawn rate for a focus at full throttle, shapes per
/// second. Five tanks farming steadily out-pace it, which is what keeps the
/// resource finite in the short run and makes travelling somewhere else worth
/// doing.
pub const SHAPE_SPAWN_RATE: f32 = 3.0;

/// Pentagon spawn rate at the nest, as a multiple of [`SHAPE_SPAWN_RATE`].
///
/// The nest is meant to be the richest source of points on the map by a wide
/// margin, so that the arena centre is worth crossing open ground for and worth
/// navigating carefully once you are there. Five times the baseline is what makes
/// that true in points rather than in prose.
pub const NEST_PENTAGON_RATE_MULT: f32 = 5.0;

/// Square and triangle spawn rate at the nest, as a multiple of
/// [`SHAPE_SPAWN_RATE`]. Reduced by 0.8 from the baseline, so the nest still
/// carries the common shapes but they are not what anyone goes there for.
pub const NEST_MINOR_RATE_MULT: f32 = 1.0 - 0.8;

/// Spawn rate at one wing, shapes per second. A third of the baseline.
pub const WING_SPAWN_RATE: f32 = SHAPE_SPAWN_RATE / 3.0;

/// Alpha pentagon spawn rate at the nest, shapes per second.
///
/// Set so that holding the nest answers the alternative: a team farming both wings
/// unopposed. A wing spawns one shape a second at 70% triangles and 30% squares,
/// which is 20.5 points per second, so two wings are 41. At
/// [`SHAPE_VALUE_ALPHA`] points each, 41 points per second is one alpha every
/// 19.5 seconds, or 0.051 per second. The shipped figure is a little above that,
/// because the nest is ground that has to be held and the wings are not.
///
/// Alphas alone therefore match the wings. The pentagons a defending team farms
/// with its remaining damage are the margin that makes the centre worth taking:
/// about 75 points per second against the wings' 41.
pub const NEST_ALPHA_RATE: f32 = 0.055;

// ---------------------------------------------------------------------------
// Shape population
// ---------------------------------------------------------------------------
//
// Both limits are PER FOCUS, not global. Each farming area saturates on its own,
// so a team that prioritises one area pays a cost the other areas do not. A global
// cap would let a crowded nest suppress spawning at the edges, which is the
// opposite of the pressure these are for.

/// Alive shapes from one focus at which its respawn rate begins to fall. Below
/// this the focus spawns at its configured rate.
pub const SHAPE_NUM_SLOW: usize = 1200;

/// Alive shapes from one focus at which it stops spawning entirely. Between
/// [`SHAPE_NUM_SLOW`] and this, the rate ramps linearly to zero, so a filling area
/// yields less and less and exploring elsewhere starts to pay.
///
/// A focus is also bounded by its own `capacity`, which is what its region can
/// physically hold. The effective ceiling is the smaller of the two: the nest disc
/// at radius 150 has nowhere near the area for 2000 pentagons.
pub const SHAPE_NUM_MAX: usize = 2000;

/// PROVISIONAL. How far from a base a shape must spawn, in units from the base
/// rectangle. Set to one [`TANK_SENSE_RADIUS`] so that a tank sitting on the base
/// boundary can see no freshly spawned shape. Camping a base exit then feeds
/// nobody, which is the point.
pub const SHAPE_SPAWN_BASE_KEEPOUT: f32 = TANK_SENSE_RADIUS;

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// PROVISIONAL. Ticks between a tank's death and its respawn.
///
/// `docs/DESIGN.md` leaves open whether respawn should instead cost team score,
/// making death a resource decision. A fixed delay is the placeholder, not the
/// answer.
pub const RESPAWN_DELAY_TICKS: u32 = 75;

// ---------------------------------------------------------------------------
// Mass
// ---------------------------------------------------------------------------
//
// PROVENANCE. diep.io resolves contacts through `receiveKnockback` with push and
// absorption factors carried in its physics field group, but those values are not
// published and the wiki does not carry them. Nothing was copied. The hierarchy
// below is derived from our own score table instead, normalised so a square is one:
// a shape's mass is its point value divided by `SHAPE_VALUE_SQUARE`.
//
// Deriving mass from score rather than from area is deliberate. Area would make an
// alpha pentagon only 1.27 times a pentagon, which is nowhere near enough to keep
// it still. Score is what the design already uses to say how much a shape matters,
// and "the valuable ones are hard to move" is the property the nest needs.

/// Mass of a square. The unit every other mass is expressed against.
pub const MASS_SQUARE: f32 = 1.0;

/// Mass of a triangle. `SHAPE_VALUE_TRIANGLE / SHAPE_VALUE_SQUARE`.
pub const MASS_TRIANGLE: f32 = 2.5;

/// Mass of a pentagon. Thirteen squares, so a square shoving one moves it by a
/// fourteenth of the overlap and takes the rest itself.
pub const MASS_PENTAGON: f32 = 13.0;

/// Mass of an alpha pentagon. Eighty squares. A passing square moves it by one
/// part in eighty-one, which is what stops the nest's prize from being walked out
/// of the nest by traffic.
pub const MASS_ALPHA: f32 = 80.0;

/// Mass of a tank. Between a pentagon and an alpha, so a tank clears squares and
/// triangles out of its way, shoulders past a pentagon with effort, and cannot
/// move an alpha at all.
pub const MASS_TANK: f32 = 20.0;

/// Floor applied to any mass before it divides. Guards the mass split against a
/// zero that would otherwise produce a division by zero.
pub const MIN_MASS: f32 = 0.001;

// ---------------------------------------------------------------------------
// Contact response and drag
// ---------------------------------------------------------------------------

/// PROVISIONAL. Overlap converted to velocity on contact, per unit of penetration
/// per second.
///
/// A contact is a collision, not just an overlap to be edited away: both bodies
/// come out of it moving, in inverse proportion to their mass. This is the term
/// that gives drag something to act on. Without it, separation writes positions
/// directly and a shape is walked around the arena at zero velocity, where no drag
/// force can reach it.
pub const CONTACT_IMPULSE: f32 = 3.0;

/// PROVISIONAL. Velocity a shape retains per tick. Same form as [`DRAG`], which
/// does this for tanks.
///
/// At 0.94 a shape keeps about 21% of its speed after one second, so a shove
/// travels a short way and stops rather than becoming a permanent course change.
pub const SHAPE_DRAG: f32 = 0.94;

/// PROVISIONAL. Velocity an alpha pentagon retains per tick.
///
/// Much heavier than [`SHAPE_DRAG`]. An alpha is already hard to move by mass; this
/// makes the little movement it does acquire die almost at once, so the nest's
/// prize stays where the nest put it.
pub const ALPHA_PEN_DRAG: f32 = 0.70;

/// Speed below which a shape is treated as at rest, units per second.
///
/// Its velocity is zeroed and its slot leaves the movement bitmap, so the movement
/// step stops paying for it. Small enough that a shape crosses well under a
/// hundredth of its own radius per tick before being parked.
pub const MOVING_EPSILON: f32 = 0.05;

/// Separation applied per tick to resolve an overlap, as a fraction of the
/// penetration depth. Below one so that contacts settle over a few ticks instead of
/// snapping apart, which keeps a tank pressed against a wall from jittering.
pub const SEPARATION_STRENGTH: f32 = 0.5;

# AI Development Log

Written by the assistant, one entry per session. Everything else in `docs/` is human
domain and is not edited from a session, unless specified elsewhere in AGENTS.md.

This file records what was built, what was decided, and what was found. Where a
session concludes that a specification needs changing, the proposed change is written
here as a suggestion and the specification is left untouched.

---

## Session `session_015rMEehHFh6HK9LNAyZ8NDW` — 2026-09-04

**Scope.** v0 build order step 3, the spawner. Steps 1 and 2 were complete on entry.
The session also reopened the shape tier enum in `schema` at the user's direction,
which is a step 1 change made from a step 3 session.

### Decisions Settled

Six were put to the user. Three were answered as recommended, one was overridden
with different numbers, and two were volunteered.

| Decision | Outcome |
|---|---|
| Step 3 scope | All three foci in one pass, composite. Wings present, disabled |
| Configuration format | Typed structs now. TOML deferred to the crate with a binary |
| Spawn randomness | The world generator, via `World::rng()` |
| Shape tiers | Square, Triangle, Pentagon. Replaces Common and High |
| Population limits | `SHAPE_NUM_SLOW` 1200 and `SHAPE_NUM_MAX` 2000, **per focus** |
| Placement | Random positions with a keep-out distance from both bases |
| Nest richness | Pentagons at 5x the baseline rate, common shapes at 0.2x |
| Nest exclusion | Dropped. Scatter shapes may land in the nest |
| Alpha pentagons | Added as a fourth tier, nest only, sized to yield not to bulk |

The per-focus reading of the limits is the load-bearing one. A global cap would let
a crowded nest suppress spawning at the arena edges, which is the opposite of the
pressure the limits exist to create. Per focus, each farming area saturates on its
own, so prioritising one area carries a cost the others do not pay.

The throttle curve was not specified, so the linear ramp was taken: full rate at or
below `SHAPE_NUM_SLOW`, zero at `SHAPE_NUM_MAX`, a straight line between.

### The Tier Change

`ShapeTier` was `Common | High`. It is now
`Square | Triangle | Pentagon | AlphaPentagon`, ordered, with `PartialOrd` and `Ord`
derived so a policy can compare tiers without a lookup table. Forty-five references moved with it across eight files, all mechanical, plus
a regenerated `client/src/gen/schema.ts`.

This was cheap because nothing is recorded yet. Once step 4 lands the SQLite sink,
changing a variant that appears in `Event::Spawned` means migrating recorded
matches. The window for this edit closes at step 4.

Provenance is worth stating plainly. `docs/STATS.md` carries no shape table, so the
diep.io values the new numbers are shaped after — 10, 25 and 130 points — come from
recall, not from a document in this repository. Every one of them is marked
PROVISIONAL in `constants.rs` for that reason, and the ratios are what carry the
design rather than the absolute figures.

### Built

The crate is `spawn`, in `crates/spawn`. No package-name collision, so unlike
`sim-core` the directory and the package agree.

| File | Role | Lines |
|---|---|---|
| `lib.rs` | `Spawner` trait, the `apply` free function, module docs | 65 |
| `focus.rs` | `Focus`, `CountBy`, `DriftModel`, `SpawnRequest`, throttle, tier draw | 140 |
| `config.rs` | `SpawnConfig` and the shipped arrangement of five foci | 215 |
| `place.rs` | Region sampling, rectangle distance, the `Occupancy` index | 109 |
| `composite.rs` | `CompositeSpawner`: census, rate accrual, placement | 266 |

Two things changed outside the crate. `schema::ShapeTier` gained its three variants,
and `sim_core::shape_radius` was added so the spawner can know a shape's radius
before the entity exists, without a second copy of the radius table.

`Spawner::tick` takes the world by shared reference and returns `Vec<SpawnRequest>`.
The caller applies them through `spawn::apply`, which is a free function rather than
a trait method so that the line between deciding and mutating stays visible at the
call site. The spawner's only memory between ticks is one fractional spawn credit
per focus, which is what lets a rate of 0.25 per second mean anything at 25 Hz.

Placement is rejection sampling against five rules: inside the arena with room for
the circle, inside the focus region, outside every excluded region, clear of both
bases by the keep-out, and overlapping nothing alive. The budget is 32 attempts. Out
of budget, the focus stops asking for the tick and keeps its credit.

Tests: 19 behavioural across two files plus a doc test. The whole workspace is 128
tests, `cargo fmt
--check` clean, `cargo clippy --workspace --all-targets -D warnings` clean.

One of them, `no_tick_requests_more_than_the_headroom`, exists because the obvious
assertion is not quite true. A resident count can sit slightly above its ceiling
without anything being wrong: contact separation pushes a shape out of its region on
one tick and back in on another, and a shape that re-enters was never spawned. The
band governs spawning, not membership. What holds exactly is that no tick requests
more than the headroom, so that is what is asserted, and the residency tests carry a
documented tolerance instead.

### The Alpha Pentagon

Added as a fourth tier at the user's direction, on one condition: a team that holds
the nest and nothing else must out-earn a team farming both wings without contest.
That condition set every number.

| Quantity | Value | Reasoning |
|---|---|---|
| Radius | 18 | Two units above a pentagon. Deliberately not diep.io's landmark |
| Health | 400 | Seven seconds for a team of five, half a minute for one tank |
| Value | 800 | Six pentagons of points in 1.27 pentagons of area |
| Points per health | 2.0 | Against a pentagon's 0.87 and a triangle's 0.56 |
| Spawn rate | 0.055/s | One every eighteen seconds |
| Band | 2 and 4 | Its own focus, `FOCUS_NEST_ALPHA`, sharing the nest disc |

The size instruction is the interesting one. Making the prize physically larger
would have *reduced* the nest's worth per unit area, because a bigger shape displaces
more of the disc for the same count. Barely-larger plus far-more-valuable moves
points per unit area up instead, which is what was asked for.

The rate is derived rather than chosen. A wing spawns one shape a second at 70%
triangles and 30% squares, which is 20.5 points per second, so two wings are 41. At
800 points each that is one alpha every 19.5 seconds. The shipped 0.055 per second is
a shade above, because the nest is ground that has to be held and the wings are not.
`alphas_cover_the_yield_of_both_wings` asserts the relation in both directions: at
least the wings' yield, and no more than 1.5 times it, so the wings stay a trade-off
rather than a formality.

Alphas needed a focus of their own because a `Focus` carries one population band, and
four alphas against forty-five pentagons at a twentieth of the rate is a different
band. It also makes "alpha pentagons spawn only at the nest" structural rather than a
weight that could be edited.

### Measured Yield

`cargo run -p spawn --example nest_geometry --release`. Thirty simulated minutes to
settle, then ten minutes with a team's worth of damage — 58.3 per second, five tanks
firing without pause — pointed at whatever is worth most inside the region. Wings are
enabled for the comparison; they ship disabled.

| Region | Radius | Pentagons | Alphas | Points held | Points per 10k area | Farmed per second | Farmed per second per 10k area |
|---|---|---|---|---|---|---|---|
| Nest | 150 | 24 | 5 | 7875 | 1114.1 | 76.2 | 10.78 |
| Nest x1.2 | 180 | 22 | 4 | 7585 | 745.2 | 77.9 | 7.66 |
| Nest x1.5 | 225 | 39 | 4 | 14175 | 891.3 | 78.6 | 4.94 |
| Wing | 100 | 3 | 0 | 1785 | 568.2 | 31.5 | 10.02 |

The condition holds. The nest yields 76.2 points per second against a wing's 31.5, so
holding the centre beats farming both wings unopposed — 63.0 — by 21%. Alphas supply
roughly the wings' worth on their own, and the pentagons a team farms with its
remaining damage are the margin. Standing still, the nest is 1.96 times a wing in
points per unit area.

### The Nest Radius Experiment

Growing the nest makes it worse per unit area, almost exactly in proportion to the
area added.

| Nest radius | Area, relative | Farmed per second | Per unit area | Per unit area, relative |
|---|---|---|---|---|
| 150 | 1.00 | 76.2 | 10.78 | 1.00 |
| 180 | 1.44 | 77.9 | 7.66 | 0.71 |
| 225 | 2.25 | 78.6 | 4.94 | 0.46 |

The reciprocals of the area ratios are 0.69 and 0.44. The measured yield densities are
0.71 and 0.46. That is not a coincidence and it has one cause: **the nest's yield is
limited by how fast a team can deal damage, not by how much the nest supplies.** The
nest already spawns pentagons at 15 per second, worth 1950 points per second, while
five tanks can only extract about 77. Adding area adds supply that was never the
constraint, and spreads the same yield over more ground.

The practical consequence is that a bigger nest is a worse objective, not a better
one. At radius 150 the nest and a wing are within 8% of each other in yield per unit
area — 10.78 against 10.02 — so the centre is worth taking because it is *concentrated*,
not because it is generous. At 1.5 times the radius that concentration is gone and the
nest is less rewarding per unit of ground held than a wing, while being far harder to
hold. If the nest is ever widened, the bands and the alpha rate have to be scaled with
the area or the centre stops being worth contesting.

### Configuration Moved To A File

`AGENTS.md` specifies step 3 as config-driven. It was typed structs only for most of
this session, by an early decision to defer TOML until a crate had a binary to load
it. That decision was reversed and `config/spawn.toml` now exists, read by `toml`,
a new workspace dependency.

The load-bearing piece is not the loader. It is the test:
`the_shipped_file_matches_the_shipped_defaults` parses the file and asserts equality
with `SpawnConfig::default()`. A configuration file and a set of defaults that drift
apart silently are worse than no file at all, because every run would then use values
nobody read.

That test caught its first problem immediately. `SHAPE_SPAWN_RATE * (5.0 + 0.2)` is
15.599999 in `f32`, not 15.6, so a hand-typed `15.6` parses to a different float and
the comparison fails. The shipped file carries the exact serialized value. Writing
the file by transcribing the serializer's output, then adding comments, is the only
way this stays true; typing the numbers from the source would not have.

`examples/steady_state` reads the file rather than the defaults, so the configuration
is genuinely load-bearing. Disabling the nest focus in `config/spawn.toml` and re-running
takes the nest column to zero and the points on the board from 15,355 to 10,995.

`arena.toml` and `match.toml` are deliberately absent. They belong to the crates that
read them, and inventing their shape before anything consumes it would fix the wrong
decisions early.

### Collision Response Rewritten

`World::separate` split every overlap evenly and wrote positions directly. It now
resolves a contact as a collision between two bodies with mass, splitting both a
positional correction and a velocity impulse in inverse proportion to mass.

Mass is derived from the score table, normalised so a square is one: square 1,
triangle 2.5, pentagon 13, alpha pentagon 80, tank 20. Deriving from score rather than
from area is the point. By area an alpha is 1.27 pentagons, nowhere near enough to
keep it still; by score it is eighty squares, and a passing square then moves it by
one part in eighty-one.

Provenance, since `AGENTS.md` names diepcustom as constants-only and AGPL: diep.io
resolves contacts through `receiveKnockback` with push and absorption factors carried
in its physics field group, but those values are not published and the wikis do not
carry them. Nothing was copied. The hierarchy is ours.

Two consequences followed.

**Drag now has something to act on.** Measured before the rewrite: an alpha's speed
was exactly 0.0000 at every sample while it travelled 421 units across the arena, on
14,637 ticks out of 15,000. A position-only correction moves a shape without ever
giving it velocity, so no drag force can reach it — `-d0 * v` where `v` is zero. The
impulse is what makes drag meaningful. `SHAPE_DRAG` is 0.94 per tick and
`ALPHA_PEN_DRAG` is 0.70.

**Shapes now come to rest.** This was settled explicitly: shapes stop where a
collision leaves them, and nothing is tethered to its spawn point. Total travel from
any push is `v0 * DT / (1 - drag)`, which at the shipped constants is 2.7 units.

### The Movement Bitmap

The movement step walked every entity to move the few that were moving. It now reads
a bitset over slot indices, `crates/core/src/bitmap.rs`, and touches only what is in
motion. A shape enters when something shoves it and leaves once drag brings it under
`MOVING_EPSILON`.

| Simulated time | Entities | Moving | Share |
|---|---|---|---|
| 1 min | 841 | 25 | 3.0% |
| 6 min | 1622 | 107 | 6.6% |
| 9 min | 1837 | 158 | 8.6% |

Set bits come back in ascending slot order, the same order `Store::iter` uses, so the
determinism invariant is untouched.

Two things make it safe rather than merely fast. `World` keeps a slot-to-handle table
so a set bit resolves to an entity, and a bit whose handle no longer resolves is
dropped rather than trusted. And `entity_mut` marks its slot unconditionally: a caller
may set a velocity, nothing else would enter that slot, and drag parks it again next
tick if it turns out to be still.

**The tick got slower, not faster.** The ninety-minute soak went from 0.39 to 0.68
seconds in release, about 1.7 times. The movement step is cheaper; the contact sweep
is dearer, because shapes that come to rest settle into touching clusters instead of
drifting apart, and each contact re-triggers its neighbours. This is understood and
deliberately not fixed. See the deferred item below.

### The Alpha Pentagon Does Not Stay In The Nest

Stated plainly because it is the one requirement this session did not meet. Alpha
pentagons are `DriftModel::Static` and are placed in the inner third of the disc, and
they still leave it. Over thirty simulated minutes at the shipped configuration, five
sat in the nest and five had been pushed out of it.

The cause is in `core`, not in `spawn`. Contact separation splits the penetration
evenly between two overlapping entities regardless of what they are, so a 20-health
square shoves a 400-health alpha exactly as hard as the alpha shoves the square. The
arena at `SHAPE_NUM_MAX` runs near half its area covered, and the squares drift, so
those contacts never stop. A shape in a crowded arena is slowly walked out of wherever
it was put.

Six configurations were measured before concluding this, and none of them fixed it:

| Configuration | Alphas in the nest | Alphas outside | Points per 10k area |
|---|---|---|---|
| Shipped band, no inset | 5 | 11 | 1122 |
| Placement inset to 0.5R | 4 | 10 | 1062 |
| Placement inset to 0.35R | 5 | 6 | 1222 |
| Nest band lowered to 20/30 | 5 | 13 | 1099 |
| Inset 0.5R and band 20/30 | 5 | 7 | 1000 |
| Inset 0.5R and band 14/22 | 4 | 11 | 781 |

Loosening the nest band does not help, which rules out nest crowding as the cause:
escapes continue at 30% fill. Counting alphas by origin instead of residency caps the
total at four and stops the leak, but the nest then ends a run holding **zero** of
them, because nothing replaces one that has been pushed out. Points per unit area
falls from about 1100 to under 600. That is a worse failure than the leak.

The shipped configuration is the best of them: residency counting with placement
inset to the inner 35%, which roughly halves the escapes and gives the highest points
per unit area of any option tried.

**That mitigation has since been joined by the actual fix.** Separation is now
mass-weighted, and an alpha is eighty squares, so a passing square displaces it by one
part in eighty-one rather than by half. The eviction numbers above all predate that
change and **have not been re-measured**. They should be, before anyone concludes
either that the problem is solved or that it is not.

### Findings

**The ramp is visible in five simulated minutes.** `cargo run -p spawn --example
steady_state --release` reports every thirty seconds.

| Time | Scatter | Nest | Alphas | Spawned in the interval |
|---|---|---|---|---|
| 30s | 690 | 45 | 3 | 99 |
| 120s | 960 | 46 | 3 | 91 |
| 180s | 1140 | 45 | 3 | 95 |
| 240s | 1311 | 41 | 3 | 83 |
| 300s | 1450 | 43 | 2 | 69 |

Ninety-odd per interval while below the slow threshold, falling to sixty-nine once
past it. The nest and its alphas hold flat at their bands the whole way. That deceleration is the
farming pressure, and it is now a number rather than an intention.

**The nest rate multiples hold to within measurement noise.** Over two simulated
minutes with every band lifted out of the way, pentagons spawn at 14.82 per second
against the scatter focus's 3.00, a ratio of 4.94 where 5.0 was asked for. The
nest's common shapes come in at 0.2 times the baseline. The weights are written as
the multiples themselves and divided through by the focus rate, so the two cannot
drift apart silently.

**The scatter ceiling does not coincide with the jamming limit. Correcting an
earlier claim in this entry.** The first version of this finding said the two
limits met at 54% coverage and that the placement budget, not the band, halted the
scatter focus. Both halves were wrong, and both came from arithmetic rather than
measurement.

Measured over 45 simulated minutes: the scatter focus reaches exactly its band of
2000 and stops there, at 49.4% arena area coverage — below the roughly 54% at which
random sequential adsorption of discs jams. The band binds first. There is no
coincidence to be broken by a later change to `SHAPE_RADIUS_SQUARE`.

The number that produced the wrong claim was 949, the point at which a single
greedy prefill pass gives up. That is not a jamming limit. Prefill breaks out of a
focus on the first shape that exhausts its 32 attempts, whereas the tick path gets
a fresh budget every tick and keeps finding the gaps. A one-pass figure and a
steady-state figure are different quantities, and reading the first as the second
understated the ceiling by more than half.

**A focus that counts its shapes by origin cannot maintain a place.** This was the
session's real defect and it was invisible until the run was long enough. The nest
census counted shapes carrying its `FocusId`, wherever they had drifted to. Nest
shapes drift, keep the tag, and stop being in the nest. The focus therefore
reported itself full at 45 while the disc emptied:

| Simulated time | Nest census | Shapes actually in the disc | Points per unit area, nest against arena |
|---|---|---|---|
| 1m | 45 | 99 | 1.70x |
| 15m | 45 | 68 | 0.48x |
| 45m | 45 | 16 | 0.11x |

The richest ground on the map became nine times sparser than average. `Focus` now
carries a `CountBy` choice. The nest and the wings count *residents* — shapes they
made that are still standing in them — so a pentagon that leaves is replaced.

The middle option was tried and rejected: counting every shape standing in the
region, whoever made it. Drifting squares from the scatter focus wander through the
nest, count against its band, and starve pentagon respawn. The disc then fills with
the cheapest shapes on the map, and richness fell to 0.60x at fifteen minutes.
Whether a place is physically full is the placement budget's question, not the
population band's.

**Focus order is a placement priority, and getting it wrong is silent.** Foci are
walked in configuration order at prefill and on every tick, and placement is
first-come. With the scatter focus listed first it prefills 600 shapes across the
whole arena, the nest then packs 45 pentagons into the disc, and the alpha focus —
confined to the inner third of that disc, the smallest and most contested region on
the map — finds nowhere to stand. It placed *one* alpha in five simulated minutes
instead of four, and nothing failed: no error, no warning, just a rare shape that
was quietly never there. Listing the alpha focus first fixes it, and the ordering
now carries a comment saying it is load-bearing. Any focus with a small region and a
low rate has this problem, so a later `spawn.toml` needs to preserve order rather
than treat the list as a set.

**Drifting nest shapes are an unbounded leak.** Nothing caps a shape once it leaves
the focus that made it: the scatter band counts only scatter-origin shapes. With
the nest replacing every escapee, the arena total climbed from 2043 at twenty-five
minutes to 2502 at forty-five and was still rising. Pentagons are now
`DriftModel::Static`, which closes it — the total flattens near 2220 — and also
means the map's fixed prize stays where the map says it is. Squares and triangles
still drift, and they are 98% of shapes, so the stale-belief pressure
`SHAPE_DRIFT_SPEED` exists for is intact.

**The nest is richest by points per unit area, not by head-count.** Head-count is
the wrong measure and reading it first was misleading. The disc holds around 45
shapes against an arena-wide 2200, but a pentagon is 13 squares in points and 5.2
squares in area. Measured at the shipped configuration, the nest runs 1.5 to 1.9
times the arena's points per unit area, and covers 44 to 49% of its own ground
against the arena's 49%. Early in a match, before the arena fills, it is 5.4 times
richer.

Worth stating plainly: the nest is the densest and richest *ground*, but it is not
where most of the points are. Two thousand scattered shapes hold roughly 25,000
points against the nest's 5,000. A team that farms the edges in safety out-earns
one that contests the centre, on current numbers. Closing that gap is a matter of
lowering `SHAPE_NUM_MAX` or raising pentagon value, and both are decisions rather
than fixes.

**Per-focus limits cannot be uniform across foci.** The constants are literal for
the scatter focus, which covers the whole arena. They are meaningless for the nest:
a disc of radius 150 is 70,686 square units, and 2000 pentagons at radius 16 need
1,608,000 — over-subscribed twenty-three times. `Focus` therefore carries its own
`num_slow` and `num_max`, defaulting to the constants, and the nest and wings set
theirs to what their geometry holds. Measured rather than estimated: a greedy pass
packs 39 pentagons into the disc, so the nest band is 30 and 45. The ramp shape is
identical everywhere; only the scale moves.

**The keep-out has a principled value rather than a chosen one.**
`SHAPE_SPAWN_BASE_KEEPOUT` is `TANK_SENSE_RADIUS`, 120 units. A tank sitting on its
base boundary can then see no freshly spawned shape, so camping a base exit feeds
nobody. Tying it to the sense radius means the two move together if sensing is
retuned.

**Placements made in one tick must be tracked separately from the world.** Requests
are not entities until the caller applies them, so the spatial index cannot know
about a shape decided two lines earlier. `Occupancy` carries a small pending list
for exactly this. Without it, a tick spawning several shapes could stack them.

### Suggestions

**`docs/DESIGN.md` Entities section: two tiers is now three.** The document reads
"Two tiers: Common, High". The code has Square, Triangle, Pentagon. Squares and
triangles scatter; pentagons are nest-only, so the property the section is built
around — the richest resource sits equidistant from both bases — is unchanged.

**`docs/DESIGN.md` Spawner section: the `Focus` struct has drifted.** The document
shows `capacity: usize`. The implementation has `num_slow` and `num_max`, and adds
`count_by: CountBy`, `exclude: Vec<Region>`, `prefill: usize` and `enabled: bool`.
`CountBy` is the one that changes a design claim rather than adding a knob: a focus
maintains either a population or a place, and the nest only works as a place.
`prefill` exists because an arena that starts empty and fills over three minutes is
a different experiment from one that starts populated.

**`docs/DESIGN.md` Arena section: the nest is no longer a clearing.** The document
says high-tier shapes spawn only in the nest, which remains true. It does not say
whether common shapes may spawn there, and the implementation now says they may: the
scatter focus excludes nothing, so its squares and triangles fall inside the disc
like anywhere else, and the nest piles pentagons on top. The centre is therefore the
densest ground on the map as well as the richest, which is what makes crossing it a
navigation problem rather than a straight line.

**`docs/DESIGN.md` open question on respawn is still open**, and now blocks less
than it did. Carried forward from the previous session unchanged.

**`AGENTS.md` build order position.** Step 3 is complete. Step 4, `events`, is next
on the numbered order; the previous session's suggestion to insert `agent` after
`spawn` and `objective` before `server` is unchanged and still not applied.

**Deferred: a collision threshold for chain reactions.** `collect_contacts` is now
the dominant cost of a tick, and the soak runs 1.7 times slower than it did. The cause
is understood: shapes that come to rest settle into touching clusters rather than
drifting apart, so contacts persist and each one re-triggers its neighbours. The
proposal is a penetration threshold — below it, skip the contact and leave both bodies
at rest. Deliberately not built. A slower tick is not blocking anything, and the
threshold interacts with the movement bitmap in ways worth measuring before choosing a
number.

**Deferred: re-measure alpha eviction under mass-weighting.** As above. The last
figures are from before masses existed.

**Built, having first been proposed here: mass-weighted separation.** This was
written up as a suggestion because it is a `core` decision with consequences for
tank-against-tank contacts, and was then asked for directly. `World::separate` now
splits both the positional correction and a velocity impulse in inverse proportion to
mass. See "Collision Response Rewritten" above.

**`docs/DESIGN.md` Entities section: a shape no longer drifts indefinitely.** The
document says a shape "drifts slowly and does not fight back". With `SHAPE_DRAG` in
place a shape drifts briefly and then comes to rest, and a collision moves it a couple
of units before it settles again. This was a deliberate choice, made explicitly: the
alternative was a restoring pull toward the spawn point, and shapes being magically
dragged home was rejected.

The consequence worth naming is that `SHAPE_DRIFT_SPEED`'s stated purpose — "fast
enough that a remembered position goes stale, which is what makes belief decay matter"
— no longer holds for shapes. Belief about shape positions stops decaying once the
arena settles. Belief about *tanks* still decays, because tanks move constantly, so
the pressure survives through them rather than through terrain. Whether that is
acceptable is a design call, not an implementation one.

**Add a shape table to `docs/STATS.md`, or delete the expectation of one.** The
constants file now points at a document that has no shape numbers in it. Either the
reference pass fills that table, in which case the PROVISIONAL marks on the shape
block come off, or the block should say plainly that shapes were never in STATS.

### Carried Forward

- `config/spawn.toml` does not exist. `SpawnConfig::default()` is the configuration
  until a crate with a binary can load a file.
- Wings ship disabled, at the midpoints between the arena centre and the two corners
  the bases do not occupy. `ArenaSpec::wings` is still empty; the wing regions live
  in `SpawnConfig`, not in the arena. Those two should agree before the viewer draws
  either.
- Alpha placement is limited by room, not by its band: the nest holds two or three
  against a band of four, because the inner third of the disc is contested with
  pentagons and drifting squares. Widening the inset trades that back against more
  escapes. The trade disappears if separation becomes mass-weighted in `core`.
- The nest holds about a tenth of the points on the board once the arena fills. If
  the centre is meant to be worth fighting over in score terms and not only in
  density, `SHAPE_NUM_MAX` comes down or `SHAPE_VALUE_PENTAGON` goes up. Neither is
  a change to make without deciding which.
- Nothing farms yet, so the example's population only climbs. The curve under
  farming pressure is not observable until the `agent` crate exists.

---

## Session `session_01P39hvSyuhbe1LHAAQu7btX` — 2026-09-04

**Scope.** v0 build order step 2, the simulation core. Step 1 was complete on entry.

### Decisions Settled

Four were put to the user and answered. One was volunteered.

| Decision | Outcome |
|---|---|
| Dependencies for the core crate | `slotmap` and `rand` |
| Tick rate | 25 Hz, one tick exactly 0.04 seconds |
| Step 2 scope | Kinematics plus combat and lifecycle. Scoring stays out |
| Constants | Provisional now, extraction pass deferred |
| Tank types | One tank type in v0, uniform sensing. Volunteered by the user |

The tank type decision is the load-bearing one. Every tank shares the same sense
radius, the same field of view, and the same base stats, so that range-limited
communication is the only asymmetry under study. Class differentiation becomes a
variable to introduce later, measured against this baseline.

`rand` is pinned to `=0.10.2`. `StdRng` is documented as not reproducible across
releases, so the determinism invariant rests on that pin and on the checked-in
`Cargo.lock`, not on any guarantee from the crate. Bumping it is a decision about
whether recorded matches still replay.

### Built

The crate is `sim-core`, in `crates/core`.

The package is **not** named `core`. A crate named `core` shadows the sysroot crate
of that name in every dependent, and `core::mem` there then fails to resolve against
our crate instead of the standard library. The error message names the wrong crate.
This was verified in a scratch workspace before committing to the name. The directory
still follows the layout in `AGENTS.md`; only the package name differs.

| File | Role | Lines |
|---|---|---|
| `constants.rs` | Every tunable number, each marked STATS or PROVISIONAL with its reasoning | 248 |
| `arena.rs` | Geometry, arena bounds, base sanctuary rule | 194 |
| `store.rs` | `slotmap` arena; its key maps one-to-one onto `schema::EntityId` | 102 |
| `grid.rs` | Uniform-cell broadphase, reusable by the v0.5 sensor sweep | 116 |
| `entity.rs` | Simulation-side entity, converting to the wire field groups | 253 |
| `world.rs` | The tick | 868 |

`World::step` takes agent actions and control center [CC] commands, advances one
fixed step, and appends to an event buffer the caller drains. It opens no files,
reads no clock, and holds no channels. The tick order is fixed and documented at the
top of `world.rs`; commands drain before physics, and damage is collected across the
whole contact sweep before any of it is applied, so that an entity killed early in the
sweep still deals the contact damage it was owed.

Determinism holds by construction. One seeded generator, no wall-clock reads, and
every container walked in the tick is ordered by slot index or by key. The agent table
is a `BTreeMap` for that reason alone.

Tests: 35 behavioural, one broadphase property test against brute force, one
full-length soak. All pass, with `cargo fmt --check` and
`cargo clippy --all-targets -D warnings` clean.

A 90-minute match simulates in 0.39 seconds, roughly 346,000 ticks per second. That
figure matters for the parameter sweeps in the Experiments section of the design.

### Findings

**The focus-fire threshold now has measured numbers.** `REGEN_DELAY_TICKS` is 10 and
sits deliberately below `RELOAD_TICKS` at 15. A lone attacker leaves gaps wider than
the regeneration delay and the target heals between shots; two attackers interleaving
close those gaps and regeneration never starts. Against a healthy 50 HP tank:

| Attackers | Time to kill |
|---|---|
| 1 | 6.60 s |
| 2 | 2.08 s |
| 3 | 1.40 s |

Doubling the attackers more than triples the rate. Focus fire is a threshold, not a
sum, which is what makes agreeing on a target worth a message. The inequality is
enforced by a compile-time assertion rather than a test, because a test comparing two
constants is a tautology the optimiser may delete.

**The soak test found a real bug.** Bases sit flush in the arena corners, so
least-penetration ejection from the northwest base pushed entities north or west,
through the arena wall and out of the world at negative coordinates. An entity there
is outside every collision path and never returns. Ejection now considers only
directions that leave the circle inside the arena. Two short tests had been passing
only because of this bug; both were corrected, and a sweep over every base position
and radius pins the case.

**Shapes reflect off arena walls.** Shapes have no steering. A shape that stopped dead
at a wall would stay there for the rest of the match, and over ninety minutes every
shape ends up lining the edges while the nest quietly stops being contested.

### Suggested Changes To `docs/DESIGN.md`

Not applied. Five edits were made during the session and then reverted when the
ownership rule was stated. They are reproduced here as proposals.

1. **Rates, line 203.** `Physics 30 Hz` becomes `Physics 25 Hz`, and the following
   paragraph's `30 Hz world` becomes `25 Hz world`. Rationale worth adding: 25 Hz is
   diep.io's own tick rate, so one tick is exactly 0.04 seconds and the per-tick
   tables in `STATS.md` transfer without rounding. A reload of eight ticks is eight
   ticks, not 9.6.

2. **Volume, line 514.** `bullets at 30 Hz` becomes `bullets at 25 Hz`.

3. **Progression, after line 123.** Record the one-tank-type decision: every tank
   shares the same sense radius, field of view, and base stats; class is not chosen
   and not used. The sequencing argument is that two coordination pressures —
   range-limited communication and role differentiation — running at once makes
   neither measurable. The `Class` enum stays in the schema and stays `None` through
   v0, so introducing it later costs no migration.

4. **Coordination Pressures, lines 158-160.** Mark *Irreversible class commitment* as not
   active in v0, with a pointer to Progression. The mechanic is still wanted; it is
   deferred, not dropped.

5. **Reference Material, line 684.** `Outstanding: the constants extraction pass. Not
   done.` now understates the position. A provisional set lives in
   `crates/core/src/constants.rs`, with values traceable to `STATS.md` marked STATS
   and the rest marked PROVISIONAL alongside the reasoning that produced them. The
   reference pass against `diepcustom/src/Const/` remains undone.

### Suggested Changes To `AGENTS.md`

Also reverted, for the same reason. `AGENTS.md` sits at the repository root rather
than in `docs/`, so it falls outside the letter of the rule, but it is a human
specification in the same sense.

1. **Build order position, line 55.** `step 1 complete. Step 2 not started` is now
   stale. Step 2 is complete and step 3 is not started. Until this is edited, this log
   is the accurate record of position.

2. **Build order, item 2.** The delivered scope is wider than the line describes:
   bullet lifetime, contact damage, regeneration, and death were included by decision.

3. **Invariants, line 81.** `Position updates at 30 Hz` becomes 25 Hz, to match the
   tick rate decision.

4. **Reference Material, constants task.** Same revision as item 5 above.

5. **Optional.** A line recording what is settled for v0 — one tank type, uniform
   sense and comms radius, no class choice, physics at 25 Hz — would save
   re-deriving it next session.

### Suggestions

**Add a `Cargo.lock` note to the working agreement.** The determinism invariant now
depends on the lockfile being respected. That is not currently written down anywhere a
future session would look before running `cargo update`.

**Settle the respawn question before `objective`.** `docs/DESIGN.md` leaves open
whether respawn costs time or team score. The current implementation uses a fixed
75-tick delay as a placeholder. Score-cost respawn interacts with the point target,
so deciding it after the objective crate exists means changing both.

**The provisional constants need a tuning pass, not an extraction pass.** The arena is
1000 units against diep.io's roughly 22300. Reference physics values need rescaling on
arrival rather than copying, so extracting them exactly saves less than it appears.
The values worth extracting are the ones that are ratios rather than distances.

**Consider whether bullets should inherit the firer's velocity.** diep.io does. The
implementation does not, so that bullet range stays a constant, which both the
scripted baselines and after-match analysis lean on. Marked PROVISIONAL in the code.

**Consider a real-time display mode entry point early.** Not for v0. But the tick loop
takes its inputs as a plain struct and holds no clock, which is what makes a real-time
driver a wrapper rather than a rewrite. Worth not losing.

### Carried Forward

- Score accrual is deliberately absent. `World::add_score` and `Entity::value` are the
  hooks the `objective` crate writes through.
- The sensor sweep is empty, as `schema::sense` intends for v0. `Grid` is in place for
  the digital differential analyzer [DDA] traversal at v0.5.

---

### Build Order Status

Recorded after the viewer was scoped and then deliberately deferred. A vertical slice
through steps 3, 5 and 6 would have put something on screen sooner. Building in order
was chosen instead, so shapes, policies and transport are settled before anything
renders.

Numbered steps, as `AGENTS.md` defines them:

| Step | Crate | Delivers | Status |
|---|---|---|---|
| 1 | `schema` | Event enum, entity types, wire messages, TypeScript generation | Complete |
| 2 | `core`, packaged `sim-core` | Entities, fixed-step physics, circle collision, arena bounds, bullet lifetime, contact damage, regeneration, death | Complete |
| 3 | `spawn` | `Spawner` trait, `Focus`, composite implementation. Uniform scatter, then nest, then wings. Config-driven | Next |
| 4 | `events` | Bus, `Sink` trait, file sink, SQLite sink | Not started |
| 5 | `server` | Lockstep driver, WebSocket, snapshot and delta encoding, command intake | Not started |
| 6 | `client` | Canvas render of arena, tanks, shapes, scores. Then replay loading | Not started |

Named in the layout, absent from the numbered order:

| Crate | Delivers | Consequence of the gap |
|---|---|---|
| `agent` | `Policy` trait, scripted baselines, socket bridge | Nothing drives a tank. Step 6 renders ten stationary tanks |
| `objective` | `Objective` trait, `PointTarget` | Nothing scores. `Event::Scored` never fires and a match has no end condition |
| `config/` | `arena.toml`, `spawn.toml`, `match.toml` | Does not exist. Step 3 is specified as config-driven, so it lands there |

### Suggested Insertion Points For `agent` And `objective`

Not applied; `AGENTS.md` is human domain. Both crates are in the layout but neither has
a number, and the ordering is forced by what depends on what.

`agent` belongs directly after `spawn`. A scripted baseline is specified as "drive to
nearest shape, shoot it", which needs shapes to exist first, and the lockstep driver in
step 5 blocks until every agent replies, so the `Policy` trait must precede it.

`objective` belongs before `server`. Scoring reads kills and shape destruction, both of
which `core` already emits, and a match cannot end without a win condition. `core`
exposes `World::add_score` and `Entity::value` as the hooks it writes through.

That gives eight steps rather than six:

| Step | Crate |
|---|---|
| 1 | `schema` |
| 2 | `core` |
| 3 | `spawn` |
| 4 | `agent` |
| 5 | `events` |
| 6 | `objective` |
| 7 | `server` |
| 8 | `client` |

The alternative is to leave the order at six and treat `agent` and `objective` as
sub-tasks of the steps that need them. That hides two trait designs inside other work,
which is how a trait gets designed to fit its first caller instead of its purpose.

### Findings Held For Steps 5 And 6

The viewer was scoped before being deferred. Recording the conclusions so they are not
re-derived.

**Transport.** Three options were weighed: a thin slice of the real `server` crate on
tokio and axum, a throwaway crate on synchronous tungstenite, and compiling `sim-core`
to WebAssembly and running the simulation in the browser tab. The first is preferred.
`AGENTS.md` already commits to tokio and axum, so it introduces no new dependency
decision, while the throwaway adds a crate outside the declared stack and is then
deleted. WebAssembly needs a `wasm32` target and `wasm-pack`, neither installed on this
machine, for a path the project has no other use for.

**Real-time mode is not needed to watch a match.** Scripted policies run in-process and
return immediately, so the lockstep driver can be paced by sleeping between ticks. That
yields a watchable stream and full determinism at once, and leaves the real-time display
mode deferred as `docs/DESIGN.md` intends rather than pre-empted.

**Viewer controls need a schema change.** `ClientMsg` carries `Command`, `WatchBelief`
and `Resync`. Pause, single-step and speed control have no variant, and the simulation
clock is server-side, so a viewer cannot pause locally without buffering — which is the
save-file behaviour ruled out. Adding a variant means bumping `PROTOCOL_VERSION`. Cheap
now, expensive once matches are recorded.

**Range overlays are the feature worth having.** Drawing each tank's sense radius at 120
units and comms radius at 200 makes the network partition visible as it forms and
dissolves. Without them the viewer shows a generic shooter; with them it shows the thing
under study. Roughly twenty lines, no backend involvement.

**Toolchain present.** Node 24.18.1, npm 11.16.0. `client/` holds only the generated
`schema.ts`, so `package.json`, a Vite config and an entry point are all still needed.

# AI Development Log

Written by the assistant, one entry per session. Everything else in `docs/` is human
domain and is not edited from a session.

This file records what was built, what was decided, and what was found. Where a
session concludes that a specification needs changing, the proposed change is written
here as a suggestion and the specification is left untouched.

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

- Step 3, `spawn`. Not started. `World::spawn_shape` and `World::entity_mut` are the
  hooks it needs, and both exist.
- Score accrual is deliberately absent. `World::add_score` and `Entity::value` are the
  hooks the `objective` crate writes through.
- The sensor sweep is empty, as `schema::sense` intends for v0. `Grid` is in place for
  the digital differential analyzer [DDA] traversal at v0.5.

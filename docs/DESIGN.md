# Design Specification

Full specification for the tank coordination sandbox. `CLAUDE.md` at the repo root
carries the working agreement and build order. This document carries the reasoning.

Numbers given here are starting points, not settled values. Everything in the
Configuration section is meant to be swept.

---

## Purpose

Build a sandbox for studying multi-agent coordination under communication constraints.

Ten tanks, two teams of five, plus one static control center [CC] per team. Every
participant has a limited sensing range and a limited signal range. Teams race to a
point total. The tanks and the CC must work out a strategy together, over a link that
does not always exist.

The sandbox must be flexible enough that different people can research different
approaches. That means the interesting parts — world model, policy, message codec,
channel physics, objective — are all traits with swappable implementations, and the
simulation core depends only on the traits.

---

## Objective

First team to the point target wins. Points come from two sources:

- Destroying shapes. Value scales with shape tier.
- Killing enemy tanks. Value scales with the victim's accumulated score.

Starting target: 10,000 points.

The alternative — elimination with a shared respawn budget — is sharper to watch and
worse for gathering statistics, because it compresses a match into two or three
decisive fights. Keep it as a second `Objective` implementation, not the default.

We time limit the match to 90 minutes. 
The match can end in three states for the team: Win, Draw, Loss.

| Result | Definition |
|---|---|
| Win | Reach 10,000 points (or other winning criteria) |
| Draw | Time reaches 90 minutes without either Win or Loss |
| Loss | Other team has scored the Win condition |

---

## Arena

A unit is an abstract definition of size. To scale, we scale along the unit.
As an example, set 1 unit = 20px, and the scale accordingly.

A square, 1000 units on a side. Origin at the top-left corner.

*Shapes* spawn within the arena anywhere per specific time delay `shape_spawn_tick`.

**Bases.** Two squares, 200 units on a side, at opposite vertices. Team A occupies the
northwest corner, Team B the southeast. This is the original diep.io Team Deathmatch
layout.

A base is a sanctuary. Enemy tanks cannot enter it and enemy fire is destroyed at its
boundary. This removes spawn camping and gives a losing team somewhere to regroup, so
one bad engagement does not decide the match.

**Nest.** A disc of radius 150 at the arena centre. The only place pentagons and alpha
pentagons spawn. Because the richest resource sits equidistant from both bases, farming
and fighting become the same decision.

The nest is not a clearing. Squares and triangles fall inside the disc like anywhere
else, because the scatter focus excludes nothing, and the nest piles its own shapes on
top. The centre is therefore the densest ground on the map as well as the richest,
which makes crossing it a navigation problem rather than a straight line.

**Wings.** Optional secondary farming areas at the midpoints of the northeast and
southwest edges. Off by default. They exist so a researcher can add a second and third
contested area and watch how team allocation changes.

**Ranges.**

| Quantity | Starting value |
|---|---|
| Tank sense radius | 120 |
| Tank comms radius | 200 |
| CC comms radius | 350 |
| Base-to-base diagonal | ~1100 |

Two relationships matter more than the absolute numbers.

Comms radius exceeds sense radius. This is the reason communication has value at all —
a tank can tell a teammate about something the teammate cannot see. If comms were
shorter than sensing, information would be trapped where it was gathered.

Comms radius falls far short of the arena. Five tanks at 200 units of reach cannot
span an 1100-unit diagonal. Spread out and the network partitions. Clump up and the
team goes blind. That tension is the sandbox.

---

## Entities

**Tank.** A circle. Has position, velocity, heading, health, team, class, stat
allocation, accumulated score, and a sensor. Controlled by a policy. Respawns in its
base after a delay.

**Shape.** A polygon that does not shoot back, though it deals contact damage. Four
tiers, ordered:

- Square. Scattered across the arena. Low health, low value.
- Triangle. Scattered alongside squares. Roughly twice the health and two and a half
  times the value, so it is a marginal improvement, not a reason to travel.
- Pentagon. Nest only. High health, high value. Cannot be farmed quickly by one tank.
- Alpha pentagon. Nest only, and barely larger than a pentagon. Worth six of them. A
  team holding the nest and nothing else out-earns a team farming both wings
  unopposed, which is what this tier exists to arrange.

Squares and triangles drift and then come to rest. Drag stops a wandering shape, and a
collision moves it a couple of units before it settles again. The alternative, a
restoring pull toward the spawn point, was rejected: shapes being dragged home is a
mechanic nobody can see. Pentagons and alpha pentagons do not drift at all. A prize
that wanders is not a place worth contesting, and drifting nest shapes escape into the
arena where nothing caps them.

The consequence is that belief about shape positions stops decaying once the arena
settles. Belief about tanks still decays, because tanks move constantly, so the
staleness pressure survives through them rather than through terrain.

**Bullet.** A circle with a velocity, a lifetime, a damage value, and an owner. Expires
on timeout or contact.

**Control center.** A static entity at the centre of its team's base. No collision, no
health, cannot be destroyed. Detailed below.

All entities carry a handle: an identifier paired with a generation counter, so a
recycled slot never aliases a dead entity. Use a slotmap.

---

## Progression

There are forty-five classes in diep.io which form an interesting problem of selecting the right team via the Command Center.

However, for the current v0 we need just a simple tank with eight stats.
Every tank shares the same sense radius, field of view, and base stats; class is not chosen and not used. The sequencing argument is that two coordination pressures — range-limited communication and role differentiation — running at once makes neither measurable. The Class enum stays in the schema and stays None through v0, so introducing it later costs no migration.

Caveat to the above statement: the communication radius is still greater than the sense and field of view.

**Eight stats**: Health Regen, Max Health, Body Damage, Bullet Speed, Bullet Penetration, Bullet Damage, Reload, Movement Speed.

- 1 Skill Point (SP) per level up to Level 28.
- 1 SP at Level 30.
- 1 SP every 3 levels from Level 30 to Level 45.
- Maximum SP: 33.
- Each stat caps at 7 upgrades

These Eight stats are to be selected by the policy and strategy from command center after the mechanism is built.

**Tank types** - There are many tank types, but the focus is on 4 after v1.

### Tier 2 Tanks:

| Type | Description |
|---|---|
| Twin | Grants an extra frontal cannon while bullet power is slightly reduced.|
| Sniper | Grants a broader Field of View. Reload speed is low, movement is slowed.|
| Machine Gun | Sprays bullets from a trapezoid up front. Double the rate of a basic tank. Less accurate but covers larger area|
| Flank Guard | Adds a Cannon to the rear. Allows tank to shoot in both directions. Because both cannons have same recoil, Flank Guard has zero recoil|

**No bosses, no crashers, no drone-type projectiles.** Add them only if the sandbox
gets stale.

---

## Coordination Pressures

Each mechanic below exists because it makes solo play lose. The objective is to study these tactics play out against each other.

**Focus fire.** Tanks regenerate health a few seconds after last taking damage. One
tank alone cannot out-damage a healthy enemy's regeneration (subject to stats). Two can. Killing therefore
requires two tanks agreeing on a target inside a window, and neither can see the
other's aim. Agreement requires a message.

**Irreversible class commitment.** Five scouts lose. Five snipers may lose. The team
must divide roles before anyone knows what the enemy picked. This is a commitment game
played over a lossy link.

Irreversible class commitment is not active in v0. Check Progression heading. The mechanic is still wanted; it is deferred, not dropped.

**Contested centre.** High-tier shapes exist only in the nest. Someone farms, someone
screens the farmer, someone watches the flanks. Nobody does two of those.

**Sanctuary bases.** Losing teams can regroup, so matches produce data rather than
early routs.

**Relay topology.** A tank positioned between two clusters carries messages while
contributing nothing to any fight. Whether an agent ever chooses to be that tank is a
measurable question, and the answer is the point of the experiment.

---

## Sensing

Each tank carries a planar scanning sensor — a two-dimensional LIDAR [Light Detection
and Ranging] analogue.

**Geometry.** Cast N rays over 360 degrees from the tank's position. Each ray
terminates at the first intersection with world geometry. Ray-circle is a quadratic;
ray-polygon is a cross-product test. Both are closed form.

Broadphase matters more than the intersection math. Walk each ray through a uniform
grid using digital differential analyzer [DDA] traversal and stop at the first hit.
This tests a handful of candidates per ray instead of every entity.

Starting values: 360 rays, 10 Hz.

**Error model.** A perfect raycast teaches nothing a distance function would not.
In rough order of value:

- Range noise. Gaussian, standard deviation growing with distance.
- Dropouts. No return at grazing incidence, rising as the angle between beam and
  surface normal approaches 90 degrees. Also rising near maximum range.
- Intensity. Proportional to surface albedo times cosine of incidence angle, divided
  by range squared.
- Motion distortion. The sensor sweeps while the tank moves. Each ray is cast from the
  pose held at that instant, not the pose at sweep end. Deskewing this is a real
  algorithm an agent will have to implement.
- Mixed pixels. A beam straddling an object edge returns an intermediate range. Cheap
  to fake, and it breaks naive clustering the way real data does.

**Rates.** Decouple every loop. Physics 25 Hz. Sensor 10 Hz. Comms 5 Hz. Decision 5 Hz.

Ten tanks at 360 rays and 10 Hz is 36,000 rays per second. Free on a CPU with rayon.
The mismatch between a 25 Hz world and a 10 Hz sensor is itself instructive: the world
moves between scans and the agent must live with that.

Have Physics, Sensors, Comms, and Decision have individual multipliers to these constants within a config file that can change be changed at whim, providing richer simulations scenarios. 

**Frames.** Store a scan in polar form in the sensor frame, with the pose stamp
attached. Convert to Cartesian world coordinates only when needed. Keeping the world,
body, and sensor frames explicit from day one saves a month later.

---

## Belief

An agent never sees world state. It holds a belief, and the gap between belief and
truth is the thing being studied.

A belief contains:

- **Self pose.** Exact.
- **Teammate poses.** Timestamped, received over comms, stale by the link latency.
- **Enemy tracks.** Each with a last-known position, a velocity estimate, a last-seen
  tick, and an uncertainty radius growing as maximum enemy speed times age.
- **Occupancy and exploration state.** Where the tank has looked and when.
- **Shape map.** Semi-static. Decays slowly. Worth sharing once, rarely updated.

The uncertainty radius is not optional decoration. A track four seconds old with a
200-unit uncertainty disc is a hypothesis, not a target. Whether an agent acts on it,
ignores it, or spends a tank going to re-confirm it is the interesting behaviour, and
it only exists because staleness was modelled. Build it on day one.

---

## World Model

The representation is pluggable. A quadtree, a dense occupancy grid, a landmark graph,
and a particle set must all satisfy one trait.

```rust
pub trait WorldModel: Send {
    fn ingest_scan(&mut self, scan: &Scan);
    fn ingest_belief(&mut self, msg: &BeliefMsg, from: AgentId);
    fn tick(&mut self, now: Tick);

    fn occupancy(&self, p: Vec2) -> CellState;
    fn confidence(&self, p: Vec2) -> f32;
    fn frontiers(&self, near: Vec2, k: usize) -> Vec<Region>;
    fn tracks_near(&self, p: Vec2, r: f32) -> Vec<Track>;

    fn encode(&self, budget: usize, interest: Interest) -> BeliefMsg;
    fn footprint(&self) -> usize;
}
```

Two methods carry the weight.

`encode` takes a byte budget and a hint about what the receiver cares about, and
returns the most useful message that fits. This is where a world model earns or loses
its keep.

`footprint` reports memory used, so the runtime can enforce the per-tank ceiling.

`ingest_scan` and `ingest_belief` are separate on purpose. A scan is ground truth from
your own sensor. A belief is hearsay with an age and a source. A model that cannot tell
them apart will let teammates echo stale information back and forth and inflate each
other's confidence — rumor propagation. The sandbox should be able to exhibit that
failure, which means the interface must permit avoiding it.

### Quadtree, First Implementation

Chosen for two reasons. The stated one is compute: a quadtree bounds memory by depth
cap, gives logarithmic point queries, and stores an empty region in one node instead of
ten thousand cells.

The better reason is communication. A quadtree truncates gracefully. Cut the tree at
depth three and you have a coarse but valid map of the whole arena. Cut at depth six
and you have a detailed one. Same structure, same decoder, resolution traded for bytes
on a smooth curve. Depth is a bandwidth dial.

Pruning need not be uniform. Keep depth six near the region the receiver cares about
and depth two elsewhere. That is what the `Interest` hint in `encode` is for.

Merging is equally clean. Walk both trees from the root. Where one has a leaf and the
other a subtree, the fresher timestamp wins or the subtree is kept, by policy.
Recursive, allocation-light, terminates at the depth cap.

Store per node: state (unknown, free, occupied), a last-updated tick, and a confidence
scalar. Aggregate upward so a parent's timestamp is the oldest of its children. Then
"what do I not know recently enough" is answerable without descending, which is the
query a scouting policy asks constantly.

### What The Quadtree Does Not Hold

Moving entities. Forcing tanks into cells wastes the whole budget re-subdividing cells
a tank left two ticks ago.

Split the model. The quadtree holds occupancy, terrain, shape fields, and exploration
state. A separate flat track list holds moving entities. Index the track list by
quadtree cell so spatial queries stay cheap, but do not store tracks in the tree.

Both live behind the one `WorldModel` trait, so an implementer who wants a unified
representation is still free to build one.

---

## Communications

Every message routes through a broker. No agent ever calls a method on another agent.
The broker is the only place the link rules live.

```rust
pub trait Channel: Send {
    fn submit(&mut self, from: AgentId, to: Recipient, payload: Bytes, now: Tick);
    fn deliver(&mut self, now: Tick) -> Vec<Delivery>;
}
```

The broker enforces:

- **Range.** Euclidean distance between sender and receiver at submit time.
- **Latency.** A delivery queue. Distribution configurable.
- **Loss.** Per-message drop probability.
- **Bandwidth.** A per-link byte budget per second. Overflow is dropped or queued.

The bandwidth cap is the interesting constraint, not the range limit. A raw scan is
thousands of points. It cannot be sent. Cap the link at a few kilobytes per second and
an agent must decide what is worth transmitting: an occupancy submap, a set of detected
centroids, a pose graph edge, or a statement of intent. That decision is the actual
research question in multi-robot mapping.

The comms graph is dynamic. Log its connectivity every tick. Partition events are data.

---

## Control Center

One per team, static, at the centre of its base. Three properties, and nothing else:

**No sensing.** It knows only what tanks tell it. A CC that sees the map is a cheat and
invalidates every result.

**More compute and memory.** An order of magnitude above a tank. It can hold the
full-depth world model, run a real planner, and afford an optimizer a tank cannot. This
asymmetry is its entire reason to exist.

**A larger but finite comms radius**, anchored at the base. Tanks near home sync
cheaply. Tanks at the nest are on their own, or must relay through a teammate.

That geometry turns centralization into a dial rather than an assumption. Set the CC
radius to cover the arena and the team is centrally planned. Shrink it to the base
square and you get edge autonomy with occasional check-ins. Every value between is a
research condition.

The question this makes measurable: is a tank that drives home to report doing
something useful?

---

## Commands

A human operating the CC and an automated CC policy emit the same type. This makes
human-in-the-loop free, and lets a researcher swap a person for an algorithm and
compare directly.

```rust
pub enum Command {
    SetObjective(ObjectiveHint),
    AssignRole { agent: AgentId, role: Role },
    DesignateTarget { agent: AgentId, target: EntityId },
    SetRally { agent: AgentId, pos: Vec2 },
    RequestReport { agent: AgentId },
}

pub enum Origin { Human, Policy }
```

Commands arrive asynchronously and land in a buffer. The buffer drains at the top of a
tick, before physics, so a command issued during tick N takes effect on tick N and
never lands mid-step. Each drained command is stamped with the tick it was drained on
and written to the event log, so replays include the human's decisions.

A command is subject to the same comms rules as any other message. A human at the CC
cannot reach a tank the CC cannot reach.

---

## Spawner

A spawner is a composition of weighted foci, not a monolith.

```rust
pub trait Spawner: Send {
    fn tick(&mut self, world: &World, now: Tick, rng: &mut Rng) -> Vec<SpawnRequest>;
}

pub struct Focus {
    pub id: FocusId,
    pub region: Region,                     // Disc, Rect, Annulus, WholeArena
    pub exclude: Vec<Region>,               // off limits inside region. Placement only
    pub tier_weights: Vec<(ShapeTier, f32)>,
    pub num_slow: usize,                    // alive count where the rate starts falling
    pub num_max: usize,                     // alive count where respawning stops
    pub count_by: CountBy,                  // Origin: made here. Resident: standing here
    pub respawn_per_sec: f32,
    pub prefill: usize,                     // placed at match start, before tick one
    pub drift: DriftModel,
    pub enabled: bool,
}
```

A focus has a population band rather than a capacity. Full rate at or below
`num_slow`, nothing at or above `num_max`, a straight line between them. A filling
area becomes a diminishing one before it becomes a closed one.

`CountBy` is the field that changes a design claim rather than adding a knob. A focus
maintains either a population or a place: `Origin` counts the shapes it made wherever
they have drifted to, `Resident` counts the shapes standing in its region whoever made
them. The nest only works as a place. A pentagon that leaves the disc stops counting
and is replaced, so the centre stays packed instead of bleeding its shapes into the
arena.

`prefill` exists because an arena that starts empty and fills over three minutes is a
different experiment from one that starts populated, and the populated one is the
intended baseline.

The composite spawner holds a `Vec<Focus>` and is driven entirely from config.

- Uniform scatter is one focus covering the arena, squares and triangles, a wide band,
  excluding nothing.
- The nest is one disc focus at the centre, pentagons at five times the baseline rate
  with a minority of common shapes, counted by residency, no drift.
- Alpha pentagons are a second focus sharing the nest disc. Separate because one focus
  carries one population band, and four alphas against forty-five pentagons at a
  twentieth of the rate is a different band. It also makes "alphas spawn only at the
  nest" structural rather than a weight somebody can edit. Placed in the inner third of
  the disc, counted across the whole of it, which is what `exclude` applying to
  placement alone is for.
- Wings are two more foci at the northeast and southwest edge midpoints. Disabled by
  default.

Every spawned entity records its originating `FocusId` in the event log. That is what
makes "which farming area did the team prioritise" a query rather than a guess.

Respawn rates should be slow enough that the resource is genuinely finite in the short
run. If shapes respawn as fast as they are farmed, there is nothing to contest.

---

## Agents

Two deployment modes behind one interface.

```rust
pub trait Policy: Send {
    fn decide(&mut self, obs: &Observation, now: Tick) -> Action;
}
```

**In-process.** A Rust implementation. Fast. Used for scripted baselines and for
sweeps that need to run far faster than real time.

**Out-of-process.** A socket bridge implementing the same trait, forwarding to an
external process. This is what buys the flexibility: a policy can be written in Python,
which is where anyone doing learned control will want to be.

The simulation core never knows which it is talking to.

### Clock

Two modes, same protocol.

**Lockstep.** The server sends observations, blocks until every agent replies or a
per-agent timeout expires, then advances. Reproducible from a seed. Runs faster than
real time when nobody is watching. **This is the default.**

**Real-time.** The server ticks on a wall clock; agents that miss the deadline repeat
their last action. Smooth to watch, non-reproducible. A display mode, added later.

Experiments need determinism more than they need sixty frames per second. The viewer
can interpolate between ticks for smoothness.

### Budgets

The tanks are meant to have very little compute. Enforce that in the runtime rather
than trusting policy authors to be frugal.

**Memory**, via `WorldModel::footprint()`. A fixed ceiling per tank — start at 16 KB.
Exceed it and the model must prune, which forces a real decision about what to forget.

**Time**, a fixed budget per decision tick. Overrun and the tank repeats its last
action, and a `BudgetExceeded` event is emitted. This is what makes the choice between
a quadtree and a dense grid actually matter.

The CC gets budgets an order of magnitude larger.

---

## Events

One enum. Every sink consumes it. The event bus is the only source of truth — wire
frames, the replay file, and the database are all derived from the same stream. If the
viewer and the database can disagree, someone will eventually chase a bug that does not
exist.

```rust
pub enum Event {
    MatchStart { seed: u64, config_hash: u64 },
    TickBegin { tick: u32 },
    Spawned { id: EntityId, kind: Kind, pos: Vec2, team: Option<TeamId>, focus: Option<FocusId> },
    Despawned { id: EntityId, cause: Cause },
    Damaged { target: EntityId, source: EntityId, amount: f32, remaining: f32 },
    Killed { target: EntityId, killer: EntityId },
    Scored { team: TeamId, delta: u32, total: u32, reason: ScoreReason },
    CommandIssued { team: TeamId, origin: Origin, cmd: Command },
    ActionSubmitted { agent: AgentId, action: Action, latency_us: u32 },
    MatchEnd { winner: Option<TeamId>, tick: u32 },

    // Reserved. Not emitted in v0. Defined now so the schema does not move later.
    MessageSent { from: AgentId, to: Recipient, bytes: u32, tick: u32 },
    MessageDelivered { from: AgentId, to: AgentId, bytes: u32, latency_ticks: u32 },
    MessageDropped { from: AgentId, to: Recipient, reason: DropReason },
    BudgetExceeded { agent: AgentId, kind: BudgetKind, overage: u64 },
}
```

Adding enum variants later is cheap. Migrating a database of a hundred recorded matches
is not. Reserve them now.

```rust
pub struct TickRecord<'a> {
    pub tick: Tick,
    pub events: &'a [Event],
    /// What the agents decided this tick. The replay payload.
    pub inputs: &'a Inputs,
    pub scores: [u32; 2],
}

pub trait Sink: Send {
    fn accept(&mut self, rec: &TickRecord<'_>);
    /// Commit whatever is buffered. Must be safe to call more than once.
    fn flush(&mut self);
}
```

A record rather than a slice of events. A replay is made of the inputs agents
returned, and a replay of events alone reproduces nothing, so the bus carries events,
inputs and scores together and each sink takes what it needs.

Filtering is not divergence. The bus carries every variant to every sink; a sink
declining to store what another already holds is the reason for having more than one.
The database skips `ActionSubmitted` and `TickBegin` on exactly that ground — the
replay file already holds both, ten times more compactly.

Sinks: SQLite, replay file, WebSocket fan-out, live metrics.

### Volume

Position updates are not events in this sense. Ten tanks plus two hundred shapes plus
bullets at 25 Hz is roughly ten thousand rows per second. Putting that through the
discrete event table makes the table useless for analysis.

Split them. Discrete events go to `events`.

---

## Storage

SQLite, one file per match, write-ahead logging on, one transaction per tick. No server
to run, the file ships alongside the replay, and it opens directly in pandas.

```sql
CREATE TABLE match(
  id TEXT PRIMARY KEY,
  seed INTEGER,
  config TEXT,
  started_at TEXT,
  ended_at TEXT,
  winner INTEGER
);

CREATE TABLE events(
  tick INTEGER,
  seq INTEGER,
  kind TEXT,
  payload TEXT,
  PRIMARY KEY(tick, seq)
);

CREATE TABLE scores(tick INTEGER, team INTEGER, total INTEGER);

CREATE INDEX idx_events_kind ON events(kind);
```

There is no `kinematics` table. Trajectories were the largest thing a match produced by
an order of magnitude, and they are pure derived data: re-simulating the replay rebuilds
them at any rate and any filter. The `Kinematic` type and the recording path stay in
`core`, gated off behind `WorldSpec::record_kinematics`, for a live-metrics sink or a
materialise command later.

A match therefore writes two artefacts, split by whether losing them to a code change
matters. The replay file holds the seed, the config hash and the per-tick inputs, and
stops reproducing when the physics changes. The database holds the discrete events —
spawns, damage, kills, scores — which are the analytical facts and must outlive a code
change. Ninety minutes is roughly 25 MB and 30 MB respectively.

`payload` as JSON is right while the event shapes are still moving, and SQLite queries
JSON natively. Normalize into typed columns once they settle. Export to Parquet when
sweeps get large.

---

## Wire Protocol

WebSocket, newline-delimited JSON. Binary later, and later than you expect.

**Server to client.** An initial `Snapshot`, then per-tick `Delta`.

The delta scheme comes from diep.io: organize each entity's properties into field
groups — position, physics, health, team, score, style, relationships — and send only
the groups that changed. It is a solved answer to the problem and it is worth copying.

**Client to server.** `Command` messages only. The viewer has no other authority.

**Belief overlay.** The frame carries an optional belief payload keyed by agent
identifier. Build this into the protocol from the start. Watching a tank's belief
diverge from truth is the entire reason the sandbox exists, and retrofitting it means
changing the schema after clients depend on it.

Types are defined once in the `schema` crate and the TypeScript is generated from the
Rust with `ts-rs`. Check the generated file in and verify it in CI, so a drifted type
fails the build rather than the runtime.

---

## Viewer

The client is a viewer, not a player. No human plays. That removes client-side
prediction, input buffering, server reconciliation, and lag compensation — all the hard
parts of netcode. The stream is one-way and the client draws frames.

Two view modes:

- **Truth.** The world as it is.
- **Belief.** One selected tank's occupancy map, its enemy tracks with uncertainty
  discs, its comms links, and what it has never seen.

The viewer must decode the replay file with the same decoder it uses for the live
socket. Then a researcher hands someone a file and they watch the match in a browser by resimulating the match from the save file (because the save file has all the events saved).
That is how results get shared.

The only upward channel is `Command`, used when a human is operating a CC.

---

## Configuration

Everything below is a config file a researcher edits, not code they patch.

| Group | Keys |
|---|---|
| Arena | side length, base size, base corners, nest radius, wing foci on/off |
| Sensing | rays, rate, range per class, noise model parameters |
| Comms | tank radius, CC radius, bandwidth cap, latency distribution, drop rate |
| Budgets | tank memory ceiling, tank time budget, CC multipliers |
| World model | implementation name, quadtree depth cap |
| Spawn | foci list with region, tier weights, capacity, respawn rate |
| Objective | implementation name, point target, scoring weights |
| Teams | size, class composition, policy assignment per agent |
| Run | seed, clock mode, tick rates, output directory |

---

## Metrics

The sandbox is only worth building if it produces numbers. Derive these from the event
log after a match rather than computing them live.

- **Team score over time.**
- **Mean belief error.** Distance between where a team thought each enemy was and where
  it actually was, averaged over ticks. The direct measure of whether communication
  worked.
- **Network connectivity.** Fraction of ticks the comms graph was fully connected, and
  largest partition size.
- **Bandwidth utilisation** against the cap.
- **Focus-fire concentration.** How often two or more tanks damaged the same target
  inside a window.
- **Role entropy.** How differentiated the team's behaviour was.
- **Exploration coverage.** Fraction of arena with belief fresher than N ticks.

### Experiments

The runs that justify the build:

- Sweep tank comms radius from zero to infinite.
- Sweep CC comms radius from base-only to full-map. This is the centralization axis.
- Sweep bandwidth cap from one byte to unlimited.
- Ablate to full observability as an upper bound.
- Vary team class composition against a fixed opponent.

The gap between those curves is the value of coordination, measured. That is what the
sandbox is for.

---

## Reference Material

`github.com/abcxff/diepindepth` — reverse-engineering dossier on diep.io. Reference,
not spine.

Take:

- The field-group pattern. Solved delta scheme.
- The `<id, hash>` handle. Generation counters against stale references.
- Physics constants: entity radii, tank movement and recoil, shape health and value,
  bullet speed and lifetime.

Ignore the WebAssembly reversal, memory layouts, and packet obfuscation. They exist to
talk to a client we are not using.

`github.com/abcxff/diepcustom` — AGPL-3.0 [Affero General Public License]. Do not copy
code. Read `src/Const/` for constants only.

Also examined and rejected as a base: `Altanis/polyfight` (no license file),
`gaccob/diep.io-clone` (useful `proto/` layout as one worked example),
`dmitmel/tienk.io` (archived, Unity).

Every one of these is optimized for fidelity to diep.io. Fidelity is what we are
discarding. None has a seam where AI tanks, pluggable world models, range-limited
comms, or a control center would go.

**Outstanding: the constants extraction pass.**  A provisional set lives in `crates/core/src/constants.rs`, with values traceable to STATS.md marked STATS and the rest marked PROVISIONAL alongside the reasoning that produced them. The reference pass against `diepcustom/src/Const/` remains undone.

---

## Open Questions

**Belief message type.** Is `BeliefMsg` a common union type that every world model
serializes into, or opaque bytes only a matching model can decode? Opaque is simpler
and lets a researcher invent any representation, but then two teams running different
world models cannot talk — and mixed-model teams are one of the more interesting
experiments available. Leaning toward a common type with a small set of standard
payloads (map patch, track set, intent) plus a raw variant for anything exotic.

**Viewer frames.** Full world state per tick, or snapshot plus deltas? Full state is
trivial and fine at ten tanks. Deltas are needed once replay files get long. Leave room
in the schema either way.

**Respawn.** Fixed delay, or a cost paid in team score? A cost makes death a resource
decision rather than an inconvenience, which is more interesting — but it interacts
with the point target in ways that need thought.

**Protocol version on additive changes.** `ReplayLine` gained `Inputs` and `Repeat`
without moving `PROTOCOL_VERSION`, on the grounds that a decoder written against the
old schema still reads everything it knew. Once matches are being shared, a version
that does not move is a version that cannot tell two files apart. Bump on every wire
change, or only on breaking ones?

**Class choice timing.** At a level threshold, as in diep.io, or committed before the
match starts? Pre-match commitment is a cleaner coordination problem. In-match choice
lets a team adapt to what the enemy picked, which is a different and also interesting
problem.

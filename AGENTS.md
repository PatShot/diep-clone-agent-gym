# Tank Coordination Sandbox

A diep.io-inspired 2D arena used as a research sandbox for multi-agent coordination
under communication constraints. Two teams of five AI-controlled tanks plus a static
control center [CC] per team race to a point total. Every participant has a limited
signal range. The research question is how teams coordinate when nobody sees the whole
map and nobody can talk to everybody.

This is not a game. The game is the substrate. The deliverable is a sandbox other
people can run experiments in.

## Working Agreement

- Discuss and align on a plan before writing code or producing files. Always.
- Ask before scaffolding a new crate or adding a dependency.
- Build in the order below. Do not skip ahead to the interesting parts.
- One runnable milestone at a time. Each step must execute before the next begins.
- The Agent is instructed not to edit the AGENTS.md and within `./docs/` but instead keep a log at `docs/DEV_LOG_AI.md`.
- However, if the user asks - change simple substitutions etc for the user within these docs. Always confirm with user before making changes. 

## Stack

- Backend: Rust. Cargo workspace. tokio and axum for the server.
- Frontend: TypeScript. Vite. Canvas 2D. No framework.
- Storage: SQLite, one file per match, write-ahead logging on.
- Transport: WebSocket, newline-delimited JSON for now. Binary later, not yet.
- Types cross the wire once. Rust is the source; TypeScript is generated from it.

## Layout

```
crates/
  schema/      shared types, Event enum, TypeScript generation. Depends on nothing.
  core/        World, Tick, entities, fixed-step physics, collision
  spawn/       Spawner trait, Focus, composite implementation
  objective/   Objective trait, PointTarget implementation
  events/      event bus, Sink trait, SqliteSink, FileSink, WsSink
  server/      lockstep driver, WebSocket, snapshot and delta encoding
  agent/       Policy trait, scripted baselines, socket bridge
client/        Vite + TypeScript viewer
config/        arena.toml, spawn.toml, match.toml
docs/DESIGN.md full specification
```

Everything depends on `schema`. `schema` depends on nothing. That is what keeps the
wire contract honest.

## Milestones
We aim for 10 iterations via v0.0 -> v0.9
1. `v0` - Basic working schema.
2. `v0.5` - World Design.
3. `v0.8` - AI Design
3. `v1` - User Input and Completion. 

## v0 Build Order

Current position: step 2 complete. Step 3 not started.

1. `schema` — Event enum, entity types, wire messages, TypeScript generation working.
2. `core` — entities, fixed-step movement, circle collision, arena bounds. Headless. Bullet lifetime, contact damage, regeneration, and death were included by decision.
3. `spawn` — uniform focus first, then nest focus, then wing foci. Config-driven.
4. `events` — bus, file sink, SQLite sink. A match must write a queryable database.
5. `server` — lockstep driver, WebSocket, snapshot and delta encoding, command intake.
6. `client` — canvas render of arena, tanks, shapes, scores. Then replay file loading.

Scripted policies stay trivial throughout: drive to nearest shape, shoot it. They
exist to make the viewer show something, not to be good.

## Invariants

Breaking a rule here invalidates results.

- An agent never touches world state. It receives an observation and returns an action.
- The event bus is the only source of truth. Wire frames, the replay file, and the
  database are all derived from the same event stream.
- Lockstep first. The server blocks until every agent replies or times out, then
  advances. Real-time is a display mode added later.
- Determinism: fixed timestep, seeded per-agent RNG, integer tick counter, no
  wall-clock reads in simulation code.
- Commands drain at the top of the tick, before physics. Never mid-step.
- The browser client is a viewer, not a player. Its only upward channel is `Command`.
- A human operating the CC and a CC policy emit the same `Command` type.
- Discrete events and kinematics go to separate tables. Position updates at 25 Hz
  across 200+ entities will drown the event table otherwise.

## Deferred

Not in v0. Do not build these yet, but do not design them out either.

- Comms model (range, latency, bandwidth cap, drop rate)
- World model (quadtree occupancy plus a flat track list, behind a `WorldModel` trait)
- Per-agent compute and memory budgets enforced by the runtime
- CC behaviour beyond a static marker on the map

Reserve the `MessageSent`, `MessageDelivered`, `MessageDropped`, and `BudgetExceeded`
variants in the `Event` enum now. Adding enum variants later is cheap. Migrating a
database of recorded matches is not.

## Reference Material

`github.com/abcxff/diepindepth` is the reference, not the spine. Take:

- The field-group pattern (position, physics, health, team, score, style). Send only
  dirty groups per entity. This is the delta scheme.
- The `<id, hash>` handle: entity id paired with a generation counter so a recycled
  slot never aliases a dead entity. Use a slotmap crate.
- Physics constants: entity radii, tank movement and recoil, shape health and value,
  bullet speed and lifetime. Extract into one constants file.

Ignore the WebAssembly reversal, memory layouts, and packet obfuscation. Those exist
to talk to a client we are not using.

`github.com/abcxff/diepcustom` is AGPL-3.0 [Affero General Public License]. Do not
copy code from it. Read `src/Const/` for constants only.

**Outstanding task: the constants extraction pass.** A provisional set lives in crates/core/src/constants.rs, with values traceable to STATS.md marked STATS and the rest marked PROVISIONAL alongside the reasoning that produced them. The reference pass against diepcustom/src/Const/ remains undone.

## Open Questions

Settle these before the code that depends on them.

- Is `BeliefMsg` a common union type every world model serializes into, or opaque
  bytes only a matching model decodes? Leaning: common type with a raw escape hatch,
  so mixed-model teams remain a possible experiment.
- Does the viewer get full world state per tick, or a snapshot plus deltas? Full state
  is fine at ten tanks. Leave room in the schema for deltas.

## Style

- Prose in docs and comments: short declarative sentences, concrete words, active
  voice. Minimal hedging.
- Headings are noun phrases, not questions or clauses.
- Expand every abbreviation in brackets on first use.
- Format curl commands as single lines. No backslash continuations.

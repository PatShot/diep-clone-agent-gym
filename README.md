# Tank Coordination Sandbox

A two-dimensional arena for studying multi-agent coordination under communication
constraints. Two teams of five tanks plus one static control center [CC] per team race
to a point total. Every participant has a limited sensing range and a limited signal
range. Nobody sees the whole map and nobody can talk to everybody.

This is not a game. The game is the substrate. The deliverable is a sandbox other
people can run experiments in.

`AGENTS.md` carries the working agreement and build order. `docs/DESIGN.md` carries the
full specification and the reasoning.

## Status

v0, step 1 of 6. The `schema` crate compiles and generates TypeScript.

| Step | Crate | State |
|---|---|---|
| 1 | `schema` | done |
| 2 | `core` | not started |
| 3 | `spawn` | not started |
| 4 | `events` | not started |
| 5 | `server` | not started |
| 6 | `client` | not started |

## Layout

```
crates/schema/       shared types, Event enum, TypeScript generation
client/src/gen/      generated TypeScript, checked in
scripts/             drift check
config/              arena.toml, spawn.toml, match.toml
docs/DESIGN.md       full specification
```

Everything depends on `schema`. `schema` depends on nothing structural. That asymmetry
keeps the wire contract honest.

## Build

```
cargo build
cargo test
```

`cargo test -p schema` regenerates `client/src/gen/schema.ts` as a side effect. The file
is checked in. `scripts/check-schema-drift.sh` fails when it falls out of date, so a
changed Rust type breaks the build rather than the runtime.

## Wire Format

WebSocket carrying newline-delimited JSON [NDJSON]. One message per line. Binary later.

Two representation choices are worth knowing before reading the types.

An `EntityId` travels as `[index, generation]`, a two element array. The generation
counter stops a recycled slot from aliasing a dead entity. The packed 64-bit form,
`EntityId::to_u64`, is for database keys and the eventual binary protocol; it stays off
the JSON wire because a JavaScript number cannot hold it exactly.

A seed and a configuration hash travel as decimal strings for the same reason. They are
provenance, and provenance that silently loses its low bits is worse than useless.

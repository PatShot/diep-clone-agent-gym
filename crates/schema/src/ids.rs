//! Identifiers and the entity handle.
//!
//! Every identifier is a newtype over an integer. They are distinct types so the
//! compiler rejects passing an agent identifier where an entity identifier belongs.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Integer tick counter. The simulation clock. Never a wall-clock reading.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[ts(export, export_to = "schema.ts")]
pub struct Tick(pub u32);

impl Tick {
    pub const ZERO: Tick = Tick(0);

    /// Ticks elapsed since `earlier`. Saturates at zero rather than wrapping.
    pub fn age_since(self, earlier: Tick) -> u32 {
        self.0.saturating_sub(earlier.0)
    }

    pub fn next(self) -> Tick {
        Tick(self.0 + 1)
    }
}

/// Team identifier. Team A is 0 and holds the northwest base. Team B is 1 and
/// holds the southeast base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct TeamId(pub u8);

impl TeamId {
    pub const A: TeamId = TeamId(0);
    pub const B: TeamId = TeamId(1);

    pub fn other(self) -> TeamId {
        TeamId(1 - self.0)
    }
}

/// Identifies a participant that runs a policy: a tank or a control center [CC].
/// Stable for the whole match. Survives the death and respawn of the tank it drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct AgentId(pub u32);

/// Identifies a spawn focus. Recorded on every spawned entity so a query can
/// attribute farming activity to a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct FocusId(pub u32);

/// Handle to an entity: a slot index paired with a generation counter.
///
/// The generation counter is what stops a recycled slot from aliasing a dead
/// entity. A stale handle held across a despawn compares unequal to the handle of
/// whatever now occupies the slot, so a lookup fails loudly instead of returning
/// the wrong tank.
///
/// The wire form is a two element array, `[index, generation]`. The packed form is
/// a `u64` used for database keys and for the binary protocol that replaces
/// newline-delimited JSON [NDJSON] later. JavaScript numbers cannot hold a packed
/// handle exactly, which is why the wire form stays split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
// ts-rs cannot read `from` and `into`, and says so at build time. The `type`
// override below states the same shape by hand. Change one and change the other.
#[serde(from = "(u32, u32)", into = "(u32, u32)")]
#[ts(export, export_to = "schema.ts", type = "[number, number]")]
pub struct EntityId {
    pub index: u32,
    pub generation: u32,
}

impl EntityId {
    pub fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Packed representation. Index in the high half, generation in the low half.
    pub fn to_u64(self) -> u64 {
        ((self.index as u64) << 32) | (self.generation as u64)
    }

    pub fn from_u64(packed: u64) -> Self {
        Self {
            index: (packed >> 32) as u32,
            generation: (packed & 0xFFFF_FFFF) as u32,
        }
    }
}

impl From<(u32, u32)> for EntityId {
    fn from((index, generation): (u32, u32)) -> Self {
        Self { index, generation }
    }
}

impl From<EntityId> for (u32, u32) {
    fn from(id: EntityId) -> Self {
        (id.index, id.generation)
    }
}

//! Entity storage, keyed by a handle that cannot alias.
//!
//! A slot index alone is not enough. Something always holds a reference to an
//! entity across the tick that kills it — a bullet's owner, an agent's last target
//! — and if the slot is recycled that reference silently starts pointing at a
//! different tank. Pairing the index with a generation counter makes the stale
//! handle compare unequal, so the lookup fails loudly instead of returning the
//! wrong entity.
//!
//! `slotmap` already implements exactly this, and its key packs a 32-bit index
//! beside a 32-bit version, which is the same shape as `schema::EntityId`. The
//! conversion below is the whole integration.

use slotmap::{new_key_type, Key, KeyData, SlotMap};

use crate::entity::Entity;
use schema::EntityId;

new_key_type! {
    /// Internal key. Never crosses the wire; `EntityId` does.
    pub struct EntityKey;
}

/// Convert an internal key to the wire handle.
pub fn to_id(key: EntityKey) -> EntityId {
    // slotmap packs `(version << 32) | index`. Ours is the two halves named.
    let ffi = key.data().as_ffi();
    EntityId::new(ffi as u32, (ffi >> 32) as u32)
}

/// Convert a wire handle back to an internal key.
pub fn to_key(id: EntityId) -> EntityKey {
    KeyData::from_ffi(((id.generation as u64) << 32) | id.index as u64).into()
}

/// The entity arena.
///
/// Iteration order is by slot index and therefore stable across runs, which the
/// determinism invariant depends on. Do not swap this for a hashed container.
#[derive(Debug, Default, Clone)]
pub struct Store {
    slots: SlotMap<EntityKey, Entity>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, e: Entity) -> EntityId {
        to_id(self.slots.insert(e))
    }

    pub fn remove(&mut self, id: EntityId) -> Option<Entity> {
        self.slots.remove(to_key(id))
    }

    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.slots.get(to_key(id))
    }

    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.slots.get_mut(to_key(id))
    }

    pub fn contains(&self, id: EntityId) -> bool {
        self.slots.contains_key(to_key(id))
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Entities in slot order. Stable.
    pub fn iter(&self) -> impl Iterator<Item = (EntityId, &Entity)> {
        self.slots.iter().map(|(k, e)| (to_id(k), e))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (EntityId, &mut Entity)> {
        self.slots.iter_mut().map(|(k, e)| (to_id(k), e))
    }

    /// Handles in slot order. Collected, so the caller may mutate while walking it.
    pub fn ids(&self) -> Vec<EntityId> {
        self.slots.keys().map(to_id).collect()
    }

    /// Two entities at once. `None` when either handle is stale or both name the
    /// same slot.
    pub fn get_pair_mut(&mut self, a: EntityId, b: EntityId) -> Option<(&mut Entity, &mut Entity)> {
        let (ka, kb) = (to_key(a), to_key(b));
        if ka == kb {
            return None;
        }
        let [ea, eb] = self.slots.get_disjoint_mut([ka, kb])?;
        Some((ea, eb))
    }
}

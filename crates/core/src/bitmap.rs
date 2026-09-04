//! A bitmap over entity slot indices.
//!
//! The movement step used to walk every entity in the arena. At two thousand
//! shapes, nearly all of which are sitting still once drag has taken their spawn
//! velocity, that is two thousand multiplications a tick to move nothing.
//!
//! This marks the slots that are actually in motion. The movement step reads the
//! set bits and touches those entities only. A shape is entered when something
//! shoves it and dropped again once drag has brought it below
//! [`crate::constants::MOVING_EPSILON`].
//!
//! Bits are indexed by the slot index inside an `EntityId`, so iteration is in
//! ascending slot order — the same order `Store::iter` uses, and the same order the
//! determinism invariant already depends on.

/// A dense bitset addressed by slot index.
#[derive(Debug, Default, Clone)]
pub struct BitMap {
    words: Vec<u64>,
    len: usize,
}

impl BitMap {
    pub fn new() -> Self {
        Self::default()
    }

    fn grow_for(&mut self, index: u32) {
        let word = index as usize / 64 + 1;
        if self.words.len() < word {
            self.words.resize(word, 0);
        }
    }

    pub fn set(&mut self, index: u32) {
        self.grow_for(index);
        let (w, b) = (index as usize / 64, index as usize % 64);
        if self.words[w] & (1 << b) == 0 {
            self.words[w] |= 1 << b;
            self.len += 1;
        }
    }

    pub fn clear(&mut self, index: u32) {
        let (w, b) = (index as usize / 64, index as usize % 64);
        if w < self.words.len() && self.words[w] & (1 << b) != 0 {
            self.words[w] &= !(1 << b);
            self.len -= 1;
        }
    }

    pub fn contains(&self, index: u32) -> bool {
        let (w, b) = (index as usize / 64, index as usize % 64);
        w < self.words.len() && self.words[w] & (1 << b) != 0
    }

    /// How many bits are set. The count of entities the movement step will touch.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn clear_all(&mut self) {
        for w in &mut self.words {
            *w = 0;
        }
        self.len = 0;
    }

    /// Set bits in ascending order.
    ///
    /// Skips whole empty words at a time, which is the point: a mostly-still arena
    /// costs one comparison per sixty-four slots.
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.words
            .iter()
            .enumerate()
            .flat_map(|(w, &word)| BitsOf { word }.map(move |b| (w * 64 + b as usize) as u32))
    }
}

/// The set bit positions of one word, lowest first.
struct BitsOf {
    word: u64,
}

impl Iterator for BitsOf {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.word == 0 {
            return None;
        }
        let b = self.word.trailing_zeros();
        self.word &= self.word - 1;
        Some(b)
    }
}

#[cfg(test)]
mod tests {
    use super::BitMap;

    #[test]
    fn set_clear_and_iterate_in_order() {
        let mut m = BitMap::new();
        for i in [200u32, 3, 64, 1, 4095] {
            m.set(i);
        }
        assert_eq!(m.len(), 5);
        assert_eq!(m.iter().collect::<Vec<_>>(), vec![1, 3, 64, 200, 4095]);

        m.clear(64);
        m.clear(64); // idempotent
        assert_eq!(m.len(), 4);
        assert!(!m.contains(64));
        assert!(m.contains(4095));

        m.set(3); // already set, no double count
        assert_eq!(m.len(), 4);

        m.clear_all();
        assert!(m.is_empty());
        assert_eq!(m.iter().count(), 0);
    }

    #[test]
    fn clearing_an_index_that_was_never_set_is_harmless() {
        let mut m = BitMap::new();
        m.clear(9_999);
        assert_eq!(m.len(), 0);
    }
}

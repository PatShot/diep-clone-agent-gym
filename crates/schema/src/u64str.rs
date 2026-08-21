//! Exact 64-bit integers across a JavaScript boundary.
//!
//! A seed and a configuration hash must survive the round trip bit for bit. A
//! JavaScript number cannot hold one above 2^53, and `JSON.parse` produces a
//! number regardless of what the type declaration claims. So these fields travel
//! as decimal strings. Rust keeps them as `u64`; the viewer treats them as opaque
//! labels, which is all it does with them.
//!
//! Counters that cannot plausibly exceed 2^53 stay numbers. Only provenance values
//! use this.

use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<S: Serializer>(value: &u64, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&value.to_string())
}

pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    let text = String::deserialize(d)?;
    text.parse::<u64>().map_err(serde::de::Error::custom)
}

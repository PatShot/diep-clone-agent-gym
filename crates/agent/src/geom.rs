//! Vector arithmetic on the wire type.
//!
//! `schema::Vec2` is plain data with no operators, by design: it is the wire
//! contract, not a maths library. Policies do a little geometry, and this is it.

use schema::Vec2;

pub fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x - b.x, a.y - b.y)
}

pub fn scale(a: Vec2, k: f32) -> Vec2 {
    Vec2::new(a.x * k, a.y * k)
}

/// Unit length, or zero if the input is zero. A zero thrust is a valid action.
pub fn normalized(a: Vec2) -> Vec2 {
    let len = a.length();
    if len > 1e-6 {
        scale(a, 1.0 / len)
    } else {
        Vec2::ZERO
    }
}

/// Unit vector from `from` toward `to`.
pub fn toward(from: Vec2, to: Vec2) -> Vec2 {
    normalized(sub(to, from))
}

/// Heading of a vector in radians, in the simulation's convention: zero points
/// east, positive turns toward south.
pub fn heading(a: Vec2) -> f32 {
    a.y.atan2(a.x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_zero_is_zero() {
        assert_eq!(normalized(Vec2::ZERO), Vec2::ZERO);
    }

    #[test]
    fn toward_is_unit_length() {
        let v = toward(Vec2::new(1.0, 1.0), Vec2::new(4.0, 5.0));
        assert!((v.length() - 1.0).abs() < 1e-5);
        assert!((v.x - 0.6).abs() < 1e-5 && (v.y - 0.8).abs() < 1e-5);
    }

    #[test]
    fn heading_east_is_zero_and_south_is_positive() {
        assert_eq!(heading(Vec2::new(1.0, 0.0)), 0.0);
        assert!(heading(Vec2::new(0.0, 1.0)) > 0.0);
    }
}

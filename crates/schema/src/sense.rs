//! Sensor output.
//!
//! v0 gives an agent a plain radius query and leaves the ray fields empty. The
//! planar scanning sensor — a two-dimensional LIDAR [Light Detection and Ranging]
//! analogue — arrives at v0.5. The type is defined now because `Observation`
//! carries it, and `Observation` is what every policy is written against.
//!
//! A scan stays in polar form in the sensor frame with its pose stamp attached.
//! Converting to world coordinates is the consumer's job. Keeping the world, body,
//! and sensor frames distinct from the first commit saves a month later.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::geom::Vec2;
use crate::ids::Tick;

/// Position and heading at an instant. A scan carries two of these because the
/// sensor sweeps while the tank moves, and deskewing that motion is a real
/// algorithm an agent has to implement.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct Pose {
    pub pos: Vec2,
    pub heading: f32,
    pub tick: Tick,
}

/// One sweep of the sensor.
///
/// `ranges` holds one entry per ray, evenly spaced over `sweep` radians starting at
/// `angle_start`, measured in the sensor frame. A `None` entry is a dropout: no
/// return, either from a grazing incidence angle or from exceeding maximum range.
/// Dropouts are information and must not be silently replaced with the maximum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "schema.ts")]
pub struct Scan {
    /// Pose when the sweep began.
    pub pose_start: Pose,
    /// Pose when the sweep ended. Equal to `pose_start` when motion distortion is
    /// disabled.
    pub pose_end: Pose,
    pub angle_start: f32,
    pub sweep: f32,
    pub max_range: f32,
    pub ranges: Vec<Option<f32>>,
    /// Return strength per ray, when the intensity model is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intensities: Option<Vec<f32>>,
}

impl Scan {
    /// Bearing of ray `i` in the sensor frame, radians.
    pub fn bearing(&self, i: usize) -> f32 {
        if self.ranges.is_empty() {
            return self.angle_start;
        }
        self.angle_start + self.sweep * (i as f32) / (self.ranges.len() as f32)
    }

    /// An empty sweep. What v0 hands to a policy while sensing is a radius query.
    pub fn empty(pose: Pose, max_range: f32) -> Self {
        Self {
            pose_start: pose,
            pose_end: pose,
            angle_start: 0.0,
            sweep: std::f32::consts::TAU,
            max_range,
            ranges: Vec::new(),
            intensities: None,
        }
    }
}

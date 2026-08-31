//! Baked light-probe volume for indirect lighting.

use crate::ReflectVec3;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Largest number of probes a single volume may hold, matching the GPU
/// uniform array. A volume asking for more is clamped rather than
/// rejected -- an over-specified scene should still render.
pub const MAX_PROBES: usize = 32;

/// A box filled with a regular grid of baked light probes, giving
/// position-varying indirect light inside it.
///
/// Probes sit on the grid's **lattice points**, not cell centres: trilinear
/// interpolation is defined by the eight corners of a cell, so corner
/// placement is what makes the interpolation well-formed.
///
/// Absent means no probe GI, and the existing IBL path runs unchanged.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct LightProbeVolume {
    /// Half-extents of the box, relative to the entity's `Transform`.
    pub half_extents: ReflectVec3,
    /// Probe count along each axis. The product is clamped to
    /// [`MAX_PROBES`]. `[4, 2, 4]` gives exactly 32.
    pub resolution: [u32; 3],
}

impl Default for LightProbeVolume {
    fn default() -> Self {
        Self {
            half_extents: glam::Vec3::splat(5.0).into(),
            resolution: [4, 2, 4],
        }
    }
}

impl LightProbeVolume {
    /// Resolution with each axis at least 2 (one probe per axis cannot
    /// interpolate) and the product clamped to [`MAX_PROBES`].
    pub fn clamped_resolution(&self) -> [u32; 3] {
        let mut r = [
            self.resolution[0].max(2),
            self.resolution[1].max(2),
            self.resolution[2].max(2),
        ];
        while (r[0] * r[1] * r[2]) as usize > MAX_PROBES {
            // Shrink the largest axis first, keeping the grid as even as
            // possible rather than collapsing one direction.
            let largest = if r[0] >= r[1] && r[0] >= r[2] {
                0
            } else if r[1] >= r[2] {
                1
            } else {
                2
            };
            if r[largest] <= 2 {
                break;
            }
            r[largest] -= 1;
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_resolution_fits_max_probes_exactly() {
        let v = LightProbeVolume::default();
        let r = v.clamped_resolution();
        assert_eq!((r[0] * r[1] * r[2]) as usize, MAX_PROBES);
    }

    #[test]
    fn an_oversized_resolution_is_clamped_not_rejected() {
        let v = LightProbeVolume {
            resolution: [16, 16, 16],
            ..Default::default()
        };
        let r = v.clamped_resolution();
        assert!((r[0] * r[1] * r[2]) as usize <= MAX_PROBES);
        assert!(
            r.iter().all(|&a| a >= 2),
            "every axis must keep >= 2 probes"
        );
    }

    #[test]
    fn a_degenerate_resolution_is_raised_to_two_per_axis() {
        let v = LightProbeVolume {
            resolution: [1, 0, 1],
            ..Default::default()
        };
        assert_eq!(v.clamped_resolution(), [2, 2, 2]);
    }
}

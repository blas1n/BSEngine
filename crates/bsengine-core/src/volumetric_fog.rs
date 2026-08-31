//! Volumetric fog settings for a camera.

use crate::ReflectVec3;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Atmospheric scattering applied by a camera.
///
/// Absent means off, leaving rendering exactly as it was -- which is what
/// keeps every existing pixel test passing unchanged.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct VolumetricFog {
    /// Whether the effect is applied at all.
    pub enabled: bool,
    /// Extinction per world unit. Higher is thicker.
    pub density: f32,
    /// Scattering albedo -- the colour the fog scatters.
    pub color: ReflectVec3,
    /// Henyey-Greenstein anisotropy in (-1, 1). 0 is isotropic; positive
    /// scatters forward, giving the bright halo around a light seen
    /// through haze.
    pub anisotropy: f32,
}

impl Default for VolumetricFog {
    fn default() -> Self {
        Self {
            enabled: true,
            density: 0.02,
            color: glam::Vec3::splat(1.0).into(),
            anisotropy: 0.3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_enabled_and_physically_sensible() {
        let f = VolumetricFog::default();
        assert!(f.enabled);
        assert!(
            f.density > 0.0,
            "a zero default density would render nothing"
        );
        assert!(
            f.anisotropy > -1.0 && f.anisotropy < 1.0,
            "anisotropy outside (-1, 1) makes the phase function singular"
        );
    }
}

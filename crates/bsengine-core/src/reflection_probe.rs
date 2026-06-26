use bevy_ecs::prelude::Component;
use glam::Vec3;

/// How often the reflection probe re-captures its surroundings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeUpdateMode {
    /// Captured once (baked). Cheapest — use for static environments.
    Static,
    /// Re-captured every frame. Most accurate but highest cost.
    EveryFrame,
    /// Re-captured once every N frames. Balances quality and cost.
    EveryNthFrame(u32),
}

/// Local reflection probe — captures surroundings into a cubemap used for specular reflections.
/// The entity's world-space position is the capture point; `half_extents` defines the influence volume.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ReflectionProbe {
    /// Half-size of the axis-aligned box that this probe influences.
    pub half_extents: Vec3,
    /// Overall intensity multiplier for the captured reflections.
    pub intensity: f32,
    /// When to regenerate the captured cubemap.
    pub update_mode: ProbeUpdateMode,
    pub enabled: bool,
}

impl ReflectionProbe {
    pub fn new(half_extents: Vec3) -> Self {
        Self {
            half_extents: half_extents.max(Vec3::ZERO),
            intensity: 1.0,
            update_mode: ProbeUpdateMode::Static,
            enabled: true,
        }
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.max(0.0);
        self
    }

    pub fn with_update_mode(mut self, mode: ProbeUpdateMode) -> Self {
        self.update_mode = mode;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflection_probe_defaults() {
        let rp = ReflectionProbe::new(Vec3::splat(5.0));
        assert_eq!(rp.half_extents, Vec3::splat(5.0));
        assert!((rp.intensity - 1.0).abs() < 0.001);
        assert_eq!(rp.update_mode, ProbeUpdateMode::Static);
        assert!(rp.enabled);
    }

    #[test]
    fn half_extents_clamped_to_zero() {
        let rp = ReflectionProbe::new(Vec3::splat(-1.0));
        assert_eq!(rp.half_extents, Vec3::ZERO);
    }

    #[test]
    fn intensity_clamped() {
        let rp = ReflectionProbe::new(Vec3::ONE).with_intensity(-2.0);
        assert_eq!(rp.intensity, 0.0);
    }

    #[test]
    fn every_nth_frame_mode() {
        let rp =
            ReflectionProbe::new(Vec3::ONE).with_update_mode(ProbeUpdateMode::EveryNthFrame(4));
        assert_eq!(rp.update_mode, ProbeUpdateMode::EveryNthFrame(4));
    }

    #[test]
    fn disabled_flag() {
        let rp = ReflectionProbe::new(Vec3::ONE).disabled();
        assert!(!rp.enabled);
    }
}

use bevy_ecs::prelude::Component;

use crate::Color;

/// How distance affects fog density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FogMode {
    /// Density increases linearly from `start_distance` to `end_distance`.
    Linear,
    /// Density grows exponentially with distance: `exp(-density * d)`.
    #[default]
    Exponential,
    /// Exponential-squared — denser than Exponential in the mid-ground.
    ExponentialSquared,
}

/// Atmospheric distance fog applied by a camera.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Fog {
    pub color: Color,
    /// Fog density coefficient used by Exponential and ExponentialSquared modes.
    pub density: f32,
    /// Distance at which Linear fog begins.
    pub start_distance: f32,
    /// Distance at which Linear fog is fully opaque.
    pub end_distance: f32,
    pub mode: FogMode,
    pub enabled: bool,
}

impl Fog {
    pub fn new(color: Color) -> Self {
        Self {
            color,
            density: 0.01,
            start_distance: 10.0,
            end_distance: 100.0,
            mode: FogMode::Exponential,
            enabled: true,
        }
    }

    pub fn with_density(mut self, density: f32) -> Self {
        self.density = density.max(0.0);
        self
    }

    pub fn with_linear_range(mut self, start: f32, end: f32) -> Self {
        self.start_distance = start.max(0.0);
        self.end_distance = end.max(self.start_distance);
        self
    }

    pub fn with_mode(mut self, mode: FogMode) -> Self {
        self.mode = mode;
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
    fn fog_defaults() {
        let fog = Fog::new(Color::WHITE);
        assert_eq!(fog.color, Color::WHITE);
        assert!((fog.density - 0.01).abs() < 0.0001);
        assert!((fog.start_distance - 10.0).abs() < 0.001);
        assert!((fog.end_distance - 100.0).abs() < 0.001);
        assert_eq!(fog.mode, FogMode::Exponential);
        assert!(fog.enabled);
    }

    #[test]
    fn density_clamped() {
        let fog = Fog::new(Color::WHITE).with_density(-1.0);
        assert_eq!(fog.density, 0.0);
    }

    #[test]
    fn linear_range_end_clamped_to_start() {
        let fog = Fog::new(Color::WHITE).with_linear_range(50.0, 20.0);
        assert!((fog.start_distance - 50.0).abs() < 0.001);
        assert!(fog.end_distance >= fog.start_distance);
    }

    #[test]
    fn fog_mode_stored() {
        let fog = Fog::new(Color::WHITE).with_mode(FogMode::Linear);
        assert_eq!(fog.mode, FogMode::Linear);
    }

    #[test]
    fn disabled_flag() {
        let fog = Fog::new(Color::WHITE).disabled();
        assert!(!fog.enabled);
    }
}

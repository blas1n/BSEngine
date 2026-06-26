use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Axis along which the oscillation is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OscillateAxis {
    /// Sinusoidal translation along a world-space vector.
    Translation,
    /// Sinusoidal rotation around a world-space axis (angle in radians).
    Rotation,
}

/// Periodic sinusoidal motion for bobbing pickups, swinging platforms, pendulums, etc.
///
/// Each frame the movement system reads `offset()` and applies it on top of the entity's
/// base transform. The `phase` accumulates via `tick(dt)`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Oscillate {
    /// Oscillation kind — translation or rotation.
    pub axis: OscillateAxis,
    /// World-space direction (translation) or rotation axis (rotation). Should be normalised.
    pub direction: Vec3,
    /// Peak displacement (metres for translation, radians for rotation).
    pub amplitude: f32,
    /// Full cycles per second.
    pub frequency: f32,
    /// Current phase accumulator (radians).
    pub phase: f32,
    /// Phase offset at spawn (radians) — stagger multiple instances.
    pub phase_offset: f32,
    pub enabled: bool,
}

impl Oscillate {
    pub fn translation(direction: Vec3, amplitude: f32, frequency: f32) -> Self {
        Self {
            axis: OscillateAxis::Translation,
            direction: direction.normalize_or_zero(),
            amplitude: amplitude.abs(),
            frequency: frequency.abs(),
            phase: 0.0,
            phase_offset: 0.0,
            enabled: true,
        }
    }

    pub fn rotation(axis: Vec3, amplitude_radians: f32, frequency: f32) -> Self {
        Self {
            axis: OscillateAxis::Rotation,
            direction: axis.normalize_or_zero(),
            amplitude: amplitude_radians.abs(),
            frequency: frequency.abs(),
            phase: 0.0,
            phase_offset: 0.0,
            enabled: true,
        }
    }

    pub fn with_phase_offset(mut self, offset_radians: f32) -> Self {
        self.phase_offset = offset_radians;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Advance the phase accumulator by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        if self.enabled {
            self.phase += dt * self.frequency * std::f32::consts::TAU;
        }
    }

    /// Current signed scalar offset (metres or radians).
    /// Apply as: `base_pos + oscillate.direction * oscillate.scalar_offset()`
    pub fn scalar_offset(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        (self.phase + self.phase_offset).sin() * self.amplitude
    }

    /// Convenience: translation offset as a Vec3 (zero for rotation axes).
    pub fn translation_offset(&self) -> Vec3 {
        if self.axis == OscillateAxis::Translation {
            self.direction * self.scalar_offset()
        } else {
            Vec3::ZERO
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, TAU};

    #[test]
    fn zero_phase_gives_zero_offset() {
        let o = Oscillate::translation(Vec3::Y, 1.0, 1.0);
        assert!((o.scalar_offset()).abs() < 1e-6);
    }

    #[test]
    fn quarter_cycle_gives_peak_amplitude() {
        let mut o = Oscillate::translation(Vec3::Y, 2.0, 1.0);
        o.tick(0.25); // 0.25 s * 1 Hz * TAU = π/2
        assert!((o.scalar_offset() - 2.0).abs() < 1e-5);
    }

    #[test]
    fn half_cycle_returns_to_zero() {
        let mut o = Oscillate::translation(Vec3::Y, 2.0, 1.0);
        o.tick(0.5); // π
        assert!(o.scalar_offset().abs() < 1e-5);
    }

    #[test]
    fn phase_offset_shifts_start() {
        let o = Oscillate::translation(Vec3::Y, 1.0, 1.0).with_phase_offset(FRAC_PI_2);
        // sin(π/2) = 1
        assert!((o.scalar_offset() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn disabled_oscillate_returns_zero() {
        let mut o = Oscillate::translation(Vec3::Y, 5.0, 10.0).disabled();
        o.tick(0.25);
        assert!((o.scalar_offset()).abs() < 1e-6);
        assert_eq!(o.translation_offset(), Vec3::ZERO);
    }

    #[test]
    fn rotation_oscillate_produces_nonzero_scalar() {
        let mut o = Oscillate::rotation(Vec3::Z, FRAC_PI_2, 1.0);
        o.tick(0.25);
        assert!((o.scalar_offset() - FRAC_PI_2).abs() < 1e-5);
        // translation_offset is zero for rotation axis
        assert_eq!(o.translation_offset(), Vec3::ZERO);
    }

    #[test]
    fn full_cycle_returns_phase_to_near_zero_offset() {
        let mut o = Oscillate::translation(Vec3::X, 3.0, 2.0);
        o.tick(0.5); // 0.5 s * 2 Hz * TAU = 2π
        assert!(o.scalar_offset().abs() < 1e-4);
    }
}

use bevy_ecs::prelude::Component;
use glam::Vec3;

/// How the tint intensity changes over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TintMode {
    /// Intensity held constant until explicitly changed.
    Constant,
    /// Intensity fades toward zero at `fade_rate` per second.
    Fading,
    /// Intensity oscillates between 0 and peak at `pulse_speed` Hz.
    Pulsing,
}

/// Persistent color-tint overlay component.
///
/// Distinct from `Flash` (one-shot transient hit-flash) — `Tint` models
/// sustained color overlays such as poison (green), burning (red), freeze
/// (blue), or curse (purple). Tints can be stacked or blended by the
/// renderer using `color * intensity`.
///
/// Call `set(color, intensity)` to apply a tint. `tick(dt)` handles fade
/// and pulse modes. `clear()` resets to no tint.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Tint {
    /// Linear RGB color of the overlay (values may exceed 1.0 for HDR).
    pub color: Vec3,
    /// Current intensity [0, 1] for Constant/Fading modes.
    pub intensity: f32,
    pub mode: TintMode,
    /// Intensity lost per second in Fading mode.
    pub fade_rate: f32,
    /// Oscillation frequency in Hz (cycles/second) in Pulsing mode.
    pub pulse_speed: f32,
    /// Internal phase accumulator for Pulsing (radians).
    pub pulse_phase: f32,
    /// Peak intensity used as the ceiling for Pulsing mode.
    pub peak_intensity: f32,
    pub enabled: bool,
}

impl Tint {
    pub fn new(color: Vec3) -> Self {
        Self {
            color,
            intensity: 0.0,
            mode: TintMode::Constant,
            fade_rate: 0.0,
            pulse_speed: 1.0,
            pulse_phase: 0.0,
            peak_intensity: 1.0,
            enabled: true,
        }
    }

    /// Poison green tint.
    pub fn poison() -> Self {
        Self::new(Vec3::new(0.0, 1.0, 0.0))
    }

    /// Burn / fire red tint.
    pub fn burn() -> Self {
        Self::new(Vec3::new(1.0, 0.2, 0.0))
    }

    /// Freeze / ice blue tint.
    pub fn freeze() -> Self {
        Self::new(Vec3::new(0.3, 0.7, 1.0))
    }

    pub fn with_fade(mut self, rate: f32) -> Self {
        self.fade_rate = rate.max(0.0);
        self.mode = TintMode::Fading;
        self
    }

    pub fn with_pulse(mut self, speed_hz: f32) -> Self {
        self.pulse_speed = speed_hz.max(0.0);
        self.mode = TintMode::Pulsing;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Apply or re-apply the tint at the given intensity.
    pub fn set(&mut self, color: Vec3, intensity: f32) {
        if !self.enabled {
            return;
        }
        self.color = color;
        self.intensity = intensity.clamp(0.0, 1.0);
        self.peak_intensity = self.intensity;
        self.pulse_phase = 0.0;
    }

    /// Remove the tint instantly.
    pub fn clear(&mut self) {
        self.intensity = 0.0;
        self.peak_intensity = 0.0;
        self.pulse_phase = 0.0;
    }

    /// Advance the tint. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled {
            return;
        }
        match self.mode {
            TintMode::Fading => {
                self.intensity = (self.intensity - self.fade_rate * dt).max(0.0);
            }
            TintMode::Pulsing => {
                use std::f32::consts::TAU;
                self.pulse_phase = (self.pulse_phase + self.pulse_speed * TAU * dt) % TAU;
                // intensity oscillates [0, peak] using (1 + sin) / 2
                self.intensity = self.peak_intensity * (1.0 + self.pulse_phase.sin()) * 0.5;
            }
            TintMode::Constant => {}
        }
    }

    pub fn is_active(&self) -> bool {
        self.enabled && self.intensity > 0.0
    }

    /// The tinted color at current intensity (pre-multiplied alpha equivalent).
    pub fn tinted_color(&self) -> Vec3 {
        self.color * self.intensity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_query() {
        let mut t = Tint::new(Vec3::new(0.0, 1.0, 0.0));
        t.set(Vec3::new(0.0, 1.0, 0.0), 0.8);
        assert!(t.is_active());
        assert!((t.intensity - 0.8).abs() < 1e-5);
    }

    #[test]
    fn clear_deactivates() {
        let mut t = Tint::poison();
        t.set(Vec3::new(0.0, 1.0, 0.0), 1.0);
        t.clear();
        assert!(!t.is_active());
    }

    #[test]
    fn fading_decays_intensity() {
        let mut t = Tint::poison().with_fade(1.0);
        t.set(Vec3::new(0.0, 1.0, 0.0), 1.0);
        t.tick(0.5);
        assert!((t.intensity - 0.5).abs() < 1e-5);
    }

    #[test]
    fn fading_clamps_at_zero() {
        let mut t = Tint::poison().with_fade(2.0);
        t.set(Vec3::new(0.0, 1.0, 0.0), 1.0);
        t.tick(1.0);
        assert_eq!(t.intensity, 0.0);
    }

    #[test]
    fn pulsing_oscillates_in_range() {
        let mut t = Tint::burn().with_pulse(1.0);
        t.set(Vec3::new(1.0, 0.2, 0.0), 1.0);
        for _ in 0..60 {
            t.tick(1.0 / 60.0);
            assert!(t.intensity >= 0.0 && t.intensity <= 1.0 + 1e-5);
        }
    }

    #[test]
    fn disabled_ignores_set() {
        let mut t = Tint::poison().disabled();
        t.set(Vec3::new(0.0, 1.0, 0.0), 1.0);
        assert!(!t.is_active());
    }
}

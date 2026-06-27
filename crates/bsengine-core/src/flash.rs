use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Controls how the flash intensity falls off over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashCurve {
    /// Constant intensity until expired.
    Constant,
    /// Linearly fades from peak to zero over `duration`.
    Linear,
    /// Exponentially decays (smooth ease-out feel).
    Exponential,
}

/// Transient color-flash effect component for damage hits, pickups, and screen events.
///
/// Call `trigger(intensity, duration)` to start a flash. Subsequent calls while
/// a flash is active either stack on top of the current intensity or replace it
/// (whichever is larger). `tick(dt)` updates the timer and returns the current
/// intensity via `current_intensity()` for the rendering system.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Flash {
    /// Base color of the flash (linear RGB, values may exceed 1.0 for HDR).
    pub color: Vec3,
    /// Peak intensity at the moment the flash was triggered.
    pub peak_intensity: f32,
    /// How long the flash lasts (seconds).
    pub duration: f32,
    /// Time remaining.
    pub timer: f32,
    /// Falloff curve used when computing `current_intensity()`.
    pub curve: FlashCurve,
    pub enabled: bool,
}

impl Flash {
    pub fn new(color: Vec3) -> Self {
        Self {
            color,
            peak_intensity: 0.0,
            duration: 0.0,
            timer: 0.0,
            curve: FlashCurve::Linear,
            enabled: true,
        }
    }

    /// White flash (damage / hit indicator).
    pub fn white() -> Self {
        Self::new(Vec3::ONE)
    }

    /// Red flash (player damage).
    pub fn red() -> Self {
        Self::new(Vec3::new(1.0, 0.0, 0.0))
    }

    pub fn with_curve(mut self, curve: FlashCurve) -> Self {
        self.curve = curve;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Start or re-trigger a flash. If a flash is already active, takes the
    /// maximum of the current remaining intensity and the new trigger.
    pub fn trigger(&mut self, intensity: f32, duration: f32) {
        if !self.enabled {
            return;
        }
        let new_i = intensity.max(0.0);
        let new_d = duration.max(0.0);
        if new_i > self.current_intensity() {
            self.peak_intensity = new_i;
            self.duration = new_d;
            self.timer = new_d;
        }
    }

    /// Advance the flash timer. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        if self.timer > 0.0 {
            self.timer = (self.timer - dt).max(0.0);
        }
    }

    /// Current flash intensity after applying the falloff curve.
    pub fn current_intensity(&self) -> f32 {
        if self.timer <= 0.0 || self.duration <= 0.0 {
            return 0.0;
        }
        let t = self.timer / self.duration; // 1.0 at start, 0.0 at end
        match self.curve {
            FlashCurve::Constant => self.peak_intensity,
            FlashCurve::Linear => self.peak_intensity * t,
            FlashCurve::Exponential => self.peak_intensity * t * t,
        }
    }

    pub fn is_active(&self) -> bool {
        self.enabled && self.timer > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_starts_flash() {
        let mut f = Flash::white();
        f.trigger(1.0, 0.5);
        assert!(f.is_active());
        assert!((f.current_intensity() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_timer() {
        let mut f = Flash::white();
        f.trigger(1.0, 1.0);
        f.tick(0.5);
        assert!((f.timer - 0.5).abs() < 1e-5);
    }

    #[test]
    fn linear_curve_fades_correctly() {
        let mut f = Flash::white().with_curve(FlashCurve::Linear);
        f.trigger(2.0, 1.0);
        f.tick(0.5); // timer = 0.5, t = 0.5
        assert!((f.current_intensity() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn constant_curve_holds_intensity() {
        let mut f = Flash::white().with_curve(FlashCurve::Constant);
        f.trigger(1.0, 1.0);
        f.tick(0.9);
        assert!((f.current_intensity() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn expired_flash_returns_zero_intensity() {
        let mut f = Flash::white();
        f.trigger(1.0, 0.5);
        f.tick(1.0);
        assert!(!f.is_active());
        assert_eq!(f.current_intensity(), 0.0);
    }

    #[test]
    fn disabled_ignores_trigger() {
        let mut f = Flash::white().disabled();
        f.trigger(1.0, 1.0);
        assert!(!f.is_active());
    }
}

use bevy_ecs::prelude::Component;

/// Illumination flare component — a burning signal attached to a flare entity
/// that lights up an area, exposes stealthed targets, and fades over time.
///
/// Attach to the flare projectile entity. Lighting and stealth-reveal systems
/// query `is_burning()` and read `radius` / `current_intensity()` each frame.
/// `current_intensity()` decays linearly so the flare dims as it burns out.
///
/// `ignite(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_extinguished` on burnout. `extinguish()` puts it out early.
///
/// Distinct from `Flash` (single-frame screen-space effect) and `Blind`
/// (debuff on an entity): Flare is a persistent area-light source in the world.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Flare {
    pub duration: f32,
    pub timer: f32,
    /// Illumination radius in world units.
    pub radius: f32,
    /// Peak light intensity [0.0, 1.0] when the flare is freshly ignited.
    pub intensity: f32,
    pub just_ignited: bool,
    pub just_extinguished: bool,
    pub enabled: bool,
}

impl Flare {
    pub fn new(radius: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            radius: radius.max(0.0),
            intensity: 1.0,
            just_ignited: false,
            just_extinguished: false,
            enabled: true,
        }
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }

    /// Ignite or extend the flare for `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer.
    pub fn ignite(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_burning = self.is_burning();
            self.duration = duration;
            self.timer = duration;
            if !was_burning {
                self.just_ignited = true;
            }
        }
    }

    /// Extinguish the flare early.
    pub fn extinguish(&mut self) {
        if self.is_burning() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_extinguished = true;
        }
    }

    /// Advance the timer; sets `just_extinguished` when the flare burns out.
    pub fn tick(&mut self, dt: f32) {
        self.just_ignited = false;
        self.just_extinguished = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_extinguished = true;
            }
        }
    }

    pub fn is_burning(&self) -> bool {
        self.timer > 0.0
    }

    /// Actual light intensity this frame, decaying linearly as the flare burns
    /// down. Returns `intensity * remaining_fraction()` while burning, `0.0`
    /// once extinguished.
    pub fn current_intensity(&self) -> f32 {
        if !self.is_burning() {
            return 0.0;
        }
        self.intensity * self.remaining_fraction()
    }

    /// Fraction of the flare duration remaining [1.0 = just ignited, 0.0 = out].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Flare {
    fn default() -> Self {
        Self::new(15.0).with_intensity(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignite_starts_flare() {
        let mut f = Flare::new(10.0);
        f.ignite(5.0);
        assert!(f.is_burning());
        assert!(f.just_ignited);
    }

    #[test]
    fn ignite_extends_on_longer_duration() {
        let mut f = Flare::new(10.0);
        f.ignite(3.0);
        f.tick(0.016);
        f.ignite(8.0);
        assert!((f.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn ignite_no_extend_on_shorter_duration() {
        let mut f = Flare::new(10.0);
        f.ignite(8.0);
        f.ignite(3.0);
        assert!((f.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_flare() {
        let mut f = Flare::new(10.0);
        f.ignite(1.0);
        f.tick(1.1);
        assert!(!f.is_burning());
        assert!(f.just_extinguished);
    }

    #[test]
    fn extinguish_ends_early() {
        let mut f = Flare::new(10.0);
        f.ignite(5.0);
        f.extinguish();
        assert!(!f.is_burning());
        assert!(f.just_extinguished);
    }

    #[test]
    fn current_intensity_at_full_when_just_ignited() {
        let mut f = Flare::new(10.0).with_intensity(0.8);
        f.ignite(10.0);
        // remaining_fraction = 1.0 → current = 0.8
        assert!((f.current_intensity() - 0.8).abs() < 1e-5);
    }

    #[test]
    fn current_intensity_decays_to_half_at_midpoint() {
        let mut f = Flare::new(10.0).with_intensity(1.0);
        f.ignite(2.0);
        f.tick(1.0);
        assert!((f.current_intensity() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn current_intensity_zero_when_extinguished() {
        let f = Flare::new(10.0).with_intensity(1.0);
        assert!((f.current_intensity() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut f = Flare::new(10.0);
        f.ignite(2.0);
        f.tick(1.0);
        assert!((f.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_ignite_no_op() {
        let mut f = Flare::new(10.0);
        f.enabled = false;
        f.ignite(5.0);
        assert!(!f.is_burning());
    }

    #[test]
    fn tick_clears_just_ignited() {
        let mut f = Flare::new(10.0);
        f.ignite(5.0);
        f.tick(0.016);
        assert!(!f.just_ignited);
    }
}

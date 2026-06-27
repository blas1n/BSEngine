use bevy_ecs::prelude::Component;

/// Projectile-spread debuff that multiplies an entity's fire cone and
/// optionally adds extra pellets per shot.
///
/// While scattered, the projectile-spawn system multiplies the base spread
/// angle by `spread_multiplier` and fires `extra_pellets` additional
/// projectiles alongside the normal shot.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_cleared` on expiry. `clear()` removes the effect early.
///
/// Distinct from `Daze` (random per-frame aim deviation from the aim system)
/// and `Tremble` (hand-shake reducing precision on aimed shots): Scatter
/// directly expands the projectile spawn cone and adds extra pellets,
/// regardless of how the base aim is computed.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Scatter {
    pub duration: f32,
    pub timer: f32,
    /// Factor by which the base fire-cone half-angle is multiplied (>= 1.0).
    pub spread_multiplier: f32,
    /// Extra projectiles fired alongside the main shot while scattered.
    pub extra_pellets: u32,
    pub just_scattered: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Scatter {
    pub fn new(spread_multiplier: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            spread_multiplier: spread_multiplier.max(1.0),
            extra_pellets: 0,
            just_scattered: false,
            just_cleared: false,
            enabled: true,
        }
    }

    pub fn with_extra_pellets(mut self, n: u32) -> Self {
        self.extra_pellets = n;
        self
    }

    /// Apply or extend the scatter for `duration` seconds. High-watermark:
    /// only replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_scattered = true;
            }
        }
    }

    /// Remove the scatter immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_cleared = true;
        }
    }

    /// Advance the timer; sets `just_cleared` when the effect expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_scattered = false;
        self.just_cleared = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_cleared = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective fire-cone half-angle in radians. Returns `base_rad *
    /// spread_multiplier` while active, `base_rad` otherwise.
    pub fn effective_spread(&self, base_rad: f32) -> f32 {
        if self.is_active() {
            base_rad * self.spread_multiplier
        } else {
            base_rad
        }
    }

    /// Fraction of the scatter duration remaining [1.0 = just applied, 0.0 = cleared].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Scatter {
    fn default() -> Self {
        Self::new(3.0).with_extra_pellets(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_scatter() {
        let mut s = Scatter::new(2.0);
        s.apply(3.0);
        assert!(s.is_active());
        assert!(s.just_scattered);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut s = Scatter::new(2.0);
        s.apply(2.0);
        s.tick(0.016);
        s.apply(5.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut s = Scatter::new(2.0);
        s.apply(5.0);
        s.apply(2.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_scatter() {
        let mut s = Scatter::new(2.0);
        s.apply(1.0);
        s.tick(1.1);
        assert!(!s.is_active());
        assert!(s.just_cleared);
    }

    #[test]
    fn clear_ends_early() {
        let mut s = Scatter::new(2.0);
        s.apply(5.0);
        s.clear();
        assert!(!s.is_active());
        assert!(s.just_cleared);
    }

    #[test]
    fn effective_spread_while_active() {
        let mut s = Scatter::new(3.0);
        s.apply(5.0);
        let base = std::f32::consts::FRAC_PI_8; // 22.5°
        let spread = s.effective_spread(base);
        assert!((spread - base * 3.0).abs() < 1e-5);
    }

    #[test]
    fn effective_spread_when_inactive() {
        let s = Scatter::new(3.0);
        let base = std::f32::consts::FRAC_PI_8;
        assert!((s.effective_spread(base) - base).abs() < 1e-5);
    }

    #[test]
    fn extra_pellets_set_by_builder() {
        let s = Scatter::new(2.0).with_extra_pellets(3);
        assert_eq!(s.extra_pellets, 3);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut s = Scatter::new(2.0);
        s.apply(2.0);
        s.tick(1.0);
        assert!((s.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut s = Scatter::new(2.0);
        s.enabled = false;
        s.apply(5.0);
        assert!(!s.is_active());
    }

    #[test]
    fn tick_clears_just_scattered() {
        let mut s = Scatter::new(2.0);
        s.apply(3.0);
        s.tick(0.016);
        assert!(!s.just_scattered);
    }

    #[test]
    fn spread_multiplier_clamped_to_min_one() {
        let s = Scatter::new(0.5);
        assert!((s.spread_multiplier - 1.0).abs() < 1e-5);
    }
}

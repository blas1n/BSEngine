use bevy_ecs::prelude::Component;

/// Movement-speed reduction debuff.
///
/// The locomotion system multiplies the entity's base speed by
/// `effective_multiplier()` while `is_active()` returns true.
/// Multiple slow sources can coexist by attaching this component with
/// the highest reduction winning (or using additive stacking — the system
/// decides the model; `Slow` only tracks the strongest single application).
///
/// `apply(reduction, duration)` replaces the current slow if the new one
/// has a higher `reduction` or longer remaining duration. `tick(dt)` advances
/// the timer and `clear()` removes it immediately (e.g. anti-slow effect).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Slow {
    /// Speed reduction fraction [0.0, 1.0].
    /// 0.0 = no slowdown, 0.5 = half speed, 1.0 = complete stop.
    pub reduction: f32,
    pub duration: f32,
    pub timer: f32,
    /// True on the first frame a slow is applied (or refreshed).
    pub just_slowed: bool,
    /// True on the first frame the slow expires.
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Slow {
    pub fn new() -> Self {
        Self {
            reduction: 0.0,
            duration: 0.0,
            timer: 0.0,
            just_slowed: false,
            just_recovered: false,
            enabled: true,
        }
    }

    /// Apply a slow. Replaces the existing slow if the new reduction is higher
    /// OR the new duration provides more remaining slow time.
    pub fn apply(&mut self, reduction: f32, duration: f32) {
        if !self.enabled {
            return;
        }
        let reduction = reduction.clamp(0.0, 1.0);
        let remaining = self.duration - self.timer;
        if reduction >= self.reduction || duration > remaining {
            self.reduction = reduction;
            self.duration = duration.max(0.0);
            self.timer = 0.0;
        }
        self.just_slowed = true;
    }

    /// Remove the slow immediately.
    pub fn clear(&mut self) {
        self.reduction = 0.0;
        self.duration = 0.0;
        self.timer = 0.0;
    }

    /// Advance the timer by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.just_slowed = false;
        self.just_recovered = false;

        if !self.is_active() {
            return;
        }

        self.timer += dt;
        if self.timer >= self.duration {
            self.timer = self.duration;
            self.reduction = 0.0;
            self.just_recovered = true;
        }
    }

    pub fn is_active(&self) -> bool {
        self.reduction > 0.0 && self.timer < self.duration
    }

    /// Speed multiplier to apply to movement (1.0 = no slow, 0.0 = stopped).
    pub fn effective_multiplier(&self) -> f32 {
        if self.is_active() {
            (1.0 - self.reduction).max(0.0)
        } else {
            1.0
        }
    }

    /// Remaining slow duration in seconds.
    pub fn remaining(&self) -> f32 {
        (self.duration - self.timer).max(0.0)
    }

    /// Fraction of the slow duration elapsed [0.0, 1.0].
    pub fn elapsed_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Slow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_starts_slow() {
        let mut s = Slow::new();
        s.apply(0.5, 3.0);
        assert!(s.is_active());
        assert!(s.just_slowed);
        assert!((s.reduction - 0.5).abs() < 1e-5);
    }

    #[test]
    fn stronger_slow_replaces_weaker() {
        let mut s = Slow::new();
        s.apply(0.3, 5.0);
        s.apply(0.7, 2.0);
        assert!((s.reduction - 0.7).abs() < 1e-5);
    }

    #[test]
    fn weaker_shorter_slow_does_not_replace() {
        let mut s = Slow::new();
        s.apply(0.7, 5.0);
        s.apply(0.3, 1.0);
        assert!((s.reduction - 0.7).abs() < 1e-5);
        assert_eq!(s.duration, 5.0);
    }

    #[test]
    fn tick_expires_slow() {
        let mut s = Slow::new();
        s.apply(0.5, 1.0);
        s.tick(1.1);
        assert!(!s.is_active());
        assert!(s.just_recovered);
    }

    #[test]
    fn clear_deactivates() {
        let mut s = Slow::new();
        s.apply(0.5, 5.0);
        s.clear();
        assert!(!s.is_active());
        assert!((s.effective_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_multiplier_halved() {
        let mut s = Slow::new();
        s.apply(0.5, 3.0);
        assert!((s.effective_multiplier() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn full_stop_at_reduction_one() {
        let mut s = Slow::new();
        s.apply(1.0, 3.0);
        assert!((s.effective_multiplier()).abs() < 1e-5);
    }

    #[test]
    fn disabled_ignores_apply() {
        let mut s = Slow::new();
        s.enabled = false;
        s.apply(0.5, 3.0);
        assert!(!s.is_active());
    }

    #[test]
    fn elapsed_fraction_correct() {
        let mut s = Slow::new();
        s.apply(0.5, 4.0);
        s.tick(1.0);
        let frac = s.elapsed_fraction();
        assert!((frac - 0.25).abs() < 1e-5);
    }
}

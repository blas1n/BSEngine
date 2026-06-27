use bevy_ecs::prelude::Component;

/// Adrenaline/electrical buff that reduces the time needed to execute actions
/// (attacks, casts, animations). The action system calls
/// `effective_duration(base_seconds)` to compress animation and cast timings
/// while the buff is active.
///
/// A `speed_multiplier` of 2.0 halves all action durations — a 1-second cast
/// becomes 0.5 seconds.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_worn_off` on expiry. `clear()` removes the buff early.
///
/// Distinct from `Haste` (movement speed only), `Surge` (raw damage burst),
/// and `Amplify` (scales output power): Galvanize compresses how long each
/// action takes, effectively increasing the rate at which the entity acts
/// without changing the power of individual actions.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Galvanize {
    pub duration: f32,
    pub timer: f32,
    /// Divisor applied to action durations while galvanized (>= 1.0).
    /// e.g. 2.0 = all actions complete twice as fast.
    pub speed_multiplier: f32,
    pub just_galvanized: bool,
    pub just_worn_off: bool,
    pub enabled: bool,
}

impl Galvanize {
    pub fn new(speed_multiplier: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            speed_multiplier: speed_multiplier.max(1.0),
            just_galvanized: false,
            just_worn_off: false,
            enabled: true,
        }
    }

    /// Apply or extend the galvanize buff for `duration` seconds. High-watermark:
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
                self.just_galvanized = true;
            }
        }
    }

    /// Remove the galvanize buff immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_worn_off = true;
        }
    }

    /// Advance the timer; sets `just_worn_off` when the buff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_galvanized = false;
        self.just_worn_off = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_worn_off = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Compress an action duration by the speed multiplier. Returns
    /// `base_seconds / speed_multiplier` while active, `base_seconds` otherwise.
    pub fn effective_duration(&self, base_seconds: f32) -> f32 {
        if self.is_active() {
            base_seconds / self.speed_multiplier
        } else {
            base_seconds
        }
    }

    /// Fraction of the galvanize duration remaining [1.0 = just applied, 0.0 = worn off].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Galvanize {
    fn default() -> Self {
        Self::new(1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_galvanize() {
        let mut g = Galvanize::new(2.0);
        g.apply(3.0);
        assert!(g.is_active());
        assert!(g.just_galvanized);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut g = Galvanize::new(2.0);
        g.apply(2.0);
        g.tick(0.016);
        g.apply(5.0);
        assert!((g.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut g = Galvanize::new(2.0);
        g.apply(5.0);
        g.apply(2.0);
        assert!((g.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_galvanize() {
        let mut g = Galvanize::new(2.0);
        g.apply(1.0);
        g.tick(1.1);
        assert!(!g.is_active());
        assert!(g.just_worn_off);
    }

    #[test]
    fn clear_ends_early() {
        let mut g = Galvanize::new(2.0);
        g.apply(5.0);
        g.clear();
        assert!(!g.is_active());
        assert!(g.just_worn_off);
    }

    #[test]
    fn effective_duration_compressed_while_active() {
        let mut g = Galvanize::new(2.0);
        g.apply(5.0);
        let compressed = g.effective_duration(1.0);
        assert!((compressed - 0.5).abs() < 1e-5); // 1.0 / 2.0
    }

    #[test]
    fn effective_duration_passthrough_when_inactive() {
        let g = Galvanize::new(2.0);
        assert!((g.effective_duration(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut g = Galvanize::new(2.0);
        g.apply(2.0);
        g.tick(1.0);
        assert!((g.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut g = Galvanize::new(2.0);
        g.enabled = false;
        g.apply(5.0);
        assert!(!g.is_active());
    }

    #[test]
    fn tick_clears_just_galvanized() {
        let mut g = Galvanize::new(2.0);
        g.apply(3.0);
        g.tick(0.016);
        assert!(!g.just_galvanized);
    }

    #[test]
    fn speed_multiplier_clamped_to_min_one() {
        let g = Galvanize::new(0.5);
        assert!((g.speed_multiplier - 1.0).abs() < 1e-5);
    }
}

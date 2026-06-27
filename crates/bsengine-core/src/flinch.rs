use bevy_ecs::prelude::Component;

/// Tracks brief involuntary hit-reaction animation triggers.
///
/// When an entity takes damage above `threshold`, `notify_hit(damage)` returns
/// `true` and sets `just_flinched` for a single frame. The animation system
/// reads this flag to play a short recoil clip without interrupting the entity's
/// actual actions (unlike `Interrupt`, which cancels active abilities).
///
/// `resistance` [0.0, 1.0] reduces effective incoming damage before comparing
/// to `threshold` — a resistant entity needs a harder hit to flinch.
/// `tick(dt)` clears the flag each frame.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Flinch {
    /// Minimum effective damage required to trigger a flinch.
    pub threshold: f32,
    /// Fraction [0.0, 1.0] that reduces effective damage before the check.
    /// 0.0 = no resistance; 1.0 = never flinches from damage alone.
    pub resistance: f32,
    /// True on the first frame a flinch fires.
    pub just_flinched: bool,
    /// Total flinches received.
    pub flinch_count: u32,
    pub enabled: bool,
}

impl Flinch {
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold: threshold.max(0.0),
            resistance: 0.0,
            just_flinched: false,
            flinch_count: 0,
            enabled: true,
        }
    }

    pub fn with_resistance(mut self, resistance: f32) -> Self {
        self.resistance = resistance.clamp(0.0, 1.0);
        self
    }

    /// Check whether `damage` triggers a flinch.
    ///
    /// Returns `true` when `enabled` and effective damage ≥ `threshold`.
    /// Sets `just_flinched` and increments `flinch_count` on success.
    pub fn notify_hit(&mut self, damage: f32) -> bool {
        if !self.enabled || self.threshold <= 0.0 {
            return false;
        }

        let effective = damage * (1.0 - self.resistance);
        if effective >= self.threshold {
            self.just_flinched = true;
            self.flinch_count += 1;
            return true;
        }

        false
    }

    /// Force a flinch regardless of damage (e.g. from an ability effect).
    pub fn force_flinch(&mut self) {
        if self.enabled {
            self.just_flinched = true;
            self.flinch_count += 1;
        }
    }

    /// Clear `just_flinched`; call once per frame.
    pub fn tick(&mut self, _dt: f32) {
        self.just_flinched = false;
    }
}

impl Default for Flinch {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_hit_above_threshold() {
        let mut f = Flinch::new(10.0);
        let flinched = f.notify_hit(15.0);
        assert!(flinched);
        assert!(f.just_flinched);
        assert_eq!(f.flinch_count, 1);
    }

    #[test]
    fn notify_hit_below_threshold() {
        let mut f = Flinch::new(10.0);
        let flinched = f.notify_hit(5.0);
        assert!(!flinched);
        assert!(!f.just_flinched);
    }

    #[test]
    fn notify_hit_at_exact_threshold() {
        let mut f = Flinch::new(10.0);
        let flinched = f.notify_hit(10.0);
        assert!(flinched);
    }

    #[test]
    fn resistance_reduces_effective_damage() {
        let mut f = Flinch::new(10.0).with_resistance(0.5);
        // effective = 15.0 * 0.5 = 7.5 < 10.0 → no flinch
        assert!(!f.notify_hit(15.0));
        // effective = 25.0 * 0.5 = 12.5 >= 10.0 → flinch
        assert!(f.notify_hit(25.0));
    }

    #[test]
    fn full_resistance_never_flinches() {
        let mut f = Flinch::new(1.0).with_resistance(1.0);
        assert!(!f.notify_hit(1_000_000.0));
    }

    #[test]
    fn tick_clears_just_flinched() {
        let mut f = Flinch::new(5.0);
        f.notify_hit(10.0);
        f.tick(0.016);
        assert!(!f.just_flinched);
    }

    #[test]
    fn force_flinch_bypasses_threshold() {
        let mut f = Flinch::new(1000.0);
        f.force_flinch();
        assert!(f.just_flinched);
        assert_eq!(f.flinch_count, 1);
    }

    #[test]
    fn disabled_no_flinch() {
        let mut f = Flinch::new(5.0);
        f.enabled = false;
        assert!(!f.notify_hit(100.0));
        assert!(!f.just_flinched);
    }

    #[test]
    fn flinch_count_accumulates() {
        let mut f = Flinch::new(5.0);
        f.notify_hit(10.0);
        f.tick(0.016);
        f.notify_hit(10.0);
        assert_eq!(f.flinch_count, 2);
    }
}

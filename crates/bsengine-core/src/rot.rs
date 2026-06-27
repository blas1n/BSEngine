use bevy_ecs::prelude::Component;

/// Progressive max-HP decay: while `is_rotting()`, the entity's maximum HP
/// decreases at `decay_rate` units per second, accumulating in
/// `total_decayed`. `effective_max_hp(base)` returns the reduced ceiling
/// (floored at 1.0 so the entity always has at least 1 max HP).
///
/// `infect()` starts the rot; `cleanse()` stops it without restoring lost
/// capacity; `restore()` stops it AND resets `total_decayed` to 0.
/// `tick(dt)` advances the decay and sets `just_capped` when `total_decayed`
/// reaches `decay_cap`. One-frame flags are cleared at the start of each tick.
///
/// Distinct from `Venom`/`Burn`/`Bleed` (direct damage over time) and
/// `Wither` (stat reduction): Rot **erodes maximum HP** — it silently shrinks
/// the health ceiling without dealing direct damage, threatening to push
/// current HP above the new cap on the next frame.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Rot {
    pub active: bool,
    /// Max HP units reduced per second while active. Clamped ≥ 0.0.
    pub decay_rate: f32,
    /// Cumulative max HP reduction applied so far. Clamped ≥ 0.0.
    pub total_decayed: f32,
    /// Ceiling on `total_decayed`. Clamped ≥ 0.0.
    pub decay_cap: f32,
    pub just_began: bool,
    pub just_capped: bool,
    pub enabled: bool,
}

impl Rot {
    pub fn new(decay_rate: f32, decay_cap: f32) -> Self {
        Self {
            active: false,
            decay_rate: decay_rate.max(0.0),
            total_decayed: 0.0,
            decay_cap: decay_cap.max(0.0),
            just_began: false,
            just_capped: false,
            enabled: true,
        }
    }

    /// Start the rot. Sets `just_began` on the inactive → active transition.
    /// No-op when disabled.
    pub fn infect(&mut self) {
        if !self.enabled {
            return;
        }
        if !self.active {
            self.active = true;
            self.just_began = true;
        }
    }

    /// Stop the rot without restoring `total_decayed`. The max HP reduction
    /// remains until `restore()` is called.
    pub fn cleanse(&mut self) {
        self.active = false;
    }

    /// Stop the rot and reset `total_decayed` to 0, fully recovering the lost
    /// max HP capacity.
    pub fn restore(&mut self) {
        self.active = false;
        self.total_decayed = 0.0;
        self.just_capped = false;
    }

    /// Advance the decay. Sets `just_capped` the frame `total_decayed` first
    /// reaches `decay_cap`. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_began = false;
        self.just_capped = false;

        if self.active && self.enabled && !self.is_capped() {
            let was_below = self.total_decayed < self.decay_cap;
            self.total_decayed += self.decay_rate * dt;
            if self.total_decayed >= self.decay_cap {
                self.total_decayed = self.decay_cap;
                if was_below {
                    self.just_capped = true;
                }
            }
        }
    }

    /// `true` when active, enabled, and not yet at the decay cap.
    pub fn is_rotting(&self) -> bool {
        self.active && self.enabled && !self.is_capped()
    }

    pub fn is_capped(&self) -> bool {
        self.total_decayed >= self.decay_cap
    }

    /// The entity's effective maximum HP after rot reduction.
    /// Floored at 1.0 so max HP never reaches zero.
    pub fn effective_max_hp(&self, base_max_hp: f32) -> f32 {
        (base_max_hp - self.total_decayed).max(1.0)
    }

    /// Fraction of `base_max_hp` that has been decayed away [0.0, 1.0].
    pub fn decayed_fraction(&self, base_max_hp: f32) -> f32 {
        if base_max_hp <= 0.0 {
            return 0.0;
        }
        (self.total_decayed / base_max_hp).clamp(0.0, 1.0)
    }
}

impl Default for Rot {
    fn default() -> Self {
        Self::new(5.0, 50.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infect_starts_rot() {
        let mut r = Rot::new(5.0, 50.0);
        r.infect();
        assert!(r.active);
        assert!(r.just_began);
    }

    #[test]
    fn infect_no_op_when_disabled() {
        let mut r = Rot::new(5.0, 50.0);
        r.enabled = false;
        r.infect();
        assert!(!r.active);
    }

    #[test]
    fn just_began_not_set_on_re_infect() {
        let mut r = Rot::new(5.0, 50.0);
        r.infect();
        r.tick(0.016);
        r.infect();
        assert!(!r.just_began);
    }

    #[test]
    fn cleanse_stops_rot_without_restoring() {
        let mut r = Rot::new(5.0, 50.0);
        r.infect();
        r.tick(1.0);
        let decayed = r.total_decayed;
        r.cleanse();
        assert!(!r.active);
        assert!((r.total_decayed - decayed).abs() < 1e-5);
    }

    #[test]
    fn restore_stops_and_resets() {
        let mut r = Rot::new(5.0, 50.0);
        r.infect();
        r.tick(2.0);
        r.restore();
        assert!(!r.active);
        assert_eq!(r.total_decayed, 0.0);
    }

    #[test]
    fn tick_advances_decay() {
        let mut r = Rot::new(5.0, 50.0);
        r.infect();
        r.tick(2.0);
        assert!((r.total_decayed - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_clears_just_began() {
        let mut r = Rot::new(5.0, 50.0);
        r.infect();
        r.tick(0.016);
        assert!(!r.just_began);
    }

    #[test]
    fn tick_clamps_at_decay_cap() {
        let mut r = Rot::new(5.0, 10.0);
        r.infect();
        r.tick(10.0);
        assert!((r.total_decayed - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_sets_just_capped_on_crossing() {
        let mut r = Rot::new(5.0, 10.0);
        r.infect();
        r.tick(2.1);
        assert!(r.just_capped);
    }

    #[test]
    fn tick_no_just_capped_on_subsequent_ticks() {
        let mut r = Rot::new(5.0, 10.0);
        r.infect();
        r.tick(2.1);
        r.tick(0.016);
        assert!(!r.just_capped);
    }

    #[test]
    fn tick_no_advance_when_inactive() {
        let mut r = Rot::new(5.0, 50.0);
        r.tick(5.0);
        assert_eq!(r.total_decayed, 0.0);
    }

    #[test]
    fn tick_no_advance_when_capped() {
        let mut r = Rot::new(5.0, 10.0);
        r.infect();
        r.tick(10.0);
        r.tick(10.0);
        assert!((r.total_decayed - 10.0).abs() < 1e-5);
    }

    #[test]
    fn is_rotting_false_when_capped() {
        let mut r = Rot::new(5.0, 10.0);
        r.infect();
        r.tick(3.0);
        assert!(!r.is_rotting());
        assert!(r.is_capped());
    }

    #[test]
    fn is_rotting_false_when_disabled() {
        let mut r = Rot::new(5.0, 50.0);
        r.infect();
        r.enabled = false;
        assert!(!r.is_rotting());
    }

    #[test]
    fn effective_max_hp_reduced_by_decay() {
        let mut r = Rot::new(5.0, 50.0);
        r.infect();
        r.tick(4.0); // 20 decayed
        assert!((r.effective_max_hp(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_max_hp_floored_at_one() {
        let mut r = Rot::new(5.0, 200.0);
        r.infect();
        r.tick(100.0); // 500 decay but cap 200; base 100 → 100 - 200 = negative
        assert!((r.effective_max_hp(100.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn decayed_fraction_at_half() {
        let mut r = Rot::new(5.0, 50.0);
        r.infect();
        r.tick(10.0); // 50 decayed, base 100 → 0.5
        assert!((r.decayed_fraction(100.0) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn decayed_fraction_zero_with_no_decay() {
        let r = Rot::new(5.0, 50.0);
        assert!((r.decayed_fraction(100.0)).abs() < 1e-5);
    }
}

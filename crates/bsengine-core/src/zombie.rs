use bevy_ecs::prelude::Component;

/// Undead-momentum tracker. `shamble` builds via `lurch(amount)` and rots
/// passively at `rot_rate` per second in `tick(dt)` or immediately via
/// `decay(amount)`.
///
/// Models undead persistence, reanimation meters, lingering-threat gauges,
/// shambling-enemy escalation, horror-creature charge, or any mechanic
/// where something that should be dead keeps building momentum before
/// finally collapsing.
///
/// `lurch(amount)` adds shamble; fires `just_risen` when first reaching
/// `max_shamble`. No-op when disabled.
///
/// `decay(amount)` reduces shamble immediately; fires `just_decayed` when
/// reaching 0. No-op when disabled or already decayed.
///
/// `tick(dt)` clears both flags, then rots shamble by `rot_rate * dt`
/// (floored at 0). Fires `just_decayed` when reaching 0 via rot. No-op
/// when disabled or rate is 0.
///
/// `is_risen()` returns `shamble >= max_shamble && enabled`.
///
/// `is_decayed()` returns `shamble == 0.0` (not gated by `enabled`).
///
/// `shamble_fraction()` returns `(shamble / max_shamble).clamp(0, 1)`.
///
/// `effective_menace(scale)` returns `scale * shamble_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 8.0)` — rots at 8 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zombie {
    pub shamble: f32,
    pub max_shamble: f32,
    pub rot_rate: f32,
    pub just_risen: bool,
    pub just_decayed: bool,
    pub enabled: bool,
}

impl Zombie {
    pub fn new(max_shamble: f32, rot_rate: f32) -> Self {
        Self {
            shamble: 0.0,
            max_shamble: max_shamble.max(0.1),
            rot_rate: rot_rate.max(0.0),
            just_risen: false,
            just_decayed: false,
            enabled: true,
        }
    }

    /// Add shamble; fires `just_risen` when first reaching max.
    /// No-op when disabled.
    pub fn lurch(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.shamble < self.max_shamble;
        self.shamble = (self.shamble + amount).min(self.max_shamble);
        if was_below && self.shamble >= self.max_shamble {
            self.just_risen = true;
        }
    }

    /// Reduce shamble; fires `just_decayed` when reaching 0.
    /// No-op when disabled or already decayed.
    pub fn decay(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.shamble <= 0.0 {
            return;
        }
        self.shamble = (self.shamble - amount).max(0.0);
        if self.shamble <= 0.0 {
            self.just_decayed = true;
        }
    }

    /// Clear flags, then rot shamble by `rot_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_risen = false;
        self.just_decayed = false;
        if self.enabled && self.rot_rate > 0.0 && self.shamble > 0.0 {
            self.shamble = (self.shamble - self.rot_rate * dt).max(0.0);
            if self.shamble <= 0.0 {
                self.just_decayed = true;
            }
        }
    }

    /// `true` when shamble is at maximum and component is enabled.
    pub fn is_risen(&self) -> bool {
        self.shamble >= self.max_shamble && self.enabled
    }

    /// `true` when shamble is 0 (not gated by `enabled`).
    pub fn is_decayed(&self) -> bool {
        self.shamble == 0.0
    }

    /// Fraction of maximum shamble [0.0, 1.0].
    pub fn shamble_fraction(&self) -> f32 {
        (self.shamble / self.max_shamble).clamp(0.0, 1.0)
    }

    /// Returns `scale * shamble_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_menace(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.shamble_fraction()
    }
}

impl Default for Zombie {
    fn default() -> Self {
        Self::new(100.0, 8.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zombie {
        Zombie::new(100.0, 8.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_decayed() {
        let z = z();
        assert_eq!(z.shamble, 0.0);
        assert!(z.is_decayed());
        assert!(!z.is_risen());
    }

    #[test]
    fn new_clamps_max_shamble() {
        let z = Zombie::new(-5.0, 8.0);
        assert!((z.max_shamble - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_rot_rate() {
        let z = Zombie::new(100.0, -3.0);
        assert_eq!(z.rot_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zombie::default();
        assert!((z.max_shamble - 100.0).abs() < 1e-5);
        assert!((z.rot_rate - 8.0).abs() < 1e-5);
    }

    // --- lurch ---

    #[test]
    fn lurch_adds_shamble() {
        let mut z = z();
        z.lurch(40.0);
        assert!((z.shamble - 40.0).abs() < 1e-3);
    }

    #[test]
    fn lurch_clamps_at_max() {
        let mut z = z();
        z.lurch(200.0);
        assert!((z.shamble - 100.0).abs() < 1e-3);
    }

    #[test]
    fn lurch_fires_just_risen_at_max() {
        let mut z = z();
        z.lurch(100.0);
        assert!(z.just_risen);
        assert!(z.is_risen());
    }

    #[test]
    fn lurch_no_just_risen_when_already_at_max() {
        let mut z = z();
        z.shamble = 100.0;
        z.lurch(10.0);
        assert!(!z.just_risen);
    }

    #[test]
    fn lurch_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.lurch(50.0);
        assert_eq!(z.shamble, 0.0);
    }

    #[test]
    fn lurch_no_op_when_amount_zero() {
        let mut z = z();
        z.lurch(0.0);
        assert_eq!(z.shamble, 0.0);
    }

    // --- decay ---

    #[test]
    fn decay_reduces_shamble() {
        let mut z = z();
        z.shamble = 60.0;
        z.decay(20.0);
        assert!((z.shamble - 40.0).abs() < 1e-3);
    }

    #[test]
    fn decay_clamps_at_zero() {
        let mut z = z();
        z.shamble = 30.0;
        z.decay(200.0);
        assert_eq!(z.shamble, 0.0);
    }

    #[test]
    fn decay_fires_just_decayed_at_zero() {
        let mut z = z();
        z.shamble = 30.0;
        z.decay(30.0);
        assert!(z.just_decayed);
    }

    #[test]
    fn decay_no_op_when_already_decayed() {
        let mut z = z();
        z.decay(10.0);
        assert!(!z.just_decayed);
    }

    #[test]
    fn decay_no_op_when_disabled() {
        let mut z = z();
        z.shamble = 50.0;
        z.enabled = false;
        z.decay(50.0);
        assert!((z.shamble - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_rots_shamble() {
        let mut z = z(); // rot=8
        z.shamble = 60.0;
        z.tick(1.0); // 60 - 8 = 52
        assert!((z.shamble - 52.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_decayed_on_rot_to_zero() {
        let mut z = Zombie::new(100.0, 200.0);
        z.shamble = 5.0;
        z.tick(1.0);
        assert!(z.just_decayed);
        assert!(z.is_decayed());
    }

    #[test]
    fn tick_no_rot_when_already_decayed() {
        let mut z = z();
        z.tick(10.0);
        assert!(!z.just_decayed);
    }

    #[test]
    fn tick_no_rot_when_rate_zero() {
        let mut z = Zombie::new(100.0, 0.0);
        z.shamble = 50.0;
        z.tick(100.0);
        assert!((z.shamble - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_rot_when_disabled() {
        let mut z = z();
        z.shamble = 50.0;
        z.enabled = false;
        z.tick(1.0);
        assert!((z.shamble - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_risen() {
        let mut z = z();
        z.lurch(100.0);
        z.tick(0.016);
        assert!(!z.just_risen);
    }

    #[test]
    fn tick_clears_just_decayed() {
        let mut z = Zombie::new(100.0, 200.0);
        z.shamble = 5.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_decayed);
    }

    #[test]
    fn tick_scales_rot_with_dt() {
        let mut z = z(); // rot=8
        z.shamble = 100.0;
        z.tick(3.0); // 100 - 8*3 = 76
        assert!((z.shamble - 76.0).abs() < 1e-3);
    }

    // --- is_risen / is_decayed ---

    #[test]
    fn is_risen_false_when_disabled() {
        let mut z = z();
        z.shamble = 100.0;
        z.enabled = false;
        assert!(!z.is_risen());
    }

    #[test]
    fn is_decayed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_decayed());
    }

    // --- shamble_fraction / effective_menace ---

    #[test]
    fn shamble_fraction_zero_when_decayed() {
        assert_eq!(z().shamble_fraction(), 0.0);
    }

    #[test]
    fn shamble_fraction_half_at_midpoint() {
        let mut z = z();
        z.shamble = 50.0;
        assert!((z.shamble_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_menace_zero_when_decayed() {
        assert_eq!(z().effective_menace(100.0), 0.0);
    }

    #[test]
    fn effective_menace_scales_with_shamble() {
        let mut z = z();
        z.shamble = 80.0;
        assert!((z.effective_menace(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_menace_zero_when_disabled() {
        let mut z = z();
        z.shamble = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_menace(100.0), 0.0);
    }
}

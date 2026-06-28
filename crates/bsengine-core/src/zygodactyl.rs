use bevy_ecs::prelude::Component;

/// Opposable-grip accumulation tracker named after zygodactyl, the
/// adjective describing feet in which two toes point forward and two
/// point backward — an arrangement that gives climbing and perching birds
/// a vice-like grip on branches, prey, and vertical surfaces. The
/// configuration is convergently evolved across bird orders: parrots
/// (Psittaciformes) depend on it to manipulate food with one foot while
/// gripping a branch with the other; woodpeckers (Picidae) use it to
/// press their bodies against tree trunks while their chisel bills hammer
/// through bark; cuckoos (Cuculidae) rely on it for clambering through
/// dense vegetation; and ospreys (Pandionidae) deploy it as they strike
/// the water to seize fish, all four toes snapping shut around the
/// slippery body before the bird reverses the toe arrangement mid-air to
/// carry the fish head-forward. The anatomical mechanism is a reversal of
/// the fourth toe from the standard anisodactyl arrangement — three
/// forward, one back — by rotating at the ankle joint, converting the
/// foot from a simple perching hand into a double-sided grapple. `grip`
/// builds via `grasp(amount)` and accumulates passively at `lock_rate`
/// per second in `tick(dt)` or diminishes via `release(amount)`.
///
/// Models zygodactyl-grip fill levels, perching-force saturation bars,
/// vice-grip accumulation trackers, avian-predator-clasp gauges,
/// climbing-hold-strength fill levels, opposable-toe-lock saturation
/// indicators, bark-clinging intensity accumulation bars, prey-grip
/// force meters, arboreal-manoeuvre grip fill levels, or any mechanic
/// where a character, creature, or device slowly brings a set of opposing
/// clamps to their maximum closing force — vice-jaws tightening around
/// a target, magnetic claws locking onto a hull, grappling hooks sinking
/// into stone — until the grip is secure enough to bear any load.
///
/// `grasp(amount)` adds grip; fires `just_grasped` when first reaching
/// `max_grip`. No-op when disabled.
///
/// `release(amount)` reduces grip immediately; fires `just_released`
/// when reaching 0. No-op when disabled or already released.
///
/// `tick(dt)` clears both flags, then increases grip by
/// `lock_rate * dt` (capped at `max_grip`). Fires `just_grasped`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_grasped()` returns `grip >= max_grip && enabled`.
///
/// `is_released()` returns `grip == 0.0` (not gated by `enabled`).
///
/// `grip_fraction()` returns `(grip / max_grip).clamp(0, 1)`.
///
/// `effective_perch(scale)` returns `scale * grip_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — locks at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zygodactyl {
    pub grip: f32,
    pub max_grip: f32,
    pub lock_rate: f32,
    pub just_grasped: bool,
    pub just_released: bool,
    pub enabled: bool,
}

impl Zygodactyl {
    pub fn new(max_grip: f32, lock_rate: f32) -> Self {
        Self {
            grip: 0.0,
            max_grip: max_grip.max(0.1),
            lock_rate: lock_rate.max(0.0),
            just_grasped: false,
            just_released: false,
            enabled: true,
        }
    }

    /// Add grip; fires `just_grasped` when first reaching max.
    /// No-op when disabled.
    pub fn grasp(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.grip < self.max_grip;
        self.grip = (self.grip + amount).min(self.max_grip);
        if was_below && self.grip >= self.max_grip {
            self.just_grasped = true;
        }
    }

    /// Reduce grip; fires `just_released` when reaching 0.
    /// No-op when disabled or already released.
    pub fn release(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.grip <= 0.0 {
            return;
        }
        self.grip = (self.grip - amount).max(0.0);
        if self.grip <= 0.0 {
            self.just_released = true;
        }
    }

    /// Clear flags, then increase grip by `lock_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_grasped = false;
        self.just_released = false;
        if self.enabled && self.lock_rate > 0.0 && self.grip < self.max_grip {
            let was_below = self.grip < self.max_grip;
            self.grip = (self.grip + self.lock_rate * dt).min(self.max_grip);
            if was_below && self.grip >= self.max_grip {
                self.just_grasped = true;
            }
        }
    }

    /// `true` when grip is at maximum and component is enabled.
    pub fn is_grasped(&self) -> bool {
        self.grip >= self.max_grip && self.enabled
    }

    /// `true` when grip is 0 (not gated by `enabled`).
    pub fn is_released(&self) -> bool {
        self.grip == 0.0
    }

    /// Fraction of maximum grip [0.0, 1.0].
    pub fn grip_fraction(&self) -> f32 {
        (self.grip / self.max_grip).clamp(0.0, 1.0)
    }

    /// Returns `scale * grip_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_perch(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.grip_fraction()
    }
}

impl Default for Zygodactyl {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zygodactyl {
        Zygodactyl::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_released() {
        let z = z();
        assert_eq!(z.grip, 0.0);
        assert!(z.is_released());
        assert!(!z.is_grasped());
    }

    #[test]
    fn new_clamps_max_grip() {
        let z = Zygodactyl::new(-5.0, 1.5);
        assert!((z.max_grip - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_lock_rate() {
        let z = Zygodactyl::new(100.0, -1.5);
        assert_eq!(z.lock_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zygodactyl::default();
        assert!((z.max_grip - 100.0).abs() < 1e-5);
        assert!((z.lock_rate - 1.5).abs() < 1e-5);
    }

    // --- grasp ---

    #[test]
    fn grasp_adds_grip() {
        let mut z = z();
        z.grasp(40.0);
        assert!((z.grip - 40.0).abs() < 1e-3);
    }

    #[test]
    fn grasp_clamps_at_max() {
        let mut z = z();
        z.grasp(200.0);
        assert!((z.grip - 100.0).abs() < 1e-3);
    }

    #[test]
    fn grasp_fires_just_grasped_at_max() {
        let mut z = z();
        z.grasp(100.0);
        assert!(z.just_grasped);
        assert!(z.is_grasped());
    }

    #[test]
    fn grasp_no_just_grasped_when_already_at_max() {
        let mut z = z();
        z.grip = 100.0;
        z.grasp(10.0);
        assert!(!z.just_grasped);
    }

    #[test]
    fn grasp_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.grasp(50.0);
        assert_eq!(z.grip, 0.0);
    }

    #[test]
    fn grasp_no_op_when_amount_zero() {
        let mut z = z();
        z.grasp(0.0);
        assert_eq!(z.grip, 0.0);
    }

    // --- release ---

    #[test]
    fn release_reduces_grip() {
        let mut z = z();
        z.grip = 60.0;
        z.release(20.0);
        assert!((z.grip - 40.0).abs() < 1e-3);
    }

    #[test]
    fn release_clamps_at_zero() {
        let mut z = z();
        z.grip = 30.0;
        z.release(200.0);
        assert_eq!(z.grip, 0.0);
    }

    #[test]
    fn release_fires_just_released_at_zero() {
        let mut z = z();
        z.grip = 30.0;
        z.release(30.0);
        assert!(z.just_released);
    }

    #[test]
    fn release_no_op_when_already_released() {
        let mut z = z();
        z.release(10.0);
        assert!(!z.just_released);
    }

    #[test]
    fn release_no_op_when_disabled() {
        let mut z = z();
        z.grip = 50.0;
        z.enabled = false;
        z.release(50.0);
        assert!((z.grip - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_locks_grip() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.grip - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_grasped_on_lock_to_max() {
        let mut z = Zygodactyl::new(100.0, 200.0);
        z.grip = 95.0;
        z.tick(1.0);
        assert!(z.just_grasped);
        assert!(z.is_grasped());
    }

    #[test]
    fn tick_no_lock_when_already_grasped() {
        let mut z = z();
        z.grip = 100.0;
        z.tick(1.0);
        assert!(!z.just_grasped);
    }

    #[test]
    fn tick_no_lock_when_rate_zero() {
        let mut z = Zygodactyl::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.grip, 0.0);
    }

    #[test]
    fn tick_no_lock_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.grip, 0.0);
    }

    #[test]
    fn tick_clears_just_grasped() {
        let mut z = Zygodactyl::new(100.0, 200.0);
        z.grip = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_grasped);
    }

    #[test]
    fn tick_clears_just_released() {
        let mut z = z();
        z.grip = 10.0;
        z.release(10.0);
        z.tick(0.016);
        assert!(!z.just_released);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.grip - 9.0).abs() < 1e-3);
    }

    // --- is_grasped / is_released ---

    #[test]
    fn is_grasped_false_when_disabled() {
        let mut z = z();
        z.grip = 100.0;
        z.enabled = false;
        assert!(!z.is_grasped());
    }

    #[test]
    fn is_released_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_released());
    }

    // --- grip_fraction / effective_perch ---

    #[test]
    fn grip_fraction_zero_when_released() {
        assert_eq!(z().grip_fraction(), 0.0);
    }

    #[test]
    fn grip_fraction_half_at_midpoint() {
        let mut z = z();
        z.grip = 50.0;
        assert!((z.grip_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_perch_zero_when_released() {
        assert_eq!(z().effective_perch(100.0), 0.0);
    }

    #[test]
    fn effective_perch_scales_with_grip() {
        let mut z = z();
        z.grip = 75.0;
        assert!((z.effective_perch(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_perch_zero_when_disabled() {
        let mut z = z();
        z.grip = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_perch(100.0), 0.0);
    }
}

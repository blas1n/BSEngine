use bevy_ecs::prelude::Component;

/// Death-prevention buff that allows the entity to survive lethal damage.
///
/// While grace is active, a lethal hit (one that would reduce HP to ≤ 0) should
/// instead be intercepted by the damage pipeline, which calls `trigger()` to
/// consume a charge. If `trigger()` returns `true`, the entity survives with
/// at least `min_hp_fraction` of their maximum HP rather than dying.
///
/// `apply(duration, charges)` refreshes grace, taking the maximum of the new
/// and current timer and charges. `tick(dt)` counts down and sets `just_expired`
/// if the timer runs out while charges remain.
///
/// Distinct from `Invincible` (full, continuous immunity), `Absorption`
/// (shields a specific HP pool), and `Revive` (resurrection after death):
/// Grace is a single-trigger cheat-death mechanic — it activates only on a
/// killing blow and is consumed in the process.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Grace {
    pub duration: f32,
    pub timer: f32,
    /// Number of lethal hits the entity can survive while grace is active.
    pub charges: u32,
    /// Minimum HP fraction left after a grace trigger, clamped to (0.0, 1.0].
    pub min_hp_fraction: f32,
    pub just_granted: bool,
    pub just_triggered: bool,
    pub just_expired: bool,
    pub enabled: bool,
}

impl Grace {
    pub fn new(charges: u32, min_hp_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            charges: 0,
            min_hp_fraction: min_hp_fraction.clamp(0.001, 1.0),
            just_granted: false,
            just_triggered: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Grant or refresh grace. Takes the greater of the new vs current timer
    /// and the sum of current + new charges (refreshing existing charges
    /// stacks up). No-op when disabled.
    pub fn apply(&mut self, duration: f32, charges: u32) {
        if !self.enabled {
            return;
        }

        let was_active = self.is_active();
        if duration > self.timer {
            self.duration = duration;
            self.timer = duration;
        }
        self.charges += charges;
        if !was_active && self.is_active() {
            self.just_granted = true;
        }
    }

    /// Attempt to trigger grace on a lethal hit. Consumes one charge and sets
    /// `just_triggered`. Returns `true` if grace activates (was active), `false`
    /// if there was no grace to spend (damage pipeline should proceed to death).
    pub fn trigger(&mut self) -> bool {
        if !self.is_active() {
            return false;
        }
        self.charges -= 1;
        self.just_triggered = true;
        if self.charges == 0 {
            self.timer = 0.0;
            self.duration = 0.0;
        }
        true
    }

    /// Remove grace immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.charges = 0;
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_expired = true;
        }
    }

    /// Advance the timer; sets `just_expired` when time runs out while charges
    /// remain. Does not fire if charges are already 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_granted = false;
        self.just_triggered = false;
        self.just_expired = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                if self.charges > 0 {
                    self.charges = 0;
                    self.just_expired = true;
                }
            }
        }
    }

    /// Grace is active when there are charges remaining and the timer has not expired.
    pub fn is_active(&self) -> bool {
        self.charges > 0 && self.timer > 0.0
    }

    /// HP fraction to enforce after a grace trigger. Equal to `min_hp_fraction`.
    pub fn survival_hp_fraction(&self) -> f32 {
        self.min_hp_fraction
    }

    /// Fraction of the grace duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Grace {
    fn default() -> Self {
        let mut g = Self::new(1, 0.01);
        g.charges = 0; // starts inactive
        g
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_grace() {
        let mut g = Grace::new(1, 0.01);
        g.apply(5.0, 1);
        assert!(g.is_active());
        assert!(g.just_granted);
        assert_eq!(g.charges, 1);
    }

    #[test]
    fn apply_stacks_charges() {
        let mut g = Grace::new(1, 0.01);
        g.apply(5.0, 1);
        g.tick(0.016);
        g.apply(5.0, 2);
        assert_eq!(g.charges, 3);
    }

    #[test]
    fn apply_extends_timer_to_max() {
        let mut g = Grace::new(1, 0.01);
        g.apply(3.0, 1);
        g.apply(7.0, 0); // longer timer wins
        assert!((g.timer - 7.0).abs() < 1e-4);
    }

    #[test]
    fn apply_does_not_shrink_timer() {
        let mut g = Grace::new(1, 0.01);
        g.apply(7.0, 1);
        g.apply(3.0, 0); // shorter timer ignored
        assert!((g.timer - 7.0).abs() < 1e-4);
    }

    #[test]
    fn trigger_consumes_charge() {
        let mut g = Grace::new(1, 0.01);
        g.apply(5.0, 1);
        let saved = g.trigger();
        assert!(saved);
        assert!(!g.is_active());
        assert!(g.just_triggered);
    }

    #[test]
    fn trigger_on_multi_charge_leaves_remaining() {
        let mut g = Grace::new(1, 0.01);
        g.apply(5.0, 3);
        g.trigger();
        assert!(g.is_active());
        assert_eq!(g.charges, 2);
    }

    #[test]
    fn trigger_returns_false_when_inactive() {
        let mut g = Grace::new(1, 0.01);
        let saved = g.trigger();
        assert!(!saved);
    }

    #[test]
    fn tick_expires_grace() {
        let mut g = Grace::new(1, 0.01);
        g.apply(1.0, 1);
        g.tick(1.1);
        assert!(!g.is_active());
        assert!(g.just_expired);
    }

    #[test]
    fn tick_no_expiry_event_when_charges_zero() {
        let mut g = Grace::new(1, 0.01);
        g.apply(1.0, 1);
        g.trigger(); // consumes charge
        g.tick(0.016);
        g.tick(2.0); // timer runs out but charges already 0
        assert!(!g.just_expired);
    }

    #[test]
    fn clear_ends_grace() {
        let mut g = Grace::new(1, 0.01);
        g.apply(5.0, 2);
        g.clear();
        assert!(!g.is_active());
        assert!(g.just_expired);
        assert_eq!(g.charges, 0);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut g = Grace::new(1, 0.01);
        g.apply(4.0, 1);
        g.tick(2.0);
        assert!((g.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut g = Grace::new(1, 0.01);
        g.enabled = false;
        g.apply(5.0, 1);
        assert!(!g.is_active());
    }

    #[test]
    fn survival_hp_fraction_clamped() {
        let g = Grace::new(1, -0.1);
        assert!(g.min_hp_fraction > 0.0);
        let g2 = Grace::new(1, 1.5);
        assert!((g2.min_hp_fraction - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_granted() {
        let mut g = Grace::new(1, 0.01);
        g.apply(5.0, 1);
        g.tick(0.016);
        assert!(!g.just_granted);
    }
}

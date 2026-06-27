use bevy_ecs::prelude::Component;

/// Kill-triggered burst state: entity deals bonus damage and attacks faster
/// for a limited window after eliminating a target.
///
/// `trigger(duration)` starts or extends the burst using a high-watermark:
/// only replaces the current timer when `duration > timer`. Fires
/// `just_triggered` on the inactive → active transition. No-op when disabled
/// or `duration ≤ 0`.
///
/// `tick(dt)` counts down and fires `just_expired` on natural expiry.
/// One-frame flags are cleared at the start of each `tick` call.
///
/// `effective_damage(base)` and `effective_attack_speed(base)` apply their
/// respective bonuses only while `is_ravaging()`.
///
/// Distinct from `Fervor` (stacks on any hit), `Rage` (continuous anger
/// resource), `Reckless` (permanent on/off trade-off), and `Rampage`
/// (territory-based multiplicative damage): Ravage is a **kill-triggered
/// burst state** — entity enters a blood-fuelled frenzy specifically after
/// eliminating a target, not on every hit.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ravage {
    pub active: bool,
    pub timer: f32,
    /// Current burst window's duration (used for `remaining_fraction`).
    pub duration: f32,
    /// Bonus damage fraction while ravaging. Clamped ≥ 0.0.
    pub damage_bonus: f32,
    /// Bonus attack speed fraction while ravaging. Clamped ≥ 0.0.
    pub attack_speed_bonus: f32,
    pub just_triggered: bool,
    pub just_expired: bool,
    pub enabled: bool,
}

impl Ravage {
    pub fn new(damage_bonus: f32, attack_speed_bonus: f32) -> Self {
        Self {
            active: false,
            timer: 0.0,
            duration: 0.0,
            damage_bonus: damage_bonus.max(0.0),
            attack_speed_bonus: attack_speed_bonus.max(0.0),
            just_triggered: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Start or extend the ravage burst for `duration` seconds.
    /// High-watermark: only replaces the current timer when
    /// `duration > timer`. Fires `just_triggered` on the inactive → active
    /// transition. No-op when disabled or `duration ≤ 0`.
    pub fn trigger(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_active = self.active;
            self.timer = duration;
            self.duration = duration;
            self.active = true;
            if !was_active {
                self.just_triggered = true;
            }
        }
    }

    /// Advance the burst countdown. Fires `just_expired` when the timer
    /// reaches 0. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_triggered = false;
        self.just_expired = false;

        if self.active && self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.active = false;
                self.just_expired = true;
            }
        }
    }

    /// `true` while the burst is active and the component is enabled.
    pub fn is_ravaging(&self) -> bool {
        self.active && self.enabled
    }

    /// Outgoing damage with ravage bonus applied.
    /// Returns `base * (1 + damage_bonus)` while ravaging; `base` otherwise.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_ravaging() {
            base * (1.0 + self.damage_bonus)
        } else {
            base
        }
    }

    /// Attack speed with ravage bonus applied.
    /// Returns `base * (1 + attack_speed_bonus)` while ravaging; `base`
    /// otherwise.
    pub fn effective_attack_speed(&self, base: f32) -> f32 {
        if self.is_ravaging() {
            base * (1.0 + self.attack_speed_bonus)
        } else {
            base
        }
    }

    /// Fraction of the burst window remaining [1.0 = just triggered, 0.0 =
    /// expired or not active].
    pub fn remaining_fraction(&self) -> f32 {
        if !self.active || self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Ravage {
    fn default() -> Self {
        Self::new(0.3, 0.25)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_not_ravaging() {
        let r = Ravage::new(0.3, 0.25);
        assert!(!r.is_ravaging());
        assert_eq!(r.timer, 0.0);
    }

    #[test]
    fn trigger_activates() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(5.0);
        assert!(r.is_ravaging());
        assert!(r.just_triggered);
        assert!((r.timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn trigger_extends_on_longer_duration() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(3.0);
        r.tick(1.0);
        r.trigger(5.0);
        assert!((r.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn trigger_no_extend_on_shorter_duration() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(5.0);
        r.trigger(2.0);
        assert!((r.timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn just_triggered_not_set_on_extend() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(2.0);
        r.tick(0.016);
        r.trigger(5.0);
        assert!(!r.just_triggered);
    }

    #[test]
    fn trigger_no_op_when_disabled() {
        let mut r = Ravage::new(0.3, 0.25);
        r.enabled = false;
        r.trigger(5.0);
        assert!(!r.active);
    }

    #[test]
    fn trigger_no_op_when_duration_zero() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(0.0);
        assert!(!r.active);
    }

    #[test]
    fn trigger_no_op_when_duration_negative() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(-1.0);
        assert!(!r.active);
    }

    #[test]
    fn tick_counts_down() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(5.0);
        r.tick(2.0);
        assert!((r.timer - 3.0).abs() < 1e-4);
        assert!(r.is_ravaging());
    }

    #[test]
    fn tick_expires_burst() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(2.0);
        r.tick(2.5);
        assert!(!r.is_ravaging());
        assert!(r.just_expired);
        assert_eq!(r.timer, 0.0);
    }

    #[test]
    fn tick_clears_just_triggered() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(5.0);
        r.tick(0.016);
        assert!(!r.just_triggered);
    }

    #[test]
    fn tick_clears_just_expired() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(1.0);
        r.tick(2.0); // expires
        r.tick(0.016);
        assert!(!r.just_expired);
    }

    #[test]
    fn is_ravaging_false_when_disabled() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(5.0);
        r.enabled = false;
        assert!(!r.is_ravaging());
    }

    #[test]
    fn effective_damage_applies_bonus() {
        let mut r = Ravage::new(0.5, 0.25);
        r.trigger(5.0);
        // 100 * (1 + 0.5) = 150
        assert!((r.effective_damage(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_base_when_not_ravaging() {
        let r = Ravage::new(0.5, 0.25);
        assert!((r.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_attack_speed_applies_bonus() {
        let mut r = Ravage::new(0.3, 0.5);
        r.trigger(5.0);
        // 100 * (1 + 0.5) = 150
        assert!((r.effective_attack_speed(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_attack_speed_base_when_not_ravaging() {
        let r = Ravage::new(0.3, 0.5);
        assert!((r.effective_attack_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_full_on_trigger() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(4.0);
        assert!((r.remaining_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(4.0);
        r.tick(2.0);
        assert!((r.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_not_ravaging() {
        let r = Ravage::new(0.3, 0.25);
        assert!((r.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn damage_bonus_clamped_non_negative() {
        let r = Ravage::new(-0.5, 0.25);
        assert_eq!(r.damage_bonus, 0.0);
    }

    #[test]
    fn attack_speed_bonus_clamped_non_negative() {
        let r = Ravage::new(0.3, -0.25);
        assert_eq!(r.attack_speed_bonus, 0.0);
    }

    #[test]
    fn can_re_trigger_after_expiry() {
        let mut r = Ravage::new(0.3, 0.25);
        r.trigger(1.0);
        r.tick(2.0); // expires
        r.tick(0.016); // clear flags
        r.trigger(3.0);
        assert!(r.is_ravaging());
        assert!(r.just_triggered);
    }
}

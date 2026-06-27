use bevy_ecs::prelude::Component;

/// Ally-damage-triggered retaliatory offence burst. When the cumulative
/// damage taken by observed allies reaches `threshold`, the entity enters
/// an avenging state where `effective_outgoing()` applies `bonus_multiplier`
/// for `duration` seconds.
///
/// `register_ally_damage(amount)` adds to `tracked_damage`. Fires
/// `just_triggered` on the transition from inactive to avenging (tracked
/// crosses threshold). While already avenging, additional calls extend
/// no further: tracked_damage is not accumulated during an active avenge.
/// No-op when disabled or `amount ≤ 0`.
///
/// `tick(dt)` clears one-frame flags at start; when avenging, counts down
/// `timer`; fires `just_subsided` and returns to idle (resetting
/// `tracked_damage`) when the timer expires.
///
/// `is_avenging()` returns `avenging && enabled`.
///
/// `effective_outgoing(base)` returns `base * bonus_multiplier` when avenging
/// and enabled; returns `base` otherwise.
///
/// `tracked_fraction()` returns `(tracked_damage / threshold).clamp(0, 1)`.
///
/// Distinct from `Rally` (entity boosts nearby allies on an ally event),
/// `Morale` (group morale tracking for the whole team), `Strife` (enemy-hit
/// accumulator for the entity itself), and `Wrath` (timed voluntary offence
/// trade): Avenge is an **ally-damage-accumulation retaliatory burst** — the
/// entity measures pain dealt to its comrades and retaliates when enough
/// has piled up.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Avenge {
    /// Ally damage accumulated toward the next avenge trigger.
    /// Reset to 0 when an avenge begins.
    pub tracked_damage: f32,
    /// Ally damage threshold before avenging triggers. Clamped ≥ 0.001.
    pub threshold: f32,
    /// Outgoing damage multiplier while avenging. Clamped ≥ 1.0.
    pub bonus_multiplier: f32,
    /// Countdown during an active avenge state.
    pub timer: f32,
    /// Duration of each avenge burst. Clamped ≥ 0.0.
    pub duration: f32,
    pub avenging: bool,
    pub just_triggered: bool,
    pub just_subsided: bool,
    pub enabled: bool,
}

impl Avenge {
    pub fn new(threshold: f32, bonus_multiplier: f32, duration: f32) -> Self {
        Self {
            tracked_damage: 0.0,
            threshold: threshold.max(0.001),
            bonus_multiplier: bonus_multiplier.max(1.0),
            duration: duration.max(0.0),
            timer: 0.0,
            avenging: false,
            just_triggered: false,
            just_subsided: false,
            enabled: true,
        }
    }

    /// Accumulate ally damage toward the avenge threshold. Fires
    /// `just_triggered` on the transition to avenging. No-op when already
    /// avenging, disabled, or `amount ≤ 0`.
    pub fn register_ally_damage(&mut self, amount: f32) {
        if !self.enabled || self.avenging || amount <= 0.0 {
            return;
        }
        self.tracked_damage += amount;
        if self.tracked_damage >= self.threshold {
            self.tracked_damage = 0.0;
            self.avenging = true;
            self.timer = self.duration;
            self.just_triggered = true;
        }
    }

    /// Clear one-frame flags; count down avenge timer; fire `just_subsided`
    /// on expiry and return to idle.
    pub fn tick(&mut self, dt: f32) {
        self.just_triggered = false;
        self.just_subsided = false;

        if self.avenging {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.avenging = false;
                self.just_subsided = true;
            }
        }
    }

    /// `true` when the avenging burst is active and the component is enabled.
    pub fn is_avenging(&self) -> bool {
        self.avenging && self.enabled
    }

    /// Outgoing damage multiplied by `bonus_multiplier` while avenging.
    /// Returns `base * bonus_multiplier` when avenging; `base` otherwise.
    pub fn effective_outgoing(&self, base: f32) -> f32 {
        if self.is_avenging() {
            base * self.bonus_multiplier
        } else {
            base
        }
    }

    /// Fraction of ally-damage threshold accumulated [0.0, 1.0].
    /// Returns 0.0 while avenging (tracker is reset on trigger).
    pub fn tracked_fraction(&self) -> f32 {
        (self.tracked_damage / self.threshold).clamp(0.0, 1.0)
    }
}

impl Default for Avenge {
    fn default() -> Self {
        Self::new(50.0, 1.5, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_idle() {
        let a = Avenge::new(50.0, 1.5, 5.0);
        assert!(!a.avenging);
        assert_eq!(a.tracked_damage, 0.0);
    }

    #[test]
    fn register_ally_damage_accumulates() {
        let mut a = Avenge::new(50.0, 1.5, 5.0);
        a.register_ally_damage(20.0);
        assert!((a.tracked_damage - 20.0).abs() < 1e-5);
    }

    #[test]
    fn register_ally_damage_triggers_avenge_at_threshold() {
        let mut a = Avenge::new(30.0, 1.5, 5.0);
        a.register_ally_damage(30.0);
        assert!(a.avenging);
        assert!(a.just_triggered);
        assert!(a.is_avenging());
    }

    #[test]
    fn register_ally_damage_resets_tracked_on_trigger() {
        let mut a = Avenge::new(30.0, 1.5, 5.0);
        a.register_ally_damage(30.0);
        assert_eq!(a.tracked_damage, 0.0);
    }

    #[test]
    fn register_ally_damage_triggers_above_threshold() {
        let mut a = Avenge::new(10.0, 1.5, 5.0);
        a.register_ally_damage(15.0); // exceeds threshold
        assert!(a.avenging);
        assert!(a.just_triggered);
    }

    #[test]
    fn register_ally_damage_no_trigger_below_threshold() {
        let mut a = Avenge::new(50.0, 1.5, 5.0);
        a.register_ally_damage(20.0);
        assert!(!a.avenging);
        assert!(!a.just_triggered);
    }

    #[test]
    fn register_ally_damage_no_op_when_already_avenging() {
        let mut a = Avenge::new(10.0, 1.5, 5.0);
        a.register_ally_damage(10.0); // trigger
        a.tick(0.0);
        a.register_ally_damage(50.0); // should not restart
        assert!((a.tracked_damage).abs() < 1e-5); // still 0 from trigger reset
    }

    #[test]
    fn register_ally_damage_no_op_when_disabled() {
        let mut a = Avenge::new(10.0, 1.5, 5.0);
        a.enabled = false;
        a.register_ally_damage(50.0);
        assert!(!a.avenging);
        assert_eq!(a.tracked_damage, 0.0);
    }

    #[test]
    fn register_ally_damage_no_op_on_zero() {
        let mut a = Avenge::new(10.0, 1.5, 5.0);
        a.register_ally_damage(0.0);
        assert_eq!(a.tracked_damage, 0.0);
    }

    #[test]
    fn register_ally_damage_no_op_on_negative() {
        let mut a = Avenge::new(10.0, 1.5, 5.0);
        a.register_ally_damage(-5.0);
        assert_eq!(a.tracked_damage, 0.0);
    }

    #[test]
    fn tick_counts_down_timer() {
        let mut a = Avenge::new(10.0, 1.5, 5.0);
        a.register_ally_damage(10.0); // trigger, timer = 5.0
        a.tick(2.0);
        assert!((a.timer - 3.0).abs() < 1e-5);
        assert!(a.avenging);
    }

    #[test]
    fn tick_fires_just_subsided_on_expiry() {
        let mut a = Avenge::new(10.0, 1.5, 2.0);
        a.register_ally_damage(10.0); // timer = 2.0
        a.tick(2.0); // expires
        assert!(!a.avenging);
        assert!(a.just_subsided);
    }

    #[test]
    fn tick_clears_just_triggered() {
        let mut a = Avenge::new(10.0, 1.5, 5.0);
        a.register_ally_damage(10.0);
        a.tick(0.016);
        assert!(!a.just_triggered);
    }

    #[test]
    fn tick_clears_just_subsided_next_frame() {
        let mut a = Avenge::new(10.0, 1.5, 1.0);
        a.register_ally_damage(10.0);
        a.tick(1.0); // subsides
        a.tick(0.016); // cleared
        assert!(!a.just_subsided);
    }

    #[test]
    fn tick_no_change_when_idle() {
        let mut a = Avenge::new(50.0, 1.5, 5.0);
        a.tick(100.0);
        assert!(!a.avenging);
    }

    #[test]
    fn is_avenging_false_when_disabled() {
        let mut a = Avenge::new(10.0, 1.5, 5.0);
        a.register_ally_damage(10.0);
        a.enabled = false;
        assert!(!a.is_avenging());
    }

    #[test]
    fn effective_outgoing_multiplied_while_avenging() {
        let mut a = Avenge::new(10.0, 2.0, 5.0);
        a.register_ally_damage(10.0);
        // 100 * 2.0 = 200
        assert!((a.effective_outgoing(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_outgoing_base_when_idle() {
        let a = Avenge::new(50.0, 2.0, 5.0);
        assert!((a.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_base_when_disabled() {
        let mut a = Avenge::new(10.0, 2.0, 5.0);
        a.register_ally_damage(10.0);
        a.enabled = false;
        assert!((a.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_base_after_avenge_expires() {
        let mut a = Avenge::new(10.0, 2.0, 1.0);
        a.register_ally_damage(10.0);
        a.tick(1.0); // expires
        assert!((a.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn tracked_fraction_at_zero() {
        let a = Avenge::new(50.0, 1.5, 5.0);
        assert_eq!(a.tracked_fraction(), 0.0);
    }

    #[test]
    fn tracked_fraction_at_half() {
        let mut a = Avenge::new(50.0, 1.5, 5.0);
        a.register_ally_damage(25.0);
        assert!((a.tracked_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn tracked_fraction_resets_on_trigger() {
        let mut a = Avenge::new(20.0, 1.5, 5.0);
        a.register_ally_damage(20.0); // triggers
        assert_eq!(a.tracked_fraction(), 0.0);
    }

    #[test]
    fn re_triggers_after_avenge_ends() {
        let mut a = Avenge::new(10.0, 1.5, 1.0);
        a.register_ally_damage(10.0); // first avenge
        a.tick(1.0); // expires
        a.tick(0.016);
        a.register_ally_damage(10.0); // second avenge
        assert!(a.just_triggered);
        assert!(a.avenging);
    }

    #[test]
    fn incremental_damage_triggers_avenge() {
        let mut a = Avenge::new(30.0, 1.5, 5.0);
        a.register_ally_damage(10.0);
        a.register_ally_damage(10.0);
        a.register_ally_damage(10.0); // crosses 30
        assert!(a.avenging);
    }

    #[test]
    fn threshold_clamped_to_minimum() {
        let a = Avenge::new(0.0, 1.5, 5.0);
        assert!(a.threshold >= 0.001);
    }

    #[test]
    fn bonus_multiplier_clamped_to_one() {
        let a = Avenge::new(50.0, 0.5, 5.0);
        assert!((a.bonus_multiplier - 1.0).abs() < 1e-5);
    }

    #[test]
    fn duration_clamped_to_zero() {
        let a = Avenge::new(50.0, 1.5, -1.0);
        assert_eq!(a.duration, 0.0);
    }

    #[test]
    fn zero_duration_avenge_expires_on_next_tick() {
        let mut a = Avenge::new(10.0, 1.5, 0.0);
        a.register_ally_damage(10.0); // trigger, timer = 0
        a.tick(0.016); // immediate expiry
        assert!(!a.avenging);
        assert!(a.just_subsided);
    }
}

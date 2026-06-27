use bevy_ecs::prelude::Component;

/// Timed double-edged power surge: while wrathful the entity's outgoing damage
/// is scaled by `damage_multiplier` and all incoming damage is scaled by
/// `1.0 + defense_penalty`. The entity explicitly trades defense for burst
/// offense for a fixed duration.
///
/// `enrage(duration)` starts or extends the wrath period (high-watermark:
/// only replaces the timer when `duration > timer`). Fires `just_entered` on
/// the inactive → active transition. No-op when disabled or `duration ≤ 0`.
///
/// `calm()` ends wrath early. Fires `just_exited`. No-op when not wrathful.
///
/// `tick(dt)` clears one-frame flags at start, then counts down. Fires
/// `just_exited` when the timer expires naturally.
///
/// `is_wrathful()` returns `active && enabled`.
///
/// `effective_damage(base)` returns `base * damage_multiplier` while
/// wrathful and enabled; returns `base` otherwise.
///
/// `effective_incoming(base)` returns `base * (1.0 + defense_penalty)` while
/// wrathful and enabled; returns `base` otherwise.
///
/// Distinct from `Rage` (continuous HP-loss-triggered scaling anger with no
/// timer), `Fury` (scales proportionally with current HP deficit), `Feral`
/// (activated by HP falling below a threshold with a speed bonus), and
/// `Strife` (hit-frequency accumulator): Wrath is a **timed double-edged
/// power surge** — the entity voluntarily accepts incoming damage amplification
/// in exchange for a fixed outgoing damage boost for a set duration.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrath {
    pub active: bool,
    pub timer: f32,
    /// Outgoing damage multiplier while wrathful. Clamped ≥ 1.0.
    pub damage_multiplier: f32,
    /// Fraction by which incoming damage increases while wrathful. Clamped [0.0, 1.0].
    /// e.g. `0.3` means incoming damage is multiplied by 1.3.
    pub defense_penalty: f32,
    pub just_entered: bool,
    pub just_exited: bool,
    pub enabled: bool,
}

impl Wrath {
    pub fn new(damage_multiplier: f32, defense_penalty: f32) -> Self {
        Self {
            active: false,
            timer: 0.0,
            damage_multiplier: damage_multiplier.max(1.0),
            defense_penalty: defense_penalty.clamp(0.0, 1.0),
            just_entered: false,
            just_exited: false,
            enabled: true,
        }
    }

    /// Start or extend wrath for `duration` seconds. High-watermark: only
    /// replaces the timer when `duration > timer`. Fires `just_entered` on
    /// the inactive → active transition. No-op when disabled or
    /// `duration ≤ 0`.
    pub fn enrage(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_inactive = !self.is_wrathful();
            self.timer = duration;
            self.active = true;
            if was_inactive {
                self.just_entered = true;
            }
        }
    }

    /// End wrath early. Fires `just_exited`. No-op when not wrathful.
    pub fn calm(&mut self) {
        if !self.is_wrathful() {
            return;
        }
        self.active = false;
        self.timer = 0.0;
        self.just_exited = true;
    }

    /// Advance the wrath timer. Clears one-frame flags at start. Fires
    /// `just_exited` when the timer expires naturally.
    pub fn tick(&mut self, dt: f32) {
        self.just_entered = false;
        self.just_exited = false;

        if self.active {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.active = false;
                self.just_exited = true;
            }
        }
    }

    /// `true` when wrath is active and the component is enabled.
    pub fn is_wrathful(&self) -> bool {
        self.active && self.enabled
    }

    /// Outgoing damage scaled by `damage_multiplier` while wrathful.
    /// Returns `base` unchanged otherwise.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_wrathful() {
            base * self.damage_multiplier
        } else {
            base
        }
    }

    /// Incoming damage increased by `defense_penalty` fraction while wrathful.
    /// Returns `base * (1.0 + defense_penalty)` while wrathful and enabled;
    /// returns `base` otherwise.
    pub fn effective_incoming(&self, base: f32) -> f32 {
        if self.is_wrathful() {
            base * (1.0 + self.defense_penalty)
        } else {
            base
        }
    }

    /// Fraction of the current wrath timer relative to a known original
    /// duration. Returns `(timer / original_duration).clamp(0, 1)`;
    /// 0.0 when inactive or `original_duration ≤ 0`.
    pub fn remaining_fraction(&self, original_duration: f32) -> f32 {
        if !self.active || original_duration <= 0.0 {
            return 0.0;
        }
        (self.timer / original_duration).clamp(0.0, 1.0)
    }
}

impl Default for Wrath {
    fn default() -> Self {
        Self::new(2.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_inactive() {
        let w = Wrath::new(2.0, 0.5);
        assert!(!w.active);
        assert!(!w.is_wrathful());
    }

    #[test]
    fn enrage_starts_wrath() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(5.0);
        assert!(w.active);
        assert!(w.just_entered);
        assert!(w.is_wrathful());
    }

    #[test]
    fn enrage_extends_on_longer_duration() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(3.0);
        w.tick(0.016);
        w.enrage(10.0);
        assert!((w.timer - 10.0).abs() < 1e-4);
    }

    #[test]
    fn enrage_no_extend_on_shorter_duration() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(10.0);
        w.enrage(3.0);
        assert!((w.timer - 10.0).abs() < 1e-4);
    }

    #[test]
    fn just_entered_not_set_on_extend() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(3.0);
        w.tick(0.016);
        w.enrage(10.0);
        assert!(!w.just_entered);
    }

    #[test]
    fn enrage_no_op_when_disabled() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enabled = false;
        w.enrage(5.0);
        assert!(!w.active);
    }

    #[test]
    fn enrage_no_op_at_zero_duration() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(0.0);
        assert!(!w.active);
    }

    #[test]
    fn calm_ends_wrath() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(5.0);
        w.calm();
        assert!(!w.active);
        assert!(!w.is_wrathful());
        assert!(w.just_exited);
    }

    #[test]
    fn calm_no_op_when_inactive() {
        let mut w = Wrath::new(2.0, 0.5);
        w.calm();
        assert!(!w.just_exited);
    }

    #[test]
    fn calm_no_op_when_disabled() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(5.0);
        w.tick(0.016);
        w.enabled = false;
        // is_wrathful() returns false when disabled, so calm() is a no-op
        w.calm();
        assert!(!w.just_exited);
    }

    #[test]
    fn tick_expires_naturally() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(1.0);
        w.tick(0.016);
        w.tick(2.0);
        assert!(!w.active);
        assert!(w.just_exited);
    }

    #[test]
    fn tick_clears_just_entered() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(5.0);
        w.tick(0.016);
        assert!(!w.just_entered);
    }

    #[test]
    fn tick_clears_just_exited() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(0.5);
        w.tick(0.016);
        w.tick(1.0); // expires → just_exited
        w.tick(0.016);
        assert!(!w.just_exited);
    }

    #[test]
    fn is_wrathful_false_when_disabled() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(5.0);
        w.enabled = false;
        assert!(!w.is_wrathful());
    }

    #[test]
    fn effective_damage_multiplied_while_wrathful() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(5.0);
        // 100 * 2.0 = 200
        assert!((w.effective_damage(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_base_when_inactive() {
        let w = Wrath::new(2.0, 0.5);
        assert!((w.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_damage_base_when_disabled() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(5.0);
        w.enabled = false;
        assert!((w.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_increased_while_wrathful() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(5.0);
        // 100 * (1 + 0.5) = 150
        assert!((w.effective_incoming(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_incoming_base_when_inactive() {
        let w = Wrath::new(2.0, 0.5);
        assert!((w.effective_incoming(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_base_when_disabled() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(5.0);
        w.enabled = false;
        assert!((w.effective_incoming(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(4.0);
        w.tick(2.0);
        assert!((w.remaining_fraction(4.0) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_inactive() {
        let w = Wrath::new(2.0, 0.5);
        assert_eq!(w.remaining_fraction(5.0), 0.0);
    }

    #[test]
    fn remaining_fraction_zero_at_zero_original() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(5.0);
        assert_eq!(w.remaining_fraction(0.0), 0.0);
    }

    #[test]
    fn damage_multiplier_clamped_to_one() {
        let w = Wrath::new(0.5, 0.3);
        assert!((w.damage_multiplier - 1.0).abs() < 1e-5);
    }

    #[test]
    fn defense_penalty_clamped_at_one() {
        let w = Wrath::new(2.0, 2.0);
        assert!((w.defense_penalty - 1.0).abs() < 1e-5);
    }

    #[test]
    fn defense_penalty_clamped_at_zero() {
        let w = Wrath::new(2.0, -0.5);
        assert_eq!(w.defense_penalty, 0.0);
    }

    #[test]
    fn re_enters_wrath_after_expiry() {
        let mut w = Wrath::new(2.0, 0.5);
        w.enrage(0.5);
        w.tick(0.016);
        w.tick(1.0); // expires
        w.tick(0.016);
        w.enrage(5.0); // re-enter
        assert!(w.just_entered);
    }

    #[test]
    fn zero_defense_penalty_no_incoming_increase() {
        let mut w = Wrath::new(2.0, 0.0);
        w.enrage(5.0);
        assert!((w.effective_incoming(100.0) - 100.0).abs() < 1e-5);
    }
}

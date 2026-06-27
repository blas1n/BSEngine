use bevy_ecs::prelude::Component;

/// Lone-wolf isolation state: while isolated, the entity receives reduced
/// ally buffs/heals AND reduced enemy debuffs/damage effects, making it
/// self-reliant but unresponsive to support.
///
/// `seclude(duration)` starts or extends the isolation (high-watermark); sets
/// `just_began` on the inactive → active transition. `rejoin()` ends it early.
/// `tick(dt)` counts down and sets `just_ended` on expiry.
///
/// `effective_buff(base)` scales incoming support effects down by
/// `buff_reduction` fraction while isolated; `effective_debuff(base)` similarly
/// scales incoming debuff/damage effects down by `debuff_reduction`.
///
/// Distinct from `Stealth` (visibility hiding), `Immune` (full debuff immunity),
/// and `Invincible` (full damage immunity): Isolate is a **symmetrical
/// self-reliance tradeoff** — harder to support and harder to debuff in equal
/// measure.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Isolate {
    pub duration: f32,
    pub timer: f32,
    /// Fraction of incoming buff/heal effects blocked while isolated. [0.0, 1.0]
    pub buff_reduction: f32,
    /// Fraction of incoming debuff/damage effects blocked while isolated. [0.0, 1.0]
    pub debuff_reduction: f32,
    pub just_began: bool,
    pub just_ended: bool,
    pub enabled: bool,
}

impl Isolate {
    pub fn new(buff_reduction: f32, debuff_reduction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            buff_reduction: buff_reduction.clamp(0.0, 1.0),
            debuff_reduction: debuff_reduction.clamp(0.0, 1.0),
            just_began: false,
            just_ended: false,
            enabled: true,
        }
    }

    /// Enter or extend the isolation state for `duration` seconds.
    /// High-watermark: only replaces the timer when `duration > timer`. Sets
    /// `just_began` on the inactive → active transition. No-op when disabled
    /// or `duration ≤ 0`.
    pub fn seclude(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_isolated = self.is_isolated();
            self.duration = duration;
            self.timer = duration;
            if !was_isolated {
                self.just_began = true;
            }
        }
    }

    /// Rejoin allies, ending the isolation early. Sets `just_ended`.
    /// No-op when not isolated.
    pub fn rejoin(&mut self) {
        if !self.is_isolated() {
            return;
        }
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_ended = true;
    }

    /// Advance the isolation timer. Sets `just_ended` when the state expires
    /// naturally. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_began = false;
        self.just_ended = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_ended = true;
            }
        }
    }

    pub fn is_isolated(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective incoming buff/heal value after isolation reduction.
    /// Returns `base * (1 - buff_reduction)` while isolated and enabled,
    /// floored at 0.0. Returns `base` otherwise.
    pub fn effective_buff(&self, base: f32) -> f32 {
        if self.is_isolated() && self.enabled {
            (base * (1.0 - self.buff_reduction)).max(0.0)
        } else {
            base
        }
    }

    /// Effective incoming debuff/damage value after isolation reduction.
    /// Returns `base * (1 - debuff_reduction)` while isolated and enabled,
    /// floored at 0.0. Returns `base` otherwise.
    pub fn effective_debuff(&self, base: f32) -> f32 {
        if self.is_isolated() && self.enabled {
            (base * (1.0 - self.debuff_reduction)).max(0.0)
        } else {
            base
        }
    }

    /// Fraction of the isolation duration remaining [1.0 = just began, 0.0 = ended].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Isolate {
    fn default() -> Self {
        Self::new(0.5, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seclude_starts_isolation() {
        let mut iso = Isolate::new(0.5, 0.5);
        iso.seclude(5.0);
        assert!(iso.is_isolated());
        assert!(iso.just_began);
    }

    #[test]
    fn seclude_extends_on_longer_duration() {
        let mut iso = Isolate::new(0.5, 0.5);
        iso.seclude(3.0);
        iso.tick(0.016);
        iso.seclude(8.0);
        assert!((iso.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn seclude_no_extend_on_shorter_duration() {
        let mut iso = Isolate::new(0.5, 0.5);
        iso.seclude(8.0);
        iso.seclude(3.0);
        assert!((iso.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn just_began_not_set_on_extend() {
        let mut iso = Isolate::new(0.5, 0.5);
        iso.seclude(3.0);
        iso.tick(0.016);
        iso.seclude(8.0);
        assert!(!iso.just_began);
    }

    #[test]
    fn rejoin_ends_isolation() {
        let mut iso = Isolate::new(0.5, 0.5);
        iso.seclude(5.0);
        iso.rejoin();
        assert!(!iso.is_isolated());
        assert!(iso.just_ended);
    }

    #[test]
    fn rejoin_no_op_when_not_isolated() {
        let mut iso = Isolate::new(0.5, 0.5);
        iso.rejoin();
        assert!(!iso.just_ended);
    }

    #[test]
    fn tick_expires_isolation() {
        let mut iso = Isolate::new(0.5, 0.5);
        iso.seclude(1.0);
        iso.tick(1.1);
        assert!(!iso.is_isolated());
        assert!(iso.just_ended);
    }

    #[test]
    fn tick_clears_just_began() {
        let mut iso = Isolate::new(0.5, 0.5);
        iso.seclude(5.0);
        iso.tick(0.016);
        assert!(!iso.just_began);
    }

    #[test]
    fn tick_clears_just_ended() {
        let mut iso = Isolate::new(0.5, 0.5);
        iso.seclude(0.5);
        iso.tick(1.0);
        iso.tick(0.016);
        assert!(!iso.just_ended);
    }

    #[test]
    fn effective_buff_reduced_while_isolated() {
        let mut iso = Isolate::new(0.5, 0.3);
        iso.seclude(5.0);
        // base=100, 100 * (1 - 0.5) = 50
        assert!((iso.effective_buff(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_buff_base_when_not_isolated() {
        let iso = Isolate::new(0.5, 0.3);
        assert!((iso.effective_buff(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_buff_floored_at_zero() {
        let mut iso = Isolate::new(1.0, 0.5);
        iso.seclude(5.0);
        assert!((iso.effective_buff(100.0)).abs() < 1e-5);
    }

    #[test]
    fn effective_debuff_reduced_while_isolated() {
        let mut iso = Isolate::new(0.3, 0.6);
        iso.seclude(5.0);
        // base=100, 100 * (1 - 0.6) = 40
        assert!((iso.effective_debuff(100.0) - 40.0).abs() < 1e-3);
    }

    #[test]
    fn effective_debuff_base_when_not_isolated() {
        let iso = Isolate::new(0.3, 0.6);
        assert!((iso.effective_debuff(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut iso = Isolate::new(0.5, 0.5);
        iso.seclude(4.0);
        iso.tick(2.0);
        assert!((iso.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_not_isolated() {
        let iso = Isolate::new(0.5, 0.5);
        assert!((iso.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_seclude_no_op() {
        let mut iso = Isolate::new(0.5, 0.5);
        iso.enabled = false;
        iso.seclude(5.0);
        assert!(!iso.is_isolated());
    }

    #[test]
    fn disabled_effective_buff_base() {
        let mut iso = Isolate::new(0.5, 0.5);
        iso.seclude(5.0);
        iso.enabled = false;
        assert!((iso.effective_buff(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_effective_debuff_base() {
        let mut iso = Isolate::new(0.5, 0.5);
        iso.seclude(5.0);
        iso.enabled = false;
        assert!((iso.effective_debuff(100.0) - 100.0).abs() < 1e-5);
    }
}

use bevy_ecs::prelude::Component;

/// Persistent combat-spirit meter: `morale` floats between 0.0 (broken) and
/// 1.0 (peak). High morale amplifies damage and speed proportionally; the
/// meter decays passively each tick and rises on successful events via
/// `boost(amount)`.
///
/// `just_peaked` fires the frame `morale` reaches 1.0; `just_broke` fires when
/// it drops to 0.0. Clears one-frame flags at the start of each `tick`.
///
/// Distinct from `Fear` (binary flee state), `Demoralize` (externally applied
/// debuff), and `Buff` (generic stat modifier): Morale is a **continuous
/// self-sustaining spirit meter** — it rewards momentum (kills, success events)
/// with proportional combat bonuses and punishes inaction with natural decay.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Morale {
    /// Current morale level [0.0, 1.0].
    pub morale: f32,
    /// Morale lost per second (natural decay). Clamped ≥ 0.0.
    pub decay_rate: f32,
    /// Maximum damage bonus fraction when morale = 1.0. Clamped ≥ 0.0.
    pub damage_bonus: f32,
    /// Maximum speed bonus fraction when morale = 1.0. Clamped ≥ 0.0.
    pub speed_bonus: f32,
    pub just_peaked: bool,
    pub just_broke: bool,
    pub enabled: bool,
}

impl Morale {
    pub fn new(decay_rate: f32, damage_bonus: f32, speed_bonus: f32) -> Self {
        Self {
            morale: 0.5,
            decay_rate: decay_rate.max(0.0),
            damage_bonus: damage_bonus.max(0.0),
            speed_bonus: speed_bonus.max(0.0),
            just_peaked: false,
            just_broke: false,
            enabled: true,
        }
    }

    /// Increase morale by `amount`, clamped at 1.0. Sets `just_peaked` the
    /// frame morale first reaches 1.0. No-op for `amount ≤ 0`.
    pub fn boost(&mut self, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        let was_below_peak = self.morale < 1.0;
        self.morale = (self.morale + amount).min(1.0);
        if was_below_peak && self.morale >= 1.0 {
            self.just_peaked = true;
        }
    }

    /// Decrease morale by `amount`, floored at 0.0. Sets `just_broke` the
    /// frame morale first reaches 0.0. No-op for `amount ≤ 0`.
    pub fn drop(&mut self, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        let was_above_zero = self.morale > 0.0;
        self.morale = (self.morale - amount).max(0.0);
        if was_above_zero && self.morale <= 0.0 {
            self.just_broke = true;
        }
    }

    /// Advance the passive decay. Sets `just_broke` when morale drops to 0.
    /// Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;
        self.just_broke = false;

        if self.morale > 0.0 {
            let was_positive = self.morale > 0.0;
            self.morale -= self.decay_rate * dt;
            if self.morale <= 0.0 {
                self.morale = 0.0;
                if was_positive {
                    self.just_broke = true;
                }
            }
        }
    }

    pub fn is_broken(&self) -> bool {
        self.morale <= 0.0
    }

    pub fn is_peaked(&self) -> bool {
        self.morale >= 1.0
    }

    /// Effective damage with morale bonus.
    /// Returns `base * (1 + damage_bonus * morale)` when enabled, else `base`.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.enabled {
            base * (1.0 + self.damage_bonus * self.morale)
        } else {
            base
        }
    }

    /// Effective movement speed with morale bonus.
    /// Returns `base * (1 + speed_bonus * morale)` when enabled, else `base`.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if self.enabled {
            base * (1.0 + self.speed_bonus * self.morale)
        } else {
            base
        }
    }
}

impl Default for Morale {
    fn default() -> Self {
        Self::new(0.05, 0.3, 0.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_morale_is_half() {
        let m = Morale::new(0.05, 0.3, 0.2);
        assert!((m.morale - 0.5).abs() < 1e-5);
    }

    #[test]
    fn boost_increases_morale() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.boost(0.3);
        assert!((m.morale - 0.8).abs() < 1e-5);
    }

    #[test]
    fn boost_clamps_at_one() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.boost(1.0);
        assert!((m.morale - 1.0).abs() < 1e-5);
    }

    #[test]
    fn boost_sets_just_peaked_on_crossing() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.boost(0.6); // 0.5 + 0.6 = 1.1 → clamp 1.0
        assert!(m.just_peaked);
    }

    #[test]
    fn boost_no_just_peaked_when_already_peaked() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.boost(0.6);
        m.tick(0.016);
        m.boost(0.1); // still at peak
        assert!(!m.just_peaked);
    }

    #[test]
    fn boost_no_op_for_zero_or_negative() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        let before = m.morale;
        m.boost(0.0);
        m.boost(-0.1);
        assert!((m.morale - before).abs() < 1e-5);
    }

    #[test]
    fn drop_decreases_morale() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.drop(0.3);
        assert!((m.morale - 0.2).abs() < 1e-5);
    }

    #[test]
    fn drop_floors_at_zero() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.drop(1.0);
        assert_eq!(m.morale, 0.0);
    }

    #[test]
    fn drop_sets_just_broke_on_crossing() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.drop(0.6); // 0.5 - 0.6 → 0.0
        assert!(m.just_broke);
    }

    #[test]
    fn drop_no_just_broke_when_already_broken() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.drop(0.6);
        m.tick(0.016);
        m.drop(0.1); // still at 0
        assert!(!m.just_broke);
    }

    #[test]
    fn tick_decays_morale() {
        let mut m = Morale::new(0.1, 0.3, 0.2);
        m.tick(1.0);
        assert!((m.morale - 0.4).abs() < 1e-4);
    }

    #[test]
    fn tick_sets_just_broke_on_decay_to_zero() {
        let mut m = Morale::new(1.0, 0.3, 0.2);
        m.tick(1.0);
        assert_eq!(m.morale, 0.0);
        assert!(m.just_broke);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.boost(0.6);
        m.tick(0.016);
        assert!(!m.just_peaked);
    }

    #[test]
    fn tick_clears_just_broke() {
        let mut m = Morale::new(1.0, 0.3, 0.2);
        m.tick(1.0);
        m.tick(0.016);
        assert!(!m.just_broke);
    }

    #[test]
    fn tick_no_advance_at_zero() {
        let mut m = Morale::new(0.1, 0.3, 0.2);
        m.drop(0.6);
        m.tick(5.0);
        assert_eq!(m.morale, 0.0);
        assert!(!m.just_broke);
    }

    #[test]
    fn effective_damage_scales_with_morale() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.morale = 1.0;
        // base=100, 100*(1+0.3*1.0) = 130
        assert!((m.effective_damage(100.0) - 130.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_at_zero_morale() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.morale = 0.0;
        assert!((m.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_scales_with_morale() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.morale = 1.0;
        // base=100, 100*(1+0.2*1.0) = 120
        assert!((m.effective_speed(100.0) - 120.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_at_zero_morale() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.morale = 0.0;
        assert!((m.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_effective_damage_base() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.morale = 1.0;
        m.enabled = false;
        assert!((m.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_effective_speed_base() {
        let mut m = Morale::new(0.05, 0.3, 0.2);
        m.morale = 1.0;
        m.enabled = false;
        assert!((m.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }
}

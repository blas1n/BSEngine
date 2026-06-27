use bevy_ecs::prelude::Component;

/// Fall-speed-scaled AoE impact: the movement/physics system accumulates
/// impact magnitude via `accumulate(speed)`, which stores the maximum speed
/// seen since the last `trigger()`. When the ability system calls `trigger()`,
/// it returns `magnitude * damage_per_unit` (the effective area damage base),
/// fires `just_stomped` for one frame, and resets `magnitude` to zero. The
/// game layer is responsible for querying entities within `impact_radius` and
/// applying the returned damage value to each.
///
/// `accumulate(speed)` is a high-watermark — it keeps whichever speed is
/// larger (current `magnitude` vs the given `speed`) so the maximum descent
/// velocity is preserved even if multiple accumulation calls arrive per tick.
/// No-op when disabled.
///
/// Distinct from `Slam` (melee windup attack), `Knockback` (directional
/// push), and `Crash` (collision damage): Stomp is a **fall-velocity-fueled
/// landing AoE** — it explicitly stores and converts movement magnitude into
/// a radius-based damage burst on demand.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Stomp {
    /// Accumulated impact magnitude. Updated by `accumulate`; reset by `trigger`.
    pub magnitude: f32,
    /// AoE radius queried by the game layer after `trigger()`. Clamped ≥ 0.0.
    pub impact_radius: f32,
    /// Damage dealt per unit of accumulated magnitude. Clamped ≥ 0.0.
    pub damage_per_unit: f32,
    pub just_stomped: bool,
    pub enabled: bool,
}

impl Stomp {
    pub fn new(impact_radius: f32, damage_per_unit: f32) -> Self {
        Self {
            magnitude: 0.0,
            impact_radius: impact_radius.max(0.0),
            damage_per_unit: damage_per_unit.max(0.0),
            just_stomped: false,
            enabled: true,
        }
    }

    /// High-watermark update: keeps the larger of `self.magnitude` and
    /// `speed`. No-op when disabled or `speed ≤ 0`.
    pub fn accumulate(&mut self, speed: f32) {
        if !self.enabled || speed <= 0.0 {
            return;
        }
        if speed > self.magnitude {
            self.magnitude = speed;
        }
    }

    /// Fire the stomp: returns `magnitude * damage_per_unit`, sets
    /// `just_stomped`, and resets `magnitude` to zero. Returns `0.0` when
    /// `magnitude` is zero or the component is disabled. The caller uses the
    /// return value and `impact_radius` to apply area damage.
    pub fn trigger(&mut self) -> f32 {
        if !self.enabled || self.magnitude <= 0.0 {
            return 0.0;
        }
        let damage = self.magnitude * self.damage_per_unit;
        self.magnitude = 0.0;
        self.just_stomped = true;
        damage
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_stomped = false;
    }

    pub fn has_magnitude(&self) -> bool {
        self.magnitude > 0.0
    }

    /// Preview the damage that would result from `trigger()` without firing.
    pub fn expected_damage(&self) -> f32 {
        self.magnitude * self.damage_per_unit
    }
}

impl Default for Stomp {
    fn default() -> Self {
        Self::new(3.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulate_sets_magnitude() {
        let mut s = Stomp::new(3.0, 5.0);
        s.accumulate(10.0);
        assert!((s.magnitude - 10.0).abs() < 1e-5);
        assert!(s.has_magnitude());
    }

    #[test]
    fn accumulate_keeps_high_watermark() {
        let mut s = Stomp::new(3.0, 5.0);
        s.accumulate(10.0);
        s.accumulate(5.0); // lower → no change
        assert!((s.magnitude - 10.0).abs() < 1e-5);
    }

    #[test]
    fn accumulate_updates_on_higher_value() {
        let mut s = Stomp::new(3.0, 5.0);
        s.accumulate(5.0);
        s.accumulate(12.0); // higher → updates
        assert!((s.magnitude - 12.0).abs() < 1e-5);
    }

    #[test]
    fn accumulate_no_op_on_non_positive() {
        let mut s = Stomp::new(3.0, 5.0);
        s.accumulate(0.0);
        s.accumulate(-1.0);
        assert!(!s.has_magnitude());
    }

    #[test]
    fn accumulate_no_op_when_disabled() {
        let mut s = Stomp::new(3.0, 5.0);
        s.enabled = false;
        s.accumulate(10.0);
        assert!(!s.has_magnitude());
    }

    #[test]
    fn trigger_returns_damage_and_resets() {
        let mut s = Stomp::new(3.0, 5.0);
        s.accumulate(10.0);
        let dmg = s.trigger();
        // 10 * 5 = 50
        assert!((dmg - 50.0).abs() < 1e-3);
        assert!(!s.has_magnitude());
        assert!(s.just_stomped);
    }

    #[test]
    fn trigger_returns_zero_when_no_magnitude() {
        let mut s = Stomp::new(3.0, 5.0);
        let dmg = s.trigger();
        assert!((dmg).abs() < 1e-5);
        assert!(!s.just_stomped);
    }

    #[test]
    fn trigger_returns_zero_when_disabled() {
        let mut s = Stomp::new(3.0, 5.0);
        s.accumulate(10.0);
        s.enabled = false;
        let dmg = s.trigger();
        assert!((dmg).abs() < 1e-5);
        assert!(!s.just_stomped);
        assert!((s.magnitude - 10.0).abs() < 1e-5); // magnitude preserved
    }

    #[test]
    fn tick_clears_just_stomped() {
        let mut s = Stomp::new(3.0, 5.0);
        s.accumulate(10.0);
        s.trigger();
        s.tick();
        assert!(!s.just_stomped);
    }

    #[test]
    fn expected_damage_preview_without_firing() {
        let mut s = Stomp::new(3.0, 2.0);
        s.accumulate(8.0);
        // 8 * 2 = 16
        assert!((s.expected_damage() - 16.0).abs() < 1e-3);
        assert!(s.has_magnitude()); // not consumed
    }

    #[test]
    fn expected_damage_zero_when_no_magnitude() {
        let s = Stomp::new(3.0, 5.0);
        assert!((s.expected_damage()).abs() < 1e-5);
    }

    #[test]
    fn accum_trigger_cycle_resets_for_next_stomp() {
        let mut s = Stomp::new(3.0, 5.0);
        s.accumulate(10.0);
        s.trigger();
        s.tick();
        // second stomp cycle
        s.accumulate(6.0);
        let dmg = s.trigger();
        assert!((dmg - 30.0).abs() < 1e-3);
    }

    #[test]
    fn impact_radius_clamped_non_negative() {
        let s = Stomp::new(-5.0, 5.0);
        assert!(s.impact_radius >= 0.0);
    }

    #[test]
    fn damage_per_unit_clamped_non_negative() {
        let s = Stomp::new(3.0, -1.0);
        assert!(s.damage_per_unit >= 0.0);
    }

    #[test]
    fn zero_damage_per_unit_trigger_returns_zero() {
        let mut s = Stomp::new(3.0, 0.0);
        s.accumulate(100.0);
        let dmg = s.trigger();
        assert!((dmg).abs() < 1e-5);
        assert!(s.just_stomped); // still fires the event
        assert!(!s.has_magnitude());
    }
}

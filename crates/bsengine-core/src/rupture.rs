use bevy_ecs::prelude::Component;

/// Stack-based internal wound that amplifies subsequent hit damage. Each
/// `apply()` call adds rupture stacks, and each hit call from the combat
/// system passes the incoming force through `burst_damage(hit_force)` to
/// get extra damage proportional to active stacks.
///
/// `apply(count)` adds `count` stacks (capped at `max_stacks`). Fires
/// `just_maxed` on the first transition to `max_stacks`. No-op when disabled
/// or `count == 0`.
///
/// `cleanse(count)` removes up to `count` stacks (saturating subtraction).
/// No-op when disabled or `count == 0`.
///
/// `tick()` clears `just_maxed` each frame.
///
/// `burst_damage(hit_force)` returns `hit_force * damage_per_stack * stacks`
/// when enabled and stacks > 0; returns `0.0` otherwise. Add this to base
/// incoming damage to model the wound amplification.
///
/// `is_ruptured()` returns `stacks > 0 && enabled`.
///
/// `at_max()` returns `stacks >= max_stacks`.
///
/// Distinct from `Bleed` (per-tick flat damage independent of hit force),
/// `Lacerate` (on-hit damage-over-time application), and `Corrosion` (armor
/// reduction): Rupture is a **force-amplified burst wound** — the damage
/// bonus scales with both the force of the triggering hit and the number of
/// accumulated stacks, rewarding sustained offensive pressure.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Rupture {
    /// Active rupture stacks [0, max_stacks].
    pub stacks: u32,
    /// Maximum stacks the wound can accumulate. Clamped ≥ 1.
    pub max_stacks: u32,
    /// Extra damage multiplied per stack per unit of hit force. Clamped ≥ 0.0.
    pub damage_per_stack: f32,
    pub just_maxed: bool,
    pub enabled: bool,
}

impl Rupture {
    pub fn new(max_stacks: u32, damage_per_stack: f32) -> Self {
        Self {
            stacks: 0,
            max_stacks: max_stacks.max(1),
            damage_per_stack: damage_per_stack.max(0.0),
            just_maxed: false,
            enabled: true,
        }
    }

    /// Add `count` rupture stacks (capped at `max_stacks`). Fires `just_maxed`
    /// on the first transition to `max_stacks`. No-op when disabled or
    /// `count == 0`.
    pub fn apply(&mut self, count: u32) {
        if !self.enabled || count == 0 {
            return;
        }
        let was_at_max = self.at_max();
        self.stacks = (self.stacks + count).min(self.max_stacks);
        if !was_at_max && self.at_max() {
            self.just_maxed = true;
        }
    }

    /// Remove up to `count` stacks (saturating). No-op when disabled or
    /// `count == 0`.
    pub fn cleanse(&mut self, count: u32) {
        if !self.enabled || count == 0 {
            return;
        }
        self.stacks = self.stacks.saturating_sub(count);
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_maxed = false;
    }

    /// Extra damage burst from active stacks: `hit_force * damage_per_stack * stacks`
    /// when enabled and stacks > 0; returns `0.0` otherwise. Add to base
    /// incoming damage.
    pub fn burst_damage(&self, hit_force: f32) -> f32 {
        if !self.enabled || self.stacks == 0 {
            return 0.0;
        }
        (hit_force * self.damage_per_stack * self.stacks as f32).max(0.0)
    }

    /// `true` when at least one stack is active and the component is enabled.
    pub fn is_ruptured(&self) -> bool {
        self.stacks > 0 && self.enabled
    }

    /// `true` when stacks have reached `max_stacks`.
    pub fn at_max(&self) -> bool {
        self.stacks >= self.max_stacks
    }

    /// Stack fill fraction [0.0 = none, 1.0 = max].
    pub fn stack_fraction(&self) -> f32 {
        self.stacks as f32 / self.max_stacks as f32
    }
}

impl Default for Rupture {
    fn default() -> Self {
        Self::new(5, 0.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero_stacks() {
        let r = Rupture::new(5, 0.2);
        assert_eq!(r.stacks, 0);
        assert!(!r.is_ruptured());
    }

    #[test]
    fn apply_adds_stacks() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(2);
        assert_eq!(r.stacks, 2);
    }

    #[test]
    fn apply_caps_at_max() {
        let mut r = Rupture::new(3, 0.2);
        r.apply(10);
        assert_eq!(r.stacks, 3);
    }

    #[test]
    fn apply_fires_just_maxed_on_transition() {
        let mut r = Rupture::new(3, 0.2);
        r.apply(2);
        assert!(!r.just_maxed);
        r.apply(1); // hits 3 = max
        assert!(r.just_maxed);
        assert!(r.at_max());
    }

    #[test]
    fn apply_no_just_maxed_when_already_at_max() {
        let mut r = Rupture::new(3, 0.2);
        r.apply(3); // hits max
        r.tick();
        r.apply(1); // still at max
        assert!(!r.just_maxed);
    }

    #[test]
    fn apply_no_op_when_disabled() {
        let mut r = Rupture::new(5, 0.2);
        r.enabled = false;
        r.apply(3);
        assert_eq!(r.stacks, 0);
    }

    #[test]
    fn apply_no_op_at_zero_count() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(0);
        assert_eq!(r.stacks, 0);
    }

    #[test]
    fn cleanse_removes_stacks() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(4);
        r.cleanse(2);
        assert_eq!(r.stacks, 2);
    }

    #[test]
    fn cleanse_saturates_at_zero() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(2);
        r.cleanse(10);
        assert_eq!(r.stacks, 0);
    }

    #[test]
    fn cleanse_no_op_when_disabled() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(3);
        r.tick();
        r.enabled = false;
        r.cleanse(3);
        assert_eq!(r.stacks, 3);
    }

    #[test]
    fn cleanse_no_op_at_zero_count() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(3);
        r.tick();
        r.cleanse(0);
        assert_eq!(r.stacks, 3);
    }

    #[test]
    fn tick_clears_just_maxed() {
        let mut r = Rupture::new(3, 0.2);
        r.apply(3);
        r.tick();
        assert!(!r.just_maxed);
    }

    #[test]
    fn burst_damage_at_one_stack() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(1);
        // 10.0 * 0.2 * 1 = 2.0
        assert!((r.burst_damage(10.0) - 2.0).abs() < 1e-4);
    }

    #[test]
    fn burst_damage_at_max_stacks() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(5);
        // 10.0 * 0.2 * 5 = 10.0
        assert!((r.burst_damage(10.0) - 10.0).abs() < 1e-4);
    }

    #[test]
    fn burst_damage_zero_when_no_stacks() {
        let r = Rupture::new(5, 0.2);
        assert_eq!(r.burst_damage(100.0), 0.0);
    }

    #[test]
    fn burst_damage_zero_when_disabled() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(3);
        r.tick();
        r.enabled = false;
        assert_eq!(r.burst_damage(100.0), 0.0);
    }

    #[test]
    fn burst_damage_floored_at_zero() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(3);
        assert_eq!(r.burst_damage(-100.0), 0.0);
    }

    #[test]
    fn is_ruptured_false_at_zero_stacks() {
        let r = Rupture::new(5, 0.2);
        assert!(!r.is_ruptured());
    }

    #[test]
    fn is_ruptured_true_with_stacks() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(1);
        assert!(r.is_ruptured());
    }

    #[test]
    fn is_ruptured_false_when_disabled() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(3);
        r.enabled = false;
        assert!(!r.is_ruptured());
    }

    #[test]
    fn at_max_true_at_capacity() {
        let mut r = Rupture::new(3, 0.2);
        r.apply(3);
        assert!(r.at_max());
    }

    #[test]
    fn at_max_false_below_capacity() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(3);
        assert!(!r.at_max());
    }

    #[test]
    fn stack_fraction_at_half() {
        let mut r = Rupture::new(4, 0.2);
        r.apply(2);
        assert!((r.stack_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_at_full() {
        let mut r = Rupture::new(4, 0.2);
        r.apply(4);
        assert!((r.stack_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_at_zero() {
        let r = Rupture::new(4, 0.2);
        assert!(r.stack_fraction().abs() < 1e-5);
    }

    #[test]
    fn max_stacks_clamped_to_one() {
        let r = Rupture::new(0, 0.2);
        assert_eq!(r.max_stacks, 1);
    }

    #[test]
    fn damage_per_stack_clamped_non_negative() {
        let r = Rupture::new(5, -1.0);
        assert_eq!(r.damage_per_stack, 0.0);
    }

    #[test]
    fn apply_and_cleanse_cycle() {
        let mut r = Rupture::new(5, 0.2);
        r.apply(3);
        r.tick();
        r.cleanse(1);
        assert_eq!(r.stacks, 2);
        assert!((r.burst_damage(10.0) - 4.0).abs() < 1e-4); // 10 * 0.2 * 2
    }

    #[test]
    fn re_maxes_after_cleanse() {
        let mut r = Rupture::new(3, 0.2);
        r.apply(3); // maxed
        r.tick();
        r.cleanse(2); // back to 1
        r.tick();
        r.apply(2); // back to max
        assert!(r.just_maxed);
    }
}

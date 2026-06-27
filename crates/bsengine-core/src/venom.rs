use bevy_ecs::prelude::Component;

/// Stacking toxic DoT: each application adds one stack (capped at `max_stacks`),
/// and the total damage per frame scales with the current stack count.
///
/// Each `apply()` call resets the shared timer to `duration`, keeping the venom
/// alive as long as hits keep coming. Once the timer expires all stacks are cleared.
/// Call `tick(dt)` each frame — it returns the damage to apply this frame
/// (`damage_per_stack * stacks * dt`) and manages the timer.
///
/// Distinct from `Poison` (flat rate, single application), `Bleed` (physical wound),
/// and `Burn` (fire amplification): `Venom` rewards repeated attacks with
/// geometrically scaling damage.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Venom {
    /// Damage per stack per second.
    pub damage_per_stack: f32,
    /// Currently active stacks.
    pub stacks: u32,
    /// Maximum stacks that can accumulate.
    pub max_stacks: u32,
    /// Duration in seconds before stacks expire (reset on each apply).
    pub duration: f32,
    pub timer: f32,
    pub just_applied: bool,
    pub just_expired: bool,
    pub enabled: bool,
}

impl Venom {
    pub fn new(damage_per_stack: f32, max_stacks: u32) -> Self {
        Self {
            damage_per_stack: damage_per_stack.max(0.0),
            stacks: 0,
            max_stacks: max_stacks.max(1),
            duration: 3.0,
            timer: 0.0,
            just_applied: false,
            just_expired: false,
            enabled: true,
        }
    }

    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration.max(0.0);
        self
    }

    /// Add one stack and reset the timer. No-op if disabled or at max stacks.
    pub fn apply(&mut self) {
        if !self.enabled {
            return;
        }

        if self.stacks < self.max_stacks {
            self.stacks += 1;
        }
        self.timer = self.duration;
        self.just_applied = true;
    }

    /// Add `n` stacks at once and reset the timer.
    pub fn apply_n(&mut self, n: u32) {
        if !self.enabled {
            return;
        }

        self.stacks = (self.stacks + n).min(self.max_stacks);
        self.timer = self.duration;
        self.just_applied = true;
    }

    /// Remove all stacks immediately.
    pub fn clear(&mut self) {
        self.stacks = 0;
        self.timer = 0.0;
        self.just_expired = false;
    }

    /// Advance the timer and return damage to apply this frame.
    /// Returns `0.0` when inactive or disabled.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_applied = false;
        self.just_expired = false;

        if !self.enabled || !self.is_active() {
            return 0.0;
        }

        let damage = self.damage_per_stack * self.stacks as f32 * dt;

        self.timer -= dt;
        if self.timer <= 0.0 {
            self.timer = 0.0;
            self.stacks = 0;
            self.just_expired = true;
        }

        damage
    }

    pub fn is_active(&self) -> bool {
        self.stacks > 0 && self.timer > 0.0
    }

    /// Total damage per second at the current stack count.
    pub fn damage_per_second(&self) -> f32 {
        self.damage_per_stack * self.stacks as f32
    }

    /// Fraction of the venom duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Venom {
    fn default() -> Self {
        Self::new(5.0, 5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_adds_stack_and_starts_timer() {
        let mut v = Venom::new(10.0, 5).with_duration(3.0);
        v.apply();
        assert_eq!(v.stacks, 1);
        assert!(v.is_active());
        assert!(v.just_applied);
    }

    #[test]
    fn apply_capped_at_max_stacks() {
        let mut v = Venom::new(10.0, 3).with_duration(3.0);
        v.apply();
        v.apply();
        v.apply();
        v.apply(); // 4th exceeds max
        assert_eq!(v.stacks, 3);
    }

    #[test]
    fn apply_n_adds_multiple_stacks() {
        let mut v = Venom::new(10.0, 5).with_duration(3.0);
        v.apply_n(3);
        assert_eq!(v.stacks, 3);
    }

    #[test]
    fn apply_resets_timer() {
        let mut v = Venom::new(10.0, 5).with_duration(3.0);
        v.apply();
        v.tick(2.0);
        v.apply(); // timer resets to 3.0
        assert!((v.timer - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_returns_scaled_damage() {
        let mut v = Venom::new(10.0, 5).with_duration(3.0);
        v.apply_n(2); // 2 stacks × 10 = 20 dps
        let dmg = v.tick(0.1);
        assert!((dmg - 2.0).abs() < 1e-4); // 20 * 0.1
    }

    #[test]
    fn tick_expires_on_timer_end() {
        let mut v = Venom::new(10.0, 5).with_duration(1.0);
        v.apply();
        v.tick(1.1);
        assert!(!v.is_active());
        assert!(v.just_expired);
        assert_eq!(v.stacks, 0);
    }

    #[test]
    fn tick_zero_when_inactive() {
        let mut v = Venom::new(10.0, 5).with_duration(1.0);
        let dmg = v.tick(0.1);
        assert!((dmg - 0.0).abs() < 1e-5);
    }

    #[test]
    fn clear_removes_all_stacks() {
        let mut v = Venom::new(10.0, 5).with_duration(3.0);
        v.apply_n(3);
        v.clear();
        assert!(!v.is_active());
        assert_eq!(v.stacks, 0);
    }

    #[test]
    fn damage_per_second_scales_with_stacks() {
        let mut v = Venom::new(5.0, 10).with_duration(3.0);
        v.apply_n(4);
        assert!((v.damage_per_second() - 20.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut v = Venom::new(10.0, 5).with_duration(2.0);
        v.apply();
        v.tick(1.0);
        assert!((v.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut v = Venom::new(10.0, 5).with_duration(3.0);
        v.enabled = false;
        v.apply();
        assert!(!v.is_active());
    }

    #[test]
    fn disabled_tick_returns_zero() {
        let mut v = Venom::new(10.0, 5).with_duration(3.0);
        v.apply();
        v.enabled = false;
        let dmg = v.tick(0.1);
        assert!((dmg - 0.0).abs() < 1e-5);
    }
}

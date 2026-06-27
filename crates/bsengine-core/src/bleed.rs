use bevy_ecs::prelude::Component;

/// Bleeding damage-over-time with stackable wound severity.
///
/// Each `apply()` call adds one stack, up to `max_stacks`. Every `tick_interval`
/// seconds the system should drain `pending_damage` and apply it to `Health`.
/// Stacks are unified under a single `duration` timer — the entire bleed is
/// refreshed on each new application (no per-stack expiry tracking).
///
/// `heal_reduction` is a multiplier on incoming healing while bleeding is active
/// (0.0 = no healing; 1.0 = full healing). The health system reads this value.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Bleed {
    pub stacks: u32,
    /// Maximum stacks that can accumulate.
    pub max_stacks: u32,
    /// Damage dealt per stack per tick (not per second).
    pub damage_per_stack_per_tick: f32,
    /// How often (in seconds) a damage tick occurs.
    pub tick_interval: f32,
    pub tick_timer: f32,
    /// How long (in seconds) the bleed lasts. Refreshed on each `apply()`.
    pub duration: f32,
    pub duration_timer: f32,
    /// Multiplier on incoming healing while bleeding [0.0, 1.0].
    pub heal_reduction: f32,
    /// Damage accumulated since last drain; cleared by the health system.
    pub pending_damage: f32,
    /// True on the first frame a stack is applied (including reapply).
    pub just_applied: bool,
    /// True on the first frame all stacks expire.
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Bleed {
    pub fn new(damage_per_stack_per_tick: f32, tick_interval: f32, duration: f32) -> Self {
        Self {
            stacks: 0,
            max_stacks: 5,
            damage_per_stack_per_tick: damage_per_stack_per_tick.max(0.0),
            tick_interval: tick_interval.max(0.01),
            tick_timer: 0.0,
            duration: duration.max(0.0),
            duration_timer: 0.0,
            heal_reduction: 0.5,
            pending_damage: 0.0,
            just_applied: false,
            just_cleared: false,
            enabled: true,
        }
    }

    pub fn with_max_stacks(mut self, max: u32) -> Self {
        self.max_stacks = max.max(1);
        self
    }

    pub fn with_heal_reduction(mut self, reduction: f32) -> Self {
        self.heal_reduction = reduction.clamp(0.0, 1.0);
        self
    }

    /// Add one bleed stack and refresh the duration timer.
    ///
    /// Does nothing if `enabled` is false. Caps at `max_stacks`.
    pub fn apply(&mut self) {
        if !self.enabled {
            return;
        }
        self.stacks = (self.stacks + 1).min(self.max_stacks);
        self.duration_timer = 0.0;
        self.just_applied = true;
    }

    /// Remove all stacks immediately (e.g. on cleanse).
    pub fn clear(&mut self) {
        if self.stacks > 0 {
            self.stacks = 0;
            self.pending_damage = 0.0;
            self.tick_timer = 0.0;
            self.duration_timer = 0.0;
            self.just_cleared = true;
        }
    }

    /// Advance timers, emit damage ticks, and expire the bleed when due.
    pub fn tick(&mut self, dt: f32) {
        self.just_applied = false;
        self.just_cleared = false;

        if self.stacks == 0 {
            return;
        }

        self.tick_timer += dt;
        self.duration_timer += dt;

        if self.tick_timer >= self.tick_interval {
            self.tick_timer -= self.tick_interval;
            self.pending_damage += self.stacks as f32 * self.damage_per_stack_per_tick;
        }

        if self.duration_timer >= self.duration {
            self.stacks = 0;
            self.pending_damage = 0.0;
            self.tick_timer = 0.0;
            self.duration_timer = 0.0;
            self.just_cleared = true;
        }
    }

    /// Drain all pending damage and return it. Call each frame from the health system.
    pub fn drain(&mut self) -> f32 {
        let dmg = self.pending_damage;
        self.pending_damage = 0.0;
        dmg
    }

    pub fn is_active(&self) -> bool {
        self.stacks > 0
    }

    /// Total damage per tick across all stacks.
    pub fn total_damage_per_tick(&self) -> f32 {
        self.stacks as f32 * self.damage_per_stack_per_tick
    }

    /// Fraction of the bleed duration elapsed [0.0, 1.0].
    pub fn duration_fraction(&self) -> f32 {
        if self.duration <= 0.0 || self.stacks == 0 {
            return 0.0;
        }
        (self.duration_timer / self.duration).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_increments_stacks() {
        let mut b = Bleed::new(5.0, 1.0, 5.0);
        b.apply();
        b.apply();
        assert_eq!(b.stacks, 2);
        assert!(b.just_applied);
    }

    #[test]
    fn apply_caps_at_max_stacks() {
        let mut b = Bleed::new(5.0, 1.0, 5.0).with_max_stacks(3);
        for _ in 0..5 {
            b.apply();
        }
        assert_eq!(b.stacks, 3);
    }

    #[test]
    fn apply_refreshes_duration() {
        let mut b = Bleed::new(5.0, 1.0, 5.0);
        b.apply();
        b.tick(3.0);
        b.apply(); // refresh
        assert_eq!(b.duration_timer, 0.0);
        assert!(b.is_active());
    }

    #[test]
    fn tick_emits_damage() {
        let mut b = Bleed::new(10.0, 1.0, 10.0);
        b.apply();
        b.tick(1.0);
        assert!(b.pending_damage > 0.0);
        let dmg = b.drain();
        assert!((dmg - 10.0).abs() < 1e-4);
        assert_eq!(b.pending_damage, 0.0);
    }

    #[test]
    fn tick_scales_with_stacks() {
        let mut b = Bleed::new(5.0, 1.0, 10.0);
        b.apply();
        b.apply();
        b.tick(1.0);
        assert!((b.pending_damage - 10.0).abs() < 1e-4);
    }

    #[test]
    fn expires_after_duration() {
        let mut b = Bleed::new(5.0, 0.5, 2.0);
        b.apply();
        b.tick(2.1);
        assert!(!b.is_active());
        assert!(b.just_cleared);
    }

    #[test]
    fn clear_removes_all_stacks() {
        let mut b = Bleed::new(5.0, 1.0, 5.0);
        b.apply();
        b.apply();
        b.clear();
        assert_eq!(b.stacks, 0);
        assert!(b.just_cleared);
        assert_eq!(b.pending_damage, 0.0);
    }

    #[test]
    fn disabled_ignores_apply() {
        let mut b = Bleed::new(5.0, 1.0, 5.0);
        b.enabled = false;
        b.apply();
        assert_eq!(b.stacks, 0);
    }

    #[test]
    fn duration_fraction_in_range() {
        let mut b = Bleed::new(5.0, 1.0, 4.0);
        b.apply();
        b.tick(1.0);
        let frac = b.duration_fraction();
        assert!((frac - 0.25).abs() < 1e-4);
    }
}

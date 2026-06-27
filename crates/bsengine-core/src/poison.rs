use bevy_ecs::prelude::Component;

/// Poison damage-over-time with stacking severity and virulence spread flag.
///
/// Unlike `Bleed`, poison does not reduce healing but each additional stack
/// decreases `tick_interval`, making higher stacks tick faster (intensifying
/// toxin). The health system reads `pending_damage` via `drain()` each frame.
///
/// Call `apply()` to add a stack (refreshes duration). `tick(dt)` advances
/// timers and accumulates `pending_damage`. `clear()` purges all stacks.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Poison {
    pub stacks: u32,
    /// Maximum stacks before capping.
    pub max_stacks: u32,
    /// Damage per tick per stack. Multiplied by `stacks` each interval.
    pub damage_per_stack_per_tick: f32,
    /// Base tick interval in seconds (full tick period with 1 stack).
    pub base_tick_interval: f32,
    /// Minimum tick interval when stacks are maxed (clamp floor).
    pub min_tick_interval: f32,
    pub tick_timer: f32,
    /// Duration of the poison effect; refreshed on each `apply()`.
    pub duration: f32,
    pub duration_timer: f32,
    /// When true, the poison system should attempt to spread this to
    /// nearby entities (spreading logic lives in the system, not here).
    pub virulent: bool,
    /// Accumulated damage to apply to Health; cleared by health system.
    pub pending_damage: f32,
    /// True on the first frame a stack is applied (including reapply).
    pub just_poisoned: bool,
    /// True on the first frame all stacks expire.
    pub just_cured: bool,
    pub enabled: bool,
}

impl Poison {
    pub fn new(damage_per_stack_per_tick: f32, base_tick_interval: f32, duration: f32) -> Self {
        Self {
            stacks: 0,
            max_stacks: 8,
            damage_per_stack_per_tick: damage_per_stack_per_tick.max(0.0),
            base_tick_interval: base_tick_interval.max(0.05),
            min_tick_interval: base_tick_interval * 0.25,
            tick_timer: 0.0,
            duration: duration.max(0.0),
            duration_timer: 0.0,
            virulent: false,
            pending_damage: 0.0,
            just_poisoned: false,
            just_cured: false,
            enabled: true,
        }
    }

    pub fn with_max_stacks(mut self, max: u32) -> Self {
        self.max_stacks = max.max(1);
        self
    }

    pub fn with_min_interval(mut self, min: f32) -> Self {
        self.min_tick_interval = min.max(0.01);
        self
    }

    pub fn virulent(mut self) -> Self {
        self.virulent = true;
        self
    }

    /// Add one stack and refresh the duration timer. Caps at `max_stacks`.
    pub fn apply(&mut self) {
        if !self.enabled {
            return;
        }
        self.stacks = (self.stacks + 1).min(self.max_stacks);
        self.duration_timer = 0.0;
        self.just_poisoned = true;
    }

    /// Remove all stacks (antidote / cleanse).
    pub fn clear(&mut self) {
        if self.stacks > 0 {
            self.stacks = 0;
            self.pending_damage = 0.0;
            self.tick_timer = 0.0;
            self.duration_timer = 0.0;
            self.just_cured = true;
        }
    }

    /// Current effective tick interval — shorter with more stacks.
    ///
    /// Linear interpolation: 1 stack → `base_tick_interval`,
    /// `max_stacks` stacks → `min_tick_interval`.
    pub fn effective_interval(&self) -> f32 {
        if self.max_stacks <= 1 || self.stacks == 0 {
            return self.base_tick_interval;
        }
        let t = (self.stacks - 1) as f32 / (self.max_stacks - 1) as f32;
        let interval =
            self.base_tick_interval + t * (self.min_tick_interval - self.base_tick_interval);
        interval.max(self.min_tick_interval)
    }

    /// Advance timers, emit damage ticks, and expire when duration elapses.
    pub fn tick(&mut self, dt: f32) {
        self.just_poisoned = false;
        self.just_cured = false;

        if self.stacks == 0 {
            return;
        }

        let interval = self.effective_interval();
        self.tick_timer += dt;
        self.duration_timer += dt;

        if self.tick_timer >= interval {
            self.tick_timer -= interval;
            self.pending_damage += self.stacks as f32 * self.damage_per_stack_per_tick;
        }

        if self.duration_timer >= self.duration {
            self.stacks = 0;
            self.pending_damage = 0.0;
            self.tick_timer = 0.0;
            self.duration_timer = 0.0;
            self.just_cured = true;
        }
    }

    /// Drain and return accumulated damage. Called by the health system.
    pub fn drain(&mut self) -> f32 {
        let dmg = self.pending_damage;
        self.pending_damage = 0.0;
        dmg
    }

    pub fn is_active(&self) -> bool {
        self.stacks > 0
    }

    /// Fraction of the poison duration elapsed [0.0, 1.0].
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
        let mut p = Poison::new(3.0, 1.0, 5.0);
        p.apply();
        p.apply();
        assert_eq!(p.stacks, 2);
        assert!(p.just_poisoned);
    }

    #[test]
    fn apply_caps_at_max_stacks() {
        let mut p = Poison::new(3.0, 1.0, 5.0).with_max_stacks(3);
        for _ in 0..5 {
            p.apply();
        }
        assert_eq!(p.stacks, 3);
    }

    #[test]
    fn apply_refreshes_duration() {
        let mut p = Poison::new(3.0, 1.0, 5.0);
        p.apply();
        p.tick(3.0);
        p.apply();
        assert_eq!(p.duration_timer, 0.0);
        assert!(p.is_active());
    }

    #[test]
    fn tick_emits_damage() {
        let mut p = Poison::new(5.0, 1.0, 10.0);
        p.apply();
        p.tick(1.0);
        let dmg = p.drain();
        assert!((dmg - 5.0).abs() < 1e-4);
        assert_eq!(p.pending_damage, 0.0);
    }

    #[test]
    fn higher_stacks_increase_damage_per_tick() {
        let mut p = Poison::new(5.0, 1.0, 10.0);
        p.apply();
        p.apply();
        p.tick(1.0);
        let dmg = p.drain();
        assert!((dmg - 10.0).abs() < 1e-4);
    }

    #[test]
    fn effective_interval_decreases_with_stacks() {
        let p1 = Poison::new(5.0, 1.0, 10.0).with_max_stacks(4);
        let mut p4 = Poison::new(5.0, 1.0, 10.0).with_max_stacks(4);
        for _ in 0..4 {
            p4.apply();
        }
        assert!(p4.effective_interval() < p1.base_tick_interval);
    }

    #[test]
    fn expires_after_duration() {
        let mut p = Poison::new(5.0, 0.5, 2.0);
        p.apply();
        p.tick(2.1);
        assert!(!p.is_active());
        assert!(p.just_cured);
    }

    #[test]
    fn clear_removes_all_stacks() {
        let mut p = Poison::new(5.0, 1.0, 5.0);
        p.apply();
        p.apply();
        p.clear();
        assert_eq!(p.stacks, 0);
        assert!(p.just_cured);
    }

    #[test]
    fn disabled_ignores_apply() {
        let mut p = Poison::new(5.0, 1.0, 5.0);
        p.enabled = false;
        p.apply();
        assert_eq!(p.stacks, 0);
    }

    #[test]
    fn duration_fraction_in_range() {
        let mut p = Poison::new(5.0, 1.0, 4.0);
        p.apply();
        p.tick(1.0);
        let frac = p.duration_fraction();
        assert!((frac - 0.25).abs() < 1e-4);
    }
}

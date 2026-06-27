use bevy_ecs::prelude::Component;

/// Tracks persistent injuries that reduce maximum health and deal slow bleed.
///
/// Unlike `Bleed` (a temporary DoT debuff), wounds are semi-permanent injuries
/// that cap the entity's effective max HP. Each wound reduces the max-HP
/// fraction by `max_health_reduction_per_wound` and adds `bleed_per_wound`
/// passive damage per second. Wounds can only be removed by specific healing
/// actions (bandaging, regeneration above a threshold, etc.).
///
/// The health system multiplies `Health::max` by `max_health_multiplier()` and
/// applies `bleed_damage_per_sec()` each frame. `tick(dt)` accumulates pending
/// bleed into `pending_damage`; the caller drains it with `drain()`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wound {
    /// Number of active wounds.
    pub count: u32,
    /// Maximum wounds before the entity is considered critically wounded.
    pub max_count: u32,
    /// Max-HP fraction lost per wound (e.g. 0.1 = 10% per wound).
    pub max_health_reduction_per_wound: f32,
    /// Bleed damage per second per wound (can be 0 for wounds without bleed).
    pub bleed_per_wound: f32,
    /// Bleed tick interval in seconds.
    pub bleed_interval: f32,
    pub bleed_timer: f32,
    /// Accumulated bleed damage ready to drain.
    pub pending_damage: f32,
    /// True on the first frame a wound is added.
    pub just_wounded: bool,
    /// True on the first frame a wound is removed.
    pub just_healed: bool,
    pub enabled: bool,
}

impl Wound {
    pub fn new(max_health_reduction_per_wound: f32) -> Self {
        Self {
            count: 0,
            max_count: 5,
            max_health_reduction_per_wound: max_health_reduction_per_wound.clamp(0.0, 1.0),
            bleed_per_wound: 0.0,
            bleed_interval: 1.0,
            bleed_timer: 0.0,
            pending_damage: 0.0,
            just_wounded: false,
            just_healed: false,
            enabled: true,
        }
    }

    pub fn with_bleed(mut self, bleed_per_wound: f32, interval: f32) -> Self {
        self.bleed_per_wound = bleed_per_wound.max(0.0);
        self.bleed_interval = interval.max(0.01);
        self
    }

    pub fn with_max_count(mut self, max: u32) -> Self {
        self.max_count = max;
        self
    }

    /// Add one wound (clamped to `max_count`).
    pub fn add(&mut self) {
        if !self.enabled || self.count >= self.max_count {
            return;
        }
        self.count += 1;
        self.just_wounded = true;
    }

    /// Remove one wound (bandage / heal action).
    pub fn heal_one(&mut self) {
        if self.count == 0 {
            return;
        }
        self.count -= 1;
        self.just_healed = true;
    }

    /// Remove all wounds instantly.
    pub fn heal_all(&mut self) {
        if self.count == 0 {
            return;
        }
        self.count = 0;
        self.just_healed = true;
    }

    /// Advance bleed timer and accumulate bleed damage.
    pub fn tick(&mut self, dt: f32) {
        self.just_wounded = false;
        self.just_healed = false;

        if !self.enabled || self.count == 0 || self.bleed_per_wound <= 0.0 {
            return;
        }

        self.bleed_timer += dt;
        while self.bleed_timer >= self.bleed_interval {
            self.bleed_timer -= self.bleed_interval;
            self.pending_damage += self.bleed_per_wound * self.count as f32;
        }
    }

    /// Drain accumulated bleed damage (health system calls this).
    pub fn drain(&mut self) -> f32 {
        let dmg = self.pending_damage;
        self.pending_damage = 0.0;
        dmg
    }

    /// Max-HP multiplier: 1.0 - (count * reduction_per_wound), clamped to [0.05, 1.0].
    pub fn max_health_multiplier(&self) -> f32 {
        let reduction = self.count as f32 * self.max_health_reduction_per_wound;
        (1.0 - reduction).clamp(0.05, 1.0)
    }

    pub fn is_critically_wounded(&self) -> bool {
        self.count >= self.max_count
    }

    pub fn bleed_damage_per_sec(&self) -> f32 {
        if self.bleed_interval <= 0.0 {
            return 0.0;
        }
        self.bleed_per_wound * self.count as f32 / self.bleed_interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_increases_count() {
        let mut w = Wound::new(0.1);
        w.add();
        assert_eq!(w.count, 1);
        assert!(w.just_wounded);
    }

    #[test]
    fn add_clamps_at_max() {
        let mut w = Wound::new(0.1).with_max_count(2);
        w.add();
        w.add();
        w.add(); // should be clamped
        assert_eq!(w.count, 2);
    }

    #[test]
    fn heal_one_decrements() {
        let mut w = Wound::new(0.1);
        w.add();
        w.add();
        w.heal_one();
        assert_eq!(w.count, 1);
        assert!(w.just_healed);
    }

    #[test]
    fn heal_all_clears() {
        let mut w = Wound::new(0.1);
        w.add();
        w.add();
        w.heal_all();
        assert_eq!(w.count, 0);
        assert!(w.just_healed);
    }

    #[test]
    fn max_health_multiplier_reduces_per_wound() {
        let mut w = Wound::new(0.1);
        w.add(); // 1 wound → 0.9
        assert!((w.max_health_multiplier() - 0.9).abs() < 1e-5);
        w.add(); // 2 wounds → 0.8
        assert!((w.max_health_multiplier() - 0.8).abs() < 1e-5);
    }

    #[test]
    fn max_health_multiplier_floor() {
        let mut w = Wound::new(0.5).with_max_count(10);
        for _ in 0..10 {
            w.add();
        }
        assert!(w.max_health_multiplier() >= 0.05);
    }

    #[test]
    fn bleed_accumulates_on_tick() {
        let mut w = Wound::new(0.1).with_bleed(5.0, 1.0);
        w.add();
        w.tick(1.0);
        assert!((w.pending_damage - 5.0).abs() < 1e-5);
    }

    #[test]
    fn drain_returns_and_clears() {
        let mut w = Wound::new(0.1).with_bleed(5.0, 1.0);
        w.add();
        w.tick(1.0);
        let dmg = w.drain();
        assert!((dmg - 5.0).abs() < 1e-5);
        assert_eq!(w.pending_damage, 0.0);
    }

    #[test]
    fn disabled_ignores_add() {
        let mut w = Wound::new(0.1);
        w.enabled = false;
        w.add();
        assert_eq!(w.count, 0);
    }

    #[test]
    fn critically_wounded_at_max() {
        let mut w = Wound::new(0.1).with_max_count(3);
        for _ in 0..3 {
            w.add();
        }
        assert!(w.is_critically_wounded());
    }
}

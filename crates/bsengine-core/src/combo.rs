use bevy_ecs::prelude::Component;

/// Attack-combo counter for action/fighting games.
///
/// Tracks consecutive hits in a time window. The combo resets if `reset_timeout`
/// seconds pass without a registered hit. On reset, `just_reset` fires for one
/// frame.
///
/// Distinct from `melee` (which models attack animation phases) and `damage`
/// (a per-hit data packet). `Combo` is a persistent meta-counter that lives on
/// the attacker and informs damage scaling, animations, and UI.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Combo {
    /// Current consecutive-hit streak.
    pub count: u32,
    /// Maximum count to clamp at (0 = no limit).
    pub max_count: u32,
    /// Highest streak reached in this fight/encounter.
    pub peak_count: u32,
    /// Seconds without a hit before the combo resets.
    pub reset_timeout: f32,
    /// Countdown to reset. Refreshed on every `on_hit()` call.
    pub reset_timer: f32,
    /// True on the exact frame the combo drops to zero.
    pub just_reset: bool,
    pub enabled: bool,
}

impl Combo {
    pub fn new(reset_timeout: f32) -> Self {
        Self {
            count: 0,
            max_count: 0,
            peak_count: 0,
            reset_timeout: reset_timeout.max(0.0),
            reset_timer: 0.0,
            just_reset: false,
            enabled: true,
        }
    }

    pub fn with_max(mut self, max_count: u32) -> Self {
        self.max_count = max_count;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Register a hit. Returns the new combo count.
    /// No-ops when disabled.
    pub fn on_hit(&mut self) -> u32 {
        if !self.enabled {
            return self.count;
        }
        self.count = if self.max_count > 0 {
            (self.count + 1).min(self.max_count)
        } else {
            self.count + 1
        };
        if self.count > self.peak_count {
            self.peak_count = self.count;
        }
        self.reset_timer = self.reset_timeout;
        self.count
    }

    /// Immediately drop the combo to zero (e.g., player was staggered).
    pub fn reset(&mut self) {
        if self.count > 0 {
            self.count = 0;
            self.reset_timer = 0.0;
            self.just_reset = true;
        }
    }

    /// Advance the reset timer. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_reset = false;

        if !self.enabled || self.count == 0 {
            return;
        }

        if self.reset_timeout > 0.0 {
            self.reset_timer = (self.reset_timer - dt).max(0.0);
            if self.reset_timer <= 0.0 {
                self.count = 0;
                self.just_reset = true;
            }
        }
    }

    /// [0, 1] ratio of current count to max_count. Returns 0 if no max.
    pub fn fill_fraction(&self) -> f32 {
        if self.max_count == 0 {
            0.0
        } else {
            self.count as f32 / self.max_count as f32
        }
    }

    pub fn is_active(&self) -> bool {
        self.enabled && self.count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter() -> Combo {
        Combo::new(2.0)
    }

    #[test]
    fn on_hit_increments_count() {
        let mut c = counter();
        assert_eq!(c.on_hit(), 1);
        assert_eq!(c.on_hit(), 2);
        assert_eq!(c.count, 2);
    }

    #[test]
    fn peak_tracked() {
        let mut c = counter();
        c.on_hit();
        c.on_hit();
        c.on_hit();
        c.reset();
        assert_eq!(c.peak_count, 3);
    }

    #[test]
    fn timeout_resets_combo() {
        let mut c = counter();
        c.on_hit();
        c.tick(2.0);
        assert_eq!(c.count, 0);
        assert!(c.just_reset);
    }

    #[test]
    fn hit_refreshes_timer() {
        let mut c = counter();
        c.on_hit();
        c.tick(1.5);
        c.on_hit(); // refreshes timer
        c.tick(1.5);
        assert_eq!(c.count, 2); // still alive
    }

    #[test]
    fn max_count_clamps() {
        let mut c = Combo::new(5.0).with_max(3);
        for _ in 0..5 {
            c.on_hit();
        }
        assert_eq!(c.count, 3);
    }

    #[test]
    fn manual_reset_fires_event() {
        let mut c = counter();
        c.on_hit();
        c.reset();
        assert_eq!(c.count, 0);
        assert!(c.just_reset);
        c.tick(0.0);
        assert!(!c.just_reset);
    }

    #[test]
    fn disabled_blocks_hits() {
        let mut c = counter().disabled();
        c.on_hit();
        assert_eq!(c.count, 0);
    }
}

use bevy_ecs::prelude::Component;

/// Tracks physical overexertion, imposing speed and stamina-regen penalties.
///
/// `level` rises when the entity overexerts (caller increments it) and recovers
/// at `recovery_rate` per second when at rest. Once `level` crosses `threshold`
/// the entity is considered "exhausted" and the stamina and movement systems
/// apply `penalty_speed` / `penalty_regen` multipliers.
///
/// Systems should call `tick(dt)` each frame to advance recovery. `add(amount)`
/// increases exhaustion (e.g. from sprinting or heavy attacks). `clear()` resets
/// exhaustion instantly (e.g. a resting ability).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Exhaustion {
    /// Current exhaustion level [0.0, 1.0]. 0 = none, 1 = fully exhausted.
    pub level: f32,
    /// Exhaustion recovery rate per second (when not actively adding exhaustion).
    pub recovery_rate: f32,
    /// Exhaustion level at which penalties begin (default 0.8).
    pub threshold: f32,
    /// Speed multiplier applied when exhausted (e.g. 0.6 = 60% speed).
    pub penalty_speed: f32,
    /// Stamina regen multiplier applied when exhausted (e.g. 0.3 = 30% regen).
    pub penalty_regen: f32,
    /// True on the first frame level crosses `threshold` (exhaustion onset).
    pub just_exhausted: bool,
    /// True on the first frame level drops back below `threshold` (recovery).
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Exhaustion {
    pub fn new(recovery_rate: f32) -> Self {
        Self {
            level: 0.0,
            recovery_rate: recovery_rate.max(0.0),
            threshold: 0.8,
            penalty_speed: 0.6,
            penalty_regen: 0.3,
            just_exhausted: false,
            just_recovered: false,
            enabled: true,
        }
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn with_penalties(mut self, speed: f32, regen: f32) -> Self {
        self.penalty_speed = speed.clamp(0.0, 1.0);
        self.penalty_regen = regen.clamp(0.0, 1.0);
        self
    }

    /// Add `amount` exhaustion (clamped to [0, 1]).
    pub fn add(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        let was_below = self.level < self.threshold;
        self.level = (self.level + amount.max(0.0)).min(1.0);
        if was_below && self.level >= self.threshold {
            self.just_exhausted = true;
        }
    }

    /// Recover exhaustion passively; call once per frame with `dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_exhausted = false;
        self.just_recovered = false;

        if !self.enabled || self.level <= 0.0 || self.recovery_rate <= 0.0 {
            return;
        }

        let was_above = self.level >= self.threshold;
        self.level = (self.level - self.recovery_rate * dt).max(0.0);
        if was_above && self.level < self.threshold {
            self.just_recovered = true;
        }
    }

    /// Reset exhaustion to zero immediately.
    pub fn clear(&mut self) {
        let was_above = self.level >= self.threshold;
        self.level = 0.0;
        if was_above {
            self.just_recovered = true;
        }
    }

    pub fn is_exhausted(&self) -> bool {
        self.level >= self.threshold
    }

    /// Speed multiplier to apply: 1.0 when not exhausted, `penalty_speed` when exhausted.
    pub fn effective_speed_multiplier(&self) -> f32 {
        if self.is_exhausted() {
            self.penalty_speed
        } else {
            1.0
        }
    }

    /// Stamina-regen multiplier to apply: 1.0 when not exhausted, `penalty_regen` when exhausted.
    pub fn effective_regen_multiplier(&self) -> f32 {
        if self.is_exhausted() {
            self.penalty_regen
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_increases_level() {
        let mut e = Exhaustion::new(0.1);
        e.add(0.5);
        assert!((e.level - 0.5).abs() < 1e-5);
    }

    #[test]
    fn add_clamps_at_one() {
        let mut e = Exhaustion::new(0.1);
        e.add(0.6);
        e.add(0.6);
        assert!((e.level - 1.0).abs() < 1e-5);
    }

    #[test]
    fn just_exhausted_on_threshold_cross() {
        let mut e = Exhaustion::new(0.1).with_threshold(0.5);
        e.add(0.6);
        assert!(e.just_exhausted);
        assert!(e.is_exhausted());
    }

    #[test]
    fn tick_recovers_level() {
        let mut e = Exhaustion::new(0.2);
        e.add(0.6);
        e.tick(1.0); // recover 0.2
        assert!((e.level - 0.4).abs() < 1e-4);
    }

    #[test]
    fn just_recovered_on_threshold_cross() {
        let mut e = Exhaustion::new(0.5).with_threshold(0.5);
        e.add(0.8);
        e.tick(0.01); // clear just_exhausted
        e.tick(1.0); // recover past threshold
        assert!(e.just_recovered);
        assert!(!e.is_exhausted());
    }

    #[test]
    fn clear_resets_level() {
        let mut e = Exhaustion::new(0.1);
        e.add(1.0);
        e.clear();
        assert_eq!(e.level, 0.0);
        assert!(e.just_recovered);
    }

    #[test]
    fn disabled_ignores_add() {
        let mut e = Exhaustion::new(0.1);
        e.enabled = false;
        e.add(0.5);
        assert_eq!(e.level, 0.0);
    }

    #[test]
    fn effective_multipliers_when_exhausted() {
        let mut e = Exhaustion::new(0.1)
            .with_threshold(0.5)
            .with_penalties(0.6, 0.3);
        e.add(0.6);
        assert!((e.effective_speed_multiplier() - 0.6).abs() < 1e-5);
        assert!((e.effective_regen_multiplier() - 0.3).abs() < 1e-5);
    }

    #[test]
    fn effective_multipliers_when_not_exhausted() {
        let e = Exhaustion::new(0.1);
        assert!((e.effective_speed_multiplier() - 1.0).abs() < 1e-5);
        assert!((e.effective_regen_multiplier() - 1.0).abs() < 1e-5);
    }
}

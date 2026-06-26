use bevy_ecs::prelude::Component;

/// Physical stamina resource — consumed by sprinting, dodging, and heavy attacks.
///
/// The movement/ability system calls `spend(cost)` and checks `can_afford(cost)`.
/// The regen system calls `tick(dt)` each frame.
/// The exhaustion system reads `is_exhausted()` to prevent spending while depleted.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Stamina {
    pub current: f32,
    pub max: f32,
    /// Stamina regenerated per second.
    pub regen_rate: f32,
    /// Seconds after spending before regen resumes.
    pub regen_delay: f32,
    /// Remaining delay before regen resumes.
    pub regen_timer: f32,
    /// When current reaches 0 the entity is exhausted and cannot spend until
    /// it recovers to `exhaustion_recovery_threshold`.
    pub exhausted: bool,
    /// Fraction of max that must be recovered before exhaustion clears (e.g. 0.25).
    pub exhaustion_recovery_threshold: f32,
    pub regen_enabled: bool,
    pub enabled: bool,
}

impl Stamina {
    pub fn new(max: f32) -> Self {
        Self {
            current: max.max(0.0),
            max: max.max(0.0),
            regen_rate: 20.0,
            regen_delay: 1.0,
            regen_timer: 0.0,
            exhausted: false,
            exhaustion_recovery_threshold: 0.25,
            regen_enabled: true,
            enabled: true,
        }
    }

    pub fn with_regen(mut self, rate: f32) -> Self {
        self.regen_rate = rate.max(0.0);
        self
    }

    pub fn with_regen_delay(mut self, delay: f32) -> Self {
        self.regen_delay = delay.max(0.0);
        self
    }

    pub fn with_exhaustion_threshold(mut self, threshold: f32) -> Self {
        self.exhaustion_recovery_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Try to spend `cost` stamina. Returns `true` if successful.
    pub fn spend(&mut self, cost: f32) -> bool {
        if !self.enabled || self.exhausted || self.current < cost {
            return false;
        }
        self.current -= cost;
        self.regen_timer = self.regen_delay;
        if self.current <= 0.0 {
            self.current = 0.0;
            self.exhausted = true;
        }
        true
    }

    /// Restore `amount` stamina directly (e.g. from a potion), clamped to max.
    pub fn restore(&mut self, amount: f32) {
        self.current = (self.current + amount.max(0.0)).min(self.max);
        self.check_exhaustion_recovery();
    }

    /// Call each frame to regen and manage the exhaustion state.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || !self.regen_enabled {
            return;
        }
        if self.regen_timer > 0.0 {
            self.regen_timer -= dt;
            return;
        }
        if self.regen_rate > 0.0 && self.current < self.max {
            self.current = (self.current + self.regen_rate * dt).min(self.max);
            self.check_exhaustion_recovery();
        }
    }

    fn check_exhaustion_recovery(&mut self) {
        if self.exhausted && self.current >= self.max * self.exhaustion_recovery_threshold {
            self.exhausted = false;
        }
    }

    pub fn can_afford(&self, cost: f32) -> bool {
        self.enabled && !self.exhausted && self.current >= cost
    }

    pub fn is_exhausted(&self) -> bool {
        self.exhausted
    }

    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            return 0.0;
        }
        (self.current / self.max).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spend_succeeds_when_sufficient() {
        let mut s = Stamina::new(100.0);
        assert!(s.spend(40.0));
        assert!((s.current - 60.0).abs() < 0.001);
    }

    #[test]
    fn spend_causes_exhaustion_at_zero() {
        let mut s = Stamina::new(50.0);
        s.spend(50.0);
        assert!(s.is_exhausted());
        assert!(!s.spend(1.0));
    }

    #[test]
    fn exhaustion_clears_after_threshold_recovery() {
        let mut s = Stamina::new(100.0)
            .with_exhaustion_threshold(0.25)
            .with_regen(50.0);
        s.spend(100.0);
        assert!(s.is_exhausted());
        s.regen_timer = 0.0;
        s.tick(0.5);
        assert!(!s.is_exhausted());
    }

    #[test]
    fn regen_delay_blocks_regen() {
        let mut s = Stamina::new(100.0).with_regen(10.0).with_regen_delay(2.0);
        s.spend(50.0);
        s.tick(1.0);
        assert!((s.current - 50.0).abs() < 0.001);
    }

    #[test]
    fn fraction_correct() {
        let mut s = Stamina::new(200.0);
        s.spend(50.0);
        assert!((s.fraction() - 0.75).abs() < 0.001);
    }
}

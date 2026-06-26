use bevy_ecs::prelude::Component;

/// Mana / energy resource for ability costs and cooldown gating.
///
/// The ability system reads `current` before casting and calls `spend(cost)`.
/// The regen system calls `tick(dt)` each frame to regenerate.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Mana {
    pub current: f32,
    pub max: f32,
    /// Mana regenerated per second while `regen_enabled`.
    pub regen_rate: f32,
    /// Delay in seconds after spending mana before regen resumes.
    pub regen_delay: f32,
    /// Remaining delay before regen resumes. Set to `regen_delay` on each spend.
    pub regen_timer: f32,
    pub regen_enabled: bool,
    pub enabled: bool,
}

impl Mana {
    pub fn new(max: f32) -> Self {
        Self {
            current: max.max(0.0),
            max: max.max(0.0),
            regen_rate: 0.0,
            regen_delay: 0.0,
            regen_timer: 0.0,
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

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Spend `cost` mana. Returns `true` if sufficient mana was available.
    pub fn spend(&mut self, cost: f32) -> bool {
        if !self.enabled || self.current < cost {
            return false;
        }
        self.current -= cost;
        self.regen_timer = self.regen_delay;
        true
    }

    /// Restore `amount` mana directly, clamped to `max`.
    pub fn restore(&mut self, amount: f32) {
        self.current = (self.current + amount.max(0.0)).min(self.max);
    }

    /// Regenerate mana over `dt` seconds. Call each frame.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || !self.regen_enabled {
            return;
        }
        if self.regen_timer > 0.0 {
            self.regen_timer -= dt;
            return;
        }
        if self.regen_rate > 0.0 && self.current < self.max {
            self.restore(self.regen_rate * dt);
        }
    }

    pub fn is_full(&self) -> bool {
        self.current >= self.max
    }

    pub fn is_empty(&self) -> bool {
        self.current <= 0.0
    }

    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            return 0.0;
        }
        (self.current / self.max).clamp(0.0, 1.0)
    }

    pub fn can_afford(&self, cost: f32) -> bool {
        self.enabled && self.current >= cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spend_succeeds_when_sufficient() {
        let mut m = Mana::new(100.0);
        assert!(m.spend(40.0));
        assert!((m.current - 60.0).abs() < 0.001);
    }

    #[test]
    fn spend_fails_when_insufficient() {
        let mut m = Mana::new(30.0);
        assert!(!m.spend(50.0));
        assert!((m.current - 30.0).abs() < 0.001);
    }

    #[test]
    fn regen_restores_over_time() {
        let mut m = Mana::new(100.0).with_regen(10.0);
        m.spend(50.0);
        m.regen_timer = 0.0;
        m.tick(2.0);
        assert!((m.current - 70.0).abs() < 0.001);
    }

    #[test]
    fn regen_delay_blocks_regen() {
        let mut m = Mana::new(100.0).with_regen(10.0).with_regen_delay(2.0);
        m.spend(50.0);
        m.tick(1.0);
        assert!((m.current - 50.0).abs() < 0.001);
    }

    #[test]
    fn fraction_correct() {
        let mut m = Mana::new(200.0);
        m.spend(50.0);
        assert!((m.fraction() - 0.75).abs() < 0.001);
    }
}

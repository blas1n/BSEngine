use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Shield {
    pub current: f32,
    pub max: f32,
    /// HP per second restored while recharging.
    pub recharge_rate: f32,
    /// Seconds after last damage before recharging begins.
    pub recharge_delay: f32,
    /// Current cooldown countdown. Decrements each frame; recharge starts when it reaches 0.
    pub recharge_cooldown: f32,
}

impl Shield {
    pub fn new(max: f32) -> Self {
        let max = max.max(0.0);
        Self {
            current: max,
            max,
            recharge_rate: 0.0,
            recharge_delay: 0.0,
            recharge_cooldown: 0.0,
        }
    }

    pub fn with_recharge(mut self, rate: f32, delay: f32) -> Self {
        self.recharge_rate = rate.max(0.0);
        self.recharge_delay = delay.max(0.0);
        self
    }

    pub fn is_depleted(&self) -> bool {
        self.current <= 0.0
    }

    pub fn is_full(&self) -> bool {
        self.current >= self.max
    }

    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            0.0
        } else {
            (self.current / self.max).clamp(0.0, 1.0)
        }
    }

    /// Absorbs up to `amount` damage. Returns the remaining damage that passes through to Health.
    pub fn absorb(&mut self, amount: f32) -> f32 {
        let amount = amount.max(0.0);
        let absorbed = amount.min(self.current);
        self.current -= absorbed;
        if absorbed > 0.0 {
            self.recharge_cooldown = self.recharge_delay;
        }
        amount - absorbed
    }

    /// Advance recharge logic by `dt` seconds. Called by ShieldPlugin each frame.
    pub fn tick(&mut self, dt: f32) {
        if self.is_full() || self.recharge_rate <= 0.0 {
            return;
        }
        if self.recharge_cooldown > 0.0 {
            self.recharge_cooldown = (self.recharge_cooldown - dt).max(0.0);
        } else {
            self.current = (self.current + self.recharge_rate * dt).min(self.max);
        }
    }
}

impl Default for Shield {
    fn default() -> Self {
        Self::new(100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shield_starts_full() {
        let s = Shield::new(50.0);
        assert!(s.is_full());
        assert!(!s.is_depleted());
        assert!((s.fraction() - 1.0).abs() < 0.001);
    }

    #[test]
    fn absorb_reduces_shield() {
        let mut s = Shield::new(100.0);
        let pass_through = s.absorb(30.0);
        assert!((s.current - 70.0).abs() < 0.001);
        assert_eq!(pass_through, 0.0);
    }

    #[test]
    fn absorb_passes_overflow_through() {
        let mut s = Shield::new(20.0);
        let pass_through = s.absorb(50.0);
        assert_eq!(s.current, 0.0);
        assert!(s.is_depleted());
        assert!((pass_through - 30.0).abs() < 0.001);
    }

    #[test]
    fn recharge_starts_after_delay() {
        let mut s = Shield::new(100.0).with_recharge(10.0, 1.0);
        s.absorb(50.0); // cooldown = 1.0
        s.tick(0.5); // cooldown = 0.5, no recharge yet
        assert!((s.current - 50.0).abs() < 0.001);
        s.tick(0.5); // cooldown = 0.0
        s.tick(1.0); // recharge: +10 HP
        assert!((s.current - 60.0).abs() < 0.001);
    }

    #[test]
    fn recharge_clamps_to_max() {
        let mut s = Shield::new(100.0).with_recharge(200.0, 0.0);
        s.absorb(10.0);
        s.tick(1.0);
        assert_eq!(s.current, 100.0);
    }

    #[test]
    fn no_recharge_when_rate_zero() {
        let mut s = Shield::new(100.0);
        s.absorb(50.0);
        s.tick(10.0);
        assert!((s.current - 50.0).abs() < 0.001);
    }
}

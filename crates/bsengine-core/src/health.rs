use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        let max = max.max(0.0);
        Self { current: max, max }
    }

    pub fn is_dead(&self) -> bool {
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

    pub fn damage(&mut self, amount: f32) {
        self.current = (self.current - amount.max(0.0)).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount.max(0.0)).min(self.max);
    }

    pub fn set_full(&mut self) {
        self.current = self.max;
    }

    pub fn kill(&mut self) {
        self.current = 0.0;
    }
}

impl Default for Health {
    fn default() -> Self {
        Self::new(100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_starts_full() {
        let h = Health::new(100.0);
        assert!(h.is_full());
        assert!(!h.is_dead());
        assert!((h.fraction() - 1.0).abs() < 0.001);
    }

    #[test]
    fn damage_reduces_health() {
        let mut h = Health::new(100.0);
        h.damage(30.0);
        assert!((h.current - 70.0).abs() < 0.001);
        assert!(!h.is_dead());
    }

    #[test]
    fn damage_clamps_to_zero() {
        let mut h = Health::new(50.0);
        h.damage(200.0);
        assert_eq!(h.current, 0.0);
        assert!(h.is_dead());
    }

    #[test]
    fn heal_clamps_to_max() {
        let mut h = Health::new(100.0);
        h.damage(50.0);
        h.heal(200.0);
        assert_eq!(h.current, 100.0);
        assert!(h.is_full());
    }

    #[test]
    fn negative_damage_is_ignored() {
        let mut h = Health::new(100.0);
        h.damage(-10.0); // should be no-op
        assert!(h.is_full());
    }

    #[test]
    fn fraction_is_correct() {
        let mut h = Health::new(200.0);
        h.damage(50.0);
        assert!((h.fraction() - 0.75).abs() < 0.001);
    }

    #[test]
    fn kill_sets_dead() {
        let mut h = Health::new(100.0);
        h.kill();
        assert!(h.is_dead());
        assert_eq!(h.fraction(), 0.0);
    }
}

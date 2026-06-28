use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wile {
    pub cunning: f32,
    pub max_cunning: f32,
    pub scheme_rate: f32,
    pub just_crafty: bool,
    pub just_naive: bool,
    pub enabled: bool,
}

impl Default for Wile {
    fn default() -> Self {
        Self {
            cunning: 0.0,
            max_cunning: 100.0,
            scheme_rate: 1.0,
            just_crafty: false,
            just_naive: false,
            enabled: true,
        }
    }
}

impl Wile {
    pub fn scheme(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_crafty = false;
        self.just_naive = false;
        let prev = self.cunning;
        self.cunning = (self.cunning + amount).clamp(0.0, self.max_cunning);
        if self.cunning >= self.max_cunning && prev < self.max_cunning {
            self.just_crafty = true;
        }
    }

    pub fn expose(&mut self, amount: f32) {
        if !self.enabled || self.cunning <= 0.0 {
            return;
        }
        self.just_crafty = false;
        self.just_naive = false;
        let prev = self.cunning;
        self.cunning = (self.cunning - amount).max(0.0);
        if self.cunning <= 0.0 && prev > 0.0 {
            self.just_naive = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.cunning >= self.max_cunning {
            return;
        }
        self.scheme(self.scheme_rate * dt);
    }

    pub fn is_crafty(&self) -> bool {
        self.enabled && self.cunning >= self.max_cunning
    }

    pub fn is_naive(&self) -> bool {
        self.cunning <= 0.0
    }

    pub fn cunning_fraction(&self) -> f32 {
        if self.max_cunning <= 0.0 {
            return 0.0;
        }
        self.cunning / self.max_cunning
    }

    pub fn effective_deception(&self, scale: f32) -> f32 {
        self.cunning_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wile() -> Wile {
        Wile {
            cunning: 0.0,
            max_cunning: 100.0,
            scheme_rate: 10.0,
            just_crafty: false,
            just_naive: false,
            enabled: true,
        }
    }

    #[test]
    fn default_cunning_zero() {
        let w = Wile::default();
        assert_eq!(w.cunning, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wile::default().enabled);
    }

    #[test]
    fn scheme_increases_cunning() {
        let mut w = wile();
        w.scheme(30.0);
        assert_eq!(w.cunning, 30.0);
    }

    #[test]
    fn scheme_clamps_at_max() {
        let mut w = wile();
        w.scheme(200.0);
        assert_eq!(w.cunning, 100.0);
    }

    #[test]
    fn scheme_no_op_when_disabled() {
        let mut w = wile();
        w.enabled = false;
        w.scheme(50.0);
        assert_eq!(w.cunning, 0.0);
    }

    #[test]
    fn scheme_sets_just_crafty_at_max() {
        let mut w = wile();
        w.scheme(100.0);
        assert!(w.just_crafty);
    }

    #[test]
    fn scheme_no_just_crafty_if_already_max() {
        let mut w = wile();
        w.cunning = 100.0;
        w.scheme(1.0);
        assert!(!w.just_crafty);
    }

    #[test]
    fn expose_decreases_cunning() {
        let mut w = wile();
        w.cunning = 60.0;
        w.expose(20.0);
        assert_eq!(w.cunning, 40.0);
    }

    #[test]
    fn expose_clamps_at_zero() {
        let mut w = wile();
        w.cunning = 30.0;
        w.expose(200.0);
        assert_eq!(w.cunning, 0.0);
    }

    #[test]
    fn expose_no_op_when_disabled() {
        let mut w = wile();
        w.cunning = 50.0;
        w.enabled = false;
        w.expose(10.0);
        assert_eq!(w.cunning, 50.0);
    }

    #[test]
    fn expose_no_op_when_already_naive() {
        let mut w = wile();
        w.expose(10.0);
        assert_eq!(w.cunning, 0.0);
    }

    #[test]
    fn expose_sets_just_naive_at_zero() {
        let mut w = wile();
        w.cunning = 10.0;
        w.expose(10.0);
        assert!(w.just_naive);
    }

    #[test]
    fn expose_no_just_naive_if_already_zero() {
        let mut w = wile();
        w.expose(1.0);
        assert!(!w.just_naive);
    }

    #[test]
    fn tick_increases_cunning() {
        let mut w = wile();
        w.tick(1.0);
        assert_eq!(w.cunning, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wile();
        w.tick(2.0);
        assert_eq!(w.cunning, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wile();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.cunning, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_crafty() {
        let mut w = wile();
        w.cunning = 100.0;
        w.tick(1.0);
        assert_eq!(w.cunning, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wile();
        w.scheme_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.cunning, 0.0);
    }

    #[test]
    fn is_crafty_true_at_max() {
        let mut w = wile();
        w.cunning = 100.0;
        assert!(w.is_crafty());
    }

    #[test]
    fn is_crafty_false_below_max() {
        let mut w = wile();
        w.cunning = 50.0;
        assert!(!w.is_crafty());
    }

    #[test]
    fn is_crafty_false_when_disabled() {
        let mut w = wile();
        w.cunning = 100.0;
        w.enabled = false;
        assert!(!w.is_crafty());
    }

    #[test]
    fn is_naive_true_at_zero() {
        let w = wile();
        assert!(w.is_naive());
    }

    #[test]
    fn is_naive_false_above_zero() {
        let mut w = wile();
        w.cunning = 1.0;
        assert!(!w.is_naive());
    }

    #[test]
    fn cunning_fraction_zero_when_naive() {
        let w = wile();
        assert_eq!(w.cunning_fraction(), 0.0);
    }

    #[test]
    fn cunning_fraction_one_at_max() {
        let mut w = wile();
        w.cunning = 100.0;
        assert_eq!(w.cunning_fraction(), 1.0);
    }

    #[test]
    fn cunning_fraction_half_at_midpoint() {
        let mut w = wile();
        w.cunning = 50.0;
        assert_eq!(w.cunning_fraction(), 0.5);
    }

    #[test]
    fn cunning_fraction_zero_when_max_zero() {
        let mut w = wile();
        w.max_cunning = 0.0;
        assert_eq!(w.cunning_fraction(), 0.0);
    }

    #[test]
    fn effective_deception_scales() {
        let mut w = wile();
        w.cunning = 50.0;
        assert_eq!(w.effective_deception(2.0), 1.0);
    }

    #[test]
    fn effective_deception_zero_when_naive() {
        let w = wile();
        assert_eq!(w.effective_deception(10.0), 0.0);
    }

    #[test]
    fn just_crafty_cleared_on_next_scheme() {
        let mut w = wile();
        w.scheme(100.0);
        assert!(w.just_crafty);
        w.scheme(1.0);
        assert!(!w.just_crafty);
    }

    #[test]
    fn just_naive_cleared_on_next_expose() {
        let mut w = wile();
        w.cunning = 10.0;
        w.expose(10.0);
        assert!(w.just_naive);
        w.cunning = 10.0;
        w.expose(1.0);
        assert!(!w.just_naive);
    }
}

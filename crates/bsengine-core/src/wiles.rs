use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wiles {
    pub guile: f32,
    pub max_guile: f32,
    pub scheme_rate: f32,
    pub just_cunning: bool,
    pub just_guileless: bool,
    pub enabled: bool,
}

impl Default for Wiles {
    fn default() -> Self {
        Self {
            guile: 0.0,
            max_guile: 100.0,
            scheme_rate: 1.0,
            just_cunning: false,
            just_guileless: false,
            enabled: true,
        }
    }
}

impl Wiles {
    pub fn scheme(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_cunning = false;
        self.just_guileless = false;
        let prev = self.guile;
        self.guile = (self.guile + amount).clamp(0.0, self.max_guile);
        if self.guile >= self.max_guile && prev < self.max_guile {
            self.just_cunning = true;
        }
    }

    pub fn disarm(&mut self, amount: f32) {
        if !self.enabled || self.guile <= 0.0 {
            return;
        }
        self.just_cunning = false;
        self.just_guileless = false;
        let prev = self.guile;
        self.guile = (self.guile - amount).max(0.0);
        if self.guile <= 0.0 && prev > 0.0 {
            self.just_guileless = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.guile >= self.max_guile {
            return;
        }
        self.scheme(self.scheme_rate * dt);
    }

    pub fn is_cunning(&self) -> bool {
        self.enabled && self.guile >= self.max_guile
    }

    pub fn is_guileless(&self) -> bool {
        self.guile <= 0.0
    }

    pub fn guile_fraction(&self) -> f32 {
        if self.max_guile <= 0.0 {
            return 0.0;
        }
        self.guile / self.max_guile
    }

    pub fn effective_trick(&self, scale: f32) -> f32 {
        self.guile_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wiles() -> Wiles {
        Wiles {
            guile: 0.0,
            max_guile: 100.0,
            scheme_rate: 10.0,
            just_cunning: false,
            just_guileless: false,
            enabled: true,
        }
    }

    #[test]
    fn default_guile_zero() {
        let w = Wiles::default();
        assert_eq!(w.guile, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wiles::default().enabled);
    }

    #[test]
    fn scheme_increases_guile() {
        let mut w = wiles();
        w.scheme(30.0);
        assert_eq!(w.guile, 30.0);
    }

    #[test]
    fn scheme_clamps_at_max() {
        let mut w = wiles();
        w.scheme(200.0);
        assert_eq!(w.guile, 100.0);
    }

    #[test]
    fn scheme_no_op_when_disabled() {
        let mut w = wiles();
        w.enabled = false;
        w.scheme(50.0);
        assert_eq!(w.guile, 0.0);
    }

    #[test]
    fn scheme_sets_just_cunning_at_max() {
        let mut w = wiles();
        w.scheme(100.0);
        assert!(w.just_cunning);
    }

    #[test]
    fn scheme_no_just_cunning_if_already_max() {
        let mut w = wiles();
        w.guile = 100.0;
        w.scheme(1.0);
        assert!(!w.just_cunning);
    }

    #[test]
    fn disarm_decreases_guile() {
        let mut w = wiles();
        w.guile = 60.0;
        w.disarm(20.0);
        assert_eq!(w.guile, 40.0);
    }

    #[test]
    fn disarm_clamps_at_zero() {
        let mut w = wiles();
        w.guile = 30.0;
        w.disarm(200.0);
        assert_eq!(w.guile, 0.0);
    }

    #[test]
    fn disarm_no_op_when_disabled() {
        let mut w = wiles();
        w.guile = 50.0;
        w.enabled = false;
        w.disarm(10.0);
        assert_eq!(w.guile, 50.0);
    }

    #[test]
    fn disarm_no_op_when_already_guileless() {
        let mut w = wiles();
        w.disarm(10.0);
        assert_eq!(w.guile, 0.0);
    }

    #[test]
    fn disarm_sets_just_guileless_at_zero() {
        let mut w = wiles();
        w.guile = 10.0;
        w.disarm(10.0);
        assert!(w.just_guileless);
    }

    #[test]
    fn disarm_no_just_guileless_if_already_zero() {
        let mut w = wiles();
        w.disarm(1.0);
        assert!(!w.just_guileless);
    }

    #[test]
    fn tick_increases_guile() {
        let mut w = wiles();
        w.tick(1.0);
        assert_eq!(w.guile, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wiles();
        w.tick(2.0);
        assert_eq!(w.guile, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wiles();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.guile, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_cunning() {
        let mut w = wiles();
        w.guile = 100.0;
        w.tick(1.0);
        assert_eq!(w.guile, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wiles();
        w.scheme_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.guile, 0.0);
    }

    #[test]
    fn is_cunning_true_at_max() {
        let mut w = wiles();
        w.guile = 100.0;
        assert!(w.is_cunning());
    }

    #[test]
    fn is_cunning_false_below_max() {
        let mut w = wiles();
        w.guile = 50.0;
        assert!(!w.is_cunning());
    }

    #[test]
    fn is_cunning_false_when_disabled() {
        let mut w = wiles();
        w.guile = 100.0;
        w.enabled = false;
        assert!(!w.is_cunning());
    }

    #[test]
    fn is_guileless_true_at_zero() {
        let w = wiles();
        assert!(w.is_guileless());
    }

    #[test]
    fn is_guileless_false_above_zero() {
        let mut w = wiles();
        w.guile = 1.0;
        assert!(!w.is_guileless());
    }

    #[test]
    fn guile_fraction_zero_when_guileless() {
        let w = wiles();
        assert_eq!(w.guile_fraction(), 0.0);
    }

    #[test]
    fn guile_fraction_one_at_max() {
        let mut w = wiles();
        w.guile = 100.0;
        assert_eq!(w.guile_fraction(), 1.0);
    }

    #[test]
    fn guile_fraction_half_at_midpoint() {
        let mut w = wiles();
        w.guile = 50.0;
        assert_eq!(w.guile_fraction(), 0.5);
    }

    #[test]
    fn guile_fraction_zero_when_max_zero() {
        let mut w = wiles();
        w.max_guile = 0.0;
        assert_eq!(w.guile_fraction(), 0.0);
    }

    #[test]
    fn effective_trick_scales() {
        let mut w = wiles();
        w.guile = 50.0;
        assert_eq!(w.effective_trick(2.0), 1.0);
    }

    #[test]
    fn effective_trick_zero_when_guileless() {
        let w = wiles();
        assert_eq!(w.effective_trick(10.0), 0.0);
    }

    #[test]
    fn just_cunning_cleared_on_next_scheme() {
        let mut w = wiles();
        w.scheme(100.0);
        assert!(w.just_cunning);
        w.scheme(1.0);
        assert!(!w.just_cunning);
    }

    #[test]
    fn just_guileless_cleared_on_next_disarm() {
        let mut w = wiles();
        w.guile = 10.0;
        w.disarm(10.0);
        assert!(w.just_guileless);
        w.guile = 10.0;
        w.disarm(1.0);
        assert!(!w.just_guileless);
    }
}

use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Woozy {
    pub dizziness: f32,
    pub max_dizziness: f32,
    pub spin_rate: f32,
    pub just_reeling: bool,
    pub just_clear: bool,
    pub enabled: bool,
}

impl Default for Woozy {
    fn default() -> Self {
        Self {
            dizziness: 0.0,
            max_dizziness: 100.0,
            spin_rate: 1.0,
            just_reeling: false,
            just_clear: false,
            enabled: true,
        }
    }
}

impl Woozy {
    pub fn spin(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_reeling = false;
        self.just_clear = false;
        let prev = self.dizziness;
        self.dizziness = (self.dizziness + amount).clamp(0.0, self.max_dizziness);
        if self.dizziness >= self.max_dizziness && prev < self.max_dizziness {
            self.just_reeling = true;
        }
    }

    pub fn recover(&mut self, amount: f32) {
        if !self.enabled || self.dizziness <= 0.0 {
            return;
        }
        self.just_reeling = false;
        self.just_clear = false;
        let prev = self.dizziness;
        self.dizziness = (self.dizziness - amount).max(0.0);
        if self.dizziness <= 0.0 && prev > 0.0 {
            self.just_clear = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.dizziness >= self.max_dizziness {
            return;
        }
        self.spin(self.spin_rate * dt);
    }

    pub fn is_reeling(&self) -> bool {
        self.enabled && self.dizziness >= self.max_dizziness
    }

    pub fn is_clear(&self) -> bool {
        self.dizziness <= 0.0
    }

    pub fn dizziness_fraction(&self) -> f32 {
        if self.max_dizziness <= 0.0 {
            return 0.0;
        }
        self.dizziness / self.max_dizziness
    }

    pub fn effective_vertigo(&self, scale: f32) -> f32 {
        self.dizziness_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn woozy() -> Woozy {
        Woozy {
            dizziness: 0.0,
            max_dizziness: 100.0,
            spin_rate: 10.0,
            just_reeling: false,
            just_clear: false,
            enabled: true,
        }
    }

    #[test]
    fn default_dizziness_zero() {
        let w = Woozy::default();
        assert_eq!(w.dizziness, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Woozy::default().enabled);
    }

    #[test]
    fn spin_increases_dizziness() {
        let mut w = woozy();
        w.spin(30.0);
        assert_eq!(w.dizziness, 30.0);
    }

    #[test]
    fn spin_clamps_at_max() {
        let mut w = woozy();
        w.spin(200.0);
        assert_eq!(w.dizziness, 100.0);
    }

    #[test]
    fn spin_no_op_when_disabled() {
        let mut w = woozy();
        w.enabled = false;
        w.spin(50.0);
        assert_eq!(w.dizziness, 0.0);
    }

    #[test]
    fn spin_sets_just_reeling_at_max() {
        let mut w = woozy();
        w.spin(100.0);
        assert!(w.just_reeling);
    }

    #[test]
    fn spin_no_just_reeling_if_already_max() {
        let mut w = woozy();
        w.dizziness = 100.0;
        w.spin(1.0);
        assert!(!w.just_reeling);
    }

    #[test]
    fn recover_decreases_dizziness() {
        let mut w = woozy();
        w.dizziness = 60.0;
        w.recover(20.0);
        assert_eq!(w.dizziness, 40.0);
    }

    #[test]
    fn recover_clamps_at_zero() {
        let mut w = woozy();
        w.dizziness = 30.0;
        w.recover(200.0);
        assert_eq!(w.dizziness, 0.0);
    }

    #[test]
    fn recover_no_op_when_disabled() {
        let mut w = woozy();
        w.dizziness = 50.0;
        w.enabled = false;
        w.recover(10.0);
        assert_eq!(w.dizziness, 50.0);
    }

    #[test]
    fn recover_no_op_when_already_clear() {
        let mut w = woozy();
        w.recover(10.0);
        assert_eq!(w.dizziness, 0.0);
    }

    #[test]
    fn recover_sets_just_clear_at_zero() {
        let mut w = woozy();
        w.dizziness = 10.0;
        w.recover(10.0);
        assert!(w.just_clear);
    }

    #[test]
    fn recover_no_just_clear_if_already_zero() {
        let mut w = woozy();
        w.recover(1.0);
        assert!(!w.just_clear);
    }

    #[test]
    fn tick_increases_dizziness() {
        let mut w = woozy();
        w.tick(1.0);
        assert_eq!(w.dizziness, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = woozy();
        w.tick(2.0);
        assert_eq!(w.dizziness, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = woozy();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.dizziness, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_reeling() {
        let mut w = woozy();
        w.dizziness = 100.0;
        w.tick(1.0);
        assert_eq!(w.dizziness, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = woozy();
        w.spin_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.dizziness, 0.0);
    }

    #[test]
    fn is_reeling_true_at_max() {
        let mut w = woozy();
        w.dizziness = 100.0;
        assert!(w.is_reeling());
    }

    #[test]
    fn is_reeling_false_below_max() {
        let mut w = woozy();
        w.dizziness = 50.0;
        assert!(!w.is_reeling());
    }

    #[test]
    fn is_reeling_false_when_disabled() {
        let mut w = woozy();
        w.dizziness = 100.0;
        w.enabled = false;
        assert!(!w.is_reeling());
    }

    #[test]
    fn is_clear_true_at_zero() {
        let w = woozy();
        assert!(w.is_clear());
    }

    #[test]
    fn is_clear_false_above_zero() {
        let mut w = woozy();
        w.dizziness = 1.0;
        assert!(!w.is_clear());
    }

    #[test]
    fn dizziness_fraction_zero_when_clear() {
        let w = woozy();
        assert_eq!(w.dizziness_fraction(), 0.0);
    }

    #[test]
    fn dizziness_fraction_one_at_max() {
        let mut w = woozy();
        w.dizziness = 100.0;
        assert_eq!(w.dizziness_fraction(), 1.0);
    }

    #[test]
    fn dizziness_fraction_half_at_midpoint() {
        let mut w = woozy();
        w.dizziness = 50.0;
        assert_eq!(w.dizziness_fraction(), 0.5);
    }

    #[test]
    fn dizziness_fraction_zero_when_max_zero() {
        let mut w = woozy();
        w.max_dizziness = 0.0;
        assert_eq!(w.dizziness_fraction(), 0.0);
    }

    #[test]
    fn effective_vertigo_scales() {
        let mut w = woozy();
        w.dizziness = 50.0;
        assert_eq!(w.effective_vertigo(2.0), 1.0);
    }

    #[test]
    fn effective_vertigo_zero_when_clear() {
        let w = woozy();
        assert_eq!(w.effective_vertigo(10.0), 0.0);
    }

    #[test]
    fn just_reeling_cleared_on_next_spin() {
        let mut w = woozy();
        w.spin(100.0);
        assert!(w.just_reeling);
        w.spin(1.0);
        assert!(!w.just_reeling);
    }

    #[test]
    fn just_clear_cleared_on_next_recover() {
        let mut w = woozy();
        w.dizziness = 10.0;
        w.recover(10.0);
        assert!(w.just_clear);
        w.dizziness = 10.0;
        w.recover(1.0);
        assert!(!w.just_clear);
    }
}

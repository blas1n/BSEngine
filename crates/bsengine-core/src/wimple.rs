use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wimple {
    pub folds: f32,
    pub max_folds: f32,
    pub drape_rate: f32,
    pub just_draped: bool,
    pub just_plain: bool,
    pub enabled: bool,
}

impl Default for Wimple {
    fn default() -> Self {
        Self {
            folds: 0.0,
            max_folds: 100.0,
            drape_rate: 1.0,
            just_draped: false,
            just_plain: false,
            enabled: true,
        }
    }
}

impl Wimple {
    pub fn drape(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_draped = false;
        self.just_plain = false;
        let prev = self.folds;
        self.folds = (self.folds + amount).clamp(0.0, self.max_folds);
        if self.folds >= self.max_folds && prev < self.max_folds {
            self.just_draped = true;
        }
    }

    pub fn unfold(&mut self, amount: f32) {
        if !self.enabled || self.folds <= 0.0 {
            return;
        }
        self.just_draped = false;
        self.just_plain = false;
        let prev = self.folds;
        self.folds = (self.folds - amount).max(0.0);
        if self.folds <= 0.0 && prev > 0.0 {
            self.just_plain = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.folds >= self.max_folds {
            return;
        }
        self.drape(self.drape_rate * dt);
    }

    pub fn is_draped(&self) -> bool {
        self.enabled && self.folds >= self.max_folds
    }

    pub fn is_plain(&self) -> bool {
        self.folds <= 0.0
    }

    pub fn folds_fraction(&self) -> f32 {
        if self.max_folds <= 0.0 {
            return 0.0;
        }
        self.folds / self.max_folds
    }

    pub fn effective_modesty(&self, scale: f32) -> f32 {
        self.folds_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wimple() -> Wimple {
        Wimple {
            folds: 0.0,
            max_folds: 100.0,
            drape_rate: 10.0,
            just_draped: false,
            just_plain: false,
            enabled: true,
        }
    }

    #[test]
    fn default_folds_zero() {
        let w = Wimple::default();
        assert_eq!(w.folds, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wimple::default().enabled);
    }

    #[test]
    fn drape_increases_folds() {
        let mut w = wimple();
        w.drape(30.0);
        assert_eq!(w.folds, 30.0);
    }

    #[test]
    fn drape_clamps_at_max() {
        let mut w = wimple();
        w.drape(200.0);
        assert_eq!(w.folds, 100.0);
    }

    #[test]
    fn drape_no_op_when_disabled() {
        let mut w = wimple();
        w.enabled = false;
        w.drape(50.0);
        assert_eq!(w.folds, 0.0);
    }

    #[test]
    fn drape_sets_just_draped_at_max() {
        let mut w = wimple();
        w.drape(100.0);
        assert!(w.just_draped);
    }

    #[test]
    fn drape_no_just_draped_if_already_max() {
        let mut w = wimple();
        w.folds = 100.0;
        w.drape(1.0);
        assert!(!w.just_draped);
    }

    #[test]
    fn unfold_decreases_folds() {
        let mut w = wimple();
        w.folds = 60.0;
        w.unfold(20.0);
        assert_eq!(w.folds, 40.0);
    }

    #[test]
    fn unfold_clamps_at_zero() {
        let mut w = wimple();
        w.folds = 30.0;
        w.unfold(200.0);
        assert_eq!(w.folds, 0.0);
    }

    #[test]
    fn unfold_no_op_when_disabled() {
        let mut w = wimple();
        w.folds = 50.0;
        w.enabled = false;
        w.unfold(10.0);
        assert_eq!(w.folds, 50.0);
    }

    #[test]
    fn unfold_no_op_when_already_plain() {
        let mut w = wimple();
        w.unfold(10.0);
        assert_eq!(w.folds, 0.0);
    }

    #[test]
    fn unfold_sets_just_plain_at_zero() {
        let mut w = wimple();
        w.folds = 10.0;
        w.unfold(10.0);
        assert!(w.just_plain);
    }

    #[test]
    fn unfold_no_just_plain_if_already_zero() {
        let mut w = wimple();
        w.unfold(1.0);
        assert!(!w.just_plain);
    }

    #[test]
    fn tick_increases_folds() {
        let mut w = wimple();
        w.tick(1.0);
        assert_eq!(w.folds, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wimple();
        w.tick(2.0);
        assert_eq!(w.folds, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wimple();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.folds, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_draped() {
        let mut w = wimple();
        w.folds = 100.0;
        w.tick(1.0);
        assert_eq!(w.folds, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wimple();
        w.drape_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.folds, 0.0);
    }

    #[test]
    fn is_draped_true_at_max() {
        let mut w = wimple();
        w.folds = 100.0;
        assert!(w.is_draped());
    }

    #[test]
    fn is_draped_false_below_max() {
        let mut w = wimple();
        w.folds = 50.0;
        assert!(!w.is_draped());
    }

    #[test]
    fn is_draped_false_when_disabled() {
        let mut w = wimple();
        w.folds = 100.0;
        w.enabled = false;
        assert!(!w.is_draped());
    }

    #[test]
    fn is_plain_true_at_zero() {
        let w = wimple();
        assert!(w.is_plain());
    }

    #[test]
    fn is_plain_false_above_zero() {
        let mut w = wimple();
        w.folds = 1.0;
        assert!(!w.is_plain());
    }

    #[test]
    fn folds_fraction_zero_when_plain() {
        let w = wimple();
        assert_eq!(w.folds_fraction(), 0.0);
    }

    #[test]
    fn folds_fraction_one_at_max() {
        let mut w = wimple();
        w.folds = 100.0;
        assert_eq!(w.folds_fraction(), 1.0);
    }

    #[test]
    fn folds_fraction_half_at_midpoint() {
        let mut w = wimple();
        w.folds = 50.0;
        assert_eq!(w.folds_fraction(), 0.5);
    }

    #[test]
    fn folds_fraction_zero_when_max_zero() {
        let mut w = wimple();
        w.max_folds = 0.0;
        assert_eq!(w.folds_fraction(), 0.0);
    }

    #[test]
    fn effective_modesty_scales() {
        let mut w = wimple();
        w.folds = 50.0;
        assert_eq!(w.effective_modesty(2.0), 1.0);
    }

    #[test]
    fn effective_modesty_zero_when_plain() {
        let w = wimple();
        assert_eq!(w.effective_modesty(10.0), 0.0);
    }

    #[test]
    fn just_draped_cleared_on_next_drape() {
        let mut w = wimple();
        w.drape(100.0);
        assert!(w.just_draped);
        w.drape(1.0);
        assert!(!w.just_draped);
    }

    #[test]
    fn just_plain_cleared_on_next_unfold() {
        let mut w = wimple();
        w.folds = 10.0;
        w.unfold(10.0);
        assert!(w.just_plain);
        w.folds = 10.0;
        w.unfold(1.0);
        assert!(!w.just_plain);
    }
}

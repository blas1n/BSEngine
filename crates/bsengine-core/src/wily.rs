use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wily {
    pub cunning: f32,
    pub max_cunning: f32,
    pub plot_rate: f32,
    pub just_sly: bool,
    pub just_naive: bool,
    pub enabled: bool,
}

impl Default for Wily {
    fn default() -> Self {
        Self {
            cunning: 0.0,
            max_cunning: 100.0,
            plot_rate: 1.0,
            just_sly: false,
            just_naive: false,
            enabled: true,
        }
    }
}

impl Wily {
    pub fn plot(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_sly = false;
        self.just_naive = false;
        let prev = self.cunning;
        self.cunning = (self.cunning + amount).clamp(0.0, self.max_cunning);
        if self.cunning >= self.max_cunning && prev < self.max_cunning {
            self.just_sly = true;
        }
    }

    pub fn naive(&mut self, amount: f32) {
        if !self.enabled || self.cunning <= 0.0 {
            return;
        }
        self.just_sly = false;
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
        self.plot(self.plot_rate * dt);
    }

    pub fn is_sly(&self) -> bool {
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

    pub fn effective_craft(&self, scale: f32) -> f32 {
        self.cunning_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wily() -> Wily {
        Wily {
            cunning: 0.0,
            max_cunning: 100.0,
            plot_rate: 10.0,
            just_sly: false,
            just_naive: false,
            enabled: true,
        }
    }

    #[test]
    fn default_cunning_zero() {
        let w = Wily::default();
        assert_eq!(w.cunning, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wily::default().enabled);
    }

    #[test]
    fn plot_increases_cunning() {
        let mut w = wily();
        w.plot(30.0);
        assert_eq!(w.cunning, 30.0);
    }

    #[test]
    fn plot_clamps_at_max() {
        let mut w = wily();
        w.plot(200.0);
        assert_eq!(w.cunning, 100.0);
    }

    #[test]
    fn plot_no_op_when_disabled() {
        let mut w = wily();
        w.enabled = false;
        w.plot(50.0);
        assert_eq!(w.cunning, 0.0);
    }

    #[test]
    fn plot_sets_just_sly_at_max() {
        let mut w = wily();
        w.plot(100.0);
        assert!(w.just_sly);
    }

    #[test]
    fn plot_no_just_sly_if_already_max() {
        let mut w = wily();
        w.cunning = 100.0;
        w.plot(1.0);
        assert!(!w.just_sly);
    }

    #[test]
    fn naive_decreases_cunning() {
        let mut w = wily();
        w.cunning = 60.0;
        w.naive(20.0);
        assert_eq!(w.cunning, 40.0);
    }

    #[test]
    fn naive_clamps_at_zero() {
        let mut w = wily();
        w.cunning = 30.0;
        w.naive(200.0);
        assert_eq!(w.cunning, 0.0);
    }

    #[test]
    fn naive_no_op_when_disabled() {
        let mut w = wily();
        w.cunning = 50.0;
        w.enabled = false;
        w.naive(10.0);
        assert_eq!(w.cunning, 50.0);
    }

    #[test]
    fn naive_no_op_when_already_naive() {
        let mut w = wily();
        w.naive(10.0);
        assert_eq!(w.cunning, 0.0);
    }

    #[test]
    fn naive_sets_just_naive_at_zero() {
        let mut w = wily();
        w.cunning = 10.0;
        w.naive(10.0);
        assert!(w.just_naive);
    }

    #[test]
    fn naive_no_just_naive_if_already_zero() {
        let mut w = wily();
        w.naive(1.0);
        assert!(!w.just_naive);
    }

    #[test]
    fn tick_increases_cunning() {
        let mut w = wily();
        w.tick(1.0);
        assert_eq!(w.cunning, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wily();
        w.tick(2.0);
        assert_eq!(w.cunning, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wily();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.cunning, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_sly() {
        let mut w = wily();
        w.cunning = 100.0;
        w.tick(1.0);
        assert_eq!(w.cunning, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wily();
        w.plot_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.cunning, 0.0);
    }

    #[test]
    fn is_sly_true_at_max() {
        let mut w = wily();
        w.cunning = 100.0;
        assert!(w.is_sly());
    }

    #[test]
    fn is_sly_false_below_max() {
        let mut w = wily();
        w.cunning = 50.0;
        assert!(!w.is_sly());
    }

    #[test]
    fn is_sly_false_when_disabled() {
        let mut w = wily();
        w.cunning = 100.0;
        w.enabled = false;
        assert!(!w.is_sly());
    }

    #[test]
    fn is_naive_true_at_zero() {
        let w = wily();
        assert!(w.is_naive());
    }

    #[test]
    fn is_naive_false_above_zero() {
        let mut w = wily();
        w.cunning = 1.0;
        assert!(!w.is_naive());
    }

    #[test]
    fn cunning_fraction_zero_when_naive() {
        let w = wily();
        assert_eq!(w.cunning_fraction(), 0.0);
    }

    #[test]
    fn cunning_fraction_one_at_max() {
        let mut w = wily();
        w.cunning = 100.0;
        assert_eq!(w.cunning_fraction(), 1.0);
    }

    #[test]
    fn cunning_fraction_half_at_midpoint() {
        let mut w = wily();
        w.cunning = 50.0;
        assert_eq!(w.cunning_fraction(), 0.5);
    }

    #[test]
    fn cunning_fraction_zero_when_max_zero() {
        let mut w = wily();
        w.max_cunning = 0.0;
        assert_eq!(w.cunning_fraction(), 0.0);
    }

    #[test]
    fn effective_craft_scales() {
        let mut w = wily();
        w.cunning = 50.0;
        assert_eq!(w.effective_craft(2.0), 1.0);
    }

    #[test]
    fn effective_craft_zero_when_naive() {
        let w = wily();
        assert_eq!(w.effective_craft(10.0), 0.0);
    }

    #[test]
    fn just_sly_cleared_on_next_plot() {
        let mut w = wily();
        w.plot(100.0);
        assert!(w.just_sly);
        w.plot(1.0);
        assert!(!w.just_sly);
    }

    #[test]
    fn just_naive_cleared_on_next_naive() {
        let mut w = wily();
        w.cunning = 10.0;
        w.naive(10.0);
        assert!(w.just_naive);
        w.cunning = 10.0;
        w.naive(1.0);
        assert!(!w.just_naive);
    }
}

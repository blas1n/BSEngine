use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wig {
    pub coverage: f32,
    pub max_coverage: f32,
    pub style_rate: f32,
    pub just_coiffed: bool,
    pub just_bare: bool,
    pub enabled: bool,
}

impl Default for Wig {
    fn default() -> Self {
        Self {
            coverage: 0.0,
            max_coverage: 100.0,
            style_rate: 1.0,
            just_coiffed: false,
            just_bare: false,
            enabled: true,
        }
    }
}

impl Wig {
    pub fn style(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_coiffed = false;
        self.just_bare = false;
        let prev = self.coverage;
        self.coverage = (self.coverage + amount).clamp(0.0, self.max_coverage);
        if self.coverage >= self.max_coverage && prev < self.max_coverage {
            self.just_coiffed = true;
        }
    }

    pub fn shed(&mut self, amount: f32) {
        if !self.enabled || self.coverage <= 0.0 {
            return;
        }
        self.just_coiffed = false;
        self.just_bare = false;
        let prev = self.coverage;
        self.coverage = (self.coverage - amount).max(0.0);
        if self.coverage <= 0.0 && prev > 0.0 {
            self.just_bare = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.coverage >= self.max_coverage {
            return;
        }
        self.style(self.style_rate * dt);
    }

    pub fn is_coiffed(&self) -> bool {
        self.enabled && self.coverage >= self.max_coverage
    }

    pub fn is_bare(&self) -> bool {
        self.coverage <= 0.0
    }

    pub fn coverage_fraction(&self) -> f32 {
        if self.max_coverage <= 0.0 {
            return 0.0;
        }
        self.coverage / self.max_coverage
    }

    pub fn effective_disguise(&self, scale: f32) -> f32 {
        self.coverage_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wig() -> Wig {
        Wig {
            coverage: 0.0,
            max_coverage: 100.0,
            style_rate: 10.0,
            just_coiffed: false,
            just_bare: false,
            enabled: true,
        }
    }

    #[test]
    fn default_coverage_zero() {
        let w = Wig::default();
        assert_eq!(w.coverage, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wig::default().enabled);
    }

    #[test]
    fn style_increases_coverage() {
        let mut w = wig();
        w.style(30.0);
        assert_eq!(w.coverage, 30.0);
    }

    #[test]
    fn style_clamps_at_max() {
        let mut w = wig();
        w.style(200.0);
        assert_eq!(w.coverage, 100.0);
    }

    #[test]
    fn style_no_op_when_disabled() {
        let mut w = wig();
        w.enabled = false;
        w.style(50.0);
        assert_eq!(w.coverage, 0.0);
    }

    #[test]
    fn style_sets_just_coiffed_at_max() {
        let mut w = wig();
        w.style(100.0);
        assert!(w.just_coiffed);
    }

    #[test]
    fn style_no_just_coiffed_if_already_max() {
        let mut w = wig();
        w.coverage = 100.0;
        w.style(1.0);
        assert!(!w.just_coiffed);
    }

    #[test]
    fn shed_decreases_coverage() {
        let mut w = wig();
        w.coverage = 60.0;
        w.shed(20.0);
        assert_eq!(w.coverage, 40.0);
    }

    #[test]
    fn shed_clamps_at_zero() {
        let mut w = wig();
        w.coverage = 30.0;
        w.shed(200.0);
        assert_eq!(w.coverage, 0.0);
    }

    #[test]
    fn shed_no_op_when_disabled() {
        let mut w = wig();
        w.coverage = 50.0;
        w.enabled = false;
        w.shed(10.0);
        assert_eq!(w.coverage, 50.0);
    }

    #[test]
    fn shed_no_op_when_already_bare() {
        let mut w = wig();
        w.shed(10.0);
        assert_eq!(w.coverage, 0.0);
    }

    #[test]
    fn shed_sets_just_bare_at_zero() {
        let mut w = wig();
        w.coverage = 10.0;
        w.shed(10.0);
        assert!(w.just_bare);
    }

    #[test]
    fn shed_no_just_bare_if_already_zero() {
        let mut w = wig();
        w.shed(1.0);
        assert!(!w.just_bare);
    }

    #[test]
    fn tick_increases_coverage() {
        let mut w = wig();
        w.tick(1.0);
        assert_eq!(w.coverage, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wig();
        w.tick(2.0);
        assert_eq!(w.coverage, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wig();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.coverage, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_coiffed() {
        let mut w = wig();
        w.coverage = 100.0;
        w.tick(1.0);
        assert_eq!(w.coverage, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wig();
        w.style_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.coverage, 0.0);
    }

    #[test]
    fn is_coiffed_true_at_max() {
        let mut w = wig();
        w.coverage = 100.0;
        assert!(w.is_coiffed());
    }

    #[test]
    fn is_coiffed_false_below_max() {
        let mut w = wig();
        w.coverage = 50.0;
        assert!(!w.is_coiffed());
    }

    #[test]
    fn is_coiffed_false_when_disabled() {
        let mut w = wig();
        w.coverage = 100.0;
        w.enabled = false;
        assert!(!w.is_coiffed());
    }

    #[test]
    fn is_bare_true_at_zero() {
        let w = wig();
        assert!(w.is_bare());
    }

    #[test]
    fn is_bare_false_above_zero() {
        let mut w = wig();
        w.coverage = 1.0;
        assert!(!w.is_bare());
    }

    #[test]
    fn coverage_fraction_zero_when_bare() {
        let w = wig();
        assert_eq!(w.coverage_fraction(), 0.0);
    }

    #[test]
    fn coverage_fraction_one_at_max() {
        let mut w = wig();
        w.coverage = 100.0;
        assert_eq!(w.coverage_fraction(), 1.0);
    }

    #[test]
    fn coverage_fraction_half_at_midpoint() {
        let mut w = wig();
        w.coverage = 50.0;
        assert_eq!(w.coverage_fraction(), 0.5);
    }

    #[test]
    fn coverage_fraction_zero_when_max_zero() {
        let mut w = wig();
        w.max_coverage = 0.0;
        assert_eq!(w.coverage_fraction(), 0.0);
    }

    #[test]
    fn effective_disguise_scales() {
        let mut w = wig();
        w.coverage = 50.0;
        assert_eq!(w.effective_disguise(2.0), 1.0);
    }

    #[test]
    fn effective_disguise_zero_when_bare() {
        let w = wig();
        assert_eq!(w.effective_disguise(10.0), 0.0);
    }

    #[test]
    fn just_coiffed_cleared_on_next_style() {
        let mut w = wig();
        w.style(100.0);
        assert!(w.just_coiffed);
        w.style(1.0);
        assert!(!w.just_coiffed);
    }

    #[test]
    fn just_bare_cleared_on_next_shed() {
        let mut w = wig();
        w.coverage = 10.0;
        w.shed(10.0);
        assert!(w.just_bare);
        w.coverage = 10.0;
        w.shed(1.0);
        assert!(!w.just_bare);
    }
}

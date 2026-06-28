use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Whelp {
    pub youth: f32,
    pub max_youth: f32,
    pub grow_rate: f32,
    pub just_matured: bool,
    pub just_whelped: bool,
    pub enabled: bool,
}

impl Default for Whelp {
    fn default() -> Self {
        Self {
            youth: 0.0,
            max_youth: 100.0,
            grow_rate: 1.0,
            just_matured: false,
            just_whelped: false,
            enabled: true,
        }
    }
}

impl Whelp {
    pub fn grow(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_matured = false;
        self.just_whelped = false;
        let prev = self.youth;
        self.youth = (self.youth + amount).clamp(0.0, self.max_youth);
        if self.youth >= self.max_youth && prev < self.max_youth {
            self.just_matured = true;
        }
    }

    pub fn regress(&mut self, amount: f32) {
        if !self.enabled || self.youth <= 0.0 {
            return;
        }
        self.just_matured = false;
        self.just_whelped = false;
        let prev = self.youth;
        self.youth = (self.youth - amount).max(0.0);
        if self.youth <= 0.0 && prev > 0.0 {
            self.just_whelped = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.youth >= self.max_youth {
            return;
        }
        self.grow(self.grow_rate * dt);
    }

    pub fn is_matured(&self) -> bool {
        self.enabled && self.youth >= self.max_youth
    }

    pub fn is_whelped(&self) -> bool {
        self.youth <= 0.0
    }

    pub fn youth_fraction(&self) -> f32 {
        if self.max_youth <= 0.0 {
            return 0.0;
        }
        self.youth / self.max_youth
    }

    pub fn effective_vigor(&self, scale: f32) -> f32 {
        self.youth_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn whelp() -> Whelp {
        Whelp {
            youth: 0.0,
            max_youth: 100.0,
            grow_rate: 10.0,
            just_matured: false,
            just_whelped: false,
            enabled: true,
        }
    }

    #[test]
    fn default_youth_zero() {
        let w = Whelp::default();
        assert_eq!(w.youth, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Whelp::default().enabled);
    }

    #[test]
    fn grow_increases_youth() {
        let mut w = whelp();
        w.grow(30.0);
        assert_eq!(w.youth, 30.0);
    }

    #[test]
    fn grow_clamps_at_max() {
        let mut w = whelp();
        w.grow(200.0);
        assert_eq!(w.youth, 100.0);
    }

    #[test]
    fn grow_no_op_when_disabled() {
        let mut w = whelp();
        w.enabled = false;
        w.grow(50.0);
        assert_eq!(w.youth, 0.0);
    }

    #[test]
    fn grow_sets_just_matured_at_max() {
        let mut w = whelp();
        w.grow(100.0);
        assert!(w.just_matured);
    }

    #[test]
    fn grow_no_just_matured_if_already_max() {
        let mut w = whelp();
        w.youth = 100.0;
        w.grow(1.0);
        assert!(!w.just_matured);
    }

    #[test]
    fn regress_decreases_youth() {
        let mut w = whelp();
        w.youth = 60.0;
        w.regress(20.0);
        assert_eq!(w.youth, 40.0);
    }

    #[test]
    fn regress_clamps_at_zero() {
        let mut w = whelp();
        w.youth = 30.0;
        w.regress(200.0);
        assert_eq!(w.youth, 0.0);
    }

    #[test]
    fn regress_no_op_when_disabled() {
        let mut w = whelp();
        w.youth = 50.0;
        w.enabled = false;
        w.regress(10.0);
        assert_eq!(w.youth, 50.0);
    }

    #[test]
    fn regress_no_op_when_already_whelped() {
        let mut w = whelp();
        w.regress(10.0);
        assert_eq!(w.youth, 0.0);
    }

    #[test]
    fn regress_sets_just_whelped_at_zero() {
        let mut w = whelp();
        w.youth = 10.0;
        w.regress(10.0);
        assert!(w.just_whelped);
    }

    #[test]
    fn regress_no_just_whelped_if_already_zero() {
        let mut w = whelp();
        w.regress(1.0);
        assert!(!w.just_whelped);
    }

    #[test]
    fn tick_increases_youth() {
        let mut w = whelp();
        w.tick(1.0);
        assert_eq!(w.youth, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = whelp();
        w.tick(2.0);
        assert_eq!(w.youth, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = whelp();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.youth, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_matured() {
        let mut w = whelp();
        w.youth = 100.0;
        w.tick(1.0);
        assert_eq!(w.youth, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = whelp();
        w.grow_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.youth, 0.0);
    }

    #[test]
    fn is_matured_true_at_max() {
        let mut w = whelp();
        w.youth = 100.0;
        assert!(w.is_matured());
    }

    #[test]
    fn is_matured_false_below_max() {
        let mut w = whelp();
        w.youth = 50.0;
        assert!(!w.is_matured());
    }

    #[test]
    fn is_matured_false_when_disabled() {
        let mut w = whelp();
        w.youth = 100.0;
        w.enabled = false;
        assert!(!w.is_matured());
    }

    #[test]
    fn is_whelped_true_at_zero() {
        let w = whelp();
        assert!(w.is_whelped());
    }

    #[test]
    fn is_whelped_false_above_zero() {
        let mut w = whelp();
        w.youth = 1.0;
        assert!(!w.is_whelped());
    }

    #[test]
    fn youth_fraction_zero_when_whelped() {
        let w = whelp();
        assert_eq!(w.youth_fraction(), 0.0);
    }

    #[test]
    fn youth_fraction_one_at_max() {
        let mut w = whelp();
        w.youth = 100.0;
        assert_eq!(w.youth_fraction(), 1.0);
    }

    #[test]
    fn youth_fraction_half_at_midpoint() {
        let mut w = whelp();
        w.youth = 50.0;
        assert_eq!(w.youth_fraction(), 0.5);
    }

    #[test]
    fn youth_fraction_zero_when_max_zero() {
        let mut w = whelp();
        w.max_youth = 0.0;
        assert_eq!(w.youth_fraction(), 0.0);
    }

    #[test]
    fn effective_vigor_scales() {
        let mut w = whelp();
        w.youth = 50.0;
        assert_eq!(w.effective_vigor(2.0), 1.0);
    }

    #[test]
    fn effective_vigor_zero_when_whelped() {
        let w = whelp();
        assert_eq!(w.effective_vigor(10.0), 0.0);
    }

    #[test]
    fn just_matured_cleared_on_next_grow() {
        let mut w = whelp();
        w.grow(100.0);
        assert!(w.just_matured);
        w.grow(1.0);
        assert!(!w.just_matured);
    }

    #[test]
    fn just_whelped_cleared_on_next_regress() {
        let mut w = whelp();
        w.youth = 10.0;
        w.regress(10.0);
        assert!(w.just_whelped);
        w.youth = 10.0;
        w.regress(1.0);
        assert!(!w.just_whelped);
    }
}

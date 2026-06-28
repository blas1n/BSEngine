use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Whiff {
    pub scent: f32,
    pub max_scent: f32,
    pub drift_rate: f32,
    pub just_pungent: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Default for Whiff {
    fn default() -> Self {
        Self {
            scent: 0.0,
            max_scent: 100.0,
            drift_rate: 1.0,
            just_pungent: false,
            just_faded: false,
            enabled: true,
        }
    }
}

impl Whiff {
    pub fn waft(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_pungent = false;
        self.just_faded = false;
        let prev = self.scent;
        self.scent = (self.scent + amount).clamp(0.0, self.max_scent);
        if self.scent >= self.max_scent && prev < self.max_scent {
            self.just_pungent = true;
        }
    }

    pub fn dissipate(&mut self, amount: f32) {
        if !self.enabled || self.scent <= 0.0 {
            return;
        }
        self.just_pungent = false;
        self.just_faded = false;
        let prev = self.scent;
        self.scent = (self.scent - amount).max(0.0);
        if self.scent <= 0.0 && prev > 0.0 {
            self.just_faded = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.scent >= self.max_scent {
            return;
        }
        self.waft(self.drift_rate * dt);
    }

    pub fn is_pungent(&self) -> bool {
        self.enabled && self.scent >= self.max_scent
    }

    pub fn is_faded(&self) -> bool {
        self.scent <= 0.0
    }

    pub fn scent_fraction(&self) -> f32 {
        if self.max_scent <= 0.0 {
            return 0.0;
        }
        self.scent / self.max_scent
    }

    pub fn effective_odor(&self, scale: f32) -> f32 {
        self.scent_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn whiff() -> Whiff {
        Whiff {
            scent: 0.0,
            max_scent: 100.0,
            drift_rate: 10.0,
            just_pungent: false,
            just_faded: false,
            enabled: true,
        }
    }

    #[test]
    fn default_scent_zero() {
        let w = Whiff::default();
        assert_eq!(w.scent, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Whiff::default().enabled);
    }

    #[test]
    fn waft_increases_scent() {
        let mut w = whiff();
        w.waft(30.0);
        assert_eq!(w.scent, 30.0);
    }

    #[test]
    fn waft_clamps_at_max() {
        let mut w = whiff();
        w.waft(200.0);
        assert_eq!(w.scent, 100.0);
    }

    #[test]
    fn waft_no_op_when_disabled() {
        let mut w = whiff();
        w.enabled = false;
        w.waft(50.0);
        assert_eq!(w.scent, 0.0);
    }

    #[test]
    fn waft_sets_just_pungent_at_max() {
        let mut w = whiff();
        w.waft(100.0);
        assert!(w.just_pungent);
    }

    #[test]
    fn waft_no_just_pungent_if_already_max() {
        let mut w = whiff();
        w.scent = 100.0;
        w.waft(1.0);
        assert!(!w.just_pungent);
    }

    #[test]
    fn dissipate_decreases_scent() {
        let mut w = whiff();
        w.scent = 60.0;
        w.dissipate(20.0);
        assert_eq!(w.scent, 40.0);
    }

    #[test]
    fn dissipate_clamps_at_zero() {
        let mut w = whiff();
        w.scent = 30.0;
        w.dissipate(200.0);
        assert_eq!(w.scent, 0.0);
    }

    #[test]
    fn dissipate_no_op_when_disabled() {
        let mut w = whiff();
        w.scent = 50.0;
        w.enabled = false;
        w.dissipate(10.0);
        assert_eq!(w.scent, 50.0);
    }

    #[test]
    fn dissipate_no_op_when_already_faded() {
        let mut w = whiff();
        w.dissipate(10.0);
        assert_eq!(w.scent, 0.0);
    }

    #[test]
    fn dissipate_sets_just_faded_at_zero() {
        let mut w = whiff();
        w.scent = 10.0;
        w.dissipate(10.0);
        assert!(w.just_faded);
    }

    #[test]
    fn dissipate_no_just_faded_if_already_zero() {
        let mut w = whiff();
        w.dissipate(1.0);
        assert!(!w.just_faded);
    }

    #[test]
    fn tick_increases_scent() {
        let mut w = whiff();
        w.tick(1.0);
        assert_eq!(w.scent, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = whiff();
        w.tick(2.0);
        assert_eq!(w.scent, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = whiff();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.scent, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_pungent() {
        let mut w = whiff();
        w.scent = 100.0;
        w.tick(1.0);
        assert_eq!(w.scent, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = whiff();
        w.drift_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.scent, 0.0);
    }

    #[test]
    fn is_pungent_true_at_max() {
        let mut w = whiff();
        w.scent = 100.0;
        assert!(w.is_pungent());
    }

    #[test]
    fn is_pungent_false_below_max() {
        let mut w = whiff();
        w.scent = 50.0;
        assert!(!w.is_pungent());
    }

    #[test]
    fn is_pungent_false_when_disabled() {
        let mut w = whiff();
        w.scent = 100.0;
        w.enabled = false;
        assert!(!w.is_pungent());
    }

    #[test]
    fn is_faded_true_at_zero() {
        let w = whiff();
        assert!(w.is_faded());
    }

    #[test]
    fn is_faded_false_above_zero() {
        let mut w = whiff();
        w.scent = 1.0;
        assert!(!w.is_faded());
    }

    #[test]
    fn scent_fraction_zero_when_faded() {
        let w = whiff();
        assert_eq!(w.scent_fraction(), 0.0);
    }

    #[test]
    fn scent_fraction_one_at_max() {
        let mut w = whiff();
        w.scent = 100.0;
        assert_eq!(w.scent_fraction(), 1.0);
    }

    #[test]
    fn scent_fraction_half_at_midpoint() {
        let mut w = whiff();
        w.scent = 50.0;
        assert_eq!(w.scent_fraction(), 0.5);
    }

    #[test]
    fn scent_fraction_zero_when_max_zero() {
        let mut w = whiff();
        w.max_scent = 0.0;
        assert_eq!(w.scent_fraction(), 0.0);
    }

    #[test]
    fn effective_odor_scales() {
        let mut w = whiff();
        w.scent = 50.0;
        assert_eq!(w.effective_odor(2.0), 1.0);
    }

    #[test]
    fn effective_odor_zero_when_faded() {
        let w = whiff();
        assert_eq!(w.effective_odor(10.0), 0.0);
    }

    #[test]
    fn just_pungent_cleared_on_next_waft() {
        let mut w = whiff();
        w.waft(100.0);
        assert!(w.just_pungent);
        w.waft(1.0);
        assert!(!w.just_pungent);
    }

    #[test]
    fn just_faded_cleared_on_next_dissipate() {
        let mut w = whiff();
        w.scent = 10.0;
        w.dissipate(10.0);
        assert!(w.just_faded);
        w.scent = 10.0;
        w.dissipate(1.0);
        assert!(!w.just_faded);
    }
}

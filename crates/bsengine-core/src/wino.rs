use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wino {
    pub indulgence: f32,
    pub max_indulgence: f32,
    pub tipple_rate: f32,
    pub just_tipsy: bool,
    pub just_sober: bool,
    pub enabled: bool,
}

impl Default for Wino {
    fn default() -> Self {
        Self {
            indulgence: 0.0,
            max_indulgence: 100.0,
            tipple_rate: 1.0,
            just_tipsy: false,
            just_sober: false,
            enabled: true,
        }
    }
}

impl Wino {
    pub fn tipple(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_tipsy = false;
        self.just_sober = false;
        let prev = self.indulgence;
        self.indulgence = (self.indulgence + amount).clamp(0.0, self.max_indulgence);
        if self.indulgence >= self.max_indulgence && prev < self.max_indulgence {
            self.just_tipsy = true;
        }
    }

    pub fn sober(&mut self, amount: f32) {
        if !self.enabled || self.indulgence <= 0.0 {
            return;
        }
        self.just_tipsy = false;
        self.just_sober = false;
        let prev = self.indulgence;
        self.indulgence = (self.indulgence - amount).max(0.0);
        if self.indulgence <= 0.0 && prev > 0.0 {
            self.just_sober = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.indulgence >= self.max_indulgence {
            return;
        }
        self.tipple(self.tipple_rate * dt);
    }

    pub fn is_tipsy(&self) -> bool {
        self.enabled && self.indulgence >= self.max_indulgence
    }

    pub fn is_sober(&self) -> bool {
        self.indulgence <= 0.0
    }

    pub fn indulgence_fraction(&self) -> f32 {
        if self.max_indulgence <= 0.0 {
            return 0.0;
        }
        self.indulgence / self.max_indulgence
    }

    pub fn effective_revelry(&self, scale: f32) -> f32 {
        self.indulgence_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wino() -> Wino {
        Wino {
            indulgence: 0.0,
            max_indulgence: 100.0,
            tipple_rate: 10.0,
            just_tipsy: false,
            just_sober: false,
            enabled: true,
        }
    }

    #[test]
    fn default_indulgence_zero() {
        let w = Wino::default();
        assert_eq!(w.indulgence, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wino::default().enabled);
    }

    #[test]
    fn tipple_increases_indulgence() {
        let mut w = wino();
        w.tipple(30.0);
        assert_eq!(w.indulgence, 30.0);
    }

    #[test]
    fn tipple_clamps_at_max() {
        let mut w = wino();
        w.tipple(200.0);
        assert_eq!(w.indulgence, 100.0);
    }

    #[test]
    fn tipple_no_op_when_disabled() {
        let mut w = wino();
        w.enabled = false;
        w.tipple(50.0);
        assert_eq!(w.indulgence, 0.0);
    }

    #[test]
    fn tipple_sets_just_tipsy_at_max() {
        let mut w = wino();
        w.tipple(100.0);
        assert!(w.just_tipsy);
    }

    #[test]
    fn tipple_no_just_tipsy_if_already_max() {
        let mut w = wino();
        w.indulgence = 100.0;
        w.tipple(1.0);
        assert!(!w.just_tipsy);
    }

    #[test]
    fn sober_decreases_indulgence() {
        let mut w = wino();
        w.indulgence = 60.0;
        w.sober(20.0);
        assert_eq!(w.indulgence, 40.0);
    }

    #[test]
    fn sober_clamps_at_zero() {
        let mut w = wino();
        w.indulgence = 30.0;
        w.sober(200.0);
        assert_eq!(w.indulgence, 0.0);
    }

    #[test]
    fn sober_no_op_when_disabled() {
        let mut w = wino();
        w.indulgence = 50.0;
        w.enabled = false;
        w.sober(10.0);
        assert_eq!(w.indulgence, 50.0);
    }

    #[test]
    fn sober_no_op_when_already_sober() {
        let mut w = wino();
        w.sober(10.0);
        assert_eq!(w.indulgence, 0.0);
    }

    #[test]
    fn sober_sets_just_sober_at_zero() {
        let mut w = wino();
        w.indulgence = 10.0;
        w.sober(10.0);
        assert!(w.just_sober);
    }

    #[test]
    fn sober_no_just_sober_if_already_zero() {
        let mut w = wino();
        w.sober(1.0);
        assert!(!w.just_sober);
    }

    #[test]
    fn tick_increases_indulgence() {
        let mut w = wino();
        w.tick(1.0);
        assert_eq!(w.indulgence, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wino();
        w.tick(2.0);
        assert_eq!(w.indulgence, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wino();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.indulgence, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_tipsy() {
        let mut w = wino();
        w.indulgence = 100.0;
        w.tick(1.0);
        assert_eq!(w.indulgence, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wino();
        w.tipple_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.indulgence, 0.0);
    }

    #[test]
    fn is_tipsy_true_at_max() {
        let mut w = wino();
        w.indulgence = 100.0;
        assert!(w.is_tipsy());
    }

    #[test]
    fn is_tipsy_false_below_max() {
        let mut w = wino();
        w.indulgence = 50.0;
        assert!(!w.is_tipsy());
    }

    #[test]
    fn is_tipsy_false_when_disabled() {
        let mut w = wino();
        w.indulgence = 100.0;
        w.enabled = false;
        assert!(!w.is_tipsy());
    }

    #[test]
    fn is_sober_true_at_zero() {
        let w = wino();
        assert!(w.is_sober());
    }

    #[test]
    fn is_sober_false_above_zero() {
        let mut w = wino();
        w.indulgence = 1.0;
        assert!(!w.is_sober());
    }

    #[test]
    fn indulgence_fraction_zero_when_sober() {
        let w = wino();
        assert_eq!(w.indulgence_fraction(), 0.0);
    }

    #[test]
    fn indulgence_fraction_one_at_max() {
        let mut w = wino();
        w.indulgence = 100.0;
        assert_eq!(w.indulgence_fraction(), 1.0);
    }

    #[test]
    fn indulgence_fraction_half_at_midpoint() {
        let mut w = wino();
        w.indulgence = 50.0;
        assert_eq!(w.indulgence_fraction(), 0.5);
    }

    #[test]
    fn indulgence_fraction_zero_when_max_zero() {
        let mut w = wino();
        w.max_indulgence = 0.0;
        assert_eq!(w.indulgence_fraction(), 0.0);
    }

    #[test]
    fn effective_revelry_scales() {
        let mut w = wino();
        w.indulgence = 50.0;
        assert_eq!(w.effective_revelry(2.0), 1.0);
    }

    #[test]
    fn effective_revelry_zero_when_sober() {
        let w = wino();
        assert_eq!(w.effective_revelry(10.0), 0.0);
    }

    #[test]
    fn just_tipsy_cleared_on_next_tipple() {
        let mut w = wino();
        w.tipple(100.0);
        assert!(w.just_tipsy);
        w.tipple(1.0);
        assert!(!w.just_tipsy);
    }

    #[test]
    fn just_sober_cleared_on_next_sober() {
        let mut w = wino();
        w.indulgence = 10.0;
        w.sober(10.0);
        assert!(w.just_sober);
        w.indulgence = 10.0;
        w.sober(1.0);
        assert!(!w.just_sober);
    }
}

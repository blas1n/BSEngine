use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wafer {
    pub thinness: f32,
    pub max_thinness: f32,
    pub pare_rate: f32,
    pub just_gossamer: bool,
    pub just_thick: bool,
    pub enabled: bool,
}

impl Default for Wafer {
    fn default() -> Self {
        Self {
            thinness: 0.0,
            max_thinness: 100.0,
            pare_rate: 1.0,
            just_gossamer: false,
            just_thick: false,
            enabled: true,
        }
    }
}

impl Wafer {
    pub fn pare(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_gossamer = false;
        self.just_thick = false;
        let prev = self.thinness;
        self.thinness = (self.thinness + amount).clamp(0.0, self.max_thinness);
        if self.thinness >= self.max_thinness && prev < self.max_thinness {
            self.just_gossamer = true;
        }
    }

    pub fn thicken(&mut self, amount: f32) {
        if !self.enabled || self.thinness <= 0.0 {
            return;
        }
        self.just_gossamer = false;
        self.just_thick = false;
        let prev = self.thinness;
        self.thinness = (self.thinness - amount).max(0.0);
        if self.thinness <= 0.0 && prev > 0.0 {
            self.just_thick = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.thinness >= self.max_thinness {
            return;
        }
        self.pare(self.pare_rate * dt);
    }

    pub fn is_gossamer(&self) -> bool {
        self.enabled && self.thinness >= self.max_thinness
    }

    pub fn is_thick(&self) -> bool {
        self.thinness <= 0.0
    }

    pub fn thinness_fraction(&self) -> f32 {
        if self.max_thinness <= 0.0 {
            return 0.0;
        }
        self.thinness / self.max_thinness
    }

    pub fn effective_sliver(&self, scale: f32) -> f32 {
        self.thinness_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wafer() -> Wafer {
        Wafer {
            thinness: 0.0,
            max_thinness: 100.0,
            pare_rate: 10.0,
            just_gossamer: false,
            just_thick: false,
            enabled: true,
        }
    }

    #[test]
    fn default_thinness_zero() {
        let w = Wafer::default();
        assert_eq!(w.thinness, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wafer::default().enabled);
    }

    #[test]
    fn pare_increases_thinness() {
        let mut w = wafer();
        w.pare(30.0);
        assert_eq!(w.thinness, 30.0);
    }

    #[test]
    fn pare_clamps_at_max() {
        let mut w = wafer();
        w.pare(200.0);
        assert_eq!(w.thinness, 100.0);
    }

    #[test]
    fn pare_no_op_when_disabled() {
        let mut w = wafer();
        w.enabled = false;
        w.pare(50.0);
        assert_eq!(w.thinness, 0.0);
    }

    #[test]
    fn pare_sets_just_gossamer_at_max() {
        let mut w = wafer();
        w.pare(100.0);
        assert!(w.just_gossamer);
    }

    #[test]
    fn pare_no_just_gossamer_if_already_max() {
        let mut w = wafer();
        w.thinness = 100.0;
        w.pare(1.0);
        assert!(!w.just_gossamer);
    }

    #[test]
    fn thicken_decreases_thinness() {
        let mut w = wafer();
        w.thinness = 60.0;
        w.thicken(20.0);
        assert_eq!(w.thinness, 40.0);
    }

    #[test]
    fn thicken_clamps_at_zero() {
        let mut w = wafer();
        w.thinness = 30.0;
        w.thicken(200.0);
        assert_eq!(w.thinness, 0.0);
    }

    #[test]
    fn thicken_no_op_when_disabled() {
        let mut w = wafer();
        w.thinness = 50.0;
        w.enabled = false;
        w.thicken(10.0);
        assert_eq!(w.thinness, 50.0);
    }

    #[test]
    fn thicken_no_op_when_already_thick() {
        let mut w = wafer();
        w.thicken(10.0);
        assert_eq!(w.thinness, 0.0);
    }

    #[test]
    fn thicken_sets_just_thick_at_zero() {
        let mut w = wafer();
        w.thinness = 10.0;
        w.thicken(10.0);
        assert!(w.just_thick);
    }

    #[test]
    fn thicken_no_just_thick_if_already_zero() {
        let mut w = wafer();
        w.thicken(1.0);
        assert!(!w.just_thick);
    }

    #[test]
    fn tick_increases_thinness() {
        let mut w = wafer();
        w.tick(1.0);
        assert_eq!(w.thinness, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wafer();
        w.tick(2.0);
        assert_eq!(w.thinness, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wafer();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.thinness, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_gossamer() {
        let mut w = wafer();
        w.thinness = 100.0;
        w.tick(1.0);
        assert_eq!(w.thinness, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wafer();
        w.pare_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.thinness, 0.0);
    }

    #[test]
    fn is_gossamer_true_at_max() {
        let mut w = wafer();
        w.thinness = 100.0;
        assert!(w.is_gossamer());
    }

    #[test]
    fn is_gossamer_false_below_max() {
        let mut w = wafer();
        w.thinness = 50.0;
        assert!(!w.is_gossamer());
    }

    #[test]
    fn is_gossamer_false_when_disabled() {
        let mut w = wafer();
        w.thinness = 100.0;
        w.enabled = false;
        assert!(!w.is_gossamer());
    }

    #[test]
    fn is_thick_true_at_zero() {
        let w = wafer();
        assert!(w.is_thick());
    }

    #[test]
    fn is_thick_false_above_zero() {
        let mut w = wafer();
        w.thinness = 1.0;
        assert!(!w.is_thick());
    }

    #[test]
    fn thinness_fraction_zero_when_thick() {
        let w = wafer();
        assert_eq!(w.thinness_fraction(), 0.0);
    }

    #[test]
    fn thinness_fraction_one_at_max() {
        let mut w = wafer();
        w.thinness = 100.0;
        assert_eq!(w.thinness_fraction(), 1.0);
    }

    #[test]
    fn thinness_fraction_half_at_midpoint() {
        let mut w = wafer();
        w.thinness = 50.0;
        assert_eq!(w.thinness_fraction(), 0.5);
    }

    #[test]
    fn thinness_fraction_zero_when_max_zero() {
        let mut w = wafer();
        w.max_thinness = 0.0;
        assert_eq!(w.thinness_fraction(), 0.0);
    }

    #[test]
    fn effective_sliver_scales() {
        let mut w = wafer();
        w.thinness = 50.0;
        assert_eq!(w.effective_sliver(2.0), 1.0);
    }

    #[test]
    fn effective_sliver_zero_when_thick() {
        let w = wafer();
        assert_eq!(w.effective_sliver(10.0), 0.0);
    }

    #[test]
    fn just_gossamer_cleared_on_next_pare() {
        let mut w = wafer();
        w.pare(100.0);
        assert!(w.just_gossamer);
        w.pare(1.0);
        assert!(!w.just_gossamer);
    }

    #[test]
    fn just_thick_cleared_on_next_thicken() {
        let mut w = wafer();
        w.thinness = 10.0;
        w.thicken(10.0);
        assert!(w.just_thick);
        w.thinness = 10.0;
        w.thicken(1.0);
        assert!(!w.just_thick);
    }
}

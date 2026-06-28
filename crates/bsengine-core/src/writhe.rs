use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Writhe {
    pub contortion: f32,
    pub max_contortion: f32,
    pub squirm_rate: f32,
    pub just_contorted: bool,
    pub just_still: bool,
    pub enabled: bool,
}

impl Default for Writhe {
    fn default() -> Self {
        Self {
            contortion: 0.0,
            max_contortion: 100.0,
            squirm_rate: 1.0,
            just_contorted: false,
            just_still: false,
            enabled: true,
        }
    }
}

impl Writhe {
    pub fn squirm(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_contorted = false;
        self.just_still = false;
        let prev = self.contortion;
        self.contortion = (self.contortion + amount).clamp(0.0, self.max_contortion);
        if self.contortion >= self.max_contortion && prev < self.max_contortion {
            self.just_contorted = true;
        }
    }

    pub fn still(&mut self, amount: f32) {
        if !self.enabled || self.contortion <= 0.0 {
            return;
        }
        self.just_contorted = false;
        self.just_still = false;
        let prev = self.contortion;
        self.contortion = (self.contortion - amount).max(0.0);
        if self.contortion <= 0.0 && prev > 0.0 {
            self.just_still = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.contortion >= self.max_contortion {
            return;
        }
        self.squirm(self.squirm_rate * dt);
    }

    pub fn is_contorted(&self) -> bool {
        self.enabled && self.contortion >= self.max_contortion
    }

    pub fn is_still(&self) -> bool {
        self.contortion <= 0.0
    }

    pub fn contortion_fraction(&self) -> f32 {
        if self.max_contortion <= 0.0 {
            return 0.0;
        }
        self.contortion / self.max_contortion
    }

    pub fn effective_agony(&self, scale: f32) -> f32 {
        self.contortion_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn writhe() -> Writhe {
        Writhe {
            contortion: 0.0,
            max_contortion: 100.0,
            squirm_rate: 10.0,
            just_contorted: false,
            just_still: false,
            enabled: true,
        }
    }

    #[test]
    fn default_contortion_zero() {
        let w = Writhe::default();
        assert_eq!(w.contortion, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Writhe::default().enabled);
    }

    #[test]
    fn squirm_increases_contortion() {
        let mut w = writhe();
        w.squirm(30.0);
        assert_eq!(w.contortion, 30.0);
    }

    #[test]
    fn squirm_clamps_at_max() {
        let mut w = writhe();
        w.squirm(200.0);
        assert_eq!(w.contortion, 100.0);
    }

    #[test]
    fn squirm_no_op_when_disabled() {
        let mut w = writhe();
        w.enabled = false;
        w.squirm(50.0);
        assert_eq!(w.contortion, 0.0);
    }

    #[test]
    fn squirm_sets_just_contorted_at_max() {
        let mut w = writhe();
        w.squirm(100.0);
        assert!(w.just_contorted);
    }

    #[test]
    fn squirm_no_just_contorted_if_already_max() {
        let mut w = writhe();
        w.contortion = 100.0;
        w.squirm(1.0);
        assert!(!w.just_contorted);
    }

    #[test]
    fn still_decreases_contortion() {
        let mut w = writhe();
        w.contortion = 60.0;
        w.still(20.0);
        assert_eq!(w.contortion, 40.0);
    }

    #[test]
    fn still_clamps_at_zero() {
        let mut w = writhe();
        w.contortion = 30.0;
        w.still(200.0);
        assert_eq!(w.contortion, 0.0);
    }

    #[test]
    fn still_no_op_when_disabled() {
        let mut w = writhe();
        w.contortion = 50.0;
        w.enabled = false;
        w.still(10.0);
        assert_eq!(w.contortion, 50.0);
    }

    #[test]
    fn still_no_op_when_already_still() {
        let mut w = writhe();
        w.still(10.0);
        assert_eq!(w.contortion, 0.0);
    }

    #[test]
    fn still_sets_just_still_at_zero() {
        let mut w = writhe();
        w.contortion = 10.0;
        w.still(10.0);
        assert!(w.just_still);
    }

    #[test]
    fn still_no_just_still_if_already_zero() {
        let mut w = writhe();
        w.still(1.0);
        assert!(!w.just_still);
    }

    #[test]
    fn tick_increases_contortion() {
        let mut w = writhe();
        w.tick(1.0);
        assert_eq!(w.contortion, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = writhe();
        w.tick(2.0);
        assert_eq!(w.contortion, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = writhe();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.contortion, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_contorted() {
        let mut w = writhe();
        w.contortion = 100.0;
        w.tick(1.0);
        assert_eq!(w.contortion, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = writhe();
        w.squirm_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.contortion, 0.0);
    }

    #[test]
    fn is_contorted_true_at_max() {
        let mut w = writhe();
        w.contortion = 100.0;
        assert!(w.is_contorted());
    }

    #[test]
    fn is_contorted_false_below_max() {
        let mut w = writhe();
        w.contortion = 50.0;
        assert!(!w.is_contorted());
    }

    #[test]
    fn is_contorted_false_when_disabled() {
        let mut w = writhe();
        w.contortion = 100.0;
        w.enabled = false;
        assert!(!w.is_contorted());
    }

    #[test]
    fn is_still_true_at_zero() {
        let w = writhe();
        assert!(w.is_still());
    }

    #[test]
    fn is_still_false_above_zero() {
        let mut w = writhe();
        w.contortion = 1.0;
        assert!(!w.is_still());
    }

    #[test]
    fn contortion_fraction_zero_when_still() {
        let w = writhe();
        assert_eq!(w.contortion_fraction(), 0.0);
    }

    #[test]
    fn contortion_fraction_one_at_max() {
        let mut w = writhe();
        w.contortion = 100.0;
        assert_eq!(w.contortion_fraction(), 1.0);
    }

    #[test]
    fn contortion_fraction_half_at_midpoint() {
        let mut w = writhe();
        w.contortion = 50.0;
        assert_eq!(w.contortion_fraction(), 0.5);
    }

    #[test]
    fn contortion_fraction_zero_when_max_zero() {
        let mut w = writhe();
        w.max_contortion = 0.0;
        assert_eq!(w.contortion_fraction(), 0.0);
    }

    #[test]
    fn effective_agony_scales() {
        let mut w = writhe();
        w.contortion = 50.0;
        assert_eq!(w.effective_agony(2.0), 1.0);
    }

    #[test]
    fn effective_agony_zero_when_still() {
        let w = writhe();
        assert_eq!(w.effective_agony(10.0), 0.0);
    }

    #[test]
    fn just_contorted_cleared_on_next_squirm() {
        let mut w = writhe();
        w.squirm(100.0);
        assert!(w.just_contorted);
        w.squirm(1.0);
        assert!(!w.just_contorted);
    }

    #[test]
    fn just_still_cleared_on_next_still() {
        let mut w = writhe();
        w.contortion = 10.0;
        w.still(10.0);
        assert!(w.just_still);
        w.contortion = 10.0;
        w.still(1.0);
        assert!(!w.just_still);
    }
}

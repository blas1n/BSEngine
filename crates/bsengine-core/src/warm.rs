use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Warm {
    pub heat: f32,
    pub max_heat: f32,
    pub kindle_rate: f32,
    pub just_warm: bool,
    pub just_cold: bool,
    pub enabled: bool,
}

impl Default for Warm {
    fn default() -> Self {
        Self {
            heat: 0.0,
            max_heat: 100.0,
            kindle_rate: 1.0,
            just_warm: false,
            just_cold: false,
            enabled: true,
        }
    }
}

impl Warm {
    pub fn kindle(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_warm = false;
        self.just_cold = false;
        let prev = self.heat;
        self.heat = (self.heat + amount).clamp(0.0, self.max_heat);
        if self.heat >= self.max_heat && prev < self.max_heat {
            self.just_warm = true;
        }
    }

    pub fn chill(&mut self, amount: f32) {
        if !self.enabled || self.heat <= 0.0 {
            return;
        }
        self.just_warm = false;
        self.just_cold = false;
        let prev = self.heat;
        self.heat = (self.heat - amount).max(0.0);
        if self.heat <= 0.0 && prev > 0.0 {
            self.just_cold = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.heat >= self.max_heat {
            return;
        }
        self.kindle(self.kindle_rate * dt);
    }

    pub fn is_warm(&self) -> bool {
        self.enabled && self.heat >= self.max_heat
    }

    pub fn is_cold(&self) -> bool {
        self.heat <= 0.0
    }

    pub fn heat_fraction(&self) -> f32 {
        if self.max_heat <= 0.0 {
            return 0.0;
        }
        self.heat / self.max_heat
    }

    pub fn effective_warmth(&self, scale: f32) -> f32 {
        self.heat_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warm() -> Warm {
        Warm {
            heat: 0.0,
            max_heat: 100.0,
            kindle_rate: 10.0,
            just_warm: false,
            just_cold: false,
            enabled: true,
        }
    }

    #[test]
    fn default_heat_zero() {
        let w = Warm::default();
        assert_eq!(w.heat, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Warm::default().enabled);
    }

    #[test]
    fn kindle_increases_heat() {
        let mut w = warm();
        w.kindle(30.0);
        assert_eq!(w.heat, 30.0);
    }

    #[test]
    fn kindle_clamps_at_max() {
        let mut w = warm();
        w.kindle(200.0);
        assert_eq!(w.heat, 100.0);
    }

    #[test]
    fn kindle_no_op_when_disabled() {
        let mut w = warm();
        w.enabled = false;
        w.kindle(50.0);
        assert_eq!(w.heat, 0.0);
    }

    #[test]
    fn kindle_sets_just_warm_at_max() {
        let mut w = warm();
        w.kindle(100.0);
        assert!(w.just_warm);
    }

    #[test]
    fn kindle_no_just_warm_if_already_max() {
        let mut w = warm();
        w.heat = 100.0;
        w.kindle(1.0);
        assert!(!w.just_warm);
    }

    #[test]
    fn chill_decreases_heat() {
        let mut w = warm();
        w.heat = 60.0;
        w.chill(20.0);
        assert_eq!(w.heat, 40.0);
    }

    #[test]
    fn chill_clamps_at_zero() {
        let mut w = warm();
        w.heat = 30.0;
        w.chill(200.0);
        assert_eq!(w.heat, 0.0);
    }

    #[test]
    fn chill_no_op_when_disabled() {
        let mut w = warm();
        w.heat = 50.0;
        w.enabled = false;
        w.chill(10.0);
        assert_eq!(w.heat, 50.0);
    }

    #[test]
    fn chill_no_op_when_already_cold() {
        let mut w = warm();
        w.chill(10.0);
        assert_eq!(w.heat, 0.0);
    }

    #[test]
    fn chill_sets_just_cold_at_zero() {
        let mut w = warm();
        w.heat = 10.0;
        w.chill(10.0);
        assert!(w.just_cold);
    }

    #[test]
    fn chill_no_just_cold_if_already_zero() {
        let mut w = warm();
        w.chill(1.0);
        assert!(!w.just_cold);
    }

    #[test]
    fn tick_increases_heat() {
        let mut w = warm();
        w.tick(1.0);
        assert_eq!(w.heat, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = warm();
        w.tick(2.0);
        assert_eq!(w.heat, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = warm();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.heat, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_warm() {
        let mut w = warm();
        w.heat = 100.0;
        w.tick(1.0);
        assert_eq!(w.heat, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = warm();
        w.kindle_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.heat, 0.0);
    }

    #[test]
    fn is_warm_true_at_max() {
        let mut w = warm();
        w.heat = 100.0;
        assert!(w.is_warm());
    }

    #[test]
    fn is_warm_false_below_max() {
        let mut w = warm();
        w.heat = 50.0;
        assert!(!w.is_warm());
    }

    #[test]
    fn is_warm_false_when_disabled() {
        let mut w = warm();
        w.heat = 100.0;
        w.enabled = false;
        assert!(!w.is_warm());
    }

    #[test]
    fn is_cold_true_at_zero() {
        let w = warm();
        assert!(w.is_cold());
    }

    #[test]
    fn is_cold_false_above_zero() {
        let mut w = warm();
        w.heat = 1.0;
        assert!(!w.is_cold());
    }

    #[test]
    fn heat_fraction_zero_when_cold() {
        let w = warm();
        assert_eq!(w.heat_fraction(), 0.0);
    }

    #[test]
    fn heat_fraction_one_at_max() {
        let mut w = warm();
        w.heat = 100.0;
        assert_eq!(w.heat_fraction(), 1.0);
    }

    #[test]
    fn heat_fraction_half_at_midpoint() {
        let mut w = warm();
        w.heat = 50.0;
        assert_eq!(w.heat_fraction(), 0.5);
    }

    #[test]
    fn heat_fraction_zero_when_max_zero() {
        let mut w = warm();
        w.max_heat = 0.0;
        assert_eq!(w.heat_fraction(), 0.0);
    }

    #[test]
    fn effective_warmth_scales() {
        let mut w = warm();
        w.heat = 50.0;
        assert_eq!(w.effective_warmth(2.0), 1.0);
    }

    #[test]
    fn effective_warmth_zero_when_cold() {
        let w = warm();
        assert_eq!(w.effective_warmth(10.0), 0.0);
    }

    #[test]
    fn just_warm_cleared_on_next_kindle() {
        let mut w = warm();
        w.kindle(100.0);
        assert!(w.just_warm);
        w.kindle(1.0);
        assert!(!w.just_warm);
    }

    #[test]
    fn just_cold_cleared_on_next_chill() {
        let mut w = warm();
        w.heat = 10.0;
        w.chill(10.0);
        assert!(w.just_cold);
        w.heat = 10.0;
        w.chill(1.0);
        assert!(!w.just_cold);
    }
}

use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wool {
    pub warmth: f32,
    pub max_warmth: f32,
    pub insulate_rate: f32,
    pub just_warm: bool,
    pub just_cold: bool,
    pub enabled: bool,
}

impl Default for Wool {
    fn default() -> Self {
        Self {
            warmth: 0.0,
            max_warmth: 100.0,
            insulate_rate: 1.0,
            just_warm: false,
            just_cold: false,
            enabled: true,
        }
    }
}

impl Wool {
    pub fn insulate(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_warm = false;
        self.just_cold = false;
        let prev = self.warmth;
        self.warmth = (self.warmth + amount).clamp(0.0, self.max_warmth);
        if self.warmth >= self.max_warmth && prev < self.max_warmth {
            self.just_warm = true;
        }
    }

    pub fn chill(&mut self, amount: f32) {
        if !self.enabled || self.warmth <= 0.0 {
            return;
        }
        self.just_warm = false;
        self.just_cold = false;
        let prev = self.warmth;
        self.warmth = (self.warmth - amount).max(0.0);
        if self.warmth <= 0.0 && prev > 0.0 {
            self.just_cold = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.warmth >= self.max_warmth {
            return;
        }
        self.insulate(self.insulate_rate * dt);
    }

    pub fn is_warm(&self) -> bool {
        self.enabled && self.warmth >= self.max_warmth
    }

    pub fn is_cold(&self) -> bool {
        self.warmth <= 0.0
    }

    pub fn warmth_fraction(&self) -> f32 {
        if self.max_warmth <= 0.0 {
            return 0.0;
        }
        self.warmth / self.max_warmth
    }

    pub fn effective_insulation(&self, scale: f32) -> f32 {
        self.warmth_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wool() -> Wool {
        Wool {
            warmth: 0.0,
            max_warmth: 100.0,
            insulate_rate: 10.0,
            just_warm: false,
            just_cold: false,
            enabled: true,
        }
    }

    #[test]
    fn default_warmth_zero() {
        let w = Wool::default();
        assert_eq!(w.warmth, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wool::default().enabled);
    }

    #[test]
    fn insulate_increases_warmth() {
        let mut w = wool();
        w.insulate(30.0);
        assert_eq!(w.warmth, 30.0);
    }

    #[test]
    fn insulate_clamps_at_max() {
        let mut w = wool();
        w.insulate(200.0);
        assert_eq!(w.warmth, 100.0);
    }

    #[test]
    fn insulate_no_op_when_disabled() {
        let mut w = wool();
        w.enabled = false;
        w.insulate(50.0);
        assert_eq!(w.warmth, 0.0);
    }

    #[test]
    fn insulate_sets_just_warm_at_max() {
        let mut w = wool();
        w.insulate(100.0);
        assert!(w.just_warm);
    }

    #[test]
    fn insulate_no_just_warm_if_already_max() {
        let mut w = wool();
        w.warmth = 100.0;
        w.insulate(1.0);
        assert!(!w.just_warm);
    }

    #[test]
    fn chill_decreases_warmth() {
        let mut w = wool();
        w.warmth = 60.0;
        w.chill(20.0);
        assert_eq!(w.warmth, 40.0);
    }

    #[test]
    fn chill_clamps_at_zero() {
        let mut w = wool();
        w.warmth = 30.0;
        w.chill(200.0);
        assert_eq!(w.warmth, 0.0);
    }

    #[test]
    fn chill_no_op_when_disabled() {
        let mut w = wool();
        w.warmth = 50.0;
        w.enabled = false;
        w.chill(10.0);
        assert_eq!(w.warmth, 50.0);
    }

    #[test]
    fn chill_no_op_when_already_cold() {
        let mut w = wool();
        w.chill(10.0);
        assert_eq!(w.warmth, 0.0);
    }

    #[test]
    fn chill_sets_just_cold_at_zero() {
        let mut w = wool();
        w.warmth = 10.0;
        w.chill(10.0);
        assert!(w.just_cold);
    }

    #[test]
    fn chill_no_just_cold_if_already_cold() {
        let mut w = wool();
        w.chill(1.0);
        assert!(!w.just_cold);
    }

    #[test]
    fn tick_increases_warmth() {
        let mut w = wool();
        w.tick(1.0);
        assert_eq!(w.warmth, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wool();
        w.tick(2.0);
        assert_eq!(w.warmth, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wool();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.warmth, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_warm() {
        let mut w = wool();
        w.warmth = 100.0;
        w.tick(1.0);
        assert_eq!(w.warmth, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wool();
        w.insulate_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.warmth, 0.0);
    }

    #[test]
    fn is_warm_true_at_max() {
        let mut w = wool();
        w.warmth = 100.0;
        assert!(w.is_warm());
    }

    #[test]
    fn is_warm_false_below_max() {
        let mut w = wool();
        w.warmth = 50.0;
        assert!(!w.is_warm());
    }

    #[test]
    fn is_warm_false_when_disabled() {
        let mut w = wool();
        w.warmth = 100.0;
        w.enabled = false;
        assert!(!w.is_warm());
    }

    #[test]
    fn is_cold_true_at_zero() {
        let w = wool();
        assert!(w.is_cold());
    }

    #[test]
    fn is_cold_false_above_zero() {
        let mut w = wool();
        w.warmth = 1.0;
        assert!(!w.is_cold());
    }

    #[test]
    fn warmth_fraction_zero_when_cold() {
        let w = wool();
        assert_eq!(w.warmth_fraction(), 0.0);
    }

    #[test]
    fn warmth_fraction_one_at_max() {
        let mut w = wool();
        w.warmth = 100.0;
        assert_eq!(w.warmth_fraction(), 1.0);
    }

    #[test]
    fn warmth_fraction_half_at_midpoint() {
        let mut w = wool();
        w.warmth = 50.0;
        assert_eq!(w.warmth_fraction(), 0.5);
    }

    #[test]
    fn warmth_fraction_zero_when_max_zero() {
        let mut w = wool();
        w.max_warmth = 0.0;
        assert_eq!(w.warmth_fraction(), 0.0);
    }

    #[test]
    fn effective_insulation_scales() {
        let mut w = wool();
        w.warmth = 50.0;
        assert_eq!(w.effective_insulation(2.0), 1.0);
    }

    #[test]
    fn effective_insulation_zero_when_cold() {
        let w = wool();
        assert_eq!(w.effective_insulation(10.0), 0.0);
    }

    #[test]
    fn just_warm_cleared_on_next_insulate() {
        let mut w = wool();
        w.insulate(100.0);
        assert!(w.just_warm);
        w.insulate(1.0);
        assert!(!w.just_warm);
    }

    #[test]
    fn just_cold_cleared_on_next_chill() {
        let mut w = wool();
        w.warmth = 10.0;
        w.chill(10.0);
        assert!(w.just_cold);
        w.warmth = 10.0;
        w.chill(1.0);
        assert!(!w.just_cold);
    }
}

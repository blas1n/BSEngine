use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weigh {
    pub burden: f32,
    pub max_burden: f32,
    pub load_rate: f32,
    pub just_laden: bool,
    pub just_light: bool,
    pub enabled: bool,
}

impl Default for Weigh {
    fn default() -> Self {
        Self {
            burden: 0.0,
            max_burden: 100.0,
            load_rate: 1.0,
            just_laden: false,
            just_light: false,
            enabled: true,
        }
    }
}

impl Weigh {
    pub fn load(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_laden = false;
        self.just_light = false;
        let prev = self.burden;
        self.burden = (self.burden + amount).clamp(0.0, self.max_burden);
        if self.burden >= self.max_burden && prev < self.max_burden {
            self.just_laden = true;
        }
    }

    pub fn lighten(&mut self, amount: f32) {
        if !self.enabled || self.burden <= 0.0 {
            return;
        }
        self.just_laden = false;
        self.just_light = false;
        let prev = self.burden;
        self.burden = (self.burden - amount).max(0.0);
        if self.burden <= 0.0 && prev > 0.0 {
            self.just_light = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.burden >= self.max_burden {
            return;
        }
        self.load(self.load_rate * dt);
    }

    pub fn is_laden(&self) -> bool {
        self.enabled && self.burden >= self.max_burden
    }

    pub fn is_light(&self) -> bool {
        self.burden <= 0.0
    }

    pub fn burden_fraction(&self) -> f32 {
        if self.max_burden <= 0.0 {
            return 0.0;
        }
        self.burden / self.max_burden
    }

    pub fn effective_gravity(&self, scale: f32) -> f32 {
        self.burden_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weigh() -> Weigh {
        Weigh {
            burden: 0.0,
            max_burden: 100.0,
            load_rate: 10.0,
            just_laden: false,
            just_light: false,
            enabled: true,
        }
    }

    #[test]
    fn default_burden_zero() {
        let w = Weigh::default();
        assert_eq!(w.burden, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Weigh::default().enabled);
    }

    #[test]
    fn load_increases_burden() {
        let mut w = weigh();
        w.load(30.0);
        assert_eq!(w.burden, 30.0);
    }

    #[test]
    fn load_clamps_at_max() {
        let mut w = weigh();
        w.load(200.0);
        assert_eq!(w.burden, 100.0);
    }

    #[test]
    fn load_no_op_when_disabled() {
        let mut w = weigh();
        w.enabled = false;
        w.load(50.0);
        assert_eq!(w.burden, 0.0);
    }

    #[test]
    fn load_sets_just_laden_at_max() {
        let mut w = weigh();
        w.load(100.0);
        assert!(w.just_laden);
    }

    #[test]
    fn load_no_just_laden_if_already_max() {
        let mut w = weigh();
        w.burden = 100.0;
        w.load(1.0);
        assert!(!w.just_laden);
    }

    #[test]
    fn lighten_decreases_burden() {
        let mut w = weigh();
        w.burden = 60.0;
        w.lighten(20.0);
        assert_eq!(w.burden, 40.0);
    }

    #[test]
    fn lighten_clamps_at_zero() {
        let mut w = weigh();
        w.burden = 30.0;
        w.lighten(200.0);
        assert_eq!(w.burden, 0.0);
    }

    #[test]
    fn lighten_no_op_when_disabled() {
        let mut w = weigh();
        w.burden = 50.0;
        w.enabled = false;
        w.lighten(10.0);
        assert_eq!(w.burden, 50.0);
    }

    #[test]
    fn lighten_no_op_when_already_light() {
        let mut w = weigh();
        w.lighten(10.0);
        assert_eq!(w.burden, 0.0);
    }

    #[test]
    fn lighten_sets_just_light_at_zero() {
        let mut w = weigh();
        w.burden = 10.0;
        w.lighten(10.0);
        assert!(w.just_light);
    }

    #[test]
    fn lighten_no_just_light_if_already_zero() {
        let mut w = weigh();
        w.lighten(1.0);
        assert!(!w.just_light);
    }

    #[test]
    fn tick_increases_burden() {
        let mut w = weigh();
        w.tick(1.0);
        assert_eq!(w.burden, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = weigh();
        w.tick(2.0);
        assert_eq!(w.burden, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = weigh();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.burden, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_laden() {
        let mut w = weigh();
        w.burden = 100.0;
        w.tick(1.0);
        assert_eq!(w.burden, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = weigh();
        w.load_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.burden, 0.0);
    }

    #[test]
    fn is_laden_true_at_max() {
        let mut w = weigh();
        w.burden = 100.0;
        assert!(w.is_laden());
    }

    #[test]
    fn is_laden_false_below_max() {
        let mut w = weigh();
        w.burden = 50.0;
        assert!(!w.is_laden());
    }

    #[test]
    fn is_laden_false_when_disabled() {
        let mut w = weigh();
        w.burden = 100.0;
        w.enabled = false;
        assert!(!w.is_laden());
    }

    #[test]
    fn is_light_true_at_zero() {
        let w = weigh();
        assert!(w.is_light());
    }

    #[test]
    fn is_light_false_above_zero() {
        let mut w = weigh();
        w.burden = 1.0;
        assert!(!w.is_light());
    }

    #[test]
    fn burden_fraction_zero_when_light() {
        let w = weigh();
        assert_eq!(w.burden_fraction(), 0.0);
    }

    #[test]
    fn burden_fraction_one_at_max() {
        let mut w = weigh();
        w.burden = 100.0;
        assert_eq!(w.burden_fraction(), 1.0);
    }

    #[test]
    fn burden_fraction_half_at_midpoint() {
        let mut w = weigh();
        w.burden = 50.0;
        assert_eq!(w.burden_fraction(), 0.5);
    }

    #[test]
    fn burden_fraction_zero_when_max_zero() {
        let mut w = weigh();
        w.max_burden = 0.0;
        assert_eq!(w.burden_fraction(), 0.0);
    }

    #[test]
    fn effective_gravity_scales() {
        let mut w = weigh();
        w.burden = 50.0;
        assert_eq!(w.effective_gravity(2.0), 1.0);
    }

    #[test]
    fn effective_gravity_zero_when_light() {
        let w = weigh();
        assert_eq!(w.effective_gravity(10.0), 0.0);
    }

    #[test]
    fn just_laden_cleared_on_next_load() {
        let mut w = weigh();
        w.load(100.0);
        assert!(w.just_laden);
        w.load(1.0);
        assert!(!w.just_laden);
    }

    #[test]
    fn just_light_cleared_on_next_lighten() {
        let mut w = weigh();
        w.burden = 10.0;
        w.lighten(10.0);
        assert!(w.just_light);
        w.burden = 10.0;
        w.lighten(1.0);
        assert!(!w.just_light);
    }
}

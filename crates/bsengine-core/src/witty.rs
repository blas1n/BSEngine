use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Witty {
    pub wit: f32,
    pub max_wit: f32,
    pub quip_rate: f32,
    pub just_sharp: bool,
    pub just_dull: bool,
    pub enabled: bool,
}

impl Default for Witty {
    fn default() -> Self {
        Self {
            wit: 0.0,
            max_wit: 100.0,
            quip_rate: 1.0,
            just_sharp: false,
            just_dull: false,
            enabled: true,
        }
    }
}

impl Witty {
    pub fn quip(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_sharp = false;
        self.just_dull = false;
        let prev = self.wit;
        self.wit = (self.wit + amount).clamp(0.0, self.max_wit);
        if self.wit >= self.max_wit && prev < self.max_wit {
            self.just_sharp = true;
        }
    }

    pub fn dull(&mut self, amount: f32) {
        if !self.enabled || self.wit <= 0.0 {
            return;
        }
        self.just_sharp = false;
        self.just_dull = false;
        let prev = self.wit;
        self.wit = (self.wit - amount).max(0.0);
        if self.wit <= 0.0 && prev > 0.0 {
            self.just_dull = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.wit >= self.max_wit {
            return;
        }
        self.quip(self.quip_rate * dt);
    }

    pub fn is_sharp(&self) -> bool {
        self.enabled && self.wit >= self.max_wit
    }

    pub fn is_dull(&self) -> bool {
        self.wit <= 0.0
    }

    pub fn wit_fraction(&self) -> f32 {
        if self.max_wit <= 0.0 {
            return 0.0;
        }
        self.wit / self.max_wit
    }

    pub fn effective_banter(&self, scale: f32) -> f32 {
        self.wit_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn witty() -> Witty {
        Witty {
            wit: 0.0,
            max_wit: 100.0,
            quip_rate: 10.0,
            just_sharp: false,
            just_dull: false,
            enabled: true,
        }
    }

    #[test]
    fn default_wit_zero() {
        let w = Witty::default();
        assert_eq!(w.wit, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Witty::default().enabled);
    }

    #[test]
    fn quip_increases_wit() {
        let mut w = witty();
        w.quip(30.0);
        assert_eq!(w.wit, 30.0);
    }

    #[test]
    fn quip_clamps_at_max() {
        let mut w = witty();
        w.quip(200.0);
        assert_eq!(w.wit, 100.0);
    }

    #[test]
    fn quip_no_op_when_disabled() {
        let mut w = witty();
        w.enabled = false;
        w.quip(50.0);
        assert_eq!(w.wit, 0.0);
    }

    #[test]
    fn quip_sets_just_sharp_at_max() {
        let mut w = witty();
        w.quip(100.0);
        assert!(w.just_sharp);
    }

    #[test]
    fn quip_no_just_sharp_if_already_max() {
        let mut w = witty();
        w.wit = 100.0;
        w.quip(1.0);
        assert!(!w.just_sharp);
    }

    #[test]
    fn dull_decreases_wit() {
        let mut w = witty();
        w.wit = 60.0;
        w.dull(20.0);
        assert_eq!(w.wit, 40.0);
    }

    #[test]
    fn dull_clamps_at_zero() {
        let mut w = witty();
        w.wit = 30.0;
        w.dull(200.0);
        assert_eq!(w.wit, 0.0);
    }

    #[test]
    fn dull_no_op_when_disabled() {
        let mut w = witty();
        w.wit = 50.0;
        w.enabled = false;
        w.dull(10.0);
        assert_eq!(w.wit, 50.0);
    }

    #[test]
    fn dull_no_op_when_already_dull() {
        let mut w = witty();
        w.dull(10.0);
        assert_eq!(w.wit, 0.0);
    }

    #[test]
    fn dull_sets_just_dull_at_zero() {
        let mut w = witty();
        w.wit = 10.0;
        w.dull(10.0);
        assert!(w.just_dull);
    }

    #[test]
    fn dull_no_just_dull_if_already_zero() {
        let mut w = witty();
        w.dull(1.0);
        assert!(!w.just_dull);
    }

    #[test]
    fn tick_increases_wit() {
        let mut w = witty();
        w.tick(1.0);
        assert_eq!(w.wit, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = witty();
        w.tick(2.0);
        assert_eq!(w.wit, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = witty();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.wit, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_sharp() {
        let mut w = witty();
        w.wit = 100.0;
        w.tick(1.0);
        assert_eq!(w.wit, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = witty();
        w.quip_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.wit, 0.0);
    }

    #[test]
    fn is_sharp_true_at_max() {
        let mut w = witty();
        w.wit = 100.0;
        assert!(w.is_sharp());
    }

    #[test]
    fn is_sharp_false_below_max() {
        let mut w = witty();
        w.wit = 50.0;
        assert!(!w.is_sharp());
    }

    #[test]
    fn is_sharp_false_when_disabled() {
        let mut w = witty();
        w.wit = 100.0;
        w.enabled = false;
        assert!(!w.is_sharp());
    }

    #[test]
    fn is_dull_true_at_zero() {
        let w = witty();
        assert!(w.is_dull());
    }

    #[test]
    fn is_dull_false_above_zero() {
        let mut w = witty();
        w.wit = 1.0;
        assert!(!w.is_dull());
    }

    #[test]
    fn wit_fraction_zero_when_dull() {
        let w = witty();
        assert_eq!(w.wit_fraction(), 0.0);
    }

    #[test]
    fn wit_fraction_one_at_max() {
        let mut w = witty();
        w.wit = 100.0;
        assert_eq!(w.wit_fraction(), 1.0);
    }

    #[test]
    fn wit_fraction_half_at_midpoint() {
        let mut w = witty();
        w.wit = 50.0;
        assert_eq!(w.wit_fraction(), 0.5);
    }

    #[test]
    fn wit_fraction_zero_when_max_zero() {
        let mut w = witty();
        w.max_wit = 0.0;
        assert_eq!(w.wit_fraction(), 0.0);
    }

    #[test]
    fn effective_banter_scales() {
        let mut w = witty();
        w.wit = 50.0;
        assert_eq!(w.effective_banter(2.0), 1.0);
    }

    #[test]
    fn effective_banter_zero_when_dull() {
        let w = witty();
        assert_eq!(w.effective_banter(10.0), 0.0);
    }

    #[test]
    fn just_sharp_cleared_on_next_quip() {
        let mut w = witty();
        w.quip(100.0);
        assert!(w.just_sharp);
        w.quip(1.0);
        assert!(!w.just_sharp);
    }

    #[test]
    fn just_dull_cleared_on_next_dull() {
        let mut w = witty();
        w.wit = 10.0;
        w.dull(10.0);
        assert!(w.just_dull);
        w.wit = 10.0;
        w.dull(1.0);
        assert!(!w.just_dull);
    }
}

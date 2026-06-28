use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wangle {
    pub scheme: f32,
    pub max_scheme: f32,
    pub maneuver_rate: f32,
    pub just_schemed: bool,
    pub just_foiled: bool,
    pub enabled: bool,
}

impl Default for Wangle {
    fn default() -> Self {
        Self {
            scheme: 0.0,
            max_scheme: 100.0,
            maneuver_rate: 1.0,
            just_schemed: false,
            just_foiled: false,
            enabled: true,
        }
    }
}

impl Wangle {
    pub fn maneuver(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_schemed = false;
        self.just_foiled = false;
        let prev = self.scheme;
        self.scheme = (self.scheme + amount).clamp(0.0, self.max_scheme);
        if self.scheme >= self.max_scheme && prev < self.max_scheme {
            self.just_schemed = true;
        }
    }

    pub fn foil(&mut self, amount: f32) {
        if !self.enabled || self.scheme <= 0.0 {
            return;
        }
        self.just_schemed = false;
        self.just_foiled = false;
        let prev = self.scheme;
        self.scheme = (self.scheme - amount).max(0.0);
        if self.scheme <= 0.0 && prev > 0.0 {
            self.just_foiled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.scheme >= self.max_scheme {
            return;
        }
        self.maneuver(self.maneuver_rate * dt);
    }

    pub fn is_schemed(&self) -> bool {
        self.enabled && self.scheme >= self.max_scheme
    }

    pub fn is_foiled(&self) -> bool {
        self.scheme <= 0.0
    }

    pub fn scheme_fraction(&self) -> f32 {
        if self.max_scheme <= 0.0 {
            return 0.0;
        }
        self.scheme / self.max_scheme
    }

    pub fn effective_cunning(&self, scale: f32) -> f32 {
        self.scheme_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wangle() -> Wangle {
        Wangle {
            scheme: 0.0,
            max_scheme: 100.0,
            maneuver_rate: 10.0,
            just_schemed: false,
            just_foiled: false,
            enabled: true,
        }
    }

    #[test]
    fn default_scheme_zero() {
        let w = Wangle::default();
        assert_eq!(w.scheme, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wangle::default().enabled);
    }

    #[test]
    fn maneuver_increases_scheme() {
        let mut w = wangle();
        w.maneuver(30.0);
        assert_eq!(w.scheme, 30.0);
    }

    #[test]
    fn maneuver_clamps_at_max() {
        let mut w = wangle();
        w.maneuver(200.0);
        assert_eq!(w.scheme, 100.0);
    }

    #[test]
    fn maneuver_no_op_when_disabled() {
        let mut w = wangle();
        w.enabled = false;
        w.maneuver(50.0);
        assert_eq!(w.scheme, 0.0);
    }

    #[test]
    fn maneuver_sets_just_schemed_at_max() {
        let mut w = wangle();
        w.maneuver(100.0);
        assert!(w.just_schemed);
    }

    #[test]
    fn maneuver_no_just_schemed_if_already_max() {
        let mut w = wangle();
        w.scheme = 100.0;
        w.maneuver(1.0);
        assert!(!w.just_schemed);
    }

    #[test]
    fn foil_decreases_scheme() {
        let mut w = wangle();
        w.scheme = 60.0;
        w.foil(20.0);
        assert_eq!(w.scheme, 40.0);
    }

    #[test]
    fn foil_clamps_at_zero() {
        let mut w = wangle();
        w.scheme = 30.0;
        w.foil(200.0);
        assert_eq!(w.scheme, 0.0);
    }

    #[test]
    fn foil_no_op_when_disabled() {
        let mut w = wangle();
        w.scheme = 50.0;
        w.enabled = false;
        w.foil(10.0);
        assert_eq!(w.scheme, 50.0);
    }

    #[test]
    fn foil_no_op_when_already_foiled() {
        let mut w = wangle();
        w.foil(10.0);
        assert_eq!(w.scheme, 0.0);
    }

    #[test]
    fn foil_sets_just_foiled_at_zero() {
        let mut w = wangle();
        w.scheme = 10.0;
        w.foil(10.0);
        assert!(w.just_foiled);
    }

    #[test]
    fn foil_no_just_foiled_if_already_zero() {
        let mut w = wangle();
        w.foil(1.0);
        assert!(!w.just_foiled);
    }

    #[test]
    fn tick_increases_scheme() {
        let mut w = wangle();
        w.tick(1.0);
        assert_eq!(w.scheme, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wangle();
        w.tick(2.0);
        assert_eq!(w.scheme, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wangle();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.scheme, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_schemed() {
        let mut w = wangle();
        w.scheme = 100.0;
        w.tick(1.0);
        assert_eq!(w.scheme, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wangle();
        w.maneuver_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.scheme, 0.0);
    }

    #[test]
    fn is_schemed_true_at_max() {
        let mut w = wangle();
        w.scheme = 100.0;
        assert!(w.is_schemed());
    }

    #[test]
    fn is_schemed_false_below_max() {
        let mut w = wangle();
        w.scheme = 50.0;
        assert!(!w.is_schemed());
    }

    #[test]
    fn is_schemed_false_when_disabled() {
        let mut w = wangle();
        w.scheme = 100.0;
        w.enabled = false;
        assert!(!w.is_schemed());
    }

    #[test]
    fn is_foiled_true_at_zero() {
        let w = wangle();
        assert!(w.is_foiled());
    }

    #[test]
    fn is_foiled_false_above_zero() {
        let mut w = wangle();
        w.scheme = 1.0;
        assert!(!w.is_foiled());
    }

    #[test]
    fn scheme_fraction_zero_when_foiled() {
        let w = wangle();
        assert_eq!(w.scheme_fraction(), 0.0);
    }

    #[test]
    fn scheme_fraction_one_at_max() {
        let mut w = wangle();
        w.scheme = 100.0;
        assert_eq!(w.scheme_fraction(), 1.0);
    }

    #[test]
    fn scheme_fraction_half_at_midpoint() {
        let mut w = wangle();
        w.scheme = 50.0;
        assert_eq!(w.scheme_fraction(), 0.5);
    }

    #[test]
    fn scheme_fraction_zero_when_max_zero() {
        let mut w = wangle();
        w.max_scheme = 0.0;
        assert_eq!(w.scheme_fraction(), 0.0);
    }

    #[test]
    fn effective_cunning_scales() {
        let mut w = wangle();
        w.scheme = 50.0;
        assert_eq!(w.effective_cunning(2.0), 1.0);
    }

    #[test]
    fn effective_cunning_zero_when_foiled() {
        let w = wangle();
        assert_eq!(w.effective_cunning(10.0), 0.0);
    }

    #[test]
    fn just_schemed_cleared_on_next_maneuver() {
        let mut w = wangle();
        w.maneuver(100.0);
        assert!(w.just_schemed);
        w.maneuver(1.0);
        assert!(!w.just_schemed);
    }

    #[test]
    fn just_foiled_cleared_on_next_foil() {
        let mut w = wangle();
        w.scheme = 10.0;
        w.foil(10.0);
        assert!(w.just_foiled);
        w.scheme = 10.0;
        w.foil(1.0);
        assert!(!w.just_foiled);
    }
}

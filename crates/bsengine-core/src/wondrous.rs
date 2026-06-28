use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wondrous {
    pub marvel: f32,
    pub max_marvel: f32,
    pub awe_rate: f32,
    pub just_astounded: bool,
    pub just_mundane: bool,
    pub enabled: bool,
}

impl Default for Wondrous {
    fn default() -> Self {
        Self {
            marvel: 0.0,
            max_marvel: 100.0,
            awe_rate: 1.0,
            just_astounded: false,
            just_mundane: false,
            enabled: true,
        }
    }
}

impl Wondrous {
    pub fn astound(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_astounded = false;
        self.just_mundane = false;
        let prev = self.marvel;
        self.marvel = (self.marvel + amount).clamp(0.0, self.max_marvel);
        if self.marvel >= self.max_marvel && prev < self.max_marvel {
            self.just_astounded = true;
        }
    }

    pub fn mundanify(&mut self, amount: f32) {
        if !self.enabled || self.marvel <= 0.0 {
            return;
        }
        self.just_astounded = false;
        self.just_mundane = false;
        let prev = self.marvel;
        self.marvel = (self.marvel - amount).max(0.0);
        if self.marvel <= 0.0 && prev > 0.0 {
            self.just_mundane = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.marvel >= self.max_marvel {
            return;
        }
        self.astound(self.awe_rate * dt);
    }

    pub fn is_astounded(&self) -> bool {
        self.enabled && self.marvel >= self.max_marvel
    }

    pub fn is_mundane(&self) -> bool {
        self.marvel <= 0.0
    }

    pub fn marvel_fraction(&self) -> f32 {
        if self.max_marvel <= 0.0 {
            return 0.0;
        }
        self.marvel / self.max_marvel
    }

    pub fn effective_wonder(&self, scale: f32) -> f32 {
        self.marvel_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wondrous() -> Wondrous {
        Wondrous {
            marvel: 0.0,
            max_marvel: 100.0,
            awe_rate: 10.0,
            just_astounded: false,
            just_mundane: false,
            enabled: true,
        }
    }

    #[test]
    fn default_marvel_zero() {
        let w = Wondrous::default();
        assert_eq!(w.marvel, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wondrous::default().enabled);
    }

    #[test]
    fn astound_increases_marvel() {
        let mut w = wondrous();
        w.astound(30.0);
        assert_eq!(w.marvel, 30.0);
    }

    #[test]
    fn astound_clamps_at_max() {
        let mut w = wondrous();
        w.astound(200.0);
        assert_eq!(w.marvel, 100.0);
    }

    #[test]
    fn astound_no_op_when_disabled() {
        let mut w = wondrous();
        w.enabled = false;
        w.astound(50.0);
        assert_eq!(w.marvel, 0.0);
    }

    #[test]
    fn astound_sets_just_astounded_at_max() {
        let mut w = wondrous();
        w.astound(100.0);
        assert!(w.just_astounded);
    }

    #[test]
    fn astound_no_just_astounded_if_already_max() {
        let mut w = wondrous();
        w.marvel = 100.0;
        w.astound(1.0);
        assert!(!w.just_astounded);
    }

    #[test]
    fn mundanify_decreases_marvel() {
        let mut w = wondrous();
        w.marvel = 60.0;
        w.mundanify(20.0);
        assert_eq!(w.marvel, 40.0);
    }

    #[test]
    fn mundanify_clamps_at_zero() {
        let mut w = wondrous();
        w.marvel = 30.0;
        w.mundanify(200.0);
        assert_eq!(w.marvel, 0.0);
    }

    #[test]
    fn mundanify_no_op_when_disabled() {
        let mut w = wondrous();
        w.marvel = 50.0;
        w.enabled = false;
        w.mundanify(10.0);
        assert_eq!(w.marvel, 50.0);
    }

    #[test]
    fn mundanify_no_op_when_already_mundane() {
        let mut w = wondrous();
        w.mundanify(10.0);
        assert_eq!(w.marvel, 0.0);
    }

    #[test]
    fn mundanify_sets_just_mundane_at_zero() {
        let mut w = wondrous();
        w.marvel = 10.0;
        w.mundanify(10.0);
        assert!(w.just_mundane);
    }

    #[test]
    fn mundanify_no_just_mundane_if_already_zero() {
        let mut w = wondrous();
        w.mundanify(1.0);
        assert!(!w.just_mundane);
    }

    #[test]
    fn tick_increases_marvel() {
        let mut w = wondrous();
        w.tick(1.0);
        assert_eq!(w.marvel, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wondrous();
        w.tick(2.0);
        assert_eq!(w.marvel, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wondrous();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.marvel, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_astounded() {
        let mut w = wondrous();
        w.marvel = 100.0;
        w.tick(1.0);
        assert_eq!(w.marvel, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wondrous();
        w.awe_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.marvel, 0.0);
    }

    #[test]
    fn is_astounded_true_at_max() {
        let mut w = wondrous();
        w.marvel = 100.0;
        assert!(w.is_astounded());
    }

    #[test]
    fn is_astounded_false_below_max() {
        let mut w = wondrous();
        w.marvel = 50.0;
        assert!(!w.is_astounded());
    }

    #[test]
    fn is_astounded_false_when_disabled() {
        let mut w = wondrous();
        w.marvel = 100.0;
        w.enabled = false;
        assert!(!w.is_astounded());
    }

    #[test]
    fn is_mundane_true_at_zero() {
        let w = wondrous();
        assert!(w.is_mundane());
    }

    #[test]
    fn is_mundane_false_above_zero() {
        let mut w = wondrous();
        w.marvel = 1.0;
        assert!(!w.is_mundane());
    }

    #[test]
    fn marvel_fraction_zero_when_mundane() {
        let w = wondrous();
        assert_eq!(w.marvel_fraction(), 0.0);
    }

    #[test]
    fn marvel_fraction_one_at_max() {
        let mut w = wondrous();
        w.marvel = 100.0;
        assert_eq!(w.marvel_fraction(), 1.0);
    }

    #[test]
    fn marvel_fraction_half_at_midpoint() {
        let mut w = wondrous();
        w.marvel = 50.0;
        assert_eq!(w.marvel_fraction(), 0.5);
    }

    #[test]
    fn marvel_fraction_zero_when_max_zero() {
        let mut w = wondrous();
        w.max_marvel = 0.0;
        assert_eq!(w.marvel_fraction(), 0.0);
    }

    #[test]
    fn effective_wonder_scales() {
        let mut w = wondrous();
        w.marvel = 50.0;
        assert_eq!(w.effective_wonder(2.0), 1.0);
    }

    #[test]
    fn effective_wonder_zero_when_mundane() {
        let w = wondrous();
        assert_eq!(w.effective_wonder(10.0), 0.0);
    }

    #[test]
    fn just_astounded_cleared_on_next_astound() {
        let mut w = wondrous();
        w.astound(100.0);
        assert!(w.just_astounded);
        w.astound(1.0);
        assert!(!w.just_astounded);
    }

    #[test]
    fn just_mundane_cleared_on_next_mundanify() {
        let mut w = wondrous();
        w.marvel = 10.0;
        w.mundanify(10.0);
        assert!(w.just_mundane);
        w.marvel = 10.0;
        w.mundanify(1.0);
        assert!(!w.just_mundane);
    }
}

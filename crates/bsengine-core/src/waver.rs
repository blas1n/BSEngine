use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Waver {
    pub doubt: f32,
    pub max_doubt: f32,
    pub waver_rate: f32,
    pub just_wavered: bool,
    pub just_resolved: bool,
    pub enabled: bool,
}

impl Default for Waver {
    fn default() -> Self {
        Self {
            doubt: 0.0,
            max_doubt: 100.0,
            waver_rate: 1.0,
            just_wavered: false,
            just_resolved: false,
            enabled: true,
        }
    }
}

impl Waver {
    pub fn hesitate(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_wavered = false;
        self.just_resolved = false;
        let prev = self.doubt;
        self.doubt = (self.doubt + amount).clamp(0.0, self.max_doubt);
        if self.doubt >= self.max_doubt && prev < self.max_doubt {
            self.just_wavered = true;
        }
    }

    pub fn resolve(&mut self, amount: f32) {
        if !self.enabled || self.doubt <= 0.0 {
            return;
        }
        self.just_wavered = false;
        self.just_resolved = false;
        let prev = self.doubt;
        self.doubt = (self.doubt - amount).max(0.0);
        if self.doubt <= 0.0 && prev > 0.0 {
            self.just_resolved = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.doubt >= self.max_doubt {
            return;
        }
        self.hesitate(self.waver_rate * dt);
    }

    pub fn is_wavered(&self) -> bool {
        self.enabled && self.doubt >= self.max_doubt
    }

    pub fn is_resolved(&self) -> bool {
        self.doubt <= 0.0
    }

    pub fn doubt_fraction(&self) -> f32 {
        if self.max_doubt <= 0.0 {
            return 0.0;
        }
        self.doubt / self.max_doubt
    }

    pub fn effective_vacillation(&self, scale: f32) -> f32 {
        self.doubt_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waver() -> Waver {
        Waver {
            doubt: 0.0,
            max_doubt: 100.0,
            waver_rate: 10.0,
            just_wavered: false,
            just_resolved: false,
            enabled: true,
        }
    }

    #[test]
    fn default_doubt_zero() {
        let w = Waver::default();
        assert_eq!(w.doubt, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Waver::default().enabled);
    }

    #[test]
    fn hesitate_increases_doubt() {
        let mut w = waver();
        w.hesitate(30.0);
        assert_eq!(w.doubt, 30.0);
    }

    #[test]
    fn hesitate_clamps_at_max() {
        let mut w = waver();
        w.hesitate(200.0);
        assert_eq!(w.doubt, 100.0);
    }

    #[test]
    fn hesitate_no_op_when_disabled() {
        let mut w = waver();
        w.enabled = false;
        w.hesitate(50.0);
        assert_eq!(w.doubt, 0.0);
    }

    #[test]
    fn hesitate_sets_just_wavered_at_max() {
        let mut w = waver();
        w.hesitate(100.0);
        assert!(w.just_wavered);
    }

    #[test]
    fn hesitate_no_just_wavered_if_already_max() {
        let mut w = waver();
        w.doubt = 100.0;
        w.hesitate(1.0);
        assert!(!w.just_wavered);
    }

    #[test]
    fn resolve_decreases_doubt() {
        let mut w = waver();
        w.doubt = 60.0;
        w.resolve(20.0);
        assert_eq!(w.doubt, 40.0);
    }

    #[test]
    fn resolve_clamps_at_zero() {
        let mut w = waver();
        w.doubt = 30.0;
        w.resolve(200.0);
        assert_eq!(w.doubt, 0.0);
    }

    #[test]
    fn resolve_no_op_when_disabled() {
        let mut w = waver();
        w.doubt = 50.0;
        w.enabled = false;
        w.resolve(10.0);
        assert_eq!(w.doubt, 50.0);
    }

    #[test]
    fn resolve_no_op_when_already_resolved() {
        let mut w = waver();
        w.resolve(10.0);
        assert_eq!(w.doubt, 0.0);
    }

    #[test]
    fn resolve_sets_just_resolved_at_zero() {
        let mut w = waver();
        w.doubt = 10.0;
        w.resolve(10.0);
        assert!(w.just_resolved);
    }

    #[test]
    fn resolve_no_just_resolved_if_already_zero() {
        let mut w = waver();
        w.resolve(1.0);
        assert!(!w.just_resolved);
    }

    #[test]
    fn tick_increases_doubt() {
        let mut w = waver();
        w.tick(1.0);
        assert_eq!(w.doubt, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = waver();
        w.tick(2.0);
        assert_eq!(w.doubt, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = waver();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.doubt, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_wavered() {
        let mut w = waver();
        w.doubt = 100.0;
        w.tick(1.0);
        assert_eq!(w.doubt, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = waver();
        w.waver_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.doubt, 0.0);
    }

    #[test]
    fn is_wavered_true_at_max() {
        let mut w = waver();
        w.doubt = 100.0;
        assert!(w.is_wavered());
    }

    #[test]
    fn is_wavered_false_below_max() {
        let mut w = waver();
        w.doubt = 50.0;
        assert!(!w.is_wavered());
    }

    #[test]
    fn is_wavered_false_when_disabled() {
        let mut w = waver();
        w.doubt = 100.0;
        w.enabled = false;
        assert!(!w.is_wavered());
    }

    #[test]
    fn is_resolved_true_at_zero() {
        let w = waver();
        assert!(w.is_resolved());
    }

    #[test]
    fn is_resolved_false_above_zero() {
        let mut w = waver();
        w.doubt = 1.0;
        assert!(!w.is_resolved());
    }

    #[test]
    fn doubt_fraction_zero_when_resolved() {
        let w = waver();
        assert_eq!(w.doubt_fraction(), 0.0);
    }

    #[test]
    fn doubt_fraction_one_at_max() {
        let mut w = waver();
        w.doubt = 100.0;
        assert_eq!(w.doubt_fraction(), 1.0);
    }

    #[test]
    fn doubt_fraction_half_at_midpoint() {
        let mut w = waver();
        w.doubt = 50.0;
        assert_eq!(w.doubt_fraction(), 0.5);
    }

    #[test]
    fn doubt_fraction_zero_when_max_zero() {
        let mut w = waver();
        w.max_doubt = 0.0;
        assert_eq!(w.doubt_fraction(), 0.0);
    }

    #[test]
    fn effective_vacillation_scales() {
        let mut w = waver();
        w.doubt = 50.0;
        assert_eq!(w.effective_vacillation(2.0), 1.0);
    }

    #[test]
    fn effective_vacillation_zero_when_resolved() {
        let w = waver();
        assert_eq!(w.effective_vacillation(10.0), 0.0);
    }

    #[test]
    fn just_wavered_cleared_on_next_hesitate() {
        let mut w = waver();
        w.hesitate(100.0);
        assert!(w.just_wavered);
        w.hesitate(1.0);
        assert!(!w.just_wavered);
    }

    #[test]
    fn just_resolved_cleared_on_next_resolve() {
        let mut w = waver();
        w.doubt = 10.0;
        w.resolve(10.0);
        assert!(w.just_resolved);
        w.doubt = 10.0;
        w.resolve(1.0);
        assert!(!w.just_resolved);
    }
}

use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrathful {
    pub fury: f32,
    pub max_fury: f32,
    pub rage_rate: f32,
    pub just_enraged: bool,
    pub just_calmed: bool,
    pub enabled: bool,
}

impl Default for Wrathful {
    fn default() -> Self {
        Self {
            fury: 0.0,
            max_fury: 100.0,
            rage_rate: 1.0,
            just_enraged: false,
            just_calmed: false,
            enabled: true,
        }
    }
}

impl Wrathful {
    pub fn rage(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_enraged = false;
        self.just_calmed = false;
        let prev = self.fury;
        self.fury = (self.fury + amount).clamp(0.0, self.max_fury);
        if self.fury >= self.max_fury && prev < self.max_fury {
            self.just_enraged = true;
        }
    }

    pub fn calm(&mut self, amount: f32) {
        if !self.enabled || self.fury <= 0.0 {
            return;
        }
        self.just_enraged = false;
        self.just_calmed = false;
        let prev = self.fury;
        self.fury = (self.fury - amount).max(0.0);
        if self.fury <= 0.0 && prev > 0.0 {
            self.just_calmed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.fury >= self.max_fury {
            return;
        }
        self.rage(self.rage_rate * dt);
    }

    pub fn is_enraged(&self) -> bool {
        self.enabled && self.fury >= self.max_fury
    }

    pub fn is_calmed(&self) -> bool {
        self.fury <= 0.0
    }

    pub fn fury_fraction(&self) -> f32 {
        if self.max_fury <= 0.0 {
            return 0.0;
        }
        self.fury / self.max_fury
    }

    pub fn effective_wrath(&self, scale: f32) -> f32 {
        self.fury_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrathful() -> Wrathful {
        Wrathful {
            fury: 0.0,
            max_fury: 100.0,
            rage_rate: 10.0,
            just_enraged: false,
            just_calmed: false,
            enabled: true,
        }
    }

    #[test]
    fn default_fury_zero() {
        let w = Wrathful::default();
        assert_eq!(w.fury, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wrathful::default().enabled);
    }

    #[test]
    fn rage_increases_fury() {
        let mut w = wrathful();
        w.rage(30.0);
        assert_eq!(w.fury, 30.0);
    }

    #[test]
    fn rage_clamps_at_max() {
        let mut w = wrathful();
        w.rage(200.0);
        assert_eq!(w.fury, 100.0);
    }

    #[test]
    fn rage_no_op_when_disabled() {
        let mut w = wrathful();
        w.enabled = false;
        w.rage(50.0);
        assert_eq!(w.fury, 0.0);
    }

    #[test]
    fn rage_sets_just_enraged_at_max() {
        let mut w = wrathful();
        w.rage(100.0);
        assert!(w.just_enraged);
    }

    #[test]
    fn rage_no_just_enraged_if_already_max() {
        let mut w = wrathful();
        w.fury = 100.0;
        w.rage(1.0);
        assert!(!w.just_enraged);
    }

    #[test]
    fn calm_decreases_fury() {
        let mut w = wrathful();
        w.fury = 60.0;
        w.calm(20.0);
        assert_eq!(w.fury, 40.0);
    }

    #[test]
    fn calm_clamps_at_zero() {
        let mut w = wrathful();
        w.fury = 30.0;
        w.calm(200.0);
        assert_eq!(w.fury, 0.0);
    }

    #[test]
    fn calm_no_op_when_disabled() {
        let mut w = wrathful();
        w.fury = 50.0;
        w.enabled = false;
        w.calm(10.0);
        assert_eq!(w.fury, 50.0);
    }

    #[test]
    fn calm_no_op_when_already_calmed() {
        let mut w = wrathful();
        w.calm(10.0);
        assert_eq!(w.fury, 0.0);
    }

    #[test]
    fn calm_sets_just_calmed_at_zero() {
        let mut w = wrathful();
        w.fury = 10.0;
        w.calm(10.0);
        assert!(w.just_calmed);
    }

    #[test]
    fn calm_no_just_calmed_if_already_zero() {
        let mut w = wrathful();
        w.calm(1.0);
        assert!(!w.just_calmed);
    }

    #[test]
    fn tick_increases_fury() {
        let mut w = wrathful();
        w.tick(1.0);
        assert_eq!(w.fury, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wrathful();
        w.tick(2.0);
        assert_eq!(w.fury, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wrathful();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.fury, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_enraged() {
        let mut w = wrathful();
        w.fury = 100.0;
        w.tick(1.0);
        assert_eq!(w.fury, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wrathful();
        w.rage_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.fury, 0.0);
    }

    #[test]
    fn is_enraged_true_at_max() {
        let mut w = wrathful();
        w.fury = 100.0;
        assert!(w.is_enraged());
    }

    #[test]
    fn is_enraged_false_below_max() {
        let mut w = wrathful();
        w.fury = 50.0;
        assert!(!w.is_enraged());
    }

    #[test]
    fn is_enraged_false_when_disabled() {
        let mut w = wrathful();
        w.fury = 100.0;
        w.enabled = false;
        assert!(!w.is_enraged());
    }

    #[test]
    fn is_calmed_true_at_zero() {
        let w = wrathful();
        assert!(w.is_calmed());
    }

    #[test]
    fn is_calmed_false_above_zero() {
        let mut w = wrathful();
        w.fury = 1.0;
        assert!(!w.is_calmed());
    }

    #[test]
    fn fury_fraction_zero_when_calmed() {
        let w = wrathful();
        assert_eq!(w.fury_fraction(), 0.0);
    }

    #[test]
    fn fury_fraction_one_at_max() {
        let mut w = wrathful();
        w.fury = 100.0;
        assert_eq!(w.fury_fraction(), 1.0);
    }

    #[test]
    fn fury_fraction_half_at_midpoint() {
        let mut w = wrathful();
        w.fury = 50.0;
        assert_eq!(w.fury_fraction(), 0.5);
    }

    #[test]
    fn fury_fraction_zero_when_max_zero() {
        let mut w = wrathful();
        w.max_fury = 0.0;
        assert_eq!(w.fury_fraction(), 0.0);
    }

    #[test]
    fn effective_wrath_scales() {
        let mut w = wrathful();
        w.fury = 50.0;
        assert_eq!(w.effective_wrath(2.0), 1.0);
    }

    #[test]
    fn effective_wrath_zero_when_calmed() {
        let w = wrathful();
        assert_eq!(w.effective_wrath(10.0), 0.0);
    }

    #[test]
    fn just_enraged_cleared_on_next_rage() {
        let mut w = wrathful();
        w.rage(100.0);
        assert!(w.just_enraged);
        w.rage(1.0);
        assert!(!w.just_enraged);
    }

    #[test]
    fn just_calmed_cleared_on_next_calm() {
        let mut w = wrathful();
        w.fury = 10.0;
        w.calm(10.0);
        assert!(w.just_calmed);
        w.fury = 10.0;
        w.calm(1.0);
        assert!(!w.just_calmed);
    }
}

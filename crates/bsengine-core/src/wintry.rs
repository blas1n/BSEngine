use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wintry {
    pub cold: f32,
    pub max_cold: f32,
    pub chill_rate: f32,
    pub just_frozen: bool,
    pub just_thawed: bool,
    pub enabled: bool,
}

impl Default for Wintry {
    fn default() -> Self {
        Self {
            cold: 0.0,
            max_cold: 100.0,
            chill_rate: 1.0,
            just_frozen: false,
            just_thawed: false,
            enabled: true,
        }
    }
}

impl Wintry {
    pub fn chill(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_frozen = false;
        self.just_thawed = false;
        let prev = self.cold;
        self.cold = (self.cold + amount).clamp(0.0, self.max_cold);
        if self.cold >= self.max_cold && prev < self.max_cold {
            self.just_frozen = true;
        }
    }

    pub fn thaw(&mut self, amount: f32) {
        if !self.enabled || self.cold <= 0.0 {
            return;
        }
        self.just_frozen = false;
        self.just_thawed = false;
        let prev = self.cold;
        self.cold = (self.cold - amount).max(0.0);
        if self.cold <= 0.0 && prev > 0.0 {
            self.just_thawed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.cold >= self.max_cold {
            return;
        }
        self.chill(self.chill_rate * dt);
    }

    pub fn is_frozen(&self) -> bool {
        self.enabled && self.cold >= self.max_cold
    }

    pub fn is_thawed(&self) -> bool {
        self.cold <= 0.0
    }

    pub fn cold_fraction(&self) -> f32 {
        if self.max_cold <= 0.0 {
            return 0.0;
        }
        self.cold / self.max_cold
    }

    pub fn effective_frost(&self, scale: f32) -> f32 {
        self.cold_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wintry() -> Wintry {
        Wintry {
            cold: 0.0,
            max_cold: 100.0,
            chill_rate: 10.0,
            just_frozen: false,
            just_thawed: false,
            enabled: true,
        }
    }

    #[test]
    fn default_cold_zero() {
        let w = Wintry::default();
        assert_eq!(w.cold, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wintry::default().enabled);
    }

    #[test]
    fn chill_increases_cold() {
        let mut w = wintry();
        w.chill(30.0);
        assert_eq!(w.cold, 30.0);
    }

    #[test]
    fn chill_clamps_at_max() {
        let mut w = wintry();
        w.chill(200.0);
        assert_eq!(w.cold, 100.0);
    }

    #[test]
    fn chill_no_op_when_disabled() {
        let mut w = wintry();
        w.enabled = false;
        w.chill(50.0);
        assert_eq!(w.cold, 0.0);
    }

    #[test]
    fn chill_sets_just_frozen_at_max() {
        let mut w = wintry();
        w.chill(100.0);
        assert!(w.just_frozen);
    }

    #[test]
    fn chill_no_just_frozen_if_already_max() {
        let mut w = wintry();
        w.cold = 100.0;
        w.chill(1.0);
        assert!(!w.just_frozen);
    }

    #[test]
    fn thaw_decreases_cold() {
        let mut w = wintry();
        w.cold = 60.0;
        w.thaw(20.0);
        assert_eq!(w.cold, 40.0);
    }

    #[test]
    fn thaw_clamps_at_zero() {
        let mut w = wintry();
        w.cold = 30.0;
        w.thaw(200.0);
        assert_eq!(w.cold, 0.0);
    }

    #[test]
    fn thaw_no_op_when_disabled() {
        let mut w = wintry();
        w.cold = 50.0;
        w.enabled = false;
        w.thaw(10.0);
        assert_eq!(w.cold, 50.0);
    }

    #[test]
    fn thaw_no_op_when_already_thawed() {
        let mut w = wintry();
        w.thaw(10.0);
        assert_eq!(w.cold, 0.0);
    }

    #[test]
    fn thaw_sets_just_thawed_at_zero() {
        let mut w = wintry();
        w.cold = 10.0;
        w.thaw(10.0);
        assert!(w.just_thawed);
    }

    #[test]
    fn thaw_no_just_thawed_if_already_zero() {
        let mut w = wintry();
        w.thaw(1.0);
        assert!(!w.just_thawed);
    }

    #[test]
    fn tick_increases_cold() {
        let mut w = wintry();
        w.tick(1.0);
        assert_eq!(w.cold, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wintry();
        w.tick(2.0);
        assert_eq!(w.cold, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wintry();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.cold, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_frozen() {
        let mut w = wintry();
        w.cold = 100.0;
        w.tick(1.0);
        assert_eq!(w.cold, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wintry();
        w.chill_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.cold, 0.0);
    }

    #[test]
    fn is_frozen_true_at_max() {
        let mut w = wintry();
        w.cold = 100.0;
        assert!(w.is_frozen());
    }

    #[test]
    fn is_frozen_false_below_max() {
        let mut w = wintry();
        w.cold = 50.0;
        assert!(!w.is_frozen());
    }

    #[test]
    fn is_frozen_false_when_disabled() {
        let mut w = wintry();
        w.cold = 100.0;
        w.enabled = false;
        assert!(!w.is_frozen());
    }

    #[test]
    fn is_thawed_true_at_zero() {
        let w = wintry();
        assert!(w.is_thawed());
    }

    #[test]
    fn is_thawed_false_above_zero() {
        let mut w = wintry();
        w.cold = 1.0;
        assert!(!w.is_thawed());
    }

    #[test]
    fn cold_fraction_zero_when_thawed() {
        let w = wintry();
        assert_eq!(w.cold_fraction(), 0.0);
    }

    #[test]
    fn cold_fraction_one_at_max() {
        let mut w = wintry();
        w.cold = 100.0;
        assert_eq!(w.cold_fraction(), 1.0);
    }

    #[test]
    fn cold_fraction_half_at_midpoint() {
        let mut w = wintry();
        w.cold = 50.0;
        assert_eq!(w.cold_fraction(), 0.5);
    }

    #[test]
    fn cold_fraction_zero_when_max_zero() {
        let mut w = wintry();
        w.max_cold = 0.0;
        assert_eq!(w.cold_fraction(), 0.0);
    }

    #[test]
    fn effective_frost_scales() {
        let mut w = wintry();
        w.cold = 50.0;
        assert_eq!(w.effective_frost(2.0), 1.0);
    }

    #[test]
    fn effective_frost_zero_when_thawed() {
        let w = wintry();
        assert_eq!(w.effective_frost(10.0), 0.0);
    }

    #[test]
    fn just_frozen_cleared_on_next_chill() {
        let mut w = wintry();
        w.chill(100.0);
        assert!(w.just_frozen);
        w.chill(1.0);
        assert!(!w.just_frozen);
    }

    #[test]
    fn just_thawed_cleared_on_next_thaw() {
        let mut w = wintry();
        w.cold = 10.0;
        w.thaw(10.0);
        assert!(w.just_thawed);
        w.cold = 10.0;
        w.thaw(1.0);
        assert!(!w.just_thawed);
    }
}

use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Witless {
    pub folly: f32,
    pub max_folly: f32,
    pub blunder_rate: f32,
    pub just_addled: bool,
    pub just_lucid: bool,
    pub enabled: bool,
}

impl Default for Witless {
    fn default() -> Self {
        Self {
            folly: 0.0,
            max_folly: 100.0,
            blunder_rate: 1.0,
            just_addled: false,
            just_lucid: false,
            enabled: true,
        }
    }
}

impl Witless {
    pub fn blunder(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_addled = false;
        self.just_lucid = false;
        let prev = self.folly;
        self.folly = (self.folly + amount).clamp(0.0, self.max_folly);
        if self.folly >= self.max_folly && prev < self.max_folly {
            self.just_addled = true;
        }
    }

    pub fn lucid(&mut self, amount: f32) {
        if !self.enabled || self.folly <= 0.0 {
            return;
        }
        self.just_addled = false;
        self.just_lucid = false;
        let prev = self.folly;
        self.folly = (self.folly - amount).max(0.0);
        if self.folly <= 0.0 && prev > 0.0 {
            self.just_lucid = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.folly >= self.max_folly {
            return;
        }
        self.blunder(self.blunder_rate * dt);
    }

    pub fn is_addled(&self) -> bool {
        self.enabled && self.folly >= self.max_folly
    }

    pub fn is_lucid(&self) -> bool {
        self.folly <= 0.0
    }

    pub fn folly_fraction(&self) -> f32 {
        if self.max_folly <= 0.0 {
            return 0.0;
        }
        self.folly / self.max_folly
    }

    pub fn effective_muddle(&self, scale: f32) -> f32 {
        self.folly_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn witless() -> Witless {
        Witless {
            folly: 0.0,
            max_folly: 100.0,
            blunder_rate: 10.0,
            just_addled: false,
            just_lucid: false,
            enabled: true,
        }
    }

    #[test]
    fn default_folly_zero() {
        let w = Witless::default();
        assert_eq!(w.folly, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Witless::default().enabled);
    }

    #[test]
    fn blunder_increases_folly() {
        let mut w = witless();
        w.blunder(30.0);
        assert_eq!(w.folly, 30.0);
    }

    #[test]
    fn blunder_clamps_at_max() {
        let mut w = witless();
        w.blunder(200.0);
        assert_eq!(w.folly, 100.0);
    }

    #[test]
    fn blunder_no_op_when_disabled() {
        let mut w = witless();
        w.enabled = false;
        w.blunder(50.0);
        assert_eq!(w.folly, 0.0);
    }

    #[test]
    fn blunder_sets_just_addled_at_max() {
        let mut w = witless();
        w.blunder(100.0);
        assert!(w.just_addled);
    }

    #[test]
    fn blunder_no_just_addled_if_already_max() {
        let mut w = witless();
        w.folly = 100.0;
        w.blunder(1.0);
        assert!(!w.just_addled);
    }

    #[test]
    fn lucid_decreases_folly() {
        let mut w = witless();
        w.folly = 60.0;
        w.lucid(20.0);
        assert_eq!(w.folly, 40.0);
    }

    #[test]
    fn lucid_clamps_at_zero() {
        let mut w = witless();
        w.folly = 30.0;
        w.lucid(200.0);
        assert_eq!(w.folly, 0.0);
    }

    #[test]
    fn lucid_no_op_when_disabled() {
        let mut w = witless();
        w.folly = 50.0;
        w.enabled = false;
        w.lucid(10.0);
        assert_eq!(w.folly, 50.0);
    }

    #[test]
    fn lucid_no_op_when_already_lucid() {
        let mut w = witless();
        w.lucid(10.0);
        assert_eq!(w.folly, 0.0);
    }

    #[test]
    fn lucid_sets_just_lucid_at_zero() {
        let mut w = witless();
        w.folly = 10.0;
        w.lucid(10.0);
        assert!(w.just_lucid);
    }

    #[test]
    fn lucid_no_just_lucid_if_already_zero() {
        let mut w = witless();
        w.lucid(1.0);
        assert!(!w.just_lucid);
    }

    #[test]
    fn tick_increases_folly() {
        let mut w = witless();
        w.tick(1.0);
        assert_eq!(w.folly, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = witless();
        w.tick(2.0);
        assert_eq!(w.folly, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = witless();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.folly, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_addled() {
        let mut w = witless();
        w.folly = 100.0;
        w.tick(1.0);
        assert_eq!(w.folly, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = witless();
        w.blunder_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.folly, 0.0);
    }

    #[test]
    fn is_addled_true_at_max() {
        let mut w = witless();
        w.folly = 100.0;
        assert!(w.is_addled());
    }

    #[test]
    fn is_addled_false_below_max() {
        let mut w = witless();
        w.folly = 50.0;
        assert!(!w.is_addled());
    }

    #[test]
    fn is_addled_false_when_disabled() {
        let mut w = witless();
        w.folly = 100.0;
        w.enabled = false;
        assert!(!w.is_addled());
    }

    #[test]
    fn is_lucid_true_at_zero() {
        let w = witless();
        assert!(w.is_lucid());
    }

    #[test]
    fn is_lucid_false_above_zero() {
        let mut w = witless();
        w.folly = 1.0;
        assert!(!w.is_lucid());
    }

    #[test]
    fn folly_fraction_zero_when_lucid() {
        let w = witless();
        assert_eq!(w.folly_fraction(), 0.0);
    }

    #[test]
    fn folly_fraction_one_at_max() {
        let mut w = witless();
        w.folly = 100.0;
        assert_eq!(w.folly_fraction(), 1.0);
    }

    #[test]
    fn folly_fraction_half_at_midpoint() {
        let mut w = witless();
        w.folly = 50.0;
        assert_eq!(w.folly_fraction(), 0.5);
    }

    #[test]
    fn folly_fraction_zero_when_max_zero() {
        let mut w = witless();
        w.max_folly = 0.0;
        assert_eq!(w.folly_fraction(), 0.0);
    }

    #[test]
    fn effective_muddle_scales() {
        let mut w = witless();
        w.folly = 50.0;
        assert_eq!(w.effective_muddle(2.0), 1.0);
    }

    #[test]
    fn effective_muddle_zero_when_lucid() {
        let w = witless();
        assert_eq!(w.effective_muddle(10.0), 0.0);
    }

    #[test]
    fn just_addled_cleared_on_next_blunder() {
        let mut w = witless();
        w.blunder(100.0);
        assert!(w.just_addled);
        w.blunder(1.0);
        assert!(!w.just_addled);
    }

    #[test]
    fn just_lucid_cleared_on_next_lucid() {
        let mut w = witless();
        w.folly = 10.0;
        w.lucid(10.0);
        assert!(w.just_lucid);
        w.folly = 10.0;
        w.lucid(1.0);
        assert!(!w.just_lucid);
    }
}

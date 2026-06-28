use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Waylay {
    pub ambush: f32,
    pub max_ambush: f32,
    pub lurk_rate: f32,
    pub just_sprung: bool,
    pub just_dispersed: bool,
    pub enabled: bool,
}

impl Default for Waylay {
    fn default() -> Self {
        Self {
            ambush: 0.0,
            max_ambush: 100.0,
            lurk_rate: 1.0,
            just_sprung: false,
            just_dispersed: false,
            enabled: true,
        }
    }
}

impl Waylay {
    pub fn lurk(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_sprung = false;
        self.just_dispersed = false;
        let prev = self.ambush;
        self.ambush = (self.ambush + amount).clamp(0.0, self.max_ambush);
        if self.ambush >= self.max_ambush && prev < self.max_ambush {
            self.just_sprung = true;
        }
    }

    pub fn disperse(&mut self, amount: f32) {
        if !self.enabled || self.ambush <= 0.0 {
            return;
        }
        self.just_sprung = false;
        self.just_dispersed = false;
        let prev = self.ambush;
        self.ambush = (self.ambush - amount).max(0.0);
        if self.ambush <= 0.0 && prev > 0.0 {
            self.just_dispersed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.ambush >= self.max_ambush {
            return;
        }
        self.lurk(self.lurk_rate * dt);
    }

    pub fn is_sprung(&self) -> bool {
        self.enabled && self.ambush >= self.max_ambush
    }

    pub fn is_dispersed(&self) -> bool {
        self.ambush <= 0.0
    }

    pub fn ambush_fraction(&self) -> f32 {
        if self.max_ambush <= 0.0 {
            return 0.0;
        }
        self.ambush / self.max_ambush
    }

    pub fn effective_trap(&self, scale: f32) -> f32 {
        self.ambush_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waylay() -> Waylay {
        Waylay {
            ambush: 0.0,
            max_ambush: 100.0,
            lurk_rate: 10.0,
            just_sprung: false,
            just_dispersed: false,
            enabled: true,
        }
    }

    #[test]
    fn default_ambush_zero() {
        let w = Waylay::default();
        assert_eq!(w.ambush, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Waylay::default().enabled);
    }

    #[test]
    fn lurk_increases_ambush() {
        let mut w = waylay();
        w.lurk(30.0);
        assert_eq!(w.ambush, 30.0);
    }

    #[test]
    fn lurk_clamps_at_max() {
        let mut w = waylay();
        w.lurk(200.0);
        assert_eq!(w.ambush, 100.0);
    }

    #[test]
    fn lurk_no_op_when_disabled() {
        let mut w = waylay();
        w.enabled = false;
        w.lurk(50.0);
        assert_eq!(w.ambush, 0.0);
    }

    #[test]
    fn lurk_sets_just_sprung_at_max() {
        let mut w = waylay();
        w.lurk(100.0);
        assert!(w.just_sprung);
    }

    #[test]
    fn lurk_no_just_sprung_if_already_max() {
        let mut w = waylay();
        w.ambush = 100.0;
        w.lurk(1.0);
        assert!(!w.just_sprung);
    }

    #[test]
    fn disperse_decreases_ambush() {
        let mut w = waylay();
        w.ambush = 60.0;
        w.disperse(20.0);
        assert_eq!(w.ambush, 40.0);
    }

    #[test]
    fn disperse_clamps_at_zero() {
        let mut w = waylay();
        w.ambush = 30.0;
        w.disperse(200.0);
        assert_eq!(w.ambush, 0.0);
    }

    #[test]
    fn disperse_no_op_when_disabled() {
        let mut w = waylay();
        w.ambush = 50.0;
        w.enabled = false;
        w.disperse(10.0);
        assert_eq!(w.ambush, 50.0);
    }

    #[test]
    fn disperse_no_op_when_already_dispersed() {
        let mut w = waylay();
        w.disperse(10.0);
        assert_eq!(w.ambush, 0.0);
    }

    #[test]
    fn disperse_sets_just_dispersed_at_zero() {
        let mut w = waylay();
        w.ambush = 10.0;
        w.disperse(10.0);
        assert!(w.just_dispersed);
    }

    #[test]
    fn disperse_no_just_dispersed_if_already_zero() {
        let mut w = waylay();
        w.disperse(1.0);
        assert!(!w.just_dispersed);
    }

    #[test]
    fn tick_increases_ambush() {
        let mut w = waylay();
        w.tick(1.0);
        assert_eq!(w.ambush, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = waylay();
        w.tick(2.0);
        assert_eq!(w.ambush, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = waylay();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.ambush, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_sprung() {
        let mut w = waylay();
        w.ambush = 100.0;
        w.tick(1.0);
        assert_eq!(w.ambush, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = waylay();
        w.lurk_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.ambush, 0.0);
    }

    #[test]
    fn is_sprung_true_at_max() {
        let mut w = waylay();
        w.ambush = 100.0;
        assert!(w.is_sprung());
    }

    #[test]
    fn is_sprung_false_below_max() {
        let mut w = waylay();
        w.ambush = 50.0;
        assert!(!w.is_sprung());
    }

    #[test]
    fn is_sprung_false_when_disabled() {
        let mut w = waylay();
        w.ambush = 100.0;
        w.enabled = false;
        assert!(!w.is_sprung());
    }

    #[test]
    fn is_dispersed_true_at_zero() {
        let w = waylay();
        assert!(w.is_dispersed());
    }

    #[test]
    fn is_dispersed_false_above_zero() {
        let mut w = waylay();
        w.ambush = 1.0;
        assert!(!w.is_dispersed());
    }

    #[test]
    fn ambush_fraction_zero_when_dispersed() {
        let w = waylay();
        assert_eq!(w.ambush_fraction(), 0.0);
    }

    #[test]
    fn ambush_fraction_one_at_max() {
        let mut w = waylay();
        w.ambush = 100.0;
        assert_eq!(w.ambush_fraction(), 1.0);
    }

    #[test]
    fn ambush_fraction_half_at_midpoint() {
        let mut w = waylay();
        w.ambush = 50.0;
        assert_eq!(w.ambush_fraction(), 0.5);
    }

    #[test]
    fn ambush_fraction_zero_when_max_zero() {
        let mut w = waylay();
        w.max_ambush = 0.0;
        assert_eq!(w.ambush_fraction(), 0.0);
    }

    #[test]
    fn effective_trap_scales() {
        let mut w = waylay();
        w.ambush = 50.0;
        assert_eq!(w.effective_trap(2.0), 1.0);
    }

    #[test]
    fn effective_trap_zero_when_dispersed() {
        let w = waylay();
        assert_eq!(w.effective_trap(10.0), 0.0);
    }

    #[test]
    fn just_sprung_cleared_on_next_lurk() {
        let mut w = waylay();
        w.lurk(100.0);
        assert!(w.just_sprung);
        w.lurk(1.0);
        assert!(!w.just_sprung);
    }

    #[test]
    fn just_dispersed_cleared_on_next_disperse() {
        let mut w = waylay();
        w.ambush = 10.0;
        w.disperse(10.0);
        assert!(w.just_dispersed);
        w.ambush = 10.0;
        w.disperse(1.0);
        assert!(!w.just_dispersed);
    }
}

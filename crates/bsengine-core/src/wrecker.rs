use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrecker {
    pub destruction: f32,
    pub max_destruction: f32,
    pub demolish_rate: f32,
    pub just_ruined: bool,
    pub just_intact: bool,
    pub enabled: bool,
}

impl Default for Wrecker {
    fn default() -> Self {
        Self {
            destruction: 0.0,
            max_destruction: 100.0,
            demolish_rate: 1.0,
            just_ruined: false,
            just_intact: false,
            enabled: true,
        }
    }
}

impl Wrecker {
    pub fn demolish(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_ruined = false;
        self.just_intact = false;
        let prev = self.destruction;
        self.destruction = (self.destruction + amount).clamp(0.0, self.max_destruction);
        if self.destruction >= self.max_destruction && prev < self.max_destruction {
            self.just_ruined = true;
        }
    }

    pub fn rebuild(&mut self, amount: f32) {
        if !self.enabled || self.destruction <= 0.0 {
            return;
        }
        self.just_ruined = false;
        self.just_intact = false;
        let prev = self.destruction;
        self.destruction = (self.destruction - amount).max(0.0);
        if self.destruction <= 0.0 && prev > 0.0 {
            self.just_intact = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.destruction >= self.max_destruction {
            return;
        }
        self.demolish(self.demolish_rate * dt);
    }

    pub fn is_ruined(&self) -> bool {
        self.enabled && self.destruction >= self.max_destruction
    }

    pub fn is_intact(&self) -> bool {
        self.destruction <= 0.0
    }

    pub fn destruction_fraction(&self) -> f32 {
        if self.max_destruction <= 0.0 {
            return 0.0;
        }
        self.destruction / self.max_destruction
    }

    pub fn effective_ruin(&self, scale: f32) -> f32 {
        self.destruction_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrecker() -> Wrecker {
        Wrecker {
            destruction: 0.0,
            max_destruction: 100.0,
            demolish_rate: 10.0,
            just_ruined: false,
            just_intact: false,
            enabled: true,
        }
    }

    #[test]
    fn default_destruction_zero() {
        let w = Wrecker::default();
        assert_eq!(w.destruction, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wrecker::default().enabled);
    }

    #[test]
    fn demolish_increases_destruction() {
        let mut w = wrecker();
        w.demolish(30.0);
        assert_eq!(w.destruction, 30.0);
    }

    #[test]
    fn demolish_clamps_at_max() {
        let mut w = wrecker();
        w.demolish(200.0);
        assert_eq!(w.destruction, 100.0);
    }

    #[test]
    fn demolish_no_op_when_disabled() {
        let mut w = wrecker();
        w.enabled = false;
        w.demolish(50.0);
        assert_eq!(w.destruction, 0.0);
    }

    #[test]
    fn demolish_sets_just_ruined_at_max() {
        let mut w = wrecker();
        w.demolish(100.0);
        assert!(w.just_ruined);
    }

    #[test]
    fn demolish_no_just_ruined_if_already_max() {
        let mut w = wrecker();
        w.destruction = 100.0;
        w.demolish(1.0);
        assert!(!w.just_ruined);
    }

    #[test]
    fn rebuild_decreases_destruction() {
        let mut w = wrecker();
        w.destruction = 60.0;
        w.rebuild(20.0);
        assert_eq!(w.destruction, 40.0);
    }

    #[test]
    fn rebuild_clamps_at_zero() {
        let mut w = wrecker();
        w.destruction = 30.0;
        w.rebuild(200.0);
        assert_eq!(w.destruction, 0.0);
    }

    #[test]
    fn rebuild_no_op_when_disabled() {
        let mut w = wrecker();
        w.destruction = 50.0;
        w.enabled = false;
        w.rebuild(10.0);
        assert_eq!(w.destruction, 50.0);
    }

    #[test]
    fn rebuild_no_op_when_already_intact() {
        let mut w = wrecker();
        w.rebuild(10.0);
        assert_eq!(w.destruction, 0.0);
    }

    #[test]
    fn rebuild_sets_just_intact_at_zero() {
        let mut w = wrecker();
        w.destruction = 10.0;
        w.rebuild(10.0);
        assert!(w.just_intact);
    }

    #[test]
    fn rebuild_no_just_intact_if_already_zero() {
        let mut w = wrecker();
        w.rebuild(1.0);
        assert!(!w.just_intact);
    }

    #[test]
    fn tick_increases_destruction() {
        let mut w = wrecker();
        w.tick(1.0);
        assert_eq!(w.destruction, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wrecker();
        w.tick(2.0);
        assert_eq!(w.destruction, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wrecker();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.destruction, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_ruined() {
        let mut w = wrecker();
        w.destruction = 100.0;
        w.tick(1.0);
        assert_eq!(w.destruction, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wrecker();
        w.demolish_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.destruction, 0.0);
    }

    #[test]
    fn is_ruined_true_at_max() {
        let mut w = wrecker();
        w.destruction = 100.0;
        assert!(w.is_ruined());
    }

    #[test]
    fn is_ruined_false_below_max() {
        let mut w = wrecker();
        w.destruction = 50.0;
        assert!(!w.is_ruined());
    }

    #[test]
    fn is_ruined_false_when_disabled() {
        let mut w = wrecker();
        w.destruction = 100.0;
        w.enabled = false;
        assert!(!w.is_ruined());
    }

    #[test]
    fn is_intact_true_at_zero() {
        let w = wrecker();
        assert!(w.is_intact());
    }

    #[test]
    fn is_intact_false_above_zero() {
        let mut w = wrecker();
        w.destruction = 1.0;
        assert!(!w.is_intact());
    }

    #[test]
    fn destruction_fraction_zero_when_intact() {
        let w = wrecker();
        assert_eq!(w.destruction_fraction(), 0.0);
    }

    #[test]
    fn destruction_fraction_one_at_max() {
        let mut w = wrecker();
        w.destruction = 100.0;
        assert_eq!(w.destruction_fraction(), 1.0);
    }

    #[test]
    fn destruction_fraction_half_at_midpoint() {
        let mut w = wrecker();
        w.destruction = 50.0;
        assert_eq!(w.destruction_fraction(), 0.5);
    }

    #[test]
    fn destruction_fraction_zero_when_max_zero() {
        let mut w = wrecker();
        w.max_destruction = 0.0;
        assert_eq!(w.destruction_fraction(), 0.0);
    }

    #[test]
    fn effective_ruin_scales() {
        let mut w = wrecker();
        w.destruction = 50.0;
        assert_eq!(w.effective_ruin(2.0), 1.0);
    }

    #[test]
    fn effective_ruin_zero_when_intact() {
        let w = wrecker();
        assert_eq!(w.effective_ruin(10.0), 0.0);
    }

    #[test]
    fn just_ruined_cleared_on_next_demolish() {
        let mut w = wrecker();
        w.demolish(100.0);
        assert!(w.just_ruined);
        w.demolish(1.0);
        assert!(!w.just_ruined);
    }

    #[test]
    fn just_intact_cleared_on_next_rebuild() {
        let mut w = wrecker();
        w.destruction = 10.0;
        w.rebuild(10.0);
        assert!(w.just_intact);
        w.destruction = 10.0;
        w.rebuild(1.0);
        assert!(!w.just_intact);
    }
}

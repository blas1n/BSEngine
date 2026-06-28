use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wold {
    pub expanse: f32,
    pub max_expanse: f32,
    pub roam_rate: f32,
    pub just_vast: bool,
    pub just_barren: bool,
    pub enabled: bool,
}

impl Default for Wold {
    fn default() -> Self {
        Self {
            expanse: 0.0,
            max_expanse: 100.0,
            roam_rate: 1.0,
            just_vast: false,
            just_barren: false,
            enabled: true,
        }
    }
}

impl Wold {
    pub fn roam(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_vast = false;
        self.just_barren = false;
        let prev = self.expanse;
        self.expanse = (self.expanse + amount).clamp(0.0, self.max_expanse);
        if self.expanse >= self.max_expanse && prev < self.max_expanse {
            self.just_vast = true;
        }
    }

    pub fn shrink(&mut self, amount: f32) {
        if !self.enabled || self.expanse <= 0.0 {
            return;
        }
        self.just_vast = false;
        self.just_barren = false;
        let prev = self.expanse;
        self.expanse = (self.expanse - amount).max(0.0);
        if self.expanse <= 0.0 && prev > 0.0 {
            self.just_barren = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.expanse >= self.max_expanse {
            return;
        }
        self.roam(self.roam_rate * dt);
    }

    pub fn is_vast(&self) -> bool {
        self.enabled && self.expanse >= self.max_expanse
    }

    pub fn is_barren(&self) -> bool {
        self.expanse <= 0.0
    }

    pub fn expanse_fraction(&self) -> f32 {
        if self.max_expanse <= 0.0 {
            return 0.0;
        }
        self.expanse / self.max_expanse
    }

    pub fn effective_reach(&self, scale: f32) -> f32 {
        self.expanse_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wold() -> Wold {
        Wold {
            expanse: 0.0,
            max_expanse: 100.0,
            roam_rate: 10.0,
            just_vast: false,
            just_barren: false,
            enabled: true,
        }
    }

    #[test]
    fn default_expanse_zero() {
        let w = Wold::default();
        assert_eq!(w.expanse, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wold::default().enabled);
    }

    #[test]
    fn roam_increases_expanse() {
        let mut w = wold();
        w.roam(30.0);
        assert_eq!(w.expanse, 30.0);
    }

    #[test]
    fn roam_clamps_at_max() {
        let mut w = wold();
        w.roam(200.0);
        assert_eq!(w.expanse, 100.0);
    }

    #[test]
    fn roam_no_op_when_disabled() {
        let mut w = wold();
        w.enabled = false;
        w.roam(50.0);
        assert_eq!(w.expanse, 0.0);
    }

    #[test]
    fn roam_sets_just_vast_at_max() {
        let mut w = wold();
        w.roam(100.0);
        assert!(w.just_vast);
    }

    #[test]
    fn roam_no_just_vast_if_already_max() {
        let mut w = wold();
        w.expanse = 100.0;
        w.roam(1.0);
        assert!(!w.just_vast);
    }

    #[test]
    fn shrink_decreases_expanse() {
        let mut w = wold();
        w.expanse = 60.0;
        w.shrink(20.0);
        assert_eq!(w.expanse, 40.0);
    }

    #[test]
    fn shrink_clamps_at_zero() {
        let mut w = wold();
        w.expanse = 30.0;
        w.shrink(200.0);
        assert_eq!(w.expanse, 0.0);
    }

    #[test]
    fn shrink_no_op_when_disabled() {
        let mut w = wold();
        w.expanse = 50.0;
        w.enabled = false;
        w.shrink(10.0);
        assert_eq!(w.expanse, 50.0);
    }

    #[test]
    fn shrink_no_op_when_already_barren() {
        let mut w = wold();
        w.shrink(10.0);
        assert_eq!(w.expanse, 0.0);
    }

    #[test]
    fn shrink_sets_just_barren_at_zero() {
        let mut w = wold();
        w.expanse = 10.0;
        w.shrink(10.0);
        assert!(w.just_barren);
    }

    #[test]
    fn shrink_no_just_barren_if_already_zero() {
        let mut w = wold();
        w.shrink(1.0);
        assert!(!w.just_barren);
    }

    #[test]
    fn tick_increases_expanse() {
        let mut w = wold();
        w.tick(1.0);
        assert_eq!(w.expanse, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wold();
        w.tick(2.0);
        assert_eq!(w.expanse, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wold();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.expanse, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_vast() {
        let mut w = wold();
        w.expanse = 100.0;
        w.tick(1.0);
        assert_eq!(w.expanse, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wold();
        w.roam_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.expanse, 0.0);
    }

    #[test]
    fn is_vast_true_at_max() {
        let mut w = wold();
        w.expanse = 100.0;
        assert!(w.is_vast());
    }

    #[test]
    fn is_vast_false_below_max() {
        let mut w = wold();
        w.expanse = 50.0;
        assert!(!w.is_vast());
    }

    #[test]
    fn is_vast_false_when_disabled() {
        let mut w = wold();
        w.expanse = 100.0;
        w.enabled = false;
        assert!(!w.is_vast());
    }

    #[test]
    fn is_barren_true_at_zero() {
        let w = wold();
        assert!(w.is_barren());
    }

    #[test]
    fn is_barren_false_above_zero() {
        let mut w = wold();
        w.expanse = 1.0;
        assert!(!w.is_barren());
    }

    #[test]
    fn expanse_fraction_zero_when_barren() {
        let w = wold();
        assert_eq!(w.expanse_fraction(), 0.0);
    }

    #[test]
    fn expanse_fraction_one_at_max() {
        let mut w = wold();
        w.expanse = 100.0;
        assert_eq!(w.expanse_fraction(), 1.0);
    }

    #[test]
    fn expanse_fraction_half_at_midpoint() {
        let mut w = wold();
        w.expanse = 50.0;
        assert_eq!(w.expanse_fraction(), 0.5);
    }

    #[test]
    fn expanse_fraction_zero_when_max_zero() {
        let mut w = wold();
        w.max_expanse = 0.0;
        assert_eq!(w.expanse_fraction(), 0.0);
    }

    #[test]
    fn effective_reach_scales() {
        let mut w = wold();
        w.expanse = 50.0;
        assert_eq!(w.effective_reach(2.0), 1.0);
    }

    #[test]
    fn effective_reach_zero_when_barren() {
        let w = wold();
        assert_eq!(w.effective_reach(10.0), 0.0);
    }

    #[test]
    fn just_vast_cleared_on_next_roam() {
        let mut w = wold();
        w.roam(100.0);
        assert!(w.just_vast);
        w.roam(1.0);
        assert!(!w.just_vast);
    }

    #[test]
    fn just_barren_cleared_on_next_shrink() {
        let mut w = wold();
        w.expanse = 10.0;
        w.shrink(10.0);
        assert!(w.just_barren);
        w.expanse = 10.0;
        w.shrink(1.0);
        assert!(!w.just_barren);
    }
}

use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Walrus {
    pub bulk: f32,
    pub max_bulk: f32,
    pub lumber_rate: f32,
    pub just_massive: bool,
    pub just_lean: bool,
    pub enabled: bool,
}

impl Default for Walrus {
    fn default() -> Self {
        Self {
            bulk: 0.0,
            max_bulk: 100.0,
            lumber_rate: 1.0,
            just_massive: false,
            just_lean: false,
            enabled: true,
        }
    }
}

impl Walrus {
    pub fn lumber(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_massive = false;
        self.just_lean = false;
        let prev = self.bulk;
        self.bulk = (self.bulk + amount).clamp(0.0, self.max_bulk);
        if self.bulk >= self.max_bulk && prev < self.max_bulk {
            self.just_massive = true;
        }
    }

    pub fn slim(&mut self, amount: f32) {
        if !self.enabled || self.bulk <= 0.0 {
            return;
        }
        self.just_massive = false;
        self.just_lean = false;
        let prev = self.bulk;
        self.bulk = (self.bulk - amount).max(0.0);
        if self.bulk <= 0.0 && prev > 0.0 {
            self.just_lean = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.bulk >= self.max_bulk {
            return;
        }
        self.lumber(self.lumber_rate * dt);
    }

    pub fn is_massive(&self) -> bool {
        self.enabled && self.bulk >= self.max_bulk
    }

    pub fn is_lean(&self) -> bool {
        self.bulk <= 0.0
    }

    pub fn bulk_fraction(&self) -> f32 {
        if self.max_bulk <= 0.0 {
            return 0.0;
        }
        self.bulk / self.max_bulk
    }

    pub fn effective_heft(&self, scale: f32) -> f32 {
        self.bulk_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walrus() -> Walrus {
        Walrus {
            bulk: 0.0,
            max_bulk: 100.0,
            lumber_rate: 10.0,
            just_massive: false,
            just_lean: false,
            enabled: true,
        }
    }

    #[test]
    fn default_bulk_zero() {
        let w = Walrus::default();
        assert_eq!(w.bulk, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Walrus::default().enabled);
    }

    #[test]
    fn lumber_increases_bulk() {
        let mut w = walrus();
        w.lumber(30.0);
        assert_eq!(w.bulk, 30.0);
    }

    #[test]
    fn lumber_clamps_at_max() {
        let mut w = walrus();
        w.lumber(200.0);
        assert_eq!(w.bulk, 100.0);
    }

    #[test]
    fn lumber_no_op_when_disabled() {
        let mut w = walrus();
        w.enabled = false;
        w.lumber(50.0);
        assert_eq!(w.bulk, 0.0);
    }

    #[test]
    fn lumber_sets_just_massive_at_max() {
        let mut w = walrus();
        w.lumber(100.0);
        assert!(w.just_massive);
    }

    #[test]
    fn lumber_no_just_massive_if_already_max() {
        let mut w = walrus();
        w.bulk = 100.0;
        w.lumber(1.0);
        assert!(!w.just_massive);
    }

    #[test]
    fn slim_decreases_bulk() {
        let mut w = walrus();
        w.bulk = 60.0;
        w.slim(20.0);
        assert_eq!(w.bulk, 40.0);
    }

    #[test]
    fn slim_clamps_at_zero() {
        let mut w = walrus();
        w.bulk = 30.0;
        w.slim(200.0);
        assert_eq!(w.bulk, 0.0);
    }

    #[test]
    fn slim_no_op_when_disabled() {
        let mut w = walrus();
        w.bulk = 50.0;
        w.enabled = false;
        w.slim(10.0);
        assert_eq!(w.bulk, 50.0);
    }

    #[test]
    fn slim_no_op_when_already_lean() {
        let mut w = walrus();
        w.slim(10.0);
        assert_eq!(w.bulk, 0.0);
    }

    #[test]
    fn slim_sets_just_lean_at_zero() {
        let mut w = walrus();
        w.bulk = 10.0;
        w.slim(10.0);
        assert!(w.just_lean);
    }

    #[test]
    fn slim_no_just_lean_if_already_zero() {
        let mut w = walrus();
        w.slim(1.0);
        assert!(!w.just_lean);
    }

    #[test]
    fn tick_increases_bulk() {
        let mut w = walrus();
        w.tick(1.0);
        assert_eq!(w.bulk, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = walrus();
        w.tick(2.0);
        assert_eq!(w.bulk, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = walrus();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.bulk, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_massive() {
        let mut w = walrus();
        w.bulk = 100.0;
        w.tick(1.0);
        assert_eq!(w.bulk, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = walrus();
        w.lumber_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.bulk, 0.0);
    }

    #[test]
    fn is_massive_true_at_max() {
        let mut w = walrus();
        w.bulk = 100.0;
        assert!(w.is_massive());
    }

    #[test]
    fn is_massive_false_below_max() {
        let mut w = walrus();
        w.bulk = 50.0;
        assert!(!w.is_massive());
    }

    #[test]
    fn is_massive_false_when_disabled() {
        let mut w = walrus();
        w.bulk = 100.0;
        w.enabled = false;
        assert!(!w.is_massive());
    }

    #[test]
    fn is_lean_true_at_zero() {
        let w = walrus();
        assert!(w.is_lean());
    }

    #[test]
    fn is_lean_false_above_zero() {
        let mut w = walrus();
        w.bulk = 1.0;
        assert!(!w.is_lean());
    }

    #[test]
    fn bulk_fraction_zero_when_lean() {
        let w = walrus();
        assert_eq!(w.bulk_fraction(), 0.0);
    }

    #[test]
    fn bulk_fraction_one_at_max() {
        let mut w = walrus();
        w.bulk = 100.0;
        assert_eq!(w.bulk_fraction(), 1.0);
    }

    #[test]
    fn bulk_fraction_half_at_midpoint() {
        let mut w = walrus();
        w.bulk = 50.0;
        assert_eq!(w.bulk_fraction(), 0.5);
    }

    #[test]
    fn bulk_fraction_zero_when_max_zero() {
        let mut w = walrus();
        w.max_bulk = 0.0;
        assert_eq!(w.bulk_fraction(), 0.0);
    }

    #[test]
    fn effective_heft_scales() {
        let mut w = walrus();
        w.bulk = 50.0;
        assert_eq!(w.effective_heft(2.0), 1.0);
    }

    #[test]
    fn effective_heft_zero_when_lean() {
        let w = walrus();
        assert_eq!(w.effective_heft(10.0), 0.0);
    }

    #[test]
    fn just_massive_cleared_on_next_lumber() {
        let mut w = walrus();
        w.lumber(100.0);
        assert!(w.just_massive);
        w.lumber(1.0);
        assert!(!w.just_massive);
    }

    #[test]
    fn just_lean_cleared_on_next_slim() {
        let mut w = walrus();
        w.bulk = 10.0;
        w.slim(10.0);
        assert!(w.just_lean);
        w.bulk = 10.0;
        w.slim(1.0);
        assert!(!w.just_lean);
    }
}

use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Waive {
    pub concession: f32,
    pub max_concession: f32,
    pub yield_rate: f32,
    pub just_yielded: bool,
    pub just_claimed: bool,
    pub enabled: bool,
}

impl Default for Waive {
    fn default() -> Self {
        Self {
            concession: 0.0,
            max_concession: 100.0,
            yield_rate: 1.0,
            just_yielded: false,
            just_claimed: false,
            enabled: true,
        }
    }
}

impl Waive {
    pub fn yield_right(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_yielded = false;
        self.just_claimed = false;
        let prev = self.concession;
        self.concession = (self.concession + amount).clamp(0.0, self.max_concession);
        if self.concession >= self.max_concession && prev < self.max_concession {
            self.just_yielded = true;
        }
    }

    pub fn claim(&mut self, amount: f32) {
        if !self.enabled || self.concession <= 0.0 {
            return;
        }
        self.just_yielded = false;
        self.just_claimed = false;
        let prev = self.concession;
        self.concession = (self.concession - amount).max(0.0);
        if self.concession <= 0.0 && prev > 0.0 {
            self.just_claimed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.concession >= self.max_concession {
            return;
        }
        self.yield_right(self.yield_rate * dt);
    }

    pub fn is_yielded(&self) -> bool {
        self.enabled && self.concession >= self.max_concession
    }

    pub fn is_claimed(&self) -> bool {
        self.concession <= 0.0
    }

    pub fn concession_fraction(&self) -> f32 {
        if self.max_concession <= 0.0 {
            return 0.0;
        }
        self.concession / self.max_concession
    }

    pub fn effective_deference(&self, scale: f32) -> f32 {
        self.concession_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waive() -> Waive {
        Waive {
            concession: 0.0,
            max_concession: 100.0,
            yield_rate: 10.0,
            just_yielded: false,
            just_claimed: false,
            enabled: true,
        }
    }

    #[test]
    fn default_concession_zero() {
        let w = Waive::default();
        assert_eq!(w.concession, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Waive::default().enabled);
    }

    #[test]
    fn yield_right_increases_concession() {
        let mut w = waive();
        w.yield_right(30.0);
        assert_eq!(w.concession, 30.0);
    }

    #[test]
    fn yield_right_clamps_at_max() {
        let mut w = waive();
        w.yield_right(200.0);
        assert_eq!(w.concession, 100.0);
    }

    #[test]
    fn yield_right_no_op_when_disabled() {
        let mut w = waive();
        w.enabled = false;
        w.yield_right(50.0);
        assert_eq!(w.concession, 0.0);
    }

    #[test]
    fn yield_right_sets_just_yielded_at_max() {
        let mut w = waive();
        w.yield_right(100.0);
        assert!(w.just_yielded);
    }

    #[test]
    fn yield_right_no_just_yielded_if_already_max() {
        let mut w = waive();
        w.concession = 100.0;
        w.yield_right(1.0);
        assert!(!w.just_yielded);
    }

    #[test]
    fn claim_decreases_concession() {
        let mut w = waive();
        w.concession = 60.0;
        w.claim(20.0);
        assert_eq!(w.concession, 40.0);
    }

    #[test]
    fn claim_clamps_at_zero() {
        let mut w = waive();
        w.concession = 30.0;
        w.claim(200.0);
        assert_eq!(w.concession, 0.0);
    }

    #[test]
    fn claim_no_op_when_disabled() {
        let mut w = waive();
        w.concession = 50.0;
        w.enabled = false;
        w.claim(10.0);
        assert_eq!(w.concession, 50.0);
    }

    #[test]
    fn claim_no_op_when_already_claimed() {
        let mut w = waive();
        w.claim(10.0);
        assert_eq!(w.concession, 0.0);
    }

    #[test]
    fn claim_sets_just_claimed_at_zero() {
        let mut w = waive();
        w.concession = 10.0;
        w.claim(10.0);
        assert!(w.just_claimed);
    }

    #[test]
    fn claim_no_just_claimed_if_already_zero() {
        let mut w = waive();
        w.claim(1.0);
        assert!(!w.just_claimed);
    }

    #[test]
    fn tick_increases_concession() {
        let mut w = waive();
        w.tick(1.0);
        assert_eq!(w.concession, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = waive();
        w.tick(2.0);
        assert_eq!(w.concession, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = waive();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.concession, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_yielded() {
        let mut w = waive();
        w.concession = 100.0;
        w.tick(1.0);
        assert_eq!(w.concession, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = waive();
        w.yield_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.concession, 0.0);
    }

    #[test]
    fn is_yielded_true_at_max() {
        let mut w = waive();
        w.concession = 100.0;
        assert!(w.is_yielded());
    }

    #[test]
    fn is_yielded_false_below_max() {
        let mut w = waive();
        w.concession = 50.0;
        assert!(!w.is_yielded());
    }

    #[test]
    fn is_yielded_false_when_disabled() {
        let mut w = waive();
        w.concession = 100.0;
        w.enabled = false;
        assert!(!w.is_yielded());
    }

    #[test]
    fn is_claimed_true_at_zero() {
        let w = waive();
        assert!(w.is_claimed());
    }

    #[test]
    fn is_claimed_false_above_zero() {
        let mut w = waive();
        w.concession = 1.0;
        assert!(!w.is_claimed());
    }

    #[test]
    fn concession_fraction_zero_when_claimed() {
        let w = waive();
        assert_eq!(w.concession_fraction(), 0.0);
    }

    #[test]
    fn concession_fraction_one_at_max() {
        let mut w = waive();
        w.concession = 100.0;
        assert_eq!(w.concession_fraction(), 1.0);
    }

    #[test]
    fn concession_fraction_half_at_midpoint() {
        let mut w = waive();
        w.concession = 50.0;
        assert_eq!(w.concession_fraction(), 0.5);
    }

    #[test]
    fn concession_fraction_zero_when_max_zero() {
        let mut w = waive();
        w.max_concession = 0.0;
        assert_eq!(w.concession_fraction(), 0.0);
    }

    #[test]
    fn effective_deference_scales() {
        let mut w = waive();
        w.concession = 50.0;
        assert_eq!(w.effective_deference(2.0), 1.0);
    }

    #[test]
    fn effective_deference_zero_when_claimed() {
        let w = waive();
        assert_eq!(w.effective_deference(10.0), 0.0);
    }

    #[test]
    fn just_yielded_cleared_on_next_yield_right() {
        let mut w = waive();
        w.yield_right(100.0);
        assert!(w.just_yielded);
        w.yield_right(1.0);
        assert!(!w.just_yielded);
    }

    #[test]
    fn just_claimed_cleared_on_next_claim() {
        let mut w = waive();
        w.concession = 10.0;
        w.claim(10.0);
        assert!(w.just_claimed);
        w.concession = 10.0;
        w.claim(1.0);
        assert!(!w.just_claimed);
    }
}

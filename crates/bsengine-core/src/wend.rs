use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wend {
    pub journey: f32,
    pub max_journey: f32,
    pub travel_rate: f32,
    pub just_arrived: bool,
    pub just_lost: bool,
    pub enabled: bool,
}

impl Default for Wend {
    fn default() -> Self {
        Self {
            journey: 0.0,
            max_journey: 100.0,
            travel_rate: 1.0,
            just_arrived: false,
            just_lost: false,
            enabled: true,
        }
    }
}

impl Wend {
    pub fn traverse(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_arrived = false;
        self.just_lost = false;
        let prev = self.journey;
        self.journey = (self.journey + amount).clamp(0.0, self.max_journey);
        if self.journey >= self.max_journey && prev < self.max_journey {
            self.just_arrived = true;
        }
    }

    pub fn backtrack(&mut self, amount: f32) {
        if !self.enabled || self.journey <= 0.0 {
            return;
        }
        self.just_arrived = false;
        self.just_lost = false;
        let prev = self.journey;
        self.journey = (self.journey - amount).max(0.0);
        if self.journey <= 0.0 && prev > 0.0 {
            self.just_lost = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.journey >= self.max_journey {
            return;
        }
        self.traverse(self.travel_rate * dt);
    }

    pub fn is_arrived(&self) -> bool {
        self.enabled && self.journey >= self.max_journey
    }

    pub fn is_lost(&self) -> bool {
        self.journey <= 0.0
    }

    pub fn journey_fraction(&self) -> f32 {
        if self.max_journey <= 0.0 {
            return 0.0;
        }
        self.journey / self.max_journey
    }

    pub fn effective_progress(&self, scale: f32) -> f32 {
        self.journey_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wend() -> Wend {
        Wend {
            journey: 0.0,
            max_journey: 100.0,
            travel_rate: 10.0,
            just_arrived: false,
            just_lost: false,
            enabled: true,
        }
    }

    #[test]
    fn default_journey_zero() {
        let w = Wend::default();
        assert_eq!(w.journey, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wend::default().enabled);
    }

    #[test]
    fn traverse_increases_journey() {
        let mut w = wend();
        w.traverse(30.0);
        assert_eq!(w.journey, 30.0);
    }

    #[test]
    fn traverse_clamps_at_max() {
        let mut w = wend();
        w.traverse(200.0);
        assert_eq!(w.journey, 100.0);
    }

    #[test]
    fn traverse_no_op_when_disabled() {
        let mut w = wend();
        w.enabled = false;
        w.traverse(50.0);
        assert_eq!(w.journey, 0.0);
    }

    #[test]
    fn traverse_sets_just_arrived_at_max() {
        let mut w = wend();
        w.traverse(100.0);
        assert!(w.just_arrived);
    }

    #[test]
    fn traverse_no_just_arrived_if_already_max() {
        let mut w = wend();
        w.journey = 100.0;
        w.traverse(1.0);
        assert!(!w.just_arrived);
    }

    #[test]
    fn backtrack_decreases_journey() {
        let mut w = wend();
        w.journey = 60.0;
        w.backtrack(20.0);
        assert_eq!(w.journey, 40.0);
    }

    #[test]
    fn backtrack_clamps_at_zero() {
        let mut w = wend();
        w.journey = 30.0;
        w.backtrack(200.0);
        assert_eq!(w.journey, 0.0);
    }

    #[test]
    fn backtrack_no_op_when_disabled() {
        let mut w = wend();
        w.journey = 50.0;
        w.enabled = false;
        w.backtrack(10.0);
        assert_eq!(w.journey, 50.0);
    }

    #[test]
    fn backtrack_no_op_when_already_lost() {
        let mut w = wend();
        w.backtrack(10.0);
        assert_eq!(w.journey, 0.0);
    }

    #[test]
    fn backtrack_sets_just_lost_at_zero() {
        let mut w = wend();
        w.journey = 10.0;
        w.backtrack(10.0);
        assert!(w.just_lost);
    }

    #[test]
    fn backtrack_no_just_lost_if_already_zero() {
        let mut w = wend();
        w.backtrack(1.0);
        assert!(!w.just_lost);
    }

    #[test]
    fn tick_increases_journey() {
        let mut w = wend();
        w.tick(1.0);
        assert_eq!(w.journey, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wend();
        w.tick(2.0);
        assert_eq!(w.journey, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wend();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.journey, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_arrived() {
        let mut w = wend();
        w.journey = 100.0;
        w.tick(1.0);
        assert_eq!(w.journey, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wend();
        w.travel_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.journey, 0.0);
    }

    #[test]
    fn is_arrived_true_at_max() {
        let mut w = wend();
        w.journey = 100.0;
        assert!(w.is_arrived());
    }

    #[test]
    fn is_arrived_false_below_max() {
        let mut w = wend();
        w.journey = 50.0;
        assert!(!w.is_arrived());
    }

    #[test]
    fn is_arrived_false_when_disabled() {
        let mut w = wend();
        w.journey = 100.0;
        w.enabled = false;
        assert!(!w.is_arrived());
    }

    #[test]
    fn is_lost_true_at_zero() {
        let w = wend();
        assert!(w.is_lost());
    }

    #[test]
    fn is_lost_false_above_zero() {
        let mut w = wend();
        w.journey = 1.0;
        assert!(!w.is_lost());
    }

    #[test]
    fn journey_fraction_zero_when_lost() {
        let w = wend();
        assert_eq!(w.journey_fraction(), 0.0);
    }

    #[test]
    fn journey_fraction_one_at_max() {
        let mut w = wend();
        w.journey = 100.0;
        assert_eq!(w.journey_fraction(), 1.0);
    }

    #[test]
    fn journey_fraction_half_at_midpoint() {
        let mut w = wend();
        w.journey = 50.0;
        assert_eq!(w.journey_fraction(), 0.5);
    }

    #[test]
    fn journey_fraction_zero_when_max_zero() {
        let mut w = wend();
        w.max_journey = 0.0;
        assert_eq!(w.journey_fraction(), 0.0);
    }

    #[test]
    fn effective_progress_scales() {
        let mut w = wend();
        w.journey = 50.0;
        assert_eq!(w.effective_progress(2.0), 1.0);
    }

    #[test]
    fn effective_progress_zero_when_lost() {
        let w = wend();
        assert_eq!(w.effective_progress(10.0), 0.0);
    }

    #[test]
    fn just_arrived_cleared_on_next_traverse() {
        let mut w = wend();
        w.traverse(100.0);
        assert!(w.just_arrived);
        w.traverse(1.0);
        assert!(!w.just_arrived);
    }

    #[test]
    fn just_lost_cleared_on_next_backtrack() {
        let mut w = wend();
        w.journey = 10.0;
        w.backtrack(10.0);
        assert!(w.just_lost);
        w.journey = 10.0;
        w.backtrack(1.0);
        assert!(!w.just_lost);
    }
}

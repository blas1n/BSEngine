use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Woke {
    pub awareness: f32,
    pub max_awareness: f32,
    pub rouse_rate: f32,
    pub just_roused: bool,
    pub just_dormant: bool,
    pub enabled: bool,
}

impl Default for Woke {
    fn default() -> Self {
        Self {
            awareness: 0.0,
            max_awareness: 100.0,
            rouse_rate: 1.0,
            just_roused: false,
            just_dormant: false,
            enabled: true,
        }
    }
}

impl Woke {
    pub fn rouse(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_roused = false;
        self.just_dormant = false;
        let prev = self.awareness;
        self.awareness = (self.awareness + amount).clamp(0.0, self.max_awareness);
        if self.awareness >= self.max_awareness && prev < self.max_awareness {
            self.just_roused = true;
        }
    }

    pub fn dull(&mut self, amount: f32) {
        if !self.enabled || self.awareness <= 0.0 {
            return;
        }
        self.just_roused = false;
        self.just_dormant = false;
        let prev = self.awareness;
        self.awareness = (self.awareness - amount).max(0.0);
        if self.awareness <= 0.0 && prev > 0.0 {
            self.just_dormant = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.awareness >= self.max_awareness {
            return;
        }
        self.rouse(self.rouse_rate * dt);
    }

    pub fn is_roused(&self) -> bool {
        self.enabled && self.awareness >= self.max_awareness
    }

    pub fn is_dormant(&self) -> bool {
        self.awareness <= 0.0
    }

    pub fn awareness_fraction(&self) -> f32 {
        if self.max_awareness <= 0.0 {
            return 0.0;
        }
        self.awareness / self.max_awareness
    }

    pub fn effective_alertness(&self, scale: f32) -> f32 {
        self.awareness_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn woke() -> Woke {
        Woke {
            awareness: 0.0,
            max_awareness: 100.0,
            rouse_rate: 10.0,
            just_roused: false,
            just_dormant: false,
            enabled: true,
        }
    }

    #[test]
    fn default_awareness_zero() {
        let w = Woke::default();
        assert_eq!(w.awareness, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Woke::default().enabled);
    }

    #[test]
    fn rouse_increases_awareness() {
        let mut w = woke();
        w.rouse(30.0);
        assert_eq!(w.awareness, 30.0);
    }

    #[test]
    fn rouse_clamps_at_max() {
        let mut w = woke();
        w.rouse(200.0);
        assert_eq!(w.awareness, 100.0);
    }

    #[test]
    fn rouse_no_op_when_disabled() {
        let mut w = woke();
        w.enabled = false;
        w.rouse(50.0);
        assert_eq!(w.awareness, 0.0);
    }

    #[test]
    fn rouse_sets_just_roused_at_max() {
        let mut w = woke();
        w.rouse(100.0);
        assert!(w.just_roused);
    }

    #[test]
    fn rouse_no_just_roused_if_already_max() {
        let mut w = woke();
        w.awareness = 100.0;
        w.rouse(1.0);
        assert!(!w.just_roused);
    }

    #[test]
    fn dull_decreases_awareness() {
        let mut w = woke();
        w.awareness = 60.0;
        w.dull(20.0);
        assert_eq!(w.awareness, 40.0);
    }

    #[test]
    fn dull_clamps_at_zero() {
        let mut w = woke();
        w.awareness = 30.0;
        w.dull(200.0);
        assert_eq!(w.awareness, 0.0);
    }

    #[test]
    fn dull_no_op_when_disabled() {
        let mut w = woke();
        w.awareness = 50.0;
        w.enabled = false;
        w.dull(10.0);
        assert_eq!(w.awareness, 50.0);
    }

    #[test]
    fn dull_no_op_when_already_dormant() {
        let mut w = woke();
        w.dull(10.0);
        assert_eq!(w.awareness, 0.0);
    }

    #[test]
    fn dull_sets_just_dormant_at_zero() {
        let mut w = woke();
        w.awareness = 10.0;
        w.dull(10.0);
        assert!(w.just_dormant);
    }

    #[test]
    fn dull_no_just_dormant_if_already_zero() {
        let mut w = woke();
        w.dull(1.0);
        assert!(!w.just_dormant);
    }

    #[test]
    fn tick_increases_awareness() {
        let mut w = woke();
        w.tick(1.0);
        assert_eq!(w.awareness, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = woke();
        w.tick(2.0);
        assert_eq!(w.awareness, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = woke();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.awareness, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_roused() {
        let mut w = woke();
        w.awareness = 100.0;
        w.tick(1.0);
        assert_eq!(w.awareness, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = woke();
        w.rouse_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.awareness, 0.0);
    }

    #[test]
    fn is_roused_true_at_max() {
        let mut w = woke();
        w.awareness = 100.0;
        assert!(w.is_roused());
    }

    #[test]
    fn is_roused_false_below_max() {
        let mut w = woke();
        w.awareness = 50.0;
        assert!(!w.is_roused());
    }

    #[test]
    fn is_roused_false_when_disabled() {
        let mut w = woke();
        w.awareness = 100.0;
        w.enabled = false;
        assert!(!w.is_roused());
    }

    #[test]
    fn is_dormant_true_at_zero() {
        let w = woke();
        assert!(w.is_dormant());
    }

    #[test]
    fn is_dormant_false_above_zero() {
        let mut w = woke();
        w.awareness = 1.0;
        assert!(!w.is_dormant());
    }

    #[test]
    fn awareness_fraction_zero_when_dormant() {
        let w = woke();
        assert_eq!(w.awareness_fraction(), 0.0);
    }

    #[test]
    fn awareness_fraction_one_at_max() {
        let mut w = woke();
        w.awareness = 100.0;
        assert_eq!(w.awareness_fraction(), 1.0);
    }

    #[test]
    fn awareness_fraction_half_at_midpoint() {
        let mut w = woke();
        w.awareness = 50.0;
        assert_eq!(w.awareness_fraction(), 0.5);
    }

    #[test]
    fn awareness_fraction_zero_when_max_zero() {
        let mut w = woke();
        w.max_awareness = 0.0;
        assert_eq!(w.awareness_fraction(), 0.0);
    }

    #[test]
    fn effective_alertness_scales() {
        let mut w = woke();
        w.awareness = 50.0;
        assert_eq!(w.effective_alertness(2.0), 1.0);
    }

    #[test]
    fn effective_alertness_zero_when_dormant() {
        let w = woke();
        assert_eq!(w.effective_alertness(10.0), 0.0);
    }

    #[test]
    fn just_roused_cleared_on_next_rouse() {
        let mut w = woke();
        w.rouse(100.0);
        assert!(w.just_roused);
        w.rouse(1.0);
        assert!(!w.just_roused);
    }

    #[test]
    fn just_dormant_cleared_on_next_dull() {
        let mut w = woke();
        w.awareness = 10.0;
        w.dull(10.0);
        assert!(w.just_dormant);
        w.awareness = 10.0;
        w.dull(1.0);
        assert!(!w.just_dormant);
    }
}

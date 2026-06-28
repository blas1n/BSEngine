use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wonky {
    pub instability: f32,
    pub max_instability: f32,
    pub wobble_rate: f32,
    pub just_toppled: bool,
    pub just_steadied: bool,
    pub enabled: bool,
}

impl Default for Wonky {
    fn default() -> Self {
        Self {
            instability: 0.0,
            max_instability: 100.0,
            wobble_rate: 1.0,
            just_toppled: false,
            just_steadied: false,
            enabled: true,
        }
    }
}

impl Wonky {
    pub fn wobble(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_toppled = false;
        self.just_steadied = false;
        let prev = self.instability;
        self.instability = (self.instability + amount).clamp(0.0, self.max_instability);
        if self.instability >= self.max_instability && prev < self.max_instability {
            self.just_toppled = true;
        }
    }

    pub fn steady(&mut self, amount: f32) {
        if !self.enabled || self.instability <= 0.0 {
            return;
        }
        self.just_toppled = false;
        self.just_steadied = false;
        let prev = self.instability;
        self.instability = (self.instability - amount).max(0.0);
        if self.instability <= 0.0 && prev > 0.0 {
            self.just_steadied = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.instability >= self.max_instability {
            return;
        }
        self.wobble(self.wobble_rate * dt);
    }

    pub fn is_toppled(&self) -> bool {
        self.enabled && self.instability >= self.max_instability
    }

    pub fn is_steadied(&self) -> bool {
        self.instability <= 0.0
    }

    pub fn instability_fraction(&self) -> f32 {
        if self.max_instability <= 0.0 {
            return 0.0;
        }
        self.instability / self.max_instability
    }

    pub fn effective_wobble(&self, scale: f32) -> f32 {
        self.instability_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wonky() -> Wonky {
        Wonky {
            instability: 0.0,
            max_instability: 100.0,
            wobble_rate: 10.0,
            just_toppled: false,
            just_steadied: false,
            enabled: true,
        }
    }

    #[test]
    fn default_instability_zero() {
        let w = Wonky::default();
        assert_eq!(w.instability, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wonky::default().enabled);
    }

    #[test]
    fn wobble_increases_instability() {
        let mut w = wonky();
        w.wobble(30.0);
        assert_eq!(w.instability, 30.0);
    }

    #[test]
    fn wobble_clamps_at_max() {
        let mut w = wonky();
        w.wobble(200.0);
        assert_eq!(w.instability, 100.0);
    }

    #[test]
    fn wobble_no_op_when_disabled() {
        let mut w = wonky();
        w.enabled = false;
        w.wobble(50.0);
        assert_eq!(w.instability, 0.0);
    }

    #[test]
    fn wobble_sets_just_toppled_at_max() {
        let mut w = wonky();
        w.wobble(100.0);
        assert!(w.just_toppled);
    }

    #[test]
    fn wobble_no_just_toppled_if_already_max() {
        let mut w = wonky();
        w.instability = 100.0;
        w.wobble(1.0);
        assert!(!w.just_toppled);
    }

    #[test]
    fn steady_decreases_instability() {
        let mut w = wonky();
        w.instability = 60.0;
        w.steady(20.0);
        assert_eq!(w.instability, 40.0);
    }

    #[test]
    fn steady_clamps_at_zero() {
        let mut w = wonky();
        w.instability = 30.0;
        w.steady(200.0);
        assert_eq!(w.instability, 0.0);
    }

    #[test]
    fn steady_no_op_when_disabled() {
        let mut w = wonky();
        w.instability = 50.0;
        w.enabled = false;
        w.steady(10.0);
        assert_eq!(w.instability, 50.0);
    }

    #[test]
    fn steady_no_op_when_already_steadied() {
        let mut w = wonky();
        w.steady(10.0);
        assert_eq!(w.instability, 0.0);
    }

    #[test]
    fn steady_sets_just_steadied_at_zero() {
        let mut w = wonky();
        w.instability = 10.0;
        w.steady(10.0);
        assert!(w.just_steadied);
    }

    #[test]
    fn steady_no_just_steadied_if_already_zero() {
        let mut w = wonky();
        w.steady(1.0);
        assert!(!w.just_steadied);
    }

    #[test]
    fn tick_increases_instability() {
        let mut w = wonky();
        w.tick(1.0);
        assert_eq!(w.instability, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wonky();
        w.tick(2.0);
        assert_eq!(w.instability, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wonky();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.instability, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_toppled() {
        let mut w = wonky();
        w.instability = 100.0;
        w.tick(1.0);
        assert_eq!(w.instability, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wonky();
        w.wobble_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.instability, 0.0);
    }

    #[test]
    fn is_toppled_true_at_max() {
        let mut w = wonky();
        w.instability = 100.0;
        assert!(w.is_toppled());
    }

    #[test]
    fn is_toppled_false_below_max() {
        let mut w = wonky();
        w.instability = 50.0;
        assert!(!w.is_toppled());
    }

    #[test]
    fn is_toppled_false_when_disabled() {
        let mut w = wonky();
        w.instability = 100.0;
        w.enabled = false;
        assert!(!w.is_toppled());
    }

    #[test]
    fn is_steadied_true_at_zero() {
        let w = wonky();
        assert!(w.is_steadied());
    }

    #[test]
    fn is_steadied_false_above_zero() {
        let mut w = wonky();
        w.instability = 1.0;
        assert!(!w.is_steadied());
    }

    #[test]
    fn instability_fraction_zero_when_steadied() {
        let w = wonky();
        assert_eq!(w.instability_fraction(), 0.0);
    }

    #[test]
    fn instability_fraction_one_at_max() {
        let mut w = wonky();
        w.instability = 100.0;
        assert_eq!(w.instability_fraction(), 1.0);
    }

    #[test]
    fn instability_fraction_half_at_midpoint() {
        let mut w = wonky();
        w.instability = 50.0;
        assert_eq!(w.instability_fraction(), 0.5);
    }

    #[test]
    fn instability_fraction_zero_when_max_zero() {
        let mut w = wonky();
        w.max_instability = 0.0;
        assert_eq!(w.instability_fraction(), 0.0);
    }

    #[test]
    fn effective_wobble_scales() {
        let mut w = wonky();
        w.instability = 50.0;
        assert_eq!(w.effective_wobble(2.0), 1.0);
    }

    #[test]
    fn effective_wobble_zero_when_steadied() {
        let w = wonky();
        assert_eq!(w.effective_wobble(10.0), 0.0);
    }

    #[test]
    fn just_toppled_cleared_on_next_wobble() {
        let mut w = wonky();
        w.wobble(100.0);
        assert!(w.just_toppled);
        w.wobble(1.0);
        assert!(!w.just_toppled);
    }

    #[test]
    fn just_steadied_cleared_on_next_steady() {
        let mut w = wonky();
        w.instability = 10.0;
        w.steady(10.0);
        assert!(w.just_steadied);
        w.instability = 10.0;
        w.steady(1.0);
        assert!(!w.just_steadied);
    }
}

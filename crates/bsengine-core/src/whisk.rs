use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Whisk {
    pub agitation: f32,
    pub max_agitation: f32,
    pub beat_rate: f32,
    pub just_frothy: bool,
    pub just_settled: bool,
    pub enabled: bool,
}

impl Default for Whisk {
    fn default() -> Self {
        Self {
            agitation: 0.0,
            max_agitation: 100.0,
            beat_rate: 1.0,
            just_frothy: false,
            just_settled: false,
            enabled: true,
        }
    }
}

impl Whisk {
    pub fn beat(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_frothy = false;
        self.just_settled = false;
        let prev = self.agitation;
        self.agitation = (self.agitation + amount).clamp(0.0, self.max_agitation);
        if self.agitation >= self.max_agitation && prev < self.max_agitation {
            self.just_frothy = true;
        }
    }

    pub fn settle(&mut self, amount: f32) {
        if !self.enabled || self.agitation <= 0.0 {
            return;
        }
        self.just_frothy = false;
        self.just_settled = false;
        let prev = self.agitation;
        self.agitation = (self.agitation - amount).max(0.0);
        if self.agitation <= 0.0 && prev > 0.0 {
            self.just_settled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.agitation >= self.max_agitation {
            return;
        }
        self.beat(self.beat_rate * dt);
    }

    pub fn is_frothy(&self) -> bool {
        self.enabled && self.agitation >= self.max_agitation
    }

    pub fn is_settled(&self) -> bool {
        self.agitation <= 0.0
    }

    pub fn agitation_fraction(&self) -> f32 {
        if self.max_agitation <= 0.0 {
            return 0.0;
        }
        self.agitation / self.max_agitation
    }

    pub fn effective_mix(&self, scale: f32) -> f32 {
        self.agitation_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn whisk() -> Whisk {
        Whisk {
            agitation: 0.0,
            max_agitation: 100.0,
            beat_rate: 10.0,
            just_frothy: false,
            just_settled: false,
            enabled: true,
        }
    }

    #[test]
    fn default_agitation_zero() {
        let w = Whisk::default();
        assert_eq!(w.agitation, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Whisk::default().enabled);
    }

    #[test]
    fn beat_increases_agitation() {
        let mut w = whisk();
        w.beat(30.0);
        assert_eq!(w.agitation, 30.0);
    }

    #[test]
    fn beat_clamps_at_max() {
        let mut w = whisk();
        w.beat(200.0);
        assert_eq!(w.agitation, 100.0);
    }

    #[test]
    fn beat_no_op_when_disabled() {
        let mut w = whisk();
        w.enabled = false;
        w.beat(50.0);
        assert_eq!(w.agitation, 0.0);
    }

    #[test]
    fn beat_sets_just_frothy_at_max() {
        let mut w = whisk();
        w.beat(100.0);
        assert!(w.just_frothy);
    }

    #[test]
    fn beat_no_just_frothy_if_already_max() {
        let mut w = whisk();
        w.agitation = 100.0;
        w.beat(1.0);
        assert!(!w.just_frothy);
    }

    #[test]
    fn settle_decreases_agitation() {
        let mut w = whisk();
        w.agitation = 60.0;
        w.settle(20.0);
        assert_eq!(w.agitation, 40.0);
    }

    #[test]
    fn settle_clamps_at_zero() {
        let mut w = whisk();
        w.agitation = 30.0;
        w.settle(200.0);
        assert_eq!(w.agitation, 0.0);
    }

    #[test]
    fn settle_no_op_when_disabled() {
        let mut w = whisk();
        w.agitation = 50.0;
        w.enabled = false;
        w.settle(10.0);
        assert_eq!(w.agitation, 50.0);
    }

    #[test]
    fn settle_no_op_when_already_settled() {
        let mut w = whisk();
        w.settle(10.0);
        assert_eq!(w.agitation, 0.0);
    }

    #[test]
    fn settle_sets_just_settled_at_zero() {
        let mut w = whisk();
        w.agitation = 10.0;
        w.settle(10.0);
        assert!(w.just_settled);
    }

    #[test]
    fn settle_no_just_settled_if_already_zero() {
        let mut w = whisk();
        w.settle(1.0);
        assert!(!w.just_settled);
    }

    #[test]
    fn tick_increases_agitation() {
        let mut w = whisk();
        w.tick(1.0);
        assert_eq!(w.agitation, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = whisk();
        w.tick(2.0);
        assert_eq!(w.agitation, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = whisk();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.agitation, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_frothy() {
        let mut w = whisk();
        w.agitation = 100.0;
        w.tick(1.0);
        assert_eq!(w.agitation, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = whisk();
        w.beat_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.agitation, 0.0);
    }

    #[test]
    fn is_frothy_true_at_max() {
        let mut w = whisk();
        w.agitation = 100.0;
        assert!(w.is_frothy());
    }

    #[test]
    fn is_frothy_false_below_max() {
        let mut w = whisk();
        w.agitation = 50.0;
        assert!(!w.is_frothy());
    }

    #[test]
    fn is_frothy_false_when_disabled() {
        let mut w = whisk();
        w.agitation = 100.0;
        w.enabled = false;
        assert!(!w.is_frothy());
    }

    #[test]
    fn is_settled_true_at_zero() {
        let w = whisk();
        assert!(w.is_settled());
    }

    #[test]
    fn is_settled_false_above_zero() {
        let mut w = whisk();
        w.agitation = 1.0;
        assert!(!w.is_settled());
    }

    #[test]
    fn agitation_fraction_zero_when_settled() {
        let w = whisk();
        assert_eq!(w.agitation_fraction(), 0.0);
    }

    #[test]
    fn agitation_fraction_one_at_max() {
        let mut w = whisk();
        w.agitation = 100.0;
        assert_eq!(w.agitation_fraction(), 1.0);
    }

    #[test]
    fn agitation_fraction_half_at_midpoint() {
        let mut w = whisk();
        w.agitation = 50.0;
        assert_eq!(w.agitation_fraction(), 0.5);
    }

    #[test]
    fn agitation_fraction_zero_when_max_zero() {
        let mut w = whisk();
        w.max_agitation = 0.0;
        assert_eq!(w.agitation_fraction(), 0.0);
    }

    #[test]
    fn effective_mix_scales() {
        let mut w = whisk();
        w.agitation = 50.0;
        assert_eq!(w.effective_mix(2.0), 1.0);
    }

    #[test]
    fn effective_mix_zero_when_settled() {
        let w = whisk();
        assert_eq!(w.effective_mix(10.0), 0.0);
    }

    #[test]
    fn just_frothy_cleared_on_next_beat() {
        let mut w = whisk();
        w.beat(100.0);
        assert!(w.just_frothy);
        w.beat(1.0);
        assert!(!w.just_frothy);
    }

    #[test]
    fn just_settled_cleared_on_next_settle() {
        let mut w = whisk();
        w.agitation = 10.0;
        w.settle(10.0);
        assert!(w.just_settled);
        w.agitation = 10.0;
        w.settle(1.0);
        assert!(!w.just_settled);
    }
}

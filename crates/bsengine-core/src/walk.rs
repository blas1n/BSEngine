use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Walk {
    pub stride: f32,
    pub max_stride: f32,
    pub pace_rate: f32,
    pub just_striding: bool,
    pub just_halted: bool,
    pub enabled: bool,
}

impl Default for Walk {
    fn default() -> Self {
        Self {
            stride: 0.0,
            max_stride: 100.0,
            pace_rate: 1.0,
            just_striding: false,
            just_halted: false,
            enabled: true,
        }
    }
}

impl Walk {
    pub fn step(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_striding = false;
        self.just_halted = false;
        let prev = self.stride;
        self.stride = (self.stride + amount).clamp(0.0, self.max_stride);
        if self.stride >= self.max_stride && prev < self.max_stride {
            self.just_striding = true;
        }
    }

    pub fn rest(&mut self, amount: f32) {
        if !self.enabled || self.stride <= 0.0 {
            return;
        }
        self.just_striding = false;
        self.just_halted = false;
        let prev = self.stride;
        self.stride = (self.stride - amount).max(0.0);
        if self.stride <= 0.0 && prev > 0.0 {
            self.just_halted = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.stride >= self.max_stride {
            return;
        }
        self.step(self.pace_rate * dt);
    }

    pub fn is_striding(&self) -> bool {
        self.enabled && self.stride >= self.max_stride
    }

    pub fn is_halted(&self) -> bool {
        self.stride <= 0.0
    }

    pub fn stride_fraction(&self) -> f32 {
        if self.max_stride <= 0.0 {
            return 0.0;
        }
        self.stride / self.max_stride
    }

    pub fn effective_pace(&self, scale: f32) -> f32 {
        self.stride_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk() -> Walk {
        Walk {
            stride: 0.0,
            max_stride: 100.0,
            pace_rate: 10.0,
            just_striding: false,
            just_halted: false,
            enabled: true,
        }
    }

    #[test]
    fn default_stride_zero() {
        let w = Walk::default();
        assert_eq!(w.stride, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Walk::default().enabled);
    }

    #[test]
    fn step_increases_stride() {
        let mut w = walk();
        w.step(30.0);
        assert_eq!(w.stride, 30.0);
    }

    #[test]
    fn step_clamps_at_max() {
        let mut w = walk();
        w.step(200.0);
        assert_eq!(w.stride, 100.0);
    }

    #[test]
    fn step_no_op_when_disabled() {
        let mut w = walk();
        w.enabled = false;
        w.step(50.0);
        assert_eq!(w.stride, 0.0);
    }

    #[test]
    fn step_sets_just_striding_at_max() {
        let mut w = walk();
        w.step(100.0);
        assert!(w.just_striding);
    }

    #[test]
    fn step_no_just_striding_if_already_max() {
        let mut w = walk();
        w.stride = 100.0;
        w.step(1.0);
        assert!(!w.just_striding);
    }

    #[test]
    fn rest_decreases_stride() {
        let mut w = walk();
        w.stride = 60.0;
        w.rest(20.0);
        assert_eq!(w.stride, 40.0);
    }

    #[test]
    fn rest_clamps_at_zero() {
        let mut w = walk();
        w.stride = 30.0;
        w.rest(200.0);
        assert_eq!(w.stride, 0.0);
    }

    #[test]
    fn rest_no_op_when_disabled() {
        let mut w = walk();
        w.stride = 50.0;
        w.enabled = false;
        w.rest(10.0);
        assert_eq!(w.stride, 50.0);
    }

    #[test]
    fn rest_no_op_when_already_halted() {
        let mut w = walk();
        w.rest(10.0);
        assert_eq!(w.stride, 0.0);
    }

    #[test]
    fn rest_sets_just_halted_at_zero() {
        let mut w = walk();
        w.stride = 10.0;
        w.rest(10.0);
        assert!(w.just_halted);
    }

    #[test]
    fn rest_no_just_halted_if_already_zero() {
        let mut w = walk();
        w.rest(1.0);
        assert!(!w.just_halted);
    }

    #[test]
    fn tick_increases_stride() {
        let mut w = walk();
        w.tick(1.0);
        assert_eq!(w.stride, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = walk();
        w.tick(2.0);
        assert_eq!(w.stride, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = walk();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.stride, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_striding() {
        let mut w = walk();
        w.stride = 100.0;
        w.tick(1.0);
        assert_eq!(w.stride, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = walk();
        w.pace_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.stride, 0.0);
    }

    #[test]
    fn is_striding_true_at_max() {
        let mut w = walk();
        w.stride = 100.0;
        assert!(w.is_striding());
    }

    #[test]
    fn is_striding_false_below_max() {
        let mut w = walk();
        w.stride = 50.0;
        assert!(!w.is_striding());
    }

    #[test]
    fn is_striding_false_when_disabled() {
        let mut w = walk();
        w.stride = 100.0;
        w.enabled = false;
        assert!(!w.is_striding());
    }

    #[test]
    fn is_halted_true_at_zero() {
        let w = walk();
        assert!(w.is_halted());
    }

    #[test]
    fn is_halted_false_above_zero() {
        let mut w = walk();
        w.stride = 1.0;
        assert!(!w.is_halted());
    }

    #[test]
    fn stride_fraction_zero_when_halted() {
        let w = walk();
        assert_eq!(w.stride_fraction(), 0.0);
    }

    #[test]
    fn stride_fraction_one_at_max() {
        let mut w = walk();
        w.stride = 100.0;
        assert_eq!(w.stride_fraction(), 1.0);
    }

    #[test]
    fn stride_fraction_half_at_midpoint() {
        let mut w = walk();
        w.stride = 50.0;
        assert_eq!(w.stride_fraction(), 0.5);
    }

    #[test]
    fn stride_fraction_zero_when_max_zero() {
        let mut w = walk();
        w.max_stride = 0.0;
        assert_eq!(w.stride_fraction(), 0.0);
    }

    #[test]
    fn effective_pace_scales() {
        let mut w = walk();
        w.stride = 50.0;
        assert_eq!(w.effective_pace(2.0), 1.0);
    }

    #[test]
    fn effective_pace_zero_when_halted() {
        let w = walk();
        assert_eq!(w.effective_pace(10.0), 0.0);
    }

    #[test]
    fn just_striding_cleared_on_next_step() {
        let mut w = walk();
        w.step(100.0);
        assert!(w.just_striding);
        w.step(1.0);
        assert!(!w.just_striding);
    }

    #[test]
    fn just_halted_cleared_on_next_rest() {
        let mut w = walk();
        w.stride = 10.0;
        w.rest(10.0);
        assert!(w.just_halted);
        w.stride = 10.0;
        w.rest(1.0);
        assert!(!w.just_halted);
    }
}

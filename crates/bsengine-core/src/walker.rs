use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Walker {
    pub traversal: f32,
    pub max_traversal: f32,
    pub stride_rate: f32,
    pub just_peaked: bool,
    pub just_halted: bool,
    pub enabled: bool,
}

impl Default for Walker {
    fn default() -> Self {
        Self {
            traversal: 0.0,
            max_traversal: 100.0,
            stride_rate: 1.0,
            just_peaked: false,
            just_halted: false,
            enabled: true,
        }
    }
}

impl Walker {
    pub fn stride(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_peaked = false;
        self.just_halted = false;
        let prev = self.traversal;
        self.traversal = (self.traversal + amount).clamp(0.0, self.max_traversal);
        if self.traversal >= self.max_traversal && prev < self.max_traversal {
            self.just_peaked = true;
        }
    }

    pub fn halt(&mut self, amount: f32) {
        if !self.enabled || self.traversal <= 0.0 {
            return;
        }
        self.just_peaked = false;
        self.just_halted = false;
        let prev = self.traversal;
        self.traversal = (self.traversal - amount).max(0.0);
        if self.traversal <= 0.0 && prev > 0.0 {
            self.just_halted = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.traversal >= self.max_traversal {
            return;
        }
        self.stride(self.stride_rate * dt);
    }

    pub fn is_peaked(&self) -> bool {
        self.enabled && self.traversal >= self.max_traversal
    }

    pub fn is_halted(&self) -> bool {
        self.traversal <= 0.0
    }

    pub fn traversal_fraction(&self) -> f32 {
        if self.max_traversal <= 0.0 {
            return 0.0;
        }
        self.traversal / self.max_traversal
    }

    pub fn effective_pace(&self, scale: f32) -> f32 {
        self.traversal_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walker() -> Walker {
        Walker {
            traversal: 0.0,
            max_traversal: 100.0,
            stride_rate: 10.0,
            just_peaked: false,
            just_halted: false,
            enabled: true,
        }
    }

    #[test]
    fn default_traversal_zero() {
        let w = Walker::default();
        assert_eq!(w.traversal, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Walker::default().enabled);
    }

    #[test]
    fn stride_increases_traversal() {
        let mut w = walker();
        w.stride(30.0);
        assert_eq!(w.traversal, 30.0);
    }

    #[test]
    fn stride_clamps_at_max() {
        let mut w = walker();
        w.stride(200.0);
        assert_eq!(w.traversal, 100.0);
    }

    #[test]
    fn stride_no_op_when_disabled() {
        let mut w = walker();
        w.enabled = false;
        w.stride(50.0);
        assert_eq!(w.traversal, 0.0);
    }

    #[test]
    fn stride_sets_just_peaked_at_max() {
        let mut w = walker();
        w.stride(100.0);
        assert!(w.just_peaked);
    }

    #[test]
    fn stride_no_just_peaked_if_already_max() {
        let mut w = walker();
        w.traversal = 100.0;
        w.stride(1.0);
        assert!(!w.just_peaked);
    }

    #[test]
    fn halt_decreases_traversal() {
        let mut w = walker();
        w.traversal = 60.0;
        w.halt(20.0);
        assert_eq!(w.traversal, 40.0);
    }

    #[test]
    fn halt_clamps_at_zero() {
        let mut w = walker();
        w.traversal = 30.0;
        w.halt(200.0);
        assert_eq!(w.traversal, 0.0);
    }

    #[test]
    fn halt_no_op_when_disabled() {
        let mut w = walker();
        w.traversal = 50.0;
        w.enabled = false;
        w.halt(10.0);
        assert_eq!(w.traversal, 50.0);
    }

    #[test]
    fn halt_no_op_when_already_halted() {
        let mut w = walker();
        w.halt(10.0);
        assert_eq!(w.traversal, 0.0);
    }

    #[test]
    fn halt_sets_just_halted_at_zero() {
        let mut w = walker();
        w.traversal = 10.0;
        w.halt(10.0);
        assert!(w.just_halted);
    }

    #[test]
    fn halt_no_just_halted_if_already_zero() {
        let mut w = walker();
        w.halt(1.0);
        assert!(!w.just_halted);
    }

    #[test]
    fn tick_increases_traversal() {
        let mut w = walker();
        w.tick(1.0);
        assert_eq!(w.traversal, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = walker();
        w.tick(2.0);
        assert_eq!(w.traversal, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = walker();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.traversal, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_peaked() {
        let mut w = walker();
        w.traversal = 100.0;
        w.tick(1.0);
        assert_eq!(w.traversal, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = walker();
        w.stride_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.traversal, 0.0);
    }

    #[test]
    fn is_peaked_true_at_max() {
        let mut w = walker();
        w.traversal = 100.0;
        assert!(w.is_peaked());
    }

    #[test]
    fn is_peaked_false_below_max() {
        let mut w = walker();
        w.traversal = 50.0;
        assert!(!w.is_peaked());
    }

    #[test]
    fn is_peaked_false_when_disabled() {
        let mut w = walker();
        w.traversal = 100.0;
        w.enabled = false;
        assert!(!w.is_peaked());
    }

    #[test]
    fn is_halted_true_at_zero() {
        let w = walker();
        assert!(w.is_halted());
    }

    #[test]
    fn is_halted_false_above_zero() {
        let mut w = walker();
        w.traversal = 1.0;
        assert!(!w.is_halted());
    }

    #[test]
    fn traversal_fraction_zero_when_halted() {
        let w = walker();
        assert_eq!(w.traversal_fraction(), 0.0);
    }

    #[test]
    fn traversal_fraction_one_at_max() {
        let mut w = walker();
        w.traversal = 100.0;
        assert_eq!(w.traversal_fraction(), 1.0);
    }

    #[test]
    fn traversal_fraction_half_at_midpoint() {
        let mut w = walker();
        w.traversal = 50.0;
        assert_eq!(w.traversal_fraction(), 0.5);
    }

    #[test]
    fn traversal_fraction_zero_when_max_zero() {
        let mut w = walker();
        w.max_traversal = 0.0;
        assert_eq!(w.traversal_fraction(), 0.0);
    }

    #[test]
    fn effective_pace_scales() {
        let mut w = walker();
        w.traversal = 50.0;
        assert_eq!(w.effective_pace(2.0), 1.0);
    }

    #[test]
    fn effective_pace_zero_when_halted() {
        let w = walker();
        assert_eq!(w.effective_pace(10.0), 0.0);
    }

    #[test]
    fn just_peaked_cleared_on_next_stride() {
        let mut w = walker();
        w.stride(100.0);
        assert!(w.just_peaked);
        w.stride(1.0);
        assert!(!w.just_peaked);
    }

    #[test]
    fn just_halted_cleared_on_next_halt() {
        let mut w = walker();
        w.traversal = 10.0;
        w.halt(10.0);
        assert!(w.just_halted);
        w.traversal = 10.0;
        w.halt(1.0);
        assert!(!w.just_halted);
    }
}

use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wader {
    pub depth: f32,
    pub max_depth: f32,
    pub wade_rate: f32,
    pub just_submerged: bool,
    pub just_emerged: bool,
    pub enabled: bool,
}

impl Default for Wader {
    fn default() -> Self {
        Self {
            depth: 0.0,
            max_depth: 100.0,
            wade_rate: 1.0,
            just_submerged: false,
            just_emerged: false,
            enabled: true,
        }
    }
}

impl Wader {
    pub fn wade(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_submerged = false;
        self.just_emerged = false;
        let prev = self.depth;
        self.depth = (self.depth + amount).clamp(0.0, self.max_depth);
        if self.depth >= self.max_depth && prev < self.max_depth {
            self.just_submerged = true;
        }
    }

    pub fn emerge(&mut self, amount: f32) {
        if !self.enabled || self.depth <= 0.0 {
            return;
        }
        self.just_submerged = false;
        self.just_emerged = false;
        let prev = self.depth;
        self.depth = (self.depth - amount).max(0.0);
        if self.depth <= 0.0 && prev > 0.0 {
            self.just_emerged = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.depth >= self.max_depth {
            return;
        }
        self.wade(self.wade_rate * dt);
    }

    pub fn is_submerged(&self) -> bool {
        self.enabled && self.depth >= self.max_depth
    }

    pub fn is_emerged(&self) -> bool {
        self.depth <= 0.0
    }

    pub fn depth_fraction(&self) -> f32 {
        if self.max_depth <= 0.0 {
            return 0.0;
        }
        self.depth / self.max_depth
    }

    pub fn effective_drag(&self, scale: f32) -> f32 {
        self.depth_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wader() -> Wader {
        Wader {
            depth: 0.0,
            max_depth: 100.0,
            wade_rate: 10.0,
            just_submerged: false,
            just_emerged: false,
            enabled: true,
        }
    }

    #[test]
    fn default_depth_zero() {
        let w = Wader::default();
        assert_eq!(w.depth, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wader::default().enabled);
    }

    #[test]
    fn wade_increases_depth() {
        let mut w = wader();
        w.wade(30.0);
        assert_eq!(w.depth, 30.0);
    }

    #[test]
    fn wade_clamps_at_max() {
        let mut w = wader();
        w.wade(200.0);
        assert_eq!(w.depth, 100.0);
    }

    #[test]
    fn wade_no_op_when_disabled() {
        let mut w = wader();
        w.enabled = false;
        w.wade(50.0);
        assert_eq!(w.depth, 0.0);
    }

    #[test]
    fn wade_sets_just_submerged_at_max() {
        let mut w = wader();
        w.wade(100.0);
        assert!(w.just_submerged);
    }

    #[test]
    fn wade_no_just_submerged_if_already_max() {
        let mut w = wader();
        w.depth = 100.0;
        w.wade(1.0);
        assert!(!w.just_submerged);
    }

    #[test]
    fn emerge_decreases_depth() {
        let mut w = wader();
        w.depth = 60.0;
        w.emerge(20.0);
        assert_eq!(w.depth, 40.0);
    }

    #[test]
    fn emerge_clamps_at_zero() {
        let mut w = wader();
        w.depth = 30.0;
        w.emerge(200.0);
        assert_eq!(w.depth, 0.0);
    }

    #[test]
    fn emerge_no_op_when_disabled() {
        let mut w = wader();
        w.depth = 50.0;
        w.enabled = false;
        w.emerge(10.0);
        assert_eq!(w.depth, 50.0);
    }

    #[test]
    fn emerge_no_op_when_already_emerged() {
        let mut w = wader();
        w.emerge(10.0);
        assert_eq!(w.depth, 0.0);
    }

    #[test]
    fn emerge_sets_just_emerged_at_zero() {
        let mut w = wader();
        w.depth = 10.0;
        w.emerge(10.0);
        assert!(w.just_emerged);
    }

    #[test]
    fn emerge_no_just_emerged_if_already_zero() {
        let mut w = wader();
        w.emerge(1.0);
        assert!(!w.just_emerged);
    }

    #[test]
    fn tick_increases_depth() {
        let mut w = wader();
        w.tick(1.0);
        assert_eq!(w.depth, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wader();
        w.tick(2.0);
        assert_eq!(w.depth, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wader();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.depth, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_submerged() {
        let mut w = wader();
        w.depth = 100.0;
        w.tick(1.0);
        assert_eq!(w.depth, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wader();
        w.wade_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.depth, 0.0);
    }

    #[test]
    fn is_submerged_true_at_max() {
        let mut w = wader();
        w.depth = 100.0;
        assert!(w.is_submerged());
    }

    #[test]
    fn is_submerged_false_below_max() {
        let mut w = wader();
        w.depth = 50.0;
        assert!(!w.is_submerged());
    }

    #[test]
    fn is_submerged_false_when_disabled() {
        let mut w = wader();
        w.depth = 100.0;
        w.enabled = false;
        assert!(!w.is_submerged());
    }

    #[test]
    fn is_emerged_true_at_zero() {
        let w = wader();
        assert!(w.is_emerged());
    }

    #[test]
    fn is_emerged_false_above_zero() {
        let mut w = wader();
        w.depth = 1.0;
        assert!(!w.is_emerged());
    }

    #[test]
    fn depth_fraction_zero_when_emerged() {
        let w = wader();
        assert_eq!(w.depth_fraction(), 0.0);
    }

    #[test]
    fn depth_fraction_one_at_max() {
        let mut w = wader();
        w.depth = 100.0;
        assert_eq!(w.depth_fraction(), 1.0);
    }

    #[test]
    fn depth_fraction_half_at_midpoint() {
        let mut w = wader();
        w.depth = 50.0;
        assert_eq!(w.depth_fraction(), 0.5);
    }

    #[test]
    fn depth_fraction_zero_when_max_zero() {
        let mut w = wader();
        w.max_depth = 0.0;
        assert_eq!(w.depth_fraction(), 0.0);
    }

    #[test]
    fn effective_drag_scales() {
        let mut w = wader();
        w.depth = 50.0;
        assert_eq!(w.effective_drag(2.0), 1.0);
    }

    #[test]
    fn effective_drag_zero_when_emerged() {
        let w = wader();
        assert_eq!(w.effective_drag(10.0), 0.0);
    }

    #[test]
    fn just_submerged_cleared_on_next_wade() {
        let mut w = wader();
        w.wade(100.0);
        assert!(w.just_submerged);
        w.wade(1.0);
        assert!(!w.just_submerged);
    }

    #[test]
    fn just_emerged_cleared_on_next_emerge() {
        let mut w = wader();
        w.depth = 10.0;
        w.emerge(10.0);
        assert!(w.just_emerged);
        w.depth = 10.0;
        w.emerge(1.0);
        assert!(!w.just_emerged);
    }
}

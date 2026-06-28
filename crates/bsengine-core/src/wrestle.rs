use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrestle {
    pub grapple: f32,
    pub max_grapple: f32,
    pub clinch_rate: f32,
    pub just_pinned: bool,
    pub just_released: bool,
    pub enabled: bool,
}

impl Default for Wrestle {
    fn default() -> Self {
        Self {
            grapple: 0.0,
            max_grapple: 100.0,
            clinch_rate: 1.0,
            just_pinned: false,
            just_released: false,
            enabled: true,
        }
    }
}

impl Wrestle {
    pub fn clinch(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_pinned = false;
        self.just_released = false;
        let prev = self.grapple;
        self.grapple = (self.grapple + amount).clamp(0.0, self.max_grapple);
        if self.grapple >= self.max_grapple && prev < self.max_grapple {
            self.just_pinned = true;
        }
    }

    pub fn release(&mut self, amount: f32) {
        if !self.enabled || self.grapple <= 0.0 {
            return;
        }
        self.just_pinned = false;
        self.just_released = false;
        let prev = self.grapple;
        self.grapple = (self.grapple - amount).max(0.0);
        if self.grapple <= 0.0 && prev > 0.0 {
            self.just_released = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.grapple >= self.max_grapple {
            return;
        }
        self.clinch(self.clinch_rate * dt);
    }

    pub fn is_pinned(&self) -> bool {
        self.enabled && self.grapple >= self.max_grapple
    }

    pub fn is_released(&self) -> bool {
        self.grapple <= 0.0
    }

    pub fn grapple_fraction(&self) -> f32 {
        if self.max_grapple <= 0.0 {
            return 0.0;
        }
        self.grapple / self.max_grapple
    }

    pub fn effective_hold(&self, scale: f32) -> f32 {
        self.grapple_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrestle() -> Wrestle {
        Wrestle {
            grapple: 0.0,
            max_grapple: 100.0,
            clinch_rate: 10.0,
            just_pinned: false,
            just_released: false,
            enabled: true,
        }
    }

    #[test]
    fn default_grapple_zero() {
        let w = Wrestle::default();
        assert_eq!(w.grapple, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wrestle::default().enabled);
    }

    #[test]
    fn clinch_increases_grapple() {
        let mut w = wrestle();
        w.clinch(30.0);
        assert_eq!(w.grapple, 30.0);
    }

    #[test]
    fn clinch_clamps_at_max() {
        let mut w = wrestle();
        w.clinch(200.0);
        assert_eq!(w.grapple, 100.0);
    }

    #[test]
    fn clinch_no_op_when_disabled() {
        let mut w = wrestle();
        w.enabled = false;
        w.clinch(50.0);
        assert_eq!(w.grapple, 0.0);
    }

    #[test]
    fn clinch_sets_just_pinned_at_max() {
        let mut w = wrestle();
        w.clinch(100.0);
        assert!(w.just_pinned);
    }

    #[test]
    fn clinch_no_just_pinned_if_already_max() {
        let mut w = wrestle();
        w.grapple = 100.0;
        w.clinch(1.0);
        assert!(!w.just_pinned);
    }

    #[test]
    fn release_decreases_grapple() {
        let mut w = wrestle();
        w.grapple = 60.0;
        w.release(20.0);
        assert_eq!(w.grapple, 40.0);
    }

    #[test]
    fn release_clamps_at_zero() {
        let mut w = wrestle();
        w.grapple = 30.0;
        w.release(200.0);
        assert_eq!(w.grapple, 0.0);
    }

    #[test]
    fn release_no_op_when_disabled() {
        let mut w = wrestle();
        w.grapple = 50.0;
        w.enabled = false;
        w.release(10.0);
        assert_eq!(w.grapple, 50.0);
    }

    #[test]
    fn release_no_op_when_already_released() {
        let mut w = wrestle();
        w.release(10.0);
        assert_eq!(w.grapple, 0.0);
    }

    #[test]
    fn release_sets_just_released_at_zero() {
        let mut w = wrestle();
        w.grapple = 10.0;
        w.release(10.0);
        assert!(w.just_released);
    }

    #[test]
    fn release_no_just_released_if_already_zero() {
        let mut w = wrestle();
        w.release(1.0);
        assert!(!w.just_released);
    }

    #[test]
    fn tick_increases_grapple() {
        let mut w = wrestle();
        w.tick(1.0);
        assert_eq!(w.grapple, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wrestle();
        w.tick(2.0);
        assert_eq!(w.grapple, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wrestle();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.grapple, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_pinned() {
        let mut w = wrestle();
        w.grapple = 100.0;
        w.tick(1.0);
        assert_eq!(w.grapple, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wrestle();
        w.clinch_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.grapple, 0.0);
    }

    #[test]
    fn is_pinned_true_at_max() {
        let mut w = wrestle();
        w.grapple = 100.0;
        assert!(w.is_pinned());
    }

    #[test]
    fn is_pinned_false_below_max() {
        let mut w = wrestle();
        w.grapple = 50.0;
        assert!(!w.is_pinned());
    }

    #[test]
    fn is_pinned_false_when_disabled() {
        let mut w = wrestle();
        w.grapple = 100.0;
        w.enabled = false;
        assert!(!w.is_pinned());
    }

    #[test]
    fn is_released_true_at_zero() {
        let w = wrestle();
        assert!(w.is_released());
    }

    #[test]
    fn is_released_false_above_zero() {
        let mut w = wrestle();
        w.grapple = 1.0;
        assert!(!w.is_released());
    }

    #[test]
    fn grapple_fraction_zero_when_released() {
        let w = wrestle();
        assert_eq!(w.grapple_fraction(), 0.0);
    }

    #[test]
    fn grapple_fraction_one_at_max() {
        let mut w = wrestle();
        w.grapple = 100.0;
        assert_eq!(w.grapple_fraction(), 1.0);
    }

    #[test]
    fn grapple_fraction_half_at_midpoint() {
        let mut w = wrestle();
        w.grapple = 50.0;
        assert_eq!(w.grapple_fraction(), 0.5);
    }

    #[test]
    fn grapple_fraction_zero_when_max_zero() {
        let mut w = wrestle();
        w.max_grapple = 0.0;
        assert_eq!(w.grapple_fraction(), 0.0);
    }

    #[test]
    fn effective_hold_scales() {
        let mut w = wrestle();
        w.grapple = 50.0;
        assert_eq!(w.effective_hold(2.0), 1.0);
    }

    #[test]
    fn effective_hold_zero_when_released() {
        let w = wrestle();
        assert_eq!(w.effective_hold(10.0), 0.0);
    }

    #[test]
    fn just_pinned_cleared_on_next_clinch() {
        let mut w = wrestle();
        w.clinch(100.0);
        assert!(w.just_pinned);
        w.clinch(1.0);
        assert!(!w.just_pinned);
    }

    #[test]
    fn just_released_cleared_on_next_release() {
        let mut w = wrestle();
        w.grapple = 10.0;
        w.release(10.0);
        assert!(w.just_released);
        w.grapple = 10.0;
        w.release(1.0);
        assert!(!w.just_released);
    }
}

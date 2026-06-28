use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wallow {
    pub immersion: f32,
    pub max_immersion: f32,
    pub sink_rate: f32,
    pub just_submerged: bool,
    pub just_surfaced: bool,
    pub enabled: bool,
}

impl Default for Wallow {
    fn default() -> Self {
        Self {
            immersion: 0.0,
            max_immersion: 100.0,
            sink_rate: 1.0,
            just_submerged: false,
            just_surfaced: false,
            enabled: true,
        }
    }
}

impl Wallow {
    pub fn sink(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_submerged = false;
        self.just_surfaced = false;
        let prev = self.immersion;
        self.immersion = (self.immersion + amount).clamp(0.0, self.max_immersion);
        if self.immersion >= self.max_immersion && prev < self.max_immersion {
            self.just_submerged = true;
        }
    }

    pub fn surface(&mut self, amount: f32) {
        if !self.enabled || self.immersion <= 0.0 {
            return;
        }
        self.just_submerged = false;
        self.just_surfaced = false;
        let prev = self.immersion;
        self.immersion = (self.immersion - amount).max(0.0);
        if self.immersion <= 0.0 && prev > 0.0 {
            self.just_surfaced = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.immersion >= self.max_immersion {
            return;
        }
        self.sink(self.sink_rate * dt);
    }

    pub fn is_submerged(&self) -> bool {
        self.enabled && self.immersion >= self.max_immersion
    }

    pub fn is_surfaced(&self) -> bool {
        self.immersion <= 0.0
    }

    pub fn immersion_fraction(&self) -> f32 {
        if self.max_immersion <= 0.0 {
            return 0.0;
        }
        self.immersion / self.max_immersion
    }

    pub fn effective_drag(&self, scale: f32) -> f32 {
        self.immersion_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wallow() -> Wallow {
        Wallow {
            immersion: 0.0,
            max_immersion: 100.0,
            sink_rate: 10.0,
            just_submerged: false,
            just_surfaced: false,
            enabled: true,
        }
    }

    #[test]
    fn default_immersion_zero() {
        let w = Wallow::default();
        assert_eq!(w.immersion, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wallow::default().enabled);
    }

    #[test]
    fn sink_increases_immersion() {
        let mut w = wallow();
        w.sink(30.0);
        assert_eq!(w.immersion, 30.0);
    }

    #[test]
    fn sink_clamps_at_max() {
        let mut w = wallow();
        w.sink(200.0);
        assert_eq!(w.immersion, 100.0);
    }

    #[test]
    fn sink_no_op_when_disabled() {
        let mut w = wallow();
        w.enabled = false;
        w.sink(50.0);
        assert_eq!(w.immersion, 0.0);
    }

    #[test]
    fn sink_sets_just_submerged_at_max() {
        let mut w = wallow();
        w.sink(100.0);
        assert!(w.just_submerged);
    }

    #[test]
    fn sink_no_just_submerged_if_already_max() {
        let mut w = wallow();
        w.immersion = 100.0;
        w.sink(1.0);
        assert!(!w.just_submerged);
    }

    #[test]
    fn surface_decreases_immersion() {
        let mut w = wallow();
        w.immersion = 60.0;
        w.surface(20.0);
        assert_eq!(w.immersion, 40.0);
    }

    #[test]
    fn surface_clamps_at_zero() {
        let mut w = wallow();
        w.immersion = 30.0;
        w.surface(200.0);
        assert_eq!(w.immersion, 0.0);
    }

    #[test]
    fn surface_no_op_when_disabled() {
        let mut w = wallow();
        w.immersion = 50.0;
        w.enabled = false;
        w.surface(10.0);
        assert_eq!(w.immersion, 50.0);
    }

    #[test]
    fn surface_no_op_when_already_surfaced() {
        let mut w = wallow();
        w.surface(10.0);
        assert_eq!(w.immersion, 0.0);
    }

    #[test]
    fn surface_sets_just_surfaced_at_zero() {
        let mut w = wallow();
        w.immersion = 10.0;
        w.surface(10.0);
        assert!(w.just_surfaced);
    }

    #[test]
    fn surface_no_just_surfaced_if_already_zero() {
        let mut w = wallow();
        w.surface(1.0);
        assert!(!w.just_surfaced);
    }

    #[test]
    fn tick_increases_immersion() {
        let mut w = wallow();
        w.tick(1.0);
        assert_eq!(w.immersion, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wallow();
        w.tick(2.0);
        assert_eq!(w.immersion, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wallow();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.immersion, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_submerged() {
        let mut w = wallow();
        w.immersion = 100.0;
        w.tick(1.0);
        assert_eq!(w.immersion, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wallow();
        w.sink_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.immersion, 0.0);
    }

    #[test]
    fn is_submerged_true_at_max() {
        let mut w = wallow();
        w.immersion = 100.0;
        assert!(w.is_submerged());
    }

    #[test]
    fn is_submerged_false_below_max() {
        let mut w = wallow();
        w.immersion = 50.0;
        assert!(!w.is_submerged());
    }

    #[test]
    fn is_submerged_false_when_disabled() {
        let mut w = wallow();
        w.immersion = 100.0;
        w.enabled = false;
        assert!(!w.is_submerged());
    }

    #[test]
    fn is_surfaced_true_at_zero() {
        let w = wallow();
        assert!(w.is_surfaced());
    }

    #[test]
    fn is_surfaced_false_above_zero() {
        let mut w = wallow();
        w.immersion = 1.0;
        assert!(!w.is_surfaced());
    }

    #[test]
    fn immersion_fraction_zero_when_surfaced() {
        let w = wallow();
        assert_eq!(w.immersion_fraction(), 0.0);
    }

    #[test]
    fn immersion_fraction_one_at_max() {
        let mut w = wallow();
        w.immersion = 100.0;
        assert_eq!(w.immersion_fraction(), 1.0);
    }

    #[test]
    fn immersion_fraction_half_at_midpoint() {
        let mut w = wallow();
        w.immersion = 50.0;
        assert_eq!(w.immersion_fraction(), 0.5);
    }

    #[test]
    fn immersion_fraction_zero_when_max_zero() {
        let mut w = wallow();
        w.max_immersion = 0.0;
        assert_eq!(w.immersion_fraction(), 0.0);
    }

    #[test]
    fn effective_drag_scales() {
        let mut w = wallow();
        w.immersion = 50.0;
        assert_eq!(w.effective_drag(2.0), 1.0);
    }

    #[test]
    fn effective_drag_zero_when_surfaced() {
        let w = wallow();
        assert_eq!(w.effective_drag(10.0), 0.0);
    }

    #[test]
    fn just_submerged_cleared_on_next_sink() {
        let mut w = wallow();
        w.sink(100.0);
        assert!(w.just_submerged);
        w.sink(1.0);
        assert!(!w.just_submerged);
    }

    #[test]
    fn just_surfaced_cleared_on_next_surface() {
        let mut w = wallow();
        w.immersion = 10.0;
        w.surface(10.0);
        assert!(w.just_surfaced);
        w.immersion = 10.0;
        w.surface(1.0);
        assert!(!w.just_surfaced);
    }
}

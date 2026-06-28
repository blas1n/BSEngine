use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wretched {
    pub misery: f32,
    pub max_misery: f32,
    pub suffer_rate: f32,
    pub just_abject: bool,
    pub just_unburdened: bool,
    pub enabled: bool,
}

impl Default for Wretched {
    fn default() -> Self {
        Self {
            misery: 0.0,
            max_misery: 100.0,
            suffer_rate: 1.0,
            just_abject: false,
            just_unburdened: false,
            enabled: true,
        }
    }
}

impl Wretched {
    pub fn suffer(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_abject = false;
        self.just_unburdened = false;
        let prev = self.misery;
        self.misery = (self.misery + amount).clamp(0.0, self.max_misery);
        if self.misery >= self.max_misery && prev < self.max_misery {
            self.just_abject = true;
        }
    }

    pub fn relieve(&mut self, amount: f32) {
        if !self.enabled || self.misery <= 0.0 {
            return;
        }
        self.just_abject = false;
        self.just_unburdened = false;
        let prev = self.misery;
        self.misery = (self.misery - amount).max(0.0);
        if self.misery <= 0.0 && prev > 0.0 {
            self.just_unburdened = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.misery >= self.max_misery {
            return;
        }
        self.suffer(self.suffer_rate * dt);
    }

    pub fn is_abject(&self) -> bool {
        self.enabled && self.misery >= self.max_misery
    }

    pub fn is_unburdened(&self) -> bool {
        self.misery <= 0.0
    }

    pub fn misery_fraction(&self) -> f32 {
        if self.max_misery <= 0.0 {
            return 0.0;
        }
        self.misery / self.max_misery
    }

    pub fn effective_suffering(&self, scale: f32) -> f32 {
        self.misery_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wretched() -> Wretched {
        Wretched {
            misery: 0.0,
            max_misery: 100.0,
            suffer_rate: 10.0,
            just_abject: false,
            just_unburdened: false,
            enabled: true,
        }
    }

    #[test]
    fn default_misery_zero() {
        let w = Wretched::default();
        assert_eq!(w.misery, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wretched::default().enabled);
    }

    #[test]
    fn suffer_increases_misery() {
        let mut w = wretched();
        w.suffer(30.0);
        assert_eq!(w.misery, 30.0);
    }

    #[test]
    fn suffer_clamps_at_max() {
        let mut w = wretched();
        w.suffer(200.0);
        assert_eq!(w.misery, 100.0);
    }

    #[test]
    fn suffer_no_op_when_disabled() {
        let mut w = wretched();
        w.enabled = false;
        w.suffer(50.0);
        assert_eq!(w.misery, 0.0);
    }

    #[test]
    fn suffer_sets_just_abject_at_max() {
        let mut w = wretched();
        w.suffer(100.0);
        assert!(w.just_abject);
    }

    #[test]
    fn suffer_no_just_abject_if_already_max() {
        let mut w = wretched();
        w.misery = 100.0;
        w.suffer(1.0);
        assert!(!w.just_abject);
    }

    #[test]
    fn relieve_decreases_misery() {
        let mut w = wretched();
        w.misery = 60.0;
        w.relieve(20.0);
        assert_eq!(w.misery, 40.0);
    }

    #[test]
    fn relieve_clamps_at_zero() {
        let mut w = wretched();
        w.misery = 30.0;
        w.relieve(200.0);
        assert_eq!(w.misery, 0.0);
    }

    #[test]
    fn relieve_no_op_when_disabled() {
        let mut w = wretched();
        w.misery = 50.0;
        w.enabled = false;
        w.relieve(10.0);
        assert_eq!(w.misery, 50.0);
    }

    #[test]
    fn relieve_no_op_when_already_unburdened() {
        let mut w = wretched();
        w.relieve(10.0);
        assert_eq!(w.misery, 0.0);
    }

    #[test]
    fn relieve_sets_just_unburdened_at_zero() {
        let mut w = wretched();
        w.misery = 10.0;
        w.relieve(10.0);
        assert!(w.just_unburdened);
    }

    #[test]
    fn relieve_no_just_unburdened_if_already_zero() {
        let mut w = wretched();
        w.relieve(1.0);
        assert!(!w.just_unburdened);
    }

    #[test]
    fn tick_increases_misery() {
        let mut w = wretched();
        w.tick(1.0);
        assert_eq!(w.misery, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wretched();
        w.tick(2.0);
        assert_eq!(w.misery, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wretched();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.misery, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_abject() {
        let mut w = wretched();
        w.misery = 100.0;
        w.tick(1.0);
        assert_eq!(w.misery, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wretched();
        w.suffer_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.misery, 0.0);
    }

    #[test]
    fn is_abject_true_at_max() {
        let mut w = wretched();
        w.misery = 100.0;
        assert!(w.is_abject());
    }

    #[test]
    fn is_abject_false_below_max() {
        let mut w = wretched();
        w.misery = 50.0;
        assert!(!w.is_abject());
    }

    #[test]
    fn is_abject_false_when_disabled() {
        let mut w = wretched();
        w.misery = 100.0;
        w.enabled = false;
        assert!(!w.is_abject());
    }

    #[test]
    fn is_unburdened_true_at_zero() {
        let w = wretched();
        assert!(w.is_unburdened());
    }

    #[test]
    fn is_unburdened_false_above_zero() {
        let mut w = wretched();
        w.misery = 1.0;
        assert!(!w.is_unburdened());
    }

    #[test]
    fn misery_fraction_zero_when_unburdened() {
        let w = wretched();
        assert_eq!(w.misery_fraction(), 0.0);
    }

    #[test]
    fn misery_fraction_one_at_max() {
        let mut w = wretched();
        w.misery = 100.0;
        assert_eq!(w.misery_fraction(), 1.0);
    }

    #[test]
    fn misery_fraction_half_at_midpoint() {
        let mut w = wretched();
        w.misery = 50.0;
        assert_eq!(w.misery_fraction(), 0.5);
    }

    #[test]
    fn misery_fraction_zero_when_max_zero() {
        let mut w = wretched();
        w.max_misery = 0.0;
        assert_eq!(w.misery_fraction(), 0.0);
    }

    #[test]
    fn effective_suffering_scales() {
        let mut w = wretched();
        w.misery = 50.0;
        assert_eq!(w.effective_suffering(2.0), 1.0);
    }

    #[test]
    fn effective_suffering_zero_when_unburdened() {
        let w = wretched();
        assert_eq!(w.effective_suffering(10.0), 0.0);
    }

    #[test]
    fn just_abject_cleared_on_next_suffer() {
        let mut w = wretched();
        w.suffer(100.0);
        assert!(w.just_abject);
        w.suffer(1.0);
        assert!(!w.just_abject);
    }

    #[test]
    fn just_unburdened_cleared_on_next_relieve() {
        let mut w = wretched();
        w.misery = 10.0;
        w.relieve(10.0);
        assert!(w.just_unburdened);
        w.misery = 10.0;
        w.relieve(1.0);
        assert!(!w.just_unburdened);
    }
}

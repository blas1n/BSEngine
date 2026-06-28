use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrapper {
    pub enclosure: f32,
    pub max_enclosure: f32,
    pub bind_rate: f32,
    pub just_bound: bool,
    pub just_exposed: bool,
    pub enabled: bool,
}

impl Default for Wrapper {
    fn default() -> Self {
        Self {
            enclosure: 0.0,
            max_enclosure: 100.0,
            bind_rate: 1.0,
            just_bound: false,
            just_exposed: false,
            enabled: true,
        }
    }
}

impl Wrapper {
    pub fn bind(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_bound = false;
        self.just_exposed = false;
        let prev = self.enclosure;
        self.enclosure = (self.enclosure + amount).clamp(0.0, self.max_enclosure);
        if self.enclosure >= self.max_enclosure && prev < self.max_enclosure {
            self.just_bound = true;
        }
    }

    pub fn unwrap(&mut self, amount: f32) {
        if !self.enabled || self.enclosure <= 0.0 {
            return;
        }
        self.just_bound = false;
        self.just_exposed = false;
        let prev = self.enclosure;
        self.enclosure = (self.enclosure - amount).max(0.0);
        if self.enclosure <= 0.0 && prev > 0.0 {
            self.just_exposed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.enclosure >= self.max_enclosure {
            return;
        }
        self.bind(self.bind_rate * dt);
    }

    pub fn is_bound(&self) -> bool {
        self.enabled && self.enclosure >= self.max_enclosure
    }

    pub fn is_exposed(&self) -> bool {
        self.enclosure <= 0.0
    }

    pub fn enclosure_fraction(&self) -> f32 {
        if self.max_enclosure <= 0.0 {
            return 0.0;
        }
        self.enclosure / self.max_enclosure
    }

    pub fn effective_protection(&self, scale: f32) -> f32 {
        self.enclosure_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrapper() -> Wrapper {
        Wrapper {
            enclosure: 0.0,
            max_enclosure: 100.0,
            bind_rate: 10.0,
            just_bound: false,
            just_exposed: false,
            enabled: true,
        }
    }

    #[test]
    fn default_enclosure_zero() {
        let w = Wrapper::default();
        assert_eq!(w.enclosure, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wrapper::default().enabled);
    }

    #[test]
    fn bind_increases_enclosure() {
        let mut w = wrapper();
        w.bind(30.0);
        assert_eq!(w.enclosure, 30.0);
    }

    #[test]
    fn bind_clamps_at_max() {
        let mut w = wrapper();
        w.bind(200.0);
        assert_eq!(w.enclosure, 100.0);
    }

    #[test]
    fn bind_no_op_when_disabled() {
        let mut w = wrapper();
        w.enabled = false;
        w.bind(50.0);
        assert_eq!(w.enclosure, 0.0);
    }

    #[test]
    fn bind_sets_just_bound_at_max() {
        let mut w = wrapper();
        w.bind(100.0);
        assert!(w.just_bound);
    }

    #[test]
    fn bind_no_just_bound_if_already_max() {
        let mut w = wrapper();
        w.enclosure = 100.0;
        w.bind(1.0);
        assert!(!w.just_bound);
    }

    #[test]
    fn unwrap_decreases_enclosure() {
        let mut w = wrapper();
        w.enclosure = 60.0;
        w.unwrap(20.0);
        assert_eq!(w.enclosure, 40.0);
    }

    #[test]
    fn unwrap_clamps_at_zero() {
        let mut w = wrapper();
        w.enclosure = 30.0;
        w.unwrap(200.0);
        assert_eq!(w.enclosure, 0.0);
    }

    #[test]
    fn unwrap_no_op_when_disabled() {
        let mut w = wrapper();
        w.enclosure = 50.0;
        w.enabled = false;
        w.unwrap(10.0);
        assert_eq!(w.enclosure, 50.0);
    }

    #[test]
    fn unwrap_no_op_when_already_exposed() {
        let mut w = wrapper();
        w.unwrap(10.0);
        assert_eq!(w.enclosure, 0.0);
    }

    #[test]
    fn unwrap_sets_just_exposed_at_zero() {
        let mut w = wrapper();
        w.enclosure = 10.0;
        w.unwrap(10.0);
        assert!(w.just_exposed);
    }

    #[test]
    fn unwrap_no_just_exposed_if_already_zero() {
        let mut w = wrapper();
        w.unwrap(1.0);
        assert!(!w.just_exposed);
    }

    #[test]
    fn tick_increases_enclosure() {
        let mut w = wrapper();
        w.tick(1.0);
        assert_eq!(w.enclosure, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wrapper();
        w.tick(2.0);
        assert_eq!(w.enclosure, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wrapper();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.enclosure, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_bound() {
        let mut w = wrapper();
        w.enclosure = 100.0;
        w.tick(1.0);
        assert_eq!(w.enclosure, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wrapper();
        w.bind_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.enclosure, 0.0);
    }

    #[test]
    fn is_bound_true_at_max() {
        let mut w = wrapper();
        w.enclosure = 100.0;
        assert!(w.is_bound());
    }

    #[test]
    fn is_bound_false_below_max() {
        let mut w = wrapper();
        w.enclosure = 50.0;
        assert!(!w.is_bound());
    }

    #[test]
    fn is_bound_false_when_disabled() {
        let mut w = wrapper();
        w.enclosure = 100.0;
        w.enabled = false;
        assert!(!w.is_bound());
    }

    #[test]
    fn is_exposed_true_at_zero() {
        let w = wrapper();
        assert!(w.is_exposed());
    }

    #[test]
    fn is_exposed_false_above_zero() {
        let mut w = wrapper();
        w.enclosure = 1.0;
        assert!(!w.is_exposed());
    }

    #[test]
    fn enclosure_fraction_zero_when_exposed() {
        let w = wrapper();
        assert_eq!(w.enclosure_fraction(), 0.0);
    }

    #[test]
    fn enclosure_fraction_one_at_max() {
        let mut w = wrapper();
        w.enclosure = 100.0;
        assert_eq!(w.enclosure_fraction(), 1.0);
    }

    #[test]
    fn enclosure_fraction_half_at_midpoint() {
        let mut w = wrapper();
        w.enclosure = 50.0;
        assert_eq!(w.enclosure_fraction(), 0.5);
    }

    #[test]
    fn enclosure_fraction_zero_when_max_zero() {
        let mut w = wrapper();
        w.max_enclosure = 0.0;
        assert_eq!(w.enclosure_fraction(), 0.0);
    }

    #[test]
    fn effective_protection_scales() {
        let mut w = wrapper();
        w.enclosure = 50.0;
        assert_eq!(w.effective_protection(2.0), 1.0);
    }

    #[test]
    fn effective_protection_zero_when_exposed() {
        let w = wrapper();
        assert_eq!(w.effective_protection(10.0), 0.0);
    }

    #[test]
    fn just_bound_cleared_on_next_bind() {
        let mut w = wrapper();
        w.bind(100.0);
        assert!(w.just_bound);
        w.bind(1.0);
        assert!(!w.just_bound);
    }

    #[test]
    fn just_exposed_cleared_on_next_unwrap() {
        let mut w = wrapper();
        w.enclosure = 10.0;
        w.unwrap(10.0);
        assert!(w.just_exposed);
        w.enclosure = 10.0;
        w.unwrap(1.0);
        assert!(!w.just_exposed);
    }
}

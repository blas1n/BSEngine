use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weight {
    pub mass: f32,
    pub max_mass: f32,
    pub accrue_rate: f32,
    pub just_heavy: bool,
    pub just_weightless: bool,
    pub enabled: bool,
}

impl Default for Weight {
    fn default() -> Self {
        Self {
            mass: 0.0,
            max_mass: 100.0,
            accrue_rate: 1.0,
            just_heavy: false,
            just_weightless: false,
            enabled: true,
        }
    }
}

impl Weight {
    pub fn accrue(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_heavy = false;
        self.just_weightless = false;
        let prev = self.mass;
        self.mass = (self.mass + amount).clamp(0.0, self.max_mass);
        if self.mass >= self.max_mass && prev < self.max_mass {
            self.just_heavy = true;
        }
    }

    pub fn shed(&mut self, amount: f32) {
        if !self.enabled || self.mass <= 0.0 {
            return;
        }
        self.just_heavy = false;
        self.just_weightless = false;
        let prev = self.mass;
        self.mass = (self.mass - amount).max(0.0);
        if self.mass <= 0.0 && prev > 0.0 {
            self.just_weightless = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.mass >= self.max_mass {
            return;
        }
        self.accrue(self.accrue_rate * dt);
    }

    pub fn is_heavy(&self) -> bool {
        self.enabled && self.mass >= self.max_mass
    }

    pub fn is_weightless(&self) -> bool {
        self.mass <= 0.0
    }

    pub fn mass_fraction(&self) -> f32 {
        if self.max_mass <= 0.0 {
            return 0.0;
        }
        self.mass / self.max_mass
    }

    pub fn effective_pressure(&self, scale: f32) -> f32 {
        self.mass_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weight() -> Weight {
        Weight {
            mass: 0.0,
            max_mass: 100.0,
            accrue_rate: 10.0,
            just_heavy: false,
            just_weightless: false,
            enabled: true,
        }
    }

    #[test]
    fn default_mass_zero() {
        let w = Weight::default();
        assert_eq!(w.mass, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Weight::default().enabled);
    }

    #[test]
    fn accrue_increases_mass() {
        let mut w = weight();
        w.accrue(30.0);
        assert_eq!(w.mass, 30.0);
    }

    #[test]
    fn accrue_clamps_at_max() {
        let mut w = weight();
        w.accrue(200.0);
        assert_eq!(w.mass, 100.0);
    }

    #[test]
    fn accrue_no_op_when_disabled() {
        let mut w = weight();
        w.enabled = false;
        w.accrue(50.0);
        assert_eq!(w.mass, 0.0);
    }

    #[test]
    fn accrue_sets_just_heavy_at_max() {
        let mut w = weight();
        w.accrue(100.0);
        assert!(w.just_heavy);
    }

    #[test]
    fn accrue_no_just_heavy_if_already_max() {
        let mut w = weight();
        w.mass = 100.0;
        w.accrue(1.0);
        assert!(!w.just_heavy);
    }

    #[test]
    fn shed_decreases_mass() {
        let mut w = weight();
        w.mass = 60.0;
        w.shed(20.0);
        assert_eq!(w.mass, 40.0);
    }

    #[test]
    fn shed_clamps_at_zero() {
        let mut w = weight();
        w.mass = 30.0;
        w.shed(200.0);
        assert_eq!(w.mass, 0.0);
    }

    #[test]
    fn shed_no_op_when_disabled() {
        let mut w = weight();
        w.mass = 50.0;
        w.enabled = false;
        w.shed(10.0);
        assert_eq!(w.mass, 50.0);
    }

    #[test]
    fn shed_no_op_when_already_weightless() {
        let mut w = weight();
        w.shed(10.0);
        assert_eq!(w.mass, 0.0);
    }

    #[test]
    fn shed_sets_just_weightless_at_zero() {
        let mut w = weight();
        w.mass = 10.0;
        w.shed(10.0);
        assert!(w.just_weightless);
    }

    #[test]
    fn shed_no_just_weightless_if_already_zero() {
        let mut w = weight();
        w.shed(1.0);
        assert!(!w.just_weightless);
    }

    #[test]
    fn tick_increases_mass() {
        let mut w = weight();
        w.tick(1.0);
        assert_eq!(w.mass, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = weight();
        w.tick(2.0);
        assert_eq!(w.mass, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = weight();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.mass, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_heavy() {
        let mut w = weight();
        w.mass = 100.0;
        w.tick(1.0);
        assert_eq!(w.mass, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = weight();
        w.accrue_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.mass, 0.0);
    }

    #[test]
    fn is_heavy_true_at_max() {
        let mut w = weight();
        w.mass = 100.0;
        assert!(w.is_heavy());
    }

    #[test]
    fn is_heavy_false_below_max() {
        let mut w = weight();
        w.mass = 50.0;
        assert!(!w.is_heavy());
    }

    #[test]
    fn is_heavy_false_when_disabled() {
        let mut w = weight();
        w.mass = 100.0;
        w.enabled = false;
        assert!(!w.is_heavy());
    }

    #[test]
    fn is_weightless_true_at_zero() {
        let w = weight();
        assert!(w.is_weightless());
    }

    #[test]
    fn is_weightless_false_above_zero() {
        let mut w = weight();
        w.mass = 1.0;
        assert!(!w.is_weightless());
    }

    #[test]
    fn mass_fraction_zero_when_weightless() {
        let w = weight();
        assert_eq!(w.mass_fraction(), 0.0);
    }

    #[test]
    fn mass_fraction_one_at_max() {
        let mut w = weight();
        w.mass = 100.0;
        assert_eq!(w.mass_fraction(), 1.0);
    }

    #[test]
    fn mass_fraction_half_at_midpoint() {
        let mut w = weight();
        w.mass = 50.0;
        assert_eq!(w.mass_fraction(), 0.5);
    }

    #[test]
    fn mass_fraction_zero_when_max_zero() {
        let mut w = weight();
        w.max_mass = 0.0;
        assert_eq!(w.mass_fraction(), 0.0);
    }

    #[test]
    fn effective_pressure_scales() {
        let mut w = weight();
        w.mass = 50.0;
        assert_eq!(w.effective_pressure(2.0), 1.0);
    }

    #[test]
    fn effective_pressure_zero_when_weightless() {
        let w = weight();
        assert_eq!(w.effective_pressure(10.0), 0.0);
    }

    #[test]
    fn just_heavy_cleared_on_next_accrue() {
        let mut w = weight();
        w.accrue(100.0);
        assert!(w.just_heavy);
        w.accrue(1.0);
        assert!(!w.just_heavy);
    }

    #[test]
    fn just_weightless_cleared_on_next_shed() {
        let mut w = weight();
        w.mass = 10.0;
        w.shed(10.0);
        assert!(w.just_weightless);
        w.mass = 10.0;
        w.shed(1.0);
        assert!(!w.just_weightless);
    }
}

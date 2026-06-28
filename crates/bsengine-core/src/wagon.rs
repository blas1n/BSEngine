use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wagon {
    pub load: f32,
    pub max_load: f32,
    pub haul_rate: f32,
    pub just_full: bool,
    pub just_empty: bool,
    pub enabled: bool,
}

impl Default for Wagon {
    fn default() -> Self {
        Self {
            load: 0.0,
            max_load: 100.0,
            haul_rate: 1.0,
            just_full: false,
            just_empty: false,
            enabled: true,
        }
    }
}

impl Wagon {
    pub fn haul(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_full = false;
        self.just_empty = false;
        let prev = self.load;
        self.load = (self.load + amount).clamp(0.0, self.max_load);
        if self.load >= self.max_load && prev < self.max_load {
            self.just_full = true;
        }
    }

    pub fn unload(&mut self, amount: f32) {
        if !self.enabled || self.load <= 0.0 {
            return;
        }
        self.just_full = false;
        self.just_empty = false;
        let prev = self.load;
        self.load = (self.load - amount).max(0.0);
        if self.load <= 0.0 && prev > 0.0 {
            self.just_empty = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.load >= self.max_load {
            return;
        }
        self.haul(self.haul_rate * dt);
    }

    pub fn is_full(&self) -> bool {
        self.enabled && self.load >= self.max_load
    }

    pub fn is_empty(&self) -> bool {
        self.load <= 0.0
    }

    pub fn load_fraction(&self) -> f32 {
        if self.max_load <= 0.0 {
            return 0.0;
        }
        self.load / self.max_load
    }

    pub fn effective_cargo(&self, scale: f32) -> f32 {
        self.load_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wagon() -> Wagon {
        Wagon {
            load: 0.0,
            max_load: 100.0,
            haul_rate: 10.0,
            just_full: false,
            just_empty: false,
            enabled: true,
        }
    }

    #[test]
    fn default_load_zero() {
        let w = Wagon::default();
        assert_eq!(w.load, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wagon::default().enabled);
    }

    #[test]
    fn haul_increases_load() {
        let mut w = wagon();
        w.haul(30.0);
        assert_eq!(w.load, 30.0);
    }

    #[test]
    fn haul_clamps_at_max() {
        let mut w = wagon();
        w.haul(200.0);
        assert_eq!(w.load, 100.0);
    }

    #[test]
    fn haul_no_op_when_disabled() {
        let mut w = wagon();
        w.enabled = false;
        w.haul(50.0);
        assert_eq!(w.load, 0.0);
    }

    #[test]
    fn haul_sets_just_full_at_max() {
        let mut w = wagon();
        w.haul(100.0);
        assert!(w.just_full);
    }

    #[test]
    fn haul_no_just_full_if_already_max() {
        let mut w = wagon();
        w.load = 100.0;
        w.haul(1.0);
        assert!(!w.just_full);
    }

    #[test]
    fn unload_decreases_load() {
        let mut w = wagon();
        w.load = 60.0;
        w.unload(20.0);
        assert_eq!(w.load, 40.0);
    }

    #[test]
    fn unload_clamps_at_zero() {
        let mut w = wagon();
        w.load = 30.0;
        w.unload(200.0);
        assert_eq!(w.load, 0.0);
    }

    #[test]
    fn unload_no_op_when_disabled() {
        let mut w = wagon();
        w.load = 50.0;
        w.enabled = false;
        w.unload(10.0);
        assert_eq!(w.load, 50.0);
    }

    #[test]
    fn unload_no_op_when_already_empty() {
        let mut w = wagon();
        w.unload(10.0);
        assert_eq!(w.load, 0.0);
    }

    #[test]
    fn unload_sets_just_empty_at_zero() {
        let mut w = wagon();
        w.load = 10.0;
        w.unload(10.0);
        assert!(w.just_empty);
    }

    #[test]
    fn unload_no_just_empty_if_already_zero() {
        let mut w = wagon();
        w.unload(1.0);
        assert!(!w.just_empty);
    }

    #[test]
    fn tick_increases_load() {
        let mut w = wagon();
        w.tick(1.0);
        assert_eq!(w.load, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wagon();
        w.tick(2.0);
        assert_eq!(w.load, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wagon();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.load, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_full() {
        let mut w = wagon();
        w.load = 100.0;
        w.tick(1.0);
        assert_eq!(w.load, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wagon();
        w.haul_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.load, 0.0);
    }

    #[test]
    fn is_full_true_at_max() {
        let mut w = wagon();
        w.load = 100.0;
        assert!(w.is_full());
    }

    #[test]
    fn is_full_false_below_max() {
        let mut w = wagon();
        w.load = 50.0;
        assert!(!w.is_full());
    }

    #[test]
    fn is_full_false_when_disabled() {
        let mut w = wagon();
        w.load = 100.0;
        w.enabled = false;
        assert!(!w.is_full());
    }

    #[test]
    fn is_empty_true_at_zero() {
        let w = wagon();
        assert!(w.is_empty());
    }

    #[test]
    fn is_empty_false_above_zero() {
        let mut w = wagon();
        w.load = 1.0;
        assert!(!w.is_empty());
    }

    #[test]
    fn load_fraction_zero_when_empty() {
        let w = wagon();
        assert_eq!(w.load_fraction(), 0.0);
    }

    #[test]
    fn load_fraction_one_at_max() {
        let mut w = wagon();
        w.load = 100.0;
        assert_eq!(w.load_fraction(), 1.0);
    }

    #[test]
    fn load_fraction_half_at_midpoint() {
        let mut w = wagon();
        w.load = 50.0;
        assert_eq!(w.load_fraction(), 0.5);
    }

    #[test]
    fn load_fraction_zero_when_max_zero() {
        let mut w = wagon();
        w.max_load = 0.0;
        assert_eq!(w.load_fraction(), 0.0);
    }

    #[test]
    fn effective_cargo_scales() {
        let mut w = wagon();
        w.load = 50.0;
        assert_eq!(w.effective_cargo(2.0), 1.0);
    }

    #[test]
    fn effective_cargo_zero_when_empty() {
        let w = wagon();
        assert_eq!(w.effective_cargo(10.0), 0.0);
    }

    #[test]
    fn just_full_cleared_on_next_haul() {
        let mut w = wagon();
        w.haul(100.0);
        assert!(w.just_full);
        w.haul(1.0);
        assert!(!w.just_full);
    }

    #[test]
    fn just_empty_cleared_on_next_unload() {
        let mut w = wagon();
        w.load = 10.0;
        w.unload(10.0);
        assert!(w.just_empty);
        w.load = 10.0;
        w.unload(1.0);
        assert!(!w.just_empty);
    }
}

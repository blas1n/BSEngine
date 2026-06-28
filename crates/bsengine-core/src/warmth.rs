use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Warmth {
    pub heat: f32,
    pub max_heat: f32,
    pub radiate_rate: f32,
    pub just_scalding: bool,
    pub just_frigid: bool,
    pub enabled: bool,
}

impl Default for Warmth {
    fn default() -> Self {
        Self {
            heat: 0.0,
            max_heat: 100.0,
            radiate_rate: 1.0,
            just_scalding: false,
            just_frigid: false,
            enabled: true,
        }
    }
}

impl Warmth {
    pub fn radiate(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_scalding = false;
        self.just_frigid = false;
        let prev = self.heat;
        self.heat = (self.heat + amount).clamp(0.0, self.max_heat);
        if self.heat >= self.max_heat && prev < self.max_heat {
            self.just_scalding = true;
        }
    }

    pub fn cool(&mut self, amount: f32) {
        if !self.enabled || self.heat <= 0.0 {
            return;
        }
        self.just_scalding = false;
        self.just_frigid = false;
        let prev = self.heat;
        self.heat = (self.heat - amount).max(0.0);
        if self.heat <= 0.0 && prev > 0.0 {
            self.just_frigid = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.heat >= self.max_heat {
            return;
        }
        self.radiate(self.radiate_rate * dt);
    }

    pub fn is_scalding(&self) -> bool {
        self.enabled && self.heat >= self.max_heat
    }

    pub fn is_frigid(&self) -> bool {
        self.heat <= 0.0
    }

    pub fn heat_fraction(&self) -> f32 {
        if self.max_heat <= 0.0 {
            return 0.0;
        }
        self.heat / self.max_heat
    }

    pub fn effective_warmth(&self, scale: f32) -> f32 {
        self.heat_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warmth() -> Warmth {
        Warmth {
            heat: 0.0,
            max_heat: 100.0,
            radiate_rate: 10.0,
            just_scalding: false,
            just_frigid: false,
            enabled: true,
        }
    }

    #[test]
    fn default_heat_zero() {
        let w = Warmth::default();
        assert_eq!(w.heat, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Warmth::default().enabled);
    }

    #[test]
    fn radiate_increases_heat() {
        let mut w = warmth();
        w.radiate(30.0);
        assert_eq!(w.heat, 30.0);
    }

    #[test]
    fn radiate_clamps_at_max() {
        let mut w = warmth();
        w.radiate(200.0);
        assert_eq!(w.heat, 100.0);
    }

    #[test]
    fn radiate_no_op_when_disabled() {
        let mut w = warmth();
        w.enabled = false;
        w.radiate(50.0);
        assert_eq!(w.heat, 0.0);
    }

    #[test]
    fn radiate_sets_just_scalding_at_max() {
        let mut w = warmth();
        w.radiate(100.0);
        assert!(w.just_scalding);
    }

    #[test]
    fn radiate_no_just_scalding_if_already_max() {
        let mut w = warmth();
        w.heat = 100.0;
        w.radiate(1.0);
        assert!(!w.just_scalding);
    }

    #[test]
    fn cool_decreases_heat() {
        let mut w = warmth();
        w.heat = 60.0;
        w.cool(20.0);
        assert_eq!(w.heat, 40.0);
    }

    #[test]
    fn cool_clamps_at_zero() {
        let mut w = warmth();
        w.heat = 30.0;
        w.cool(200.0);
        assert_eq!(w.heat, 0.0);
    }

    #[test]
    fn cool_no_op_when_disabled() {
        let mut w = warmth();
        w.heat = 50.0;
        w.enabled = false;
        w.cool(10.0);
        assert_eq!(w.heat, 50.0);
    }

    #[test]
    fn cool_no_op_when_already_frigid() {
        let mut w = warmth();
        w.cool(10.0);
        assert_eq!(w.heat, 0.0);
    }

    #[test]
    fn cool_sets_just_frigid_at_zero() {
        let mut w = warmth();
        w.heat = 10.0;
        w.cool(10.0);
        assert!(w.just_frigid);
    }

    #[test]
    fn cool_no_just_frigid_if_already_zero() {
        let mut w = warmth();
        w.cool(1.0);
        assert!(!w.just_frigid);
    }

    #[test]
    fn tick_increases_heat() {
        let mut w = warmth();
        w.tick(1.0);
        assert_eq!(w.heat, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = warmth();
        w.tick(2.0);
        assert_eq!(w.heat, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = warmth();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.heat, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_scalding() {
        let mut w = warmth();
        w.heat = 100.0;
        w.tick(1.0);
        assert_eq!(w.heat, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = warmth();
        w.radiate_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.heat, 0.0);
    }

    #[test]
    fn is_scalding_true_at_max() {
        let mut w = warmth();
        w.heat = 100.0;
        assert!(w.is_scalding());
    }

    #[test]
    fn is_scalding_false_below_max() {
        let mut w = warmth();
        w.heat = 50.0;
        assert!(!w.is_scalding());
    }

    #[test]
    fn is_scalding_false_when_disabled() {
        let mut w = warmth();
        w.heat = 100.0;
        w.enabled = false;
        assert!(!w.is_scalding());
    }

    #[test]
    fn is_frigid_true_at_zero() {
        let w = warmth();
        assert!(w.is_frigid());
    }

    #[test]
    fn is_frigid_false_above_zero() {
        let mut w = warmth();
        w.heat = 1.0;
        assert!(!w.is_frigid());
    }

    #[test]
    fn heat_fraction_zero_when_frigid() {
        let w = warmth();
        assert_eq!(w.heat_fraction(), 0.0);
    }

    #[test]
    fn heat_fraction_one_at_max() {
        let mut w = warmth();
        w.heat = 100.0;
        assert_eq!(w.heat_fraction(), 1.0);
    }

    #[test]
    fn heat_fraction_half_at_midpoint() {
        let mut w = warmth();
        w.heat = 50.0;
        assert_eq!(w.heat_fraction(), 0.5);
    }

    #[test]
    fn heat_fraction_zero_when_max_zero() {
        let mut w = warmth();
        w.max_heat = 0.0;
        assert_eq!(w.heat_fraction(), 0.0);
    }

    #[test]
    fn effective_warmth_scales() {
        let mut w = warmth();
        w.heat = 50.0;
        assert_eq!(w.effective_warmth(2.0), 1.0);
    }

    #[test]
    fn effective_warmth_zero_when_frigid() {
        let w = warmth();
        assert_eq!(w.effective_warmth(10.0), 0.0);
    }

    #[test]
    fn just_scalding_cleared_on_next_radiate() {
        let mut w = warmth();
        w.radiate(100.0);
        assert!(w.just_scalding);
        w.radiate(1.0);
        assert!(!w.just_scalding);
    }

    #[test]
    fn just_frigid_cleared_on_next_cool() {
        let mut w = warmth();
        w.heat = 10.0;
        w.cool(10.0);
        assert!(w.just_frigid);
        w.heat = 10.0;
        w.cool(1.0);
        assert!(!w.just_frigid);
    }
}

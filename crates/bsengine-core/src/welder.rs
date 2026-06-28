use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Welder {
    pub bond_strength: f32,
    pub max_bond_strength: f32,
    pub fuse_rate: f32,
    pub just_fused: bool,
    pub just_fractured: bool,
    pub enabled: bool,
}

impl Default for Welder {
    fn default() -> Self {
        Self {
            bond_strength: 0.0,
            max_bond_strength: 100.0,
            fuse_rate: 1.0,
            just_fused: false,
            just_fractured: false,
            enabled: true,
        }
    }
}

impl Welder {
    pub fn fuse(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_fused = false;
        self.just_fractured = false;
        let prev = self.bond_strength;
        self.bond_strength = (self.bond_strength + amount).clamp(0.0, self.max_bond_strength);
        if self.bond_strength >= self.max_bond_strength && prev < self.max_bond_strength {
            self.just_fused = true;
        }
    }

    pub fn fracture(&mut self, amount: f32) {
        if !self.enabled || self.bond_strength <= 0.0 {
            return;
        }
        self.just_fused = false;
        self.just_fractured = false;
        let prev = self.bond_strength;
        self.bond_strength = (self.bond_strength - amount).max(0.0);
        if self.bond_strength <= 0.0 && prev > 0.0 {
            self.just_fractured = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.bond_strength >= self.max_bond_strength {
            return;
        }
        self.fuse(self.fuse_rate * dt);
    }

    pub fn is_fused(&self) -> bool {
        self.enabled && self.bond_strength >= self.max_bond_strength
    }

    pub fn is_fractured(&self) -> bool {
        self.bond_strength <= 0.0
    }

    pub fn bond_fraction(&self) -> f32 {
        if self.max_bond_strength <= 0.0 {
            return 0.0;
        }
        self.bond_strength / self.max_bond_strength
    }

    pub fn effective_joint(&self, scale: f32) -> f32 {
        self.bond_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn welder() -> Welder {
        Welder {
            bond_strength: 0.0,
            max_bond_strength: 100.0,
            fuse_rate: 10.0,
            just_fused: false,
            just_fractured: false,
            enabled: true,
        }
    }

    #[test]
    fn default_bond_strength_zero() {
        let w = Welder::default();
        assert_eq!(w.bond_strength, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Welder::default().enabled);
    }

    #[test]
    fn fuse_increases_bond_strength() {
        let mut w = welder();
        w.fuse(30.0);
        assert_eq!(w.bond_strength, 30.0);
    }

    #[test]
    fn fuse_clamps_at_max() {
        let mut w = welder();
        w.fuse(200.0);
        assert_eq!(w.bond_strength, 100.0);
    }

    #[test]
    fn fuse_no_op_when_disabled() {
        let mut w = welder();
        w.enabled = false;
        w.fuse(50.0);
        assert_eq!(w.bond_strength, 0.0);
    }

    #[test]
    fn fuse_sets_just_fused_at_max() {
        let mut w = welder();
        w.fuse(100.0);
        assert!(w.just_fused);
    }

    #[test]
    fn fuse_no_just_fused_if_already_max() {
        let mut w = welder();
        w.bond_strength = 100.0;
        w.fuse(1.0);
        assert!(!w.just_fused);
    }

    #[test]
    fn fracture_decreases_bond_strength() {
        let mut w = welder();
        w.bond_strength = 60.0;
        w.fracture(20.0);
        assert_eq!(w.bond_strength, 40.0);
    }

    #[test]
    fn fracture_clamps_at_zero() {
        let mut w = welder();
        w.bond_strength = 30.0;
        w.fracture(200.0);
        assert_eq!(w.bond_strength, 0.0);
    }

    #[test]
    fn fracture_no_op_when_disabled() {
        let mut w = welder();
        w.bond_strength = 50.0;
        w.enabled = false;
        w.fracture(10.0);
        assert_eq!(w.bond_strength, 50.0);
    }

    #[test]
    fn fracture_no_op_when_already_fractured() {
        let mut w = welder();
        w.fracture(10.0);
        assert_eq!(w.bond_strength, 0.0);
    }

    #[test]
    fn fracture_sets_just_fractured_at_zero() {
        let mut w = welder();
        w.bond_strength = 10.0;
        w.fracture(10.0);
        assert!(w.just_fractured);
    }

    #[test]
    fn fracture_no_just_fractured_if_already_zero() {
        let mut w = welder();
        w.fracture(1.0);
        assert!(!w.just_fractured);
    }

    #[test]
    fn tick_increases_bond_strength() {
        let mut w = welder();
        w.tick(1.0);
        assert_eq!(w.bond_strength, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = welder();
        w.tick(2.0);
        assert_eq!(w.bond_strength, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = welder();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.bond_strength, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_fused() {
        let mut w = welder();
        w.bond_strength = 100.0;
        w.tick(1.0);
        assert_eq!(w.bond_strength, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = welder();
        w.fuse_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.bond_strength, 0.0);
    }

    #[test]
    fn is_fused_true_at_max() {
        let mut w = welder();
        w.bond_strength = 100.0;
        assert!(w.is_fused());
    }

    #[test]
    fn is_fused_false_below_max() {
        let mut w = welder();
        w.bond_strength = 50.0;
        assert!(!w.is_fused());
    }

    #[test]
    fn is_fused_false_when_disabled() {
        let mut w = welder();
        w.bond_strength = 100.0;
        w.enabled = false;
        assert!(!w.is_fused());
    }

    #[test]
    fn is_fractured_true_at_zero() {
        let w = welder();
        assert!(w.is_fractured());
    }

    #[test]
    fn is_fractured_false_above_zero() {
        let mut w = welder();
        w.bond_strength = 1.0;
        assert!(!w.is_fractured());
    }

    #[test]
    fn bond_fraction_zero_when_fractured() {
        let w = welder();
        assert_eq!(w.bond_fraction(), 0.0);
    }

    #[test]
    fn bond_fraction_one_at_max() {
        let mut w = welder();
        w.bond_strength = 100.0;
        assert_eq!(w.bond_fraction(), 1.0);
    }

    #[test]
    fn bond_fraction_half_at_midpoint() {
        let mut w = welder();
        w.bond_strength = 50.0;
        assert_eq!(w.bond_fraction(), 0.5);
    }

    #[test]
    fn bond_fraction_zero_when_max_zero() {
        let mut w = welder();
        w.max_bond_strength = 0.0;
        assert_eq!(w.bond_fraction(), 0.0);
    }

    #[test]
    fn effective_joint_scales() {
        let mut w = welder();
        w.bond_strength = 50.0;
        assert_eq!(w.effective_joint(2.0), 1.0);
    }

    #[test]
    fn effective_joint_zero_when_fractured() {
        let w = welder();
        assert_eq!(w.effective_joint(10.0), 0.0);
    }

    #[test]
    fn just_fused_cleared_on_next_fuse() {
        let mut w = welder();
        w.fuse(100.0);
        assert!(w.just_fused);
        w.fuse(1.0);
        assert!(!w.just_fused);
    }

    #[test]
    fn just_fractured_cleared_on_next_fracture() {
        let mut w = welder();
        w.bond_strength = 10.0;
        w.fracture(10.0);
        assert!(w.just_fractured);
        w.bond_strength = 10.0;
        w.fracture(1.0);
        assert!(!w.just_fractured);
    }
}

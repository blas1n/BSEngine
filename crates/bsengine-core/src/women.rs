use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Women {
    pub solidarity: f32,
    pub max_solidarity: f32,
    pub bond_rate: f32,
    pub just_bonded: bool,
    pub just_severed: bool,
    pub enabled: bool,
}

impl Default for Women {
    fn default() -> Self {
        Self {
            solidarity: 0.0,
            max_solidarity: 100.0,
            bond_rate: 1.0,
            just_bonded: false,
            just_severed: false,
            enabled: true,
        }
    }
}

impl Women {
    pub fn bond(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_bonded = false;
        self.just_severed = false;
        let prev = self.solidarity;
        self.solidarity = (self.solidarity + amount).clamp(0.0, self.max_solidarity);
        if self.solidarity >= self.max_solidarity && prev < self.max_solidarity {
            self.just_bonded = true;
        }
    }

    pub fn sever(&mut self, amount: f32) {
        if !self.enabled || self.solidarity <= 0.0 {
            return;
        }
        self.just_bonded = false;
        self.just_severed = false;
        let prev = self.solidarity;
        self.solidarity = (self.solidarity - amount).max(0.0);
        if self.solidarity <= 0.0 && prev > 0.0 {
            self.just_severed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.solidarity >= self.max_solidarity {
            return;
        }
        self.bond(self.bond_rate * dt);
    }

    pub fn is_bonded(&self) -> bool {
        self.enabled && self.solidarity >= self.max_solidarity
    }

    pub fn is_severed(&self) -> bool {
        self.solidarity <= 0.0
    }

    pub fn solidarity_fraction(&self) -> f32 {
        if self.max_solidarity <= 0.0 {
            return 0.0;
        }
        self.solidarity / self.max_solidarity
    }

    pub fn effective_unity(&self, scale: f32) -> f32 {
        self.solidarity_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn women() -> Women {
        Women {
            solidarity: 0.0,
            max_solidarity: 100.0,
            bond_rate: 10.0,
            just_bonded: false,
            just_severed: false,
            enabled: true,
        }
    }

    #[test]
    fn default_solidarity_zero() {
        let w = Women::default();
        assert_eq!(w.solidarity, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Women::default().enabled);
    }

    #[test]
    fn bond_increases_solidarity() {
        let mut w = women();
        w.bond(30.0);
        assert_eq!(w.solidarity, 30.0);
    }

    #[test]
    fn bond_clamps_at_max() {
        let mut w = women();
        w.bond(200.0);
        assert_eq!(w.solidarity, 100.0);
    }

    #[test]
    fn bond_no_op_when_disabled() {
        let mut w = women();
        w.enabled = false;
        w.bond(50.0);
        assert_eq!(w.solidarity, 0.0);
    }

    #[test]
    fn bond_sets_just_bonded_at_max() {
        let mut w = women();
        w.bond(100.0);
        assert!(w.just_bonded);
    }

    #[test]
    fn bond_no_just_bonded_if_already_max() {
        let mut w = women();
        w.solidarity = 100.0;
        w.bond(1.0);
        assert!(!w.just_bonded);
    }

    #[test]
    fn sever_decreases_solidarity() {
        let mut w = women();
        w.solidarity = 60.0;
        w.sever(20.0);
        assert_eq!(w.solidarity, 40.0);
    }

    #[test]
    fn sever_clamps_at_zero() {
        let mut w = women();
        w.solidarity = 30.0;
        w.sever(200.0);
        assert_eq!(w.solidarity, 0.0);
    }

    #[test]
    fn sever_no_op_when_disabled() {
        let mut w = women();
        w.solidarity = 50.0;
        w.enabled = false;
        w.sever(10.0);
        assert_eq!(w.solidarity, 50.0);
    }

    #[test]
    fn sever_no_op_when_already_severed() {
        let mut w = women();
        w.sever(10.0);
        assert_eq!(w.solidarity, 0.0);
    }

    #[test]
    fn sever_sets_just_severed_at_zero() {
        let mut w = women();
        w.solidarity = 10.0;
        w.sever(10.0);
        assert!(w.just_severed);
    }

    #[test]
    fn sever_no_just_severed_if_already_zero() {
        let mut w = women();
        w.sever(1.0);
        assert!(!w.just_severed);
    }

    #[test]
    fn tick_increases_solidarity() {
        let mut w = women();
        w.tick(1.0);
        assert_eq!(w.solidarity, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = women();
        w.tick(2.0);
        assert_eq!(w.solidarity, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = women();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.solidarity, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_bonded() {
        let mut w = women();
        w.solidarity = 100.0;
        w.tick(1.0);
        assert_eq!(w.solidarity, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = women();
        w.bond_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.solidarity, 0.0);
    }

    #[test]
    fn is_bonded_true_at_max() {
        let mut w = women();
        w.solidarity = 100.0;
        assert!(w.is_bonded());
    }

    #[test]
    fn is_bonded_false_below_max() {
        let mut w = women();
        w.solidarity = 50.0;
        assert!(!w.is_bonded());
    }

    #[test]
    fn is_bonded_false_when_disabled() {
        let mut w = women();
        w.solidarity = 100.0;
        w.enabled = false;
        assert!(!w.is_bonded());
    }

    #[test]
    fn is_severed_true_at_zero() {
        let w = women();
        assert!(w.is_severed());
    }

    #[test]
    fn is_severed_false_above_zero() {
        let mut w = women();
        w.solidarity = 1.0;
        assert!(!w.is_severed());
    }

    #[test]
    fn solidarity_fraction_zero_when_severed() {
        let w = women();
        assert_eq!(w.solidarity_fraction(), 0.0);
    }

    #[test]
    fn solidarity_fraction_one_at_max() {
        let mut w = women();
        w.solidarity = 100.0;
        assert_eq!(w.solidarity_fraction(), 1.0);
    }

    #[test]
    fn solidarity_fraction_half_at_midpoint() {
        let mut w = women();
        w.solidarity = 50.0;
        assert_eq!(w.solidarity_fraction(), 0.5);
    }

    #[test]
    fn solidarity_fraction_zero_when_max_zero() {
        let mut w = women();
        w.max_solidarity = 0.0;
        assert_eq!(w.solidarity_fraction(), 0.0);
    }

    #[test]
    fn effective_unity_scales() {
        let mut w = women();
        w.solidarity = 50.0;
        assert_eq!(w.effective_unity(2.0), 1.0);
    }

    #[test]
    fn effective_unity_zero_when_severed() {
        let w = women();
        assert_eq!(w.effective_unity(10.0), 0.0);
    }

    #[test]
    fn just_bonded_cleared_on_next_bond() {
        let mut w = women();
        w.bond(100.0);
        assert!(w.just_bonded);
        w.bond(1.0);
        assert!(!w.just_bonded);
    }

    #[test]
    fn just_severed_cleared_on_next_sever() {
        let mut w = women();
        w.solidarity = 10.0;
        w.sever(10.0);
        assert!(w.just_severed);
        w.solidarity = 10.0;
        w.sever(1.0);
        assert!(!w.just_severed);
    }
}

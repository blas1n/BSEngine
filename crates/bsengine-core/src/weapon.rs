use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weapon {
    pub lethality: f32,
    pub max_lethality: f32,
    pub hone_rate: f32,
    pub just_honed: bool,
    pub just_dulled: bool,
    pub enabled: bool,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            lethality: 0.0,
            max_lethality: 100.0,
            hone_rate: 1.0,
            just_honed: false,
            just_dulled: false,
            enabled: true,
        }
    }
}

impl Weapon {
    pub fn hone(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_honed = false;
        self.just_dulled = false;
        let prev = self.lethality;
        self.lethality = (self.lethality + amount).clamp(0.0, self.max_lethality);
        if self.lethality >= self.max_lethality && prev < self.max_lethality {
            self.just_honed = true;
        }
    }

    pub fn dull(&mut self, amount: f32) {
        if !self.enabled || self.lethality <= 0.0 {
            return;
        }
        self.just_honed = false;
        self.just_dulled = false;
        let prev = self.lethality;
        self.lethality = (self.lethality - amount).max(0.0);
        if self.lethality <= 0.0 && prev > 0.0 {
            self.just_dulled = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.lethality >= self.max_lethality {
            return;
        }
        self.hone(self.hone_rate * dt);
    }

    pub fn is_honed(&self) -> bool {
        self.enabled && self.lethality >= self.max_lethality
    }

    pub fn is_dulled(&self) -> bool {
        self.lethality <= 0.0
    }

    pub fn lethality_fraction(&self) -> f32 {
        if self.max_lethality <= 0.0 {
            return 0.0;
        }
        self.lethality / self.max_lethality
    }

    pub fn effective_damage(&self, scale: f32) -> f32 {
        self.lethality_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weapon() -> Weapon {
        Weapon {
            lethality: 0.0,
            max_lethality: 100.0,
            hone_rate: 10.0,
            just_honed: false,
            just_dulled: false,
            enabled: true,
        }
    }

    #[test]
    fn default_lethality_zero() {
        let w = Weapon::default();
        assert_eq!(w.lethality, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Weapon::default().enabled);
    }

    #[test]
    fn hone_increases_lethality() {
        let mut w = weapon();
        w.hone(30.0);
        assert_eq!(w.lethality, 30.0);
    }

    #[test]
    fn hone_clamps_at_max() {
        let mut w = weapon();
        w.hone(200.0);
        assert_eq!(w.lethality, 100.0);
    }

    #[test]
    fn hone_no_op_when_disabled() {
        let mut w = weapon();
        w.enabled = false;
        w.hone(50.0);
        assert_eq!(w.lethality, 0.0);
    }

    #[test]
    fn hone_sets_just_honed_at_max() {
        let mut w = weapon();
        w.hone(100.0);
        assert!(w.just_honed);
    }

    #[test]
    fn hone_no_just_honed_if_already_max() {
        let mut w = weapon();
        w.lethality = 100.0;
        w.hone(1.0);
        assert!(!w.just_honed);
    }

    #[test]
    fn dull_decreases_lethality() {
        let mut w = weapon();
        w.lethality = 60.0;
        w.dull(20.0);
        assert_eq!(w.lethality, 40.0);
    }

    #[test]
    fn dull_clamps_at_zero() {
        let mut w = weapon();
        w.lethality = 30.0;
        w.dull(200.0);
        assert_eq!(w.lethality, 0.0);
    }

    #[test]
    fn dull_no_op_when_disabled() {
        let mut w = weapon();
        w.lethality = 50.0;
        w.enabled = false;
        w.dull(10.0);
        assert_eq!(w.lethality, 50.0);
    }

    #[test]
    fn dull_no_op_when_already_dulled() {
        let mut w = weapon();
        w.dull(10.0);
        assert_eq!(w.lethality, 0.0);
    }

    #[test]
    fn dull_sets_just_dulled_at_zero() {
        let mut w = weapon();
        w.lethality = 10.0;
        w.dull(10.0);
        assert!(w.just_dulled);
    }

    #[test]
    fn dull_no_just_dulled_if_already_zero() {
        let mut w = weapon();
        w.dull(1.0);
        assert!(!w.just_dulled);
    }

    #[test]
    fn tick_increases_lethality() {
        let mut w = weapon();
        w.tick(1.0);
        assert_eq!(w.lethality, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = weapon();
        w.tick(2.0);
        assert_eq!(w.lethality, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = weapon();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.lethality, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_honed() {
        let mut w = weapon();
        w.lethality = 100.0;
        w.tick(1.0);
        assert_eq!(w.lethality, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = weapon();
        w.hone_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.lethality, 0.0);
    }

    #[test]
    fn is_honed_true_at_max() {
        let mut w = weapon();
        w.lethality = 100.0;
        assert!(w.is_honed());
    }

    #[test]
    fn is_honed_false_below_max() {
        let mut w = weapon();
        w.lethality = 50.0;
        assert!(!w.is_honed());
    }

    #[test]
    fn is_honed_false_when_disabled() {
        let mut w = weapon();
        w.lethality = 100.0;
        w.enabled = false;
        assert!(!w.is_honed());
    }

    #[test]
    fn is_dulled_true_at_zero() {
        let w = weapon();
        assert!(w.is_dulled());
    }

    #[test]
    fn is_dulled_false_above_zero() {
        let mut w = weapon();
        w.lethality = 1.0;
        assert!(!w.is_dulled());
    }

    #[test]
    fn lethality_fraction_zero_when_dulled() {
        let w = weapon();
        assert_eq!(w.lethality_fraction(), 0.0);
    }

    #[test]
    fn lethality_fraction_one_at_max() {
        let mut w = weapon();
        w.lethality = 100.0;
        assert_eq!(w.lethality_fraction(), 1.0);
    }

    #[test]
    fn lethality_fraction_half_at_midpoint() {
        let mut w = weapon();
        w.lethality = 50.0;
        assert_eq!(w.lethality_fraction(), 0.5);
    }

    #[test]
    fn lethality_fraction_zero_when_max_zero() {
        let mut w = weapon();
        w.max_lethality = 0.0;
        assert_eq!(w.lethality_fraction(), 0.0);
    }

    #[test]
    fn effective_damage_scales() {
        let mut w = weapon();
        w.lethality = 50.0;
        assert_eq!(w.effective_damage(2.0), 1.0);
    }

    #[test]
    fn effective_damage_zero_when_dulled() {
        let w = weapon();
        assert_eq!(w.effective_damage(10.0), 0.0);
    }

    #[test]
    fn just_honed_cleared_on_next_hone() {
        let mut w = weapon();
        w.hone(100.0);
        assert!(w.just_honed);
        w.hone(1.0);
        assert!(!w.just_honed);
    }

    #[test]
    fn just_dulled_cleared_on_next_dull() {
        let mut w = weapon();
        w.lethality = 10.0;
        w.dull(10.0);
        assert!(w.just_dulled);
        w.lethality = 10.0;
        w.dull(1.0);
        assert!(!w.just_dulled);
    }
}

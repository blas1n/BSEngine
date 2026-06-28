use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wonder {
    pub awe: f32,
    pub max_awe: f32,
    pub marvel_rate: f32,
    pub just_awed: bool,
    pub just_jaded: bool,
    pub enabled: bool,
}

impl Default for Wonder {
    fn default() -> Self {
        Self {
            awe: 0.0,
            max_awe: 100.0,
            marvel_rate: 1.0,
            just_awed: false,
            just_jaded: false,
            enabled: true,
        }
    }
}

impl Wonder {
    pub fn marvel(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_awed = false;
        self.just_jaded = false;
        let prev = self.awe;
        self.awe = (self.awe + amount).clamp(0.0, self.max_awe);
        if self.awe >= self.max_awe && prev < self.max_awe {
            self.just_awed = true;
        }
    }

    pub fn jade(&mut self, amount: f32) {
        if !self.enabled || self.awe <= 0.0 {
            return;
        }
        self.just_awed = false;
        self.just_jaded = false;
        let prev = self.awe;
        self.awe = (self.awe - amount).max(0.0);
        if self.awe <= 0.0 && prev > 0.0 {
            self.just_jaded = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.awe >= self.max_awe {
            return;
        }
        self.marvel(self.marvel_rate * dt);
    }

    pub fn is_awed(&self) -> bool {
        self.enabled && self.awe >= self.max_awe
    }

    pub fn is_jaded(&self) -> bool {
        self.awe <= 0.0
    }

    pub fn awe_fraction(&self) -> f32 {
        if self.max_awe <= 0.0 {
            return 0.0;
        }
        self.awe / self.max_awe
    }

    pub fn effective_amazement(&self, scale: f32) -> f32 {
        self.awe_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wonder() -> Wonder {
        Wonder {
            awe: 0.0,
            max_awe: 100.0,
            marvel_rate: 10.0,
            just_awed: false,
            just_jaded: false,
            enabled: true,
        }
    }

    #[test]
    fn default_awe_zero() {
        let w = Wonder::default();
        assert_eq!(w.awe, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wonder::default().enabled);
    }

    #[test]
    fn marvel_increases_awe() {
        let mut w = wonder();
        w.marvel(30.0);
        assert_eq!(w.awe, 30.0);
    }

    #[test]
    fn marvel_clamps_at_max() {
        let mut w = wonder();
        w.marvel(200.0);
        assert_eq!(w.awe, 100.0);
    }

    #[test]
    fn marvel_no_op_when_disabled() {
        let mut w = wonder();
        w.enabled = false;
        w.marvel(50.0);
        assert_eq!(w.awe, 0.0);
    }

    #[test]
    fn marvel_sets_just_awed_at_max() {
        let mut w = wonder();
        w.marvel(100.0);
        assert!(w.just_awed);
    }

    #[test]
    fn marvel_no_just_awed_if_already_max() {
        let mut w = wonder();
        w.awe = 100.0;
        w.marvel(1.0);
        assert!(!w.just_awed);
    }

    #[test]
    fn jade_decreases_awe() {
        let mut w = wonder();
        w.awe = 60.0;
        w.jade(20.0);
        assert_eq!(w.awe, 40.0);
    }

    #[test]
    fn jade_clamps_at_zero() {
        let mut w = wonder();
        w.awe = 30.0;
        w.jade(200.0);
        assert_eq!(w.awe, 0.0);
    }

    #[test]
    fn jade_no_op_when_disabled() {
        let mut w = wonder();
        w.awe = 50.0;
        w.enabled = false;
        w.jade(10.0);
        assert_eq!(w.awe, 50.0);
    }

    #[test]
    fn jade_no_op_when_already_jaded() {
        let mut w = wonder();
        w.jade(10.0);
        assert_eq!(w.awe, 0.0);
    }

    #[test]
    fn jade_sets_just_jaded_at_zero() {
        let mut w = wonder();
        w.awe = 10.0;
        w.jade(10.0);
        assert!(w.just_jaded);
    }

    #[test]
    fn jade_no_just_jaded_if_already_zero() {
        let mut w = wonder();
        w.jade(1.0);
        assert!(!w.just_jaded);
    }

    #[test]
    fn tick_increases_awe() {
        let mut w = wonder();
        w.tick(1.0);
        assert_eq!(w.awe, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wonder();
        w.tick(2.0);
        assert_eq!(w.awe, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wonder();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.awe, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_awed() {
        let mut w = wonder();
        w.awe = 100.0;
        w.tick(1.0);
        assert_eq!(w.awe, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wonder();
        w.marvel_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.awe, 0.0);
    }

    #[test]
    fn is_awed_true_at_max() {
        let mut w = wonder();
        w.awe = 100.0;
        assert!(w.is_awed());
    }

    #[test]
    fn is_awed_false_below_max() {
        let mut w = wonder();
        w.awe = 50.0;
        assert!(!w.is_awed());
    }

    #[test]
    fn is_awed_false_when_disabled() {
        let mut w = wonder();
        w.awe = 100.0;
        w.enabled = false;
        assert!(!w.is_awed());
    }

    #[test]
    fn is_jaded_true_at_zero() {
        let w = wonder();
        assert!(w.is_jaded());
    }

    #[test]
    fn is_jaded_false_above_zero() {
        let mut w = wonder();
        w.awe = 1.0;
        assert!(!w.is_jaded());
    }

    #[test]
    fn awe_fraction_zero_when_jaded() {
        let w = wonder();
        assert_eq!(w.awe_fraction(), 0.0);
    }

    #[test]
    fn awe_fraction_one_at_max() {
        let mut w = wonder();
        w.awe = 100.0;
        assert_eq!(w.awe_fraction(), 1.0);
    }

    #[test]
    fn awe_fraction_half_at_midpoint() {
        let mut w = wonder();
        w.awe = 50.0;
        assert_eq!(w.awe_fraction(), 0.5);
    }

    #[test]
    fn awe_fraction_zero_when_max_zero() {
        let mut w = wonder();
        w.max_awe = 0.0;
        assert_eq!(w.awe_fraction(), 0.0);
    }

    #[test]
    fn effective_amazement_scales() {
        let mut w = wonder();
        w.awe = 50.0;
        assert_eq!(w.effective_amazement(2.0), 1.0);
    }

    #[test]
    fn effective_amazement_zero_when_jaded() {
        let w = wonder();
        assert_eq!(w.effective_amazement(10.0), 0.0);
    }

    #[test]
    fn just_awed_cleared_on_next_marvel() {
        let mut w = wonder();
        w.marvel(100.0);
        assert!(w.just_awed);
        w.marvel(1.0);
        assert!(!w.just_awed);
    }

    #[test]
    fn just_jaded_cleared_on_next_jade() {
        let mut w = wonder();
        w.awe = 10.0;
        w.jade(10.0);
        assert!(w.just_jaded);
        w.awe = 10.0;
        w.jade(1.0);
        assert!(!w.just_jaded);
    }
}

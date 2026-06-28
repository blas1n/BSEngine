use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Welkin {
    pub expanse: f32,
    pub max_expanse: f32,
    pub vault_rate: f32,
    pub just_heavens: bool,
    pub just_zenith: bool,
    pub enabled: bool,
}

impl Default for Welkin {
    fn default() -> Self {
        Self {
            expanse: 0.0,
            max_expanse: 100.0,
            vault_rate: 1.0,
            just_heavens: false,
            just_zenith: false,
            enabled: true,
        }
    }
}

impl Welkin {
    pub fn vault(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_heavens = false;
        self.just_zenith = false;
        let prev = self.expanse;
        self.expanse = (self.expanse + amount).clamp(0.0, self.max_expanse);
        if self.expanse >= self.max_expanse && prev < self.max_expanse {
            self.just_heavens = true;
        }
    }

    pub fn descend(&mut self, amount: f32) {
        if !self.enabled || self.expanse <= 0.0 {
            return;
        }
        self.just_heavens = false;
        self.just_zenith = false;
        let prev = self.expanse;
        self.expanse = (self.expanse - amount).max(0.0);
        if self.expanse <= 0.0 && prev > 0.0 {
            self.just_zenith = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.expanse >= self.max_expanse {
            return;
        }
        self.vault(self.vault_rate * dt);
    }

    pub fn is_heavens(&self) -> bool {
        self.enabled && self.expanse >= self.max_expanse
    }

    pub fn is_zenith(&self) -> bool {
        self.expanse <= 0.0
    }

    pub fn expanse_fraction(&self) -> f32 {
        if self.max_expanse <= 0.0 {
            return 0.0;
        }
        self.expanse / self.max_expanse
    }

    pub fn effective_sky(&self, scale: f32) -> f32 {
        self.expanse_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn welkin() -> Welkin {
        Welkin {
            expanse: 0.0,
            max_expanse: 100.0,
            vault_rate: 10.0,
            just_heavens: false,
            just_zenith: false,
            enabled: true,
        }
    }

    #[test]
    fn default_expanse_zero() {
        let w = Welkin::default();
        assert_eq!(w.expanse, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Welkin::default().enabled);
    }

    #[test]
    fn vault_increases_expanse() {
        let mut w = welkin();
        w.vault(30.0);
        assert_eq!(w.expanse, 30.0);
    }

    #[test]
    fn vault_clamps_at_max() {
        let mut w = welkin();
        w.vault(200.0);
        assert_eq!(w.expanse, 100.0);
    }

    #[test]
    fn vault_no_op_when_disabled() {
        let mut w = welkin();
        w.enabled = false;
        w.vault(50.0);
        assert_eq!(w.expanse, 0.0);
    }

    #[test]
    fn vault_sets_just_heavens_at_max() {
        let mut w = welkin();
        w.vault(100.0);
        assert!(w.just_heavens);
    }

    #[test]
    fn vault_no_just_heavens_if_already_max() {
        let mut w = welkin();
        w.expanse = 100.0;
        w.vault(1.0);
        assert!(!w.just_heavens);
    }

    #[test]
    fn descend_decreases_expanse() {
        let mut w = welkin();
        w.expanse = 60.0;
        w.descend(20.0);
        assert_eq!(w.expanse, 40.0);
    }

    #[test]
    fn descend_clamps_at_zero() {
        let mut w = welkin();
        w.expanse = 30.0;
        w.descend(200.0);
        assert_eq!(w.expanse, 0.0);
    }

    #[test]
    fn descend_no_op_when_disabled() {
        let mut w = welkin();
        w.expanse = 50.0;
        w.enabled = false;
        w.descend(10.0);
        assert_eq!(w.expanse, 50.0);
    }

    #[test]
    fn descend_no_op_when_already_zenith() {
        let mut w = welkin();
        w.descend(10.0);
        assert_eq!(w.expanse, 0.0);
    }

    #[test]
    fn descend_sets_just_zenith_at_zero() {
        let mut w = welkin();
        w.expanse = 10.0;
        w.descend(10.0);
        assert!(w.just_zenith);
    }

    #[test]
    fn descend_no_just_zenith_if_already_zero() {
        let mut w = welkin();
        w.descend(1.0);
        assert!(!w.just_zenith);
    }

    #[test]
    fn tick_increases_expanse() {
        let mut w = welkin();
        w.tick(1.0);
        assert_eq!(w.expanse, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = welkin();
        w.tick(2.0);
        assert_eq!(w.expanse, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = welkin();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.expanse, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_heavens() {
        let mut w = welkin();
        w.expanse = 100.0;
        w.tick(1.0);
        assert_eq!(w.expanse, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = welkin();
        w.vault_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.expanse, 0.0);
    }

    #[test]
    fn is_heavens_true_at_max() {
        let mut w = welkin();
        w.expanse = 100.0;
        assert!(w.is_heavens());
    }

    #[test]
    fn is_heavens_false_below_max() {
        let mut w = welkin();
        w.expanse = 50.0;
        assert!(!w.is_heavens());
    }

    #[test]
    fn is_heavens_false_when_disabled() {
        let mut w = welkin();
        w.expanse = 100.0;
        w.enabled = false;
        assert!(!w.is_heavens());
    }

    #[test]
    fn is_zenith_true_at_zero() {
        let w = welkin();
        assert!(w.is_zenith());
    }

    #[test]
    fn is_zenith_false_above_zero() {
        let mut w = welkin();
        w.expanse = 1.0;
        assert!(!w.is_zenith());
    }

    #[test]
    fn expanse_fraction_zero_when_zenith() {
        let w = welkin();
        assert_eq!(w.expanse_fraction(), 0.0);
    }

    #[test]
    fn expanse_fraction_one_at_max() {
        let mut w = welkin();
        w.expanse = 100.0;
        assert_eq!(w.expanse_fraction(), 1.0);
    }

    #[test]
    fn expanse_fraction_half_at_midpoint() {
        let mut w = welkin();
        w.expanse = 50.0;
        assert_eq!(w.expanse_fraction(), 0.5);
    }

    #[test]
    fn expanse_fraction_zero_when_max_zero() {
        let mut w = welkin();
        w.max_expanse = 0.0;
        assert_eq!(w.expanse_fraction(), 0.0);
    }

    #[test]
    fn effective_sky_scales() {
        let mut w = welkin();
        w.expanse = 50.0;
        assert_eq!(w.effective_sky(2.0), 1.0);
    }

    #[test]
    fn effective_sky_zero_when_zenith() {
        let w = welkin();
        assert_eq!(w.effective_sky(10.0), 0.0);
    }

    #[test]
    fn just_heavens_cleared_on_next_vault() {
        let mut w = welkin();
        w.vault(100.0);
        assert!(w.just_heavens);
        w.vault(1.0);
        assert!(!w.just_heavens);
    }

    #[test]
    fn just_zenith_cleared_on_next_descend() {
        let mut w = welkin();
        w.expanse = 10.0;
        w.descend(10.0);
        assert!(w.just_zenith);
        w.expanse = 10.0;
        w.descend(1.0);
        assert!(!w.just_zenith);
    }
}

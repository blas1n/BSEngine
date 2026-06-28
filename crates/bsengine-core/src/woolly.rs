use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Woolly {
    pub fleece: f32,
    pub max_fleece: f32,
    pub grow_rate: f32,
    pub just_shaggy: bool,
    pub just_shorn: bool,
    pub enabled: bool,
}

impl Default for Woolly {
    fn default() -> Self {
        Self {
            fleece: 0.0,
            max_fleece: 100.0,
            grow_rate: 1.0,
            just_shaggy: false,
            just_shorn: false,
            enabled: true,
        }
    }
}

impl Woolly {
    pub fn grow(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_shaggy = false;
        self.just_shorn = false;
        let prev = self.fleece;
        self.fleece = (self.fleece + amount).clamp(0.0, self.max_fleece);
        if self.fleece >= self.max_fleece && prev < self.max_fleece {
            self.just_shaggy = true;
        }
    }

    pub fn shear(&mut self, amount: f32) {
        if !self.enabled || self.fleece <= 0.0 {
            return;
        }
        self.just_shaggy = false;
        self.just_shorn = false;
        let prev = self.fleece;
        self.fleece = (self.fleece - amount).max(0.0);
        if self.fleece <= 0.0 && prev > 0.0 {
            self.just_shorn = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.fleece >= self.max_fleece {
            return;
        }
        self.grow(self.grow_rate * dt);
    }

    pub fn is_shaggy(&self) -> bool {
        self.enabled && self.fleece >= self.max_fleece
    }

    pub fn is_shorn(&self) -> bool {
        self.fleece <= 0.0
    }

    pub fn fleece_fraction(&self) -> f32 {
        if self.max_fleece <= 0.0 {
            return 0.0;
        }
        self.fleece / self.max_fleece
    }

    pub fn effective_warmth(&self, scale: f32) -> f32 {
        self.fleece_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn woolly() -> Woolly {
        Woolly {
            fleece: 0.0,
            max_fleece: 100.0,
            grow_rate: 10.0,
            just_shaggy: false,
            just_shorn: false,
            enabled: true,
        }
    }

    #[test]
    fn default_fleece_zero() {
        let w = Woolly::default();
        assert_eq!(w.fleece, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Woolly::default().enabled);
    }

    #[test]
    fn grow_increases_fleece() {
        let mut w = woolly();
        w.grow(30.0);
        assert_eq!(w.fleece, 30.0);
    }

    #[test]
    fn grow_clamps_at_max() {
        let mut w = woolly();
        w.grow(200.0);
        assert_eq!(w.fleece, 100.0);
    }

    #[test]
    fn grow_no_op_when_disabled() {
        let mut w = woolly();
        w.enabled = false;
        w.grow(50.0);
        assert_eq!(w.fleece, 0.0);
    }

    #[test]
    fn grow_sets_just_shaggy_at_max() {
        let mut w = woolly();
        w.grow(100.0);
        assert!(w.just_shaggy);
    }

    #[test]
    fn grow_no_just_shaggy_if_already_max() {
        let mut w = woolly();
        w.fleece = 100.0;
        w.grow(1.0);
        assert!(!w.just_shaggy);
    }

    #[test]
    fn shear_decreases_fleece() {
        let mut w = woolly();
        w.fleece = 60.0;
        w.shear(20.0);
        assert_eq!(w.fleece, 40.0);
    }

    #[test]
    fn shear_clamps_at_zero() {
        let mut w = woolly();
        w.fleece = 30.0;
        w.shear(200.0);
        assert_eq!(w.fleece, 0.0);
    }

    #[test]
    fn shear_no_op_when_disabled() {
        let mut w = woolly();
        w.fleece = 50.0;
        w.enabled = false;
        w.shear(10.0);
        assert_eq!(w.fleece, 50.0);
    }

    #[test]
    fn shear_no_op_when_already_shorn() {
        let mut w = woolly();
        w.shear(10.0);
        assert_eq!(w.fleece, 0.0);
    }

    #[test]
    fn shear_sets_just_shorn_at_zero() {
        let mut w = woolly();
        w.fleece = 10.0;
        w.shear(10.0);
        assert!(w.just_shorn);
    }

    #[test]
    fn shear_no_just_shorn_if_already_zero() {
        let mut w = woolly();
        w.shear(1.0);
        assert!(!w.just_shorn);
    }

    #[test]
    fn tick_increases_fleece() {
        let mut w = woolly();
        w.tick(1.0);
        assert_eq!(w.fleece, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = woolly();
        w.tick(2.0);
        assert_eq!(w.fleece, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = woolly();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.fleece, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_shaggy() {
        let mut w = woolly();
        w.fleece = 100.0;
        w.tick(1.0);
        assert_eq!(w.fleece, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = woolly();
        w.grow_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.fleece, 0.0);
    }

    #[test]
    fn is_shaggy_true_at_max() {
        let mut w = woolly();
        w.fleece = 100.0;
        assert!(w.is_shaggy());
    }

    #[test]
    fn is_shaggy_false_below_max() {
        let mut w = woolly();
        w.fleece = 50.0;
        assert!(!w.is_shaggy());
    }

    #[test]
    fn is_shaggy_false_when_disabled() {
        let mut w = woolly();
        w.fleece = 100.0;
        w.enabled = false;
        assert!(!w.is_shaggy());
    }

    #[test]
    fn is_shorn_true_at_zero() {
        let w = woolly();
        assert!(w.is_shorn());
    }

    #[test]
    fn is_shorn_false_above_zero() {
        let mut w = woolly();
        w.fleece = 1.0;
        assert!(!w.is_shorn());
    }

    #[test]
    fn fleece_fraction_zero_when_shorn() {
        let w = woolly();
        assert_eq!(w.fleece_fraction(), 0.0);
    }

    #[test]
    fn fleece_fraction_one_at_max() {
        let mut w = woolly();
        w.fleece = 100.0;
        assert_eq!(w.fleece_fraction(), 1.0);
    }

    #[test]
    fn fleece_fraction_half_at_midpoint() {
        let mut w = woolly();
        w.fleece = 50.0;
        assert_eq!(w.fleece_fraction(), 0.5);
    }

    #[test]
    fn fleece_fraction_zero_when_max_zero() {
        let mut w = woolly();
        w.max_fleece = 0.0;
        assert_eq!(w.fleece_fraction(), 0.0);
    }

    #[test]
    fn effective_warmth_scales() {
        let mut w = woolly();
        w.fleece = 50.0;
        assert_eq!(w.effective_warmth(2.0), 1.0);
    }

    #[test]
    fn effective_warmth_zero_when_shorn() {
        let w = woolly();
        assert_eq!(w.effective_warmth(10.0), 0.0);
    }

    #[test]
    fn just_shaggy_cleared_on_next_grow() {
        let mut w = woolly();
        w.grow(100.0);
        assert!(w.just_shaggy);
        w.grow(1.0);
        assert!(!w.just_shaggy);
    }

    #[test]
    fn just_shorn_cleared_on_next_shear() {
        let mut w = woolly();
        w.fleece = 10.0;
        w.shear(10.0);
        assert!(w.just_shorn);
        w.fleece = 10.0;
        w.shear(1.0);
        assert!(!w.just_shorn);
    }
}

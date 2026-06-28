use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Woof {
    pub bark: f32,
    pub max_bark: f32,
    pub howl_rate: f32,
    pub just_howling: bool,
    pub just_quiet: bool,
    pub enabled: bool,
}

impl Default for Woof {
    fn default() -> Self {
        Self {
            bark: 0.0,
            max_bark: 100.0,
            howl_rate: 1.0,
            just_howling: false,
            just_quiet: false,
            enabled: true,
        }
    }
}

impl Woof {
    pub fn howl(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_howling = false;
        self.just_quiet = false;
        let prev = self.bark;
        self.bark = (self.bark + amount).clamp(0.0, self.max_bark);
        if self.bark >= self.max_bark && prev < self.max_bark {
            self.just_howling = true;
        }
    }

    pub fn hush(&mut self, amount: f32) {
        if !self.enabled || self.bark <= 0.0 {
            return;
        }
        self.just_howling = false;
        self.just_quiet = false;
        let prev = self.bark;
        self.bark = (self.bark - amount).max(0.0);
        if self.bark <= 0.0 && prev > 0.0 {
            self.just_quiet = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.bark >= self.max_bark {
            return;
        }
        self.howl(self.howl_rate * dt);
    }

    pub fn is_howling(&self) -> bool {
        self.enabled && self.bark >= self.max_bark
    }

    pub fn is_quiet(&self) -> bool {
        self.bark <= 0.0
    }

    pub fn bark_fraction(&self) -> f32 {
        if self.max_bark <= 0.0 {
            return 0.0;
        }
        self.bark / self.max_bark
    }

    pub fn effective_volume(&self, scale: f32) -> f32 {
        self.bark_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn woof() -> Woof {
        Woof {
            bark: 0.0,
            max_bark: 100.0,
            howl_rate: 10.0,
            just_howling: false,
            just_quiet: false,
            enabled: true,
        }
    }

    #[test]
    fn default_bark_zero() {
        let w = Woof::default();
        assert_eq!(w.bark, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Woof::default().enabled);
    }

    #[test]
    fn howl_increases_bark() {
        let mut w = woof();
        w.howl(30.0);
        assert_eq!(w.bark, 30.0);
    }

    #[test]
    fn howl_clamps_at_max() {
        let mut w = woof();
        w.howl(200.0);
        assert_eq!(w.bark, 100.0);
    }

    #[test]
    fn howl_no_op_when_disabled() {
        let mut w = woof();
        w.enabled = false;
        w.howl(50.0);
        assert_eq!(w.bark, 0.0);
    }

    #[test]
    fn howl_sets_just_howling_at_max() {
        let mut w = woof();
        w.howl(100.0);
        assert!(w.just_howling);
    }

    #[test]
    fn howl_no_just_howling_if_already_max() {
        let mut w = woof();
        w.bark = 100.0;
        w.howl(1.0);
        assert!(!w.just_howling);
    }

    #[test]
    fn hush_decreases_bark() {
        let mut w = woof();
        w.bark = 60.0;
        w.hush(20.0);
        assert_eq!(w.bark, 40.0);
    }

    #[test]
    fn hush_clamps_at_zero() {
        let mut w = woof();
        w.bark = 30.0;
        w.hush(200.0);
        assert_eq!(w.bark, 0.0);
    }

    #[test]
    fn hush_no_op_when_disabled() {
        let mut w = woof();
        w.bark = 50.0;
        w.enabled = false;
        w.hush(10.0);
        assert_eq!(w.bark, 50.0);
    }

    #[test]
    fn hush_no_op_when_already_quiet() {
        let mut w = woof();
        w.hush(10.0);
        assert_eq!(w.bark, 0.0);
    }

    #[test]
    fn hush_sets_just_quiet_at_zero() {
        let mut w = woof();
        w.bark = 10.0;
        w.hush(10.0);
        assert!(w.just_quiet);
    }

    #[test]
    fn hush_no_just_quiet_if_already_zero() {
        let mut w = woof();
        w.hush(1.0);
        assert!(!w.just_quiet);
    }

    #[test]
    fn tick_increases_bark() {
        let mut w = woof();
        w.tick(1.0);
        assert_eq!(w.bark, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = woof();
        w.tick(2.0);
        assert_eq!(w.bark, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = woof();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.bark, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_howling() {
        let mut w = woof();
        w.bark = 100.0;
        w.tick(1.0);
        assert_eq!(w.bark, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = woof();
        w.howl_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.bark, 0.0);
    }

    #[test]
    fn is_howling_true_at_max() {
        let mut w = woof();
        w.bark = 100.0;
        assert!(w.is_howling());
    }

    #[test]
    fn is_howling_false_below_max() {
        let mut w = woof();
        w.bark = 50.0;
        assert!(!w.is_howling());
    }

    #[test]
    fn is_howling_false_when_disabled() {
        let mut w = woof();
        w.bark = 100.0;
        w.enabled = false;
        assert!(!w.is_howling());
    }

    #[test]
    fn is_quiet_true_at_zero() {
        let w = woof();
        assert!(w.is_quiet());
    }

    #[test]
    fn is_quiet_false_above_zero() {
        let mut w = woof();
        w.bark = 1.0;
        assert!(!w.is_quiet());
    }

    #[test]
    fn bark_fraction_zero_when_quiet() {
        let w = woof();
        assert_eq!(w.bark_fraction(), 0.0);
    }

    #[test]
    fn bark_fraction_one_at_max() {
        let mut w = woof();
        w.bark = 100.0;
        assert_eq!(w.bark_fraction(), 1.0);
    }

    #[test]
    fn bark_fraction_half_at_midpoint() {
        let mut w = woof();
        w.bark = 50.0;
        assert_eq!(w.bark_fraction(), 0.5);
    }

    #[test]
    fn bark_fraction_zero_when_max_zero() {
        let mut w = woof();
        w.max_bark = 0.0;
        assert_eq!(w.bark_fraction(), 0.0);
    }

    #[test]
    fn effective_volume_scales() {
        let mut w = woof();
        w.bark = 50.0;
        assert_eq!(w.effective_volume(2.0), 1.0);
    }

    #[test]
    fn effective_volume_zero_when_quiet() {
        let w = woof();
        assert_eq!(w.effective_volume(10.0), 0.0);
    }

    #[test]
    fn just_howling_cleared_on_next_howl() {
        let mut w = woof();
        w.howl(100.0);
        assert!(w.just_howling);
        w.howl(1.0);
        assert!(!w.just_howling);
    }

    #[test]
    fn just_quiet_cleared_on_next_hush() {
        let mut w = woof();
        w.bark = 10.0;
        w.hush(10.0);
        assert!(w.just_quiet);
        w.bark = 10.0;
        w.hush(1.0);
        assert!(!w.just_quiet);
    }
}

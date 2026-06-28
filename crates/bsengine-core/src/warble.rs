use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Warble {
    pub melody: f32,
    pub max_melody: f32,
    pub trill_rate: f32,
    pub just_crescendo: bool,
    pub just_silent: bool,
    pub enabled: bool,
}

impl Default for Warble {
    fn default() -> Self {
        Self {
            melody: 0.0,
            max_melody: 100.0,
            trill_rate: 1.0,
            just_crescendo: false,
            just_silent: false,
            enabled: true,
        }
    }
}

impl Warble {
    pub fn trill(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_crescendo = false;
        self.just_silent = false;
        let prev = self.melody;
        self.melody = (self.melody + amount).clamp(0.0, self.max_melody);
        if self.melody >= self.max_melody && prev < self.max_melody {
            self.just_crescendo = true;
        }
    }

    pub fn hush(&mut self, amount: f32) {
        if !self.enabled || self.melody <= 0.0 {
            return;
        }
        self.just_crescendo = false;
        self.just_silent = false;
        let prev = self.melody;
        self.melody = (self.melody - amount).max(0.0);
        if self.melody <= 0.0 && prev > 0.0 {
            self.just_silent = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.melody >= self.max_melody {
            return;
        }
        self.trill(self.trill_rate * dt);
    }

    pub fn is_crescendo(&self) -> bool {
        self.enabled && self.melody >= self.max_melody
    }

    pub fn is_silent(&self) -> bool {
        self.melody <= 0.0
    }

    pub fn melody_fraction(&self) -> f32 {
        if self.max_melody <= 0.0 {
            return 0.0;
        }
        self.melody / self.max_melody
    }

    pub fn effective_song(&self, scale: f32) -> f32 {
        self.melody_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warble() -> Warble {
        Warble {
            melody: 0.0,
            max_melody: 100.0,
            trill_rate: 10.0,
            just_crescendo: false,
            just_silent: false,
            enabled: true,
        }
    }

    #[test]
    fn default_melody_zero() {
        let w = Warble::default();
        assert_eq!(w.melody, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Warble::default().enabled);
    }

    #[test]
    fn trill_increases_melody() {
        let mut w = warble();
        w.trill(30.0);
        assert_eq!(w.melody, 30.0);
    }

    #[test]
    fn trill_clamps_at_max() {
        let mut w = warble();
        w.trill(200.0);
        assert_eq!(w.melody, 100.0);
    }

    #[test]
    fn trill_no_op_when_disabled() {
        let mut w = warble();
        w.enabled = false;
        w.trill(50.0);
        assert_eq!(w.melody, 0.0);
    }

    #[test]
    fn trill_sets_just_crescendo_at_max() {
        let mut w = warble();
        w.trill(100.0);
        assert!(w.just_crescendo);
    }

    #[test]
    fn trill_no_just_crescendo_if_already_max() {
        let mut w = warble();
        w.melody = 100.0;
        w.trill(1.0);
        assert!(!w.just_crescendo);
    }

    #[test]
    fn hush_decreases_melody() {
        let mut w = warble();
        w.melody = 60.0;
        w.hush(20.0);
        assert_eq!(w.melody, 40.0);
    }

    #[test]
    fn hush_clamps_at_zero() {
        let mut w = warble();
        w.melody = 30.0;
        w.hush(200.0);
        assert_eq!(w.melody, 0.0);
    }

    #[test]
    fn hush_no_op_when_disabled() {
        let mut w = warble();
        w.melody = 50.0;
        w.enabled = false;
        w.hush(10.0);
        assert_eq!(w.melody, 50.0);
    }

    #[test]
    fn hush_no_op_when_already_silent() {
        let mut w = warble();
        w.hush(10.0);
        assert_eq!(w.melody, 0.0);
    }

    #[test]
    fn hush_sets_just_silent_at_zero() {
        let mut w = warble();
        w.melody = 10.0;
        w.hush(10.0);
        assert!(w.just_silent);
    }

    #[test]
    fn hush_no_just_silent_if_already_zero() {
        let mut w = warble();
        w.hush(1.0);
        assert!(!w.just_silent);
    }

    #[test]
    fn tick_increases_melody() {
        let mut w = warble();
        w.tick(1.0);
        assert_eq!(w.melody, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = warble();
        w.tick(2.0);
        assert_eq!(w.melody, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = warble();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.melody, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_crescendo() {
        let mut w = warble();
        w.melody = 100.0;
        w.tick(1.0);
        assert_eq!(w.melody, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = warble();
        w.trill_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.melody, 0.0);
    }

    #[test]
    fn is_crescendo_true_at_max() {
        let mut w = warble();
        w.melody = 100.0;
        assert!(w.is_crescendo());
    }

    #[test]
    fn is_crescendo_false_below_max() {
        let mut w = warble();
        w.melody = 50.0;
        assert!(!w.is_crescendo());
    }

    #[test]
    fn is_crescendo_false_when_disabled() {
        let mut w = warble();
        w.melody = 100.0;
        w.enabled = false;
        assert!(!w.is_crescendo());
    }

    #[test]
    fn is_silent_true_at_zero() {
        let w = warble();
        assert!(w.is_silent());
    }

    #[test]
    fn is_silent_false_above_zero() {
        let mut w = warble();
        w.melody = 1.0;
        assert!(!w.is_silent());
    }

    #[test]
    fn melody_fraction_zero_when_silent() {
        let w = warble();
        assert_eq!(w.melody_fraction(), 0.0);
    }

    #[test]
    fn melody_fraction_one_at_max() {
        let mut w = warble();
        w.melody = 100.0;
        assert_eq!(w.melody_fraction(), 1.0);
    }

    #[test]
    fn melody_fraction_half_at_midpoint() {
        let mut w = warble();
        w.melody = 50.0;
        assert_eq!(w.melody_fraction(), 0.5);
    }

    #[test]
    fn melody_fraction_zero_when_max_zero() {
        let mut w = warble();
        w.max_melody = 0.0;
        assert_eq!(w.melody_fraction(), 0.0);
    }

    #[test]
    fn effective_song_scales() {
        let mut w = warble();
        w.melody = 50.0;
        assert_eq!(w.effective_song(2.0), 1.0);
    }

    #[test]
    fn effective_song_zero_when_silent() {
        let w = warble();
        assert_eq!(w.effective_song(10.0), 0.0);
    }

    #[test]
    fn just_crescendo_cleared_on_next_trill() {
        let mut w = warble();
        w.trill(100.0);
        assert!(w.just_crescendo);
        w.trill(1.0);
        assert!(!w.just_crescendo);
    }

    #[test]
    fn just_silent_cleared_on_next_hush() {
        let mut w = warble();
        w.melody = 10.0;
        w.hush(10.0);
        assert!(w.just_silent);
        w.melody = 10.0;
        w.hush(1.0);
        assert!(!w.just_silent);
    }
}

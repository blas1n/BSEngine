use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Waist {
    pub girth: f32,
    pub max_girth: f32,
    pub expand_rate: f32,
    pub just_bloated: bool,
    pub just_trim: bool,
    pub enabled: bool,
}

impl Default for Waist {
    fn default() -> Self {
        Self {
            girth: 0.0,
            max_girth: 100.0,
            expand_rate: 1.0,
            just_bloated: false,
            just_trim: false,
            enabled: true,
        }
    }
}

impl Waist {
    pub fn expand(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_bloated = false;
        self.just_trim = false;
        let prev = self.girth;
        self.girth = (self.girth + amount).clamp(0.0, self.max_girth);
        if self.girth >= self.max_girth && prev < self.max_girth {
            self.just_bloated = true;
        }
    }

    pub fn cinch(&mut self, amount: f32) {
        if !self.enabled || self.girth <= 0.0 {
            return;
        }
        self.just_bloated = false;
        self.just_trim = false;
        let prev = self.girth;
        self.girth = (self.girth - amount).max(0.0);
        if self.girth <= 0.0 && prev > 0.0 {
            self.just_trim = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.girth >= self.max_girth {
            return;
        }
        self.expand(self.expand_rate * dt);
    }

    pub fn is_bloated(&self) -> bool {
        self.enabled && self.girth >= self.max_girth
    }

    pub fn is_trim(&self) -> bool {
        self.girth <= 0.0
    }

    pub fn girth_fraction(&self) -> f32 {
        if self.max_girth <= 0.0 {
            return 0.0;
        }
        self.girth / self.max_girth
    }

    pub fn effective_bulk(&self, scale: f32) -> f32 {
        self.girth_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waist() -> Waist {
        Waist {
            girth: 0.0,
            max_girth: 100.0,
            expand_rate: 10.0,
            just_bloated: false,
            just_trim: false,
            enabled: true,
        }
    }

    #[test]
    fn default_girth_zero() {
        let w = Waist::default();
        assert_eq!(w.girth, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Waist::default().enabled);
    }

    #[test]
    fn expand_increases_girth() {
        let mut w = waist();
        w.expand(30.0);
        assert_eq!(w.girth, 30.0);
    }

    #[test]
    fn expand_clamps_at_max() {
        let mut w = waist();
        w.expand(200.0);
        assert_eq!(w.girth, 100.0);
    }

    #[test]
    fn expand_no_op_when_disabled() {
        let mut w = waist();
        w.enabled = false;
        w.expand(50.0);
        assert_eq!(w.girth, 0.0);
    }

    #[test]
    fn expand_sets_just_bloated_at_max() {
        let mut w = waist();
        w.expand(100.0);
        assert!(w.just_bloated);
    }

    #[test]
    fn expand_no_just_bloated_if_already_max() {
        let mut w = waist();
        w.girth = 100.0;
        w.expand(1.0);
        assert!(!w.just_bloated);
    }

    #[test]
    fn cinch_decreases_girth() {
        let mut w = waist();
        w.girth = 60.0;
        w.cinch(20.0);
        assert_eq!(w.girth, 40.0);
    }

    #[test]
    fn cinch_clamps_at_zero() {
        let mut w = waist();
        w.girth = 30.0;
        w.cinch(200.0);
        assert_eq!(w.girth, 0.0);
    }

    #[test]
    fn cinch_no_op_when_disabled() {
        let mut w = waist();
        w.girth = 50.0;
        w.enabled = false;
        w.cinch(10.0);
        assert_eq!(w.girth, 50.0);
    }

    #[test]
    fn cinch_no_op_when_already_trim() {
        let mut w = waist();
        w.cinch(10.0);
        assert_eq!(w.girth, 0.0);
    }

    #[test]
    fn cinch_sets_just_trim_at_zero() {
        let mut w = waist();
        w.girth = 10.0;
        w.cinch(10.0);
        assert!(w.just_trim);
    }

    #[test]
    fn cinch_no_just_trim_if_already_zero() {
        let mut w = waist();
        w.cinch(1.0);
        assert!(!w.just_trim);
    }

    #[test]
    fn tick_increases_girth() {
        let mut w = waist();
        w.tick(1.0);
        assert_eq!(w.girth, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = waist();
        w.tick(2.0);
        assert_eq!(w.girth, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = waist();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.girth, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_bloated() {
        let mut w = waist();
        w.girth = 100.0;
        w.tick(1.0);
        assert_eq!(w.girth, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = waist();
        w.expand_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.girth, 0.0);
    }

    #[test]
    fn is_bloated_true_at_max() {
        let mut w = waist();
        w.girth = 100.0;
        assert!(w.is_bloated());
    }

    #[test]
    fn is_bloated_false_below_max() {
        let mut w = waist();
        w.girth = 50.0;
        assert!(!w.is_bloated());
    }

    #[test]
    fn is_bloated_false_when_disabled() {
        let mut w = waist();
        w.girth = 100.0;
        w.enabled = false;
        assert!(!w.is_bloated());
    }

    #[test]
    fn is_trim_true_at_zero() {
        let w = waist();
        assert!(w.is_trim());
    }

    #[test]
    fn is_trim_false_above_zero() {
        let mut w = waist();
        w.girth = 1.0;
        assert!(!w.is_trim());
    }

    #[test]
    fn girth_fraction_zero_when_trim() {
        let w = waist();
        assert_eq!(w.girth_fraction(), 0.0);
    }

    #[test]
    fn girth_fraction_one_at_max() {
        let mut w = waist();
        w.girth = 100.0;
        assert_eq!(w.girth_fraction(), 1.0);
    }

    #[test]
    fn girth_fraction_half_at_midpoint() {
        let mut w = waist();
        w.girth = 50.0;
        assert_eq!(w.girth_fraction(), 0.5);
    }

    #[test]
    fn girth_fraction_zero_when_max_zero() {
        let mut w = waist();
        w.max_girth = 0.0;
        assert_eq!(w.girth_fraction(), 0.0);
    }

    #[test]
    fn effective_bulk_scales() {
        let mut w = waist();
        w.girth = 50.0;
        assert_eq!(w.effective_bulk(2.0), 1.0);
    }

    #[test]
    fn effective_bulk_zero_when_trim() {
        let w = waist();
        assert_eq!(w.effective_bulk(10.0), 0.0);
    }

    #[test]
    fn just_bloated_cleared_on_next_expand() {
        let mut w = waist();
        w.expand(100.0);
        assert!(w.just_bloated);
        w.expand(1.0);
        assert!(!w.just_bloated);
    }

    #[test]
    fn just_trim_cleared_on_next_cinch() {
        let mut w = waist();
        w.girth = 10.0;
        w.cinch(10.0);
        assert!(w.just_trim);
        w.girth = 10.0;
        w.cinch(1.0);
        assert!(!w.just_trim);
    }
}

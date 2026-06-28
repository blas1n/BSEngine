use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Warden {
    pub vigilance: f32,
    pub max_vigilance: f32,
    pub patrol_rate: f32,
    pub just_alert: bool,
    pub just_lax: bool,
    pub enabled: bool,
}

impl Default for Warden {
    fn default() -> Self {
        Self {
            vigilance: 0.0,
            max_vigilance: 100.0,
            patrol_rate: 1.0,
            just_alert: false,
            just_lax: false,
            enabled: true,
        }
    }
}

impl Warden {
    pub fn patrol(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_alert = false;
        self.just_lax = false;
        let prev = self.vigilance;
        self.vigilance = (self.vigilance + amount).clamp(0.0, self.max_vigilance);
        if self.vigilance >= self.max_vigilance && prev < self.max_vigilance {
            self.just_alert = true;
        }
    }

    pub fn relax(&mut self, amount: f32) {
        if !self.enabled || self.vigilance <= 0.0 {
            return;
        }
        self.just_alert = false;
        self.just_lax = false;
        let prev = self.vigilance;
        self.vigilance = (self.vigilance - amount).max(0.0);
        if self.vigilance <= 0.0 && prev > 0.0 {
            self.just_lax = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.vigilance >= self.max_vigilance {
            return;
        }
        self.patrol(self.patrol_rate * dt);
    }

    pub fn is_alert(&self) -> bool {
        self.enabled && self.vigilance >= self.max_vigilance
    }

    pub fn is_lax(&self) -> bool {
        self.vigilance <= 0.0
    }

    pub fn vigilance_fraction(&self) -> f32 {
        if self.max_vigilance <= 0.0 {
            return 0.0;
        }
        self.vigilance / self.max_vigilance
    }

    pub fn effective_watch(&self, scale: f32) -> f32 {
        self.vigilance_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warden() -> Warden {
        Warden {
            vigilance: 0.0,
            max_vigilance: 100.0,
            patrol_rate: 10.0,
            just_alert: false,
            just_lax: false,
            enabled: true,
        }
    }

    #[test]
    fn default_vigilance_zero() {
        let w = Warden::default();
        assert_eq!(w.vigilance, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Warden::default().enabled);
    }

    #[test]
    fn patrol_increases_vigilance() {
        let mut w = warden();
        w.patrol(30.0);
        assert_eq!(w.vigilance, 30.0);
    }

    #[test]
    fn patrol_clamps_at_max() {
        let mut w = warden();
        w.patrol(200.0);
        assert_eq!(w.vigilance, 100.0);
    }

    #[test]
    fn patrol_no_op_when_disabled() {
        let mut w = warden();
        w.enabled = false;
        w.patrol(50.0);
        assert_eq!(w.vigilance, 0.0);
    }

    #[test]
    fn patrol_sets_just_alert_at_max() {
        let mut w = warden();
        w.patrol(100.0);
        assert!(w.just_alert);
    }

    #[test]
    fn patrol_no_just_alert_if_already_max() {
        let mut w = warden();
        w.vigilance = 100.0;
        w.patrol(1.0);
        assert!(!w.just_alert);
    }

    #[test]
    fn relax_decreases_vigilance() {
        let mut w = warden();
        w.vigilance = 60.0;
        w.relax(20.0);
        assert_eq!(w.vigilance, 40.0);
    }

    #[test]
    fn relax_clamps_at_zero() {
        let mut w = warden();
        w.vigilance = 30.0;
        w.relax(200.0);
        assert_eq!(w.vigilance, 0.0);
    }

    #[test]
    fn relax_no_op_when_disabled() {
        let mut w = warden();
        w.vigilance = 50.0;
        w.enabled = false;
        w.relax(10.0);
        assert_eq!(w.vigilance, 50.0);
    }

    #[test]
    fn relax_no_op_when_already_lax() {
        let mut w = warden();
        w.relax(10.0);
        assert_eq!(w.vigilance, 0.0);
    }

    #[test]
    fn relax_sets_just_lax_at_zero() {
        let mut w = warden();
        w.vigilance = 10.0;
        w.relax(10.0);
        assert!(w.just_lax);
    }

    #[test]
    fn relax_no_just_lax_if_already_zero() {
        let mut w = warden();
        w.relax(1.0);
        assert!(!w.just_lax);
    }

    #[test]
    fn tick_increases_vigilance() {
        let mut w = warden();
        w.tick(1.0);
        assert_eq!(w.vigilance, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = warden();
        w.tick(2.0);
        assert_eq!(w.vigilance, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = warden();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.vigilance, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_alert() {
        let mut w = warden();
        w.vigilance = 100.0;
        w.tick(1.0);
        assert_eq!(w.vigilance, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = warden();
        w.patrol_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.vigilance, 0.0);
    }

    #[test]
    fn is_alert_true_at_max() {
        let mut w = warden();
        w.vigilance = 100.0;
        assert!(w.is_alert());
    }

    #[test]
    fn is_alert_false_below_max() {
        let mut w = warden();
        w.vigilance = 50.0;
        assert!(!w.is_alert());
    }

    #[test]
    fn is_alert_false_when_disabled() {
        let mut w = warden();
        w.vigilance = 100.0;
        w.enabled = false;
        assert!(!w.is_alert());
    }

    #[test]
    fn is_lax_true_at_zero() {
        let w = warden();
        assert!(w.is_lax());
    }

    #[test]
    fn is_lax_false_above_zero() {
        let mut w = warden();
        w.vigilance = 1.0;
        assert!(!w.is_lax());
    }

    #[test]
    fn vigilance_fraction_zero_when_lax() {
        let w = warden();
        assert_eq!(w.vigilance_fraction(), 0.0);
    }

    #[test]
    fn vigilance_fraction_one_at_max() {
        let mut w = warden();
        w.vigilance = 100.0;
        assert_eq!(w.vigilance_fraction(), 1.0);
    }

    #[test]
    fn vigilance_fraction_half_at_midpoint() {
        let mut w = warden();
        w.vigilance = 50.0;
        assert_eq!(w.vigilance_fraction(), 0.5);
    }

    #[test]
    fn vigilance_fraction_zero_when_max_zero() {
        let mut w = warden();
        w.max_vigilance = 0.0;
        assert_eq!(w.vigilance_fraction(), 0.0);
    }

    #[test]
    fn effective_watch_scales() {
        let mut w = warden();
        w.vigilance = 50.0;
        assert_eq!(w.effective_watch(2.0), 1.0);
    }

    #[test]
    fn effective_watch_zero_when_lax() {
        let w = warden();
        assert_eq!(w.effective_watch(10.0), 0.0);
    }

    #[test]
    fn just_alert_cleared_on_next_patrol() {
        let mut w = warden();
        w.patrol(100.0);
        assert!(w.just_alert);
        w.patrol(1.0);
        assert!(!w.just_alert);
    }

    #[test]
    fn just_lax_cleared_on_next_relax() {
        let mut w = warden();
        w.vigilance = 10.0;
        w.relax(10.0);
        assert!(w.just_lax);
        w.vigilance = 10.0;
        w.relax(1.0);
        assert!(!w.just_lax);
    }
}

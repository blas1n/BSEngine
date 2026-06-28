use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Watery {
    pub saturation: f32,
    pub max_saturation: f32,
    pub seep_rate: f32,
    pub just_sodden: bool,
    pub just_parched: bool,
    pub enabled: bool,
}

impl Default for Watery {
    fn default() -> Self {
        Self {
            saturation: 0.0,
            max_saturation: 100.0,
            seep_rate: 1.0,
            just_sodden: false,
            just_parched: false,
            enabled: true,
        }
    }
}

impl Watery {
    pub fn seep(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_sodden = false;
        self.just_parched = false;
        let prev = self.saturation;
        self.saturation = (self.saturation + amount).clamp(0.0, self.max_saturation);
        if self.saturation >= self.max_saturation && prev < self.max_saturation {
            self.just_sodden = true;
        }
    }

    pub fn drain(&mut self, amount: f32) {
        if !self.enabled || self.saturation <= 0.0 {
            return;
        }
        self.just_sodden = false;
        self.just_parched = false;
        let prev = self.saturation;
        self.saturation = (self.saturation - amount).max(0.0);
        if self.saturation <= 0.0 && prev > 0.0 {
            self.just_parched = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.saturation >= self.max_saturation {
            return;
        }
        self.seep(self.seep_rate * dt);
    }

    pub fn is_sodden(&self) -> bool {
        self.enabled && self.saturation >= self.max_saturation
    }

    pub fn is_parched(&self) -> bool {
        self.saturation <= 0.0
    }

    pub fn saturation_fraction(&self) -> f32 {
        if self.max_saturation <= 0.0 {
            return 0.0;
        }
        self.saturation / self.max_saturation
    }

    pub fn effective_moisture(&self, scale: f32) -> f32 {
        self.saturation_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watery() -> Watery {
        Watery {
            saturation: 0.0,
            max_saturation: 100.0,
            seep_rate: 10.0,
            just_sodden: false,
            just_parched: false,
            enabled: true,
        }
    }

    #[test]
    fn default_saturation_zero() {
        let w = Watery::default();
        assert_eq!(w.saturation, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Watery::default().enabled);
    }

    #[test]
    fn seep_increases_saturation() {
        let mut w = watery();
        w.seep(30.0);
        assert_eq!(w.saturation, 30.0);
    }

    #[test]
    fn seep_clamps_at_max() {
        let mut w = watery();
        w.seep(200.0);
        assert_eq!(w.saturation, 100.0);
    }

    #[test]
    fn seep_no_op_when_disabled() {
        let mut w = watery();
        w.enabled = false;
        w.seep(50.0);
        assert_eq!(w.saturation, 0.0);
    }

    #[test]
    fn seep_sets_just_sodden_at_max() {
        let mut w = watery();
        w.seep(100.0);
        assert!(w.just_sodden);
    }

    #[test]
    fn seep_no_just_sodden_if_already_max() {
        let mut w = watery();
        w.saturation = 100.0;
        w.seep(1.0);
        assert!(!w.just_sodden);
    }

    #[test]
    fn drain_decreases_saturation() {
        let mut w = watery();
        w.saturation = 60.0;
        w.drain(20.0);
        assert_eq!(w.saturation, 40.0);
    }

    #[test]
    fn drain_clamps_at_zero() {
        let mut w = watery();
        w.saturation = 30.0;
        w.drain(200.0);
        assert_eq!(w.saturation, 0.0);
    }

    #[test]
    fn drain_no_op_when_disabled() {
        let mut w = watery();
        w.saturation = 50.0;
        w.enabled = false;
        w.drain(10.0);
        assert_eq!(w.saturation, 50.0);
    }

    #[test]
    fn drain_no_op_when_already_parched() {
        let mut w = watery();
        w.drain(10.0);
        assert_eq!(w.saturation, 0.0);
    }

    #[test]
    fn drain_sets_just_parched_at_zero() {
        let mut w = watery();
        w.saturation = 10.0;
        w.drain(10.0);
        assert!(w.just_parched);
    }

    #[test]
    fn drain_no_just_parched_if_already_zero() {
        let mut w = watery();
        w.drain(1.0);
        assert!(!w.just_parched);
    }

    #[test]
    fn tick_increases_saturation() {
        let mut w = watery();
        w.tick(1.0);
        assert_eq!(w.saturation, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = watery();
        w.tick(2.0);
        assert_eq!(w.saturation, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = watery();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.saturation, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_sodden() {
        let mut w = watery();
        w.saturation = 100.0;
        w.tick(1.0);
        assert_eq!(w.saturation, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = watery();
        w.seep_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.saturation, 0.0);
    }

    #[test]
    fn is_sodden_true_at_max() {
        let mut w = watery();
        w.saturation = 100.0;
        assert!(w.is_sodden());
    }

    #[test]
    fn is_sodden_false_below_max() {
        let mut w = watery();
        w.saturation = 50.0;
        assert!(!w.is_sodden());
    }

    #[test]
    fn is_sodden_false_when_disabled() {
        let mut w = watery();
        w.saturation = 100.0;
        w.enabled = false;
        assert!(!w.is_sodden());
    }

    #[test]
    fn is_parched_true_at_zero() {
        let w = watery();
        assert!(w.is_parched());
    }

    #[test]
    fn is_parched_false_above_zero() {
        let mut w = watery();
        w.saturation = 1.0;
        assert!(!w.is_parched());
    }

    #[test]
    fn saturation_fraction_zero_when_parched() {
        let w = watery();
        assert_eq!(w.saturation_fraction(), 0.0);
    }

    #[test]
    fn saturation_fraction_one_at_max() {
        let mut w = watery();
        w.saturation = 100.0;
        assert_eq!(w.saturation_fraction(), 1.0);
    }

    #[test]
    fn saturation_fraction_half_at_midpoint() {
        let mut w = watery();
        w.saturation = 50.0;
        assert_eq!(w.saturation_fraction(), 0.5);
    }

    #[test]
    fn saturation_fraction_zero_when_max_zero() {
        let mut w = watery();
        w.max_saturation = 0.0;
        assert_eq!(w.saturation_fraction(), 0.0);
    }

    #[test]
    fn effective_moisture_scales() {
        let mut w = watery();
        w.saturation = 50.0;
        assert_eq!(w.effective_moisture(2.0), 1.0);
    }

    #[test]
    fn effective_moisture_zero_when_parched() {
        let w = watery();
        assert_eq!(w.effective_moisture(10.0), 0.0);
    }

    #[test]
    fn just_sodden_cleared_on_next_seep() {
        let mut w = watery();
        w.seep(100.0);
        assert!(w.just_sodden);
        w.seep(1.0);
        assert!(!w.just_sodden);
    }

    #[test]
    fn just_parched_cleared_on_next_drain() {
        let mut w = watery();
        w.saturation = 10.0;
        w.drain(10.0);
        assert!(w.just_parched);
        w.saturation = 10.0;
        w.drain(1.0);
        assert!(!w.just_parched);
    }
}

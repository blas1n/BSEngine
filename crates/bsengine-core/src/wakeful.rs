use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wakeful {
    pub alertness: f32,
    pub max_alertness: f32,
    pub rouse_rate: f32,
    pub just_vigilant: bool,
    pub just_drowsy: bool,
    pub enabled: bool,
}

impl Default for Wakeful {
    fn default() -> Self {
        Self {
            alertness: 0.0,
            max_alertness: 100.0,
            rouse_rate: 1.0,
            just_vigilant: false,
            just_drowsy: false,
            enabled: true,
        }
    }
}

impl Wakeful {
    pub fn rouse(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_vigilant = false;
        self.just_drowsy = false;
        let prev = self.alertness;
        self.alertness = (self.alertness + amount).clamp(0.0, self.max_alertness);
        if self.alertness >= self.max_alertness && prev < self.max_alertness {
            self.just_vigilant = true;
        }
    }

    pub fn drowse(&mut self, amount: f32) {
        if !self.enabled || self.alertness <= 0.0 {
            return;
        }
        self.just_vigilant = false;
        self.just_drowsy = false;
        let prev = self.alertness;
        self.alertness = (self.alertness - amount).max(0.0);
        if self.alertness <= 0.0 && prev > 0.0 {
            self.just_drowsy = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.alertness >= self.max_alertness {
            return;
        }
        self.rouse(self.rouse_rate * dt);
    }

    pub fn is_vigilant(&self) -> bool {
        self.enabled && self.alertness >= self.max_alertness
    }

    pub fn is_drowsy(&self) -> bool {
        self.alertness <= 0.0
    }

    pub fn alertness_fraction(&self) -> f32 {
        if self.max_alertness <= 0.0 {
            return 0.0;
        }
        self.alertness / self.max_alertness
    }

    pub fn effective_awareness(&self, scale: f32) -> f32 {
        self.alertness_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wakeful() -> Wakeful {
        Wakeful {
            alertness: 0.0,
            max_alertness: 100.0,
            rouse_rate: 10.0,
            just_vigilant: false,
            just_drowsy: false,
            enabled: true,
        }
    }

    #[test]
    fn default_alertness_zero() {
        let w = Wakeful::default();
        assert_eq!(w.alertness, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wakeful::default().enabled);
    }

    #[test]
    fn rouse_increases_alertness() {
        let mut w = wakeful();
        w.rouse(30.0);
        assert_eq!(w.alertness, 30.0);
    }

    #[test]
    fn rouse_clamps_at_max() {
        let mut w = wakeful();
        w.rouse(200.0);
        assert_eq!(w.alertness, 100.0);
    }

    #[test]
    fn rouse_no_op_when_disabled() {
        let mut w = wakeful();
        w.enabled = false;
        w.rouse(50.0);
        assert_eq!(w.alertness, 0.0);
    }

    #[test]
    fn rouse_sets_just_vigilant_at_max() {
        let mut w = wakeful();
        w.rouse(100.0);
        assert!(w.just_vigilant);
    }

    #[test]
    fn rouse_no_just_vigilant_if_already_max() {
        let mut w = wakeful();
        w.alertness = 100.0;
        w.rouse(1.0);
        assert!(!w.just_vigilant);
    }

    #[test]
    fn drowse_decreases_alertness() {
        let mut w = wakeful();
        w.alertness = 60.0;
        w.drowse(20.0);
        assert_eq!(w.alertness, 40.0);
    }

    #[test]
    fn drowse_clamps_at_zero() {
        let mut w = wakeful();
        w.alertness = 30.0;
        w.drowse(200.0);
        assert_eq!(w.alertness, 0.0);
    }

    #[test]
    fn drowse_no_op_when_disabled() {
        let mut w = wakeful();
        w.alertness = 50.0;
        w.enabled = false;
        w.drowse(10.0);
        assert_eq!(w.alertness, 50.0);
    }

    #[test]
    fn drowse_no_op_when_already_drowsy() {
        let mut w = wakeful();
        w.drowse(10.0);
        assert_eq!(w.alertness, 0.0);
    }

    #[test]
    fn drowse_sets_just_drowsy_at_zero() {
        let mut w = wakeful();
        w.alertness = 10.0;
        w.drowse(10.0);
        assert!(w.just_drowsy);
    }

    #[test]
    fn drowse_no_just_drowsy_if_already_zero() {
        let mut w = wakeful();
        w.drowse(1.0);
        assert!(!w.just_drowsy);
    }

    #[test]
    fn tick_increases_alertness() {
        let mut w = wakeful();
        w.tick(1.0);
        assert_eq!(w.alertness, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wakeful();
        w.tick(2.0);
        assert_eq!(w.alertness, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wakeful();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.alertness, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_vigilant() {
        let mut w = wakeful();
        w.alertness = 100.0;
        w.tick(1.0);
        assert_eq!(w.alertness, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wakeful();
        w.rouse_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.alertness, 0.0);
    }

    #[test]
    fn is_vigilant_true_at_max() {
        let mut w = wakeful();
        w.alertness = 100.0;
        assert!(w.is_vigilant());
    }

    #[test]
    fn is_vigilant_false_below_max() {
        let mut w = wakeful();
        w.alertness = 50.0;
        assert!(!w.is_vigilant());
    }

    #[test]
    fn is_vigilant_false_when_disabled() {
        let mut w = wakeful();
        w.alertness = 100.0;
        w.enabled = false;
        assert!(!w.is_vigilant());
    }

    #[test]
    fn is_drowsy_true_at_zero() {
        let w = wakeful();
        assert!(w.is_drowsy());
    }

    #[test]
    fn is_drowsy_false_above_zero() {
        let mut w = wakeful();
        w.alertness = 1.0;
        assert!(!w.is_drowsy());
    }

    #[test]
    fn alertness_fraction_zero_when_drowsy() {
        let w = wakeful();
        assert_eq!(w.alertness_fraction(), 0.0);
    }

    #[test]
    fn alertness_fraction_one_at_max() {
        let mut w = wakeful();
        w.alertness = 100.0;
        assert_eq!(w.alertness_fraction(), 1.0);
    }

    #[test]
    fn alertness_fraction_half_at_midpoint() {
        let mut w = wakeful();
        w.alertness = 50.0;
        assert_eq!(w.alertness_fraction(), 0.5);
    }

    #[test]
    fn alertness_fraction_zero_when_max_zero() {
        let mut w = wakeful();
        w.max_alertness = 0.0;
        assert_eq!(w.alertness_fraction(), 0.0);
    }

    #[test]
    fn effective_awareness_scales() {
        let mut w = wakeful();
        w.alertness = 50.0;
        assert_eq!(w.effective_awareness(2.0), 1.0);
    }

    #[test]
    fn effective_awareness_zero_when_drowsy() {
        let w = wakeful();
        assert_eq!(w.effective_awareness(10.0), 0.0);
    }

    #[test]
    fn just_vigilant_cleared_on_next_rouse() {
        let mut w = wakeful();
        w.rouse(100.0);
        assert!(w.just_vigilant);
        w.rouse(1.0);
        assert!(!w.just_vigilant);
    }

    #[test]
    fn just_drowsy_cleared_on_next_drowse() {
        let mut w = wakeful();
        w.alertness = 10.0;
        w.drowse(10.0);
        assert!(w.just_drowsy);
        w.alertness = 10.0;
        w.drowse(1.0);
        assert!(!w.just_drowsy);
    }
}

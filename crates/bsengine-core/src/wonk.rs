use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wonk {
    pub expertise: f32,
    pub max_expertise: f32,
    pub study_rate: f32,
    pub just_mastered: bool,
    pub just_lapsed: bool,
    pub enabled: bool,
}

impl Default for Wonk {
    fn default() -> Self {
        Self {
            expertise: 0.0,
            max_expertise: 100.0,
            study_rate: 1.0,
            just_mastered: false,
            just_lapsed: false,
            enabled: true,
        }
    }
}

impl Wonk {
    pub fn study(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_mastered = false;
        self.just_lapsed = false;
        let prev = self.expertise;
        self.expertise = (self.expertise + amount).clamp(0.0, self.max_expertise);
        if self.expertise >= self.max_expertise && prev < self.max_expertise {
            self.just_mastered = true;
        }
    }

    pub fn lapse(&mut self, amount: f32) {
        if !self.enabled || self.expertise <= 0.0 {
            return;
        }
        self.just_mastered = false;
        self.just_lapsed = false;
        let prev = self.expertise;
        self.expertise = (self.expertise - amount).max(0.0);
        if self.expertise <= 0.0 && prev > 0.0 {
            self.just_lapsed = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.expertise >= self.max_expertise {
            return;
        }
        self.study(self.study_rate * dt);
    }

    pub fn is_mastered(&self) -> bool {
        self.enabled && self.expertise >= self.max_expertise
    }

    pub fn is_lapsed(&self) -> bool {
        self.expertise <= 0.0
    }

    pub fn expertise_fraction(&self) -> f32 {
        if self.max_expertise <= 0.0 {
            return 0.0;
        }
        self.expertise / self.max_expertise
    }

    pub fn effective_knowledge(&self, scale: f32) -> f32 {
        self.expertise_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wonk() -> Wonk {
        Wonk {
            expertise: 0.0,
            max_expertise: 100.0,
            study_rate: 10.0,
            just_mastered: false,
            just_lapsed: false,
            enabled: true,
        }
    }

    #[test]
    fn default_expertise_zero() {
        let w = Wonk::default();
        assert_eq!(w.expertise, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wonk::default().enabled);
    }

    #[test]
    fn study_increases_expertise() {
        let mut w = wonk();
        w.study(30.0);
        assert_eq!(w.expertise, 30.0);
    }

    #[test]
    fn study_clamps_at_max() {
        let mut w = wonk();
        w.study(200.0);
        assert_eq!(w.expertise, 100.0);
    }

    #[test]
    fn study_no_op_when_disabled() {
        let mut w = wonk();
        w.enabled = false;
        w.study(50.0);
        assert_eq!(w.expertise, 0.0);
    }

    #[test]
    fn study_sets_just_mastered_at_max() {
        let mut w = wonk();
        w.study(100.0);
        assert!(w.just_mastered);
    }

    #[test]
    fn study_no_just_mastered_if_already_max() {
        let mut w = wonk();
        w.expertise = 100.0;
        w.study(1.0);
        assert!(!w.just_mastered);
    }

    #[test]
    fn lapse_decreases_expertise() {
        let mut w = wonk();
        w.expertise = 60.0;
        w.lapse(20.0);
        assert_eq!(w.expertise, 40.0);
    }

    #[test]
    fn lapse_clamps_at_zero() {
        let mut w = wonk();
        w.expertise = 30.0;
        w.lapse(200.0);
        assert_eq!(w.expertise, 0.0);
    }

    #[test]
    fn lapse_no_op_when_disabled() {
        let mut w = wonk();
        w.expertise = 50.0;
        w.enabled = false;
        w.lapse(10.0);
        assert_eq!(w.expertise, 50.0);
    }

    #[test]
    fn lapse_no_op_when_already_lapsed() {
        let mut w = wonk();
        w.lapse(10.0);
        assert_eq!(w.expertise, 0.0);
    }

    #[test]
    fn lapse_sets_just_lapsed_at_zero() {
        let mut w = wonk();
        w.expertise = 10.0;
        w.lapse(10.0);
        assert!(w.just_lapsed);
    }

    #[test]
    fn lapse_no_just_lapsed_if_already_zero() {
        let mut w = wonk();
        w.lapse(1.0);
        assert!(!w.just_lapsed);
    }

    #[test]
    fn tick_increases_expertise() {
        let mut w = wonk();
        w.tick(1.0);
        assert_eq!(w.expertise, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wonk();
        w.tick(2.0);
        assert_eq!(w.expertise, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wonk();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.expertise, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_mastered() {
        let mut w = wonk();
        w.expertise = 100.0;
        w.tick(1.0);
        assert_eq!(w.expertise, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wonk();
        w.study_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.expertise, 0.0);
    }

    #[test]
    fn is_mastered_true_at_max() {
        let mut w = wonk();
        w.expertise = 100.0;
        assert!(w.is_mastered());
    }

    #[test]
    fn is_mastered_false_below_max() {
        let mut w = wonk();
        w.expertise = 50.0;
        assert!(!w.is_mastered());
    }

    #[test]
    fn is_mastered_false_when_disabled() {
        let mut w = wonk();
        w.expertise = 100.0;
        w.enabled = false;
        assert!(!w.is_mastered());
    }

    #[test]
    fn is_lapsed_true_at_zero() {
        let w = wonk();
        assert!(w.is_lapsed());
    }

    #[test]
    fn is_lapsed_false_above_zero() {
        let mut w = wonk();
        w.expertise = 1.0;
        assert!(!w.is_lapsed());
    }

    #[test]
    fn expertise_fraction_zero_when_lapsed() {
        let w = wonk();
        assert_eq!(w.expertise_fraction(), 0.0);
    }

    #[test]
    fn expertise_fraction_one_at_max() {
        let mut w = wonk();
        w.expertise = 100.0;
        assert_eq!(w.expertise_fraction(), 1.0);
    }

    #[test]
    fn expertise_fraction_half_at_midpoint() {
        let mut w = wonk();
        w.expertise = 50.0;
        assert_eq!(w.expertise_fraction(), 0.5);
    }

    #[test]
    fn expertise_fraction_zero_when_max_zero() {
        let mut w = wonk();
        w.max_expertise = 0.0;
        assert_eq!(w.expertise_fraction(), 0.0);
    }

    #[test]
    fn effective_knowledge_scales() {
        let mut w = wonk();
        w.expertise = 50.0;
        assert_eq!(w.effective_knowledge(2.0), 1.0);
    }

    #[test]
    fn effective_knowledge_zero_when_lapsed() {
        let w = wonk();
        assert_eq!(w.effective_knowledge(10.0), 0.0);
    }

    #[test]
    fn just_mastered_cleared_on_next_study() {
        let mut w = wonk();
        w.study(100.0);
        assert!(w.just_mastered);
        w.study(1.0);
        assert!(!w.just_mastered);
    }

    #[test]
    fn just_lapsed_cleared_on_next_lapse() {
        let mut w = wonk();
        w.expertise = 10.0;
        w.lapse(10.0);
        assert!(w.just_lapsed);
        w.expertise = 10.0;
        w.lapse(1.0);
        assert!(!w.just_lapsed);
    }
}

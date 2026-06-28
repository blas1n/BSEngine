use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrangle {
    pub dispute: f32,
    pub max_dispute: f32,
    pub quarrel_rate: f32,
    pub just_embroiled: bool,
    pub just_resolved: bool,
    pub enabled: bool,
}

impl Default for Wrangle {
    fn default() -> Self {
        Self {
            dispute: 0.0,
            max_dispute: 100.0,
            quarrel_rate: 1.0,
            just_embroiled: false,
            just_resolved: false,
            enabled: true,
        }
    }
}

impl Wrangle {
    pub fn quarrel(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_embroiled = false;
        self.just_resolved = false;
        let prev = self.dispute;
        self.dispute = (self.dispute + amount).clamp(0.0, self.max_dispute);
        if self.dispute >= self.max_dispute && prev < self.max_dispute {
            self.just_embroiled = true;
        }
    }

    pub fn resolve(&mut self, amount: f32) {
        if !self.enabled || self.dispute <= 0.0 {
            return;
        }
        self.just_embroiled = false;
        self.just_resolved = false;
        let prev = self.dispute;
        self.dispute = (self.dispute - amount).max(0.0);
        if self.dispute <= 0.0 && prev > 0.0 {
            self.just_resolved = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.dispute >= self.max_dispute {
            return;
        }
        self.quarrel(self.quarrel_rate * dt);
    }

    pub fn is_embroiled(&self) -> bool {
        self.enabled && self.dispute >= self.max_dispute
    }

    pub fn is_resolved(&self) -> bool {
        self.dispute <= 0.0
    }

    pub fn dispute_fraction(&self) -> f32 {
        if self.max_dispute <= 0.0 {
            return 0.0;
        }
        self.dispute / self.max_dispute
    }

    pub fn effective_conflict(&self, scale: f32) -> f32 {
        self.dispute_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrangle() -> Wrangle {
        Wrangle {
            dispute: 0.0,
            max_dispute: 100.0,
            quarrel_rate: 10.0,
            just_embroiled: false,
            just_resolved: false,
            enabled: true,
        }
    }

    #[test]
    fn default_dispute_zero() {
        let w = Wrangle::default();
        assert_eq!(w.dispute, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wrangle::default().enabled);
    }

    #[test]
    fn quarrel_increases_dispute() {
        let mut w = wrangle();
        w.quarrel(30.0);
        assert_eq!(w.dispute, 30.0);
    }

    #[test]
    fn quarrel_clamps_at_max() {
        let mut w = wrangle();
        w.quarrel(200.0);
        assert_eq!(w.dispute, 100.0);
    }

    #[test]
    fn quarrel_no_op_when_disabled() {
        let mut w = wrangle();
        w.enabled = false;
        w.quarrel(50.0);
        assert_eq!(w.dispute, 0.0);
    }

    #[test]
    fn quarrel_sets_just_embroiled_at_max() {
        let mut w = wrangle();
        w.quarrel(100.0);
        assert!(w.just_embroiled);
    }

    #[test]
    fn quarrel_no_just_embroiled_if_already_max() {
        let mut w = wrangle();
        w.dispute = 100.0;
        w.quarrel(1.0);
        assert!(!w.just_embroiled);
    }

    #[test]
    fn resolve_decreases_dispute() {
        let mut w = wrangle();
        w.dispute = 60.0;
        w.resolve(20.0);
        assert_eq!(w.dispute, 40.0);
    }

    #[test]
    fn resolve_clamps_at_zero() {
        let mut w = wrangle();
        w.dispute = 30.0;
        w.resolve(200.0);
        assert_eq!(w.dispute, 0.0);
    }

    #[test]
    fn resolve_no_op_when_disabled() {
        let mut w = wrangle();
        w.dispute = 50.0;
        w.enabled = false;
        w.resolve(10.0);
        assert_eq!(w.dispute, 50.0);
    }

    #[test]
    fn resolve_no_op_when_already_resolved() {
        let mut w = wrangle();
        w.resolve(10.0);
        assert_eq!(w.dispute, 0.0);
    }

    #[test]
    fn resolve_sets_just_resolved_at_zero() {
        let mut w = wrangle();
        w.dispute = 10.0;
        w.resolve(10.0);
        assert!(w.just_resolved);
    }

    #[test]
    fn resolve_no_just_resolved_if_already_zero() {
        let mut w = wrangle();
        w.resolve(1.0);
        assert!(!w.just_resolved);
    }

    #[test]
    fn tick_increases_dispute() {
        let mut w = wrangle();
        w.tick(1.0);
        assert_eq!(w.dispute, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wrangle();
        w.tick(2.0);
        assert_eq!(w.dispute, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wrangle();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.dispute, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_embroiled() {
        let mut w = wrangle();
        w.dispute = 100.0;
        w.tick(1.0);
        assert_eq!(w.dispute, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wrangle();
        w.quarrel_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.dispute, 0.0);
    }

    #[test]
    fn is_embroiled_true_at_max() {
        let mut w = wrangle();
        w.dispute = 100.0;
        assert!(w.is_embroiled());
    }

    #[test]
    fn is_embroiled_false_below_max() {
        let mut w = wrangle();
        w.dispute = 50.0;
        assert!(!w.is_embroiled());
    }

    #[test]
    fn is_embroiled_false_when_disabled() {
        let mut w = wrangle();
        w.dispute = 100.0;
        w.enabled = false;
        assert!(!w.is_embroiled());
    }

    #[test]
    fn is_resolved_true_at_zero() {
        let w = wrangle();
        assert!(w.is_resolved());
    }

    #[test]
    fn is_resolved_false_above_zero() {
        let mut w = wrangle();
        w.dispute = 1.0;
        assert!(!w.is_resolved());
    }

    #[test]
    fn dispute_fraction_zero_when_resolved() {
        let w = wrangle();
        assert_eq!(w.dispute_fraction(), 0.0);
    }

    #[test]
    fn dispute_fraction_one_at_max() {
        let mut w = wrangle();
        w.dispute = 100.0;
        assert_eq!(w.dispute_fraction(), 1.0);
    }

    #[test]
    fn dispute_fraction_half_at_midpoint() {
        let mut w = wrangle();
        w.dispute = 50.0;
        assert_eq!(w.dispute_fraction(), 0.5);
    }

    #[test]
    fn dispute_fraction_zero_when_max_zero() {
        let mut w = wrangle();
        w.max_dispute = 0.0;
        assert_eq!(w.dispute_fraction(), 0.0);
    }

    #[test]
    fn effective_conflict_scales() {
        let mut w = wrangle();
        w.dispute = 50.0;
        assert_eq!(w.effective_conflict(2.0), 1.0);
    }

    #[test]
    fn effective_conflict_zero_when_resolved() {
        let w = wrangle();
        assert_eq!(w.effective_conflict(10.0), 0.0);
    }

    #[test]
    fn just_embroiled_cleared_on_next_quarrel() {
        let mut w = wrangle();
        w.quarrel(100.0);
        assert!(w.just_embroiled);
        w.quarrel(1.0);
        assert!(!w.just_embroiled);
    }

    #[test]
    fn just_resolved_cleared_on_next_resolve() {
        let mut w = wrangle();
        w.dispute = 10.0;
        w.resolve(10.0);
        assert!(w.just_resolved);
        w.dispute = 10.0;
        w.resolve(1.0);
        assert!(!w.just_resolved);
    }
}

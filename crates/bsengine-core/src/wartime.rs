use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wartime {
    pub conflict: f32,
    pub max_conflict: f32,
    pub escalate_rate: f32,
    pub just_total_war: bool,
    pub just_peace: bool,
    pub enabled: bool,
}

impl Default for Wartime {
    fn default() -> Self {
        Self {
            conflict: 0.0,
            max_conflict: 100.0,
            escalate_rate: 1.0,
            just_total_war: false,
            just_peace: false,
            enabled: true,
        }
    }
}

impl Wartime {
    pub fn escalate(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_total_war = false;
        self.just_peace = false;
        let prev = self.conflict;
        self.conflict = (self.conflict + amount).clamp(0.0, self.max_conflict);
        if self.conflict >= self.max_conflict && prev < self.max_conflict {
            self.just_total_war = true;
        }
    }

    pub fn ceasefire(&mut self, amount: f32) {
        if !self.enabled || self.conflict <= 0.0 {
            return;
        }
        self.just_total_war = false;
        self.just_peace = false;
        let prev = self.conflict;
        self.conflict = (self.conflict - amount).max(0.0);
        if self.conflict <= 0.0 && prev > 0.0 {
            self.just_peace = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.conflict >= self.max_conflict {
            return;
        }
        self.escalate(self.escalate_rate * dt);
    }

    pub fn is_total_war(&self) -> bool {
        self.enabled && self.conflict >= self.max_conflict
    }

    pub fn is_peace(&self) -> bool {
        self.conflict <= 0.0
    }

    pub fn conflict_fraction(&self) -> f32 {
        if self.max_conflict <= 0.0 {
            return 0.0;
        }
        self.conflict / self.max_conflict
    }

    pub fn effective_hostility(&self, scale: f32) -> f32 {
        self.conflict_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wartime() -> Wartime {
        Wartime {
            conflict: 0.0,
            max_conflict: 100.0,
            escalate_rate: 10.0,
            just_total_war: false,
            just_peace: false,
            enabled: true,
        }
    }

    #[test]
    fn default_conflict_zero() {
        let w = Wartime::default();
        assert_eq!(w.conflict, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wartime::default().enabled);
    }

    #[test]
    fn escalate_increases_conflict() {
        let mut w = wartime();
        w.escalate(30.0);
        assert_eq!(w.conflict, 30.0);
    }

    #[test]
    fn escalate_clamps_at_max() {
        let mut w = wartime();
        w.escalate(200.0);
        assert_eq!(w.conflict, 100.0);
    }

    #[test]
    fn escalate_no_op_when_disabled() {
        let mut w = wartime();
        w.enabled = false;
        w.escalate(50.0);
        assert_eq!(w.conflict, 0.0);
    }

    #[test]
    fn escalate_sets_just_total_war_at_max() {
        let mut w = wartime();
        w.escalate(100.0);
        assert!(w.just_total_war);
    }

    #[test]
    fn escalate_no_just_total_war_if_already_max() {
        let mut w = wartime();
        w.conflict = 100.0;
        w.escalate(1.0);
        assert!(!w.just_total_war);
    }

    #[test]
    fn ceasefire_decreases_conflict() {
        let mut w = wartime();
        w.conflict = 60.0;
        w.ceasefire(20.0);
        assert_eq!(w.conflict, 40.0);
    }

    #[test]
    fn ceasefire_clamps_at_zero() {
        let mut w = wartime();
        w.conflict = 30.0;
        w.ceasefire(200.0);
        assert_eq!(w.conflict, 0.0);
    }

    #[test]
    fn ceasefire_no_op_when_disabled() {
        let mut w = wartime();
        w.conflict = 50.0;
        w.enabled = false;
        w.ceasefire(10.0);
        assert_eq!(w.conflict, 50.0);
    }

    #[test]
    fn ceasefire_no_op_when_already_peace() {
        let mut w = wartime();
        w.ceasefire(10.0);
        assert_eq!(w.conflict, 0.0);
    }

    #[test]
    fn ceasefire_sets_just_peace_at_zero() {
        let mut w = wartime();
        w.conflict = 10.0;
        w.ceasefire(10.0);
        assert!(w.just_peace);
    }

    #[test]
    fn ceasefire_no_just_peace_if_already_zero() {
        let mut w = wartime();
        w.ceasefire(1.0);
        assert!(!w.just_peace);
    }

    #[test]
    fn tick_increases_conflict() {
        let mut w = wartime();
        w.tick(1.0);
        assert_eq!(w.conflict, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wartime();
        w.tick(2.0);
        assert_eq!(w.conflict, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wartime();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.conflict, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_total_war() {
        let mut w = wartime();
        w.conflict = 100.0;
        w.tick(1.0);
        assert_eq!(w.conflict, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wartime();
        w.escalate_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.conflict, 0.0);
    }

    #[test]
    fn is_total_war_true_at_max() {
        let mut w = wartime();
        w.conflict = 100.0;
        assert!(w.is_total_war());
    }

    #[test]
    fn is_total_war_false_below_max() {
        let mut w = wartime();
        w.conflict = 50.0;
        assert!(!w.is_total_war());
    }

    #[test]
    fn is_total_war_false_when_disabled() {
        let mut w = wartime();
        w.conflict = 100.0;
        w.enabled = false;
        assert!(!w.is_total_war());
    }

    #[test]
    fn is_peace_true_at_zero() {
        let w = wartime();
        assert!(w.is_peace());
    }

    #[test]
    fn is_peace_false_above_zero() {
        let mut w = wartime();
        w.conflict = 1.0;
        assert!(!w.is_peace());
    }

    #[test]
    fn conflict_fraction_zero_when_peace() {
        let w = wartime();
        assert_eq!(w.conflict_fraction(), 0.0);
    }

    #[test]
    fn conflict_fraction_one_at_max() {
        let mut w = wartime();
        w.conflict = 100.0;
        assert_eq!(w.conflict_fraction(), 1.0);
    }

    #[test]
    fn conflict_fraction_half_at_midpoint() {
        let mut w = wartime();
        w.conflict = 50.0;
        assert_eq!(w.conflict_fraction(), 0.5);
    }

    #[test]
    fn conflict_fraction_zero_when_max_zero() {
        let mut w = wartime();
        w.max_conflict = 0.0;
        assert_eq!(w.conflict_fraction(), 0.0);
    }

    #[test]
    fn effective_hostility_scales() {
        let mut w = wartime();
        w.conflict = 50.0;
        assert_eq!(w.effective_hostility(2.0), 1.0);
    }

    #[test]
    fn effective_hostility_zero_when_peace() {
        let w = wartime();
        assert_eq!(w.effective_hostility(10.0), 0.0);
    }

    #[test]
    fn just_total_war_cleared_on_next_escalate() {
        let mut w = wartime();
        w.escalate(100.0);
        assert!(w.just_total_war);
        w.escalate(1.0);
        assert!(!w.just_total_war);
    }

    #[test]
    fn just_peace_cleared_on_next_ceasefire() {
        let mut w = wartime();
        w.conflict = 10.0;
        w.ceasefire(10.0);
        assert!(w.just_peace);
        w.conflict = 10.0;
        w.ceasefire(1.0);
        assert!(!w.just_peace);
    }
}

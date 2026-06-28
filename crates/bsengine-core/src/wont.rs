use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wont {
    pub habit: f32,
    pub max_habit: f32,
    pub routine_rate: f32,
    pub just_ingrained: bool,
    pub just_broken: bool,
    pub enabled: bool,
}

impl Default for Wont {
    fn default() -> Self {
        Self {
            habit: 0.0,
            max_habit: 100.0,
            routine_rate: 1.0,
            just_ingrained: false,
            just_broken: false,
            enabled: true,
        }
    }
}

impl Wont {
    pub fn reinforce(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_ingrained = false;
        self.just_broken = false;
        let prev = self.habit;
        self.habit = (self.habit + amount).clamp(0.0, self.max_habit);
        if self.habit >= self.max_habit && prev < self.max_habit {
            self.just_ingrained = true;
        }
    }

    pub fn disrupt(&mut self, amount: f32) {
        if !self.enabled || self.habit <= 0.0 {
            return;
        }
        self.just_ingrained = false;
        self.just_broken = false;
        let prev = self.habit;
        self.habit = (self.habit - amount).max(0.0);
        if self.habit <= 0.0 && prev > 0.0 {
            self.just_broken = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.habit >= self.max_habit {
            return;
        }
        self.reinforce(self.routine_rate * dt);
    }

    pub fn is_ingrained(&self) -> bool {
        self.enabled && self.habit >= self.max_habit
    }

    pub fn is_broken(&self) -> bool {
        self.habit <= 0.0
    }

    pub fn habit_fraction(&self) -> f32 {
        if self.max_habit <= 0.0 {
            return 0.0;
        }
        self.habit / self.max_habit
    }

    pub fn effective_routine(&self, scale: f32) -> f32 {
        self.habit_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wont() -> Wont {
        Wont {
            habit: 0.0,
            max_habit: 100.0,
            routine_rate: 10.0,
            just_ingrained: false,
            just_broken: false,
            enabled: true,
        }
    }

    #[test]
    fn default_habit_zero() {
        let w = Wont::default();
        assert_eq!(w.habit, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wont::default().enabled);
    }

    #[test]
    fn reinforce_increases_habit() {
        let mut w = wont();
        w.reinforce(30.0);
        assert_eq!(w.habit, 30.0);
    }

    #[test]
    fn reinforce_clamps_at_max() {
        let mut w = wont();
        w.reinforce(200.0);
        assert_eq!(w.habit, 100.0);
    }

    #[test]
    fn reinforce_no_op_when_disabled() {
        let mut w = wont();
        w.enabled = false;
        w.reinforce(50.0);
        assert_eq!(w.habit, 0.0);
    }

    #[test]
    fn reinforce_sets_just_ingrained_at_max() {
        let mut w = wont();
        w.reinforce(100.0);
        assert!(w.just_ingrained);
    }

    #[test]
    fn reinforce_no_just_ingrained_if_already_max() {
        let mut w = wont();
        w.habit = 100.0;
        w.reinforce(1.0);
        assert!(!w.just_ingrained);
    }

    #[test]
    fn disrupt_decreases_habit() {
        let mut w = wont();
        w.habit = 60.0;
        w.disrupt(20.0);
        assert_eq!(w.habit, 40.0);
    }

    #[test]
    fn disrupt_clamps_at_zero() {
        let mut w = wont();
        w.habit = 30.0;
        w.disrupt(200.0);
        assert_eq!(w.habit, 0.0);
    }

    #[test]
    fn disrupt_no_op_when_disabled() {
        let mut w = wont();
        w.habit = 50.0;
        w.enabled = false;
        w.disrupt(10.0);
        assert_eq!(w.habit, 50.0);
    }

    #[test]
    fn disrupt_no_op_when_already_broken() {
        let mut w = wont();
        w.disrupt(10.0);
        assert_eq!(w.habit, 0.0);
    }

    #[test]
    fn disrupt_sets_just_broken_at_zero() {
        let mut w = wont();
        w.habit = 10.0;
        w.disrupt(10.0);
        assert!(w.just_broken);
    }

    #[test]
    fn disrupt_no_just_broken_if_already_zero() {
        let mut w = wont();
        w.disrupt(1.0);
        assert!(!w.just_broken);
    }

    #[test]
    fn tick_increases_habit() {
        let mut w = wont();
        w.tick(1.0);
        assert_eq!(w.habit, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wont();
        w.tick(2.0);
        assert_eq!(w.habit, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wont();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.habit, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_ingrained() {
        let mut w = wont();
        w.habit = 100.0;
        w.tick(1.0);
        assert_eq!(w.habit, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wont();
        w.routine_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.habit, 0.0);
    }

    #[test]
    fn is_ingrained_true_at_max() {
        let mut w = wont();
        w.habit = 100.0;
        assert!(w.is_ingrained());
    }

    #[test]
    fn is_ingrained_false_below_max() {
        let mut w = wont();
        w.habit = 50.0;
        assert!(!w.is_ingrained());
    }

    #[test]
    fn is_ingrained_false_when_disabled() {
        let mut w = wont();
        w.habit = 100.0;
        w.enabled = false;
        assert!(!w.is_ingrained());
    }

    #[test]
    fn is_broken_true_at_zero() {
        let w = wont();
        assert!(w.is_broken());
    }

    #[test]
    fn is_broken_false_above_zero() {
        let mut w = wont();
        w.habit = 1.0;
        assert!(!w.is_broken());
    }

    #[test]
    fn habit_fraction_zero_when_broken() {
        let w = wont();
        assert_eq!(w.habit_fraction(), 0.0);
    }

    #[test]
    fn habit_fraction_one_at_max() {
        let mut w = wont();
        w.habit = 100.0;
        assert_eq!(w.habit_fraction(), 1.0);
    }

    #[test]
    fn habit_fraction_half_at_midpoint() {
        let mut w = wont();
        w.habit = 50.0;
        assert_eq!(w.habit_fraction(), 0.5);
    }

    #[test]
    fn habit_fraction_zero_when_max_zero() {
        let mut w = wont();
        w.max_habit = 0.0;
        assert_eq!(w.habit_fraction(), 0.0);
    }

    #[test]
    fn effective_routine_scales() {
        let mut w = wont();
        w.habit = 50.0;
        assert_eq!(w.effective_routine(2.0), 1.0);
    }

    #[test]
    fn effective_routine_zero_when_broken() {
        let w = wont();
        assert_eq!(w.effective_routine(10.0), 0.0);
    }

    #[test]
    fn just_ingrained_cleared_on_next_reinforce() {
        let mut w = wont();
        w.reinforce(100.0);
        assert!(w.just_ingrained);
        w.reinforce(1.0);
        assert!(!w.just_ingrained);
    }

    #[test]
    fn just_broken_cleared_on_next_disrupt() {
        let mut w = wont();
        w.habit = 10.0;
        w.disrupt(10.0);
        assert!(w.just_broken);
        w.habit = 10.0;
        w.disrupt(1.0);
        assert!(!w.just_broken);
    }
}

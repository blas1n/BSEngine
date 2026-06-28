use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wordy {
    pub verbosity: f32,
    pub max_verbosity: f32,
    pub ramble_rate: f32,
    pub just_verbose: bool,
    pub just_terse: bool,
    pub enabled: bool,
}

impl Default for Wordy {
    fn default() -> Self {
        Self {
            verbosity: 0.0,
            max_verbosity: 100.0,
            ramble_rate: 1.0,
            just_verbose: false,
            just_terse: false,
            enabled: true,
        }
    }
}

impl Wordy {
    pub fn ramble(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_verbose = false;
        self.just_terse = false;
        let prev = self.verbosity;
        self.verbosity = (self.verbosity + amount).clamp(0.0, self.max_verbosity);
        if self.verbosity >= self.max_verbosity && prev < self.max_verbosity {
            self.just_verbose = true;
        }
    }

    pub fn truncate(&mut self, amount: f32) {
        if !self.enabled || self.verbosity <= 0.0 {
            return;
        }
        self.just_verbose = false;
        self.just_terse = false;
        let prev = self.verbosity;
        self.verbosity = (self.verbosity - amount).max(0.0);
        if self.verbosity <= 0.0 && prev > 0.0 {
            self.just_terse = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.verbosity >= self.max_verbosity {
            return;
        }
        self.ramble(self.ramble_rate * dt);
    }

    pub fn is_verbose(&self) -> bool {
        self.enabled && self.verbosity >= self.max_verbosity
    }

    pub fn is_terse(&self) -> bool {
        self.verbosity <= 0.0
    }

    pub fn verbosity_fraction(&self) -> f32 {
        if self.max_verbosity <= 0.0 {
            return 0.0;
        }
        self.verbosity / self.max_verbosity
    }

    pub fn effective_prose(&self, scale: f32) -> f32 {
        self.verbosity_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wordy() -> Wordy {
        Wordy {
            verbosity: 0.0,
            max_verbosity: 100.0,
            ramble_rate: 10.0,
            just_verbose: false,
            just_terse: false,
            enabled: true,
        }
    }

    #[test]
    fn default_verbosity_zero() {
        let w = Wordy::default();
        assert_eq!(w.verbosity, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wordy::default().enabled);
    }

    #[test]
    fn ramble_increases_verbosity() {
        let mut w = wordy();
        w.ramble(30.0);
        assert_eq!(w.verbosity, 30.0);
    }

    #[test]
    fn ramble_clamps_at_max() {
        let mut w = wordy();
        w.ramble(200.0);
        assert_eq!(w.verbosity, 100.0);
    }

    #[test]
    fn ramble_no_op_when_disabled() {
        let mut w = wordy();
        w.enabled = false;
        w.ramble(50.0);
        assert_eq!(w.verbosity, 0.0);
    }

    #[test]
    fn ramble_sets_just_verbose_at_max() {
        let mut w = wordy();
        w.ramble(100.0);
        assert!(w.just_verbose);
    }

    #[test]
    fn ramble_no_just_verbose_if_already_max() {
        let mut w = wordy();
        w.verbosity = 100.0;
        w.ramble(1.0);
        assert!(!w.just_verbose);
    }

    #[test]
    fn truncate_decreases_verbosity() {
        let mut w = wordy();
        w.verbosity = 60.0;
        w.truncate(20.0);
        assert_eq!(w.verbosity, 40.0);
    }

    #[test]
    fn truncate_clamps_at_zero() {
        let mut w = wordy();
        w.verbosity = 30.0;
        w.truncate(200.0);
        assert_eq!(w.verbosity, 0.0);
    }

    #[test]
    fn truncate_no_op_when_disabled() {
        let mut w = wordy();
        w.verbosity = 50.0;
        w.enabled = false;
        w.truncate(10.0);
        assert_eq!(w.verbosity, 50.0);
    }

    #[test]
    fn truncate_no_op_when_already_terse() {
        let mut w = wordy();
        w.truncate(10.0);
        assert_eq!(w.verbosity, 0.0);
    }

    #[test]
    fn truncate_sets_just_terse_at_zero() {
        let mut w = wordy();
        w.verbosity = 10.0;
        w.truncate(10.0);
        assert!(w.just_terse);
    }

    #[test]
    fn truncate_no_just_terse_if_already_zero() {
        let mut w = wordy();
        w.truncate(1.0);
        assert!(!w.just_terse);
    }

    #[test]
    fn tick_increases_verbosity() {
        let mut w = wordy();
        w.tick(1.0);
        assert_eq!(w.verbosity, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wordy();
        w.tick(2.0);
        assert_eq!(w.verbosity, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wordy();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.verbosity, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_verbose() {
        let mut w = wordy();
        w.verbosity = 100.0;
        w.tick(1.0);
        assert_eq!(w.verbosity, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wordy();
        w.ramble_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.verbosity, 0.0);
    }

    #[test]
    fn is_verbose_true_at_max() {
        let mut w = wordy();
        w.verbosity = 100.0;
        assert!(w.is_verbose());
    }

    #[test]
    fn is_verbose_false_below_max() {
        let mut w = wordy();
        w.verbosity = 50.0;
        assert!(!w.is_verbose());
    }

    #[test]
    fn is_verbose_false_when_disabled() {
        let mut w = wordy();
        w.verbosity = 100.0;
        w.enabled = false;
        assert!(!w.is_verbose());
    }

    #[test]
    fn is_terse_true_at_zero() {
        let w = wordy();
        assert!(w.is_terse());
    }

    #[test]
    fn is_terse_false_above_zero() {
        let mut w = wordy();
        w.verbosity = 1.0;
        assert!(!w.is_terse());
    }

    #[test]
    fn verbosity_fraction_zero_when_terse() {
        let w = wordy();
        assert_eq!(w.verbosity_fraction(), 0.0);
    }

    #[test]
    fn verbosity_fraction_one_at_max() {
        let mut w = wordy();
        w.verbosity = 100.0;
        assert_eq!(w.verbosity_fraction(), 1.0);
    }

    #[test]
    fn verbosity_fraction_half_at_midpoint() {
        let mut w = wordy();
        w.verbosity = 50.0;
        assert_eq!(w.verbosity_fraction(), 0.5);
    }

    #[test]
    fn verbosity_fraction_zero_when_max_zero() {
        let mut w = wordy();
        w.max_verbosity = 0.0;
        assert_eq!(w.verbosity_fraction(), 0.0);
    }

    #[test]
    fn effective_prose_scales() {
        let mut w = wordy();
        w.verbosity = 50.0;
        assert_eq!(w.effective_prose(2.0), 1.0);
    }

    #[test]
    fn effective_prose_zero_when_terse() {
        let w = wordy();
        assert_eq!(w.effective_prose(10.0), 0.0);
    }

    #[test]
    fn just_verbose_cleared_on_next_ramble() {
        let mut w = wordy();
        w.ramble(100.0);
        assert!(w.just_verbose);
        w.ramble(1.0);
        assert!(!w.just_verbose);
    }

    #[test]
    fn just_terse_cleared_on_next_truncate() {
        let mut w = wordy();
        w.verbosity = 10.0;
        w.truncate(10.0);
        assert!(w.just_terse);
        w.verbosity = 10.0;
        w.truncate(1.0);
        assert!(!w.just_terse);
    }
}

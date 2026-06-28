use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wedge {
    pub drive: f32,
    pub max_drive: f32,
    pub split_rate: f32,
    pub just_driven: bool,
    pub just_loose: bool,
    pub enabled: bool,
}

impl Default for Wedge {
    fn default() -> Self {
        Self {
            drive: 0.0,
            max_drive: 100.0,
            split_rate: 1.0,
            just_driven: false,
            just_loose: false,
            enabled: true,
        }
    }
}

impl Wedge {
    pub fn split(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_driven = false;
        self.just_loose = false;
        let prev = self.drive;
        self.drive = (self.drive + amount).clamp(0.0, self.max_drive);
        if self.drive >= self.max_drive && prev < self.max_drive {
            self.just_driven = true;
        }
    }

    pub fn extract(&mut self, amount: f32) {
        if !self.enabled || self.drive <= 0.0 {
            return;
        }
        self.just_driven = false;
        self.just_loose = false;
        let prev = self.drive;
        self.drive = (self.drive - amount).max(0.0);
        if self.drive <= 0.0 && prev > 0.0 {
            self.just_loose = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.drive >= self.max_drive {
            return;
        }
        self.split(self.split_rate * dt);
    }

    pub fn is_driven(&self) -> bool {
        self.enabled && self.drive >= self.max_drive
    }

    pub fn is_loose(&self) -> bool {
        self.drive <= 0.0
    }

    pub fn drive_fraction(&self) -> f32 {
        if self.max_drive <= 0.0 {
            return 0.0;
        }
        self.drive / self.max_drive
    }

    pub fn effective_separation(&self, scale: f32) -> f32 {
        self.drive_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wedge() -> Wedge {
        Wedge {
            drive: 0.0,
            max_drive: 100.0,
            split_rate: 10.0,
            just_driven: false,
            just_loose: false,
            enabled: true,
        }
    }

    #[test]
    fn default_drive_zero() {
        let w = Wedge::default();
        assert_eq!(w.drive, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wedge::default().enabled);
    }

    #[test]
    fn split_increases_drive() {
        let mut w = wedge();
        w.split(30.0);
        assert_eq!(w.drive, 30.0);
    }

    #[test]
    fn split_clamps_at_max() {
        let mut w = wedge();
        w.split(200.0);
        assert_eq!(w.drive, 100.0);
    }

    #[test]
    fn split_no_op_when_disabled() {
        let mut w = wedge();
        w.enabled = false;
        w.split(50.0);
        assert_eq!(w.drive, 0.0);
    }

    #[test]
    fn split_sets_just_driven_at_max() {
        let mut w = wedge();
        w.split(100.0);
        assert!(w.just_driven);
    }

    #[test]
    fn split_no_just_driven_if_already_max() {
        let mut w = wedge();
        w.drive = 100.0;
        w.split(1.0);
        assert!(!w.just_driven);
    }

    #[test]
    fn extract_decreases_drive() {
        let mut w = wedge();
        w.drive = 60.0;
        w.extract(20.0);
        assert_eq!(w.drive, 40.0);
    }

    #[test]
    fn extract_clamps_at_zero() {
        let mut w = wedge();
        w.drive = 30.0;
        w.extract(200.0);
        assert_eq!(w.drive, 0.0);
    }

    #[test]
    fn extract_no_op_when_disabled() {
        let mut w = wedge();
        w.drive = 50.0;
        w.enabled = false;
        w.extract(10.0);
        assert_eq!(w.drive, 50.0);
    }

    #[test]
    fn extract_no_op_when_already_loose() {
        let mut w = wedge();
        w.extract(10.0);
        assert_eq!(w.drive, 0.0);
    }

    #[test]
    fn extract_sets_just_loose_at_zero() {
        let mut w = wedge();
        w.drive = 10.0;
        w.extract(10.0);
        assert!(w.just_loose);
    }

    #[test]
    fn extract_no_just_loose_if_already_zero() {
        let mut w = wedge();
        w.extract(1.0);
        assert!(!w.just_loose);
    }

    #[test]
    fn tick_increases_drive() {
        let mut w = wedge();
        w.tick(1.0);
        assert_eq!(w.drive, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wedge();
        w.tick(2.0);
        assert_eq!(w.drive, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wedge();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.drive, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_driven() {
        let mut w = wedge();
        w.drive = 100.0;
        w.tick(1.0);
        assert_eq!(w.drive, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wedge();
        w.split_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.drive, 0.0);
    }

    #[test]
    fn is_driven_true_at_max() {
        let mut w = wedge();
        w.drive = 100.0;
        assert!(w.is_driven());
    }

    #[test]
    fn is_driven_false_below_max() {
        let mut w = wedge();
        w.drive = 50.0;
        assert!(!w.is_driven());
    }

    #[test]
    fn is_driven_false_when_disabled() {
        let mut w = wedge();
        w.drive = 100.0;
        w.enabled = false;
        assert!(!w.is_driven());
    }

    #[test]
    fn is_loose_true_at_zero() {
        let w = wedge();
        assert!(w.is_loose());
    }

    #[test]
    fn is_loose_false_above_zero() {
        let mut w = wedge();
        w.drive = 1.0;
        assert!(!w.is_loose());
    }

    #[test]
    fn drive_fraction_zero_when_loose() {
        let w = wedge();
        assert_eq!(w.drive_fraction(), 0.0);
    }

    #[test]
    fn drive_fraction_one_at_max() {
        let mut w = wedge();
        w.drive = 100.0;
        assert_eq!(w.drive_fraction(), 1.0);
    }

    #[test]
    fn drive_fraction_half_at_midpoint() {
        let mut w = wedge();
        w.drive = 50.0;
        assert_eq!(w.drive_fraction(), 0.5);
    }

    #[test]
    fn drive_fraction_zero_when_max_zero() {
        let mut w = wedge();
        w.max_drive = 0.0;
        assert_eq!(w.drive_fraction(), 0.0);
    }

    #[test]
    fn effective_separation_scales() {
        let mut w = wedge();
        w.drive = 50.0;
        assert_eq!(w.effective_separation(2.0), 1.0);
    }

    #[test]
    fn effective_separation_zero_when_loose() {
        let w = wedge();
        assert_eq!(w.effective_separation(10.0), 0.0);
    }

    #[test]
    fn just_driven_cleared_on_next_split() {
        let mut w = wedge();
        w.split(100.0);
        assert!(w.just_driven);
        w.split(1.0);
        assert!(!w.just_driven);
    }

    #[test]
    fn just_loose_cleared_on_next_extract() {
        let mut w = wedge();
        w.drive = 10.0;
        w.extract(10.0);
        assert!(w.just_loose);
        w.drive = 10.0;
        w.extract(1.0);
        assert!(!w.just_loose);
    }
}

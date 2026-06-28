use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub Worry Worry {
    pub anxiety: f32,
    pub max_anxiety: f32,
    pub fret_rate: f32,
    pub just_anxious: bool,
    pub just_calm: bool,
    pub enabled: bool,
}

impl Default for Worry {
    fn default() -> Self {
        Self {
            anxiety: 0.0,
            max_anxiety: 100.0,
            fret_rate: 1.0,
            just_anxious: false,
            just_calm: false,
            enabled: true,
        }
    }
}

impl Worry {
    pub fn fret(&mut self, amount: f32) {
        if !self.enabled { return; }
        self.just_anxious = false;
        self.just_calm = false;
        let prev = self.anxiety;
        self.anxiety = (self.anxiety + amount).clamp(0.0, self.max_anxiety);
        if self.anxiety >= self.max_anxiety && prev < self.max_anxiety { self.just_anxious = true; }
    }

    pub fn calm(&mut self, amount: f32) {
        if !self.enabled || self.anxiety <= 0.0 { return; }
        self.just_anxious = false;
        self.just_calm = false;
        let prev = self.anxiety;
        self.anxiety = (self.anxiety - amount).max(0.0);
        if self.anxiety <= 0.0 && prev > 0.0 { self.just_calm = true; }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.anxiety >= self.max_anxiety { return; }
        self.fret(self.fret_rate * dt);
    }

    pub fn is_anxious(&self) -> bool { self.enabled && self.anxiety >= self.max_anxiety }
    pub fn is_calm(&self) -> bool { self.anxiety <= 0.0 }

    pub fn anxiety_fraction(&self) -> f32 {
        if self.max_anxiety <= 0.0 { return 0.0; }
        self.anxiety / self.max_anxiety
    }

    pub fn effective_dread(&self, scale: f32) -> f32 { self.anxiety_fraction() * scale }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inst() -> Worry {
        Worry { anxiety: 0.0, max_anxiety: 100.0, fret_rate: 10.0, just_anxious: false, just_calm: false, enabled: true }
    }
    #[test] fn default_anxiety_zero() { assert_eq!(Worry::default().anxiety, 0.0); }
    #[test] fn default_enabled() { assert!(Worry::default().enabled); }
    #[test] fn ADD_increases() { let mut w = inst(); w.fret(30.0); assert_eq!(w.anxiety, 30.0); }
    #[test] fn ADD_clamps_at_max() { let mut w = inst(); w.fret(200.0); assert_eq!(w.anxiety, 100.0); }
    #[test] fn ADD_no_op_when_disabled() { let mut w = inst(); w.enabled = false; w.fret(50.0); assert_eq!(w.anxiety, 0.0); }
    #[test] fn ADD_sets_just_anxious_at_max() { let mut w = inst(); w.fret(100.0); assert!(w.just_anxious); }
    #[test] fn ADD_no_just_anxious_if_already_max() { let mut w = inst(); w.anxiety = 100.0; w.fret(1.0); assert!(!w.just_anxious); }
    #[test] fn REMOVE_decreases() { let mut w = inst(); w.anxiety = 60.0; w.calm(20.0); assert_eq!(w.anxiety, 40.0); }
    #[test] fn REMOVE_clamps_at_zero() { let mut w = inst(); w.anxiety = 30.0; w.calm(200.0); assert_eq!(w.anxiety, 0.0); }
    #[test] fn REMOVE_no_op_when_disabled() { let mut w = inst(); w.anxiety = 50.0; w.enabled = false; w.calm(10.0); assert_eq!(w.anxiety, 50.0); }
    #[test] fn REMOVE_no_op_when_already_zero() { let mut w = inst(); w.calm(10.0); assert_eq!(w.anxiety, 0.0); }
    #[test] fn REMOVE_sets_just_calm_at_zero() { let mut w = inst(); w.anxiety = 10.0; w.calm(10.0); assert!(w.just_calm); }
    #[test] fn REMOVE_no_just_calm_if_already_zero() { let mut w = inst(); w.calm(1.0); assert!(!w.just_calm); }
    #[test] fn tick_increases() { let mut w = inst(); w.tick(1.0); assert_eq!(w.anxiety, 10.0); }
    #[test] fn tick_scales_with_dt() { let mut w = inst(); w.tick(2.0); assert_eq!(w.anxiety, 20.0); }
    #[test] fn tick_no_op_when_disabled() { let mut w = inst(); w.enabled = false; w.tick(1.0); assert_eq!(w.anxiety, 0.0); }
    #[test] fn tick_no_op_at_max() { let mut w = inst(); w.anxiety = 100.0; w.tick(1.0); assert_eq!(w.anxiety, 100.0); }
    #[test] fn tick_no_build_when_rate_zero() { let mut w = inst(); w.fret_rate = 0.0; w.tick(1.0); assert_eq!(w.anxiety, 0.0); }
    #[test] fn is_anxious_true_at_max() { let mut w = inst(); w.anxiety = 100.0; assert!(w.is_anxious()); }
    #[test] fn is_anxious_false_below_max() { let mut w = inst(); w.anxiety = 50.0; assert!(!w.is_anxious()); }
    #[test] fn is_anxious_false_when_disabled() { let mut w = inst(); w.anxiety = 100.0; w.enabled = false; assert!(!w.is_anxious()); }
    #[test] fn is_calm_true_at_zero() { let w = inst(); assert!(w.is_calm()); }
    #[test] fn is_calm_false_above_zero() { let mut w = inst(); w.anxiety = 1.0; assert!(!w.is_calm()); }
    #[test] fn anxiety_fraction_zero_when_zero() { let w = inst(); assert_eq!(w.anxiety_fraction(), 0.0); }
    #[test] fn anxiety_fraction_one_at_max() { let mut w = inst(); w.anxiety = 100.0; assert_eq!(w.anxiety_fraction(), 1.0); }
    #[test] fn anxiety_fraction_half_at_midpoint() { let mut w = inst(); w.anxiety = 50.0; assert_eq!(w.anxiety_fraction(), 0.5); }
    #[test] fn anxiety_fraction_zero_when_max_zero() { let mut w = inst(); w.max_anxiety = 0.0; assert_eq!(w.anxiety_fraction(), 0.0); }
    #[test] fn effective_dread_scales() { let mut w = inst(); w.anxiety = 50.0; assert_eq!(w.effective_dread(2.0), 1.0); }
    #[test] fn effective_dread_zero_when_zero() { let w = inst(); assert_eq!(w.effective_dread(10.0), 0.0); }
    #[test] fn just_anxious_cleared_on_next_ADD() { let mut w = inst(); w.fret(100.0); assert!(w.just_anxious); w.fret(1.0); assert!(!w.just_anxious); }
    #[test] fn just_calm_cleared_on_next_REMOVE() { let mut w = inst(); w.anxiety = 10.0; w.calm(10.0); assert!(w.just_calm); w.anxiety = 10.0; w.calm(1.0); assert!(!w.just_calm); }
}

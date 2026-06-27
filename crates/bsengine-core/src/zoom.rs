use bevy_ecs::prelude::Component;

/// Discrete step-based zoom control. Models a camera, scope, or magnification
/// lens that has a finite number of zoom steps, incrementable or
/// decrementable one step at a time. Distinct from continuous-range components
/// (Xray, Woo, etc.) by its **integer step count** — there is no in-between
/// position, matching the feel of a physical zoom ring.
///
/// `step_in()` increments `zoom_steps` by 1. Fires `just_stepped_in`. Fires
/// `just_maxed` the first time `zoom_steps` reaches `max_steps`. No-op at
/// max or when disabled.
///
/// `step_out()` decrements `zoom_steps` by 1. Fires `just_stepped_out`. Fires
/// `just_reset` the first time `zoom_steps` reaches 0. No-op at 0 or when
/// disabled.
///
/// `tick(_dt)` clears one-frame flags only. No time-based logic.
///
/// `is_zoomed()` returns `zoom_steps > 0 && enabled`.
///
/// `is_maxed()` returns `zoom_steps >= max_steps && enabled`.
///
/// `zoom_fraction()` returns `(zoom_steps as f32 / max_steps as f32).clamp(0.0, 1.0)`.
///
/// `effective_magnification(base)` returns `base * (1.0 + zoom_fraction())`
/// when enabled — 1× at 0 steps, 2× at maximum; `base` when disabled.
///
/// Default: `new(5)` — 5 discrete zoom steps.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoom {
    /// Current zoom step [0, max_steps].
    pub zoom_steps: u32,
    /// Maximum zoom depth. Clamped >= 1.
    pub max_steps: u32,
    pub just_stepped_in: bool,
    pub just_stepped_out: bool,
    pub just_maxed: bool,
    pub just_reset: bool,
    pub enabled: bool,
}

impl Zoom {
    pub fn new(max_steps: u32) -> Self {
        Self {
            zoom_steps: 0,
            max_steps: max_steps.max(1),
            just_stepped_in: false,
            just_stepped_out: false,
            just_maxed: false,
            just_reset: false,
            enabled: true,
        }
    }

    /// Zoom in one step. Fires `just_stepped_in`; fires `just_maxed` on first
    /// reaching cap. No-op at max or when disabled.
    pub fn step_in(&mut self) {
        if !self.enabled || self.zoom_steps >= self.max_steps {
            return;
        }
        self.zoom_steps += 1;
        self.just_stepped_in = true;
        if self.zoom_steps >= self.max_steps {
            self.just_maxed = true;
        }
    }

    /// Zoom out one step. Fires `just_stepped_out`; fires `just_reset` on
    /// first reaching 0. No-op at 0 or when disabled.
    pub fn step_out(&mut self) {
        if !self.enabled || self.zoom_steps == 0 {
            return;
        }
        self.zoom_steps -= 1;
        self.just_stepped_out = true;
        if self.zoom_steps == 0 {
            self.just_reset = true;
        }
    }

    /// Advance one frame: clear one-frame flags only. No time-based changes.
    pub fn tick(&mut self, _dt: f32) {
        self.just_stepped_in = false;
        self.just_stepped_out = false;
        self.just_maxed = false;
        self.just_reset = false;
    }

    /// `true` when at least one step zoomed in and component is enabled.
    pub fn is_zoomed(&self) -> bool {
        self.zoom_steps > 0 && self.enabled
    }

    /// `true` when at maximum zoom steps and component is enabled.
    pub fn is_maxed(&self) -> bool {
        self.zoom_steps >= self.max_steps && self.enabled
    }

    /// Current zoom as a fraction of maximum [0.0, 1.0].
    pub fn zoom_fraction(&self) -> f32 {
        (self.zoom_steps as f32 / self.max_steps as f32).clamp(0.0, 1.0)
    }

    /// Scale `base` by zoom level. Returns `base * (1.0 + zoom_fraction())`
    /// when enabled — 1× at 0 steps, 2× at maximum; `base` when disabled.
    pub fn effective_magnification(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.zoom_fraction())
    }
}

impl Default for Zoom {
    fn default() -> Self {
        Self::new(5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoom {
        Zoom::new(5) // 5 steps
    }

    // --- construction ---

    #[test]
    fn new_starts_at_zero() {
        let z = z();
        assert_eq!(z.zoom_steps, 0);
        assert!(!z.just_stepped_in);
        assert!(!z.just_stepped_out);
        assert!(!z.just_maxed);
        assert!(!z.just_reset);
        assert!(!z.is_zoomed());
        assert!(!z.is_maxed());
    }

    #[test]
    fn max_steps_clamped_to_one() {
        let z = Zoom::new(0);
        assert_eq!(z.max_steps, 1);
    }

    // --- step_in ---

    #[test]
    fn step_in_increments_steps() {
        let mut z = z();
        z.step_in();
        assert_eq!(z.zoom_steps, 1);
    }

    #[test]
    fn step_in_fires_just_stepped_in() {
        let mut z = z();
        z.step_in();
        assert!(z.just_stepped_in);
    }

    #[test]
    fn step_in_fires_just_maxed_at_cap() {
        let mut z = Zoom::new(2);
        z.step_in(); // 1
        z.tick(0.016);
        z.step_in(); // 2 = max
        assert!(z.just_maxed);
        assert!(z.just_stepped_in);
    }

    #[test]
    fn step_in_no_op_at_max() {
        let mut z = z();
        for _ in 0..5 {
            z.step_in();
        }
        z.tick(0.016); // clear flags set by the 5th step
        z.step_in(); // already at max, no-op
        assert_eq!(z.zoom_steps, 5);
        assert!(!z.just_stepped_in);
    }

    #[test]
    fn step_in_does_not_refire_just_maxed_after_cap() {
        let mut z = z();
        for _ in 0..5 {
            z.step_in();
        }
        z.tick(0.016);
        z.step_in(); // no-op
        assert!(!z.just_maxed);
    }

    #[test]
    fn step_in_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.step_in();
        assert_eq!(z.zoom_steps, 0);
        assert!(!z.just_stepped_in);
    }

    // --- step_out ---

    #[test]
    fn step_out_decrements_steps() {
        let mut z = z();
        z.step_in();
        z.step_in();
        z.step_out();
        assert_eq!(z.zoom_steps, 1);
    }

    #[test]
    fn step_out_fires_just_stepped_out() {
        let mut z = z();
        z.step_in();
        z.step_out();
        assert!(z.just_stepped_out);
    }

    #[test]
    fn step_out_fires_just_reset_at_zero() {
        let mut z = z();
        z.step_in();
        z.step_out(); // back to 0
        assert!(z.just_reset);
        assert!(z.just_stepped_out);
    }

    #[test]
    fn step_out_no_op_at_zero() {
        let mut z = z();
        z.step_out(); // already 0
        assert!(!z.just_stepped_out);
        assert_eq!(z.zoom_steps, 0);
    }

    #[test]
    fn step_out_no_op_when_disabled() {
        let mut z = z();
        z.step_in();
        z.enabled = false;
        z.step_out();
        assert_eq!(z.zoom_steps, 1);
        assert!(!z.just_stepped_out);
    }

    // --- tick ---

    #[test]
    fn tick_clears_all_flags() {
        let mut z = z();
        z.just_stepped_in = true;
        z.just_stepped_out = true;
        z.just_maxed = true;
        z.just_reset = true;
        z.tick(0.016);
        assert!(!z.just_stepped_in);
        assert!(!z.just_stepped_out);
        assert!(!z.just_maxed);
        assert!(!z.just_reset);
    }

    #[test]
    fn tick_does_not_change_zoom_steps() {
        let mut z = z();
        z.step_in();
        z.step_in();
        z.tick(1000.0); // no time-based change
        assert_eq!(z.zoom_steps, 2);
    }

    // --- is_zoomed / is_maxed ---

    #[test]
    fn is_zoomed_false_at_zero() {
        let z = z();
        assert!(!z.is_zoomed());
    }

    #[test]
    fn is_zoomed_true_after_step_in() {
        let mut z = z();
        z.step_in();
        assert!(z.is_zoomed());
    }

    #[test]
    fn is_zoomed_false_when_disabled() {
        let mut z = z();
        z.step_in();
        z.enabled = false;
        assert!(!z.is_zoomed());
    }

    #[test]
    fn is_maxed_false_below_cap() {
        let mut z = z();
        z.step_in();
        assert!(!z.is_maxed());
    }

    #[test]
    fn is_maxed_true_at_cap() {
        let mut z = z();
        for _ in 0..5 {
            z.step_in();
        }
        assert!(z.is_maxed());
    }

    #[test]
    fn is_maxed_false_when_disabled() {
        let mut z = z();
        for _ in 0..5 {
            z.step_in();
        }
        z.enabled = false;
        assert!(!z.is_maxed());
    }

    // --- zoom_fraction ---

    #[test]
    fn zoom_fraction_zero_at_base() {
        let z = z();
        assert_eq!(z.zoom_fraction(), 0.0);
    }

    #[test]
    fn zoom_fraction_at_partial() {
        let mut z = Zoom::new(4); // max=4
        z.step_in();
        z.step_in(); // 2/4=0.5
        assert!((z.zoom_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn zoom_fraction_one_at_max() {
        let mut z = z(); // max=5
        for _ in 0..5 {
            z.step_in();
        }
        assert!((z.zoom_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_magnification ---

    #[test]
    fn effective_magnification_passthrough_at_zero() {
        let z = z(); // fraction=0 → 100*(1+0)=100
        assert!((z.effective_magnification(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_magnification_at_half_steps() {
        let mut z = Zoom::new(4);
        z.step_in();
        z.step_in(); // fraction=0.5 → 100*(1+0.5)=150
        assert!((z.effective_magnification(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_magnification_doubled_at_max() {
        let mut z = z(); // fraction=1.0 → 100*(1+1)=200
        for _ in 0..5 {
            z.step_in();
        }
        assert!((z.effective_magnification(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_magnification_passthrough_when_disabled() {
        let mut z = z();
        for _ in 0..5 {
            z.step_in();
        }
        z.enabled = false;
        assert!((z.effective_magnification(100.0) - 100.0).abs() < 1e-4);
    }

    // --- in/out cycle ---

    #[test]
    fn step_in_out_cycle() {
        let mut z = z();
        z.step_in();
        z.step_in();
        z.step_in();
        z.step_out();
        assert_eq!(z.zoom_steps, 2);
        z.step_out();
        z.step_out();
        assert_eq!(z.zoom_steps, 0);
        assert!(z.just_reset);
    }
}

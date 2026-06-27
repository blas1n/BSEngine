use bevy_ecs::prelude::Component;

/// Building-momentum speed bonus: each consecutive uninterrupted step in the
/// same direction increments `stride_count` (up to `max_strides`).
/// `effective_speed(base)` scales with accumulated strides, rewarding
/// sustained directional movement over erratic repositioning.
///
/// `step()` adds one stride and sets `just_peaked` on the first frame
/// `stride_count` reaches `max_strides`. `break_stride()` resets the count
/// and sets `just_broke` if strides were lost. `tick()` clears one-frame
/// flags each frame.
///
/// `step()` is a no-op when disabled. `break_stride()` is always allowed so
/// external systems (collision, directional change) can reset momentum even
/// when the component is disabled.
///
/// Distinct from `Sprint` (binary fast/slow toggle), `Move_Speed` (flat
/// speed multiplier), and `Dash` (burst displacement): Stride is a
/// **building-momentum system** — speed grows gradually with consecutive
/// steps and resets immediately on disruption.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Stride {
    /// Current consecutive stride count.
    pub stride_count: u32,
    /// Maximum strides before the bonus caps. Clamped ≥ 1.
    pub max_strides: u32,
    /// Speed bonus added per stride as a fraction of base speed. Clamped ≥ 0.0.
    pub speed_bonus: f32,
    pub just_peaked: bool,
    pub just_broke: bool,
    pub enabled: bool,
}

impl Stride {
    pub fn new(max_strides: u32, speed_bonus: f32) -> Self {
        Self {
            stride_count: 0,
            max_strides: max_strides.max(1),
            speed_bonus: speed_bonus.max(0.0),
            just_peaked: false,
            just_broke: false,
            enabled: true,
        }
    }

    /// Register one consecutive step. Increments `stride_count` up to
    /// `max_strides`. Sets `just_peaked` on the first frame the count
    /// reaches the cap. No-op when disabled.
    pub fn step(&mut self) {
        if !self.enabled {
            return;
        }
        if self.stride_count < self.max_strides {
            let was_below = self.stride_count < self.max_strides;
            self.stride_count += 1;
            if was_below && self.stride_count == self.max_strides {
                self.just_peaked = true;
            }
        }
    }

    /// Reset stride momentum (direction change, collision, stop). Sets
    /// `just_broke` when strides were lost. Always applies, even when disabled.
    pub fn break_stride(&mut self) {
        if self.stride_count > 0 {
            self.stride_count = 0;
            self.just_broke = true;
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_peaked = false;
        self.just_broke = false;
    }

    /// Effective movement speed with accumulated stride bonus.
    /// Returns `base * (1 + stride_count * speed_bonus)` when enabled;
    /// returns `base` when disabled or stride_count is 0.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if self.enabled && self.stride_count > 0 {
            base * (1.0 + self.stride_count as f32 * self.speed_bonus)
        } else {
            base
        }
    }

    /// `true` when `stride_count` has reached `max_strides` and is enabled.
    pub fn is_at_peak(&self) -> bool {
        self.enabled && self.stride_count >= self.max_strides
    }

    /// Fraction of max strides built up [0.0 = none, 1.0 = fully peaked].
    pub fn stride_fraction(&self) -> f32 {
        (self.stride_count as f32 / self.max_strides as f32).clamp(0.0, 1.0)
    }
}

impl Default for Stride {
    fn default() -> Self {
        Self::new(5, 0.05)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_increments_count() {
        let mut s = Stride::new(5, 0.1);
        s.step();
        assert_eq!(s.stride_count, 1);
    }

    #[test]
    fn step_caps_at_max() {
        let mut s = Stride::new(3, 0.1);
        s.step();
        s.step();
        s.step();
        s.step(); // over cap
        assert_eq!(s.stride_count, 3);
    }

    #[test]
    fn step_fires_just_peaked_on_reaching_max() {
        let mut s = Stride::new(3, 0.1);
        s.step();
        s.step();
        assert!(!s.just_peaked);
        s.step();
        assert!(s.just_peaked);
    }

    #[test]
    fn step_no_just_peaked_when_already_at_max() {
        let mut s = Stride::new(2, 0.1);
        s.step();
        s.step(); // peaks
        s.tick();
        s.step(); // still at max — no new peak event
        assert!(!s.just_peaked);
    }

    #[test]
    fn step_no_op_when_disabled() {
        let mut s = Stride::new(5, 0.1);
        s.enabled = false;
        s.step();
        assert_eq!(s.stride_count, 0);
    }

    #[test]
    fn break_stride_resets_count() {
        let mut s = Stride::new(5, 0.1);
        s.step();
        s.step();
        s.break_stride();
        assert_eq!(s.stride_count, 0);
        assert!(s.just_broke);
    }

    #[test]
    fn break_stride_no_op_when_already_zero() {
        let mut s = Stride::new(5, 0.1);
        s.break_stride();
        assert!(!s.just_broke);
    }

    #[test]
    fn break_stride_allowed_when_disabled() {
        let mut s = Stride::new(5, 0.1);
        s.stride_count = 3;
        s.enabled = false;
        s.break_stride();
        assert_eq!(s.stride_count, 0);
        assert!(s.just_broke);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut s = Stride::new(2, 0.1);
        s.step();
        s.step();
        s.tick();
        assert!(!s.just_peaked);
    }

    #[test]
    fn tick_clears_just_broke() {
        let mut s = Stride::new(5, 0.1);
        s.step();
        s.break_stride();
        s.tick();
        assert!(!s.just_broke);
    }

    #[test]
    fn effective_speed_scales_with_strides() {
        let mut s = Stride::new(5, 0.1);
        s.step();
        s.step(); // 2 strides: 100 * (1 + 2*0.1) = 120
        assert!((s.effective_speed(100.0) - 120.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_at_peak() {
        let mut s = Stride::new(4, 0.25);
        s.step();
        s.step();
        s.step();
        s.step(); // 4 strides: 100 * (1 + 4*0.25) = 200
        assert!((s.effective_speed(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_base_when_no_strides() {
        let s = Stride::new(5, 0.1);
        assert!((s.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_base_when_disabled() {
        let mut s = Stride::new(5, 0.1);
        s.step();
        s.step();
        s.enabled = false;
        assert!((s.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn is_at_peak_true_at_max() {
        let mut s = Stride::new(3, 0.1);
        s.step();
        s.step();
        s.step();
        assert!(s.is_at_peak());
    }

    #[test]
    fn is_at_peak_false_below_max() {
        let mut s = Stride::new(3, 0.1);
        s.step();
        s.step();
        assert!(!s.is_at_peak());
    }

    #[test]
    fn is_at_peak_false_when_disabled() {
        let mut s = Stride::new(2, 0.1);
        s.step();
        s.step();
        s.enabled = false;
        assert!(!s.is_at_peak());
    }

    #[test]
    fn stride_fraction_at_half() {
        let mut s = Stride::new(4, 0.1);
        s.step();
        s.step();
        assert!((s.stride_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn stride_fraction_zero_at_start() {
        let s = Stride::new(4, 0.1);
        assert_eq!(s.stride_fraction(), 0.0);
    }

    #[test]
    fn max_strides_clamped_to_one() {
        let s = Stride::new(0, 0.1);
        assert_eq!(s.max_strides, 1);
    }

    #[test]
    fn can_restride_after_break() {
        let mut s = Stride::new(3, 0.1);
        s.step();
        s.step();
        s.step();
        s.break_stride();
        s.tick();
        s.step();
        assert_eq!(s.stride_count, 1);
        assert!(!s.just_peaked);
    }
}

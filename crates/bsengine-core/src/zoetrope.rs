use bevy_ecs::prelude::Component;

/// Persistence-of-vision animation tracker. `frame` builds via
/// `advance(amount)` and spins passively at `spin_rate` per second in
/// `tick(dt)` or stalls immediately via `stall(amount)`.
///
/// Models optical-toy frame-sequence meters, flip-book animation progress
/// bars, stroboscopic-effect intensity accumulators, phenakistoscope
/// rotation gauges, persistence-of-vision flicker trackers, pre-cinema
/// animation frame fill levels, motion-illusion depth indicators,
/// rotating-drum frame-count bars, Victorian-novelty-toy charge meters,
/// or any mechanic where still images spun fast enough fuse into seamless
/// motion — only for the illusion to collapse when the drum slows to a
/// halt.
///
/// `advance(amount)` adds frame; fires `just_cycling` when first
/// reaching `max_frame`. No-op when disabled.
///
/// `stall(amount)` reduces frame immediately; fires `just_stalled`
/// when reaching 0. No-op when disabled or already stalled.
///
/// `tick(dt)` clears both flags, then increases frame by
/// `spin_rate * dt` (capped at `max_frame`). Fires `just_cycling`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_cycling()` returns `frame >= max_frame && enabled`.
///
/// `is_stalled()` returns `frame == 0.0` (not gated by `enabled`).
///
/// `frame_fraction()` returns `(frame / max_frame).clamp(0, 1)`.
///
/// `effective_illusion(scale)` returns `scale * frame_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 6.0)` — spins at 6 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoetrope {
    pub frame: f32,
    pub max_frame: f32,
    pub spin_rate: f32,
    pub just_cycling: bool,
    pub just_stalled: bool,
    pub enabled: bool,
}

impl Zoetrope {
    pub fn new(max_frame: f32, spin_rate: f32) -> Self {
        Self {
            frame: 0.0,
            max_frame: max_frame.max(0.1),
            spin_rate: spin_rate.max(0.0),
            just_cycling: false,
            just_stalled: false,
            enabled: true,
        }
    }

    /// Add frame; fires `just_cycling` when first reaching max.
    /// No-op when disabled.
    pub fn advance(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.frame < self.max_frame;
        self.frame = (self.frame + amount).min(self.max_frame);
        if was_below && self.frame >= self.max_frame {
            self.just_cycling = true;
        }
    }

    /// Reduce frame; fires `just_stalled` when reaching 0.
    /// No-op when disabled or already stalled.
    pub fn stall(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.frame <= 0.0 {
            return;
        }
        self.frame = (self.frame - amount).max(0.0);
        if self.frame <= 0.0 {
            self.just_stalled = true;
        }
    }

    /// Clear flags, then increase frame by `spin_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_cycling = false;
        self.just_stalled = false;
        if self.enabled && self.spin_rate > 0.0 && self.frame < self.max_frame {
            let was_below = self.frame < self.max_frame;
            self.frame = (self.frame + self.spin_rate * dt).min(self.max_frame);
            if was_below && self.frame >= self.max_frame {
                self.just_cycling = true;
            }
        }
    }

    /// `true` when frame is at maximum and component is enabled.
    pub fn is_cycling(&self) -> bool {
        self.frame >= self.max_frame && self.enabled
    }

    /// `true` when frame is 0 (not gated by `enabled`).
    pub fn is_stalled(&self) -> bool {
        self.frame == 0.0
    }

    /// Fraction of maximum frame [0.0, 1.0].
    pub fn frame_fraction(&self) -> f32 {
        (self.frame / self.max_frame).clamp(0.0, 1.0)
    }

    /// Returns `scale * frame_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_illusion(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.frame_fraction()
    }
}

impl Default for Zoetrope {
    fn default() -> Self {
        Self::new(100.0, 6.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoetrope {
        Zoetrope::new(100.0, 6.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_stalled() {
        let z = z();
        assert_eq!(z.frame, 0.0);
        assert!(z.is_stalled());
        assert!(!z.is_cycling());
    }

    #[test]
    fn new_clamps_max_frame() {
        let z = Zoetrope::new(-5.0, 6.0);
        assert!((z.max_frame - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_spin_rate() {
        let z = Zoetrope::new(100.0, -3.0);
        assert_eq!(z.spin_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoetrope::default();
        assert!((z.max_frame - 100.0).abs() < 1e-5);
        assert!((z.spin_rate - 6.0).abs() < 1e-5);
    }

    // --- advance ---

    #[test]
    fn advance_adds_frame() {
        let mut z = z();
        z.advance(40.0);
        assert!((z.frame - 40.0).abs() < 1e-3);
    }

    #[test]
    fn advance_clamps_at_max() {
        let mut z = z();
        z.advance(200.0);
        assert!((z.frame - 100.0).abs() < 1e-3);
    }

    #[test]
    fn advance_fires_just_cycling_at_max() {
        let mut z = z();
        z.advance(100.0);
        assert!(z.just_cycling);
        assert!(z.is_cycling());
    }

    #[test]
    fn advance_no_just_cycling_when_already_at_max() {
        let mut z = z();
        z.frame = 100.0;
        z.advance(10.0);
        assert!(!z.just_cycling);
    }

    #[test]
    fn advance_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.advance(50.0);
        assert_eq!(z.frame, 0.0);
    }

    #[test]
    fn advance_no_op_when_amount_zero() {
        let mut z = z();
        z.advance(0.0);
        assert_eq!(z.frame, 0.0);
    }

    // --- stall ---

    #[test]
    fn stall_reduces_frame() {
        let mut z = z();
        z.frame = 60.0;
        z.stall(20.0);
        assert!((z.frame - 40.0).abs() < 1e-3);
    }

    #[test]
    fn stall_clamps_at_zero() {
        let mut z = z();
        z.frame = 30.0;
        z.stall(200.0);
        assert_eq!(z.frame, 0.0);
    }

    #[test]
    fn stall_fires_just_stalled_at_zero() {
        let mut z = z();
        z.frame = 30.0;
        z.stall(30.0);
        assert!(z.just_stalled);
    }

    #[test]
    fn stall_no_op_when_already_stalled() {
        let mut z = z();
        z.stall(10.0);
        assert!(!z.just_stalled);
    }

    #[test]
    fn stall_no_op_when_disabled() {
        let mut z = z();
        z.frame = 50.0;
        z.enabled = false;
        z.stall(50.0);
        assert!((z.frame - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_spins_frame() {
        let mut z = z(); // rate=6
        z.tick(2.0); // 0 + 6*2 = 12
        assert!((z.frame - 12.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_cycling_on_spin_to_max() {
        let mut z = Zoetrope::new(100.0, 200.0);
        z.frame = 95.0;
        z.tick(1.0);
        assert!(z.just_cycling);
        assert!(z.is_cycling());
    }

    #[test]
    fn tick_no_spin_when_already_cycling() {
        let mut z = z();
        z.frame = 100.0;
        z.tick(1.0);
        assert!(!z.just_cycling);
    }

    #[test]
    fn tick_no_spin_when_rate_zero() {
        let mut z = Zoetrope::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.frame, 0.0);
    }

    #[test]
    fn tick_no_spin_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.frame, 0.0);
    }

    #[test]
    fn tick_clears_just_cycling() {
        let mut z = Zoetrope::new(100.0, 200.0);
        z.frame = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_cycling);
    }

    #[test]
    fn tick_clears_just_stalled() {
        let mut z = z();
        z.frame = 10.0;
        z.stall(10.0);
        z.tick(0.016);
        assert!(!z.just_stalled);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=6
        z.tick(5.0); // 6*5 = 30
        assert!((z.frame - 30.0).abs() < 1e-3);
    }

    // --- is_cycling / is_stalled ---

    #[test]
    fn is_cycling_false_when_disabled() {
        let mut z = z();
        z.frame = 100.0;
        z.enabled = false;
        assert!(!z.is_cycling());
    }

    #[test]
    fn is_stalled_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_stalled());
    }

    // --- frame_fraction / effective_illusion ---

    #[test]
    fn frame_fraction_zero_when_stalled() {
        assert_eq!(z().frame_fraction(), 0.0);
    }

    #[test]
    fn frame_fraction_half_at_midpoint() {
        let mut z = z();
        z.frame = 50.0;
        assert!((z.frame_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_illusion_zero_when_stalled() {
        assert_eq!(z().effective_illusion(100.0), 0.0);
    }

    #[test]
    fn effective_illusion_scales_with_frame() {
        let mut z = z();
        z.frame = 75.0;
        assert!((z.effective_illusion(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_illusion_zero_when_disabled() {
        let mut z = z();
        z.frame = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_illusion(100.0), 0.0);
    }
}

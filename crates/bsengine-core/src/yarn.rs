use bevy_ecs::prelude::Component;

/// Unraveling-length tracker. Manages how much yarn has been played out from
/// a spool, in range [0, `max_length`]. `unroll(amount)` extends the yarn;
/// `rewind(amount)` retracts it. Models rope dispensers, grappling-hook
/// cable, fishing-line, or any finite-reach tether that gets paid out and
/// retrieved.
///
/// `unroll(amount)` increases `length`. Fires `just_snagged` when `length`
/// first reaches `max_length`. No-op when disabled, already taut, or
/// `amount <= 0`.
///
/// `rewind(amount)` decreases `length` toward 0. Fires `just_rewound` when
/// `length` first reaches 0. No-op when disabled, already coiled, or
/// `amount <= 0`.
///
/// `tick(_dt)` clears `just_snagged` and `just_rewound` only.
///
/// `is_taut()` returns `length >= max_length && enabled`.
///
/// `is_coiled()` returns `length == 0.0` (not gated by `enabled`).
///
/// `length_fraction()` returns `(length / max_length).clamp(0, 1)`.
///
/// `effective_reach(base)` returns `base * length_fraction()` when enabled;
/// `0.0` when disabled.
///
/// Default: `new(10.0)` — starts coiled (length = 0).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yarn {
    pub length: f32,
    pub max_length: f32,
    pub just_snagged: bool,
    pub just_rewound: bool,
    pub enabled: bool,
}

impl Yarn {
    pub fn new(max_length: f32) -> Self {
        Self {
            length: 0.0,
            max_length: max_length.max(0.1),
            just_snagged: false,
            just_rewound: false,
            enabled: true,
        }
    }

    /// Extend yarn by `amount`. Fires `just_snagged` on reaching `max_length`.
    /// No-op when disabled, already taut, or `amount <= 0`.
    pub fn unroll(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.length >= self.max_length {
            return;
        }
        self.length = (self.length + amount).min(self.max_length);
        if self.length >= self.max_length {
            self.just_snagged = true;
        }
    }

    /// Retract yarn by `amount` toward 0. Fires `just_rewound` on reaching 0.
    /// No-op when disabled, already coiled, or `amount <= 0`.
    pub fn rewind(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.length <= 0.0 {
            return;
        }
        self.length = (self.length - amount).max(0.0);
        if self.length <= 0.0 {
            self.just_rewound = true;
        }
    }

    /// Advance one frame: clear `just_snagged` and `just_rewound` only.
    pub fn tick(&mut self, _dt: f32) {
        self.just_snagged = false;
        self.just_rewound = false;
    }

    /// `true` when fully extended and component is enabled.
    pub fn is_taut(&self) -> bool {
        self.length >= self.max_length && self.enabled
    }

    /// `true` when fully coiled (not gated by `enabled`).
    pub fn is_coiled(&self) -> bool {
        self.length == 0.0
    }

    /// Fraction of max length paid out [0.0, 1.0].
    pub fn length_fraction(&self) -> f32 {
        (self.length / self.max_length).clamp(0.0, 1.0)
    }

    /// Returns `base * length_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_reach(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.length_fraction()
    }
}

impl Default for Yarn {
    fn default() -> Self {
        Self::new(10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yarn {
        Yarn::new(10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_coiled() {
        let y = y();
        assert_eq!(y.length, 0.0);
        assert!(y.is_coiled());
        assert!(!y.is_taut());
    }

    #[test]
    fn new_clamps_max_length() {
        let y = Yarn::new(-5.0);
        assert!((y.max_length - 0.1).abs() < 1e-5);
    }

    #[test]
    fn default_max_length_is_ten() {
        assert!((Yarn::default().max_length - 10.0).abs() < 1e-5);
    }

    // --- unroll ---

    #[test]
    fn unroll_increases_length() {
        let mut y = y();
        y.unroll(4.0);
        assert!((y.length - 4.0).abs() < 1e-4);
    }

    #[test]
    fn unroll_clamps_at_max() {
        let mut y = y();
        y.unroll(20.0);
        assert!((y.length - 10.0).abs() < 1e-5);
    }

    #[test]
    fn unroll_fires_just_snagged_at_max() {
        let mut y = y();
        y.unroll(10.0);
        assert!(y.just_snagged);
        assert!(y.is_taut());
    }

    #[test]
    fn unroll_no_op_when_already_taut() {
        let mut y = y();
        y.unroll(10.0);
        y.tick(0.016);
        y.unroll(1.0);
        assert!(!y.just_snagged); // no refire
    }

    #[test]
    fn unroll_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.unroll(5.0);
        assert_eq!(y.length, 0.0);
    }

    #[test]
    fn unroll_no_op_for_zero_amount() {
        let mut y = y();
        y.unroll(0.0);
        assert_eq!(y.length, 0.0);
    }

    // --- rewind ---

    #[test]
    fn rewind_decreases_length() {
        let mut y = y();
        y.unroll(7.0);
        y.rewind(3.0);
        assert!((y.length - 4.0).abs() < 1e-4);
    }

    #[test]
    fn rewind_clamps_at_zero() {
        let mut y = y();
        y.unroll(5.0);
        y.rewind(10.0);
        assert_eq!(y.length, 0.0);
    }

    #[test]
    fn rewind_fires_just_rewound_at_zero() {
        let mut y = y();
        y.unroll(5.0);
        y.rewind(5.0);
        assert!(y.just_rewound);
        assert!(y.is_coiled());
    }

    #[test]
    fn rewind_no_op_when_already_coiled() {
        let mut y = y();
        y.rewind(5.0);
        assert!(!y.just_rewound);
    }

    #[test]
    fn rewind_no_op_when_disabled() {
        let mut y = y();
        y.unroll(5.0);
        y.enabled = false;
        y.rewind(3.0);
        assert!((y.length - 5.0).abs() < 1e-4);
    }

    #[test]
    fn rewind_no_op_for_zero_amount() {
        let mut y = y();
        y.unroll(5.0);
        y.rewind(0.0);
        assert!((y.length - 5.0).abs() < 1e-4);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_snagged() {
        let mut y = y();
        y.unroll(10.0);
        y.tick(0.016);
        assert!(!y.just_snagged);
    }

    #[test]
    fn tick_clears_just_rewound() {
        let mut y = y();
        y.unroll(5.0);
        y.rewind(5.0);
        y.tick(0.016);
        assert!(!y.just_rewound);
    }

    #[test]
    fn tick_does_not_change_length() {
        let mut y = y();
        y.unroll(5.0);
        y.tick(1000.0);
        assert!((y.length - 5.0).abs() < 1e-5);
    }

    // --- is_taut / is_coiled ---

    #[test]
    fn is_taut_false_when_partial() {
        let mut y = y();
        y.unroll(5.0);
        assert!(!y.is_taut());
    }

    #[test]
    fn is_taut_false_when_disabled() {
        let mut y = y();
        y.unroll(10.0);
        y.enabled = false;
        assert!(!y.is_taut());
    }

    #[test]
    fn is_coiled_true_at_zero() {
        assert!(y().is_coiled());
    }

    #[test]
    fn is_coiled_false_when_unrolled() {
        let mut y = y();
        y.unroll(1.0);
        assert!(!y.is_coiled());
    }

    #[test]
    fn is_coiled_true_even_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_coiled()); // not gated by enabled
    }

    // --- fractions / effective ---

    #[test]
    fn length_fraction_zero_when_coiled() {
        assert_eq!(y().length_fraction(), 0.0);
    }

    #[test]
    fn length_fraction_half_at_midpoint() {
        let mut y = y();
        y.unroll(5.0);
        assert!((y.length_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn length_fraction_one_when_taut() {
        let mut y = y();
        y.unroll(10.0);
        assert!((y.length_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_reach_zero_when_coiled() {
        assert_eq!(y().effective_reach(100.0), 0.0);
    }

    #[test]
    fn effective_reach_scales_with_fraction() {
        let mut y = y();
        y.unroll(5.0);
        assert!((y.effective_reach(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_reach_zero_when_disabled() {
        let mut y = y();
        y.unroll(10.0);
        y.enabled = false;
        assert_eq!(y.effective_reach(100.0), 0.0);
    }
}

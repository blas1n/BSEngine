use bevy_ecs::prelude::Component;

/// Perspective-clarity accumulation tracker named after vista, the
/// noun meaning a distant view through or along an avenue or opening
/// — from the Italian vista, past participle of vedere (to see), from
/// the Latin videre. In Italian the word entered painting and
/// garden design first: a vista was the carefully managed sightline
/// that terminated a formal garden or a painted street scene, the
/// long corridor of vision that directed the eye to a focal point
/// at the end — a fountain, a temple, an arch, a vanishing point.
/// English borrowed the word in the seventeenth century when the
/// vocabulary of Italian garden design arrived with continental
/// travel and the nascent landscape movement, and from garden design
/// it generalised into any extended view: the vista from a hilltop
/// across a valley, the vista of a mountain range seen from a
/// lookout, the vista down a corridor of columns. In its figurative
/// extension the word became available for any expansive prospect
/// of future possibility or past understanding: a vista of
/// opportunity, a vista of misery, the vista opened by a new
/// discovery. The common structural element is extent — a vista is
/// not merely a view but a long view, a view that asks the eye and
/// the mind to reach beyond the immediate foreground into depth and
/// distance. In game mechanics, a vista mechanic models the slow
/// clearing of perception — the accumulation of information,
/// visibility, or understanding that eventually reaches a point of
/// full clarity. `clarity` builds via `illuminate(amount)` and
/// accumulates passively at `reveal_rate` per second in `tick(dt)` or
/// dims via `obscure(amount)`.
///
/// Models perspective-clarity fill levels, sightline-saturation bars,
/// vista-opening accumulators, field-of-view clarity gauges, panorama-
/// approach fill levels, visibility-saturation indicators, fog-clearing
/// accumulation bars, revelation-range meters, outlook-completion fill
/// levels, or any mechanic where a character, system, or collective
/// slowly accumulates visual, informational, or strategic clarity
/// until a full vista is reached — the fog rolls back, the horizon
/// extends, and the scope of the world becomes visible in its depth.
///
/// `illuminate(amount)` adds clarity; fires `just_revealed` when
/// first reaching `max_clarity`. No-op when disabled.
///
/// `obscure(amount)` reduces clarity immediately; fires `just_dimmed`
/// when reaching 0. No-op when disabled or already dim.
///
/// `tick(dt)` clears both flags, then increases clarity by
/// `reveal_rate * dt` (capped at `max_clarity`). Fires `just_revealed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_revealed()` returns `clarity >= max_clarity && enabled`.
///
/// `is_dimmed()` returns `clarity == 0.0` (not gated by `enabled`).
///
/// `clarity_fraction()` returns `(clarity / max_clarity).clamp(0, 1)`.
///
/// `effective_vista(scale)` returns `scale * clarity_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — reveals at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vista {
    pub clarity: f32,
    pub max_clarity: f32,
    pub reveal_rate: f32,
    pub just_revealed: bool,
    pub just_dimmed: bool,
    pub enabled: bool,
}

impl Vista {
    pub fn new(max_clarity: f32, reveal_rate: f32) -> Self {
        Self {
            clarity: 0.0,
            max_clarity: max_clarity.max(0.1),
            reveal_rate: reveal_rate.max(0.0),
            just_revealed: false,
            just_dimmed: false,
            enabled: true,
        }
    }

    /// Add clarity; fires `just_revealed` when first reaching max.
    /// No-op when disabled.
    pub fn illuminate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.clarity < self.max_clarity;
        self.clarity = (self.clarity + amount).min(self.max_clarity);
        if was_below && self.clarity >= self.max_clarity {
            self.just_revealed = true;
        }
    }

    /// Reduce clarity; fires `just_dimmed` when reaching 0.
    /// No-op when disabled or already dim.
    pub fn obscure(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.clarity <= 0.0 {
            return;
        }
        self.clarity = (self.clarity - amount).max(0.0);
        if self.clarity <= 0.0 {
            self.just_dimmed = true;
        }
    }

    /// Clear flags, then increase clarity by `reveal_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_revealed = false;
        self.just_dimmed = false;
        if self.enabled && self.reveal_rate > 0.0 && self.clarity < self.max_clarity {
            let was_below = self.clarity < self.max_clarity;
            self.clarity = (self.clarity + self.reveal_rate * dt).min(self.max_clarity);
            if was_below && self.clarity >= self.max_clarity {
                self.just_revealed = true;
            }
        }
    }

    /// `true` when clarity is at maximum and component is enabled.
    pub fn is_revealed(&self) -> bool {
        self.clarity >= self.max_clarity && self.enabled
    }

    /// `true` when clarity is 0 (not gated by `enabled`).
    pub fn is_dimmed(&self) -> bool {
        self.clarity == 0.0
    }

    /// Fraction of maximum clarity [0.0, 1.0].
    pub fn clarity_fraction(&self) -> f32 {
        (self.clarity / self.max_clarity).clamp(0.0, 1.0)
    }

    /// Returns `scale * clarity_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_vista(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.clarity_fraction()
    }
}

impl Default for Vista {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Vista {
        Vista::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_dimmed() {
        let v = v();
        assert_eq!(v.clarity, 0.0);
        assert!(v.is_dimmed());
        assert!(!v.is_revealed());
    }

    #[test]
    fn new_clamps_max_clarity() {
        let v = Vista::new(-5.0, 1.5);
        assert!((v.max_clarity - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_reveal_rate() {
        let v = Vista::new(100.0, -1.5);
        assert_eq!(v.reveal_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Vista::default();
        assert!((v.max_clarity - 100.0).abs() < 1e-5);
        assert!((v.reveal_rate - 1.5).abs() < 1e-5);
    }

    // --- illuminate ---

    #[test]
    fn illuminate_adds_clarity() {
        let mut v = v();
        v.illuminate(40.0);
        assert!((v.clarity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn illuminate_clamps_at_max() {
        let mut v = v();
        v.illuminate(200.0);
        assert!((v.clarity - 100.0).abs() < 1e-3);
    }

    #[test]
    fn illuminate_fires_just_revealed_at_max() {
        let mut v = v();
        v.illuminate(100.0);
        assert!(v.just_revealed);
        assert!(v.is_revealed());
    }

    #[test]
    fn illuminate_no_just_revealed_when_already_at_max() {
        let mut v = v();
        v.clarity = 100.0;
        v.illuminate(10.0);
        assert!(!v.just_revealed);
    }

    #[test]
    fn illuminate_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.illuminate(50.0);
        assert_eq!(v.clarity, 0.0);
    }

    #[test]
    fn illuminate_no_op_when_amount_zero() {
        let mut v = v();
        v.illuminate(0.0);
        assert_eq!(v.clarity, 0.0);
    }

    // --- obscure ---

    #[test]
    fn obscure_reduces_clarity() {
        let mut v = v();
        v.clarity = 60.0;
        v.obscure(20.0);
        assert!((v.clarity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn obscure_clamps_at_zero() {
        let mut v = v();
        v.clarity = 30.0;
        v.obscure(200.0);
        assert_eq!(v.clarity, 0.0);
    }

    #[test]
    fn obscure_fires_just_dimmed_at_zero() {
        let mut v = v();
        v.clarity = 30.0;
        v.obscure(30.0);
        assert!(v.just_dimmed);
    }

    #[test]
    fn obscure_no_op_when_already_dimmed() {
        let mut v = v();
        v.obscure(10.0);
        assert!(!v.just_dimmed);
    }

    #[test]
    fn obscure_no_op_when_disabled() {
        let mut v = v();
        v.clarity = 50.0;
        v.enabled = false;
        v.obscure(50.0);
        assert!((v.clarity - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_clarity() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.clarity - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_revealed_on_clarity_to_max() {
        let mut v = Vista::new(100.0, 200.0);
        v.clarity = 95.0;
        v.tick(1.0);
        assert!(v.just_revealed);
        assert!(v.is_revealed());
    }

    #[test]
    fn tick_no_build_when_already_revealed() {
        let mut v = v();
        v.clarity = 100.0;
        v.tick(1.0);
        assert!(!v.just_revealed);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Vista::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.clarity, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.clarity, 0.0);
    }

    #[test]
    fn tick_clears_just_revealed() {
        let mut v = Vista::new(100.0, 200.0);
        v.clarity = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_revealed);
    }

    #[test]
    fn tick_clears_just_dimmed() {
        let mut v = v();
        v.clarity = 10.0;
        v.obscure(10.0);
        v.tick(0.016);
        assert!(!v.just_dimmed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.clarity - 9.0).abs() < 1e-3);
    }

    // --- is_revealed / is_dimmed ---

    #[test]
    fn is_revealed_false_when_disabled() {
        let mut v = v();
        v.clarity = 100.0;
        v.enabled = false;
        assert!(!v.is_revealed());
    }

    #[test]
    fn is_dimmed_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_dimmed());
    }

    // --- clarity_fraction / effective_vista ---

    #[test]
    fn clarity_fraction_zero_when_dimmed() {
        assert_eq!(v().clarity_fraction(), 0.0);
    }

    #[test]
    fn clarity_fraction_half_at_midpoint() {
        let mut v = v();
        v.clarity = 50.0;
        assert!((v.clarity_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_vista_zero_when_dimmed() {
        assert_eq!(v().effective_vista(100.0), 0.0);
    }

    #[test]
    fn effective_vista_scales_with_clarity() {
        let mut v = v();
        v.clarity = 75.0;
        assert!((v.effective_vista(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vista_zero_when_disabled() {
        let mut v = v();
        v.clarity = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_vista(100.0), 0.0);
    }
}

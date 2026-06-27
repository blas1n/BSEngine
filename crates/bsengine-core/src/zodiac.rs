use bevy_ecs::prelude::Component;

/// Celestial-alignment saturation tracker. `alignment` builds via
/// `attune(amount)` and advances passively at `transit_rate` per
/// second in `tick(dt)` or disperses immediately via `dispel(amount)`.
///
/// Models astrological-sign progression fill levels, constellation-
/// transit completion trackers, celestial-belt-alignment saturation
/// bars, horoscope-cycle intensity gauges, astral-influence
/// accumulation meters, zodiacal-light intensity trackers, ecliptic-
/// position build-up indicators, planetary-house saturation fill
/// levels, star-sign resonance completion bars, or any mechanic
/// where patient celestial observation slowly aligns an observer
/// with the procession of twelve constellations across the ecliptic
/// until the full zodiacal wheel completes one slow revolution —
/// only for a single misread chart or an unexpected retrograde to
/// undo months of careful attunement back to an inauspicious blank.
///
/// `attune(amount)` adds alignment; fires `just_aligned` when
/// first reaching `max_alignment`. No-op when disabled.
///
/// `dispel(amount)` reduces alignment immediately; fires
/// `just_voided` when reaching 0. No-op when disabled or already
/// voided.
///
/// `tick(dt)` clears both flags, then increases alignment by
/// `transit_rate * dt` (capped at `max_alignment`). Fires
/// `just_aligned` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_aligned()` returns `alignment >= max_alignment && enabled`.
///
/// `is_voided()` returns `alignment == 0.0` (not gated by `enabled`).
///
/// `alignment_fraction()` returns `(alignment / max_alignment).clamp(0, 1)`.
///
/// `effective_influence(scale)` returns `scale * alignment_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — transits at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zodiac {
    pub alignment: f32,
    pub max_alignment: f32,
    pub transit_rate: f32,
    pub just_aligned: bool,
    pub just_voided: bool,
    pub enabled: bool,
}

impl Zodiac {
    pub fn new(max_alignment: f32, transit_rate: f32) -> Self {
        Self {
            alignment: 0.0,
            max_alignment: max_alignment.max(0.1),
            transit_rate: transit_rate.max(0.0),
            just_aligned: false,
            just_voided: false,
            enabled: true,
        }
    }

    /// Add alignment; fires `just_aligned` when first reaching max.
    /// No-op when disabled.
    pub fn attune(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.alignment < self.max_alignment;
        self.alignment = (self.alignment + amount).min(self.max_alignment);
        if was_below && self.alignment >= self.max_alignment {
            self.just_aligned = true;
        }
    }

    /// Reduce alignment; fires `just_voided` when reaching 0.
    /// No-op when disabled or already voided.
    pub fn dispel(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.alignment <= 0.0 {
            return;
        }
        self.alignment = (self.alignment - amount).max(0.0);
        if self.alignment <= 0.0 {
            self.just_voided = true;
        }
    }

    /// Clear flags, then increase alignment by `transit_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_aligned = false;
        self.just_voided = false;
        if self.enabled && self.transit_rate > 0.0 && self.alignment < self.max_alignment {
            let was_below = self.alignment < self.max_alignment;
            self.alignment = (self.alignment + self.transit_rate * dt).min(self.max_alignment);
            if was_below && self.alignment >= self.max_alignment {
                self.just_aligned = true;
            }
        }
    }

    /// `true` when alignment is at maximum and component is enabled.
    pub fn is_aligned(&self) -> bool {
        self.alignment >= self.max_alignment && self.enabled
    }

    /// `true` when alignment is 0 (not gated by `enabled`).
    pub fn is_voided(&self) -> bool {
        self.alignment == 0.0
    }

    /// Fraction of maximum alignment [0.0, 1.0].
    pub fn alignment_fraction(&self) -> f32 {
        (self.alignment / self.max_alignment).clamp(0.0, 1.0)
    }

    /// Returns `scale * alignment_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_influence(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.alignment_fraction()
    }
}

impl Default for Zodiac {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zodiac {
        Zodiac::new(100.0, 1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_voided() {
        let z = z();
        assert_eq!(z.alignment, 0.0);
        assert!(z.is_voided());
        assert!(!z.is_aligned());
    }

    #[test]
    fn new_clamps_max_alignment() {
        let z = Zodiac::new(-5.0, 1.0);
        assert!((z.max_alignment - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_transit_rate() {
        let z = Zodiac::new(100.0, -1.0);
        assert_eq!(z.transit_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zodiac::default();
        assert!((z.max_alignment - 100.0).abs() < 1e-5);
        assert!((z.transit_rate - 1.0).abs() < 1e-5);
    }

    // --- attune ---

    #[test]
    fn attune_adds_alignment() {
        let mut z = z();
        z.attune(40.0);
        assert!((z.alignment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn attune_clamps_at_max() {
        let mut z = z();
        z.attune(200.0);
        assert!((z.alignment - 100.0).abs() < 1e-3);
    }

    #[test]
    fn attune_fires_just_aligned_at_max() {
        let mut z = z();
        z.attune(100.0);
        assert!(z.just_aligned);
        assert!(z.is_aligned());
    }

    #[test]
    fn attune_no_just_aligned_when_already_at_max() {
        let mut z = z();
        z.alignment = 100.0;
        z.attune(10.0);
        assert!(!z.just_aligned);
    }

    #[test]
    fn attune_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.attune(50.0);
        assert_eq!(z.alignment, 0.0);
    }

    #[test]
    fn attune_no_op_when_amount_zero() {
        let mut z = z();
        z.attune(0.0);
        assert_eq!(z.alignment, 0.0);
    }

    // --- dispel ---

    #[test]
    fn dispel_reduces_alignment() {
        let mut z = z();
        z.alignment = 60.0;
        z.dispel(20.0);
        assert!((z.alignment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dispel_clamps_at_zero() {
        let mut z = z();
        z.alignment = 30.0;
        z.dispel(200.0);
        assert_eq!(z.alignment, 0.0);
    }

    #[test]
    fn dispel_fires_just_voided_at_zero() {
        let mut z = z();
        z.alignment = 30.0;
        z.dispel(30.0);
        assert!(z.just_voided);
    }

    #[test]
    fn dispel_no_op_when_already_voided() {
        let mut z = z();
        z.dispel(10.0);
        assert!(!z.just_voided);
    }

    #[test]
    fn dispel_no_op_when_disabled() {
        let mut z = z();
        z.alignment = 50.0;
        z.enabled = false;
        z.dispel(50.0);
        assert!((z.alignment - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_transits_alignment() {
        let mut z = z(); // rate=1
        z.tick(6.0); // 0 + 1*6 = 6
        assert!((z.alignment - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_aligned_on_transit_to_max() {
        let mut z = Zodiac::new(100.0, 200.0);
        z.alignment = 95.0;
        z.tick(1.0);
        assert!(z.just_aligned);
        assert!(z.is_aligned());
    }

    #[test]
    fn tick_no_transit_when_already_aligned() {
        let mut z = z();
        z.alignment = 100.0;
        z.tick(1.0);
        assert!(!z.just_aligned);
    }

    #[test]
    fn tick_no_transit_when_rate_zero() {
        let mut z = Zodiac::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.alignment, 0.0);
    }

    #[test]
    fn tick_no_transit_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.alignment, 0.0);
    }

    #[test]
    fn tick_clears_just_aligned() {
        let mut z = Zodiac::new(100.0, 200.0);
        z.alignment = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_aligned);
    }

    #[test]
    fn tick_clears_just_voided() {
        let mut z = z();
        z.alignment = 10.0;
        z.dispel(10.0);
        z.tick(0.016);
        assert!(!z.just_voided);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1
        z.tick(8.0); // 1*8 = 8
        assert!((z.alignment - 8.0).abs() < 1e-3);
    }

    // --- is_aligned / is_voided ---

    #[test]
    fn is_aligned_false_when_disabled() {
        let mut z = z();
        z.alignment = 100.0;
        z.enabled = false;
        assert!(!z.is_aligned());
    }

    #[test]
    fn is_voided_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_voided());
    }

    // --- alignment_fraction / effective_influence ---

    #[test]
    fn alignment_fraction_zero_when_voided() {
        assert_eq!(z().alignment_fraction(), 0.0);
    }

    #[test]
    fn alignment_fraction_half_at_midpoint() {
        let mut z = z();
        z.alignment = 50.0;
        assert!((z.alignment_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_influence_zero_when_voided() {
        assert_eq!(z().effective_influence(100.0), 0.0);
    }

    #[test]
    fn effective_influence_scales_with_alignment() {
        let mut z = z();
        z.alignment = 75.0;
        assert!((z.effective_influence(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_influence_zero_when_disabled() {
        let mut z = z();
        z.alignment = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_influence(100.0), 0.0);
    }
}

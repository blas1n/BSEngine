use bevy_ecs::prelude::Component;

/// Scent-mark territorial-presence tracker named after zibet, the large
/// civet (Civettictis civetta) found across sub-Saharan Africa and South
/// Asia whose perineal scent glands secrete a thick musk — civetone —
/// that has been harvested for perfumery since antiquity and whose
/// potency is so extreme that a single adult's territory can remain
/// chemically signed for weeks after the animal has moved on. Zibets
/// are solitary, nocturnal, and omnivorous; they patrol fixed circuit
/// paths through dense undergrowth and deposit scent precisely at
/// crossroads, tree bases, and elevated surfaces in a behaviour called
/// chin-marking or anogenital marking. A resident's territory is
/// therefore legible as a layered chemical document whose freshness
/// signals how recently the owner passed. `scent` builds via
/// `spray(amount)` and accumulates passively at `mark_rate` per second
/// in `tick(dt)` or fades via `wane(amount)`.
///
/// Models territorial-presence saturation bars, stealth-hunter scent-
/// dominance fill levels, wildlife-census signature-freshness gauges,
/// apex-predator range-marking saturation trackers, musk-gland output
/// fill levels, nocturnal-predator territory-freshness indicators,
/// circuit-patrol chemical-record accumulation bars, ambush-predator
/// presence-density fill levels, perfumery-reagent potency saturation
/// gauges, or any mechanic where a solitary nocturnal creature slowly
/// saturates a territory with a chemical signature so dense that every
/// other animal on the same circuit simply turns around and leaves
/// before ever catching sight of the animal that left it.
///
/// `spray(amount)` adds scent; fires `just_marked` when first reaching
/// `max_scent`. No-op when disabled.
///
/// `wane(amount)` reduces scent immediately; fires `just_faded` when
/// reaching 0. No-op when disabled or already faded.
///
/// `tick(dt)` clears both flags, then increases scent by
/// `mark_rate * dt` (capped at `max_scent`). Fires `just_marked` when
/// first reaching max. No-op when disabled or rate is 0.
///
/// `is_marked()` returns `scent >= max_scent && enabled`.
///
/// `is_faded()` returns `scent == 0.0` (not gated by `enabled`).
///
/// `scent_fraction()` returns `(scent / max_scent).clamp(0, 1)`.
///
/// `effective_territory(scale)` returns `scale * scent_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — marks at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zibet {
    pub scent: f32,
    pub max_scent: f32,
    pub mark_rate: f32,
    pub just_marked: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Zibet {
    pub fn new(max_scent: f32, mark_rate: f32) -> Self {
        Self {
            scent: 0.0,
            max_scent: max_scent.max(0.1),
            mark_rate: mark_rate.max(0.0),
            just_marked: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Add scent; fires `just_marked` when first reaching max.
    /// No-op when disabled.
    pub fn spray(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.scent < self.max_scent;
        self.scent = (self.scent + amount).min(self.max_scent);
        if was_below && self.scent >= self.max_scent {
            self.just_marked = true;
        }
    }

    /// Reduce scent; fires `just_faded` when reaching 0.
    /// No-op when disabled or already faded.
    pub fn wane(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.scent <= 0.0 {
            return;
        }
        self.scent = (self.scent - amount).max(0.0);
        if self.scent <= 0.0 {
            self.just_faded = true;
        }
    }

    /// Clear flags, then increase scent by `mark_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_marked = false;
        self.just_faded = false;
        if self.enabled && self.mark_rate > 0.0 && self.scent < self.max_scent {
            let was_below = self.scent < self.max_scent;
            self.scent = (self.scent + self.mark_rate * dt).min(self.max_scent);
            if was_below && self.scent >= self.max_scent {
                self.just_marked = true;
            }
        }
    }

    /// `true` when scent is at maximum and component is enabled.
    pub fn is_marked(&self) -> bool {
        self.scent >= self.max_scent && self.enabled
    }

    /// `true` when scent is 0 (not gated by `enabled`).
    pub fn is_faded(&self) -> bool {
        self.scent == 0.0
    }

    /// Fraction of maximum scent [0.0, 1.0].
    pub fn scent_fraction(&self) -> f32 {
        (self.scent / self.max_scent).clamp(0.0, 1.0)
    }

    /// Returns `scale * scent_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_territory(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.scent_fraction()
    }
}

impl Default for Zibet {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zibet {
        Zibet::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_faded() {
        let z = z();
        assert_eq!(z.scent, 0.0);
        assert!(z.is_faded());
        assert!(!z.is_marked());
    }

    #[test]
    fn new_clamps_max_scent() {
        let z = Zibet::new(-5.0, 1.5);
        assert!((z.max_scent - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_mark_rate() {
        let z = Zibet::new(100.0, -1.5);
        assert_eq!(z.mark_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zibet::default();
        assert!((z.max_scent - 100.0).abs() < 1e-5);
        assert!((z.mark_rate - 1.5).abs() < 1e-5);
    }

    // --- spray ---

    #[test]
    fn spray_adds_scent() {
        let mut z = z();
        z.spray(40.0);
        assert!((z.scent - 40.0).abs() < 1e-3);
    }

    #[test]
    fn spray_clamps_at_max() {
        let mut z = z();
        z.spray(200.0);
        assert!((z.scent - 100.0).abs() < 1e-3);
    }

    #[test]
    fn spray_fires_just_marked_at_max() {
        let mut z = z();
        z.spray(100.0);
        assert!(z.just_marked);
        assert!(z.is_marked());
    }

    #[test]
    fn spray_no_just_marked_when_already_at_max() {
        let mut z = z();
        z.scent = 100.0;
        z.spray(10.0);
        assert!(!z.just_marked);
    }

    #[test]
    fn spray_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.spray(50.0);
        assert_eq!(z.scent, 0.0);
    }

    #[test]
    fn spray_no_op_when_amount_zero() {
        let mut z = z();
        z.spray(0.0);
        assert_eq!(z.scent, 0.0);
    }

    // --- wane ---

    #[test]
    fn wane_reduces_scent() {
        let mut z = z();
        z.scent = 60.0;
        z.wane(20.0);
        assert!((z.scent - 40.0).abs() < 1e-3);
    }

    #[test]
    fn wane_clamps_at_zero() {
        let mut z = z();
        z.scent = 30.0;
        z.wane(200.0);
        assert_eq!(z.scent, 0.0);
    }

    #[test]
    fn wane_fires_just_faded_at_zero() {
        let mut z = z();
        z.scent = 30.0;
        z.wane(30.0);
        assert!(z.just_faded);
    }

    #[test]
    fn wane_no_op_when_already_faded() {
        let mut z = z();
        z.wane(10.0);
        assert!(!z.just_faded);
    }

    #[test]
    fn wane_no_op_when_disabled() {
        let mut z = z();
        z.scent = 50.0;
        z.enabled = false;
        z.wane(50.0);
        assert!((z.scent - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_marks_scent() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.scent - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_marked_on_mark_to_max() {
        let mut z = Zibet::new(100.0, 200.0);
        z.scent = 95.0;
        z.tick(1.0);
        assert!(z.just_marked);
        assert!(z.is_marked());
    }

    #[test]
    fn tick_no_mark_when_already_marked() {
        let mut z = z();
        z.scent = 100.0;
        z.tick(1.0);
        assert!(!z.just_marked);
    }

    #[test]
    fn tick_no_mark_when_rate_zero() {
        let mut z = Zibet::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.scent, 0.0);
    }

    #[test]
    fn tick_no_mark_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.scent, 0.0);
    }

    #[test]
    fn tick_clears_just_marked() {
        let mut z = Zibet::new(100.0, 200.0);
        z.scent = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_marked);
    }

    #[test]
    fn tick_clears_just_faded() {
        let mut z = z();
        z.scent = 10.0;
        z.wane(10.0);
        z.tick(0.016);
        assert!(!z.just_faded);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.scent - 9.0).abs() < 1e-3);
    }

    // --- is_marked / is_faded ---

    #[test]
    fn is_marked_false_when_disabled() {
        let mut z = z();
        z.scent = 100.0;
        z.enabled = false;
        assert!(!z.is_marked());
    }

    #[test]
    fn is_faded_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_faded());
    }

    // --- scent_fraction / effective_territory ---

    #[test]
    fn scent_fraction_zero_when_faded() {
        assert_eq!(z().scent_fraction(), 0.0);
    }

    #[test]
    fn scent_fraction_half_at_midpoint() {
        let mut z = z();
        z.scent = 50.0;
        assert!((z.scent_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_territory_zero_when_faded() {
        assert_eq!(z().effective_territory(100.0), 0.0);
    }

    #[test]
    fn effective_territory_scales_with_scent() {
        let mut z = z();
        z.scent = 75.0;
        assert!((z.effective_territory(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_territory_zero_when_disabled() {
        let mut z = z();
        z.scent = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_territory(100.0), 0.0);
    }
}

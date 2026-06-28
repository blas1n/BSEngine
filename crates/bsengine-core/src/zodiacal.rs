use bevy_ecs::prelude::Component;

/// Celestial-alignment accumulation tracker named after zodiacal, the
/// adjective meaning "of, relating to, or having the form of the
/// zodiac." The zodiac itself is the belt of sky roughly eight degrees
/// on either side of the ecliptic — the apparent annual path of the
/// Sun — within which the Moon and the planets also travel, and which
/// ancient Babylonian astronomers divided into twelve equal signs
/// named after the constellations they originally occupied. The most
/// striking visual phenomenon bearing this adjective is the zodiacal
/// light: a faint, diffuse, pyramid-shaped glow visible in the western
/// sky after evening twilight or the eastern sky before dawn, created
/// by sunlight scattered from billions of dust grains and meteoroid
/// debris concentrated in the plane of the ecliptic. These interplane-
/// tary dust particles are constantly replenished from comets and
/// collisions between asteroids, and their preferential concentration
/// along the ecliptic gives the zodiacal light a distinctly elongated
/// shape that follows the zodiac belt across the sky. In the best
/// conditions — away from light pollution, during periods when the
/// ecliptic makes a steep angle with the horizon — the zodiacal band
/// can be traced across the entire sky, connecting the evening and
/// morning cones through a faint, diffuse bridge called the gegenschein
/// at the antisolar point. `alignment` builds via `aspect(amount)` and
/// accumulates passively at `aspect_rate` per second in `tick(dt)` or
/// disperses via `depart(amount)`.
///
/// Models celestial-alignment fill levels, astral-correspondence
/// saturation bars, stellar-aspect accumulation trackers, zodiacal-
/// light intensity gauges, ecliptic-alignment fill levels, orbital-
/// resonance saturation indicators, planetary-conjunction
/// accumulation bars, horoscope-fulfilment meters, interplanetary-
/// dust-density fill levels, or any mechanic where a character or
/// artefact slowly attunes to celestial forces — absorbing the
/// influence of the planets as they cross each sign — until the
/// alignment reaches its peak and some cosmic event, power, or
/// revelation becomes available.
///
/// `aspect(amount)` adds alignment; fires `just_aligned` when first
/// reaching `max_alignment`. No-op when disabled.
///
/// `depart(amount)` reduces alignment immediately; fires
/// `just_discordant` when reaching 0. No-op when disabled or already
/// discordant.
///
/// `tick(dt)` clears both flags, then increases alignment by
/// `aspect_rate * dt` (capped at `max_alignment`). Fires `just_aligned`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_aligned()` returns `alignment >= max_alignment && enabled`.
///
/// `is_discordant()` returns `alignment == 0.0` (not gated by
/// `enabled`).
///
/// `alignment_fraction()` returns
/// `(alignment / max_alignment).clamp(0, 1)`.
///
/// `effective_celestial(scale)` returns `scale * alignment_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — aspects at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zodiacal {
    pub alignment: f32,
    pub max_alignment: f32,
    pub aspect_rate: f32,
    pub just_aligned: bool,
    pub just_discordant: bool,
    pub enabled: bool,
}

impl Zodiacal {
    pub fn new(max_alignment: f32, aspect_rate: f32) -> Self {
        Self {
            alignment: 0.0,
            max_alignment: max_alignment.max(0.1),
            aspect_rate: aspect_rate.max(0.0),
            just_aligned: false,
            just_discordant: false,
            enabled: true,
        }
    }

    /// Add alignment; fires `just_aligned` when first reaching max.
    /// No-op when disabled.
    pub fn aspect(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.alignment < self.max_alignment;
        self.alignment = (self.alignment + amount).min(self.max_alignment);
        if was_below && self.alignment >= self.max_alignment {
            self.just_aligned = true;
        }
    }

    /// Reduce alignment; fires `just_discordant` when reaching 0.
    /// No-op when disabled or already discordant.
    pub fn depart(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.alignment <= 0.0 {
            return;
        }
        self.alignment = (self.alignment - amount).max(0.0);
        if self.alignment <= 0.0 {
            self.just_discordant = true;
        }
    }

    /// Clear flags, then increase alignment by `aspect_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_aligned = false;
        self.just_discordant = false;
        if self.enabled && self.aspect_rate > 0.0 && self.alignment < self.max_alignment {
            let was_below = self.alignment < self.max_alignment;
            self.alignment = (self.alignment + self.aspect_rate * dt).min(self.max_alignment);
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
    pub fn is_discordant(&self) -> bool {
        self.alignment == 0.0
    }

    /// Fraction of maximum alignment [0.0, 1.0].
    pub fn alignment_fraction(&self) -> f32 {
        (self.alignment / self.max_alignment).clamp(0.0, 1.0)
    }

    /// Returns `scale * alignment_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_celestial(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.alignment_fraction()
    }
}

impl Default for Zodiacal {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zodiacal {
        Zodiacal::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_discordant() {
        let z = z();
        assert_eq!(z.alignment, 0.0);
        assert!(z.is_discordant());
        assert!(!z.is_aligned());
    }

    #[test]
    fn new_clamps_max_alignment() {
        let z = Zodiacal::new(-5.0, 1.5);
        assert!((z.max_alignment - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_aspect_rate() {
        let z = Zodiacal::new(100.0, -1.5);
        assert_eq!(z.aspect_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zodiacal::default();
        assert!((z.max_alignment - 100.0).abs() < 1e-5);
        assert!((z.aspect_rate - 1.5).abs() < 1e-5);
    }

    // --- aspect ---

    #[test]
    fn aspect_adds_alignment() {
        let mut z = z();
        z.aspect(40.0);
        assert!((z.alignment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn aspect_clamps_at_max() {
        let mut z = z();
        z.aspect(200.0);
        assert!((z.alignment - 100.0).abs() < 1e-3);
    }

    #[test]
    fn aspect_fires_just_aligned_at_max() {
        let mut z = z();
        z.aspect(100.0);
        assert!(z.just_aligned);
        assert!(z.is_aligned());
    }

    #[test]
    fn aspect_no_just_aligned_when_already_at_max() {
        let mut z = z();
        z.alignment = 100.0;
        z.aspect(10.0);
        assert!(!z.just_aligned);
    }

    #[test]
    fn aspect_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.aspect(50.0);
        assert_eq!(z.alignment, 0.0);
    }

    #[test]
    fn aspect_no_op_when_amount_zero() {
        let mut z = z();
        z.aspect(0.0);
        assert_eq!(z.alignment, 0.0);
    }

    // --- depart ---

    #[test]
    fn depart_reduces_alignment() {
        let mut z = z();
        z.alignment = 60.0;
        z.depart(20.0);
        assert!((z.alignment - 40.0).abs() < 1e-3);
    }

    #[test]
    fn depart_clamps_at_zero() {
        let mut z = z();
        z.alignment = 30.0;
        z.depart(200.0);
        assert_eq!(z.alignment, 0.0);
    }

    #[test]
    fn depart_fires_just_discordant_at_zero() {
        let mut z = z();
        z.alignment = 30.0;
        z.depart(30.0);
        assert!(z.just_discordant);
    }

    #[test]
    fn depart_no_op_when_already_discordant() {
        let mut z = z();
        z.depart(10.0);
        assert!(!z.just_discordant);
    }

    #[test]
    fn depart_no_op_when_disabled() {
        let mut z = z();
        z.alignment = 50.0;
        z.enabled = false;
        z.depart(50.0);
        assert!((z.alignment - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_aspects_alignment() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.alignment - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_aligned_on_aspect_to_max() {
        let mut z = Zodiacal::new(100.0, 200.0);
        z.alignment = 95.0;
        z.tick(1.0);
        assert!(z.just_aligned);
        assert!(z.is_aligned());
    }

    #[test]
    fn tick_no_aspect_when_already_aligned() {
        let mut z = z();
        z.alignment = 100.0;
        z.tick(1.0);
        assert!(!z.just_aligned);
    }

    #[test]
    fn tick_no_aspect_when_rate_zero() {
        let mut z = Zodiacal::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.alignment, 0.0);
    }

    #[test]
    fn tick_no_aspect_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.alignment, 0.0);
    }

    #[test]
    fn tick_clears_just_aligned() {
        let mut z = Zodiacal::new(100.0, 200.0);
        z.alignment = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_aligned);
    }

    #[test]
    fn tick_clears_just_discordant() {
        let mut z = z();
        z.alignment = 10.0;
        z.depart(10.0);
        z.tick(0.016);
        assert!(!z.just_discordant);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.alignment - 9.0).abs() < 1e-3);
    }

    // --- is_aligned / is_discordant ---

    #[test]
    fn is_aligned_false_when_disabled() {
        let mut z = z();
        z.alignment = 100.0;
        z.enabled = false;
        assert!(!z.is_aligned());
    }

    #[test]
    fn is_discordant_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_discordant());
    }

    // --- alignment_fraction / effective_celestial ---

    #[test]
    fn alignment_fraction_zero_when_discordant() {
        assert_eq!(z().alignment_fraction(), 0.0);
    }

    #[test]
    fn alignment_fraction_half_at_midpoint() {
        let mut z = z();
        z.alignment = 50.0;
        assert!((z.alignment_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_celestial_zero_when_discordant() {
        assert_eq!(z().effective_celestial(100.0), 0.0);
    }

    #[test]
    fn effective_celestial_scales_with_alignment() {
        let mut z = z();
        z.alignment = 75.0;
        assert!((z.effective_celestial(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_celestial_zero_when_disabled() {
        let mut z = z();
        z.alignment = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_celestial(100.0), 0.0);
    }
}

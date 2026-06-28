use bevy_ecs::prelude::Component;

/// Comedic-energy accumulation tracker named after zanily, the adverb
/// meaning "in a zany manner" — unpredictably comic, extravagantly
/// absurd, clownishly inventive. The root noun "zany" entered English
/// in the sixteenth century from the Italian "zani" or "zanni," the
/// stock comic servant character of commedia dell'arte who clowned
/// around the principal fool, imitating him clumsily and ineptly until
/// the imitation itself became the joke. The Zanni — plural of Zanni —
/// were the masked acrobats who filled the lazzi, the improvised comic
/// bits that no script contained and no audience could quite predict:
/// a sudden pratfall, an impossible cartwheel, a cascade of malapropisms
/// that somehow illuminated the very thing they misnamed. Over four
/// centuries this energy migrated from the licensed buffoon of the
/// Italian troupe through the English harlequinade, through Victorian
/// music hall and the silent-film slapstick of Chaplin and Keaton,
/// through the anarchic humour of the Marx Brothers and the physical
/// comedy of I Love Lucy, arriving in the modern era as shorthand for
/// any humour that exceeds the limits of decorum so completely that
/// decorum itself becomes the butt. To act zanily is to commit fully
/// to the absurd premise: to milk the pratfall longer than any sane
/// performer would, to escalate the misunderstanding past the point of
/// plausibility, to wear the lampshade not as accident but as policy.
/// `verve` builds via `jest(amount)` and accumulates passively at
/// `jest_rate` per second in `tick(dt)` or is deflated via
/// `dull(amount)`.
///
/// Models comedic-energy fill levels, clown-performance saturation
/// bars, improvisational-momentum accumulators, slapstick-intensity
/// gauges, zaniness-commitment fill levels, absurdist-energy saturation
/// indicators, buffoonery-escalation accumulation bars, pratfall-
/// readiness meters, harlequin-energy fill levels, or any mechanic
/// where a performer, character, or crowd slowly charges up with
/// comedic potential — gathering momentum through escalating absurdity
/// — until the verve peaks and every gag lands perfectly, the timing
/// pristine, the pratfall inevitable, and the audience helpless.
///
/// `jest(amount)` adds verve; fires `just_capered` when first reaching
/// `max_verve`. No-op when disabled.
///
/// `dull(amount)` reduces verve immediately; fires `just_deadpan` when
/// reaching 0. No-op when disabled or already deadpan.
///
/// `tick(dt)` clears both flags, then increases verve by
/// `jest_rate * dt` (capped at `max_verve`). Fires `just_capered`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_capered()` returns `verve >= max_verve && enabled`.
///
/// `is_deadpan()` returns `verve == 0.0` (not gated by `enabled`).
///
/// `verve_fraction()` returns `(verve / max_verve).clamp(0, 1)`.
///
/// `effective_hilarity(scale)` returns `scale * verve_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — jests at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zanily {
    pub verve: f32,
    pub max_verve: f32,
    pub jest_rate: f32,
    pub just_capered: bool,
    pub just_deadpan: bool,
    pub enabled: bool,
}

impl Zanily {
    pub fn new(max_verve: f32, jest_rate: f32) -> Self {
        Self {
            verve: 0.0,
            max_verve: max_verve.max(0.1),
            jest_rate: jest_rate.max(0.0),
            just_capered: false,
            just_deadpan: false,
            enabled: true,
        }
    }

    /// Add verve; fires `just_capered` when first reaching max.
    /// No-op when disabled.
    pub fn jest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.verve < self.max_verve;
        self.verve = (self.verve + amount).min(self.max_verve);
        if was_below && self.verve >= self.max_verve {
            self.just_capered = true;
        }
    }

    /// Reduce verve; fires `just_deadpan` when reaching 0.
    /// No-op when disabled or already deadpan.
    pub fn dull(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.verve <= 0.0 {
            return;
        }
        self.verve = (self.verve - amount).max(0.0);
        if self.verve <= 0.0 {
            self.just_deadpan = true;
        }
    }

    /// Clear flags, then increase verve by `jest_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_capered = false;
        self.just_deadpan = false;
        if self.enabled && self.jest_rate > 0.0 && self.verve < self.max_verve {
            let was_below = self.verve < self.max_verve;
            self.verve = (self.verve + self.jest_rate * dt).min(self.max_verve);
            if was_below && self.verve >= self.max_verve {
                self.just_capered = true;
            }
        }
    }

    /// `true` when verve is at maximum and component is enabled.
    pub fn is_capered(&self) -> bool {
        self.verve >= self.max_verve && self.enabled
    }

    /// `true` when verve is 0 (not gated by `enabled`).
    pub fn is_deadpan(&self) -> bool {
        self.verve == 0.0
    }

    /// Fraction of maximum verve [0.0, 1.0].
    pub fn verve_fraction(&self) -> f32 {
        (self.verve / self.max_verve).clamp(0.0, 1.0)
    }

    /// Returns `scale * verve_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_hilarity(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.verve_fraction()
    }
}

impl Default for Zanily {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zanily {
        Zanily::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_deadpan() {
        let z = z();
        assert_eq!(z.verve, 0.0);
        assert!(z.is_deadpan());
        assert!(!z.is_capered());
    }

    #[test]
    fn new_clamps_max_verve() {
        let z = Zanily::new(-5.0, 1.5);
        assert!((z.max_verve - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_jest_rate() {
        let z = Zanily::new(100.0, -1.5);
        assert_eq!(z.jest_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zanily::default();
        assert!((z.max_verve - 100.0).abs() < 1e-5);
        assert!((z.jest_rate - 1.5).abs() < 1e-5);
    }

    // --- jest ---

    #[test]
    fn jest_adds_verve() {
        let mut z = z();
        z.jest(40.0);
        assert!((z.verve - 40.0).abs() < 1e-3);
    }

    #[test]
    fn jest_clamps_at_max() {
        let mut z = z();
        z.jest(200.0);
        assert!((z.verve - 100.0).abs() < 1e-3);
    }

    #[test]
    fn jest_fires_just_capered_at_max() {
        let mut z = z();
        z.jest(100.0);
        assert!(z.just_capered);
        assert!(z.is_capered());
    }

    #[test]
    fn jest_no_just_capered_when_already_at_max() {
        let mut z = z();
        z.verve = 100.0;
        z.jest(10.0);
        assert!(!z.just_capered);
    }

    #[test]
    fn jest_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.jest(50.0);
        assert_eq!(z.verve, 0.0);
    }

    #[test]
    fn jest_no_op_when_amount_zero() {
        let mut z = z();
        z.jest(0.0);
        assert_eq!(z.verve, 0.0);
    }

    // --- dull ---

    #[test]
    fn dull_reduces_verve() {
        let mut z = z();
        z.verve = 60.0;
        z.dull(20.0);
        assert!((z.verve - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dull_clamps_at_zero() {
        let mut z = z();
        z.verve = 30.0;
        z.dull(200.0);
        assert_eq!(z.verve, 0.0);
    }

    #[test]
    fn dull_fires_just_deadpan_at_zero() {
        let mut z = z();
        z.verve = 30.0;
        z.dull(30.0);
        assert!(z.just_deadpan);
    }

    #[test]
    fn dull_no_op_when_already_deadpan() {
        let mut z = z();
        z.dull(10.0);
        assert!(!z.just_deadpan);
    }

    #[test]
    fn dull_no_op_when_disabled() {
        let mut z = z();
        z.verve = 50.0;
        z.enabled = false;
        z.dull(50.0);
        assert!((z.verve - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_jests_verve() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.verve - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_capered_on_jest_to_max() {
        let mut z = Zanily::new(100.0, 200.0);
        z.verve = 95.0;
        z.tick(1.0);
        assert!(z.just_capered);
        assert!(z.is_capered());
    }

    #[test]
    fn tick_no_jest_when_already_capered() {
        let mut z = z();
        z.verve = 100.0;
        z.tick(1.0);
        assert!(!z.just_capered);
    }

    #[test]
    fn tick_no_jest_when_rate_zero() {
        let mut z = Zanily::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.verve, 0.0);
    }

    #[test]
    fn tick_no_jest_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.verve, 0.0);
    }

    #[test]
    fn tick_clears_just_capered() {
        let mut z = Zanily::new(100.0, 200.0);
        z.verve = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_capered);
    }

    #[test]
    fn tick_clears_just_deadpan() {
        let mut z = z();
        z.verve = 10.0;
        z.dull(10.0);
        z.tick(0.016);
        assert!(!z.just_deadpan);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.verve - 9.0).abs() < 1e-3);
    }

    // --- is_capered / is_deadpan ---

    #[test]
    fn is_capered_false_when_disabled() {
        let mut z = z();
        z.verve = 100.0;
        z.enabled = false;
        assert!(!z.is_capered());
    }

    #[test]
    fn is_deadpan_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_deadpan());
    }

    // --- verve_fraction / effective_hilarity ---

    #[test]
    fn verve_fraction_zero_when_deadpan() {
        assert_eq!(z().verve_fraction(), 0.0);
    }

    #[test]
    fn verve_fraction_half_at_midpoint() {
        let mut z = z();
        z.verve = 50.0;
        assert!((z.verve_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_hilarity_zero_when_deadpan() {
        assert_eq!(z().effective_hilarity(100.0), 0.0);
    }

    #[test]
    fn effective_hilarity_scales_with_verve() {
        let mut z = z();
        z.verve = 75.0;
        assert!((z.effective_hilarity(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_hilarity_zero_when_disabled() {
        let mut z = z();
        z.verve = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_hilarity(100.0), 0.0);
    }
}

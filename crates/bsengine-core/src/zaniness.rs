use bevy_ecs::prelude::Component;

/// Buffoonery-energy accumulation tracker named after zaniness, the
/// noun denoting the quality or state of being zany — that peculiar
/// blend of exuberance, absurdity, and anarchic physical comedy that
/// makes a performer simultaneously fascinating and slightly alarming.
/// The zany as theatrical type is ancient: the Italian commedia
/// dell'arte cast him as the zanni, the servant who mimics his
/// master's every gesture with such determined incompetence that the
/// imitation becomes its own art form, each stumble precisely
/// calibrated for maximum comic collapse. In the commedia tradition
/// zaniness is deeply physical — pratfalls, double-takes, the
/// exaggerated rubber-limbed reaction to a slap, the collapse that
/// defies anatomy by taking three more beats than gravity should
/// permit — but it is also fundamentally cognitive: the zany operates
/// by misunderstanding the rules of every social situation he enters,
/// then doubling down on the misunderstanding with escalating
/// commitment until the original error has been buried under layers of
/// improvised damage control that are worse than the original
/// mistake. Modern usage extends zaniness beyond the theatrical into
/// any domain where energetic, anarchic silliness threatens to derail
/// the serious project at hand — the zany colleague who derails a
/// meeting with an unstoppable pun cascade, the zany game mechanic
/// that introduces randomness through intentional absurdity, the zany
/// narrative that sustains its own internal logic long past the point
/// where external logic has been cheerfully discarded. `mirth` builds
/// via `frolic(amount)` and accumulates passively at `caprice_rate`
/// per second in `tick(dt)` or is stilled via `sober(amount)`.
///
/// Models buffoonery-energy fill levels, pratfall-readiness saturation
/// bars, comedic-potential accumulators, zany-character-energy gauges,
/// absurdity-reservoir fill levels, theatrical-silliness saturation
/// indicators, commedia-antics accumulation bars, physical-comedy
/// readiness meters, anarchic-humour fill levels, or any mechanic
/// where a character, troupe, or scene slowly charges with irrepressible
/// energy until the mirth can no longer be contained and spills out in
/// an uncontrolled cascade of pratfalls, non-sequiturs, and
/// onomatopoeic sound effects that leaves the audience — or the target
/// — struggling to keep a straight face.
///
/// `frolic(amount)` adds mirth; fires `just_clowned` when first
/// reaching `max_mirth`. No-op when disabled.
///
/// `sober(amount)` reduces mirth immediately; fires `just_sobered`
/// when reaching 0. No-op when disabled or already sobered.
///
/// `tick(dt)` clears both flags, then increases mirth by
/// `caprice_rate * dt` (capped at `max_mirth`). Fires `just_clowned`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_clowned()` returns `mirth >= max_mirth && enabled`.
///
/// `is_sobered()` returns `mirth == 0.0` (not gated by `enabled`).
///
/// `mirth_fraction()` returns `(mirth / max_mirth).clamp(0, 1)`.
///
/// `effective_buffoonery(scale)` returns `scale * mirth_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — builds mirth at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zaniness {
    pub mirth: f32,
    pub max_mirth: f32,
    pub caprice_rate: f32,
    pub just_clowned: bool,
    pub just_sobered: bool,
    pub enabled: bool,
}

impl Zaniness {
    pub fn new(max_mirth: f32, caprice_rate: f32) -> Self {
        Self {
            mirth: 0.0,
            max_mirth: max_mirth.max(0.1),
            caprice_rate: caprice_rate.max(0.0),
            just_clowned: false,
            just_sobered: false,
            enabled: true,
        }
    }

    /// Add mirth; fires `just_clowned` when first reaching max.
    /// No-op when disabled.
    pub fn frolic(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.mirth < self.max_mirth;
        self.mirth = (self.mirth + amount).min(self.max_mirth);
        if was_below && self.mirth >= self.max_mirth {
            self.just_clowned = true;
        }
    }

    /// Reduce mirth; fires `just_sobered` when reaching 0.
    /// No-op when disabled or already sobered.
    pub fn sober(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.mirth <= 0.0 {
            return;
        }
        self.mirth = (self.mirth - amount).max(0.0);
        if self.mirth <= 0.0 {
            self.just_sobered = true;
        }
    }

    /// Clear flags, then increase mirth by `caprice_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_clowned = false;
        self.just_sobered = false;
        if self.enabled && self.caprice_rate > 0.0 && self.mirth < self.max_mirth {
            let was_below = self.mirth < self.max_mirth;
            self.mirth = (self.mirth + self.caprice_rate * dt).min(self.max_mirth);
            if was_below && self.mirth >= self.max_mirth {
                self.just_clowned = true;
            }
        }
    }

    /// `true` when mirth is at maximum and component is enabled.
    pub fn is_clowned(&self) -> bool {
        self.mirth >= self.max_mirth && self.enabled
    }

    /// `true` when mirth is 0 (not gated by `enabled`).
    pub fn is_sobered(&self) -> bool {
        self.mirth == 0.0
    }

    /// Fraction of maximum mirth [0.0, 1.0].
    pub fn mirth_fraction(&self) -> f32 {
        (self.mirth / self.max_mirth).clamp(0.0, 1.0)
    }

    /// Returns `scale * mirth_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_buffoonery(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.mirth_fraction()
    }
}

impl Default for Zaniness {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zaniness {
        Zaniness::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_sobered() {
        let z = z();
        assert_eq!(z.mirth, 0.0);
        assert!(z.is_sobered());
        assert!(!z.is_clowned());
    }

    #[test]
    fn new_clamps_max_mirth() {
        let z = Zaniness::new(-5.0, 1.5);
        assert!((z.max_mirth - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_caprice_rate() {
        let z = Zaniness::new(100.0, -1.5);
        assert_eq!(z.caprice_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zaniness::default();
        assert!((z.max_mirth - 100.0).abs() < 1e-5);
        assert!((z.caprice_rate - 1.5).abs() < 1e-5);
    }

    // --- frolic ---

    #[test]
    fn frolic_adds_mirth() {
        let mut z = z();
        z.frolic(40.0);
        assert!((z.mirth - 40.0).abs() < 1e-3);
    }

    #[test]
    fn frolic_clamps_at_max() {
        let mut z = z();
        z.frolic(200.0);
        assert!((z.mirth - 100.0).abs() < 1e-3);
    }

    #[test]
    fn frolic_fires_just_clowned_at_max() {
        let mut z = z();
        z.frolic(100.0);
        assert!(z.just_clowned);
        assert!(z.is_clowned());
    }

    #[test]
    fn frolic_no_just_clowned_when_already_at_max() {
        let mut z = z();
        z.mirth = 100.0;
        z.frolic(10.0);
        assert!(!z.just_clowned);
    }

    #[test]
    fn frolic_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.frolic(50.0);
        assert_eq!(z.mirth, 0.0);
    }

    #[test]
    fn frolic_no_op_when_amount_zero() {
        let mut z = z();
        z.frolic(0.0);
        assert_eq!(z.mirth, 0.0);
    }

    // --- sober ---

    #[test]
    fn sober_reduces_mirth() {
        let mut z = z();
        z.mirth = 60.0;
        z.sober(20.0);
        assert!((z.mirth - 40.0).abs() < 1e-3);
    }

    #[test]
    fn sober_clamps_at_zero() {
        let mut z = z();
        z.mirth = 30.0;
        z.sober(200.0);
        assert_eq!(z.mirth, 0.0);
    }

    #[test]
    fn sober_fires_just_sobered_at_zero() {
        let mut z = z();
        z.mirth = 30.0;
        z.sober(30.0);
        assert!(z.just_sobered);
    }

    #[test]
    fn sober_no_op_when_already_sobered() {
        let mut z = z();
        z.sober(10.0);
        assert!(!z.just_sobered);
    }

    #[test]
    fn sober_no_op_when_disabled() {
        let mut z = z();
        z.mirth = 50.0;
        z.enabled = false;
        z.sober(50.0);
        assert!((z.mirth - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_mirth() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.mirth - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_clowned_on_mirth_to_max() {
        let mut z = Zaniness::new(100.0, 200.0);
        z.mirth = 95.0;
        z.tick(1.0);
        assert!(z.just_clowned);
        assert!(z.is_clowned());
    }

    #[test]
    fn tick_no_build_when_already_clowned() {
        let mut z = z();
        z.mirth = 100.0;
        z.tick(1.0);
        assert!(!z.just_clowned);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut z = Zaniness::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.mirth, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.mirth, 0.0);
    }

    #[test]
    fn tick_clears_just_clowned() {
        let mut z = Zaniness::new(100.0, 200.0);
        z.mirth = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_clowned);
    }

    #[test]
    fn tick_clears_just_sobered() {
        let mut z = z();
        z.mirth = 10.0;
        z.sober(10.0);
        z.tick(0.016);
        assert!(!z.just_sobered);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.mirth - 9.0).abs() < 1e-3);
    }

    // --- is_clowned / is_sobered ---

    #[test]
    fn is_clowned_false_when_disabled() {
        let mut z = z();
        z.mirth = 100.0;
        z.enabled = false;
        assert!(!z.is_clowned());
    }

    #[test]
    fn is_sobered_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_sobered());
    }

    // --- mirth_fraction / effective_buffoonery ---

    #[test]
    fn mirth_fraction_zero_when_sobered() {
        assert_eq!(z().mirth_fraction(), 0.0);
    }

    #[test]
    fn mirth_fraction_half_at_midpoint() {
        let mut z = z();
        z.mirth = 50.0;
        assert!((z.mirth_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_buffoonery_zero_when_sobered() {
        assert_eq!(z().effective_buffoonery(100.0), 0.0);
    }

    #[test]
    fn effective_buffoonery_scales_with_mirth() {
        let mut z = z();
        z.mirth = 75.0;
        assert!((z.effective_buffoonery(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_buffoonery_zero_when_disabled() {
        let mut z = z();
        z.mirth = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_buffoonery(100.0), 0.0);
    }
}

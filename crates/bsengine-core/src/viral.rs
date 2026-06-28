use bevy_ecs::prelude::Component;

/// Contagion-spread accumulation tracker named after viral, the
/// adjective meaning of, relating to, or caused by a virus — or,
/// in its more recent sense, spreading very quickly to many people,
/// usually through digital networks. The word entered English in
/// the nineteenth century when germ theory was beginning to displace
/// miasma theory as the dominant explanation for infectious disease,
/// and viruses themselves were first identified at the end of that
/// century when it became clear that some infectious agents were
/// small enough to pass through filters that stopped bacteria. The
/// term viral in its biological sense describes a property of
/// material — a viral particle, a viral infection, a viral load —
/// but it was repurposed in the early twenty-first century to
/// describe the spread behaviour of content through digital networks
/// in a way that mimics biological contagion: each person who
/// encounters the content can transmit it to multiple others, and
/// if the transmission rate exceeds one recipient per transmitter,
/// the content grows exponentially, spreading across the network
/// in a wave that can reach millions of people in hours. The viral
/// metaphor is surprisingly precise: epidemiologists model content
/// spread using the same R-number mathematics that describe disease
/// spread, and the factors that determine whether a piece of content
/// goes viral — emotional resonance, social currency, novelty,
/// surprise — map onto the factors that determine pathogen fitness
/// with uncomfortable accuracy. In game mechanics, viral propagation
/// models the spread of information, rumour, infection, or influence
/// through a population or network. `contagion` builds via
/// `expose(amount)` and accumulates passively at `spread_rate` per
/// second in `tick(dt)` or is contained via `contain(amount)`.
///
/// Models contagion-spread fill levels, viral-load saturation bars,
/// infection-propagation accumulators, rumour-network gauges, meme-
/// spread fill levels, social-contagion saturation indicators,
/// disease-transmission accumulation bars, influence-cascade meters,
/// pathogen-load fill levels, or any mechanic where an agent,
/// message, pathogen, or idea spreads through a susceptible
/// population — each infected node capable of infecting adjacent
/// nodes — until the network is saturated, the herd immunity
/// threshold is crossed, or a containment strategy breaks the
/// transmission chain before the spread reaches critical mass.
///
/// `expose(amount)` adds contagion; fires `just_spread` when first
/// reaching `max_contagion`. No-op when disabled.
///
/// `contain(amount)` reduces contagion immediately; fires
/// `just_contained` when reaching 0. No-op when disabled or already
/// contained.
///
/// `tick(dt)` clears both flags, then increases contagion by
/// `spread_rate * dt` (capped at `max_contagion`). Fires
/// `just_spread` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_spread()` returns `contagion >= max_contagion && enabled`.
///
/// `is_contained()` returns `contagion == 0.0` (not gated by
/// `enabled`).
///
/// `contagion_fraction()` returns
/// `(contagion / max_contagion).clamp(0, 1)`.
///
/// `effective_viral(scale)` returns `scale * contagion_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — spreads at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Viral {
    pub contagion: f32,
    pub max_contagion: f32,
    pub spread_rate: f32,
    pub just_spread: bool,
    pub just_contained: bool,
    pub enabled: bool,
}

impl Viral {
    pub fn new(max_contagion: f32, spread_rate: f32) -> Self {
        Self {
            contagion: 0.0,
            max_contagion: max_contagion.max(0.1),
            spread_rate: spread_rate.max(0.0),
            just_spread: false,
            just_contained: false,
            enabled: true,
        }
    }

    /// Add contagion; fires `just_spread` when first reaching max.
    /// No-op when disabled.
    pub fn expose(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.contagion < self.max_contagion;
        self.contagion = (self.contagion + amount).min(self.max_contagion);
        if was_below && self.contagion >= self.max_contagion {
            self.just_spread = true;
        }
    }

    /// Reduce contagion; fires `just_contained` when reaching 0.
    /// No-op when disabled or already contained.
    pub fn contain(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.contagion <= 0.0 {
            return;
        }
        self.contagion = (self.contagion - amount).max(0.0);
        if self.contagion <= 0.0 {
            self.just_contained = true;
        }
    }

    /// Clear flags, then increase contagion by `spread_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_spread = false;
        self.just_contained = false;
        if self.enabled && self.spread_rate > 0.0 && self.contagion < self.max_contagion {
            let was_below = self.contagion < self.max_contagion;
            self.contagion = (self.contagion + self.spread_rate * dt).min(self.max_contagion);
            if was_below && self.contagion >= self.max_contagion {
                self.just_spread = true;
            }
        }
    }

    /// `true` when contagion is at maximum and component is enabled.
    pub fn is_spread(&self) -> bool {
        self.contagion >= self.max_contagion && self.enabled
    }

    /// `true` when contagion is 0 (not gated by `enabled`).
    pub fn is_contained(&self) -> bool {
        self.contagion == 0.0
    }

    /// Fraction of maximum contagion [0.0, 1.0].
    pub fn contagion_fraction(&self) -> f32 {
        (self.contagion / self.max_contagion).clamp(0.0, 1.0)
    }

    /// Returns `scale * contagion_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_viral(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.contagion_fraction()
    }
}

impl Default for Viral {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Viral {
        Viral::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_contained() {
        let v = v();
        assert_eq!(v.contagion, 0.0);
        assert!(v.is_contained());
        assert!(!v.is_spread());
    }

    #[test]
    fn new_clamps_max_contagion() {
        let v = Viral::new(-5.0, 1.5);
        assert!((v.max_contagion - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_spread_rate() {
        let v = Viral::new(100.0, -1.5);
        assert_eq!(v.spread_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Viral::default();
        assert!((v.max_contagion - 100.0).abs() < 1e-5);
        assert!((v.spread_rate - 1.5).abs() < 1e-5);
    }

    // --- expose ---

    #[test]
    fn expose_adds_contagion() {
        let mut v = v();
        v.expose(40.0);
        assert!((v.contagion - 40.0).abs() < 1e-3);
    }

    #[test]
    fn expose_clamps_at_max() {
        let mut v = v();
        v.expose(200.0);
        assert!((v.contagion - 100.0).abs() < 1e-3);
    }

    #[test]
    fn expose_fires_just_spread_at_max() {
        let mut v = v();
        v.expose(100.0);
        assert!(v.just_spread);
        assert!(v.is_spread());
    }

    #[test]
    fn expose_no_just_spread_when_already_at_max() {
        let mut v = v();
        v.contagion = 100.0;
        v.expose(10.0);
        assert!(!v.just_spread);
    }

    #[test]
    fn expose_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.expose(50.0);
        assert_eq!(v.contagion, 0.0);
    }

    #[test]
    fn expose_no_op_when_amount_zero() {
        let mut v = v();
        v.expose(0.0);
        assert_eq!(v.contagion, 0.0);
    }

    // --- contain ---

    #[test]
    fn contain_reduces_contagion() {
        let mut v = v();
        v.contagion = 60.0;
        v.contain(20.0);
        assert!((v.contagion - 40.0).abs() < 1e-3);
    }

    #[test]
    fn contain_clamps_at_zero() {
        let mut v = v();
        v.contagion = 30.0;
        v.contain(200.0);
        assert_eq!(v.contagion, 0.0);
    }

    #[test]
    fn contain_fires_just_contained_at_zero() {
        let mut v = v();
        v.contagion = 30.0;
        v.contain(30.0);
        assert!(v.just_contained);
    }

    #[test]
    fn contain_no_op_when_already_contained() {
        let mut v = v();
        v.contain(10.0);
        assert!(!v.just_contained);
    }

    #[test]
    fn contain_no_op_when_disabled() {
        let mut v = v();
        v.contagion = 50.0;
        v.enabled = false;
        v.contain(50.0);
        assert!((v.contagion - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_spreads_contagion() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.contagion - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_spread_on_contagion_to_max() {
        let mut v = Viral::new(100.0, 200.0);
        v.contagion = 95.0;
        v.tick(1.0);
        assert!(v.just_spread);
        assert!(v.is_spread());
    }

    #[test]
    fn tick_no_spread_when_already_spread() {
        let mut v = v();
        v.contagion = 100.0;
        v.tick(1.0);
        assert!(!v.just_spread);
    }

    #[test]
    fn tick_no_spread_when_rate_zero() {
        let mut v = Viral::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.contagion, 0.0);
    }

    #[test]
    fn tick_no_spread_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.contagion, 0.0);
    }

    #[test]
    fn tick_clears_just_spread() {
        let mut v = Viral::new(100.0, 200.0);
        v.contagion = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_spread);
    }

    #[test]
    fn tick_clears_just_contained() {
        let mut v = v();
        v.contagion = 10.0;
        v.contain(10.0);
        v.tick(0.016);
        assert!(!v.just_contained);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.contagion - 9.0).abs() < 1e-3);
    }

    // --- is_spread / is_contained ---

    #[test]
    fn is_spread_false_when_disabled() {
        let mut v = v();
        v.contagion = 100.0;
        v.enabled = false;
        assert!(!v.is_spread());
    }

    #[test]
    fn is_contained_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_contained());
    }

    // --- contagion_fraction / effective_viral ---

    #[test]
    fn contagion_fraction_zero_when_contained() {
        assert_eq!(v().contagion_fraction(), 0.0);
    }

    #[test]
    fn contagion_fraction_half_at_midpoint() {
        let mut v = v();
        v.contagion = 50.0;
        assert!((v.contagion_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_viral_zero_when_contained() {
        assert_eq!(v().effective_viral(100.0), 0.0);
    }

    #[test]
    fn effective_viral_scales_with_contagion() {
        let mut v = v();
        v.contagion = 75.0;
        assert!((v.effective_viral(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_viral_zero_when_disabled() {
        let mut v = v();
        v.contagion = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_viral(100.0), 0.0);
    }
}

use bevy_ecs::prelude::Component;

/// Zoonotic-spillover potential tracker named after "zoonotic", the
/// adjective form of zoonosis — describing any pathogen capable of
/// making the cross-species leap from an animal reservoir into a human
/// (or other susceptible) host. The word is Merriam-Webster standard
/// and it describes the defining epidemiological quality of some of the
/// most consequential pathogens in recorded history: Yersinia pestis
/// moving from rats via fleas, influenza reshuffling through birds and
/// swine before emerging in people, rabies passing through the bite of
/// an infected dog, Ebola virus likely crossing from fruit bats, SARS-
/// CoV-2 traced to bat coronaviruses through an intermediate host.
/// Zoonotic risk is not a binary — it is a continuum governed by viral
/// load in the reservoir, the proximity and frequency of human contact,
/// the density of potential intermediate hosts, the genetic distance
/// between the reservoir and the new host's cell receptors, and the
/// number of discrete spill-over events that have already occurred
/// without onward transmission. `spillover` builds via `shed(amount)`
/// and accumulates passively at `transmit_rate` per second in
/// `tick(dt)` or is cleared via `clear(amount)`.
///
/// Models pathogen reservoir fill levels, species-barrier-crossing
/// saturation bars, zoonotic-emergence risk accumulation trackers,
/// viral-spillover potential gauges, wildlife-interface proximity
/// meters, bat-cave exposure fill levels, bushmeat-contact saturation
/// trackers, intermediate-host density accumulation bars, pandemic-
/// seed-event potential indicators, or any mechanic where patient
/// ecological proximity gradually charges a hidden reservoir until a
/// random threshold is crossed and the pathogen finds a foothold in
/// an entirely new host species — rewriting the rules of the outbreak
/// the moment it does.
///
/// `shed(amount)` adds spillover; fires `just_emerged` when first
/// reaching `max_spillover`. No-op when disabled.
///
/// `clear(amount)` reduces spillover immediately; fires `just_contained`
/// when reaching 0. No-op when disabled or already contained.
///
/// `tick(dt)` clears both flags, then increases spillover by
/// `transmit_rate * dt` (capped at `max_spillover`). Fires
/// `just_emerged` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_emerged()` returns `spillover >= max_spillover && enabled`.
///
/// `is_contained()` returns `spillover == 0.0` (not gated by `enabled`).
///
/// `spillover_fraction()` returns `(spillover / max_spillover).clamp(0, 1)`.
///
/// `effective_pathogenicity(scale)` returns `scale * spillover_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — transmits at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoonotic {
    pub spillover: f32,
    pub max_spillover: f32,
    pub transmit_rate: f32,
    pub just_emerged: bool,
    pub just_contained: bool,
    pub enabled: bool,
}

impl Zoonotic {
    pub fn new(max_spillover: f32, transmit_rate: f32) -> Self {
        Self {
            spillover: 0.0,
            max_spillover: max_spillover.max(0.1),
            transmit_rate: transmit_rate.max(0.0),
            just_emerged: false,
            just_contained: false,
            enabled: true,
        }
    }

    /// Add spillover; fires `just_emerged` when first reaching max.
    /// No-op when disabled.
    pub fn shed(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.spillover < self.max_spillover;
        self.spillover = (self.spillover + amount).min(self.max_spillover);
        if was_below && self.spillover >= self.max_spillover {
            self.just_emerged = true;
        }
    }

    /// Reduce spillover; fires `just_contained` when reaching 0.
    /// No-op when disabled or already contained.
    pub fn clear(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.spillover <= 0.0 {
            return;
        }
        self.spillover = (self.spillover - amount).max(0.0);
        if self.spillover <= 0.0 {
            self.just_contained = true;
        }
    }

    /// Clear flags, then increase spillover by `transmit_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_emerged = false;
        self.just_contained = false;
        if self.enabled && self.transmit_rate > 0.0 && self.spillover < self.max_spillover {
            let was_below = self.spillover < self.max_spillover;
            self.spillover = (self.spillover + self.transmit_rate * dt).min(self.max_spillover);
            if was_below && self.spillover >= self.max_spillover {
                self.just_emerged = true;
            }
        }
    }

    /// `true` when spillover is at maximum and component is enabled.
    pub fn is_emerged(&self) -> bool {
        self.spillover >= self.max_spillover && self.enabled
    }

    /// `true` when spillover is 0 (not gated by `enabled`).
    pub fn is_contained(&self) -> bool {
        self.spillover == 0.0
    }

    /// Fraction of maximum spillover [0.0, 1.0].
    pub fn spillover_fraction(&self) -> f32 {
        (self.spillover / self.max_spillover).clamp(0.0, 1.0)
    }

    /// Returns `scale * spillover_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_pathogenicity(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.spillover_fraction()
    }
}

impl Default for Zoonotic {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoonotic {
        Zoonotic::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_contained() {
        let z = z();
        assert_eq!(z.spillover, 0.0);
        assert!(z.is_contained());
        assert!(!z.is_emerged());
    }

    #[test]
    fn new_clamps_max_spillover() {
        let z = Zoonotic::new(-5.0, 1.5);
        assert!((z.max_spillover - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_transmit_rate() {
        let z = Zoonotic::new(100.0, -1.5);
        assert_eq!(z.transmit_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoonotic::default();
        assert!((z.max_spillover - 100.0).abs() < 1e-5);
        assert!((z.transmit_rate - 1.5).abs() < 1e-5);
    }

    // --- shed ---

    #[test]
    fn shed_adds_spillover() {
        let mut z = z();
        z.shed(40.0);
        assert!((z.spillover - 40.0).abs() < 1e-3);
    }

    #[test]
    fn shed_clamps_at_max() {
        let mut z = z();
        z.shed(200.0);
        assert!((z.spillover - 100.0).abs() < 1e-3);
    }

    #[test]
    fn shed_fires_just_emerged_at_max() {
        let mut z = z();
        z.shed(100.0);
        assert!(z.just_emerged);
        assert!(z.is_emerged());
    }

    #[test]
    fn shed_no_just_emerged_when_already_at_max() {
        let mut z = z();
        z.spillover = 100.0;
        z.shed(10.0);
        assert!(!z.just_emerged);
    }

    #[test]
    fn shed_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.shed(50.0);
        assert_eq!(z.spillover, 0.0);
    }

    #[test]
    fn shed_no_op_when_amount_zero() {
        let mut z = z();
        z.shed(0.0);
        assert_eq!(z.spillover, 0.0);
    }

    // --- clear ---

    #[test]
    fn clear_reduces_spillover() {
        let mut z = z();
        z.spillover = 60.0;
        z.clear(20.0);
        assert!((z.spillover - 40.0).abs() < 1e-3);
    }

    #[test]
    fn clear_clamps_at_zero() {
        let mut z = z();
        z.spillover = 30.0;
        z.clear(200.0);
        assert_eq!(z.spillover, 0.0);
    }

    #[test]
    fn clear_fires_just_contained_at_zero() {
        let mut z = z();
        z.spillover = 30.0;
        z.clear(30.0);
        assert!(z.just_contained);
    }

    #[test]
    fn clear_no_op_when_already_contained() {
        let mut z = z();
        z.clear(10.0);
        assert!(!z.just_contained);
    }

    #[test]
    fn clear_no_op_when_disabled() {
        let mut z = z();
        z.spillover = 50.0;
        z.enabled = false;
        z.clear(50.0);
        assert!((z.spillover - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_transmits_spillover() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.spillover - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_emerged_on_transmit_to_max() {
        let mut z = Zoonotic::new(100.0, 200.0);
        z.spillover = 95.0;
        z.tick(1.0);
        assert!(z.just_emerged);
        assert!(z.is_emerged());
    }

    #[test]
    fn tick_no_transmit_when_already_emerged() {
        let mut z = z();
        z.spillover = 100.0;
        z.tick(1.0);
        assert!(!z.just_emerged);
    }

    #[test]
    fn tick_no_transmit_when_rate_zero() {
        let mut z = Zoonotic::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.spillover, 0.0);
    }

    #[test]
    fn tick_no_transmit_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.spillover, 0.0);
    }

    #[test]
    fn tick_clears_just_emerged() {
        let mut z = Zoonotic::new(100.0, 200.0);
        z.spillover = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_emerged);
    }

    #[test]
    fn tick_clears_just_contained() {
        let mut z = z();
        z.spillover = 10.0;
        z.clear(10.0);
        z.tick(0.016);
        assert!(!z.just_contained);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.spillover - 9.0).abs() < 1e-3);
    }

    // --- is_emerged / is_contained ---

    #[test]
    fn is_emerged_false_when_disabled() {
        let mut z = z();
        z.spillover = 100.0;
        z.enabled = false;
        assert!(!z.is_emerged());
    }

    #[test]
    fn is_contained_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_contained());
    }

    // --- spillover_fraction / effective_pathogenicity ---

    #[test]
    fn spillover_fraction_zero_when_contained() {
        assert_eq!(z().spillover_fraction(), 0.0);
    }

    #[test]
    fn spillover_fraction_half_at_midpoint() {
        let mut z = z();
        z.spillover = 50.0;
        assert!((z.spillover_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_pathogenicity_zero_when_contained() {
        assert_eq!(z().effective_pathogenicity(100.0), 0.0);
    }

    #[test]
    fn effective_pathogenicity_scales_with_spillover() {
        let mut z = z();
        z.spillover = 75.0;
        assert!((z.effective_pathogenicity(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_pathogenicity_zero_when_disabled() {
        let mut z = z();
        z.spillover = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_pathogenicity(100.0), 0.0);
    }
}

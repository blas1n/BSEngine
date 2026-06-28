use bevy_ecs::prelude::Component;

/// Chromosomal-synapsis accumulation tracker named after the
/// zygotene, the second stage of meiotic prophase I during which
/// homologous chromosomes begin to pair along their entire length in
/// a process called synapsis. The name comes from Greek zygon (yoke
/// or pair) plus the suffix -tene (ribbon), together conveying the
/// image of two chromosomal ribbons being yoked together by the
/// synaptonemal complex, a protein scaffold that forms between
/// paired homologues like a molecular zip-fastener. The zygotene
/// stage follows the leptotene (in which chromosomes first condense
/// into visible threads) and precedes the pachytene (in which
/// synapsis is complete and the chromosomes reach their maximum
/// thickness). The synapsis that occurs in zygotene is not merely
/// mechanical alignment but is the physical prerequisite for
/// crossing-over: without intimate pairing of homologues, the
/// reciprocal exchange of genetic material that generates diversity
/// cannot occur. The fidelity of zygotene pairing therefore
/// determines the fidelity of genetic recombination; failures produce
/// asynaptic chromosomes that cannot segregate properly, leading to
/// aneuploidy, infertility, or both. `synapsis` builds via
/// `synapse(amount)` and accumulates passively at `pair_rate` per
/// second in `tick(dt)` or retreats via `dissociate(amount)`.
///
/// Models chromosomal-synapsis fill levels, homologue-pairing
/// saturation bars, recombination-readiness accumulation trackers,
/// meiotic-fidelity gauges, synaptonemal-complex formation fill
/// levels, genetic-diversity-potential saturation indicators,
/// crossing-over-prerequisite accumulation bars, asynapsis-risk
/// meters, fertility-checkpoint fill levels, or any mechanic where
/// two distinct entities must be drawn incrementally into perfect
/// register with one another before any exchange — of genetic
/// material, of information, of secrets, of contracts — can take
/// place, and where premature separation or chronic failure to pair
/// dooms the enterprise before it can begin.
///
/// `synapse(amount)` adds synapsis; fires `just_paired` when first
/// reaching `max_synapsis`. No-op when disabled.
///
/// `dissociate(amount)` reduces synapsis immediately; fires
/// `just_unpaired` when reaching 0. No-op when disabled or already
/// unpaired.
///
/// `tick(dt)` clears both flags, then increases synapsis by
/// `pair_rate * dt` (capped at `max_synapsis`). Fires `just_paired`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_paired()` returns `synapsis >= max_synapsis && enabled`.
///
/// `is_unpaired()` returns `synapsis == 0.0` (not gated by
/// `enabled`).
///
/// `synapsis_fraction()` returns
/// `(synapsis / max_synapsis).clamp(0, 1)`.
///
/// `effective_recombination(scale)` returns
/// `scale * synapsis_fraction()` when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — pairs at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zygotene {
    pub synapsis: f32,
    pub max_synapsis: f32,
    pub pair_rate: f32,
    pub just_paired: bool,
    pub just_unpaired: bool,
    pub enabled: bool,
}

impl Zygotene {
    pub fn new(max_synapsis: f32, pair_rate: f32) -> Self {
        Self {
            synapsis: 0.0,
            max_synapsis: max_synapsis.max(0.1),
            pair_rate: pair_rate.max(0.0),
            just_paired: false,
            just_unpaired: false,
            enabled: true,
        }
    }

    /// Add synapsis; fires `just_paired` when first reaching max.
    /// No-op when disabled.
    pub fn synapse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.synapsis < self.max_synapsis;
        self.synapsis = (self.synapsis + amount).min(self.max_synapsis);
        if was_below && self.synapsis >= self.max_synapsis {
            self.just_paired = true;
        }
    }

    /// Reduce synapsis; fires `just_unpaired` when reaching 0.
    /// No-op when disabled or already unpaired.
    pub fn dissociate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.synapsis <= 0.0 {
            return;
        }
        self.synapsis = (self.synapsis - amount).max(0.0);
        if self.synapsis <= 0.0 {
            self.just_unpaired = true;
        }
    }

    /// Clear flags, then increase synapsis by `pair_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_paired = false;
        self.just_unpaired = false;
        if self.enabled && self.pair_rate > 0.0 && self.synapsis < self.max_synapsis {
            let was_below = self.synapsis < self.max_synapsis;
            self.synapsis = (self.synapsis + self.pair_rate * dt).min(self.max_synapsis);
            if was_below && self.synapsis >= self.max_synapsis {
                self.just_paired = true;
            }
        }
    }

    /// `true` when synapsis is at maximum and component is enabled.
    pub fn is_paired(&self) -> bool {
        self.synapsis >= self.max_synapsis && self.enabled
    }

    /// `true` when synapsis is 0 (not gated by `enabled`).
    pub fn is_unpaired(&self) -> bool {
        self.synapsis == 0.0
    }

    /// Fraction of maximum synapsis [0.0, 1.0].
    pub fn synapsis_fraction(&self) -> f32 {
        (self.synapsis / self.max_synapsis).clamp(0.0, 1.0)
    }

    /// Returns `scale * synapsis_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_recombination(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.synapsis_fraction()
    }
}

impl Default for Zygotene {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zygotene {
        Zygotene::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_unpaired() {
        let z = z();
        assert_eq!(z.synapsis, 0.0);
        assert!(z.is_unpaired());
        assert!(!z.is_paired());
    }

    #[test]
    fn new_clamps_max_synapsis() {
        let z = Zygotene::new(-5.0, 1.5);
        assert!((z.max_synapsis - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_pair_rate() {
        let z = Zygotene::new(100.0, -1.5);
        assert_eq!(z.pair_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zygotene::default();
        assert!((z.max_synapsis - 100.0).abs() < 1e-5);
        assert!((z.pair_rate - 1.5).abs() < 1e-5);
    }

    // --- synapse ---

    #[test]
    fn synapse_adds_synapsis() {
        let mut z = z();
        z.synapse(40.0);
        assert!((z.synapsis - 40.0).abs() < 1e-3);
    }

    #[test]
    fn synapse_clamps_at_max() {
        let mut z = z();
        z.synapse(200.0);
        assert!((z.synapsis - 100.0).abs() < 1e-3);
    }

    #[test]
    fn synapse_fires_just_paired_at_max() {
        let mut z = z();
        z.synapse(100.0);
        assert!(z.just_paired);
        assert!(z.is_paired());
    }

    #[test]
    fn synapse_no_just_paired_when_already_at_max() {
        let mut z = z();
        z.synapsis = 100.0;
        z.synapse(10.0);
        assert!(!z.just_paired);
    }

    #[test]
    fn synapse_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.synapse(50.0);
        assert_eq!(z.synapsis, 0.0);
    }

    #[test]
    fn synapse_no_op_when_amount_zero() {
        let mut z = z();
        z.synapse(0.0);
        assert_eq!(z.synapsis, 0.0);
    }

    // --- dissociate ---

    #[test]
    fn dissociate_reduces_synapsis() {
        let mut z = z();
        z.synapsis = 60.0;
        z.dissociate(20.0);
        assert!((z.synapsis - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dissociate_clamps_at_zero() {
        let mut z = z();
        z.synapsis = 30.0;
        z.dissociate(200.0);
        assert_eq!(z.synapsis, 0.0);
    }

    #[test]
    fn dissociate_fires_just_unpaired_at_zero() {
        let mut z = z();
        z.synapsis = 30.0;
        z.dissociate(30.0);
        assert!(z.just_unpaired);
    }

    #[test]
    fn dissociate_no_op_when_already_unpaired() {
        let mut z = z();
        z.dissociate(10.0);
        assert!(!z.just_unpaired);
    }

    #[test]
    fn dissociate_no_op_when_disabled() {
        let mut z = z();
        z.synapsis = 50.0;
        z.enabled = false;
        z.dissociate(50.0);
        assert!((z.synapsis - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_pairs_synapsis() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.synapsis - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_paired_on_pair_to_max() {
        let mut z = Zygotene::new(100.0, 200.0);
        z.synapsis = 95.0;
        z.tick(1.0);
        assert!(z.just_paired);
        assert!(z.is_paired());
    }

    #[test]
    fn tick_no_pair_when_already_paired() {
        let mut z = z();
        z.synapsis = 100.0;
        z.tick(1.0);
        assert!(!z.just_paired);
    }

    #[test]
    fn tick_no_pair_when_rate_zero() {
        let mut z = Zygotene::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.synapsis, 0.0);
    }

    #[test]
    fn tick_no_pair_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.synapsis, 0.0);
    }

    #[test]
    fn tick_clears_just_paired() {
        let mut z = Zygotene::new(100.0, 200.0);
        z.synapsis = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_paired);
    }

    #[test]
    fn tick_clears_just_unpaired() {
        let mut z = z();
        z.synapsis = 10.0;
        z.dissociate(10.0);
        z.tick(0.016);
        assert!(!z.just_unpaired);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.synapsis - 9.0).abs() < 1e-3);
    }

    // --- is_paired / is_unpaired ---

    #[test]
    fn is_paired_false_when_disabled() {
        let mut z = z();
        z.synapsis = 100.0;
        z.enabled = false;
        assert!(!z.is_paired());
    }

    #[test]
    fn is_unpaired_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_unpaired());
    }

    // --- synapsis_fraction / effective_recombination ---

    #[test]
    fn synapsis_fraction_zero_when_unpaired() {
        assert_eq!(z().synapsis_fraction(), 0.0);
    }

    #[test]
    fn synapsis_fraction_half_at_midpoint() {
        let mut z = z();
        z.synapsis = 50.0;
        assert!((z.synapsis_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_recombination_zero_when_unpaired() {
        assert_eq!(z().effective_recombination(100.0), 0.0);
    }

    #[test]
    fn effective_recombination_scales_with_synapsis() {
        let mut z = z();
        z.synapsis = 75.0;
        assert!((z.effective_recombination(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_recombination_zero_when_disabled() {
        let mut z = z();
        z.synapsis = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_recombination(100.0), 0.0);
    }
}

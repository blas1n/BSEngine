use bevy_ecs::prelude::Component;

/// Colonial-organism integration tracker named after zooid, the
/// individual unit of a colonial animal — each calcite-chambered
/// polyp in a bryozoan colony, each tentacle-bearing hydranth in
/// a hydroid, each transparent swimming bell in a siphonophore,
/// each barrel-shaped jet-propelled individual in a salp chain.
/// Zooids share nutrients, hormones, and coordinated behaviour
/// through stolons or coenosarc tissue while retaining their own
/// feeding, defence, and reproductive specialisations; the colony
/// is simultaneously one organism and many. `integration` builds
/// via `connect(amount)` and accumulates passively at `bud_rate`
/// per second in `tick(dt)` or is reduced via `detach(amount)`.
///
/// Models colonial-organism cohesion bars, hive-mind integration
/// fill levels, network-synergy saturation trackers, distributed-
/// consciousness unity gauges, swarm-intelligence coherence meters,
/// bryozoan-colony connectivity bars, siphonophore-individual
/// coordination fill levels, cellular-slime-mould aggregation
/// trackers, mycelial-network integration indicators, or any
/// mechanic where many individual agents slowly bind themselves
/// into a single coordinated superorganism whose collective
/// capability far exceeds the sum of its parts — right up until
/// a disruption severs the connecting tissue and each unit reverts
/// to solitary, helpless individuality.
///
/// `connect(amount)` adds integration; fires `just_integrated`
/// when first reaching `max_integration`. No-op when disabled.
///
/// `detach(amount)` reduces integration immediately; fires
/// `just_isolated` when reaching 0. No-op when disabled or already
/// isolated.
///
/// `tick(dt)` clears both flags, then increases integration by
/// `bud_rate * dt` (capped at `max_integration`). Fires
/// `just_integrated` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_integrated()` returns `integration >= max_integration && enabled`.
///
/// `is_isolated()` returns `integration == 0.0` (not gated by `enabled`).
///
/// `integration_fraction()` returns `(integration / max_integration).clamp(0, 1)`.
///
/// `effective_cohesion(scale)` returns `scale * integration_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — buds at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zooid {
    pub integration: f32,
    pub max_integration: f32,
    pub bud_rate: f32,
    pub just_integrated: bool,
    pub just_isolated: bool,
    pub enabled: bool,
}

impl Zooid {
    pub fn new(max_integration: f32, bud_rate: f32) -> Self {
        Self {
            integration: 0.0,
            max_integration: max_integration.max(0.1),
            bud_rate: bud_rate.max(0.0),
            just_integrated: false,
            just_isolated: false,
            enabled: true,
        }
    }

    /// Add integration; fires `just_integrated` when first reaching max.
    /// No-op when disabled.
    pub fn connect(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.integration < self.max_integration;
        self.integration = (self.integration + amount).min(self.max_integration);
        if was_below && self.integration >= self.max_integration {
            self.just_integrated = true;
        }
    }

    /// Reduce integration; fires `just_isolated` when reaching 0.
    /// No-op when disabled or already isolated.
    pub fn detach(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.integration <= 0.0 {
            return;
        }
        self.integration = (self.integration - amount).max(0.0);
        if self.integration <= 0.0 {
            self.just_isolated = true;
        }
    }

    /// Clear flags, then increase integration by `bud_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_integrated = false;
        self.just_isolated = false;
        if self.enabled && self.bud_rate > 0.0 && self.integration < self.max_integration {
            let was_below = self.integration < self.max_integration;
            self.integration = (self.integration + self.bud_rate * dt).min(self.max_integration);
            if was_below && self.integration >= self.max_integration {
                self.just_integrated = true;
            }
        }
    }

    /// `true` when integration is at maximum and component is enabled.
    pub fn is_integrated(&self) -> bool {
        self.integration >= self.max_integration && self.enabled
    }

    /// `true` when integration is 0 (not gated by `enabled`).
    pub fn is_isolated(&self) -> bool {
        self.integration == 0.0
    }

    /// Fraction of maximum integration [0.0, 1.0].
    pub fn integration_fraction(&self) -> f32 {
        (self.integration / self.max_integration).clamp(0.0, 1.0)
    }

    /// Returns `scale * integration_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_cohesion(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.integration_fraction()
    }
}

impl Default for Zooid {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zooid {
        Zooid::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_isolated() {
        let z = z();
        assert_eq!(z.integration, 0.0);
        assert!(z.is_isolated());
        assert!(!z.is_integrated());
    }

    #[test]
    fn new_clamps_max_integration() {
        let z = Zooid::new(-5.0, 1.5);
        assert!((z.max_integration - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_bud_rate() {
        let z = Zooid::new(100.0, -1.5);
        assert_eq!(z.bud_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zooid::default();
        assert!((z.max_integration - 100.0).abs() < 1e-5);
        assert!((z.bud_rate - 1.5).abs() < 1e-5);
    }

    // --- connect ---

    #[test]
    fn connect_adds_integration() {
        let mut z = z();
        z.connect(40.0);
        assert!((z.integration - 40.0).abs() < 1e-3);
    }

    #[test]
    fn connect_clamps_at_max() {
        let mut z = z();
        z.connect(200.0);
        assert!((z.integration - 100.0).abs() < 1e-3);
    }

    #[test]
    fn connect_fires_just_integrated_at_max() {
        let mut z = z();
        z.connect(100.0);
        assert!(z.just_integrated);
        assert!(z.is_integrated());
    }

    #[test]
    fn connect_no_just_integrated_when_already_at_max() {
        let mut z = z();
        z.integration = 100.0;
        z.connect(10.0);
        assert!(!z.just_integrated);
    }

    #[test]
    fn connect_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.connect(50.0);
        assert_eq!(z.integration, 0.0);
    }

    #[test]
    fn connect_no_op_when_amount_zero() {
        let mut z = z();
        z.connect(0.0);
        assert_eq!(z.integration, 0.0);
    }

    // --- detach ---

    #[test]
    fn detach_reduces_integration() {
        let mut z = z();
        z.integration = 60.0;
        z.detach(20.0);
        assert!((z.integration - 40.0).abs() < 1e-3);
    }

    #[test]
    fn detach_clamps_at_zero() {
        let mut z = z();
        z.integration = 30.0;
        z.detach(200.0);
        assert_eq!(z.integration, 0.0);
    }

    #[test]
    fn detach_fires_just_isolated_at_zero() {
        let mut z = z();
        z.integration = 30.0;
        z.detach(30.0);
        assert!(z.just_isolated);
    }

    #[test]
    fn detach_no_op_when_already_isolated() {
        let mut z = z();
        z.detach(10.0);
        assert!(!z.just_isolated);
    }

    #[test]
    fn detach_no_op_when_disabled() {
        let mut z = z();
        z.integration = 50.0;
        z.enabled = false;
        z.detach(50.0);
        assert!((z.integration - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_buds_integration() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.integration - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_integrated_on_bud_to_max() {
        let mut z = Zooid::new(100.0, 200.0);
        z.integration = 95.0;
        z.tick(1.0);
        assert!(z.just_integrated);
        assert!(z.is_integrated());
    }

    #[test]
    fn tick_no_bud_when_already_integrated() {
        let mut z = z();
        z.integration = 100.0;
        z.tick(1.0);
        assert!(!z.just_integrated);
    }

    #[test]
    fn tick_no_bud_when_rate_zero() {
        let mut z = Zooid::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.integration, 0.0);
    }

    #[test]
    fn tick_no_bud_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.integration, 0.0);
    }

    #[test]
    fn tick_clears_just_integrated() {
        let mut z = Zooid::new(100.0, 200.0);
        z.integration = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_integrated);
    }

    #[test]
    fn tick_clears_just_isolated() {
        let mut z = z();
        z.integration = 10.0;
        z.detach(10.0);
        z.tick(0.016);
        assert!(!z.just_isolated);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.integration - 9.0).abs() < 1e-3);
    }

    // --- is_integrated / is_isolated ---

    #[test]
    fn is_integrated_false_when_disabled() {
        let mut z = z();
        z.integration = 100.0;
        z.enabled = false;
        assert!(!z.is_integrated());
    }

    #[test]
    fn is_isolated_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_isolated());
    }

    // --- integration_fraction / effective_cohesion ---

    #[test]
    fn integration_fraction_zero_when_isolated() {
        assert_eq!(z().integration_fraction(), 0.0);
    }

    #[test]
    fn integration_fraction_half_at_midpoint() {
        let mut z = z();
        z.integration = 50.0;
        assert!((z.integration_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_cohesion_zero_when_isolated() {
        assert_eq!(z().effective_cohesion(100.0), 0.0);
    }

    #[test]
    fn effective_cohesion_scales_with_integration() {
        let mut z = z();
        z.integration = 75.0;
        assert!((z.effective_cohesion(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_cohesion_zero_when_disabled() {
        let mut z = z();
        z.integration = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_cohesion(100.0), 0.0);
    }
}

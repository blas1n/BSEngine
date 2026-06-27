use bevy_ecs::prelude::Component;

/// Toggleable burst-sprint with draining charge. Distinct from `Zap`
/// (one-shot cooldown-gated discharge) and `Dash` (phased duration): Zip
/// models a **hold-to-sprint** resource — manually activated, drains while
/// active, recharges while idle, auto-deactivates when exhausted.
///
/// `activate()` starts zipping if enabled, not already active, and
/// `zip_charge > 0`. Fires `just_activated`. No-op if already active,
/// disabled, or charge is empty.
///
/// `deactivate()` stops zipping immediately regardless of charge. No flags.
///
/// `tick(dt)` clears one-frame flags first. If enabled and active: drains
/// `drain_rate * dt` from `zip_charge`, floors at 0; if reaching 0
/// auto-deactivates and fires `just_exhausted`. If enabled and idle: adds
/// `recharge_rate * dt` up to `max_charge`. No-op (beyond flag clear) when
/// disabled.
///
/// `is_zipping()` returns `is_active && zip_charge > 0.0 && enabled`.
///
/// `charge_fraction()` returns `(zip_charge / max_charge).clamp(0.0, 1.0)`.
///
/// `effective_boost(base)` returns `base * (1.0 + charge_fraction())` when
/// zipping — 1× at empty (never zips), up to 2× at full charge; `base`
/// when idle or disabled.
///
/// Default: `new(10.0, 2.0, 1.0)` — max 10, drain 2/s, recharge 1/s.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zip {
    /// Current burst charge [0, max_charge].
    pub zip_charge: f32,
    /// Maximum charge. Clamped >= 1.0.
    pub max_charge: f32,
    /// Charge drained per second while active. Clamped >= 0.0.
    pub drain_rate: f32,
    /// Charge regained per second while idle. Clamped >= 0.0.
    pub recharge_rate: f32,
    /// Whether the burst is currently active.
    pub is_active: bool,
    pub just_activated: bool,
    pub just_exhausted: bool,
    pub enabled: bool,
}

impl Zip {
    pub fn new(max_charge: f32, drain_rate: f32, recharge_rate: f32) -> Self {
        let max_charge = max_charge.max(1.0);
        Self {
            zip_charge: max_charge,
            max_charge,
            drain_rate: drain_rate.max(0.0),
            recharge_rate: recharge_rate.max(0.0),
            is_active: false,
            just_activated: false,
            just_exhausted: false,
            enabled: true,
        }
    }

    /// Start zipping. Fires `just_activated`. No-op if already active,
    /// disabled, or `zip_charge == 0`.
    pub fn activate(&mut self) {
        if !self.enabled || self.is_active || self.zip_charge <= 0.0 {
            return;
        }
        self.is_active = true;
        self.just_activated = true;
    }

    /// Stop zipping immediately. No flags fired.
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// Advance one frame: clear flags, then drain (active) or recharge
    /// (idle). Auto-deactivates and fires `just_exhausted` when charge
    /// reaches 0 while active. No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_activated = false;
        self.just_exhausted = false;

        if !self.enabled {
            return;
        }

        if self.is_active {
            self.zip_charge = (self.zip_charge - self.drain_rate * dt).max(0.0);
            if self.zip_charge == 0.0 {
                self.is_active = false;
                self.just_exhausted = true;
            }
        } else {
            self.zip_charge = (self.zip_charge + self.recharge_rate * dt).min(self.max_charge);
        }
    }

    /// `true` when actively zipping with charge remaining and enabled.
    pub fn is_zipping(&self) -> bool {
        self.is_active && self.zip_charge > 0.0 && self.enabled
    }

    /// Charge as a fraction of maximum [0.0, 1.0].
    pub fn charge_fraction(&self) -> f32 {
        (self.zip_charge / self.max_charge).clamp(0.0, 1.0)
    }

    /// Scale `base` by charge while zipping. Returns `base * (1.0 +
    /// charge_fraction())` when zipping; `base` when idle or disabled.
    pub fn effective_boost(&self, base: f32) -> f32 {
        if !self.enabled || !self.is_active {
            return base;
        }
        base * (1.0 + self.charge_fraction())
    }
}

impl Default for Zip {
    fn default() -> Self {
        Self::new(10.0, 2.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zip {
        Zip::new(10.0, 2.0, 1.0) // max=10, drain=2/s, recharge=1/s
    }

    // --- construction ---

    #[test]
    fn new_starts_full_and_inactive() {
        let z = z();
        assert!((z.zip_charge - 10.0).abs() < 1e-5);
        assert!(!z.is_active);
        assert!(!z.just_activated);
        assert!(!z.just_exhausted);
        assert!(!z.is_zipping());
    }

    #[test]
    fn max_charge_clamped_to_one() {
        let z = Zip::new(0.0, 1.0, 1.0);
        assert!((z.max_charge - 1.0).abs() < 1e-5);
        assert!((z.zip_charge - 1.0).abs() < 1e-5);
    }

    #[test]
    fn drain_rate_clamped_to_zero() {
        let z = Zip::new(10.0, -1.0, 1.0);
        assert_eq!(z.drain_rate, 0.0);
    }

    #[test]
    fn recharge_rate_clamped_to_zero() {
        let z = Zip::new(10.0, 1.0, -1.0);
        assert_eq!(z.recharge_rate, 0.0);
    }

    // --- activate ---

    #[test]
    fn activate_starts_zipping() {
        let mut z = z();
        z.activate();
        assert!(z.is_active);
        assert!(z.just_activated);
        assert!(z.is_zipping());
    }

    #[test]
    fn activate_no_op_when_already_active() {
        let mut z = z();
        z.activate();
        z.just_activated = false; // manually clear
        z.activate(); // already active
        assert!(!z.just_activated);
    }

    #[test]
    fn activate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.activate();
        assert!(!z.is_active);
        assert!(!z.just_activated);
    }

    #[test]
    fn activate_no_op_when_charge_empty() {
        let mut z = z();
        z.zip_charge = 0.0;
        z.activate();
        assert!(!z.is_active);
    }

    // --- deactivate ---

    #[test]
    fn deactivate_stops_zipping() {
        let mut z = z();
        z.activate();
        z.deactivate();
        assert!(!z.is_active);
        assert!(!z.is_zipping());
    }

    #[test]
    fn deactivate_no_op_when_already_inactive() {
        let mut z = z();
        z.deactivate(); // already inactive
        assert!(!z.is_active);
    }

    // --- tick: draining ---

    #[test]
    fn tick_drains_while_active() {
        let mut z = z(); // drain=2/s
        z.activate();
        z.tick(1.0); // 10 - 2 = 8
        assert!((z.zip_charge - 8.0).abs() < 1e-5);
    }

    #[test]
    fn tick_auto_deactivates_when_exhausted() {
        let mut z = Zip::new(2.0, 2.0, 1.0); // drains in 1s
        z.activate();
        z.tick(1.0);
        assert_eq!(z.zip_charge, 0.0);
        assert!(!z.is_active);
        assert!(z.just_exhausted);
    }

    #[test]
    fn tick_exhausted_clears_just_activated_flag() {
        let mut z = Zip::new(1.0, 2.0, 1.0);
        z.activate();
        z.tick(1.0); // exhausted immediately
        assert!(!z.just_activated); // cleared by tick
        assert!(z.just_exhausted);
    }

    #[test]
    fn tick_just_exhausted_clears_next_frame() {
        let mut z = Zip::new(2.0, 2.0, 1.0);
        z.activate();
        z.tick(1.0); // exhausted
        z.tick(0.016);
        assert!(!z.just_exhausted);
    }

    // --- tick: recharging ---

    #[test]
    fn tick_recharges_while_idle() {
        let mut z = Zip::new(10.0, 2.0, 1.0);
        z.zip_charge = 5.0;
        z.tick(2.0); // idle, recharge=1/s → +2
        assert!((z.zip_charge - 7.0).abs() < 1e-5);
    }

    #[test]
    fn tick_recharge_caps_at_max() {
        let mut z = z();
        z.zip_charge = 9.0;
        z.tick(10.0); // would overshoot, capped at 10
        assert!((z.zip_charge - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_recharge_while_active() {
        let mut z = z();
        z.zip_charge = 5.0;
        z.is_active = true;
        z.drain_rate = 0.0; // no drain so charge stays constant
        z.tick(1.0);
        assert!((z.zip_charge - 5.0).abs() < 1e-5);
    }

    // --- tick: disabled ---

    #[test]
    fn tick_no_drain_when_disabled() {
        let mut z = z();
        z.activate();
        z.enabled = false;
        z.tick(10.0);
        assert!((z.zip_charge - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_flags_when_disabled() {
        let mut z = z();
        z.just_activated = true;
        z.just_exhausted = true;
        z.enabled = false;
        z.tick(0.016);
        assert!(!z.just_activated);
        assert!(!z.just_exhausted);
    }

    // --- is_zipping ---

    #[test]
    fn is_zipping_false_when_idle() {
        let z = z();
        assert!(!z.is_zipping());
    }

    #[test]
    fn is_zipping_true_when_active_with_charge() {
        let mut z = z();
        z.activate();
        assert!(z.is_zipping());
    }

    #[test]
    fn is_zipping_false_when_disabled() {
        let mut z = z();
        z.activate();
        z.enabled = false;
        assert!(!z.is_zipping());
    }

    #[test]
    fn is_zipping_false_after_exhaustion() {
        let mut z = Zip::new(2.0, 10.0, 0.0);
        z.activate();
        z.tick(1.0); // exhausted
        assert!(!z.is_zipping());
    }

    // --- charge_fraction ---

    #[test]
    fn charge_fraction_one_when_full() {
        let z = z();
        assert!((z.charge_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn charge_fraction_half_at_mid() {
        let mut z = z();
        z.zip_charge = 5.0;
        assert!((z.charge_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn charge_fraction_zero_when_empty() {
        let mut z = z();
        z.zip_charge = 0.0;
        assert_eq!(z.charge_fraction(), 0.0);
    }

    // --- effective_boost ---

    #[test]
    fn effective_boost_passthrough_when_idle() {
        let z = z(); // idle
        assert!((z.effective_boost(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_boost_doubled_when_zipping_full() {
        let mut z = z(); // full charge, fraction=1.0 → 100*(1+1)=200
        z.activate();
        assert!((z.effective_boost(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_boost_at_half_charge() {
        let mut z = z();
        z.zip_charge = 5.0; // fraction=0.5 → 100*(1+0.5)=150
        z.activate();
        assert!((z.effective_boost(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_boost_passthrough_when_disabled() {
        let mut z = z();
        z.activate();
        z.enabled = false;
        assert!((z.effective_boost(100.0) - 100.0).abs() < 1e-4);
    }

    // --- full sprint cycle ---

    #[test]
    fn activate_drain_exhaust_recharge_cycle() {
        let mut z = Zip::new(4.0, 2.0, 1.0); // drain 2/s, recharge 1/s
        z.activate();
        z.tick(1.0); // drain 2 → 2 remaining
        assert!((z.zip_charge - 2.0).abs() < 1e-5);
        assert!(z.is_zipping());
        z.tick(1.0); // drain 2 → exhausted
        assert!(!z.is_zipping());
        assert!(z.just_exhausted);
        z.tick(2.0); // recharge 1/s × 2 = 2
        assert!((z.zip_charge - 2.0).abs() < 1e-5);
        z.activate(); // re-activate
        assert!(z.is_zipping());
    }
}

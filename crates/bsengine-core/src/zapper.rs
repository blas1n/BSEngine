use bevy_ecs::prelude::Component;

/// Electronic-discharge accumulation tracker named after the zapper,
/// a colloquial term for any device that delivers a sudden sharp
/// electrical discharge — from the handheld infrared remote control
/// that mediates between a couch-bound viewer and their television
/// set to the high-voltage insect electrocutor that crackles and
/// flashes above the doorway of every greasy spoon on every humid
/// summer evening. The word entered informal American English in the
/// late 1960s as television remote controls spread into middle-class
/// homes: the Zenith Space Command, introduced in 1956, was the first
/// truly wireless remote, and by the time the term "zapper" had
/// crystallised the device had already become a cultural object in
/// its own right — not merely a convenience but a locus of domestic
/// authority, the object over which siblings squabbled and partners
/// negotiated. In broadcast industry terminology, "zapping" describes
/// the viewer behaviour of rapidly channel-surfing during commercial
/// breaks, which advertisers began to study obsessively once VCR
/// fast-forward and then remote-channel-changing made the captive
/// audience a historical artifact. The insect-zapper sense layers
/// onto this the satisfying onomatopoeia of high-voltage electrical
/// discharge — the ultraviolet light lures the moth or mosquito into
/// the electric grid, the body bridges the gap, and the sharp crack
/// of the arc discharge punctuates the summer night with its peculiar
/// mix of the lethal and the comedic. Both senses converge on a
/// single semantic core: a device that concentrates and releases a
/// burst of energy at precisely the right moment to achieve an
/// instantaneous effect. `charge` builds via `zap(amount)` and
/// accumulates passively at `charge_rate` per second in `tick(dt)` or
/// is released via `deplete(amount)`.
///
/// Models electronic-discharge fill levels, remote-signal saturation
/// bars, zapper-charge accumulators, insect-trap energy gauges, burst-
/// energy fill levels, remote-control-signal saturation indicators,
/// electromagnetic-pulse accumulation bars, channel-switching-readiness
/// meters, arc-discharge fill levels, or any mechanic where a device,
/// turret, or character slowly charges a burst of energy — photon by
/// photon, electron by electron — until the capacitor is full and
/// the next trigger pull unleashes a brief but devastating discharge
/// that leaves the target momentarily bewildered, fried, or simply
/// watching a different channel.
///
/// `zap(amount)` adds charge; fires `just_zapped` when first reaching
/// `max_charge`. No-op when disabled.
///
/// `deplete(amount)` reduces charge immediately; fires `just_discharged`
/// when reaching 0. No-op when disabled or already discharged.
///
/// `tick(dt)` clears both flags, then increases charge by
/// `charge_rate * dt` (capped at `max_charge`). Fires `just_zapped`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_zapped()` returns `charge >= max_charge && enabled`.
///
/// `is_discharged()` returns `charge == 0.0` (not gated by `enabled`).
///
/// `charge_fraction()` returns `(charge / max_charge).clamp(0, 1)`.
///
/// `effective_signal(scale)` returns `scale * charge_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — charges at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zapper {
    pub charge: f32,
    pub max_charge: f32,
    pub charge_rate: f32,
    pub just_zapped: bool,
    pub just_discharged: bool,
    pub enabled: bool,
}

impl Zapper {
    pub fn new(max_charge: f32, charge_rate: f32) -> Self {
        Self {
            charge: 0.0,
            max_charge: max_charge.max(0.1),
            charge_rate: charge_rate.max(0.0),
            just_zapped: false,
            just_discharged: false,
            enabled: true,
        }
    }

    /// Add charge; fires `just_zapped` when first reaching max.
    /// No-op when disabled.
    pub fn zap(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.charge < self.max_charge;
        self.charge = (self.charge + amount).min(self.max_charge);
        if was_below && self.charge >= self.max_charge {
            self.just_zapped = true;
        }
    }

    /// Reduce charge; fires `just_discharged` when reaching 0.
    /// No-op when disabled or already discharged.
    pub fn deplete(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.charge <= 0.0 {
            return;
        }
        self.charge = (self.charge - amount).max(0.0);
        if self.charge <= 0.0 {
            self.just_discharged = true;
        }
    }

    /// Clear flags, then increase charge by `charge_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_zapped = false;
        self.just_discharged = false;
        if self.enabled && self.charge_rate > 0.0 && self.charge < self.max_charge {
            let was_below = self.charge < self.max_charge;
            self.charge = (self.charge + self.charge_rate * dt).min(self.max_charge);
            if was_below && self.charge >= self.max_charge {
                self.just_zapped = true;
            }
        }
    }

    /// `true` when charge is at maximum and component is enabled.
    pub fn is_zapped(&self) -> bool {
        self.charge >= self.max_charge && self.enabled
    }

    /// `true` when charge is 0 (not gated by `enabled`).
    pub fn is_discharged(&self) -> bool {
        self.charge == 0.0
    }

    /// Fraction of maximum charge [0.0, 1.0].
    pub fn charge_fraction(&self) -> f32 {
        (self.charge / self.max_charge).clamp(0.0, 1.0)
    }

    /// Returns `scale * charge_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_signal(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.charge_fraction()
    }
}

impl Default for Zapper {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zapper {
        Zapper::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_discharged() {
        let z = z();
        assert_eq!(z.charge, 0.0);
        assert!(z.is_discharged());
        assert!(!z.is_zapped());
    }

    #[test]
    fn new_clamps_max_charge() {
        let z = Zapper::new(-5.0, 1.5);
        assert!((z.max_charge - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_charge_rate() {
        let z = Zapper::new(100.0, -1.5);
        assert_eq!(z.charge_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zapper::default();
        assert!((z.max_charge - 100.0).abs() < 1e-5);
        assert!((z.charge_rate - 1.5).abs() < 1e-5);
    }

    // --- zap ---

    #[test]
    fn zap_adds_charge() {
        let mut z = z();
        z.zap(40.0);
        assert!((z.charge - 40.0).abs() < 1e-3);
    }

    #[test]
    fn zap_clamps_at_max() {
        let mut z = z();
        z.zap(200.0);
        assert!((z.charge - 100.0).abs() < 1e-3);
    }

    #[test]
    fn zap_fires_just_zapped_at_max() {
        let mut z = z();
        z.zap(100.0);
        assert!(z.just_zapped);
        assert!(z.is_zapped());
    }

    #[test]
    fn zap_no_just_zapped_when_already_at_max() {
        let mut z = z();
        z.charge = 100.0;
        z.zap(10.0);
        assert!(!z.just_zapped);
    }

    #[test]
    fn zap_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.zap(50.0);
        assert_eq!(z.charge, 0.0);
    }

    #[test]
    fn zap_no_op_when_amount_zero() {
        let mut z = z();
        z.zap(0.0);
        assert_eq!(z.charge, 0.0);
    }

    // --- deplete ---

    #[test]
    fn deplete_reduces_charge() {
        let mut z = z();
        z.charge = 60.0;
        z.deplete(20.0);
        assert!((z.charge - 40.0).abs() < 1e-3);
    }

    #[test]
    fn deplete_clamps_at_zero() {
        let mut z = z();
        z.charge = 30.0;
        z.deplete(200.0);
        assert_eq!(z.charge, 0.0);
    }

    #[test]
    fn deplete_fires_just_discharged_at_zero() {
        let mut z = z();
        z.charge = 30.0;
        z.deplete(30.0);
        assert!(z.just_discharged);
    }

    #[test]
    fn deplete_no_op_when_already_discharged() {
        let mut z = z();
        z.deplete(10.0);
        assert!(!z.just_discharged);
    }

    #[test]
    fn deplete_no_op_when_disabled() {
        let mut z = z();
        z.charge = 50.0;
        z.enabled = false;
        z.deplete(50.0);
        assert!((z.charge - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_charges_zapper() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.charge - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_zapped_on_charge_to_max() {
        let mut z = Zapper::new(100.0, 200.0);
        z.charge = 95.0;
        z.tick(1.0);
        assert!(z.just_zapped);
        assert!(z.is_zapped());
    }

    #[test]
    fn tick_no_charge_when_already_zapped() {
        let mut z = z();
        z.charge = 100.0;
        z.tick(1.0);
        assert!(!z.just_zapped);
    }

    #[test]
    fn tick_no_charge_when_rate_zero() {
        let mut z = Zapper::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.charge, 0.0);
    }

    #[test]
    fn tick_no_charge_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.charge, 0.0);
    }

    #[test]
    fn tick_clears_just_zapped() {
        let mut z = Zapper::new(100.0, 200.0);
        z.charge = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_zapped);
    }

    #[test]
    fn tick_clears_just_discharged() {
        let mut z = z();
        z.charge = 10.0;
        z.deplete(10.0);
        z.tick(0.016);
        assert!(!z.just_discharged);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.charge - 9.0).abs() < 1e-3);
    }

    // --- is_zapped / is_discharged ---

    #[test]
    fn is_zapped_false_when_disabled() {
        let mut z = z();
        z.charge = 100.0;
        z.enabled = false;
        assert!(!z.is_zapped());
    }

    #[test]
    fn is_discharged_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_discharged());
    }

    // --- charge_fraction / effective_signal ---

    #[test]
    fn charge_fraction_zero_when_discharged() {
        assert_eq!(z().charge_fraction(), 0.0);
    }

    #[test]
    fn charge_fraction_half_at_midpoint() {
        let mut z = z();
        z.charge = 50.0;
        assert!((z.charge_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_signal_zero_when_discharged() {
        assert_eq!(z().effective_signal(100.0), 0.0);
    }

    #[test]
    fn effective_signal_scales_with_charge() {
        let mut z = z();
        z.charge = 75.0;
        assert!((z.effective_signal(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_signal_zero_when_disabled() {
        let mut z = z();
        z.charge = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_signal(100.0), 0.0);
    }
}

use bevy_ecs::prelude::Component;

/// Group-harmony tracker. `harmony` builds via `align(amount)` and
/// grows passively at `attune_rate` per second in `tick(dt)` or is
/// broken immediately via `discord(amount)`.
///
/// Models collective resonance meters, faction-cohesion gauges, choir
/// synchronisation bars, squad morale trackers, spirit-circle charges,
/// or any mechanic where multiple entities tuning together gradually
/// build shared power — and a single dissonant event can break that
/// connection.
///
/// `align(amount)` adds harmony; fires `just_harmonized` when first
/// reaching `max_harmony`. No-op when disabled.
///
/// `discord(amount)` reduces harmony immediately; fires `just_discordant`
/// when reaching 0. No-op when disabled or already discordant.
///
/// `tick(dt)` clears both flags, then attunes harmony by
/// `attune_rate * dt` (capped at `max_harmony`). Fires `just_harmonized`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_harmonized()` returns `harmony >= max_harmony && enabled`.
///
/// `is_discordant()` returns `harmony == 0.0` (not gated by `enabled`).
///
/// `harmony_fraction()` returns `(harmony / max_harmony).clamp(0, 1)`.
///
/// `effective_resonance(scale)` returns `scale * harmony_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 6.0)` — attunes at 6 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zendo {
    pub harmony: f32,
    pub max_harmony: f32,
    pub attune_rate: f32,
    pub just_harmonized: bool,
    pub just_discordant: bool,
    pub enabled: bool,
}

impl Zendo {
    pub fn new(max_harmony: f32, attune_rate: f32) -> Self {
        Self {
            harmony: 0.0,
            max_harmony: max_harmony.max(0.1),
            attune_rate: attune_rate.max(0.0),
            just_harmonized: false,
            just_discordant: false,
            enabled: true,
        }
    }

    /// Add harmony; fires `just_harmonized` when first reaching max.
    /// No-op when disabled.
    pub fn align(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.harmony < self.max_harmony;
        self.harmony = (self.harmony + amount).min(self.max_harmony);
        if was_below && self.harmony >= self.max_harmony {
            self.just_harmonized = true;
        }
    }

    /// Reduce harmony; fires `just_discordant` when reaching 0.
    /// No-op when disabled or already discordant.
    pub fn discord(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.harmony <= 0.0 {
            return;
        }
        self.harmony = (self.harmony - amount).max(0.0);
        if self.harmony <= 0.0 {
            self.just_discordant = true;
        }
    }

    /// Clear flags, then attune harmony by `attune_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_harmonized = false;
        self.just_discordant = false;
        if self.enabled && self.attune_rate > 0.0 && self.harmony < self.max_harmony {
            let was_below = self.harmony < self.max_harmony;
            self.harmony = (self.harmony + self.attune_rate * dt).min(self.max_harmony);
            if was_below && self.harmony >= self.max_harmony {
                self.just_harmonized = true;
            }
        }
    }

    /// `true` when harmony is at maximum and component is enabled.
    pub fn is_harmonized(&self) -> bool {
        self.harmony >= self.max_harmony && self.enabled
    }

    /// `true` when harmony is 0 (not gated by `enabled`).
    pub fn is_discordant(&self) -> bool {
        self.harmony == 0.0
    }

    /// Fraction of maximum harmony [0.0, 1.0].
    pub fn harmony_fraction(&self) -> f32 {
        (self.harmony / self.max_harmony).clamp(0.0, 1.0)
    }

    /// Returns `scale * harmony_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_resonance(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.harmony_fraction()
    }
}

impl Default for Zendo {
    fn default() -> Self {
        Self::new(100.0, 6.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zendo {
        Zendo::new(100.0, 6.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_discordant() {
        let z = z();
        assert_eq!(z.harmony, 0.0);
        assert!(z.is_discordant());
        assert!(!z.is_harmonized());
    }

    #[test]
    fn new_clamps_max_harmony() {
        let z = Zendo::new(-5.0, 6.0);
        assert!((z.max_harmony - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_attune_rate() {
        let z = Zendo::new(100.0, -3.0);
        assert_eq!(z.attune_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zendo::default();
        assert!((z.max_harmony - 100.0).abs() < 1e-5);
        assert!((z.attune_rate - 6.0).abs() < 1e-5);
    }

    // --- align ---

    #[test]
    fn align_adds_harmony() {
        let mut z = z();
        z.align(40.0);
        assert!((z.harmony - 40.0).abs() < 1e-3);
    }

    #[test]
    fn align_clamps_at_max() {
        let mut z = z();
        z.align(200.0);
        assert!((z.harmony - 100.0).abs() < 1e-3);
    }

    #[test]
    fn align_fires_just_harmonized_at_max() {
        let mut z = z();
        z.align(100.0);
        assert!(z.just_harmonized);
        assert!(z.is_harmonized());
    }

    #[test]
    fn align_no_just_harmonized_when_already_at_max() {
        let mut z = z();
        z.harmony = 100.0;
        z.align(10.0);
        assert!(!z.just_harmonized);
    }

    #[test]
    fn align_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.align(50.0);
        assert_eq!(z.harmony, 0.0);
    }

    #[test]
    fn align_no_op_when_amount_zero() {
        let mut z = z();
        z.align(0.0);
        assert_eq!(z.harmony, 0.0);
    }

    // --- discord ---

    #[test]
    fn discord_reduces_harmony() {
        let mut z = z();
        z.harmony = 60.0;
        z.discord(20.0);
        assert!((z.harmony - 40.0).abs() < 1e-3);
    }

    #[test]
    fn discord_clamps_at_zero() {
        let mut z = z();
        z.harmony = 30.0;
        z.discord(200.0);
        assert_eq!(z.harmony, 0.0);
    }

    #[test]
    fn discord_fires_just_discordant_at_zero() {
        let mut z = z();
        z.harmony = 30.0;
        z.discord(30.0);
        assert!(z.just_discordant);
    }

    #[test]
    fn discord_no_op_when_already_discordant() {
        let mut z = z();
        z.discord(10.0);
        assert!(!z.just_discordant);
    }

    #[test]
    fn discord_no_op_when_disabled() {
        let mut z = z();
        z.harmony = 50.0;
        z.enabled = false;
        z.discord(50.0);
        assert!((z.harmony - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_attunes_harmony() {
        let mut z = z(); // attune=6
        z.tick(1.0); // 0 + 6 = 6
        assert!((z.harmony - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_harmonized_on_attune_to_max() {
        let mut z = Zendo::new(100.0, 200.0);
        z.harmony = 95.0;
        z.tick(1.0);
        assert!(z.just_harmonized);
        assert!(z.is_harmonized());
    }

    #[test]
    fn tick_no_attune_when_already_at_max() {
        let mut z = z();
        z.harmony = 100.0;
        z.tick(1.0);
        assert!(!z.just_harmonized);
    }

    #[test]
    fn tick_no_attune_when_rate_zero() {
        let mut z = Zendo::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.harmony, 0.0);
    }

    #[test]
    fn tick_no_attune_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.harmony, 0.0);
    }

    #[test]
    fn tick_clears_just_harmonized() {
        let mut z = Zendo::new(100.0, 200.0);
        z.harmony = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_harmonized);
    }

    #[test]
    fn tick_clears_just_discordant() {
        let mut z = z();
        z.harmony = 10.0;
        z.discord(10.0);
        z.tick(0.016);
        assert!(!z.just_discordant);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // attune=6
        z.tick(4.0); // 6*4 = 24
        assert!((z.harmony - 24.0).abs() < 1e-3);
    }

    // --- is_harmonized / is_discordant ---

    #[test]
    fn is_harmonized_false_when_disabled() {
        let mut z = z();
        z.harmony = 100.0;
        z.enabled = false;
        assert!(!z.is_harmonized());
    }

    #[test]
    fn is_discordant_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_discordant());
    }

    // --- harmony_fraction / effective_resonance ---

    #[test]
    fn harmony_fraction_zero_when_discordant() {
        assert_eq!(z().harmony_fraction(), 0.0);
    }

    #[test]
    fn harmony_fraction_half_at_midpoint() {
        let mut z = z();
        z.harmony = 50.0;
        assert!((z.harmony_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_resonance_zero_when_discordant() {
        assert_eq!(z().effective_resonance(100.0), 0.0);
    }

    #[test]
    fn effective_resonance_scales_with_harmony() {
        let mut z = z();
        z.harmony = 80.0;
        assert!((z.effective_resonance(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_resonance_zero_when_disabled() {
        let mut z = z();
        z.harmony = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_resonance(100.0), 0.0);
    }
}

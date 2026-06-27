use bevy_ecs::prelude::Component;

/// Void-drain charge reservoir: each attack drains a fraction of the damage
/// dealt into `void_charge`. When `void_charge` reaches `max_charge`, the
/// reservoir peaks and sets `just_peaked` for one frame. The burst system
/// calls `release()` to consume all stored charge and receive the value back
/// for the burst effect.
///
/// `drain(damage)` is called by the damage system on each outgoing hit: it
/// converts `damage * drain_fraction` into void charge (capped at
/// `max_charge`) and sets `just_peaked` on the first crossing to full.
/// `release()` returns the stored `void_charge` and resets it; returns `0.0`
/// when the reservoir is empty or disabled. `tick()` clears one-frame flags.
///
/// Distinct from `Siphon` (steals HP directly), `Leech` (converts damage to
/// healing), `Drain` (steals a target's resource), and `Mana` (magic energy
/// pool): Void is a **dealt-damage-fueled burst reservoir** — it accumulates
/// passively on every hit and pays out as a configurable burst on demand.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Void {
    /// Current stored void energy. Always in [0.0, max_charge].
    pub void_charge: f32,
    /// Maximum storable void energy. Clamped ≥ 1.0.
    pub max_charge: f32,
    /// Fraction of outgoing damage converted to void charge. Clamped [0.0, 1.0].
    pub drain_fraction: f32,
    pub just_peaked: bool,
    pub enabled: bool,
}

impl Void {
    pub fn new(max_charge: f32, drain_fraction: f32) -> Self {
        Self {
            void_charge: 0.0,
            max_charge: max_charge.max(1.0),
            drain_fraction: drain_fraction.clamp(0.0, 1.0),
            just_peaked: false,
            enabled: true,
        }
    }

    /// Convert a portion of outgoing damage into void charge. Returns the
    /// amount drained (≥ 0). Sets `just_peaked` on the first crossing to
    /// `max_charge`. No-op (returns 0.0) when disabled.
    pub fn drain(&mut self, damage: f32) -> f32 {
        if !self.enabled || damage <= 0.0 {
            return 0.0;
        }
        let gained = damage * self.drain_fraction;
        let was_below_peak = self.void_charge < self.max_charge;
        self.void_charge = (self.void_charge + gained).min(self.max_charge);
        if was_below_peak && self.void_charge >= self.max_charge {
            self.just_peaked = true;
        }
        gained.min(self.max_charge - (self.void_charge - gained).max(0.0))
    }

    /// Consume all stored void charge. Returns the amount released (for use
    /// as burst power). Returns `0.0` when empty or disabled. Does not clear
    /// `just_peaked` — that is handled by `tick()`.
    pub fn release(&mut self) -> f32 {
        if !self.enabled || self.void_charge <= 0.0 {
            return 0.0;
        }
        let amount = self.void_charge;
        self.void_charge = 0.0;
        amount
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_peaked = false;
    }

    pub fn is_full(&self) -> bool {
        self.void_charge >= self.max_charge
    }

    pub fn is_empty(&self) -> bool {
        self.void_charge <= 0.0
    }

    /// Stored charge as a fraction of max [0.0 = empty, 1.0 = full].
    pub fn charge_fraction(&self) -> f32 {
        if self.max_charge <= 0.0 {
            return 0.0;
        }
        (self.void_charge / self.max_charge).clamp(0.0, 1.0)
    }
}

impl Default for Void {
    fn default() -> Self {
        Self::new(100.0, 0.15)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_accumulates_charge() {
        let mut v = Void::new(100.0, 0.1);
        v.drain(200.0);
        assert!((v.void_charge - 20.0).abs() < 1e-3);
    }

    #[test]
    fn drain_caps_at_max() {
        let mut v = Void::new(50.0, 1.0);
        v.drain(200.0);
        assert!((v.void_charge - 50.0).abs() < 1e-3);
    }

    #[test]
    fn drain_sets_just_peaked_on_reaching_max() {
        let mut v = Void::new(50.0, 1.0);
        v.drain(50.0);
        assert!(v.just_peaked);
        assert!(v.is_full());
    }

    #[test]
    fn drain_no_just_peaked_when_below_max() {
        let mut v = Void::new(100.0, 0.1);
        v.drain(50.0); // 5.0 charge, not full
        assert!(!v.just_peaked);
    }

    #[test]
    fn drain_no_just_peaked_when_already_full() {
        let mut v = Void::new(50.0, 1.0);
        v.drain(50.0); // peaks here
        v.tick();
        v.drain(50.0); // already full
        assert!(!v.just_peaked);
    }

    #[test]
    fn release_returns_charge_and_resets() {
        let mut v = Void::new(100.0, 0.5);
        v.drain(100.0); // 50 charge
        let amount = v.release();
        assert!((amount - 50.0).abs() < 1e-3);
        assert!(v.is_empty());
    }

    #[test]
    fn release_returns_zero_when_empty() {
        let mut v = Void::new(100.0, 0.5);
        let amount = v.release();
        assert!((amount).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut v = Void::new(50.0, 1.0);
        v.drain(50.0);
        v.tick();
        assert!(!v.just_peaked);
    }

    #[test]
    fn charge_fraction_at_half() {
        let mut v = Void::new(100.0, 0.5);
        v.drain(100.0); // 50 charge
        assert!((v.charge_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn charge_fraction_zero_when_empty() {
        let v = Void::new(100.0, 0.5);
        assert!((v.charge_fraction()).abs() < 1e-5);
    }

    #[test]
    fn charge_fraction_one_when_full() {
        let mut v = Void::new(50.0, 1.0);
        v.drain(50.0);
        assert!((v.charge_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn drain_after_release_refills_from_zero() {
        let mut v = Void::new(50.0, 1.0);
        v.drain(50.0);
        v.release();
        v.tick();
        v.drain(30.0); // 30 charge
        assert!((v.void_charge - 30.0).abs() < 1e-3);
        assert!(!v.just_peaked);
    }

    #[test]
    fn disabled_drain_no_op() {
        let mut v = Void::new(100.0, 0.5);
        v.enabled = false;
        v.drain(200.0);
        assert!(v.is_empty());
        assert!(!v.just_peaked);
    }

    #[test]
    fn disabled_release_returns_zero() {
        let mut v = Void::new(50.0, 1.0);
        v.drain(50.0);
        v.enabled = false;
        let amount = v.release();
        assert!((amount).abs() < 1e-5);
        assert!((v.void_charge - 50.0).abs() < 1e-3); // charge preserved
    }

    #[test]
    fn max_charge_clamped_to_one() {
        let v = Void::new(0.0, 0.5);
        assert!((v.max_charge - 1.0).abs() < 1e-5);
    }
}

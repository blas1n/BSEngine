use bevy_ecs::prelude::Component;

/// Temporary bonus HP pool layered on top of the entity's normal health.
///
/// Incoming damage should call `absorb(amount)` first; it consumes up to
/// `current` before returning any remainder to be applied to base health.
/// The overshield decays passively at `decay_rate` HP/s via `tick(dt)`.
/// `grant(amount)` adds HP capped at `max_overshield`, setting `just_granted`
/// on the first grant (0 → any positive value). `tick(dt)` sets `just_depleted`
/// when the pool hits zero.
///
/// Distinct from `Absorption` (converts damage to a resource — no HP buffer),
/// `Barrier` (physical terrain/physics blockade), and `Shield` (parry/deflect
/// mechanic with directional timing): Overshield is a **temporary bonus HP
/// pool** — it absorbs damage directly and decays over time.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Overshield {
    pub current: f32,
    /// Maximum overshield HP. Clamped ≥ 0.0.
    pub max_overshield: f32,
    /// HP per second the overshield loses passively when positive. Clamped ≥ 0.0.
    pub decay_rate: f32,
    pub just_granted: bool,
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Overshield {
    pub fn new(max_overshield: f32, decay_rate: f32) -> Self {
        Self {
            current: 0.0,
            max_overshield: max_overshield.max(0.0),
            decay_rate: decay_rate.max(0.0),
            just_granted: false,
            just_depleted: false,
            enabled: true,
        }
    }

    /// Add `amount` HP to the overshield pool, capped at `max_overshield`.
    /// Sets `just_granted` on the 0 → positive transition. No-op when disabled.
    pub fn grant(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        let was_empty = self.current <= 0.0;
        self.current = (self.current + amount.max(0.0)).min(self.max_overshield);
        if was_empty && self.current > 0.0 {
            self.just_granted = true;
        }
    }

    /// Absorb up to `current` HP of incoming damage. Returns the damage that
    /// exceeds the overshield (to be applied to base health). Returns the full
    /// `amount` if disabled or empty.
    pub fn absorb(&mut self, amount: f32) -> f32 {
        if !self.enabled || self.current <= 0.0 {
            return amount;
        }
        let absorbed = amount.min(self.current);
        self.current -= absorbed;
        amount - absorbed
    }

    /// Advance passive decay and clear one-frame flags. Sets `just_depleted`
    /// when the pool hits zero this frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_granted = false;
        self.just_depleted = false;

        if self.current > 0.0 && self.decay_rate > 0.0 {
            let was_positive = self.current > 0.0;
            self.current = (self.current - self.decay_rate * dt).max(0.0);
            if was_positive && self.current <= 0.0 {
                self.just_depleted = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.current > 0.0
    }

    /// Fraction of max overshield remaining [0.0 = empty, 1.0 = full].
    pub fn fraction(&self) -> f32 {
        if self.max_overshield <= 0.0 {
            return 0.0;
        }
        (self.current / self.max_overshield).clamp(0.0, 1.0)
    }
}

impl Default for Overshield {
    fn default() -> Self {
        Self::new(50.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_adds_hp() {
        let mut o = Overshield::new(50.0, 5.0);
        o.grant(30.0);
        assert!((o.current - 30.0).abs() < 1e-4);
        assert!(o.just_granted);
        assert!(o.is_active());
    }

    #[test]
    fn grant_caps_at_max() {
        let mut o = Overshield::new(50.0, 5.0);
        o.grant(100.0);
        assert!((o.current - 50.0).abs() < 1e-4);
    }

    #[test]
    fn just_granted_only_on_zero_to_positive() {
        let mut o = Overshield::new(50.0, 5.0);
        o.grant(20.0);
        o.tick(0.016); // clears just_granted
        o.grant(10.0); // already positive — just_granted NOT set
        assert!(!o.just_granted);
    }

    #[test]
    fn absorb_consumes_overshield() {
        let mut o = Overshield::new(50.0, 5.0);
        o.grant(30.0);
        let remainder = o.absorb(20.0);
        assert!((remainder - 0.0).abs() < 1e-5);
        assert!((o.current - 10.0).abs() < 1e-4);
    }

    #[test]
    fn absorb_passes_excess_through() {
        let mut o = Overshield::new(50.0, 5.0);
        o.grant(10.0);
        let remainder = o.absorb(30.0);
        assert!((remainder - 20.0).abs() < 1e-4);
        assert!((o.current - 0.0).abs() < 1e-5);
    }

    #[test]
    fn absorb_full_pass_when_empty() {
        let mut o = Overshield::new(50.0, 5.0);
        let remainder = o.absorb(30.0);
        assert!((remainder - 30.0).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_current() {
        let mut o = Overshield::new(50.0, 10.0);
        o.grant(50.0);
        o.tick(2.0); // 10/s * 2s = 20 decay
        assert!((o.current - 30.0).abs() < 1e-3);
    }

    #[test]
    fn tick_sets_just_depleted() {
        let mut o = Overshield::new(50.0, 100.0);
        o.grant(10.0);
        o.tick(1.0); // decay 100 → depletes
        assert!(!o.is_active());
        assert!(o.just_depleted);
    }

    #[test]
    fn tick_clears_just_granted() {
        let mut o = Overshield::new(50.0, 5.0);
        o.grant(20.0);
        o.tick(0.016);
        assert!(!o.just_granted);
    }

    #[test]
    fn fraction_at_half() {
        let mut o = Overshield::new(50.0, 5.0);
        o.grant(25.0);
        assert!((o.fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn fraction_zero_when_empty() {
        let o = Overshield::new(50.0, 5.0);
        assert!((o.fraction() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_grant_no_op() {
        let mut o = Overshield::new(50.0, 5.0);
        o.enabled = false;
        o.grant(30.0);
        assert!(!o.is_active());
    }

    #[test]
    fn disabled_absorb_passes_all_through() {
        let mut o = Overshield::new(50.0, 5.0);
        o.grant(30.0);
        o.enabled = false;
        let remainder = o.absorb(20.0);
        assert!((remainder - 20.0).abs() < 1e-5);
        assert!((o.current - 30.0).abs() < 1e-4); // not consumed
    }
}

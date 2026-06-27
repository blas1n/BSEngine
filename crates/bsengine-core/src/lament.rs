use bevy_ecs::prelude::Component;

/// Grief-driven morale debuff that compounds a damage and movement penalty
/// and naturally decays over time.
///
/// Triggered by ally-death events or traumatic game conditions; call
/// `apply(intensity)` (high-watermark) to set the grief level. While
/// `is_lamenting()`, `effective_damage(base)` and `effective_speed(base)`
/// return penalised values. `tick(dt)` decays intensity at `decay_rate` per
/// second; once intensity reaches 0 `just_recovered` is set.
///
/// Distinct from `Fear` (flees from a threat — changes movement target),
/// `Demoralize` (attack-speed penalty, no decay), and `Weaken` (outgoing-
/// damage multiplier only): Lament is a **grief compound penalty** — it hits
/// both damage and speed, and heals automatically without external intervention.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Lament {
    /// Current grief intensity [0.0, 1.0]. Decays each tick.
    pub intensity: f32,
    /// Intensity subtracted per second. Clamped ≥ 0.0.
    pub decay_rate: f32,
    /// Fraction of outgoing damage lost at full intensity. Clamped to [0.0, 1.0].
    /// Effective penalty scales with `intensity`: `damage_penalty * intensity`.
    pub damage_penalty: f32,
    /// Fraction of base move speed lost at full intensity. Clamped to [0.0, 1.0].
    /// Effective penalty scales with `intensity`.
    pub speed_penalty: f32,
    pub just_lamented: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Lament {
    pub fn new(damage_penalty: f32, speed_penalty: f32, decay_rate: f32) -> Self {
        Self {
            intensity: 0.0,
            decay_rate: decay_rate.max(0.0),
            damage_penalty: damage_penalty.clamp(0.0, 1.0),
            speed_penalty: speed_penalty.clamp(0.0, 1.0),
            just_lamented: false,
            just_recovered: false,
            enabled: true,
        }
    }

    /// Apply or raise grief intensity. High-watermark: only increases intensity,
    /// never lowers it. `intensity` clamped to [0.0, 1.0]. No-op when disabled.
    /// Sets `just_lamented` on the inactive → active (0 → positive) transition.
    pub fn apply(&mut self, intensity: f32) {
        if !self.enabled {
            return;
        }
        let clamped = intensity.clamp(0.0, 1.0);
        if clamped > self.intensity {
            let was_inactive = self.intensity <= 0.0;
            self.intensity = clamped;
            if was_inactive {
                self.just_lamented = true;
            }
        }
    }

    /// Advance grief decay. Reduces intensity by `decay_rate * dt`; sets
    /// `just_recovered` when intensity reaches 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_lamented = false;
        self.just_recovered = false;

        if self.intensity > 0.0 {
            self.intensity -= self.decay_rate * dt;
            if self.intensity <= 0.0 {
                self.intensity = 0.0;
                self.just_recovered = true;
            }
        }
    }

    pub fn is_lamenting(&self) -> bool {
        self.intensity > 0.0
    }

    /// Effective outgoing damage after grief penalty. Returns
    /// `base * (1 - damage_penalty * intensity)` when lamenting and enabled,
    /// `base` otherwise. Never negative.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_lamenting() && self.enabled {
            (base * (1.0 - self.damage_penalty * self.intensity)).max(0.0)
        } else {
            base
        }
    }

    /// Effective move speed after grief penalty. Returns
    /// `base * (1 - speed_penalty * intensity)` when lamenting and enabled,
    /// `base` otherwise. Never negative.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if self.is_lamenting() && self.enabled {
            (base * (1.0 - self.speed_penalty * self.intensity)).max(0.0)
        } else {
            base
        }
    }
}

impl Default for Lament {
    fn default() -> Self {
        Self::new(0.3, 0.2, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_sets_intensity() {
        let mut l = Lament::new(0.3, 0.2, 0.5);
        l.apply(0.8);
        assert!((l.intensity - 0.8).abs() < 1e-5);
        assert!(l.is_lamenting());
        assert!(l.just_lamented);
    }

    #[test]
    fn apply_high_watermark_no_lower() {
        let mut l = Lament::new(0.3, 0.2, 0.5);
        l.apply(0.8);
        l.apply(0.4); // lower — ignored
        assert!((l.intensity - 0.8).abs() < 1e-5);
    }

    #[test]
    fn apply_raises_intensity() {
        let mut l = Lament::new(0.3, 0.2, 0.5);
        l.apply(0.4);
        l.tick(0.016);
        l.apply(0.9); // higher — accepted
        assert!((l.intensity - 0.9).abs() < 1e-5);
    }

    #[test]
    fn just_lamented_only_on_activation() {
        let mut l = Lament::new(0.3, 0.2, 0.5);
        l.apply(0.5);
        l.tick(0.016);
        l.apply(0.8); // raises but not a 0→positive transition
        assert!(!l.just_lamented);
    }

    #[test]
    fn apply_clamped_to_one() {
        let mut l = Lament::new(0.3, 0.2, 0.5);
        l.apply(1.5);
        assert!((l.intensity - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_intensity() {
        let mut l = Lament::new(0.3, 0.2, 0.5);
        l.apply(1.0);
        l.tick(1.0); // 1.0 - 0.5 * 1.0 = 0.5
        assert!((l.intensity - 0.5).abs() < 1e-3);
    }

    #[test]
    fn tick_sets_just_recovered_at_zero() {
        let mut l = Lament::new(0.3, 0.2, 1.0);
        l.apply(0.5);
        l.tick(0.6); // 0.5 - 1.0 * 0.6 → ≤ 0
        assert!(!l.is_lamenting());
        assert!(l.just_recovered);
    }

    #[test]
    fn tick_clears_just_lamented() {
        let mut l = Lament::new(0.3, 0.2, 0.5);
        l.apply(0.8);
        l.tick(0.016);
        assert!(!l.just_lamented);
    }

    #[test]
    fn effective_damage_penalised() {
        let mut l = Lament::new(0.4, 0.2, 0.5);
        l.apply(0.5); // intensity = 0.5 → penalty = 0.4*0.5 = 0.2
                      // 100 * (1 - 0.2) = 80
        assert!((l.effective_damage(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_full_when_not_lamenting() {
        let l = Lament::new(0.4, 0.2, 0.5);
        assert!((l.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_penalised() {
        let mut l = Lament::new(0.3, 0.5, 0.5);
        l.apply(0.4); // penalty = 0.5 * 0.4 = 0.2
                      // 10 * (1 - 0.2) = 8.0
        assert!((l.effective_speed(10.0) - 8.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_full_when_not_lamenting() {
        let l = Lament::new(0.3, 0.5, 0.5);
        assert!((l.effective_speed(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn effective_damage_not_negative() {
        let mut l = Lament::new(1.0, 1.0, 0.5);
        l.apply(1.0);
        assert!(l.effective_damage(50.0) >= 0.0);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut l = Lament::new(0.3, 0.2, 0.5);
        l.enabled = false;
        l.apply(0.8);
        assert!(!l.is_lamenting());
    }

    #[test]
    fn disabled_effective_damage_full() {
        let mut l = Lament::new(0.3, 0.2, 0.5);
        l.apply(0.8);
        l.enabled = false;
        assert!((l.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }
}

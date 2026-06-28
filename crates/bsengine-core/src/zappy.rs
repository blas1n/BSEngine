use bevy_ecs::prelude::Component;

/// Vital-spark accumulation tracker named after zappy, the adjective
/// coined in the mid-twentieth century to describe something lively,
/// energetic, and possessed of an almost electrical vitality — the
/// quality that makes a piece of dialogue crackle, a sporting
/// performance dazzle, or a children's television presenter survive
/// seven hours of broadcast without once allowing the camera to catch
/// them slumping. The word carries the onomatopoeia of its root: zap,
/// that compact monosyllable that compresses the entire arc of an
/// electrical discharge — the build-up, the jump, the crack, the
/// sudden smell of ozone — into a single phoneme. Zappy is what
/// happens when that energy is not a one-off discharge but a sustained
/// condition: the person, presentation, or performance that maintains
/// an uninterrupted crackle of enthusiasm and precision, never quite
/// fully discharging into the merely competent. In informal usage the
/// word attaches itself preferentially to experiences that are sharp
/// rather than merely loud, focused rather than scattered: a zappy
/// headline is not a noisy one but a precise one that carries a small
/// concentrated charge of wit or surprise; a zappy piece of code is
/// not sprawling but tight, its logic moving with the unexplained
/// fluency of an expert who has solved this problem so many times that
/// the intermediate steps have collapsed into reflex. The aesthetic
/// of zappiness is therefore one of economy: the minimum intervention
/// required to produce the maximum jolt. `vitality` builds via
/// `energize(amount)` and accumulates passively at `spark_rate` per
/// second in `tick(dt)` or bleeds off via `fizzle(amount)`.
///
/// Models vital-spark fill levels, energetic-presence saturation bars,
/// comedic-timing-readiness accumulators, zestful-performance gauges,
/// audience-engagement fill levels, witty-dialogue saturation
/// indicators, precision-spark accumulation bars, electrical-vitality
/// meters, sharp-performance fill levels, or any mechanic where a
/// character, device, or creative work slowly charges with a crackling,
/// focused energy until it reaches peak zappiness — the state where
/// every word, action, or frame carries more charge per unit than the
/// medium should technically be able to sustain.
///
/// `energize(amount)` adds vitality; fires `just_sparked` when first
/// reaching `max_vitality`. No-op when disabled.
///
/// `fizzle(amount)` reduces vitality immediately; fires `just_fizzled`
/// when reaching 0. No-op when disabled or already fizzled.
///
/// `tick(dt)` clears both flags, then increases vitality by
/// `spark_rate * dt` (capped at `max_vitality`). Fires `just_sparked`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_sparked()` returns `vitality >= max_vitality && enabled`.
///
/// `is_fizzled()` returns `vitality == 0.0` (not gated by `enabled`).
///
/// `vitality_fraction()` returns `(vitality / max_vitality).clamp(0, 1)`.
///
/// `effective_zest(scale)` returns `scale * vitality_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — sparks at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zappy {
    pub vitality: f32,
    pub max_vitality: f32,
    pub spark_rate: f32,
    pub just_sparked: bool,
    pub just_fizzled: bool,
    pub enabled: bool,
}

impl Zappy {
    pub fn new(max_vitality: f32, spark_rate: f32) -> Self {
        Self {
            vitality: 0.0,
            max_vitality: max_vitality.max(0.1),
            spark_rate: spark_rate.max(0.0),
            just_sparked: false,
            just_fizzled: false,
            enabled: true,
        }
    }

    /// Add vitality; fires `just_sparked` when first reaching max.
    /// No-op when disabled.
    pub fn energize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.vitality < self.max_vitality;
        self.vitality = (self.vitality + amount).min(self.max_vitality);
        if was_below && self.vitality >= self.max_vitality {
            self.just_sparked = true;
        }
    }

    /// Reduce vitality; fires `just_fizzled` when reaching 0.
    /// No-op when disabled or already fizzled.
    pub fn fizzle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.vitality <= 0.0 {
            return;
        }
        self.vitality = (self.vitality - amount).max(0.0);
        if self.vitality <= 0.0 {
            self.just_fizzled = true;
        }
    }

    /// Clear flags, then increase vitality by `spark_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_sparked = false;
        self.just_fizzled = false;
        if self.enabled && self.spark_rate > 0.0 && self.vitality < self.max_vitality {
            let was_below = self.vitality < self.max_vitality;
            self.vitality = (self.vitality + self.spark_rate * dt).min(self.max_vitality);
            if was_below && self.vitality >= self.max_vitality {
                self.just_sparked = true;
            }
        }
    }

    /// `true` when vitality is at maximum and component is enabled.
    pub fn is_sparked(&self) -> bool {
        self.vitality >= self.max_vitality && self.enabled
    }

    /// `true` when vitality is 0 (not gated by `enabled`).
    pub fn is_fizzled(&self) -> bool {
        self.vitality == 0.0
    }

    /// Fraction of maximum vitality [0.0, 1.0].
    pub fn vitality_fraction(&self) -> f32 {
        (self.vitality / self.max_vitality).clamp(0.0, 1.0)
    }

    /// Returns `scale * vitality_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_zest(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.vitality_fraction()
    }
}

impl Default for Zappy {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zappy {
        Zappy::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_fizzled() {
        let z = z();
        assert_eq!(z.vitality, 0.0);
        assert!(z.is_fizzled());
        assert!(!z.is_sparked());
    }

    #[test]
    fn new_clamps_max_vitality() {
        let z = Zappy::new(-5.0, 1.5);
        assert!((z.max_vitality - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_spark_rate() {
        let z = Zappy::new(100.0, -1.5);
        assert_eq!(z.spark_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zappy::default();
        assert!((z.max_vitality - 100.0).abs() < 1e-5);
        assert!((z.spark_rate - 1.5).abs() < 1e-5);
    }

    // --- energize ---

    #[test]
    fn energize_adds_vitality() {
        let mut z = z();
        z.energize(40.0);
        assert!((z.vitality - 40.0).abs() < 1e-3);
    }

    #[test]
    fn energize_clamps_at_max() {
        let mut z = z();
        z.energize(200.0);
        assert!((z.vitality - 100.0).abs() < 1e-3);
    }

    #[test]
    fn energize_fires_just_sparked_at_max() {
        let mut z = z();
        z.energize(100.0);
        assert!(z.just_sparked);
        assert!(z.is_sparked());
    }

    #[test]
    fn energize_no_just_sparked_when_already_at_max() {
        let mut z = z();
        z.vitality = 100.0;
        z.energize(10.0);
        assert!(!z.just_sparked);
    }

    #[test]
    fn energize_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.energize(50.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn energize_no_op_when_amount_zero() {
        let mut z = z();
        z.energize(0.0);
        assert_eq!(z.vitality, 0.0);
    }

    // --- fizzle ---

    #[test]
    fn fizzle_reduces_vitality() {
        let mut z = z();
        z.vitality = 60.0;
        z.fizzle(20.0);
        assert!((z.vitality - 40.0).abs() < 1e-3);
    }

    #[test]
    fn fizzle_clamps_at_zero() {
        let mut z = z();
        z.vitality = 30.0;
        z.fizzle(200.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn fizzle_fires_just_fizzled_at_zero() {
        let mut z = z();
        z.vitality = 30.0;
        z.fizzle(30.0);
        assert!(z.just_fizzled);
    }

    #[test]
    fn fizzle_no_op_when_already_fizzled() {
        let mut z = z();
        z.fizzle(10.0);
        assert!(!z.just_fizzled);
    }

    #[test]
    fn fizzle_no_op_when_disabled() {
        let mut z = z();
        z.vitality = 50.0;
        z.enabled = false;
        z.fizzle(50.0);
        assert!((z.vitality - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_sparks_vitality() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.vitality - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_sparked_on_vitality_to_max() {
        let mut z = Zappy::new(100.0, 200.0);
        z.vitality = 95.0;
        z.tick(1.0);
        assert!(z.just_sparked);
        assert!(z.is_sparked());
    }

    #[test]
    fn tick_no_spark_when_already_sparked() {
        let mut z = z();
        z.vitality = 100.0;
        z.tick(1.0);
        assert!(!z.just_sparked);
    }

    #[test]
    fn tick_no_spark_when_rate_zero() {
        let mut z = Zappy::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn tick_no_spark_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.vitality, 0.0);
    }

    #[test]
    fn tick_clears_just_sparked() {
        let mut z = Zappy::new(100.0, 200.0);
        z.vitality = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_sparked);
    }

    #[test]
    fn tick_clears_just_fizzled() {
        let mut z = z();
        z.vitality = 10.0;
        z.fizzle(10.0);
        z.tick(0.016);
        assert!(!z.just_fizzled);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.vitality - 9.0).abs() < 1e-3);
    }

    // --- is_sparked / is_fizzled ---

    #[test]
    fn is_sparked_false_when_disabled() {
        let mut z = z();
        z.vitality = 100.0;
        z.enabled = false;
        assert!(!z.is_sparked());
    }

    #[test]
    fn is_fizzled_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_fizzled());
    }

    // --- vitality_fraction / effective_zest ---

    #[test]
    fn vitality_fraction_zero_when_fizzled() {
        assert_eq!(z().vitality_fraction(), 0.0);
    }

    #[test]
    fn vitality_fraction_half_at_midpoint() {
        let mut z = z();
        z.vitality = 50.0;
        assert!((z.vitality_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_zest_zero_when_fizzled() {
        assert_eq!(z().effective_zest(100.0), 0.0);
    }

    #[test]
    fn effective_zest_scales_with_vitality() {
        let mut z = z();
        z.vitality = 75.0;
        assert!((z.effective_zest(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_zest_zero_when_disabled() {
        let mut z = z();
        z.vitality = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_zest(100.0), 0.0);
    }
}

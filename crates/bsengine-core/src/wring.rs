use bevy_ecs::prelude::Component;

/// Compression-squeeze accumulation tracker named after wring,
/// the verb meaning to squeeze or twist so as to extract liquid;
/// to compress or squeeze tightly; to distress or torment — from
/// the Old English wringan (to press, to squeeze, to compress),
/// from the Proto-Germanic wrangjaną (to press), from the Proto-
/// Indo-European root wrengh- (to turn, to twist), related to
/// wrench, wrist, and wrong (originally "twisted, crooked").
/// The same root gave the Old Norse rengja (to turn aside, to
/// buckle), the Middle Dutch wringen (to wring, to squeeze),
/// and the modern Dutch wringen and German ringen (to wrestle,
/// to struggle — to engage in mutual compression). To wring
/// a cloth is to compress and twist it in opposing directions
/// until no more liquid remains; to wring a neck is to apply
/// the same rotational compression to a more destructive end.
/// The metaphorical extensions — to wring one's hands in grief,
/// to wring a confession from a prisoner, to wring every last
/// advantage from a situation — preserve the sense of applying
/// maximum compression in order to extract what would otherwise
/// remain held inside. In game mechanics, a wring mechanic
/// models the slow build of compressive force — the accumulation
/// of pressure, squeeze, or torsion that eventually reaches
/// the threshold at which liquid is extracted, a material
/// yields, a confession is given, or a structure collapses
/// under sustained compression. `pressure` builds via
/// `squeeze(amount)` and accumulates passively at `press_rate`
/// per second in `tick(dt)` or releases via `ease(amount)`.
///
/// Models compression-pressure fill levels, squeeze-saturation
/// bars, torsion-accumulation trackers, pressure-build gauges,
/// constriction fill levels, grip-saturation indicators,
/// crush-accumulation bars, clench meters, wringing-completion
/// fill levels, or any mechanic where a character, device, or
/// force slowly accumulates the compressive energy required
/// to extract a resource, break a material, force an action,
/// or achieve a threshold of maximum compression.
///
/// `squeeze(amount)` adds pressure; fires `just_wrung` when
/// first reaching `max_pressure`. No-op when disabled.
///
/// `ease(amount)` reduces pressure immediately; fires
/// `just_released` when reaching 0. No-op when disabled or
/// already released.
///
/// `tick(dt)` clears both flags, then increases pressure by
/// `press_rate * dt` (capped at `max_pressure`). Fires
/// `just_wrung` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_wrung()` returns `pressure >= max_pressure && enabled`.
///
/// `is_released()` returns `pressure == 0.0` (not gated by
/// `enabled`).
///
/// `pressure_fraction()` returns
/// `(pressure / max_pressure).clamp(0, 1)`.
///
/// `effective_force(scale)` returns `scale * pressure_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — presses at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wring {
    pub pressure: f32,
    pub max_pressure: f32,
    pub press_rate: f32,
    pub just_wrung: bool,
    pub just_released: bool,
    pub enabled: bool,
}

impl Wring {
    pub fn new(max_pressure: f32, press_rate: f32) -> Self {
        Self {
            pressure: 0.0,
            max_pressure: max_pressure.max(0.1),
            press_rate: press_rate.max(0.0),
            just_wrung: false,
            just_released: false,
            enabled: true,
        }
    }

    /// Add pressure; fires `just_wrung` when first reaching max.
    /// No-op when disabled.
    pub fn squeeze(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.pressure < self.max_pressure;
        self.pressure = (self.pressure + amount).min(self.max_pressure);
        if was_below && self.pressure >= self.max_pressure {
            self.just_wrung = true;
        }
    }

    /// Reduce pressure; fires `just_released` when reaching 0.
    /// No-op when disabled or already released.
    pub fn ease(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.pressure <= 0.0 {
            return;
        }
        self.pressure = (self.pressure - amount).max(0.0);
        if self.pressure <= 0.0 {
            self.just_released = true;
        }
    }

    /// Clear flags, then increase pressure by `press_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_wrung = false;
        self.just_released = false;
        if self.enabled && self.press_rate > 0.0 && self.pressure < self.max_pressure {
            let was_below = self.pressure < self.max_pressure;
            self.pressure = (self.pressure + self.press_rate * dt).min(self.max_pressure);
            if was_below && self.pressure >= self.max_pressure {
                self.just_wrung = true;
            }
        }
    }

    /// `true` when pressure is at maximum and component is enabled.
    pub fn is_wrung(&self) -> bool {
        self.pressure >= self.max_pressure && self.enabled
    }

    /// `true` when pressure is 0 (not gated by `enabled`).
    pub fn is_released(&self) -> bool {
        self.pressure == 0.0
    }

    /// Fraction of maximum pressure [0.0, 1.0].
    pub fn pressure_fraction(&self) -> f32 {
        (self.pressure / self.max_pressure).clamp(0.0, 1.0)
    }

    /// Returns `scale * pressure_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_force(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.pressure_fraction()
    }
}

impl Default for Wring {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wring {
        Wring::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_released() {
        let w = w();
        assert_eq!(w.pressure, 0.0);
        assert!(w.is_released());
        assert!(!w.is_wrung());
    }

    #[test]
    fn new_clamps_max_pressure() {
        let w = Wring::new(-5.0, 1.5);
        assert!((w.max_pressure - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_press_rate() {
        let w = Wring::new(100.0, -1.5);
        assert_eq!(w.press_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wring::default();
        assert!((w.max_pressure - 100.0).abs() < 1e-5);
        assert!((w.press_rate - 1.5).abs() < 1e-5);
    }

    // --- squeeze ---

    #[test]
    fn squeeze_adds_pressure() {
        let mut w = w();
        w.squeeze(40.0);
        assert!((w.pressure - 40.0).abs() < 1e-3);
    }

    #[test]
    fn squeeze_clamps_at_max() {
        let mut w = w();
        w.squeeze(200.0);
        assert!((w.pressure - 100.0).abs() < 1e-3);
    }

    #[test]
    fn squeeze_fires_just_wrung_at_max() {
        let mut w = w();
        w.squeeze(100.0);
        assert!(w.just_wrung);
        assert!(w.is_wrung());
    }

    #[test]
    fn squeeze_no_just_wrung_when_already_at_max() {
        let mut w = w();
        w.pressure = 100.0;
        w.squeeze(10.0);
        assert!(!w.just_wrung);
    }

    #[test]
    fn squeeze_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.squeeze(50.0);
        assert_eq!(w.pressure, 0.0);
    }

    #[test]
    fn squeeze_no_op_when_amount_zero() {
        let mut w = w();
        w.squeeze(0.0);
        assert_eq!(w.pressure, 0.0);
    }

    // --- ease ---

    #[test]
    fn ease_reduces_pressure() {
        let mut w = w();
        w.pressure = 60.0;
        w.ease(20.0);
        assert!((w.pressure - 40.0).abs() < 1e-3);
    }

    #[test]
    fn ease_clamps_at_zero() {
        let mut w = w();
        w.pressure = 30.0;
        w.ease(200.0);
        assert_eq!(w.pressure, 0.0);
    }

    #[test]
    fn ease_fires_just_released_at_zero() {
        let mut w = w();
        w.pressure = 30.0;
        w.ease(30.0);
        assert!(w.just_released);
    }

    #[test]
    fn ease_no_op_when_already_released() {
        let mut w = w();
        w.ease(10.0);
        assert!(!w.just_released);
    }

    #[test]
    fn ease_no_op_when_disabled() {
        let mut w = w();
        w.pressure = 50.0;
        w.enabled = false;
        w.ease(50.0);
        assert!((w.pressure - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_pressure() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.pressure - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_wrung_on_pressure_to_max() {
        let mut w = Wring::new(100.0, 200.0);
        w.pressure = 95.0;
        w.tick(1.0);
        assert!(w.just_wrung);
        assert!(w.is_wrung());
    }

    #[test]
    fn tick_no_build_when_already_wrung() {
        let mut w = w();
        w.pressure = 100.0;
        w.tick(1.0);
        assert!(!w.just_wrung);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wring::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.pressure, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.pressure, 0.0);
    }

    #[test]
    fn tick_clears_just_wrung() {
        let mut w = Wring::new(100.0, 200.0);
        w.pressure = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_wrung);
    }

    #[test]
    fn tick_clears_just_released() {
        let mut w = w();
        w.pressure = 10.0;
        w.ease(10.0);
        w.tick(0.016);
        assert!(!w.just_released);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.pressure - 9.0).abs() < 1e-3);
    }

    // --- is_wrung / is_released ---

    #[test]
    fn is_wrung_false_when_disabled() {
        let mut w = w();
        w.pressure = 100.0;
        w.enabled = false;
        assert!(!w.is_wrung());
    }

    #[test]
    fn is_released_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_released());
    }

    // --- pressure_fraction / effective_force ---

    #[test]
    fn pressure_fraction_zero_when_released() {
        assert_eq!(w().pressure_fraction(), 0.0);
    }

    #[test]
    fn pressure_fraction_half_at_midpoint() {
        let mut w = w();
        w.pressure = 50.0;
        assert!((w.pressure_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_force_zero_when_released() {
        assert_eq!(w().effective_force(100.0), 0.0);
    }

    #[test]
    fn effective_force_scales_with_pressure() {
        let mut w = w();
        w.pressure = 75.0;
        assert!((w.effective_force(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_force_zero_when_disabled() {
        let mut w = w();
        w.pressure = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_force(100.0), 0.0);
    }
}

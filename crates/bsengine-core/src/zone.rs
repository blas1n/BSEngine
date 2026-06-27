use bevy_ecs::prelude::Component;

/// Territory-control accumulator driven by an external holding flag. Each
/// tick, if `is_holding` is `true`, `zone_control` advances toward
/// `max_zone` at `advance_rate`; if `false`, it decays toward 0 at
/// `decay_rate`. The game system sets `is_holding` before calling `tick`.
///
/// Fires `just_captured` the first time `zone_control` reaches `max_zone`.
/// Fires `just_lost` the first time `zone_control` falls back to 0 after
/// being > 0. Both flags are one-frame only.
///
/// Unlike all other accumulating components, Zone has **two conditional
/// rates** in a single `tick()` call, selected by an external boolean
/// (`is_holding`) rather than event-driven methods. This models capture
/// points, territory nodes, or presence-dependent buffs where the entity's
/// physical location — set by a higher-level system — determines direction.
///
/// `is_contested()` returns `zone_control > 0.0 && zone_control < max_zone && enabled`.
///
/// `is_captured()` returns `zone_control >= max_zone && enabled`.
///
/// `zone_fraction()` returns `(zone_control / max_zone).clamp(0.0, 1.0)`.
///
/// `effective_control(base)` returns `base * zone_fraction()` when enabled
/// — 0 when empty, `base` when fully captured; `base` unchanged when
/// disabled.
///
/// Default: `new(10.0, 1.0, 1.0)` — max=10, advance 1/s, decay 1/s.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zone {
    /// Current control level [0, max_zone].
    pub zone_control: f32,
    /// Full capture threshold. Clamped >= 1.0.
    pub max_zone: f32,
    /// Control gain per second while holding. Clamped >= 0.0.
    pub advance_rate: f32,
    /// Control loss per second while not holding. Clamped >= 0.0.
    pub decay_rate: f32,
    /// Set by the game system each frame: `true` if entity is holding the zone.
    pub is_holding: bool,
    pub just_captured: bool,
    pub just_lost: bool,
    pub enabled: bool,
}

impl Zone {
    pub fn new(max_zone: f32, advance_rate: f32, decay_rate: f32) -> Self {
        Self {
            zone_control: 0.0,
            max_zone: max_zone.max(1.0),
            advance_rate: advance_rate.max(0.0),
            decay_rate: decay_rate.max(0.0),
            is_holding: false,
            just_captured: false,
            just_lost: false,
            enabled: true,
        }
    }

    /// Advance one frame. Clears flags, then:
    /// - if `is_holding`: grows toward `max_zone`; fires `just_captured` at
    ///   first reach
    /// - if `!is_holding`: decays toward 0; fires `just_lost` on first reach
    ///   from above 0
    /// No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_captured = false;
        self.just_lost = false;

        if !self.enabled {
            return;
        }

        if self.is_holding {
            let prev = self.zone_control;
            self.zone_control = (self.zone_control + self.advance_rate * dt).min(self.max_zone);
            if prev < self.max_zone && self.zone_control >= self.max_zone {
                self.just_captured = true;
            }
        } else {
            let prev = self.zone_control;
            self.zone_control = (self.zone_control - self.decay_rate * dt).max(0.0);
            if prev > 0.0 && self.zone_control == 0.0 {
                self.just_lost = true;
            }
        }
    }

    /// `true` when control is partially filled (not empty, not full) and
    /// component is enabled.
    pub fn is_contested(&self) -> bool {
        self.zone_control > 0.0 && self.zone_control < self.max_zone && self.enabled
    }

    /// `true` when the zone is fully captured and component is enabled.
    pub fn is_captured(&self) -> bool {
        self.zone_control >= self.max_zone && self.enabled
    }

    /// Control as a fraction of maximum [0.0, 1.0].
    pub fn zone_fraction(&self) -> f32 {
        (self.zone_control / self.max_zone).clamp(0.0, 1.0)
    }

    /// Scale `base` by zone control. Returns `base * zone_fraction()` when
    /// enabled; `base` when disabled.
    pub fn effective_control(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * self.zone_fraction()
    }
}

impl Default for Zone {
    fn default() -> Self {
        Self::new(10.0, 1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zone {
        Zone::new(10.0, 1.0, 1.0) // max=10, advance/decay=1/s
    }

    // --- construction ---

    #[test]
    fn new_starts_empty_and_not_holding() {
        let z = z();
        assert_eq!(z.zone_control, 0.0);
        assert!(!z.is_holding);
        assert!(!z.just_captured);
        assert!(!z.just_lost);
        assert!(!z.is_captured());
        assert!(!z.is_contested());
    }

    #[test]
    fn max_zone_clamped_to_one() {
        let z = Zone::new(0.0, 1.0, 1.0);
        assert!((z.max_zone - 1.0).abs() < 1e-5);
    }

    #[test]
    fn advance_rate_clamped_to_zero() {
        let z = Zone::new(10.0, -1.0, 1.0);
        assert_eq!(z.advance_rate, 0.0);
    }

    #[test]
    fn decay_rate_clamped_to_zero() {
        let z = Zone::new(10.0, 1.0, -1.0);
        assert_eq!(z.decay_rate, 0.0);
    }

    // --- tick holding ---

    #[test]
    fn tick_advances_when_holding() {
        let mut z = z();
        z.is_holding = true;
        z.tick(3.0); // 0+3=3
        assert!((z.zone_control - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_clamps_control_at_max_when_holding() {
        let mut z = z(); // max=10
        z.is_holding = true;
        z.tick(20.0); // clamped to 10
        assert!((z.zone_control - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_fires_just_captured_on_reaching_max() {
        let mut z = z(); // max=10
        z.is_holding = true;
        z.tick(10.0); // exactly hits max
        assert!(z.just_captured);
        assert!(z.is_captured());
    }

    #[test]
    fn tick_fires_just_captured_crossing_max() {
        let mut z = z();
        z.is_holding = true;
        z.tick(7.0); // 7.0
        z.tick(5.0); // crosses 10.0
        assert!(z.just_captured);
    }

    #[test]
    fn tick_just_captured_clears_next_frame() {
        let mut z = z();
        z.is_holding = true;
        z.tick(10.0); // just_captured=true
        z.tick(0.016);
        assert!(!z.just_captured);
    }

    #[test]
    fn tick_does_not_refire_just_captured_when_already_full() {
        let mut z = z();
        z.is_holding = true;
        z.tick(10.0); // captured
        z.tick(1.0); // no re-fire
        assert!(!z.just_captured);
    }

    // --- tick not holding ---

    #[test]
    fn tick_decays_when_not_holding() {
        let mut z = z();
        z.is_holding = true;
        z.tick(8.0); // 8.0
        z.is_holding = false;
        z.tick(3.0); // 5.0
        assert!((z.zone_control - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_control_at_zero_when_decaying() {
        let mut z = z();
        z.is_holding = true;
        z.tick(3.0); // 3.0
        z.is_holding = false;
        z.tick(10.0); // clamped to 0
        assert_eq!(z.zone_control, 0.0);
    }

    #[test]
    fn tick_fires_just_lost_on_decaying_to_zero() {
        let mut z = z();
        z.is_holding = true;
        z.tick(3.0); // 3.0
        z.is_holding = false;
        z.tick(3.0); // exactly 0
        assert!(z.just_lost);
    }

    #[test]
    fn tick_fires_just_lost_crossing_zero() {
        let mut z = z();
        z.is_holding = true;
        z.tick(5.0); // 5.0
        z.is_holding = false;
        z.tick(10.0); // crosses 0
        assert!(z.just_lost);
    }

    #[test]
    fn tick_just_lost_clears_next_frame() {
        let mut z = z();
        z.is_holding = true;
        z.tick(3.0);
        z.is_holding = false;
        z.tick(3.0); // just_lost=true
        z.tick(0.016);
        assert!(!z.just_lost);
    }

    #[test]
    fn tick_no_just_lost_when_already_empty() {
        let mut z = z(); // control=0
        z.is_holding = false;
        z.tick(1.0); // nothing to lose
        assert!(!z.just_lost);
    }

    // --- tick disabled ---

    #[test]
    fn tick_no_op_when_disabled() {
        let mut z = z();
        z.is_holding = true;
        z.enabled = false;
        z.tick(5.0);
        assert_eq!(z.zone_control, 0.0);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut z = z();
        z.just_captured = true;
        z.just_lost = true;
        z.enabled = false;
        z.tick(0.016);
        assert!(!z.just_captured);
        assert!(!z.just_lost);
    }

    // --- is_contested / is_captured ---

    #[test]
    fn is_contested_false_when_empty() {
        let z = z();
        assert!(!z.is_contested());
    }

    #[test]
    fn is_contested_true_when_partial() {
        let mut z = z();
        z.is_holding = true;
        z.tick(5.0);
        assert!(z.is_contested());
    }

    #[test]
    fn is_contested_false_when_full() {
        let mut z = z();
        z.is_holding = true;
        z.tick(10.0);
        assert!(!z.is_contested());
        assert!(z.is_captured());
    }

    #[test]
    fn is_contested_false_when_disabled() {
        let mut z = z();
        z.is_holding = true;
        z.tick(5.0);
        z.enabled = false;
        assert!(!z.is_contested());
    }

    #[test]
    fn is_captured_false_when_partial() {
        let mut z = z();
        z.is_holding = true;
        z.tick(5.0);
        assert!(!z.is_captured());
    }

    #[test]
    fn is_captured_true_at_max() {
        let mut z = z();
        z.is_holding = true;
        z.tick(10.0);
        assert!(z.is_captured());
    }

    #[test]
    fn is_captured_false_when_disabled() {
        let mut z = z();
        z.is_holding = true;
        z.tick(10.0);
        z.enabled = false;
        assert!(!z.is_captured());
    }

    // --- zone_fraction ---

    #[test]
    fn zone_fraction_zero_when_empty() {
        let z = z();
        assert_eq!(z.zone_fraction(), 0.0);
    }

    #[test]
    fn zone_fraction_at_half() {
        let mut z = z(); // max=10
        z.is_holding = true;
        z.tick(5.0); // 5/10=0.5
        assert!((z.zone_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn zone_fraction_one_at_max() {
        let mut z = z();
        z.is_holding = true;
        z.tick(10.0);
        assert!((z.zone_fraction() - 1.0).abs() < 1e-4);
    }

    // --- effective_control ---

    #[test]
    fn effective_control_zero_when_empty() {
        let z = z(); // fraction=0 → 100*0=0
        assert!((z.effective_control(100.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn effective_control_at_half_capture() {
        let mut z = z();
        z.is_holding = true;
        z.tick(5.0); // fraction=0.5 → 100*0.5=50
        assert!((z.effective_control(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_control_full_at_max() {
        let mut z = z();
        z.is_holding = true;
        z.tick(10.0); // fraction=1.0 → 100*1=100
        assert!((z.effective_control(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_control_passthrough_when_disabled() {
        let z = {
            let mut z = z();
            z.enabled = false;
            z
        };
        assert!((z.effective_control(100.0) - 100.0).abs() < 1e-4);
    }

    // --- hold / release cycle ---

    #[test]
    fn capture_then_lose_cycle() {
        let mut z = z();
        z.is_holding = true;
        z.tick(10.0); // captured
        assert!(z.just_captured);
        z.is_holding = false;
        z.tick(10.0); // lost
        assert!(z.just_lost);
        assert_eq!(z.zone_control, 0.0);
    }

    #[test]
    fn asymmetric_rates() {
        let mut z = Zone::new(10.0, 2.0, 0.5); // advances 2×, decays 0.5×
        z.is_holding = true;
        z.tick(5.0); // 0+2*5=10 → captured
        assert!(z.is_captured());
        z.is_holding = false;
        z.tick(4.0); // 10-0.5*4=8
        assert!((z.zone_control - 8.0).abs() < 1e-4);
    }
}

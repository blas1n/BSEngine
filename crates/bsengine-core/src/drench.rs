use bevy_ecs::prelude::Component;

/// Water-soaked debuff that amplifies incoming lightning damage and suppresses
/// fire damage dealt by or received by the entity.
///
/// Apply with `soak(duration)` (high-watermark). While drenched:
/// - `incoming_lightning(base)` returns `base * lightning_amplify` — lightning
///   hits harder against wet targets.
/// - `outgoing_fire(base)` returns `base * (1 - fire_suppress_fraction)` —
///   the entity's own fire attacks are weakened by the moisture.
///
/// `tick(dt)` counts down and sets `just_dried` when the effect expires.
///
/// Distinct from `Douse` (instantly extinguishes active fire effects/stacks),
/// `Freeze` (immobilizes by ice), and `Frostbite` (cold damage-over-time):
/// Drench is a sustained elemental vulnerability state — it doesn't stop fire
/// directly, it just makes the entity conduct electricity better and throw fire
/// less effectively while wet.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Drench {
    pub duration: f32,
    pub timer: f32,
    /// Multiplier on incoming lightning damage while drenched. Clamped ≥ 1.0.
    /// e.g. 1.5 = 50% more lightning damage taken.
    pub lightning_amplify: f32,
    /// Fraction of outgoing fire damage suppressed while drenched.
    /// Clamped [0.0, 1.0]. e.g. 0.5 = entity deals 50% of normal fire damage.
    pub fire_suppress_fraction: f32,
    pub just_drenched: bool,
    pub just_dried: bool,
    pub enabled: bool,
}

impl Drench {
    pub fn new(lightning_amplify: f32, fire_suppress_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            lightning_amplify: lightning_amplify.max(1.0),
            fire_suppress_fraction: fire_suppress_fraction.clamp(0.0, 1.0),
            just_drenched: false,
            just_dried: false,
            enabled: true,
        }
    }

    /// Apply or extend the drench for `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer. No-op when
    /// disabled.
    pub fn soak(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        if duration > self.timer {
            let was_active = self.is_drenched();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_drenched = true;
            }
        }
    }

    /// End the drench immediately (e.g., entity enters a fire zone and dries
    /// out). Sets `just_dried`.
    pub fn clear(&mut self) {
        if self.is_drenched() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_dried = true;
        }
    }

    /// Advance the timer; sets `just_dried` when the drench expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_drenched = false;
        self.just_dried = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_dried = true;
            }
        }
    }

    pub fn is_drenched(&self) -> bool {
        self.timer > 0.0
    }

    /// Incoming lightning damage after the drench amplification.
    /// Returns `base * lightning_amplify` while drenched, `base` otherwise.
    pub fn incoming_lightning(&self, base: f32) -> f32 {
        if self.is_drenched() && self.enabled {
            base * self.lightning_amplify
        } else {
            base
        }
    }

    /// Outgoing fire damage after drench suppression.
    /// Returns `base * (1 - fire_suppress_fraction)` while drenched, `base` otherwise.
    pub fn outgoing_fire(&self, base: f32) -> f32 {
        if self.is_drenched() && self.enabled {
            base * (1.0 - self.fire_suppress_fraction)
        } else {
            base
        }
    }

    /// Fraction of the drench duration remaining [1.0 = just soaked, 0.0 = dried].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Drench {
    fn default() -> Self {
        Self::new(1.5, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soak_activates_drench() {
        let mut d = Drench::new(1.5, 0.5);
        d.soak(5.0);
        assert!(d.is_drenched());
        assert!(d.just_drenched);
    }

    #[test]
    fn soak_extends_on_longer_duration() {
        let mut d = Drench::new(1.5, 0.5);
        d.soak(3.0);
        d.tick(0.016);
        d.soak(8.0);
        assert!((d.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn soak_no_extend_on_shorter_duration() {
        let mut d = Drench::new(1.5, 0.5);
        d.soak(8.0);
        d.soak(3.0);
        assert!((d.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn just_drenched_not_set_on_extend() {
        let mut d = Drench::new(1.5, 0.5);
        d.soak(3.0);
        d.tick(0.016);
        d.soak(8.0); // extend while already drenched
        assert!(!d.just_drenched);
    }

    #[test]
    fn clear_ends_drench() {
        let mut d = Drench::new(1.5, 0.5);
        d.soak(5.0);
        d.clear();
        assert!(!d.is_drenched());
        assert!(d.just_dried);
    }

    #[test]
    fn tick_expires_drench() {
        let mut d = Drench::new(1.5, 0.5);
        d.soak(1.0);
        d.tick(1.1);
        assert!(!d.is_drenched());
        assert!(d.just_dried);
    }

    #[test]
    fn tick_clears_just_drenched() {
        let mut d = Drench::new(1.5, 0.5);
        d.soak(5.0);
        d.tick(0.016);
        assert!(!d.just_drenched);
    }

    #[test]
    fn incoming_lightning_amplified_while_drenched() {
        let mut d = Drench::new(2.0, 0.5);
        d.soak(5.0);
        assert!((d.incoming_lightning(10.0) - 20.0).abs() < 1e-4);
    }

    #[test]
    fn incoming_lightning_unaffected_when_dry() {
        let d = Drench::new(2.0, 0.5);
        assert!((d.incoming_lightning(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn outgoing_fire_suppressed_while_drenched() {
        let mut d = Drench::new(1.5, 0.5);
        d.soak(5.0);
        assert!((d.outgoing_fire(10.0) - 5.0).abs() < 1e-4);
    }

    #[test]
    fn outgoing_fire_unaffected_when_dry() {
        let d = Drench::new(1.5, 0.5);
        assert!((d.outgoing_fire(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut d = Drench::new(1.5, 0.5);
        d.soak(2.0);
        d.tick(1.0);
        assert!((d.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_soak_no_op() {
        let mut d = Drench::new(1.5, 0.5);
        d.enabled = false;
        d.soak(5.0);
        assert!(!d.is_drenched());
    }

    #[test]
    fn disabled_incoming_lightning_unaffected() {
        let mut d = Drench::new(2.0, 0.5);
        d.soak(5.0);
        d.enabled = false;
        assert!((d.incoming_lightning(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_outgoing_fire_unaffected() {
        let mut d = Drench::new(1.5, 0.5);
        d.soak(5.0);
        d.enabled = false;
        assert!((d.outgoing_fire(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn lightning_amplify_clamped_to_one() {
        let d = Drench::new(0.5, 0.5); // < 1.0 → clamped to 1.0
        assert!((d.lightning_amplify - 1.0).abs() < 1e-5);
    }
}

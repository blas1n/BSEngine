use bevy_ecs::prelude::Component;

/// Vulnerability debuff that strips defensive layers and amplifies all
/// incoming damage by `damage_multiplier` (>= 1.0).
///
/// While exposed, the damage pipeline calls `incoming_damage(raw)` to scale
/// up all damage before applying resistance or armor reductions. Exposing an
/// entity makes every follow-up attack hit harder.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_recovered` on expiry. `clear()` removes the debuff early.
///
/// Distinct from `Fracture` (structural + move penalty), `Crumble` (decaying
/// defense over time), and `ShieldBreak` (shield-specific reduction): Expose
/// is a general, flat incoming-damage amplifier that applies to all damage
/// types regardless of the defender's armor or resistances.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Expose {
    pub duration: f32,
    pub timer: f32,
    /// Multiplier applied to ALL incoming damage while exposed (>= 1.0).
    pub damage_multiplier: f32,
    pub just_exposed: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Expose {
    pub fn new(damage_multiplier: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            damage_multiplier: damage_multiplier.max(1.0),
            just_exposed: false,
            just_recovered: false,
            enabled: true,
        }
    }

    /// Apply or extend the expose for `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_exposed = true;
            }
        }
    }

    /// Remove the expose immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_recovered = true;
        }
    }

    /// Advance the timer; sets `just_recovered` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_exposed = false;
        self.just_recovered = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_recovered = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Scale incoming raw damage by the exposure multiplier.
    /// Returns `raw * damage_multiplier` while active, `raw` otherwise.
    pub fn incoming_damage(&self, raw: f32) -> f32 {
        if self.is_active() {
            raw * self.damage_multiplier
        } else {
            raw
        }
    }

    /// Fraction of the expose duration remaining [1.0 = just applied, 0.0 = recovered].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Expose {
    fn default() -> Self {
        Self::new(1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_expose() {
        let mut e = Expose::new(1.5);
        e.apply(3.0);
        assert!(e.is_active());
        assert!(e.just_exposed);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut e = Expose::new(1.5);
        e.apply(2.0);
        e.tick(0.016);
        e.apply(5.0);
        assert!((e.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut e = Expose::new(1.5);
        e.apply(5.0);
        e.apply(2.0);
        assert!((e.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_expose() {
        let mut e = Expose::new(1.5);
        e.apply(1.0);
        e.tick(1.1);
        assert!(!e.is_active());
        assert!(e.just_recovered);
    }

    #[test]
    fn clear_ends_early() {
        let mut e = Expose::new(1.5);
        e.apply(5.0);
        e.clear();
        assert!(!e.is_active());
        assert!(e.just_recovered);
    }

    #[test]
    fn incoming_damage_amplified_while_active() {
        let mut e = Expose::new(2.0);
        e.apply(3.0);
        assert!((e.incoming_damage(100.0) - 200.0).abs() < 1e-4);
    }

    #[test]
    fn incoming_damage_passthrough_when_inactive() {
        let e = Expose::new(2.0);
        assert!((e.incoming_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut e = Expose::new(1.5);
        e.apply(2.0);
        e.tick(1.0);
        assert!((e.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut e = Expose::new(1.5);
        e.enabled = false;
        e.apply(5.0);
        assert!(!e.is_active());
    }

    #[test]
    fn tick_clears_just_exposed() {
        let mut e = Expose::new(1.5);
        e.apply(3.0);
        e.tick(0.016);
        assert!(!e.just_exposed);
    }

    #[test]
    fn damage_multiplier_clamped_to_min_one() {
        let e = Expose::new(0.5);
        assert!((e.damage_multiplier - 1.0).abs() < 1e-5);
    }
}

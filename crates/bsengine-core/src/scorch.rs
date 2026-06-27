use bevy_ecs::prelude::Component;

/// Residual heat state after fire exposure: the entity is charred, amplifying
/// incoming fire damage and applying a slow health drain until the scorch fades.
///
/// Apply with `apply(duration)` (high-watermark). While scorched:
/// - `incoming_fire(base)` returns `base * fire_amplify` — charred flesh
///   catches fire more readily.
/// - `damage_per_tick(dt)` returns `dot_rate * dt` — smoldering heat continues
///   to wear the entity down.
///
/// `tick(dt)` counts down and sets `just_healed` when the effect expires.
///
/// Distinct from `Burn` (active fire damage-over-time applied while on fire),
/// `Blaze` (fire aura that ignites others), and `Ignite` (sets a target on
/// fire): Scorch is the aftermath — the entity is no longer actively burning
/// but remains heat-damaged and vulnerable to reignition.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Scorch {
    pub duration: f32,
    pub timer: f32,
    /// Multiplier on incoming fire damage while scorched. Clamped ≥ 1.0.
    /// e.g. 1.5 = 50% more fire damage taken.
    pub fire_amplify: f32,
    /// Health drained per second while scorched. Clamped ≥ 0.0.
    pub dot_rate: f32,
    pub just_scorched: bool,
    pub just_healed: bool,
    pub enabled: bool,
}

impl Scorch {
    pub fn new(fire_amplify: f32, dot_rate: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            fire_amplify: fire_amplify.max(1.0),
            dot_rate: dot_rate.max(0.0),
            just_scorched: false,
            just_healed: false,
            enabled: true,
        }
    }

    /// Apply or extend the scorch for `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer. No-op when
    /// disabled.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        if duration > self.timer {
            let was_active = self.is_scorched();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_scorched = true;
            }
        }
    }

    /// End the scorch immediately (e.g., entity is doused). Sets `just_healed`.
    pub fn clear(&mut self) {
        if self.is_scorched() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_healed = true;
        }
    }

    /// Advance the timer; sets `just_healed` when the scorch expires naturally.
    pub fn tick(&mut self, dt: f32) {
        self.just_scorched = false;
        self.just_healed = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_healed = true;
            }
        }
    }

    pub fn is_scorched(&self) -> bool {
        self.timer > 0.0
    }

    /// Incoming fire damage after the scorch amplification.
    /// Returns `base * fire_amplify` while scorched and enabled, `base` otherwise.
    pub fn incoming_fire(&self, base: f32) -> f32 {
        if self.is_scorched() && self.enabled {
            base * self.fire_amplify
        } else {
            base
        }
    }

    /// Health drained this frame from smoldering heat (`dot_rate * dt`).
    /// Returns `0.0` when not scorched or disabled.
    pub fn damage_per_tick(&self, dt: f32) -> f32 {
        if self.is_scorched() && self.enabled {
            self.dot_rate * dt
        } else {
            0.0
        }
    }

    /// Fraction of the scorch duration remaining [1.0 = just applied, 0.0 = healed].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Scorch {
    fn default() -> Self {
        Self::new(1.5, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_scorch() {
        let mut s = Scorch::new(1.5, 2.0);
        s.apply(5.0);
        assert!(s.is_scorched());
        assert!(s.just_scorched);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut s = Scorch::new(1.5, 2.0);
        s.apply(3.0);
        s.tick(0.016);
        s.apply(8.0);
        assert!((s.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut s = Scorch::new(1.5, 2.0);
        s.apply(8.0);
        s.apply(3.0);
        assert!((s.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn just_scorched_not_set_on_extend() {
        let mut s = Scorch::new(1.5, 2.0);
        s.apply(3.0);
        s.tick(0.016);
        s.apply(8.0);
        assert!(!s.just_scorched);
    }

    #[test]
    fn clear_ends_scorch() {
        let mut s = Scorch::new(1.5, 2.0);
        s.apply(5.0);
        s.clear();
        assert!(!s.is_scorched());
        assert!(s.just_healed);
    }

    #[test]
    fn tick_expires_scorch() {
        let mut s = Scorch::new(1.5, 2.0);
        s.apply(1.0);
        s.tick(1.1);
        assert!(!s.is_scorched());
        assert!(s.just_healed);
    }

    #[test]
    fn tick_clears_just_scorched() {
        let mut s = Scorch::new(1.5, 2.0);
        s.apply(5.0);
        s.tick(0.016);
        assert!(!s.just_scorched);
    }

    #[test]
    fn incoming_fire_amplified_while_scorched() {
        let mut s = Scorch::new(2.0, 2.0);
        s.apply(5.0);
        assert!((s.incoming_fire(10.0) - 20.0).abs() < 1e-4);
    }

    #[test]
    fn incoming_fire_unaffected_when_not_scorched() {
        let s = Scorch::new(2.0, 2.0);
        assert!((s.incoming_fire(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn damage_per_tick_while_scorched() {
        let mut s = Scorch::new(1.5, 4.0);
        s.apply(5.0);
        assert!((s.damage_per_tick(0.5) - 2.0).abs() < 1e-5); // 4 * 0.5 = 2
    }

    #[test]
    fn damage_per_tick_zero_when_not_scorched() {
        let s = Scorch::new(1.5, 4.0);
        assert!((s.damage_per_tick(0.5) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut s = Scorch::new(1.5, 2.0);
        s.apply(2.0);
        s.tick(1.0);
        assert!((s.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut s = Scorch::new(1.5, 2.0);
        s.enabled = false;
        s.apply(5.0);
        assert!(!s.is_scorched());
    }

    #[test]
    fn disabled_incoming_fire_unaffected() {
        let mut s = Scorch::new(2.0, 2.0);
        s.apply(5.0);
        s.enabled = false;
        assert!((s.incoming_fire(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_damage_per_tick_zero() {
        let mut s = Scorch::new(1.5, 4.0);
        s.apply(5.0);
        s.enabled = false;
        assert!((s.damage_per_tick(0.5) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn fire_amplify_clamped_to_one() {
        let s = Scorch::new(0.5, 2.0); // < 1.0 → clamped to 1.0
        assert!((s.fire_amplify - 1.0).abs() < 1e-5);
    }
}

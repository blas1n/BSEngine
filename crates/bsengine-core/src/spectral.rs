use bevy_ecs::prelude::Component;

/// Phase-shift state: entity temporarily becomes partially incorporeal,
/// reducing incoming damage while the effect lasts.
///
/// Call `enter(duration)` to activate the spectral state (high-watermark).
/// While `is_spectral()`, damage systems should call `incoming_damage(base)`
/// to apply the reduction. `exit()` ends it early; `tick(dt)` counts down
/// and sets `just_solid` when the entity returns to its physical state.
///
/// Distinct from `Ghost` (entity passes through solid physics objects —
/// separate from damage reduction), `Stealth` (enemy can't detect the entity —
/// different from invulnerability), `Invincible` (complete immunity, no
/// reduction), and `Phase` (teleportation): Spectral is a timed
/// **partial incorporeality** — the entity is still visible and targetable
/// but absorbs less damage because its physical form is temporarily diffused.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Spectral {
    pub duration: f32,
    pub timer: f32,
    /// Fraction of incoming damage blocked while spectral. Clamped to [0.0, 1.0].
    /// e.g. 0.5 = only 50% of incoming damage lands.
    pub damage_reduction: f32,
    pub just_spectral: bool,
    pub just_solid: bool,
    pub enabled: bool,
}

impl Spectral {
    pub fn new(damage_reduction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            damage_reduction: damage_reduction.clamp(0.0, 1.0),
            just_spectral: false,
            just_solid: false,
            enabled: true,
        }
    }

    /// Enter or extend the spectral state for `duration` seconds.
    /// High-watermark: only replaces timer if new duration is longer.
    /// No-op when disabled.
    pub fn enter(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        if duration > self.timer {
            let was_active = self.is_spectral();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_spectral = true;
            }
        }
    }

    /// End the spectral state immediately.
    pub fn exit(&mut self) {
        if self.is_spectral() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_solid = true;
        }
    }

    /// Advance the timer; sets `just_solid` when the state expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_spectral = false;
        self.just_solid = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_solid = true;
            }
        }
    }

    pub fn is_spectral(&self) -> bool {
        self.timer > 0.0
    }

    /// Incoming damage after spectral reduction. Returns
    /// `base * (1.0 - damage_reduction)` while spectral and enabled,
    /// `base` otherwise.
    pub fn incoming_damage(&self, base: f32) -> f32 {
        if self.is_spectral() && self.enabled {
            base * (1.0 - self.damage_reduction)
        } else {
            base
        }
    }

    /// Fraction of spectral duration remaining [1.0 = just entered, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Spectral {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_activates_spectral() {
        let mut s = Spectral::new(0.5);
        s.enter(3.0);
        assert!(s.is_spectral());
        assert!(s.just_spectral);
    }

    #[test]
    fn enter_extends_on_longer_duration() {
        let mut s = Spectral::new(0.5);
        s.enter(2.0);
        s.tick(0.016);
        s.enter(5.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn enter_no_extend_on_shorter_duration() {
        let mut s = Spectral::new(0.5);
        s.enter(5.0);
        s.enter(2.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn just_spectral_not_set_on_extend() {
        let mut s = Spectral::new(0.5);
        s.enter(2.0);
        s.tick(0.016);
        s.enter(5.0);
        assert!(!s.just_spectral);
    }

    #[test]
    fn exit_ends_spectral() {
        let mut s = Spectral::new(0.5);
        s.enter(3.0);
        s.exit();
        assert!(!s.is_spectral());
        assert!(s.just_solid);
    }

    #[test]
    fn tick_expires_spectral() {
        let mut s = Spectral::new(0.5);
        s.enter(1.0);
        s.tick(1.1);
        assert!(!s.is_spectral());
        assert!(s.just_solid);
    }

    #[test]
    fn tick_clears_just_spectral() {
        let mut s = Spectral::new(0.5);
        s.enter(3.0);
        s.tick(0.016);
        assert!(!s.just_spectral);
    }

    #[test]
    fn incoming_damage_reduced_while_spectral() {
        let mut s = Spectral::new(0.5);
        s.enter(3.0);
        assert!((s.incoming_damage(100.0) - 50.0).abs() < 1e-3); // 100 * 0.5
    }

    #[test]
    fn incoming_damage_full_when_not_spectral() {
        let s = Spectral::new(0.5);
        assert!((s.incoming_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut s = Spectral::new(0.5);
        s.enter(4.0);
        s.tick(2.0);
        assert!((s.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn damage_reduction_clamped() {
        let s = Spectral::new(1.5); // > 1.0 → 1.0
        assert!((s.damage_reduction - 1.0).abs() < 1e-5);
        let s2 = Spectral::new(-0.5); // < 0.0 → 0.0
        assert!((s2.damage_reduction - 0.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_enter_no_op() {
        let mut s = Spectral::new(0.5);
        s.enabled = false;
        s.enter(3.0);
        assert!(!s.is_spectral());
    }

    #[test]
    fn disabled_incoming_damage_unaffected() {
        let mut s = Spectral::new(0.5);
        s.enter(3.0);
        s.enabled = false;
        assert!((s.incoming_damage(100.0) - 100.0).abs() < 1e-5);
    }
}

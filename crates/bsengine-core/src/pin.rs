use bevy_ecs::prelude::Component;

/// Hard-CC anchor: while pinned the entity cannot move and, when
/// `knockback_immune` is set, is also immune to knockback and displacement.
/// Movement and physics systems should gate locomotion on `!is_pinned()` and
/// skip knockback application when `knockback_immune && is_pinned()`.
///
/// `pin(duration)` starts or extends the pin using a high-watermark — only
/// the longer of the current remaining time and the new duration takes effect.
/// Sets `just_pinned` on the inactive → active transition. `free()` ends the
/// pin early and sets `just_freed`. `tick(dt)` counts down and sets
/// `just_freed` on natural expiry.
///
/// `pin(duration)` is a no-op when disabled or `duration ≤ 0`.
///
/// Distinct from `Snare` (speed reduction), `Entangle` (vine-style soft CC
/// with damage), and `Stun` (also interrupts actions): Pin is a **hard-CC
/// anchor** — only movement is locked; the entity may still act (attack, cast)
/// unless combined with other CC. The `knockback_immune` flag is an optional
/// addendum that prevents displacement during the pin window.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Pin {
    pub active: bool,
    /// Remaining seconds of the current pin. Counts down to 0.
    pub timer: f32,
    /// Original duration of the current application. Used for fraction.
    pub duration: f32,
    /// When `true`, knockback and displacement are ignored while pinned.
    pub knockback_immune: bool,
    pub just_pinned: bool,
    pub just_freed: bool,
    pub enabled: bool,
}

impl Pin {
    pub fn new(knockback_immune: bool) -> Self {
        Self {
            active: false,
            timer: 0.0,
            duration: 0.0,
            knockback_immune,
            just_pinned: false,
            just_freed: false,
            enabled: true,
        }
    }

    /// Apply (or extend) pin for `duration` seconds. High-watermark: only
    /// replaces the remaining timer when `duration > timer`. Sets `just_pinned`
    /// on the inactive → active transition. No-op when disabled or
    /// `duration ≤ 0`.
    pub fn pin(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        let was_pinned = self.is_pinned();
        if duration > self.timer {
            self.timer = duration;
            self.duration = duration;
        }
        if !was_pinned {
            self.active = true;
            self.just_pinned = true;
        }
    }

    /// Release the pin early. Sets `just_freed`. No-op when not pinned.
    pub fn free(&mut self) {
        if !self.is_pinned() {
            return;
        }
        self.active = false;
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_freed = true;
    }

    /// Advance the pin timer by `dt` seconds. Sets `just_freed` when the pin
    /// expires naturally. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_pinned = false;
        self.just_freed = false;

        if self.active && self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.active = false;
                self.just_freed = true;
            }
        }
    }

    /// `true` while the entity is currently pinned.
    pub fn is_pinned(&self) -> bool {
        self.active && self.timer > 0.0
    }

    /// `true` when the entity resists knockback: currently pinned and
    /// `knockback_immune` is set.
    pub fn blocks_knockback(&self) -> bool {
        self.is_pinned() && self.knockback_immune
    }

    /// Fraction of pin time remaining [1.0 = just applied, 0.0 = freed].
    /// Returns 0.0 when not pinned.
    pub fn time_fraction(&self) -> f32 {
        if self.duration <= 0.0 || !self.is_pinned() {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Pin {
    fn default() -> Self {
        Self::new(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_not_pinned() {
        let p = Pin::new(true);
        assert!(!p.is_pinned());
        assert!(!p.just_pinned);
    }

    #[test]
    fn pin_activates() {
        let mut p = Pin::new(true);
        p.pin(3.0);
        assert!(p.is_pinned());
        assert!(p.just_pinned);
        assert!((p.timer - 3.0).abs() < 1e-5);
    }

    #[test]
    fn pin_no_op_when_disabled() {
        let mut p = Pin::new(true);
        p.enabled = false;
        p.pin(3.0);
        assert!(!p.is_pinned());
    }

    #[test]
    fn pin_no_op_when_duration_zero_or_negative() {
        let mut p = Pin::new(true);
        p.pin(0.0);
        assert!(!p.is_pinned());
        p.pin(-1.0);
        assert!(!p.is_pinned());
    }

    #[test]
    fn pin_high_watermark_extends_on_longer() {
        let mut p = Pin::new(true);
        p.pin(2.0);
        p.tick(1.0); // 1.0 remaining
        p.pin(4.0); // 4.0 > 1.0 → replaces
        assert!((p.timer - 4.0).abs() < 1e-3);
    }

    #[test]
    fn pin_high_watermark_no_shrink() {
        let mut p = Pin::new(true);
        p.pin(5.0);
        p.pin(2.0); // shorter → ignored
        assert!((p.timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn pin_no_just_pinned_on_extend() {
        let mut p = Pin::new(true);
        p.pin(2.0);
        p.tick(0.016);
        p.pin(6.0);
        assert!(!p.just_pinned);
    }

    #[test]
    fn free_ends_pin() {
        let mut p = Pin::new(true);
        p.pin(3.0);
        p.free();
        assert!(!p.is_pinned());
        assert!(p.just_freed);
        assert_eq!(p.timer, 0.0);
    }

    #[test]
    fn free_no_op_when_not_pinned() {
        let mut p = Pin::new(true);
        p.free(); // no panic
        assert!(!p.just_freed);
    }

    #[test]
    fn tick_counts_down() {
        let mut p = Pin::new(true);
        p.pin(5.0);
        p.tick(2.0);
        assert!((p.timer - 3.0).abs() < 1e-3);
        assert!(p.is_pinned());
    }

    #[test]
    fn tick_expires_and_fires_just_freed() {
        let mut p = Pin::new(true);
        p.pin(2.0);
        p.tick(2.5);
        assert!(!p.is_pinned());
        assert!(p.just_freed);
    }

    #[test]
    fn tick_clears_just_pinned() {
        let mut p = Pin::new(true);
        p.pin(3.0);
        p.tick(0.016);
        assert!(!p.just_pinned);
    }

    #[test]
    fn tick_clears_just_freed() {
        let mut p = Pin::new(true);
        p.pin(1.0);
        p.tick(2.0); // expires
        p.tick(0.016);
        assert!(!p.just_freed);
    }

    #[test]
    fn blocks_knockback_when_immune_and_pinned() {
        let mut p = Pin::new(true);
        p.pin(3.0);
        assert!(p.blocks_knockback());
    }

    #[test]
    fn blocks_knockback_false_when_not_immune() {
        let mut p = Pin::new(false);
        p.pin(3.0);
        assert!(!p.blocks_knockback());
    }

    #[test]
    fn blocks_knockback_false_when_not_pinned() {
        let p = Pin::new(true);
        assert!(!p.blocks_knockback());
    }

    #[test]
    fn time_fraction_at_full() {
        let mut p = Pin::new(true);
        p.pin(4.0);
        assert!((p.time_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn time_fraction_at_half() {
        let mut p = Pin::new(true);
        p.pin(4.0);
        p.tick(2.0);
        assert!((p.time_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn time_fraction_zero_when_not_pinned() {
        let p = Pin::new(true);
        assert_eq!(p.time_fraction(), 0.0);
    }

    #[test]
    fn can_repin_after_free() {
        let mut p = Pin::new(true);
        p.pin(2.0);
        p.free();
        p.pin(3.0);
        assert!(p.is_pinned());
        assert!(p.just_pinned);
    }

    #[test]
    fn can_repin_after_expiry() {
        let mut p = Pin::new(true);
        p.pin(1.0);
        p.tick(2.0); // expires
        p.tick(0.016); // clear flags
        p.pin(3.0);
        assert!(p.is_pinned());
        assert!(p.just_pinned);
    }
}

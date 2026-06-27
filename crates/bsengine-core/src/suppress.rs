use bevy_ecs::prelude::Component;

/// Debuff that partially or fully reduces the power of the target's abilities
/// without preventing their use.
///
/// While suppressed, systems should multiply ability potency by
/// `effective_potency(base)`. When `blocks_ultimates` is true, any ability
/// tagged as an ultimate is additionally blocked outright (returns 0 potency).
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_lifted` when the suppression ends. `clear()` ends it early.
///
/// Distinct from `Silence` (blocks all abilities), `Disarm` (blocks weapons
/// only), and `Weaken` (lowers outgoing damage): Suppress targets ability
/// _power_, not damage or usability. An entity can still cast spells, but they
/// hit softer.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Suppress {
    pub duration: f32,
    pub timer: f32,
    /// Fraction [0.0, 1.0] of ability potency remaining while suppressed.
    /// e.g. 0.4 = abilities fire at 40% power.
    pub potency_fraction: f32,
    /// When true, abilities marked as ultimates are fully blocked (0 potency).
    pub blocks_ultimates: bool,
    pub just_suppressed: bool,
    pub just_lifted: bool,
    pub enabled: bool,
}

impl Suppress {
    pub fn new(potency_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            potency_fraction: potency_fraction.clamp(0.0, 1.0),
            blocks_ultimates: false,
            just_suppressed: false,
            just_lifted: false,
            enabled: true,
        }
    }

    pub fn with_blocks_ultimates(mut self, blocks: bool) -> Self {
        self.blocks_ultimates = blocks;
        self
    }

    /// Apply or extend the suppress for `duration` seconds. High-watermark:
    /// only replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_suppressed = true;
            }
        }
    }

    /// Remove the suppress immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_lifted = true;
        }
    }

    /// Advance the timer; sets `just_lifted` when the suppress expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_suppressed = false;
        self.just_lifted = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_lifted = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective potency for a normal (non-ultimate) ability.
    /// Returns `base * potency_fraction` while active, `base` otherwise.
    pub fn effective_potency(&self, base: f32) -> f32 {
        if self.is_active() {
            base * self.potency_fraction
        } else {
            base
        }
    }

    /// Effective potency for an ultimate ability. Returns `0.0` when active
    /// and `blocks_ultimates` is true; otherwise delegates to `effective_potency`.
    pub fn effective_ultimate_potency(&self, base: f32) -> f32 {
        if self.is_active() && self.blocks_ultimates {
            0.0
        } else {
            self.effective_potency(base)
        }
    }

    /// Fraction of the suppress duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Suppress {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_suppress() {
        let mut s = Suppress::new(0.5);
        s.apply(3.0);
        assert!(s.is_active());
        assert!(s.just_suppressed);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut s = Suppress::new(0.5);
        s.apply(2.0);
        s.tick(0.016);
        s.apply(5.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut s = Suppress::new(0.5);
        s.apply(5.0);
        s.apply(2.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_suppress() {
        let mut s = Suppress::new(0.5);
        s.apply(1.0);
        s.tick(1.1);
        assert!(!s.is_active());
        assert!(s.just_lifted);
    }

    #[test]
    fn clear_ends_early() {
        let mut s = Suppress::new(0.5);
        s.apply(5.0);
        s.clear();
        assert!(!s.is_active());
        assert!(s.just_lifted);
    }

    #[test]
    fn effective_potency_while_active() {
        let mut s = Suppress::new(0.4);
        s.apply(3.0);
        assert!((s.effective_potency(10.0) - 4.0).abs() < 1e-4); // 10 * 0.4
    }

    #[test]
    fn effective_potency_when_inactive() {
        let s = Suppress::new(0.4);
        assert!((s.effective_potency(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn blocks_ultimates_returns_zero() {
        let mut s = Suppress::new(0.5).with_blocks_ultimates(true);
        s.apply(3.0);
        assert!((s.effective_ultimate_potency(100.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn blocks_ultimates_false_uses_normal_potency() {
        let mut s = Suppress::new(0.5).with_blocks_ultimates(false);
        s.apply(3.0);
        assert!((s.effective_ultimate_potency(10.0) - 5.0).abs() < 1e-4);
    }

    #[test]
    fn ultimate_potency_when_inactive() {
        let s = Suppress::new(0.5).with_blocks_ultimates(true);
        assert!((s.effective_ultimate_potency(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut s = Suppress::new(0.5);
        s.apply(2.0);
        s.tick(1.0);
        assert!((s.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut s = Suppress::new(0.5);
        s.enabled = false;
        s.apply(5.0);
        assert!(!s.is_active());
    }

    #[test]
    fn tick_clears_just_suppressed() {
        let mut s = Suppress::new(0.5);
        s.apply(3.0);
        s.tick(0.016);
        assert!(!s.just_suppressed);
    }
}

use bevy_ecs::prelude::Component;

/// Voluntary calm state that trades offensive capability for rapid healing
/// and threat-immunity signals to other systems.
///
/// While `is_tranquil()`, combat systems should prevent the entity from
/// attacking and AI targeting systems should consider it untargetable.
/// `effective_regen(base_regen)` returns `base_regen * regen_multiplier` to
/// amplify any concurrent regen tick.
///
/// `enter(duration)` starts the state (high-watermark). `interrupt()` ends it
/// early (e.g., the entity takes damage or moves). `tick(dt)` counts down and
/// sets `just_exited` when the duration expires naturally.
///
/// Distinct from `Stun` (involuntary loss of control), `Invincible`
/// (damage immunity for a timed window), `Regen` (always-on base
/// regeneration), and `Trance` (loss-of-self possession state): Tranquil is
/// a **voluntary restorative calm** — the entity deliberately rests, multiplying
/// healing received while signalling combat and AI systems to leave it alone.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Tranquil {
    pub duration: f32,
    pub timer: f32,
    /// Regen rate multiplier while tranquil. Clamped ≥ 1.0.
    pub regen_multiplier: f32,
    pub just_entered: bool,
    pub just_exited: bool,
    pub enabled: bool,
}

impl Tranquil {
    pub fn new(regen_multiplier: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            regen_multiplier: regen_multiplier.max(1.0),
            just_entered: false,
            just_exited: false,
            enabled: true,
        }
    }

    /// Enter the tranquil state for `duration` seconds. High-watermark: only
    /// replaces the current timer when `duration > timer`. Sets `just_entered`
    /// on the inactive → active transition. No-op when disabled or `duration
    /// ≤ 0`.
    pub fn enter(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_active = self.is_tranquil();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_entered = true;
            }
        }
    }

    /// End the tranquil state early (e.g., entity attacks, takes damage, or
    /// moves). Sets `just_exited`. No-op when already inactive.
    pub fn interrupt(&mut self) {
        if !self.is_tranquil() {
            return;
        }
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_exited = true;
    }

    /// Advance the timer. Sets `just_exited` when the state expires naturally.
    /// Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_entered = false;
        self.just_exited = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_exited = true;
            }
        }
    }

    pub fn is_tranquil(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective regeneration per tick while tranquil. Returns
    /// `base_regen * regen_multiplier` when active and enabled, `base_regen`
    /// otherwise.
    pub fn effective_regen(&self, base_regen: f32) -> f32 {
        if self.is_tranquil() && self.enabled {
            base_regen * self.regen_multiplier
        } else {
            base_regen
        }
    }

    /// Fraction of the tranquil duration remaining [1.0 = just entered,
    /// 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Tranquil {
    fn default() -> Self {
        Self::new(3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_starts_state() {
        let mut t = Tranquil::new(3.0);
        t.enter(5.0);
        assert!(t.is_tranquil());
        assert!(t.just_entered);
    }

    #[test]
    fn enter_extends_on_longer_duration() {
        let mut t = Tranquil::new(3.0);
        t.enter(5.0);
        t.tick(0.016);
        t.enter(10.0);
        assert!((t.timer - 10.0).abs() < 1e-4);
    }

    #[test]
    fn enter_no_extend_on_shorter_duration() {
        let mut t = Tranquil::new(3.0);
        t.enter(10.0);
        t.enter(5.0);
        assert!((t.timer - 10.0).abs() < 1e-4);
    }

    #[test]
    fn just_entered_not_set_on_extend() {
        let mut t = Tranquil::new(3.0);
        t.enter(5.0);
        t.tick(0.016);
        t.enter(10.0);
        assert!(!t.just_entered);
    }

    #[test]
    fn interrupt_ends_state() {
        let mut t = Tranquil::new(3.0);
        t.enter(5.0);
        t.interrupt();
        assert!(!t.is_tranquil());
        assert!(t.just_exited);
    }

    #[test]
    fn interrupt_no_op_when_inactive() {
        let mut t = Tranquil::new(3.0);
        t.interrupt();
        assert!(!t.just_exited);
    }

    #[test]
    fn tick_expires_state() {
        let mut t = Tranquil::new(3.0);
        t.enter(2.0);
        t.tick(2.1);
        assert!(!t.is_tranquil());
        assert!(t.just_exited);
    }

    #[test]
    fn tick_clears_just_entered() {
        let mut t = Tranquil::new(3.0);
        t.enter(5.0);
        t.tick(0.016);
        assert!(!t.just_entered);
    }

    #[test]
    fn effective_regen_multiplied_while_tranquil() {
        let mut t = Tranquil::new(3.0);
        t.enter(5.0);
        assert!((t.effective_regen(10.0) - 30.0).abs() < 1e-4);
    }

    #[test]
    fn effective_regen_base_when_inactive() {
        let t = Tranquil::new(3.0);
        assert!((t.effective_regen(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut t = Tranquil::new(3.0);
        t.enter(4.0);
        t.tick(2.0);
        assert!((t.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_inactive() {
        let t = Tranquil::new(3.0);
        assert!((t.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_enter_no_op() {
        let mut t = Tranquil::new(3.0);
        t.enabled = false;
        t.enter(5.0);
        assert!(!t.is_tranquil());
    }

    #[test]
    fn disabled_effective_regen_base() {
        let mut t = Tranquil::new(3.0);
        t.enter(5.0);
        t.enabled = false;
        assert!((t.effective_regen(10.0) - 10.0).abs() < 1e-5);
    }
}

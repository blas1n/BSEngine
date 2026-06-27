use bevy_ecs::prelude::Component;

/// Social banishment: while exiled, the entity is cut off from faction-based
/// buffs, ally healing, and cooperative interactions. The entity can still
/// act — it retains its own abilities, movement, and attacks — but ally
/// systems should not apply group bonuses to it and faction AI should treat
/// it as a non-member.
///
/// `banish(duration)` starts the exile (high-watermark); sets `just_exiled`
/// on the inactive → active transition. `pardon()` ends it early, setting
/// `just_returned`. `tick(dt)` counts down and sets `just_returned` when the
/// exile expires naturally.
///
/// Distinct from `Silence` (can't use own abilities), `Disarm` (can't attack),
/// `Fear` (forced flee from a target), and `Charm` (controlled by an enemy):
/// Exile is a **social banishment** — the entity is cut off from its faction
/// network, not from acting. Own abilities still work; the group does not.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Exile {
    pub duration: f32,
    pub timer: f32,
    pub just_exiled: bool,
    pub just_returned: bool,
    pub enabled: bool,
}

impl Exile {
    pub fn new() -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            just_exiled: false,
            just_returned: false,
            enabled: true,
        }
    }

    /// Exile the entity for `duration` seconds. High-watermark: only replaces
    /// the current timer when `duration > timer`. Sets `just_exiled` on the
    /// inactive → active transition. No-op when disabled or `duration ≤ 0`.
    pub fn banish(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_active = self.is_exiled();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_exiled = true;
            }
        }
    }

    /// End the exile early (e.g., a faction forgiveness mechanic). Sets
    /// `just_returned`. No-op when not exiled.
    pub fn pardon(&mut self) {
        if !self.is_exiled() {
            return;
        }
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_returned = true;
    }

    /// Advance the exile timer. Sets `just_returned` when the exile expires
    /// naturally. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_exiled = false;
        self.just_returned = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_returned = true;
            }
        }
    }

    pub fn is_exiled(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the exile duration remaining [1.0 = just banished,
    /// 0.0 = returned].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Exile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banish_starts_exile() {
        let mut e = Exile::new();
        e.banish(5.0);
        assert!(e.is_exiled());
        assert!(e.just_exiled);
    }

    #[test]
    fn banish_extends_on_longer_duration() {
        let mut e = Exile::new();
        e.banish(5.0);
        e.tick(0.016);
        e.banish(10.0);
        assert!((e.timer - 10.0).abs() < 1e-4);
    }

    #[test]
    fn banish_no_extend_on_shorter_duration() {
        let mut e = Exile::new();
        e.banish(10.0);
        e.banish(5.0);
        assert!((e.timer - 10.0).abs() < 1e-4);
    }

    #[test]
    fn just_exiled_not_set_on_extend() {
        let mut e = Exile::new();
        e.banish(5.0);
        e.tick(0.016);
        e.banish(10.0);
        assert!(!e.just_exiled);
    }

    #[test]
    fn pardon_ends_exile() {
        let mut e = Exile::new();
        e.banish(5.0);
        e.pardon();
        assert!(!e.is_exiled());
        assert!(e.just_returned);
    }

    #[test]
    fn pardon_no_op_when_not_exiled() {
        let mut e = Exile::new();
        e.pardon();
        assert!(!e.just_returned);
    }

    #[test]
    fn tick_expires_exile() {
        let mut e = Exile::new();
        e.banish(2.0);
        e.tick(2.1);
        assert!(!e.is_exiled());
        assert!(e.just_returned);
    }

    #[test]
    fn tick_clears_just_exiled() {
        let mut e = Exile::new();
        e.banish(5.0);
        e.tick(0.016);
        assert!(!e.just_exiled);
    }

    #[test]
    fn tick_clears_just_returned() {
        let mut e = Exile::new();
        e.banish(1.0);
        e.tick(1.1); // sets just_returned
        e.tick(0.016);
        assert!(!e.just_returned);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut e = Exile::new();
        e.banish(4.0);
        e.tick(2.0);
        assert!((e.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_inactive() {
        let e = Exile::new();
        assert!((e.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_banish_no_op() {
        let mut e = Exile::new();
        e.enabled = false;
        e.banish(5.0);
        assert!(!e.is_exiled());
    }

    #[test]
    fn banish_zero_duration_no_op() {
        let mut e = Exile::new();
        e.banish(0.0);
        assert!(!e.is_exiled());
    }
}

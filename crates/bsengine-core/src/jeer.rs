use bevy_ecs::prelude::Component;

/// Crowd-jeering morale debuff: the entity is mocked/taunted by enemies or a
/// hostile crowd, causing aim to waver and outgoing damage to drop.
///
/// While jeered, the aim system adds `aim_penalty_rad` random deviation and
/// the damage pipeline multiplies outgoing damage by `damage_fraction`.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_rallied` when the effect fades. `clear()` ends the jeer early (e.g.
/// crowd dispersed, morale restored by an ally ability).
///
/// Distinct from `Taunt` (forces target selection toward the taunter) and
/// `Demoralize` (attack speed/combat stat penalty): Jeer penalizes aim
/// accuracy and raw damage output — the entity is rattled and performing below
/// their true potential.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Jeer {
    pub duration: f32,
    pub timer: f32,
    /// Additional random aim deviation in radians while jeered.
    pub aim_penalty_rad: f32,
    /// Fraction [0.0, 1.0] of normal damage output while jeered.
    /// e.g. 0.75 = 25% damage reduction.
    pub damage_fraction: f32,
    pub just_jeered: bool,
    pub just_rallied: bool,
    pub enabled: bool,
}

impl Jeer {
    pub fn new(aim_penalty_rad: f32, damage_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            aim_penalty_rad: aim_penalty_rad.max(0.0),
            damage_fraction: damage_fraction.clamp(0.0, 1.0),
            just_jeered: false,
            just_rallied: false,
            enabled: true,
        }
    }

    /// Apply or extend the jeer for `duration` seconds. High-watermark:
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
                self.just_jeered = true;
            }
        }
    }

    /// Clear the jeer immediately (crowd silenced, morale restored).
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_rallied = true;
        }
    }

    /// Advance the timer; sets `just_rallied` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_jeered = false;
        self.just_rallied = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_rallied = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Outgoing damage after applying the jeer penalty.
    /// Returns `base * damage_fraction` while active, `base` otherwise.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_active() {
            base * self.damage_fraction
        } else {
            base
        }
    }

    /// Fraction of the jeer duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Jeer {
    fn default() -> Self {
        Self::new(0.2, 0.75)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_jeer() {
        let mut j = Jeer::new(0.2, 0.75);
        j.apply(3.0);
        assert!(j.is_active());
        assert!(j.just_jeered);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut j = Jeer::new(0.2, 0.75);
        j.apply(2.0);
        j.tick(0.016);
        j.apply(5.0);
        assert!((j.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut j = Jeer::new(0.2, 0.75);
        j.apply(5.0);
        j.apply(2.0);
        assert!((j.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_jeer() {
        let mut j = Jeer::new(0.2, 0.75);
        j.apply(1.0);
        j.tick(1.1);
        assert!(!j.is_active());
        assert!(j.just_rallied);
    }

    #[test]
    fn clear_ends_early() {
        let mut j = Jeer::new(0.2, 0.75);
        j.apply(5.0);
        j.clear();
        assert!(!j.is_active());
        assert!(j.just_rallied);
    }

    #[test]
    fn effective_damage_while_active() {
        let mut j = Jeer::new(0.2, 0.6);
        j.apply(3.0);
        assert!((j.effective_damage(100.0) - 60.0).abs() < 1e-4); // 100 * 0.6
    }

    #[test]
    fn effective_damage_when_inactive() {
        let j = Jeer::new(0.2, 0.6);
        assert!((j.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn aim_penalty_stored() {
        let j = Jeer::new(0.35, 0.75);
        assert!((j.aim_penalty_rad - 0.35).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut j = Jeer::new(0.2, 0.75);
        j.apply(2.0);
        j.tick(1.0);
        assert!((j.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut j = Jeer::new(0.2, 0.75);
        j.enabled = false;
        j.apply(5.0);
        assert!(!j.is_active());
    }

    #[test]
    fn tick_clears_just_jeered() {
        let mut j = Jeer::new(0.2, 0.75);
        j.apply(3.0);
        j.tick(0.016);
        assert!(!j.just_jeered);
    }
}

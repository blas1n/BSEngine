use bevy_ecs::prelude::Component;

/// Lingering cold debuff that deals ongoing cold damage and slows the entity's
/// attack and ability cast speed.
///
/// While frostbitten, the action pipeline multiplies attack/cast speed by
/// `action_speed_fraction` (< 1.0). The damage pipeline reads `tick(dt)` which
/// returns `cold_damage_per_second * dt` as the frostbite pulse.
///
/// `apply(duration)` uses high-watermark. `clear()` thaws the entity early
/// (e.g. warmth ability). `just_frostbitten` / `just_thawed` are set for
/// animation and sound hooks.
///
/// Distinct from `Freeze` (full immobilization, no movement or action at all)
/// and `Slow` (movement speed penalty only): Frostbite is a persistent cold
/// DoT with an action-speed tax representing numbness — the entity can still
/// move and fight, just slower and taking continuous cold damage.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Frostbite {
    pub duration: f32,
    pub timer: f32,
    /// Cold damage dealt per second while frostbitten.
    pub cold_damage_per_second: f32,
    /// Fraction [0.0, 1.0] of normal attack/cast speed while frostbitten.
    /// e.g. 0.6 = 60% action speed (40% slower).
    pub action_speed_fraction: f32,
    pub just_frostbitten: bool,
    pub just_thawed: bool,
    pub enabled: bool,
}

impl Frostbite {
    pub fn new(cold_damage_per_second: f32, action_speed_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            cold_damage_per_second: cold_damage_per_second.max(0.0),
            action_speed_fraction: action_speed_fraction.clamp(0.0, 1.0),
            just_frostbitten: false,
            just_thawed: false,
            enabled: true,
        }
    }

    /// Apply or extend the frostbite for `duration` seconds. High-watermark:
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
                self.just_frostbitten = true;
            }
        }
    }

    /// Thaw the entity immediately (e.g. warmth ability or fire damage).
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_thawed = true;
        }
    }

    /// Advance the timer. Returns the cold damage pulse for this frame
    /// (`cold_damage_per_second * dt`); 0.0 when not frostbitten.
    /// Sets `just_thawed` when the duration expires.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_frostbitten = false;
        self.just_thawed = false;

        if self.timer <= 0.0 {
            return 0.0;
        }

        let damage = self.cold_damage_per_second * dt;
        self.timer -= dt;
        if self.timer <= 0.0 {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_thawed = true;
        }

        damage
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective action speed after applying the frostbite penalty.
    /// Returns `base * action_speed_fraction` while active, `base` otherwise.
    pub fn effective_action_speed(&self, base: f32) -> f32 {
        if self.is_active() {
            base * self.action_speed_fraction
        } else {
            base
        }
    }

    /// Fraction of the frostbite duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Frostbite {
    fn default() -> Self {
        Self::new(5.0, 0.6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_frostbite() {
        let mut f = Frostbite::new(5.0, 0.6);
        f.apply(3.0);
        assert!(f.is_active());
        assert!(f.just_frostbitten);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut f = Frostbite::new(5.0, 0.6);
        f.apply(2.0);
        f.tick(0.016);
        f.apply(5.0);
        assert!((f.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut f = Frostbite::new(5.0, 0.6);
        f.apply(5.0);
        f.apply(2.0);
        assert!((f.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_returns_cold_damage() {
        let mut f = Frostbite::new(10.0, 0.6);
        f.apply(5.0);
        let dmg = f.tick(0.5);
        assert!((dmg - 5.0).abs() < 1e-4); // 10 * 0.5
    }

    #[test]
    fn tick_returns_zero_when_inactive() {
        let mut f = Frostbite::new(10.0, 0.6);
        assert!((f.tick(0.1) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn tick_expires_frostbite() {
        let mut f = Frostbite::new(5.0, 0.6);
        f.apply(1.0);
        f.tick(1.1);
        assert!(!f.is_active());
        assert!(f.just_thawed);
    }

    #[test]
    fn clear_thaws_early() {
        let mut f = Frostbite::new(5.0, 0.6);
        f.apply(5.0);
        f.clear();
        assert!(!f.is_active());
        assert!(f.just_thawed);
    }

    #[test]
    fn effective_action_speed_while_active() {
        let mut f = Frostbite::new(5.0, 0.5);
        f.apply(3.0);
        assert!((f.effective_action_speed(2.0) - 1.0).abs() < 1e-5); // 2 * 0.5
    }

    #[test]
    fn effective_action_speed_when_inactive() {
        let f = Frostbite::new(5.0, 0.5);
        assert!((f.effective_action_speed(2.0) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut f = Frostbite::new(5.0, 0.6);
        f.apply(2.0);
        f.tick(1.0);
        assert!((f.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut f = Frostbite::new(5.0, 0.6);
        f.enabled = false;
        f.apply(5.0);
        assert!(!f.is_active());
    }

    #[test]
    fn tick_clears_just_frostbitten() {
        let mut f = Frostbite::new(5.0, 0.6);
        f.apply(3.0);
        f.tick(0.016);
        assert!(!f.just_frostbitten);
    }
}

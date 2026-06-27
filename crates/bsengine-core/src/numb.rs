use bevy_ecs::prelude::Component;

/// Status that dulls the target's pain response: while active, incoming hits
/// do not trigger CC side-effects such as stagger, flinch, or interrupts, even
/// though the raw damage still applies.
///
/// Optional `damage_fraction` can reduce incoming damage too (e.g. a painkiller
/// that also dampens physical trauma), but the defining trait is the CC immunity.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_worn_off` when the effect ends. `clear()` removes it early.
///
/// Distinct from `Immune` (blocks all damage), `Barrier` (absorbs damage), and
/// `Invincible` (no damage or CC): Numb is specifically about bypassing pain-
/// response CC while still taking damage. Useful for a berserker/rage state
/// where the character pushes through hits.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Numb {
    pub duration: f32,
    pub timer: f32,
    /// Incoming damage multiplier [0.0, 1.0]. 1.0 = full damage, 0.5 = halved.
    /// The numbing effect (CC immunity) is always active regardless of this value.
    pub damage_fraction: f32,
    pub just_numbed: bool,
    pub just_worn_off: bool,
    pub enabled: bool,
}

impl Numb {
    pub fn new(damage_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            damage_fraction: damage_fraction.clamp(0.0, 1.0),
            just_numbed: false,
            just_worn_off: false,
            enabled: true,
        }
    }

    /// Apply or extend the numb for `duration` seconds. High-watermark: only
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
                self.just_numbed = true;
            }
        }
    }

    /// Remove the numb effect immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_worn_off = true;
        }
    }

    /// Advance the timer; sets `just_worn_off` when the effect expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_numbed = false;
        self.just_worn_off = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_worn_off = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective incoming damage after applying `damage_fraction`.
    /// Returns `incoming * damage_fraction` while active, `incoming` otherwise.
    pub fn effective_damage(&self, incoming: f32) -> f32 {
        if self.is_active() {
            incoming * self.damage_fraction
        } else {
            incoming
        }
    }

    /// Fraction of the numb duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Numb {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_numb() {
        let mut n = Numb::new(1.0);
        n.apply(3.0);
        assert!(n.is_active());
        assert!(n.just_numbed);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut n = Numb::new(1.0);
        n.apply(2.0);
        n.tick(0.016);
        n.apply(5.0);
        assert!((n.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut n = Numb::new(1.0);
        n.apply(5.0);
        n.apply(2.0);
        assert!((n.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_numb() {
        let mut n = Numb::new(1.0);
        n.apply(1.0);
        n.tick(1.1);
        assert!(!n.is_active());
        assert!(n.just_worn_off);
    }

    #[test]
    fn clear_ends_early() {
        let mut n = Numb::new(1.0);
        n.apply(5.0);
        n.clear();
        assert!(!n.is_active());
        assert!(n.just_worn_off);
    }

    #[test]
    fn effective_damage_while_active_with_reduction() {
        let mut n = Numb::new(0.5);
        n.apply(3.0);
        assert!((n.effective_damage(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_full_when_inactive() {
        let n = Numb::new(0.5);
        assert!((n.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_damage_full_when_fraction_is_one() {
        let mut n = Numb::new(1.0);
        n.apply(3.0);
        assert!((n.effective_damage(80.0) - 80.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut n = Numb::new(1.0);
        n.apply(2.0);
        n.tick(1.0);
        assert!((n.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut n = Numb::new(1.0);
        n.enabled = false;
        n.apply(5.0);
        assert!(!n.is_active());
    }

    #[test]
    fn tick_clears_just_numbed() {
        let mut n = Numb::new(1.0);
        n.apply(3.0);
        n.tick(0.016);
        assert!(!n.just_numbed);
    }
}

use bevy_ecs::prelude::Component;

/// Predatory movement buff that boosts move speed and stores a one-shot
/// ambush damage multiplier for the first strike while prowling.
///
/// While active, movement systems multiply the entity's speed by
/// `effective_speed(base)`. The first melee/ranged hit should call
/// `try_ambush()` — if the ambush bonus has not yet been consumed,
/// it marks it consumed and returns `Some(ambush_damage_multiplier)` to
/// apply to the outgoing damage. Subsequent hits return `None`.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_faded` when the buff expires. `clear()` cancels early.
///
/// Distinct from `Stealth` (full invisibility / detection-hide): Prowl is a
/// speed-and-strike buff that does not hide the entity from detection — it
/// represents a predator's focused sprint before a killing blow.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Prowl {
    pub duration: f32,
    pub timer: f32,
    /// Fractional speed bonus while prowling. e.g. 0.3 = 30% faster.
    pub speed_bonus_fraction: f32,
    /// Damage multiplier applied by the first hit while prowling.
    pub ambush_damage_multiplier: f32,
    /// True after the ambush bonus has been spent.
    pub ambush_consumed: bool,
    pub just_prowling: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Prowl {
    pub fn new(speed_bonus_fraction: f32, ambush_damage_multiplier: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            speed_bonus_fraction: speed_bonus_fraction.max(0.0),
            ambush_damage_multiplier: ambush_damage_multiplier.max(1.0),
            ambush_consumed: false,
            just_prowling: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Apply or extend the prowl for `duration` seconds. High-watermark:
    /// only replaces the current timer if the new duration is longer.
    /// Resets `ambush_consumed` so a fresh ambush window opens.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            self.ambush_consumed = false;
            if !was_active {
                self.just_prowling = true;
            }
        }
    }

    /// Cancel the prowl immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_faded = true;
        }
    }

    /// Advance the timer; sets `just_faded` when the buff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_prowling = false;
        self.just_faded = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_faded = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Spend the ambush bonus. Returns `Some(multiplier)` the first time this
    /// is called while active; returns `None` thereafter.
    pub fn try_ambush(&mut self) -> Option<f32> {
        if self.is_active() && !self.ambush_consumed {
            self.ambush_consumed = true;
            Some(self.ambush_damage_multiplier)
        } else {
            None
        }
    }

    /// Effective move speed after applying the prowl speed bonus.
    /// Returns `base * (1.0 + speed_bonus_fraction)` while active, `base` otherwise.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if self.is_active() {
            base * (1.0 + self.speed_bonus_fraction)
        } else {
            base
        }
    }

    /// Fraction of the prowl duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Prowl {
    fn default() -> Self {
        Self::new(0.3, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_prowl() {
        let mut p = Prowl::new(0.3, 2.0);
        p.apply(3.0);
        assert!(p.is_active());
        assert!(p.just_prowling);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut p = Prowl::new(0.3, 2.0);
        p.apply(2.0);
        p.tick(0.016);
        p.apply(5.0);
        assert!((p.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut p = Prowl::new(0.3, 2.0);
        p.apply(5.0);
        p.apply(2.0);
        assert!((p.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_prowl() {
        let mut p = Prowl::new(0.3, 2.0);
        p.apply(1.0);
        p.tick(1.1);
        assert!(!p.is_active());
        assert!(p.just_faded);
    }

    #[test]
    fn clear_ends_early() {
        let mut p = Prowl::new(0.3, 2.0);
        p.apply(5.0);
        p.clear();
        assert!(!p.is_active());
        assert!(p.just_faded);
    }

    #[test]
    fn try_ambush_returns_multiplier_once() {
        let mut p = Prowl::new(0.3, 3.0);
        p.apply(5.0);
        let first = p.try_ambush();
        assert_eq!(first, Some(3.0));
        let second = p.try_ambush();
        assert_eq!(second, None);
    }

    #[test]
    fn try_ambush_none_when_inactive() {
        let mut p = Prowl::new(0.3, 2.0);
        assert_eq!(p.try_ambush(), None);
    }

    #[test]
    fn apply_resets_ambush_on_refresh() {
        let mut p = Prowl::new(0.3, 2.0);
        p.apply(3.0);
        p.try_ambush(); // consume
        p.apply(10.0); // high-watermark extends
        assert!(!p.ambush_consumed); // fresh window
    }

    #[test]
    fn effective_speed_while_active() {
        let mut p = Prowl::new(0.5, 2.0);
        p.apply(3.0);
        assert!((p.effective_speed(10.0) - 15.0).abs() < 1e-4); // 10 * 1.5
    }

    #[test]
    fn effective_speed_when_inactive() {
        let p = Prowl::new(0.5, 2.0);
        assert!((p.effective_speed(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut p = Prowl::new(0.3, 2.0);
        p.apply(2.0);
        p.tick(1.0);
        assert!((p.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut p = Prowl::new(0.3, 2.0);
        p.enabled = false;
        p.apply(5.0);
        assert!(!p.is_active());
    }

    #[test]
    fn tick_clears_just_prowling() {
        let mut p = Prowl::new(0.3, 2.0);
        p.apply(3.0);
        p.tick(0.016);
        assert!(!p.just_prowling);
    }
}

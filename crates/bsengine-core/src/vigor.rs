use bevy_ecs::prelude::Component;

/// Second-wind buff that combines passive health regeneration with a temporary
/// expansion of the entity's maximum health ceiling.
///
/// While in vigor, each frame the health system should:
/// - Apply `health_regen(dt)` as healing to the entity.
/// - Use `effective_max_health(base)` as the entity's current max HP cap.
///
/// Both return neutral values when inactive. `apply(duration)` uses
/// high-watermark. `tick(dt)` counts down and sets `just_faded` on expiry.
///
/// Distinct from `Regen` (pure healing over time with no max HP change),
/// `Absorption` (damage shield layer), and `Revive` (resurrection mechanic):
/// Vigor is a "battle surge" buff — it simultaneously heals and raises the
/// health ceiling, representing an adrenaline-fueled second wind.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vigor {
    pub duration: f32,
    pub timer: f32,
    /// Passive health restored per second while in vigor.
    pub health_regen_per_second: f32,
    /// Flat bonus added to maximum health while in vigor.
    pub max_health_bonus: f32,
    pub just_invigorated: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Vigor {
    pub fn new(health_regen_per_second: f32, max_health_bonus: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            health_regen_per_second: health_regen_per_second.max(0.0),
            max_health_bonus: max_health_bonus.max(0.0),
            just_invigorated: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Apply or extend vigor for `duration` seconds. High-watermark: only
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
                self.just_invigorated = true;
            }
        }
    }

    /// Remove vigor immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_faded = true;
        }
    }

    /// Advance the timer; sets `just_faded` when the vigor expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_invigorated = false;
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

    /// Healing to apply this frame. Returns `health_regen_per_second * dt`
    /// while active, `0.0` otherwise.
    pub fn health_regen(&self, dt: f32) -> f32 {
        if self.is_active() {
            self.health_regen_per_second * dt
        } else {
            0.0
        }
    }

    /// Effective max health with the vigor bonus. Returns `base + max_health_bonus`
    /// while active, `base` otherwise.
    pub fn effective_max_health(&self, base: f32) -> f32 {
        if self.is_active() {
            base + self.max_health_bonus
        } else {
            base
        }
    }

    /// Fraction of the vigor duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Vigor {
    fn default() -> Self {
        Self::new(5.0, 50.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_vigor() {
        let mut v = Vigor::new(5.0, 50.0);
        v.apply(3.0);
        assert!(v.is_active());
        assert!(v.just_invigorated);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut v = Vigor::new(5.0, 50.0);
        v.apply(2.0);
        v.tick(0.016);
        v.apply(5.0);
        assert!((v.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut v = Vigor::new(5.0, 50.0);
        v.apply(5.0);
        v.apply(2.0);
        assert!((v.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_vigor() {
        let mut v = Vigor::new(5.0, 50.0);
        v.apply(1.0);
        v.tick(1.1);
        assert!(!v.is_active());
        assert!(v.just_faded);
    }

    #[test]
    fn clear_ends_early() {
        let mut v = Vigor::new(5.0, 50.0);
        v.apply(5.0);
        v.clear();
        assert!(!v.is_active());
        assert!(v.just_faded);
    }

    #[test]
    fn health_regen_while_active() {
        let mut v = Vigor::new(10.0, 50.0);
        v.apply(3.0);
        assert!((v.health_regen(0.1) - 1.0).abs() < 1e-5); // 10 * 0.1
    }

    #[test]
    fn health_regen_when_inactive() {
        let v = Vigor::new(10.0, 50.0);
        assert!((v.health_regen(0.1) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn effective_max_health_while_active() {
        let mut v = Vigor::new(5.0, 50.0);
        v.apply(3.0);
        assert!((v.effective_max_health(200.0) - 250.0).abs() < 1e-3);
    }

    #[test]
    fn effective_max_health_when_inactive() {
        let v = Vigor::new(5.0, 50.0);
        assert!((v.effective_max_health(200.0) - 200.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut v = Vigor::new(5.0, 50.0);
        v.apply(2.0);
        v.tick(1.0);
        assert!((v.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut v = Vigor::new(5.0, 50.0);
        v.enabled = false;
        v.apply(5.0);
        assert!(!v.is_active());
    }

    #[test]
    fn tick_clears_just_invigorated() {
        let mut v = Vigor::new(5.0, 50.0);
        v.apply(3.0);
        v.tick(0.016);
        assert!(!v.just_invigorated);
    }

    #[test]
    fn negative_params_clamped_to_zero() {
        let v = Vigor::new(-5.0, -20.0);
        assert!((v.health_regen_per_second - 0.0).abs() < 1e-5);
        assert!((v.max_health_bonus - 0.0).abs() < 1e-5);
    }
}

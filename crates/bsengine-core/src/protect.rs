use bevy_ecs::prelude::Component;

/// Self-sacrificing guard stance: while `is_guarding()`, the entity pulls a
/// fraction of incoming damage from nearby allies to itself. Ally damage systems
/// should call `absorbed_damage(incoming)` when an ally within `guard_radius`
/// takes a hit — it returns the portion the guarding entity absorbs, and the
/// remainder is dealt to the ally.
///
/// `guard(duration)` starts or extends the stance (high-watermark); sets
/// `just_began` on the inactive → active transition. `stand_down()` ends it
/// early. `tick(dt)` counts down and sets `just_ended` on expiry.
///
/// Distinct from `Shield` (personal damage absorption), `Barrier` (a field that
/// absorbs all incoming damage), and `Cover` (line-of-sight blocking): Protect
/// is a **self-sacrificing guard stance** — the entity deliberately intercepts
/// a fraction of damage meant for allies at the cost of absorbing it personally.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Protect {
    pub duration: f32,
    pub timer: f32,
    /// World-unit radius within which ally damage is intercepted.
    /// Clamped ≥ 0.0.
    pub guard_radius: f32,
    /// Fraction of ally incoming damage redirected to this entity.
    /// Clamped [0.0, 1.0].
    pub redirect_fraction: f32,
    pub just_began: bool,
    pub just_ended: bool,
    pub enabled: bool,
}

impl Protect {
    pub fn new(guard_radius: f32, redirect_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            guard_radius: guard_radius.max(0.0),
            redirect_fraction: redirect_fraction.clamp(0.0, 1.0),
            just_began: false,
            just_ended: false,
            enabled: true,
        }
    }

    /// Enter or extend the guard stance for `duration` seconds. High-watermark:
    /// only replaces the current timer when `duration > timer`. Sets `just_began`
    /// on the inactive → active transition. No-op when disabled or `duration ≤ 0`.
    pub fn guard(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_guarding = self.is_guarding();
            self.duration = duration;
            self.timer = duration;
            if !was_guarding {
                self.just_began = true;
            }
        }
    }

    /// Drop the guard stance early. Sets `just_ended`. No-op when not guarding.
    pub fn stand_down(&mut self) {
        if !self.is_guarding() {
            return;
        }
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_ended = true;
    }

    /// Advance the protect timer. Sets `just_ended` when the stance expires
    /// naturally. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_began = false;
        self.just_ended = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_ended = true;
            }
        }
    }

    pub fn is_guarding(&self) -> bool {
        self.timer > 0.0
    }

    /// Whether an ally at `distance` world units is within the guard radius.
    pub fn in_range(&self, distance: f32) -> bool {
        self.is_guarding() && distance <= self.guard_radius
    }

    /// Damage this entity absorbs from an ally hit while guarding and enabled.
    /// Returns `incoming * redirect_fraction`; the ally takes the remainder.
    /// Returns 0.0 when not guarding or disabled.
    pub fn absorbed_damage(&self, incoming: f32) -> f32 {
        if self.is_guarding() && self.enabled {
            incoming * self.redirect_fraction
        } else {
            0.0
        }
    }

    /// Fraction of the guard duration remaining [1.0 = just began, 0.0 = ended].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Protect {
    fn default() -> Self {
        Self::new(5.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_starts_protection() {
        let mut p = Protect::new(5.0, 0.5);
        p.guard(3.0);
        assert!(p.is_guarding());
        assert!(p.just_began);
    }

    #[test]
    fn guard_extends_on_longer_duration() {
        let mut p = Protect::new(5.0, 0.5);
        p.guard(2.0);
        p.tick(0.016);
        p.guard(6.0);
        assert!((p.timer - 6.0).abs() < 1e-4);
    }

    #[test]
    fn guard_no_extend_on_shorter_duration() {
        let mut p = Protect::new(5.0, 0.5);
        p.guard(6.0);
        p.guard(2.0);
        assert!((p.timer - 6.0).abs() < 1e-4);
    }

    #[test]
    fn just_began_not_set_on_extend() {
        let mut p = Protect::new(5.0, 0.5);
        p.guard(2.0);
        p.tick(0.016);
        p.guard(6.0);
        assert!(!p.just_began);
    }

    #[test]
    fn stand_down_ends_protection() {
        let mut p = Protect::new(5.0, 0.5);
        p.guard(3.0);
        p.stand_down();
        assert!(!p.is_guarding());
        assert!(p.just_ended);
    }

    #[test]
    fn stand_down_no_op_when_not_guarding() {
        let mut p = Protect::new(5.0, 0.5);
        p.stand_down();
        assert!(!p.just_ended);
    }

    #[test]
    fn tick_expires_protection() {
        let mut p = Protect::new(5.0, 0.5);
        p.guard(1.0);
        p.tick(1.1);
        assert!(!p.is_guarding());
        assert!(p.just_ended);
    }

    #[test]
    fn tick_clears_just_began() {
        let mut p = Protect::new(5.0, 0.5);
        p.guard(3.0);
        p.tick(0.016);
        assert!(!p.just_began);
    }

    #[test]
    fn tick_clears_just_ended() {
        let mut p = Protect::new(5.0, 0.5);
        p.guard(0.5);
        p.tick(1.0);
        p.tick(0.016);
        assert!(!p.just_ended);
    }

    #[test]
    fn in_range_true_within_radius() {
        let mut p = Protect::new(5.0, 0.5);
        p.guard(3.0);
        assert!(p.in_range(4.0));
        assert!(p.in_range(5.0));
    }

    #[test]
    fn in_range_false_beyond_radius() {
        let mut p = Protect::new(5.0, 0.5);
        p.guard(3.0);
        assert!(!p.in_range(6.0));
    }

    #[test]
    fn in_range_false_when_not_guarding() {
        let p = Protect::new(5.0, 0.5);
        assert!(!p.in_range(1.0));
    }

    #[test]
    fn absorbed_damage_returns_fraction_while_guarding() {
        let mut p = Protect::new(5.0, 0.5);
        p.guard(3.0);
        // 100 * 0.5 = 50 absorbed, 50 to ally
        assert!((p.absorbed_damage(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn absorbed_damage_zero_when_not_guarding() {
        let p = Protect::new(5.0, 0.5);
        assert!((p.absorbed_damage(100.0)).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut p = Protect::new(5.0, 0.5);
        p.guard(4.0);
        p.tick(2.0);
        assert!((p.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_not_guarding() {
        let p = Protect::new(5.0, 0.5);
        assert!((p.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_guard_no_op() {
        let mut p = Protect::new(5.0, 0.5);
        p.enabled = false;
        p.guard(3.0);
        assert!(!p.is_guarding());
    }

    #[test]
    fn disabled_absorbed_damage_zero() {
        let mut p = Protect::new(5.0, 0.5);
        p.guard(3.0);
        p.enabled = false;
        assert!((p.absorbed_damage(100.0)).abs() < 1e-5);
    }
}

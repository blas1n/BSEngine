use bevy_ecs::prelude::Component;

/// Illusory-decoy projector: while active, each incoming attack has a
/// `misdirect_chance` probability of being redirected to the mirage instead
/// of the real entity. The mirage is purely cosmetic from the ECS perspective;
/// this component only tracks the state and expiry.
///
/// `project(duration)` starts or extends the mirage (high-watermark); sets
/// `just_created` on the inactive → active transition. `dispel()` ends it
/// early. `tick(dt)` counts down and sets `just_faded` on expiry.
///
/// The targeting system should call `will_redirect(roll)` per incoming attack
/// with a uniform [0.0, 1.0] random roll; `true` means the attack hits the
/// mirage (miss from the entity's perspective).
///
/// Distinct from `Stealth` (reduces visibility entirely), `Dodge` (reactive
/// per-frame evasion), and `Phase` (passes through geometry): Mirage is a
/// **sustained misdirection field** that probabilistically redirects a
/// fraction of incoming attacks for its entire duration.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Mirage {
    pub duration: f32,
    pub timer: f32,
    /// Probability [0.0, 1.0] that an incoming attack hits the mirage instead.
    pub misdirect_chance: f32,
    pub just_created: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Mirage {
    pub fn new(misdirect_chance: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            misdirect_chance: misdirect_chance.clamp(0.0, 1.0),
            just_created: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Project or extend the mirage for `duration` seconds. High-watermark:
    /// only replaces the timer when `duration > timer`. Sets `just_created` on
    /// the inactive → active transition. No-op when disabled or `duration ≤ 0`.
    pub fn project(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_projecting = self.is_projecting();
            self.duration = duration;
            self.timer = duration;
            if !was_projecting {
                self.just_created = true;
            }
        }
    }

    /// End the mirage early. Sets `just_faded`. No-op when not projecting.
    pub fn dispel(&mut self) {
        if !self.is_projecting() {
            return;
        }
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_faded = true;
    }

    /// Advance the mirage timer. Sets `just_faded` when the mirage expires
    /// naturally. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_created = false;
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

    pub fn is_projecting(&self) -> bool {
        self.timer > 0.0
    }

    /// Returns `true` when the incoming attack should be redirected to the
    /// mirage. `roll` should be a uniform [0.0, 1.0] value supplied by the
    /// caller. No-op (returns `false`) when not projecting or disabled.
    pub fn will_redirect(&self, roll: f32) -> bool {
        self.is_projecting() && self.enabled && roll < self.misdirect_chance
    }

    /// Fraction of the mirage duration remaining [1.0 = just created, 0.0 = faded].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Mirage {
    fn default() -> Self {
        Self::new(0.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_starts_mirage() {
        let mut m = Mirage::new(0.4);
        m.project(5.0);
        assert!(m.is_projecting());
        assert!(m.just_created);
    }

    #[test]
    fn project_extends_on_longer_duration() {
        let mut m = Mirage::new(0.4);
        m.project(3.0);
        m.tick(0.016);
        m.project(8.0);
        assert!((m.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn project_no_extend_on_shorter_duration() {
        let mut m = Mirage::new(0.4);
        m.project(8.0);
        m.project(3.0);
        assert!((m.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn just_created_not_set_on_extend() {
        let mut m = Mirage::new(0.4);
        m.project(3.0);
        m.tick(0.016);
        m.project(8.0);
        assert!(!m.just_created);
    }

    #[test]
    fn dispel_ends_mirage() {
        let mut m = Mirage::new(0.4);
        m.project(5.0);
        m.dispel();
        assert!(!m.is_projecting());
        assert!(m.just_faded);
    }

    #[test]
    fn dispel_no_op_when_not_projecting() {
        let mut m = Mirage::new(0.4);
        m.dispel();
        assert!(!m.just_faded);
    }

    #[test]
    fn tick_expires_mirage() {
        let mut m = Mirage::new(0.4);
        m.project(1.0);
        m.tick(1.1);
        assert!(!m.is_projecting());
        assert!(m.just_faded);
    }

    #[test]
    fn tick_clears_just_created() {
        let mut m = Mirage::new(0.4);
        m.project(5.0);
        m.tick(0.016);
        assert!(!m.just_created);
    }

    #[test]
    fn tick_clears_just_faded() {
        let mut m = Mirage::new(0.4);
        m.project(0.5);
        m.tick(1.0);
        m.tick(0.016);
        assert!(!m.just_faded);
    }

    #[test]
    fn will_redirect_below_threshold() {
        let mut m = Mirage::new(0.5);
        m.project(5.0);
        assert!(m.will_redirect(0.49));
    }

    #[test]
    fn will_redirect_at_threshold_false() {
        let mut m = Mirage::new(0.5);
        m.project(5.0);
        assert!(!m.will_redirect(0.5));
    }

    #[test]
    fn will_redirect_false_when_not_projecting() {
        let m = Mirage::new(0.9);
        assert!(!m.will_redirect(0.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut m = Mirage::new(0.4);
        m.project(4.0);
        m.tick(2.0);
        assert!((m.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_not_projecting() {
        let m = Mirage::new(0.4);
        assert!((m.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_project_no_op() {
        let mut m = Mirage::new(0.4);
        m.enabled = false;
        m.project(5.0);
        assert!(!m.is_projecting());
    }

    #[test]
    fn disabled_will_redirect_false() {
        let mut m = Mirage::new(0.9);
        m.project(5.0);
        m.enabled = false;
        assert!(!m.will_redirect(0.0));
    }
}

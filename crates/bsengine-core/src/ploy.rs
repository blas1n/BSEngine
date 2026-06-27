use bevy_ecs::prelude::Component;

/// Deceptive feign stance: while `is_feigning()`, the entity appears weaker
/// or retreating, baiting nearby enemies into lowering their guard. AI and
/// combat systems should treat an entity in a ploy as an opportunity target
/// and reduce their own defensive thresholds (e.g., lower their
/// `Protect`/`Guard` duration, widen their approach arc) — making them
/// vulnerable to a counter-attack when the ploy drops.
///
/// `feign(duration)` enters or extends the stance (high-watermark); sets
/// `just_began` on the inactive → active transition. `drop_ploy()` ends it
/// early and sets `just_ended`. `tick(dt)` counts down and sets `just_ended`
/// on natural expiry.
///
/// Distinct from `Charm` (forced behavioural override), `Taunt` (forces
/// attention onto the caster), `Stealth` (entity becomes invisible), and
/// `Provoke` (raises enemy threat toward caster): Ploy is a **deliberate
/// self-weakening ruse** — the entity voluntarily presents a weakness to
/// invite a reckless attack, rather than forcing a specific response.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ploy {
    pub active: bool,
    pub timer: f32,
    pub just_began: bool,
    pub just_ended: bool,
    pub enabled: bool,
}

impl Ploy {
    pub fn new() -> Self {
        Self {
            active: false,
            timer: 0.0,
            just_began: false,
            just_ended: false,
            enabled: true,
        }
    }

    /// Enter or extend the feign stance for `duration` seconds. High-watermark:
    /// only replaces the current timer when `duration > timer`. Sets `just_began`
    /// on the inactive → active transition. No-op when disabled or `duration ≤ 0`.
    pub fn feign(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_active = self.active;
            self.active = true;
            self.timer = duration;
            if !was_active {
                self.just_began = true;
            }
        }
    }

    /// Drop the feign stance early. Sets `just_ended`. No-op when not feigning.
    pub fn drop_ploy(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.timer = 0.0;
        self.just_ended = true;
    }

    /// Advance the stance timer. Sets `just_ended` on natural expiry.
    /// Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_began = false;
        self.just_ended = false;

        if self.active {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.active = false;
                self.just_ended = true;
            }
        }
    }

    /// `true` when the entity is actively feigning and the component is enabled.
    pub fn is_feigning(&self) -> bool {
        self.active && self.enabled
    }
}

impl Default for Ploy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_inactive() {
        let p = Ploy::new();
        assert!(!p.active);
        assert!(!p.is_feigning());
    }

    #[test]
    fn feign_activates() {
        let mut p = Ploy::new();
        p.feign(2.0);
        assert!(p.active);
        assert!(p.just_began);
        assert!(p.is_feigning());
    }

    #[test]
    fn feign_extends_on_longer_duration() {
        let mut p = Ploy::new();
        p.feign(2.0);
        p.tick(0.016);
        p.feign(5.0);
        assert!((p.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn feign_no_extend_on_shorter_duration() {
        let mut p = Ploy::new();
        p.feign(5.0);
        p.feign(2.0);
        assert!((p.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn just_began_not_set_on_extend() {
        let mut p = Ploy::new();
        p.feign(2.0);
        p.tick(0.016);
        p.feign(5.0);
        assert!(!p.just_began);
    }

    #[test]
    fn feign_no_op_when_disabled() {
        let mut p = Ploy::new();
        p.enabled = false;
        p.feign(2.0);
        assert!(!p.active);
    }

    #[test]
    fn feign_no_op_at_zero_duration() {
        let mut p = Ploy::new();
        p.feign(0.0);
        assert!(!p.active);
    }

    #[test]
    fn feign_no_op_at_negative_duration() {
        let mut p = Ploy::new();
        p.feign(-1.0);
        assert!(!p.active);
    }

    #[test]
    fn drop_ploy_ends_early() {
        let mut p = Ploy::new();
        p.feign(3.0);
        p.drop_ploy();
        assert!(!p.active);
        assert!(p.just_ended);
    }

    #[test]
    fn drop_ploy_no_op_when_not_active() {
        let mut p = Ploy::new();
        p.drop_ploy();
        assert!(!p.just_ended);
    }

    #[test]
    fn tick_expires_stance() {
        let mut p = Ploy::new();
        p.feign(1.0);
        p.tick(1.1);
        assert!(!p.active);
        assert!(p.just_ended);
    }

    #[test]
    fn tick_clears_just_began() {
        let mut p = Ploy::new();
        p.feign(2.0);
        p.tick(0.016);
        assert!(!p.just_began);
    }

    #[test]
    fn tick_clears_just_ended() {
        let mut p = Ploy::new();
        p.feign(0.5);
        p.tick(1.0);
        p.tick(0.016);
        assert!(!p.just_ended);
    }

    #[test]
    fn tick_decrements_timer() {
        let mut p = Ploy::new();
        p.feign(3.0);
        p.tick(1.0);
        assert!((p.timer - 2.0).abs() < 1e-4);
        assert!(p.active);
    }

    #[test]
    fn is_feigning_false_when_disabled() {
        let mut p = Ploy::new();
        p.feign(2.0);
        p.enabled = false;
        assert!(!p.is_feigning());
    }

    #[test]
    fn second_feign_after_drop_sets_just_began_again() {
        let mut p = Ploy::new();
        p.feign(2.0);
        p.drop_ploy();
        p.tick(0.016);
        p.feign(2.0);
        assert!(p.just_began);
    }
}

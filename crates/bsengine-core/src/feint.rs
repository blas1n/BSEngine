use bevy_ecs::prelude::Component;

/// Fake-out offensive manoeuvre: while `is_feinting()`, the entity performs a
/// deceptive attack that bait's the opponent's parry/block. Combat systems
/// should treat this as a non-damaging attack and check `just_completed` to
/// know when the opponent's guard window has been broken open.
///
/// `feint(duration)` starts or extends the feint animation (high-watermark);
/// sets `just_feinted` on the inactive → active transition. `cancel()` aborts
/// the feint without breaking the opponent's guard (the fake-out was spotted).
/// `tick(dt)` counts down and sets `just_completed` on natural expiry.
///
/// Distinct from `Dodge` (evading incoming attacks), `Parry` (blocking
/// an incoming strike), and `Provoke` (forcing enemy targeting): Feint is a
/// **committed fake-attack opener** — the animation must complete for the
/// guard break to register; aborting it mid-motion resets both sides.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Feint {
    pub duration: f32,
    pub timer: f32,
    pub just_feinted: bool,
    pub just_completed: bool,
    pub enabled: bool,
}

impl Feint {
    pub fn new() -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            just_feinted: false,
            just_completed: false,
            enabled: true,
        }
    }

    /// Begin or extend the feint for `duration` seconds. High-watermark: only
    /// replaces the timer when `duration > timer`. Sets `just_feinted` on the
    /// inactive → active transition. No-op when disabled or `duration ≤ 0`.
    pub fn feint(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_feinting = self.is_feinting();
            self.duration = duration;
            self.timer = duration;
            if !was_feinting {
                self.just_feinted = true;
            }
        }
    }

    /// Abort the feint animation early. The guard break does NOT trigger —
    /// a cancelled feint reveals the deception. No-op when not feinting.
    pub fn cancel(&mut self) {
        if !self.is_feinting() {
            return;
        }
        self.timer = 0.0;
        self.duration = 0.0;
    }

    /// Advance the feint timer. Returns `true` and sets `just_completed` when
    /// the feint animation finishes naturally (guard break opens). A cancelled
    /// feint never returns `true`. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.just_feinted = false;
        self.just_completed = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_completed = true;
                return true;
            }
        }
        false
    }

    pub fn is_feinting(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the feint animation remaining [1.0 = just started, 0.0 = complete].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Feint {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feint_starts_animation() {
        let mut f = Feint::new();
        f.feint(0.5);
        assert!(f.is_feinting());
        assert!(f.just_feinted);
    }

    #[test]
    fn feint_extends_on_longer_duration() {
        let mut f = Feint::new();
        f.feint(0.5);
        f.tick(0.016);
        f.feint(1.0);
        assert!((f.timer - 1.0).abs() < 1e-4);
    }

    #[test]
    fn feint_no_extend_on_shorter_duration() {
        let mut f = Feint::new();
        f.feint(1.0);
        f.feint(0.3);
        assert!((f.timer - 1.0).abs() < 1e-4);
    }

    #[test]
    fn just_feinted_not_set_on_extend() {
        let mut f = Feint::new();
        f.feint(0.5);
        f.tick(0.016);
        f.feint(1.0);
        assert!(!f.just_feinted);
    }

    #[test]
    fn cancel_aborts_without_guard_break() {
        let mut f = Feint::new();
        f.feint(1.0);
        f.cancel();
        assert!(!f.is_feinting());
        assert!(!f.just_completed);
    }

    #[test]
    fn cancel_no_op_when_not_feinting() {
        let mut f = Feint::new();
        f.cancel();
        assert!(!f.just_completed);
    }

    #[test]
    fn tick_completes_feint_and_breaks_guard() {
        let mut f = Feint::new();
        f.feint(0.5);
        let broke = f.tick(0.6);
        assert!(broke);
        assert!(!f.is_feinting());
        assert!(f.just_completed);
    }

    #[test]
    fn tick_returns_false_before_expiry() {
        let mut f = Feint::new();
        f.feint(1.0);
        assert!(!f.tick(0.3));
    }

    #[test]
    fn tick_clears_just_feinted() {
        let mut f = Feint::new();
        f.feint(1.0);
        f.tick(0.016);
        assert!(!f.just_feinted);
    }

    #[test]
    fn tick_clears_just_completed() {
        let mut f = Feint::new();
        f.feint(0.5);
        f.tick(1.0);
        f.tick(0.016);
        assert!(!f.just_completed);
    }

    #[test]
    fn cancelled_feint_tick_returns_false() {
        let mut f = Feint::new();
        f.feint(1.0);
        f.cancel();
        assert!(!f.tick(0.016));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut f = Feint::new();
        f.feint(2.0);
        f.tick(1.0);
        assert!((f.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_idle() {
        let f = Feint::new();
        assert!((f.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_feint_no_op() {
        let mut f = Feint::new();
        f.enabled = false;
        f.feint(1.0);
        assert!(!f.is_feinting());
    }

    #[test]
    fn feint_after_re_enable() {
        let mut f = Feint::new();
        f.enabled = false;
        f.feint(1.0);
        f.enabled = true;
        f.feint(1.0);
        assert!(f.is_feinting());
    }
}

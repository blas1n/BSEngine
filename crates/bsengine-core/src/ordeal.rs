use bevy_ecs::prelude::Component;

/// Timed survival trial: when `begin(duration)` is called, an internal
/// countdown starts. If the entity survives the full duration — the caller
/// never invokes `fail()` — `tick(dt)` returns `true` and sets
/// `just_endured` when the timer reaches zero. If the entity fails (e.g. it
/// dies or a cancel condition triggers), the caller invokes `fail()`, which
/// sets `just_failed` and ends the trial early.
///
/// `begin()` is a no-op while a trial is already active, so it is safe to
/// call every frame (it won't restart a running trial). Call `reset()` to
/// force-cancel without setting `just_failed`, then `begin()` again if a
/// new trial should immediately start.
///
/// While `is_enduring()`, systems may restrict abilities, apply modifiers, or
/// show progress UI using `elapsed_fraction()`.
///
/// Distinct from `Survive` (killing-blow negation token), `Endure` (passive
/// incoming-damage reduction), and `Invincible` (full immunity window):
/// Ordeal is a **timed survival trial state** — it tracks whether the entity
/// has endured a defined period and signals success or failure, without itself
/// modifying damage or granting immunity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ordeal {
    pub duration: f32,
    pub timer: f32,
    pub just_began: bool,
    pub just_endured: bool,
    pub just_failed: bool,
    pub enabled: bool,
}

impl Ordeal {
    pub fn new() -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            just_began: false,
            just_endured: false,
            just_failed: false,
            enabled: true,
        }
    }

    /// Start a new survival trial lasting `duration` seconds. No-op if a
    /// trial is already active, `duration ≤ 0`, or disabled.
    pub fn begin(&mut self, duration: f32) {
        if !self.enabled || self.is_enduring() || duration <= 0.0 {
            return;
        }
        self.duration = duration;
        self.timer = duration;
        self.just_began = true;
    }

    /// Signal that the trial was failed (entity died, quit, etc.). Sets
    /// `just_failed` and ends the trial immediately. No-op if not active.
    pub fn fail(&mut self) {
        if !self.is_enduring() {
            return;
        }
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_failed = true;
    }

    /// Cancel the trial without signalling failure (e.g., to restart it).
    /// No-op if not active.
    pub fn reset(&mut self) {
        self.timer = 0.0;
        self.duration = 0.0;
    }

    /// Advance the trial timer. Returns `true` and sets `just_endured` the
    /// frame the trial completes successfully. Clears one-frame flags at the
    /// start of each tick.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.just_began = false;
        self.just_endured = false;
        self.just_failed = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_endured = true;
                return true;
            }
        }
        false
    }

    pub fn is_enduring(&self) -> bool {
        self.timer > 0.0
    }

    /// How far through the trial [0.0 = just started, 1.0 = complete].
    pub fn elapsed_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        let elapsed = self.duration - self.timer;
        (elapsed / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Ordeal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_starts_trial() {
        let mut o = Ordeal::new();
        o.begin(5.0);
        assert!(o.is_enduring());
        assert!(o.just_began);
    }

    #[test]
    fn begin_no_op_while_active() {
        let mut o = Ordeal::new();
        o.begin(5.0);
        o.tick(0.016);
        o.begin(10.0); // should not restart
        assert!((o.timer - (5.0 - 0.016)).abs() < 1e-4);
    }

    #[test]
    fn begin_no_op_on_zero_duration() {
        let mut o = Ordeal::new();
        o.begin(0.0);
        assert!(!o.is_enduring());
    }

    #[test]
    fn tick_completes_trial_and_returns_true() {
        let mut o = Ordeal::new();
        o.begin(1.0);
        let result = o.tick(1.1);
        assert!(result);
        assert!(o.just_endured);
        assert!(!o.is_enduring());
    }

    #[test]
    fn tick_clears_just_began() {
        let mut o = Ordeal::new();
        o.begin(5.0);
        o.tick(0.016);
        assert!(!o.just_began);
    }

    #[test]
    fn tick_returns_false_while_running() {
        let mut o = Ordeal::new();
        o.begin(5.0);
        assert!(!o.tick(0.5));
        assert!(!o.just_endured);
        assert!(o.is_enduring());
    }

    #[test]
    fn fail_ends_trial() {
        let mut o = Ordeal::new();
        o.begin(5.0);
        o.fail();
        assert!(!o.is_enduring());
        assert!(o.just_failed);
    }

    #[test]
    fn tick_clears_just_failed_set_by_fail() {
        let mut o = Ordeal::new();
        o.begin(5.0);
        o.fail();
        o.tick(0.016);
        assert!(!o.just_failed);
    }

    #[test]
    fn fail_no_op_when_not_active() {
        let mut o = Ordeal::new();
        o.fail();
        assert!(!o.just_failed);
    }

    #[test]
    fn reset_cancels_without_just_failed() {
        let mut o = Ordeal::new();
        o.begin(5.0);
        o.reset();
        assert!(!o.is_enduring());
        assert!(!o.just_failed);
    }

    #[test]
    fn elapsed_fraction_at_half() {
        let mut o = Ordeal::new();
        o.begin(4.0);
        o.tick(2.0);
        assert!((o.elapsed_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn elapsed_fraction_zero_when_inactive() {
        let o = Ordeal::new();
        assert!((o.elapsed_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_begin_no_op() {
        let mut o = Ordeal::new();
        o.enabled = false;
        o.begin(5.0);
        assert!(!o.is_enduring());
    }
}

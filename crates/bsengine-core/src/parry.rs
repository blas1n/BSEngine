use bevy_ecs::prelude::Component;

/// State of the parry timing window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParryState {
    /// No parry is active; the entity accepts hits normally.
    Idle,
    /// The parry window is open; any attack triggers a perfect parry.
    Active,
    /// The parry succeeded; the entity has a brief post-parry recovery.
    Success,
    /// The parry window closed without a hit; the entity is in recovery.
    Missed,
}

/// Tracks the parry / counter timing window for melee combat.
///
/// A parry has three timing phases:
/// 1. **Startup** (`startup_duration`): frames before the window opens.
/// 2. **Active** (`active_duration`): the parry window where hits are nullified.
/// 3. **Recovery** (`recovery_duration`): post-parry lag (success or miss).
///
/// Call `begin()` to start the sequence. `tick(dt)` advances through phases
/// and sets one-frame flags:
/// - `just_opened` — active window just opened
/// - `just_succeeded` — a parry was triggered via `notify_hit()`
/// - `just_missed` — window closed without a hit
/// - `just_finished` — recovery ended
///
/// `notify_hit()` returns `true` (and transitions to `Success`) only while
/// `state == Active`. Call this from the damage-resolution system instead of
/// applying the damage when the entity is in the parry window.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Parry {
    /// Frames/seconds before the parry window opens.
    pub startup_duration: f32,
    /// Length of the active parry window.
    pub active_duration: f32,
    /// Recovery duration after a success or miss.
    pub recovery_duration: f32,
    pub state: ParryState,
    pub timer: f32,
    /// Number of successful parries recorded.
    pub parry_count: u32,
    pub just_opened: bool,
    pub just_succeeded: bool,
    pub just_missed: bool,
    pub just_finished: bool,
    pub enabled: bool,
}

impl Parry {
    pub fn new(startup: f32, active: f32, recovery: f32) -> Self {
        Self {
            startup_duration: startup.max(0.0),
            active_duration: active.max(0.0),
            recovery_duration: recovery.max(0.0),
            state: ParryState::Idle,
            timer: 0.0,
            parry_count: 0,
            just_opened: false,
            just_succeeded: false,
            just_missed: false,
            just_finished: false,
            enabled: true,
        }
    }

    /// Begin the parry sequence (startup → active → recovery).
    /// No-op if already in progress or disabled.
    pub fn begin(&mut self) {
        if !self.enabled || self.state != ParryState::Idle {
            return;
        }
        if self.startup_duration > 0.0 {
            self.state = ParryState::Idle; // stay idle during startup
            self.timer = self.startup_duration;
        } else {
            self.state = ParryState::Active;
            self.timer = self.active_duration;
            self.just_opened = true;
        }
    }

    /// Called by the damage system. Returns `true` if the hit was parried.
    pub fn notify_hit(&mut self) -> bool {
        if self.state == ParryState::Active {
            self.state = ParryState::Success;
            self.timer = self.recovery_duration;
            self.parry_count += 1;
            self.just_succeeded = true;
            return true;
        }
        false
    }

    /// Advance the parry timer through startup → active → recovery → idle.
    pub fn tick(&mut self, dt: f32) {
        self.just_opened = false;
        self.just_succeeded = false;
        self.just_missed = false;
        self.just_finished = false;

        if !self.enabled {
            return;
        }

        match self.state {
            ParryState::Idle => {
                // If timer > 0 we're in startup.
                if self.timer > 0.0 {
                    self.timer -= dt;
                    if self.timer <= 0.0 {
                        self.state = ParryState::Active;
                        self.timer = self.active_duration;
                        self.just_opened = true;
                    }
                }
            }
            ParryState::Active => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.state = ParryState::Missed;
                    self.timer = self.recovery_duration;
                    self.just_missed = true;
                }
            }
            ParryState::Success | ParryState::Missed => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.state = ParryState::Idle;
                    self.timer = 0.0;
                    self.just_finished = true;
                }
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.state == ParryState::Active
    }

    pub fn is_idle(&self) -> bool {
        self.state == ParryState::Idle && self.timer <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_no_startup_opens_immediately() {
        let mut p = Parry::new(0.0, 0.5, 0.2);
        p.begin();
        assert_eq!(p.state, ParryState::Active);
        assert!(p.just_opened);
    }

    #[test]
    fn begin_with_startup_delays_opening() {
        let mut p = Parry::new(0.1, 0.5, 0.2);
        p.begin();
        assert_eq!(p.state, ParryState::Idle);
        p.tick(0.11);
        assert_eq!(p.state, ParryState::Active);
        assert!(p.just_opened);
    }

    #[test]
    fn notify_hit_during_active_succeeds() {
        let mut p = Parry::new(0.0, 0.5, 0.2);
        p.begin();
        let blocked = p.notify_hit();
        assert!(blocked);
        assert_eq!(p.state, ParryState::Success);
        assert!(p.just_succeeded);
        assert_eq!(p.parry_count, 1);
    }

    #[test]
    fn notify_hit_while_idle_fails() {
        let mut p = Parry::new(0.0, 0.5, 0.2);
        let blocked = p.notify_hit();
        assert!(!blocked);
    }

    #[test]
    fn active_window_expires_as_missed() {
        let mut p = Parry::new(0.0, 0.3, 0.2);
        p.begin();
        p.tick(0.31);
        assert_eq!(p.state, ParryState::Missed);
        assert!(p.just_missed);
    }

    #[test]
    fn recovery_expires_to_idle() {
        let mut p = Parry::new(0.0, 0.3, 0.2);
        p.begin();
        p.tick(0.31); // → Missed
        p.tick(0.21); // → Idle
        assert!(p.is_idle());
        assert!(p.just_finished);
    }

    #[test]
    fn begin_no_op_while_active() {
        let mut p = Parry::new(0.0, 0.5, 0.2);
        p.begin();
        p.begin(); // should be ignored
        assert!((p.timer - 0.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_begin_no_op() {
        let mut p = Parry::new(0.0, 0.5, 0.2);
        p.enabled = false;
        p.begin();
        assert!(p.is_idle());
    }
}

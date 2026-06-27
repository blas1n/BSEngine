use bevy_ecs::prelude::Component;

/// State of a sprint action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SprintState {
    /// Normal movement; no sprint active.
    Idle,
    /// Sprint is active and consuming stamina.
    Sprinting,
    /// Stamina fell below `exhaustion_threshold`; sprint locked out until
    /// stamina recovers above the threshold.
    Exhausted,
}

/// Sprint mechanic — speed boost gated by stamina.
///
/// The locomotion system reads `speed_multiplier` to scale `MoveSpeed` while
/// sprinting. The stamina system calls `notify_stamina(fraction)` every frame
/// to let `Sprint` decide whether to lock out (exhausted) or re-enable.
///
/// `begin()` / `end()` are called by player input. The component transitions:
///
/// - Idle → Sprinting on `begin()` (if enough stamina, not exhausted).
/// - Sprinting → Idle on `end()`.
/// - Sprinting → Exhausted when `notify_stamina()` sees stamina below threshold.
/// - Exhausted → Idle when `notify_stamina()` sees stamina above threshold
///   AND the entity has released the sprint key (`end()` was called).
///
/// Stamina drainage is exported via `drain_per_sec()` so the stamina system
/// knows how much to subtract each frame; `Sprint` itself does not modify the
/// `Stamina` component directly (no direct coupling).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Sprint {
    pub state: SprintState,
    /// Speed multiplier applied to `MoveSpeed` while sprinting (e.g. 1.5).
    pub speed_multiplier: f32,
    /// Stamina drain rate in units per second while sprinting.
    pub stamina_cost_per_sec: f32,
    /// Stamina fraction [0.0, 1.0] below which the entity becomes Exhausted.
    pub exhaustion_threshold: f32,
    /// True while the sprint key is being held (input state).
    pub wants_sprint: bool,
    /// True on the first frame sprinting begins.
    pub just_started: bool,
    /// True on the first frame sprinting ends (Idle or Exhausted).
    pub just_stopped: bool,
    pub enabled: bool,
}

impl Sprint {
    pub fn new(speed_multiplier: f32, stamina_cost_per_sec: f32) -> Self {
        Self {
            state: SprintState::Idle,
            speed_multiplier: speed_multiplier.max(1.0),
            stamina_cost_per_sec: stamina_cost_per_sec.max(0.0),
            exhaustion_threshold: 0.1,
            wants_sprint: false,
            just_started: false,
            just_stopped: false,
            enabled: true,
        }
    }

    pub fn with_exhaustion_threshold(mut self, fraction: f32) -> Self {
        self.exhaustion_threshold = fraction.clamp(0.0, 1.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Called by input when the sprint key is pressed.
    pub fn begin(&mut self) {
        self.wants_sprint = true;
    }

    /// Called by input when the sprint key is released.
    pub fn end(&mut self) {
        self.wants_sprint = false;
    }

    /// Called every frame with the current stamina fraction (0.0–1.0).
    ///
    /// Updates the sprint state machine based on stamina availability.
    pub fn notify_stamina(&mut self, stamina_fraction: f32) {
        self.just_started = false;
        self.just_stopped = false;

        if !self.enabled {
            return;
        }

        match self.state {
            SprintState::Idle => {
                if self.wants_sprint && stamina_fraction > self.exhaustion_threshold {
                    self.state = SprintState::Sprinting;
                    self.just_started = true;
                }
            }
            SprintState::Sprinting => {
                if !self.wants_sprint {
                    self.state = SprintState::Idle;
                    self.just_stopped = true;
                } else if stamina_fraction <= self.exhaustion_threshold {
                    self.state = SprintState::Exhausted;
                    self.just_stopped = true;
                }
            }
            SprintState::Exhausted => {
                if !self.wants_sprint && stamina_fraction > self.exhaustion_threshold {
                    self.state = SprintState::Idle;
                }
            }
        }
    }

    /// Stamina cost to deduct this frame (0 when not sprinting).
    pub fn drain_per_sec(&self) -> f32 {
        if self.state == SprintState::Sprinting {
            self.stamina_cost_per_sec
        } else {
            0.0
        }
    }

    pub fn is_sprinting(&self) -> bool {
        self.state == SprintState::Sprinting
    }

    pub fn is_exhausted(&self) -> bool {
        self.state == SprintState::Exhausted
    }

    /// The active speed multiplier (1.0 = no boost when not sprinting).
    pub fn effective_multiplier(&self) -> f32 {
        if self.is_sprinting() {
            self.speed_multiplier
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_starts_sprint() {
        let mut s = Sprint::new(1.5, 10.0);
        s.begin();
        s.notify_stamina(1.0);
        assert!(s.is_sprinting());
        assert!(s.just_started);
    }

    #[test]
    fn end_stops_sprint() {
        let mut s = Sprint::new(1.5, 10.0);
        s.begin();
        s.notify_stamina(1.0);
        s.end();
        s.notify_stamina(1.0);
        assert!(!s.is_sprinting());
        assert!(s.just_stopped);
    }

    #[test]
    fn low_stamina_causes_exhaustion() {
        let mut s = Sprint::new(1.5, 10.0).with_exhaustion_threshold(0.2);
        s.begin();
        s.notify_stamina(1.0); // start sprinting
        s.notify_stamina(0.1); // below 0.2 threshold → exhausted
        assert!(s.is_exhausted());
        assert!(s.just_stopped);
    }

    #[test]
    fn exhausted_recovers_only_after_key_release() {
        let mut s = Sprint::new(1.5, 10.0).with_exhaustion_threshold(0.2);
        s.begin();
        s.notify_stamina(1.0);
        s.notify_stamina(0.0); // exhausted (key still held)
        s.notify_stamina(1.0); // recovered but key held → stays exhausted
        assert!(s.is_exhausted());
        s.end();
        s.notify_stamina(1.0); // key released + recovered → idle
        assert_eq!(s.state, SprintState::Idle);
    }

    #[test]
    fn drain_per_sec_nonzero_only_while_sprinting() {
        let mut s = Sprint::new(1.5, 10.0);
        assert_eq!(s.drain_per_sec(), 0.0);
        s.begin();
        s.notify_stamina(1.0);
        assert!((s.drain_per_sec() - 10.0).abs() < 1e-5);
    }

    #[test]
    fn effective_multiplier_while_sprinting() {
        let mut s = Sprint::new(1.5, 10.0);
        s.begin();
        s.notify_stamina(1.0);
        assert!((s.effective_multiplier() - 1.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_blocks_sprint() {
        let mut s = Sprint::new(1.5, 10.0).disabled();
        s.begin();
        s.notify_stamina(1.0);
        assert!(!s.is_sprinting());
    }
}

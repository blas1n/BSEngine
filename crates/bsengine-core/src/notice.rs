use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Awareness state of an NPC toward a perceived threat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeState {
    /// No suspicion; operating normally.
    Unaware,
    /// Partial suspicion — NPC is looking around but not committed.
    Alert,
    /// Full suspicion — NPC actively pursues or attacks.
    Alarmed,
    /// Target lost; NPC searches the last known position.
    Searching,
}

/// NPC threat-awareness component.
///
/// The perception system calls `raise(amount)` when a stimulus (sound, visual, or
/// movement) reaches the NPC. Suspicion decays over time via `tick(dt)`. When
/// suspicion crosses `alarm_threshold`, the state transitions to `Alarmed`. When the
/// target is lost, the NPC transitions to `Searching` until `investigate_timer`
/// expires, then falls back to `Unaware`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Notice {
    pub state: NoticeState,
    /// Current suspicion level [0.0, 1.0]. Clamped on write.
    pub suspicion: f32,
    /// Rate at which suspicion decays per second when no stimuli are present.
    pub suspicion_decay_rate: f32,
    /// Suspicion level at which the NPC transitions from Unaware to Alert.
    pub alert_threshold: f32,
    /// Suspicion level at which the NPC transitions from Alert to Alarmed.
    pub alarm_threshold: f32,
    /// World-space position where the threat was last perceived.
    pub last_known_position: Vec3,
    /// Whether a valid `last_known_position` exists.
    pub has_last_known: bool,
    /// How long (seconds) the NPC has left to search before giving up.
    pub investigate_timer: f32,
    /// Total search duration before the NPC returns to Unaware.
    pub max_investigate_time: f32,
    pub enabled: bool,
}

impl Notice {
    pub fn new(alert_threshold: f32, alarm_threshold: f32, max_investigate_time: f32) -> Self {
        Self {
            state: NoticeState::Unaware,
            suspicion: 0.0,
            suspicion_decay_rate: 0.2,
            alert_threshold: alert_threshold.clamp(0.0, 1.0),
            alarm_threshold: alarm_threshold.clamp(0.0, 1.0),
            last_known_position: Vec3::ZERO,
            has_last_known: false,
            investigate_timer: 0.0,
            max_investigate_time: max_investigate_time.max(0.0),
            enabled: true,
        }
    }

    pub fn with_decay_rate(mut self, rate: f32) -> Self {
        self.suspicion_decay_rate = rate.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Raise suspicion by `amount` (clamped to [0, 1]). Updates state machine.
    /// `position` is the world-space origin of the stimulus.
    pub fn raise(&mut self, amount: f32, position: Vec3) {
        if !self.enabled {
            return;
        }
        self.suspicion = (self.suspicion + amount).clamp(0.0, 1.0);
        self.last_known_position = position;
        self.has_last_known = true;
        self.update_state();
    }

    /// Signal that the threat is no longer directly visible. Transitions to Searching.
    pub fn lose_sight(&mut self) {
        if self.state == NoticeState::Alarmed {
            self.state = NoticeState::Searching;
            self.investigate_timer = self.max_investigate_time;
        }
    }

    /// Advance the awareness timer and decay suspicion. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled {
            return;
        }

        match self.state {
            NoticeState::Searching => {
                self.investigate_timer -= dt;
                if self.investigate_timer <= 0.0 {
                    self.reset();
                }
            }
            NoticeState::Unaware | NoticeState::Alert => {
                self.suspicion = (self.suspicion - self.suspicion_decay_rate * dt).max(0.0);
                self.update_state();
            }
            NoticeState::Alarmed => {}
        }
    }

    fn update_state(&mut self) {
        self.state = if self.suspicion >= self.alarm_threshold {
            NoticeState::Alarmed
        } else if self.suspicion >= self.alert_threshold {
            NoticeState::Alert
        } else {
            NoticeState::Unaware
        };
    }

    /// Fully reset awareness to Unaware.
    pub fn reset(&mut self) {
        self.state = NoticeState::Unaware;
        self.suspicion = 0.0;
        self.has_last_known = false;
        self.investigate_timer = 0.0;
    }

    pub fn is_alarmed(&self) -> bool {
        self.state == NoticeState::Alarmed
    }

    pub fn is_searching(&self) -> bool {
        self.state == NoticeState::Searching
    }

    pub fn suspicion_fraction(&self) -> f32 {
        self.suspicion.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raise_transitions_to_alert() {
        let mut n = Notice::new(0.3, 0.8, 5.0);
        n.raise(0.5, Vec3::ZERO);
        assert_eq!(n.state, NoticeState::Alert);
    }

    #[test]
    fn raise_transitions_to_alarmed() {
        let mut n = Notice::new(0.3, 0.8, 5.0);
        n.raise(1.0, Vec3::ZERO);
        assert_eq!(n.state, NoticeState::Alarmed);
        assert!(n.is_alarmed());
    }

    #[test]
    fn tick_decays_suspicion() {
        let mut n = Notice::new(0.3, 0.8, 5.0).with_decay_rate(1.0);
        n.raise(0.5, Vec3::ZERO);
        n.tick(0.6); // 0.5 - 0.6 → 0 → Unaware
        assert_eq!(n.state, NoticeState::Unaware);
    }

    #[test]
    fn lose_sight_transitions_to_searching() {
        let mut n = Notice::new(0.3, 0.8, 5.0);
        n.raise(1.0, Vec3::ZERO);
        n.lose_sight();
        assert_eq!(n.state, NoticeState::Searching);
    }

    #[test]
    fn searching_expires_to_unaware() {
        let mut n = Notice::new(0.3, 0.8, 2.0);
        n.raise(1.0, Vec3::ZERO);
        n.lose_sight();
        n.tick(3.0); // exceeds max_investigate_time
        assert_eq!(n.state, NoticeState::Unaware);
    }

    #[test]
    fn reset_clears_all_state() {
        let mut n = Notice::new(0.3, 0.8, 5.0);
        n.raise(1.0, Vec3::new(1.0, 0.0, 2.0));
        n.reset();
        assert_eq!(n.state, NoticeState::Unaware);
        assert_eq!(n.suspicion, 0.0);
        assert!(!n.has_last_known);
    }
}

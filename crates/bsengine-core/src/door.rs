use bevy_ecs::prelude::Component;

/// Open/close state of the door.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorState {
    Closed,
    Opening,
    Open,
    Closing,
    Locked,
}

/// A door entity that can be opened, closed, or locked.
/// The animation system drives the visual using `open_progress`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Door {
    pub state: DoorState,
    /// Normalised open amount: 0.0 = fully closed, 1.0 = fully open.
    pub open_progress: f32,
    /// How fast the door moves per second (1.0 = fully open/close in 1 second).
    pub speed: f32,
    /// Whether the door automatically closes after `auto_close_delay` seconds.
    pub auto_close: bool,
    pub auto_close_delay: f32,
    /// Time the door has been fully open (used for auto-close countdown).
    pub time_open: f32,
    /// Key item ID required to unlock. `None` = no key needed.
    pub key_id: Option<String>,
    pub enabled: bool,
}

impl Door {
    pub fn new() -> Self {
        Self {
            state: DoorState::Closed,
            open_progress: 0.0,
            speed: 1.0,
            auto_close: false,
            auto_close_delay: 3.0,
            time_open: 0.0,
            key_id: None,
            enabled: true,
        }
    }

    pub fn locked(mut self, key_id: impl Into<String>) -> Self {
        self.state = DoorState::Locked;
        self.key_id = Some(key_id.into());
        self
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed.max(0.001);
        self
    }

    pub fn auto_closing(mut self, delay: f32) -> Self {
        self.auto_close = true;
        self.auto_close_delay = delay.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Try to open the door. Returns `false` if locked or disabled.
    pub fn open(&mut self) -> bool {
        if !self.enabled || self.state == DoorState::Locked {
            return false;
        }
        if self.state != DoorState::Open {
            self.state = DoorState::Opening;
            self.time_open = 0.0;
        }
        true
    }

    /// Try to close the door. No-op when locked.
    pub fn close(&mut self) -> bool {
        if !self.enabled || self.state == DoorState::Locked {
            return false;
        }
        if self.state != DoorState::Closed {
            self.state = DoorState::Closing;
        }
        true
    }

    /// Unlock with a key. Returns `true` if the key matches.
    pub fn unlock(&mut self, key_id: &str) -> bool {
        if self.key_id.as_deref() == Some(key_id) {
            self.state = DoorState::Closed;
            return true;
        }
        false
    }

    /// Advance door animation. Returns `true` when the door finishes opening or closing.
    pub fn tick(&mut self, dt: f32) -> bool {
        match self.state {
            DoorState::Opening => {
                self.open_progress = (self.open_progress + self.speed * dt).min(1.0);
                if self.open_progress >= 1.0 {
                    self.state = DoorState::Open;
                    return true;
                }
            }
            DoorState::Closing => {
                self.open_progress = (self.open_progress - self.speed * dt).max(0.0);
                if self.open_progress <= 0.0 {
                    self.state = DoorState::Closed;
                    return true;
                }
            }
            DoorState::Open if self.auto_close => {
                self.time_open += dt;
                if self.time_open >= self.auto_close_delay {
                    self.close();
                }
            }
            _ => {}
        }
        false
    }

    pub fn is_passable(&self) -> bool {
        matches!(self.state, DoorState::Open | DoorState::Opening) && self.open_progress > 0.5
    }
}

impl Default for Door {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn door_opens_on_tick() {
        let mut d = Door::new().with_speed(1.0);
        d.open();
        d.tick(0.3);
        assert!((d.open_progress - 0.3).abs() < 0.001);
        assert_eq!(d.state, DoorState::Opening);
    }

    #[test]
    fn door_fully_open_after_tick() {
        let mut d = Door::new().with_speed(2.0);
        d.open();
        let done = d.tick(1.0);
        assert!(done);
        assert_eq!(d.state, DoorState::Open);
        assert!((d.open_progress - 1.0).abs() < 0.001);
    }

    #[test]
    fn door_locked_rejects_open() {
        let mut d = Door::new().locked("gold_key");
        assert!(!d.open());
        assert!(d.unlock("gold_key"));
        assert!(d.open());
    }

    #[test]
    fn door_auto_close() {
        let mut d = Door::new().with_speed(100.0).auto_closing(1.0);
        d.open();
        d.tick(0.01);
        d.tick(1.5);
        assert_eq!(d.state, DoorState::Closing);
    }

    #[test]
    fn door_is_passable_when_half_open() {
        let mut d = Door::new().with_speed(1.0);
        d.open();
        d.tick(0.4);
        assert!(!d.is_passable());
        d.tick(0.2);
        assert!(d.is_passable());
    }
}

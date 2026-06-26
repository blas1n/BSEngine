use bevy_ecs::prelude::Component;

/// Playback state of the emote animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmoteState {
    Idle,
    Playing,
    /// Emote finished naturally; system can clean up.
    Finished,
    Cancelled,
}

/// An emote/gesture request on a character entity.
/// The animation system reads `current` and plays the matching clip, then sets
/// `state = Finished` when the clip ends.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Emote {
    /// Animation clip key of the current emote. `None` = no emote.
    pub current: Option<String>,
    pub state: EmoteState,
    /// How long the current emote has been playing in seconds.
    pub elapsed: f32,
    /// Full duration of the current emote clip (set by the animation system when it starts).
    pub duration: f32,
    /// Whether the emote loops until cancelled.
    pub looping: bool,
    /// Queue of pending emote keys (next to play after current finishes).
    pub queue: Vec<String>,
    pub enabled: bool,
}

impl Emote {
    pub fn new() -> Self {
        Self {
            current: None,
            state: EmoteState::Idle,
            elapsed: 0.0,
            duration: 0.0,
            looping: false,
            queue: Vec::new(),
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Request an emote immediately (cancels current if playing).
    pub fn play(&mut self, key: impl Into<String>) -> bool {
        if !self.enabled {
            return false;
        }
        self.current = Some(key.into());
        self.state = EmoteState::Playing;
        self.elapsed = 0.0;
        self.looping = false;
        true
    }

    /// Request a looping emote.
    pub fn play_loop(&mut self, key: impl Into<String>) -> bool {
        if self.play(key) {
            self.looping = true;
            return true;
        }
        false
    }

    /// Add an emote to the end of the queue.
    pub fn enqueue(&mut self, key: impl Into<String>) {
        self.queue.push(key.into());
    }

    /// Cancel the current emote and clear the queue.
    pub fn cancel(&mut self) {
        if self.state == EmoteState::Playing {
            self.state = EmoteState::Cancelled;
        }
        self.queue.clear();
    }

    /// Called by the animation system each frame with `dt`.
    /// Returns `true` when the current emote finishes and there is a next one ready.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.state != EmoteState::Playing || self.duration <= 0.0 {
            return false;
        }
        self.elapsed += dt;
        if self.elapsed >= self.duration {
            if self.looping {
                self.elapsed -= self.duration;
                return false;
            }
            // Advance queue.
            if let Some(next) = self.queue.first().cloned() {
                self.queue.remove(0);
                self.current = Some(next);
                self.elapsed = 0.0;
                return true;
            }
            self.state = EmoteState::Finished;
        }
        false
    }

    pub fn is_playing(&self) -> bool {
        self.state == EmoteState::Playing
    }
}

impl Default for Emote {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emote_play_sets_playing() {
        let mut e = Emote::new();
        assert!(e.play("wave"));
        assert_eq!(e.state, EmoteState::Playing);
        assert_eq!(e.current.as_deref(), Some("wave"));
    }

    #[test]
    fn emote_tick_finishes_after_duration() {
        let mut e = Emote::new();
        e.play("bow");
        e.duration = 1.0;
        e.tick(1.1);
        assert_eq!(e.state, EmoteState::Finished);
    }

    #[test]
    fn emote_looping_wraps_elapsed() {
        let mut e = Emote::new();
        e.play_loop("dance");
        e.duration = 2.0;
        e.tick(2.5);
        assert_eq!(e.state, EmoteState::Playing);
        assert!((e.elapsed - 0.5).abs() < 0.001);
    }

    #[test]
    fn emote_queue_advances() {
        let mut e = Emote::new();
        e.play("wave");
        e.enqueue("bow");
        e.duration = 1.0;
        let advanced = e.tick(1.1);
        assert!(advanced);
        assert_eq!(e.current.as_deref(), Some("bow"));
    }

    #[test]
    fn emote_disabled_rejects_play() {
        let mut e = Emote::new().disabled();
        assert!(!e.play("cheer"));
    }
}

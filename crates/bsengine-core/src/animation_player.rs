use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct AnimationPlayer {
    pub clip: String,
    pub time: f32,
    pub speed: f32,
    pub duration: f32,
    pub looping: bool,
    pub playing: bool,
}

impl AnimationPlayer {
    pub fn new(clip: impl Into<String>) -> Self {
        Self {
            clip: clip.into(),
            time: 0.0,
            speed: 1.0,
            duration: 0.0,
            looping: true,
            playing: true,
        }
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    pub fn with_looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    pub fn paused(mut self) -> Self {
        self.playing = false;
        self
    }

    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration.max(0.0);
        self
    }

    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn reset(&mut self) {
        self.time = 0.0;
    }

    pub fn is_finished(&self) -> bool {
        !self.looping && self.duration > 0.0 && self.time >= self.duration
    }

    pub fn normalized_time(&self) -> f32 {
        if self.duration <= 0.0 {
            0.0
        } else {
            (self.time / self.duration).clamp(0.0, 1.0)
        }
    }

    /// Advance playback by `dt` seconds. Called by AnimationPlugin each frame.
    pub fn tick(&mut self, dt: f32) {
        if !self.playing || self.duration <= 0.0 {
            return;
        }
        self.time += dt * self.speed;
        if self.looping {
            if self.time > self.duration {
                self.time %= self.duration;
            }
        } else if self.time >= self.duration {
            self.time = self.duration;
            self.playing = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_default_playing_looping() {
        let p = AnimationPlayer::new("walk");
        assert_eq!(p.clip, "walk");
        assert!(p.playing);
        assert!(p.looping);
        assert_eq!(p.time, 0.0);
        assert!((p.speed - 1.0).abs() < 0.001);
    }

    #[test]
    fn player_pause_and_play() {
        let mut p = AnimationPlayer::new("idle").with_duration(1.0);
        p.pause();
        assert!(!p.playing);
        p.play();
        assert!(p.playing);
    }

    #[test]
    fn player_tick_advances_time() {
        let mut p = AnimationPlayer::new("run").with_duration(2.0);
        p.tick(0.5);
        assert!((p.time - 0.5).abs() < 0.001);
    }

    #[test]
    fn player_loops_on_overflow() {
        let mut p = AnimationPlayer::new("run").with_duration(1.0);
        p.tick(1.3);
        assert!((p.time - 0.3).abs() < 0.001);
        assert!(p.playing);
    }

    #[test]
    fn player_stops_at_end_when_not_looping() {
        let mut p = AnimationPlayer::new("die")
            .with_duration(1.0)
            .with_looping(false);
        p.tick(2.0);
        assert!((p.time - 1.0).abs() < 0.001);
        assert!(!p.playing);
        assert!(p.is_finished());
    }

    #[test]
    fn player_normalized_time() {
        let mut p = AnimationPlayer::new("walk").with_duration(4.0);
        p.tick(1.0);
        assert!((p.normalized_time() - 0.25).abs() < 0.001);
    }

    #[test]
    fn player_paused_does_not_tick() {
        let mut p = AnimationPlayer::new("idle").with_duration(1.0).paused();
        p.tick(0.5);
        assert_eq!(p.time, 0.0);
    }

    #[test]
    fn player_respects_speed_multiplier() {
        let mut p = AnimationPlayer::new("fast")
            .with_duration(4.0)
            .with_speed(2.0);
        p.tick(1.0);
        assert!((p.time - 2.0).abs() < 0.001);
    }
}

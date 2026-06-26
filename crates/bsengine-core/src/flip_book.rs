use bevy_ecs::prelude::Component;

/// Frame-based sprite animation driven by a texture atlas.
/// The animation system advances `current_frame` each tick based on `fps`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct FlipBook {
    /// Total number of frames in the animation strip.
    pub frame_count: u32,
    /// Index of the currently displayed frame. Clamped to `[0, frame_count - 1]`.
    pub current_frame: u32,
    /// Playback speed in frames per second. 0 = paused.
    pub fps: f32,
    /// When true the animation loops; when false it stops on the last frame.
    pub looping: bool,
    /// Accumulated fractional frame time from the previous tick.
    pub accumulated: f32,
    pub enabled: bool,
}

impl FlipBook {
    pub fn new(frame_count: u32, fps: f32) -> Self {
        Self {
            frame_count: frame_count.max(1),
            current_frame: 0,
            fps: fps.max(0.0),
            looping: true,
            accumulated: 0.0,
            enabled: true,
        }
    }

    pub fn with_fps(mut self, fps: f32) -> Self {
        self.fps = fps.max(0.0);
        self
    }

    pub fn once(mut self) -> Self {
        self.looping = false;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns `true` if the animation has reached the last frame and is not looping.
    pub fn is_finished(&self) -> bool {
        !self.looping && self.current_frame + 1 >= self.frame_count
    }

    /// Advance the animation by `dt` seconds. Returns whether the frame changed.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.enabled || self.fps == 0.0 || self.is_finished() {
            return false;
        }
        self.accumulated += dt * self.fps;
        if self.accumulated < 1.0 {
            return false;
        }
        let steps = self.accumulated as u32;
        self.accumulated -= steps as f32;
        if self.looping {
            self.current_frame = (self.current_frame + steps) % self.frame_count;
        } else {
            self.current_frame = (self.current_frame + steps).min(self.frame_count - 1);
        }
        true
    }
}

impl Default for FlipBook {
    fn default() -> Self {
        Self::new(1, 12.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flip_book_defaults() {
        let fb = FlipBook::default();
        assert_eq!(fb.frame_count, 1);
        assert!((fb.fps - 12.0).abs() < 0.001);
        assert!(fb.looping);
        assert!(fb.enabled);
    }

    #[test]
    fn tick_advances_frame() {
        let mut fb = FlipBook::new(4, 4.0);
        let changed = fb.tick(0.3);
        assert!(changed);
        assert_eq!(fb.current_frame, 1);
    }

    #[test]
    fn tick_loops() {
        let mut fb = FlipBook::new(4, 4.0);
        for _ in 0..4 {
            fb.tick(0.25);
        }
        assert_eq!(fb.current_frame, 0);
    }

    #[test]
    fn once_stops_at_last_frame() {
        let mut fb = FlipBook::new(4, 4.0).once();
        for _ in 0..10 {
            fb.tick(0.25);
        }
        assert_eq!(fb.current_frame, 3);
        assert!(fb.is_finished());
    }

    #[test]
    fn disabled_does_not_advance() {
        let mut fb = FlipBook::new(4, 4.0).disabled();
        fb.tick(1.0);
        assert_eq!(fb.current_frame, 0);
    }
}

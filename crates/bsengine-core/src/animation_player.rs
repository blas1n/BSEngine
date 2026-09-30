use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Plays a single named animation clip on an entity, advancing its own
/// playback time each frame via [`AnimationPlayer::tick`].
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct AnimationPlayer {
    /// Name/identifier of the clip currently assigned to this player.
    pub clip: String,
    /// Current playback position, in seconds since the clip started.
    pub time: f32,
    /// Playback rate multiplier (1.0 = normal speed).
    pub speed: f32,
    /// Total length of the clip, in seconds.
    pub duration: f32,
    /// Whether playback wraps back to the start after reaching `duration`.
    pub looping: bool,
    /// Whether playback is currently advancing.
    pub playing: bool,
}

impl AnimationPlayer {
    /// Creates a player for the given clip, starting at time 0, playing, and looping.
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

    /// Sets the playback rate multiplier.
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// Sets whether the clip loops when it reaches the end.
    pub fn with_looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// Starts the player in a paused state.
    pub fn paused(mut self) -> Self {
        self.playing = false;
        self
    }

    /// Sets the clip duration, clamped to be non-negative.
    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration.max(0.0);
        self
    }

    /// Resumes playback.
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Halts playback without resetting the current time.
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Rewinds playback to the start of the clip.
    pub fn reset(&mut self) {
        self.time = 0.0;
    }

    /// Returns true once a non-looping clip has played through to its end.
    pub fn is_finished(&self) -> bool {
        !self.looping && self.duration > 0.0 && self.time >= self.duration
    }

    /// Returns the current playback position as a fraction of `duration`, clamped to `[0, 1]`.
    pub fn normalized_time(&self) -> f32 {
        if self.duration <= 0.0 {
            0.0
        } else {
            (self.time / self.duration).clamp(0.0, 1.0)
        }
    }

    /// Advance playback by `dt` seconds. Called by AnimationPlugin each frame.
    pub fn tick(&mut self, dt: f32) {
        self.tick_spans(dt);
    }

    /// Advances playback by `dt` like [`Self::tick`], and returns the stretches
    /// of clip time the playhead passed over, in the order it passed them --
    /// what decides which animation events fire.
    ///
    /// Each span is `(from, to)` in the direction of travel: `from < to`
    /// playing forward, `from > to` in reverse. A time `t` is crossed by a
    /// span when it lies strictly past `from` and at-or-before `to` in that
    /// direction -- except that a span *starting* at a clip end (0 forward,
    /// `duration` backward) includes that end, so an event there fires when
    /// playback leaves it. A loop that wraps yields one span up to the end
    /// and one from the other end; a frame long enough to lap the clip yields
    /// one span per lap, capped at a few laps so a zero-length-ish clip
    /// cannot stall the frame.
    ///
    /// Also fixes reverse looping, which used to run the time below zero
    /// forever: a looping clip wraps in both directions.
    pub fn tick_spans(&mut self, dt: f32) -> Vec<(f32, f32)> {
        let mut spans = Vec::new();
        if !self.playing || self.duration <= 0.0 {
            return spans;
        }
        let d = self.duration;
        let mut remaining = dt * self.speed;
        let forward = remaining >= 0.0;
        let mut pos = self.time.clamp(0.0, d);
        // A few laps at most: past that, every event has already fired this
        // frame, and firing each one a hundred more times helps no one.
        for _ in 0..4 {
            if remaining == 0.0 {
                break;
            }
            let room = if forward { d - pos } else { pos };
            let step = remaining.abs().min(room);
            let next = if forward { pos + step } else { pos - step };
            spans.push((pos, next));
            remaining -= if forward { step } else { -step };
            pos = next;
            let at_end = if forward { pos >= d } else { pos <= 0.0 };
            if !at_end || remaining == 0.0 {
                break;
            }
            if !self.looping {
                break;
            }
            pos = if forward { 0.0 } else { d };
        }
        if self.looping {
            // Landing exactly on the end is the start of the next lap, as
            // `%` always had it; mid-clip positions are untouched.
            self.time = if forward && pos >= d {
                0.0
            } else if !forward && pos <= 0.0 {
                d
            } else {
                pos
            };
        } else {
            self.time = pos;
            let finished = if forward { pos >= d } else { pos <= 0.0 };
            if finished {
                self.playing = false;
            }
        }
        spans
    }

    /// Whether a span from [`Self::tick_spans`] crosses clip time `t`.
    pub fn span_crosses(&self, span: (f32, f32), t: f32) -> bool {
        let (from, to) = span;
        if from <= to {
            let starts_at_end = from == 0.0;
            (t > from || (starts_at_end && t == 0.0)) && t <= to
        } else {
            let starts_at_end = from == self.duration;
            (t < from || (starts_at_end && t == self.duration)) && t >= to
        }
    }
}

impl Default for AnimationPlayer {
    fn default() -> Self {
        Self::new("")
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

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5
    }

    /// A loop that wraps reports both halves, in playback order, and lands
    /// where `%` would.
    #[test]
    fn a_wrapping_loop_reports_both_halves() {
        let mut p = AnimationPlayer::new("walk").with_duration(1.0);
        p.time = 0.9;
        let spans = p.tick_spans(0.2);
        assert_eq!(spans.len(), 2, "{spans:?}");
        assert!(
            close(spans[0], (0.9, 1.0)) && close(spans[1], (0.0, 0.1)),
            "{spans:?}"
        );
        assert!((p.time - 0.1).abs() < 1e-5);
    }

    /// Reverse looping wraps too -- it used to run the time below zero and
    /// never come back -- and its spans run backwards.
    #[test]
    fn a_reversed_loop_wraps_backwards() {
        let mut p = AnimationPlayer::new("walk")
            .with_duration(1.0)
            .with_speed(-1.0);
        p.time = 0.1;
        let spans = p.tick_spans(0.2);
        assert!(
            close(spans[0], (0.1, 0.0)) && close(spans[1], (1.0, 0.9)),
            "{spans:?}"
        );
        assert!(
            (p.time - 0.9).abs() < 1e-5,
            "wrapped to 0.9, got {}",
            p.time
        );
    }

    /// Landing exactly on the end of a loop is the start of the next lap, in
    /// both directions: forward onto `duration` reads 0, backward onto 0
    /// reads `duration`. (The two name the same moment of a loop, but only
    /// one of them lets the next frame's span start at a clip end and fire
    /// the event there once, not twice.)
    #[test]
    fn a_loop_landing_exactly_on_its_end_starts_the_next_lap() {
        let mut p = AnimationPlayer::new("walk").with_duration(1.0);
        p.time = 0.5;
        p.tick(0.5);
        assert_eq!(p.time, 0.0, "forward onto the end");

        let mut p = AnimationPlayer::new("walk")
            .with_duration(1.0)
            .with_speed(-1.0);
        p.time = 0.5;
        p.tick(0.5);
        assert_eq!(p.time, 1.0, "backward onto the start");
    }

    /// Non-looping in reverse stops at the start, as forward stops at the end.
    #[test]
    fn a_reversed_one_shot_stops_at_zero() {
        let mut p = AnimationPlayer::new("die")
            .with_duration(1.0)
            .with_looping(false)
            .with_speed(-1.0);
        p.time = 0.3;
        p.tick(1.0);
        assert_eq!(p.time, 0.0);
        assert!(!p.playing);
    }

    /// A frame longer than several clips reports a span per lap, capped, so
    /// a tiny clip cannot turn one frame into thousands.
    #[test]
    fn laps_are_reported_and_capped() {
        let mut p = AnimationPlayer::new("spin").with_duration(1.0);
        let spans = p.tick_spans(2.5);
        assert_eq!(spans.len(), 3, "two full laps and a half: {spans:?}");
        let mut tiny = AnimationPlayer::new("buzz").with_duration(0.001);
        assert!(tiny.tick_spans(10.0).len() <= 4);
    }

    /// A span includes its far end, excludes its near end -- except a near
    /// end that is the clip's own end, so an event at 0 fires as playback
    /// leaves 0.
    #[test]
    fn span_crossing_includes_the_far_end_and_a_start_at_zero() {
        let p = AnimationPlayer::new("walk").with_duration(1.0);
        assert!(p.span_crosses((0.2, 0.5), 0.5));
        assert!(!p.span_crosses((0.2, 0.5), 0.2));
        assert!(
            p.span_crosses((0.0, 0.1), 0.0),
            "leaving 0 fires an event at 0"
        );
        assert!(
            p.span_crosses((0.5, 0.2), 0.2),
            "backwards: the far end is 0.2"
        );
        assert!(!p.span_crosses((0.5, 0.2), 0.5));
        assert!(p.span_crosses((1.0, 0.9), 1.0), "leaving the end backwards");
    }
}

use bevy_ecs::prelude::Component;
use kira::sound::static_sound::StaticSoundData;

/// Holds pre-loaded audio data.  Attach together with [`AudioPlayer`] to trigger playback.
#[derive(Component, Clone)]
pub struct AudioSource {
    /// The decoded/streamed sound data to be played.
    pub data: StaticSoundData,
}

impl AudioSource {
    /// Wraps pre-loaded sound data in an [`AudioSource`] component.
    pub fn new(data: StaticSoundData) -> Self {
        Self { data }
    }
}

/// Configures playback for an [`AudioSource`].  Adding this component triggers the audio system
/// to start playing; remove it to stop.
#[derive(Component, Debug, Clone)]
pub struct AudioPlayer {
    /// Linear volume multiplier (1.0 = full, 0.0 = silent).
    pub volume: f64,
    /// Whether playback should loop.
    pub looping: bool,
    /// Playback rate multiplier (1.0 = normal speed).
    pub playback_rate: f64,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self {
            volume: 1.0,
            looping: false,
            playback_rate: 1.0,
        }
    }
}

impl AudioPlayer {
    /// Creates an [`AudioPlayer`] with the default volume, looping, and playback rate.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the linear volume multiplier (1.0 = full, 0.0 = silent).
    pub fn with_volume(mut self, volume: f64) -> Self {
        self.volume = volume;
        self
    }

    /// Enables looping playback.
    pub fn with_looping(mut self) -> Self {
        self.looping = true;
        self
    }

    /// Sets the playback rate multiplier (1.0 = normal speed).
    pub fn with_playback_rate(mut self, rate: f64) -> Self {
        self.playback_rate = rate;
        self
    }
}

/// Current playback state — written by the audio system after each update.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    /// The sound is actively producing output.
    #[default]
    Playing,
    /// Playback is suspended and can be resumed from the current position.
    Paused,
    /// Playback has ended, either naturally or because it was stopped explicitly.
    Stopped,
}

/// Internal: kira handle stored per entity after playback starts.
#[derive(Component)]
pub(crate) struct AudioHandle {
    pub handle: kira::sound::static_sound::StaticSoundHandle,
}

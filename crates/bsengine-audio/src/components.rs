use bevy_ecs::prelude::Component;
use kira::sound::static_sound::StaticSoundData;

/// Holds pre-loaded audio data.  Attach together with [`AudioPlayer`] to trigger playback.
#[derive(Component, Clone)]
pub struct AudioSource {
    pub data: StaticSoundData,
}

impl AudioSource {
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_volume(mut self, volume: f64) -> Self {
        self.volume = volume;
        self
    }

    pub fn with_looping(mut self) -> Self {
        self.looping = true;
        self
    }

    pub fn with_playback_rate(mut self, rate: f64) -> Self {
        self.playback_rate = rate;
        self
    }
}

/// Current playback state — written by the audio system after each update.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    #[default]
    Playing,
    Paused,
    Stopped,
}

/// Internal: kira handle stored per entity after playback starts.
#[derive(Component)]
pub(crate) struct AudioHandle {
    pub handle: kira::sound::static_sound::StaticSoundHandle,
}

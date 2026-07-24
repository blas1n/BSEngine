use bevy_ecs::prelude::Resource;
use kira::{
    sound::static_sound::{StaticSoundData, StaticSoundHandle},
    AudioManager, AudioManagerSettings, DefaultBackend,
};

/// ECS resource wrapping the `kira` audio manager; `None` if audio backend init failed.
#[derive(Resource)]
pub struct AudioWorld {
    manager: Option<AudioManager<DefaultBackend>>,
}

impl Default for AudioWorld {
    fn default() -> Self {
        match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
            Ok(manager) => Self {
                manager: Some(manager),
            },
            Err(e) => {
                tracing::warn!("Audio backend init failed ({e}) — running without audio");
                Self { manager: None }
            }
        }
    }
}

impl AudioWorld {
    /// Returns whether the audio backend initialized successfully and can play sounds.
    pub fn is_available(&self) -> bool {
        self.manager.is_some()
    }

    /// Starts playing the given sound data, returning a handle to it, or `None` if the audio
    /// backend is unavailable or playback failed to start.
    pub fn play(&mut self, data: StaticSoundData) -> Option<StaticSoundHandle> {
        self.manager.as_mut()?.play(data).ok()
    }
}

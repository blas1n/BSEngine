use bevy_ecs::prelude::Resource;
use kira::{
    sound::static_sound::{StaticSoundData, StaticSoundHandle},
    AudioManager, AudioManagerSettings, DefaultBackend,
};

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
    pub fn is_available(&self) -> bool {
        self.manager.is_some()
    }

    pub(crate) fn play(&mut self, data: StaticSoundData) -> Option<StaticSoundHandle> {
        self.manager.as_mut()?.play(data).ok()
    }
}

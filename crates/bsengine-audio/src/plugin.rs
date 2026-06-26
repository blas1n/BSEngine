use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use kira::sound::static_sound::StaticSoundData;

use crate::{
    components::{AudioHandle, AudioPlayer, AudioSource, PlaybackState},
    world::AudioWorld,
};

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AudioWorld::default());
        app.add_systems(Update, (start_playback, sync_state).chain());
    }
}

fn start_playback(
    mut world: ResMut<AudioWorld>,
    mut commands: Commands,
    query: Query<(Entity, &AudioSource, &AudioPlayer), Without<AudioHandle>>,
) {
    for (entity, source, player) in query.iter() {
        let data = apply_player_settings(source.data.clone(), player);
        if let Some(handle) = world.play(data) {
            commands
                .entity(entity)
                .insert((AudioHandle { handle }, PlaybackState::Playing));
        }
    }
}

fn apply_player_settings(data: StaticSoundData, player: &AudioPlayer) -> StaticSoundData {
    use kira::{Decibels, PlaybackRate};

    let volume_db = 20.0 * player.volume.max(1e-10).log10();
    let data = data.volume(Decibels(volume_db as f32));
    let data = data.playback_rate(PlaybackRate(player.playback_rate));
    if player.looping {
        data.loop_region(..)
    } else {
        data
    }
}

fn sync_state(mut query: Query<(&mut PlaybackState, &AudioHandle)>) {
    for (mut state, handle) in query.iter_mut() {
        let kira_state = handle.handle.state();
        let new_state = match kira_state {
            kira::sound::PlaybackState::Playing
            | kira::sound::PlaybackState::Pausing
            | kira::sound::PlaybackState::Resuming
            | kira::sound::PlaybackState::Stopping => PlaybackState::Playing,
            kira::sound::PlaybackState::Paused | kira::sound::PlaybackState::WaitingToResume => {
                PlaybackState::Paused
            }
            kira::sound::PlaybackState::Stopped => PlaybackState::Stopped,
        };
        if *state != new_state {
            *state = new_state;
        }
    }
}

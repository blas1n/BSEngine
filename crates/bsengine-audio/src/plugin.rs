use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use kira::sound::static_sound::StaticSoundData;

use crate::{
    components::{AudioHandle, AudioPlayer, AudioSource, PlaybackState},
    world::AudioWorld,
};

/// Registers the [`AudioWorld`] resource and the systems that start and track sound playback.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AudioWorld::default());
        // R1: a public component has to be reachable by the Inspector, by
        // `set_reflected_component`, and by a scene's `components:` list.
        // Registered from this plugin rather than from
        // `bsengine_scene::register_gameplay_reflect_types` because
        // `bsengine-scene` does not depend on this crate; `AudioPlugin` is in
        // both the windowed runtime and the headless `--test` app, so the
        // parity that function exists to protect still holds.
        //
        // `AudioSource` is deliberately absent: its `data` is a kira
        // `StaticSoundData`, a foreign type with no `Reflect` impl.
        app.register_type::<AudioPlayer>();
        app.register_type::<PlaybackState>();
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

pub mod components;
pub mod plugin;
pub mod world;

pub use components::{AudioPlayer, AudioSource, PlaybackState};
pub use plugin::AudioPlugin;
pub use world::AudioWorld;

#[cfg(test)]
mod tests {
    use bevy_app::prelude::*;

    use super::*;

    #[test]
    fn audio_player_defaults() {
        let player = AudioPlayer::default();
        assert!((player.volume - 1.0).abs() < f64::EPSILON);
        assert!(!player.looping);
        assert!((player.playback_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn audio_player_builder_methods() {
        let player = AudioPlayer::new()
            .with_volume(0.5)
            .with_looping()
            .with_playback_rate(2.0);
        assert!((player.volume - 0.5).abs() < f64::EPSILON);
        assert!(player.looping);
        assert!((player.playback_rate - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn playback_state_default_is_playing() {
        let state = PlaybackState::default();
        assert_eq!(state, PlaybackState::Playing);
    }

    #[test]
    fn audio_world_default_does_not_panic() {
        // On CI there may be no audio device — AudioWorld must degrade gracefully.
        let world = AudioWorld::default();
        // availability depends on the runner, but it must not panic
        let _ = world.is_available();
        // Leak to avoid WASAPI teardown crash on Windows (COM from worker thread).
        std::mem::forget(world);
    }

    #[test]
    fn audio_plugin_builds() {
        let mut app = App::new();
        app.add_plugins(AudioPlugin);
        // plugin registered the resource without panicking
        assert!(app.world().contains_resource::<AudioWorld>());
        // Leak the app to avoid AudioManager's COM teardown crashing on Windows CI.
        std::mem::forget(app);
    }
}

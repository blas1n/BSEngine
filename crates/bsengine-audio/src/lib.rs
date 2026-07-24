//! Audio playback for BSEngine, built on the `kira` audio engine.
//!
//! `AudioPlugin` adds an `AudioWorld` resource that manages playback.
//! `AudioSource`/`AudioPlayer` are the ECS-facing components, with
//! `PlaybackState` tracking whether a sound is playing, paused, or stopped.
#![warn(missing_docs)]

/// ECS components for attaching and configuring audio playback on entities.
pub mod components;
/// The Bevy [`AudioPlugin`] and the systems that drive playback each frame.
pub mod plugin;
/// The [`AudioWorld`] resource wrapping the underlying `kira` audio manager.
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

    // kira initializes WASAPI/COM on a background thread; creating or dropping
    // AudioManager on Windows CI (no virtual audio device) causes
    // STATUS_ACCESS_VIOLATION.  The graceful-degrade path is covered on Linux.
    #[test]
    #[cfg_attr(target_os = "windows", ignore)]
    fn audio_world_default_does_not_panic() {
        let world = AudioWorld::default();
        let _ = world.is_available();
    }

    #[test]
    #[cfg_attr(target_os = "windows", ignore)]
    fn audio_plugin_builds() {
        let mut app = App::new();
        app.add_plugins(AudioPlugin);
        assert!(app.world().contains_resource::<AudioWorld>());
    }
}

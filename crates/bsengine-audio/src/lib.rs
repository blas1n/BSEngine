//! Audio playback for BSEngine, built on the `kira` audio engine.
//!
//! This crate offers two things and no ECS components:
//!
//! * [`AudioWorld`] — a resource wrapping the `kira` audio manager, inserted by
//!   [`AudioPlugin`]. Callers hand it decoded sound data and get back a `kira`
//!   handle to pause, resume, stop, or query.
//! * [`AudioSourceAsset`] and [`AudioSourceLoader`] — the `bevy_asset` type for
//!   decoded sample data and the loader that reads `wav`/`ogg`/`mp3`/`flac`
//!   from disk.
//!
//! Playback is driven imperatively by `bsengine-scripting`, which owns the
//! requested-play queue and the live sound handles and calls
//! [`AudioWorld::play`] once an asset finishes loading. There is deliberately no
//! "attach a component to make noise" path: an earlier one existed, was never
//! reachable from any game, script, or editor surface, and was removed rather
//! than left as a second way to do the same thing.
#![warn(missing_docs)]

/// Decoded audio sample data asset ([`AudioSourceAsset`]) and its loader.
pub mod audio_source;
/// The Bevy [`AudioPlugin`], which inserts the [`AudioWorld`] resource.
pub mod plugin;
/// The [`AudioWorld`] resource wrapping the underlying `kira` audio manager.
pub mod world;

pub use audio_source::{load_audio_source, AudioSourceAsset, AudioSourceLoader};
pub use plugin::AudioPlugin;
pub use world::AudioWorld;

#[cfg(test)]
mod tests {
    use bevy_app::prelude::*;

    use super::*;

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

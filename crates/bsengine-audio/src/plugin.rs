use bevy_app::prelude::*;

use crate::world::AudioWorld;

/// Registers the [`AudioWorld`] resource that owns the `kira` audio manager.
///
/// This plugin adds no systems. Sound is driven imperatively rather than by an
/// ECS component pair: `bsengine-scripting` holds the queue of requested plays
/// (`SoundLoads`/`PendingSounds`) and the live handles (`SoundHandles`), and
/// calls [`AudioWorld::play`] directly when a decoded
/// [`AudioSourceAsset`](crate::AudioSourceAsset) becomes available. Adding
/// audio components here would duplicate that path, not extend it.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AudioWorld::default());
    }
}

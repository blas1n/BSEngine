//! "This file under `assets/` changed on disk", for the consumers hot reload
//! cannot reach through `bevy_asset`.

use bevy_ecs::message::Message;

/// A file under the project's `assets/` directory was written (or created, or
/// renamed into place) while the game ran. Sent by the asset watcher
/// (`AssetWatcherPlugin`) once per file per drain, for **every** file -- not
/// only the extensions `bevy_asset` reloads.
///
/// `AssetServer::reload` is the whole of hot reload for an asset `bevy_asset`
/// loaded, and every consumer of one already rebuilds from
/// `AssetEvent::Modified`. That leaves the files the engine reads itself, with
/// `pak_source::read_to_string`, into a cache of its own -- behaviour trees
/// (`.bt.ron`) first. Nothing told such a cache that its file changed, so an
/// edit took a restart. This is that notice; a consumer compares `path`
/// against the paths it read and re-reads the ones it holds.
///
/// `path` is in the engine form a component holds once
/// `resolve_project_path` has joined it onto `ProjectDir`
/// (`<ProjectDir>/assets/ai/guard.bt.ron`, forward slashes) -- the spelling such
/// a cache is keyed by. A deleted file sends nothing: what a cache already
/// holds is the last good version, and keeping it is the better failure.
///
/// No watcher, no messages: a `--test` replay, a browser build and a packaged
/// game do not watch. A consumer registers the message itself (`add_message`
/// is idempotent), so its reader works in those hosts too and simply never
/// sees one.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct AssetFileChanged {
    /// The changed file, engine form.
    pub path: String,
}

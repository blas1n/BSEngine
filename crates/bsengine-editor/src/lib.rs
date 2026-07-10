// Bevy ECS system params (Query<(A, B, C, ...)>, ParamSet<(...)>) routinely
// exceed clippy's type-complexity threshold; that's the idiom, not a real
// complexity problem. Bevy itself disables this lint crate-wide for the
// same reason.
#![allow(clippy::type_complexity)]

pub mod plugin;
pub mod snapshot;

pub use plugin::EditorPlugin;
pub use snapshot::{EditorCommand, EditorSnapshot, EditorSnapshotResource, EntityInfo};

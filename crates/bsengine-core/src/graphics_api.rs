//! Which graphics APIs a game may draw with on Android, in order.

use bevy_ecs::prelude::Resource;

/// A graphics API the renderer can draw with on Android.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphicsApi {
    /// Vulkan: what every Android 10+ 64-bit device ships.
    Vulkan,
    /// OpenGL ES 3.1 or later.
    Gles,
}

/// The graphics APIs an Android build tries, first to last, from
/// `project.toml`'s `[android] graphics_apis` (default `["vulkan", "gles"]`).
///
/// The first one the device can make a renderer with is used. `["gles"]`
/// alone is for a device whose Vulkan driver is broken -- the case Unity's
/// and Unreal's Vulkan deny-lists send to GLES -- and for testing the GLES
/// path on a device that has both. Unity's Player Settings keep the same
/// ordered list ("Graphics APIs"), Unreal a checkbox per API, Godot a driver
/// with a GLES fallback.
///
/// Read once, when the window's renderer is made. Desktop builds ignore it:
/// there `WGPU_BACKEND` chooses, as wgpu's own examples do.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct GraphicsApis(pub Vec<GraphicsApi>);

impl Default for GraphicsApis {
    fn default() -> Self {
        Self(vec![GraphicsApi::Vulkan, GraphicsApi::Gles])
    }
}

//! The project's MSAA setting.

use bevy_ecs::prelude::Resource;

/// Multisample antialiasing for the scene's geometry, from `project.toml`'s
/// `[render] msaa`: a project setting, as Unity's URP asset and Godot's
/// `rendering/anti_aliasing/quality/msaa_3d` make it, not a per-camera one.
///
/// `samples` above 1 turns MSAA on at the renderer's one supported count,
/// 4x (the count WebGPU guarantees); 1 is off. An adapter that cannot
/// multisample the scene's targets renders without it.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsaaSettings {
    /// Requested samples per pixel; 1 is off.
    pub samples: u32,
}

impl Default for MsaaSettings {
    /// Off -- what every project that does not ask for MSAA renders with.
    fn default() -> Self {
        Self { samples: 1 }
    }
}

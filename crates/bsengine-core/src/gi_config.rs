//! The project's realtime GI budget.

use bevy_ecs::prelude::Resource;

/// Rays the realtime light-probe volume may trace per frame, from
/// `project.toml`'s `[render] gi_rays_per_frame`. A volume whose probes need
/// more (probes x `rays_per_probe`) updates a slice of them each frame, round
/// robin, so the cost per frame stays put and a large grid refreshes over
/// several frames instead -- the budget DDGI implementations expose as
/// "probes updated per frame".
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GiSettings {
    /// Rays per frame.
    pub rays_per_frame: u32,
}

/// 2^18: 4096 probes at 64 rays each, the whole largest grid every frame.
pub const DEFAULT_GI_RAYS_PER_FRAME: u32 = 1 << 18;

impl Default for GiSettings {
    fn default() -> Self {
        Self {
            rays_per_frame: DEFAULT_GI_RAYS_PER_FRAME,
        }
    }
}

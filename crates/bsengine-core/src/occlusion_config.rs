//! Runtime switch for occlusion culling, set from `project.toml`.

use bevy_ecs::prelude::Resource;

/// Whether `render_frame` should perform occlusion culling this run.
///
/// Inserted by the runtime from `project.toml`'s `[render]` section. When
/// the resource is **absent** -- the editor, and every test that builds an
/// app directly -- occlusion culling is enabled, matching the frustum
/// culling that has always run unconditionally. Only an explicit
/// `occlusion_culling = false` turns it off.
#[derive(Resource, Debug, Clone, Copy)]
pub struct OcclusionCullingEnabled(pub bool);

impl Default for OcclusionCullingEnabled {
    fn default() -> Self {
        Self(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_enabled() {
        assert!(
            OcclusionCullingEnabled::default().0,
            "absent config must mean enabled, like frustum culling"
        );
    }
}

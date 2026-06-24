use crate::rhi::WgpuRHI;
use bevy_app::{App, Plugin};
use bsengine_ecs::Resource;
use bsengine_rhi::RHI;
use std::sync::Arc;

#[derive(Resource)]
pub struct RhiResource(pub Arc<dyn RHI>);

pub struct WgpuRHIPlugin;

impl Plugin for WgpuRHIPlugin {
    fn build(&self, app: &mut App) {
        let rhi =
            pollster::block_on(WgpuRHI::new_headless()).expect("Failed to initialize WgpuRHI");
        app.insert_resource(RhiResource(Arc::new(rhi)));
    }
}

#[cfg(test)]
mod tests {
    use super::{RhiResource, WgpuRHIPlugin};

    use bsengine_app::new_app;

    #[test]
    fn wgpu_rhi_plugin_registers_rhi_resource() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin);
        assert!(app.world().get_resource::<RhiResource>().is_some());
    }

    #[test]
    fn rhi_resource_can_create_mesh() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin);
        let rhi = app.world().resource::<RhiResource>();
        let _mesh = rhi.0.create_mesh();
    }
}

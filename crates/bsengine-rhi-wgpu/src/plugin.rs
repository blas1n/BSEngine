use crate::rhi::WgpuRHI;
use crate::surface::{WgpuSurface, WgpuSurfaceResource};
use bevy_app::{App, Plugin, Startup, Update};
use bevy_ecs::prelude::{EventReader, ResMut, World};
use bsengine_ecs::Resource;
use bsengine_rhi::RHI;
use bsengine_window::{WindowHandle, WindowResized};
use std::sync::Arc;

#[derive(Resource)]
pub struct RhiResource(pub Arc<dyn RHI>);

pub struct WgpuRHIPlugin;

impl Plugin for WgpuRHIPlugin {
    fn build(&self, app: &mut App) {
        let rhi =
            pollster::block_on(WgpuRHI::new_headless()).expect("Failed to initialize WgpuRHI");
        app.insert_resource(RhiResource(Arc::new(rhi)));
        app.add_event::<WindowResized>();
        app.add_systems(Startup, create_surface_system);
        app.add_systems(Update, handle_window_resize);
    }
}

fn create_surface_system(world: &mut World) {
    let handle = world.get_resource::<WindowHandle>().cloned();
    if let Some(handle) = handle {
        match pollster::block_on(WgpuSurface::new(handle.0)) {
            Ok(surface) => {
                world.insert_resource(WgpuSurfaceResource(surface));
                tracing::info!("wgpu surface ready");
            }
            Err(e) => {
                tracing::warn!("wgpu surface not created: {e}");
            }
        }
    }
}

fn handle_window_resize(
    mut events: EventReader<WindowResized>,
    surface: Option<ResMut<WgpuSurfaceResource>>,
) {
    let Some(mut surface) = surface else {
        for _ in events.read() {}
        return;
    };
    for ev in events.read() {
        surface.0.resize(ev.width, ev.height);
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

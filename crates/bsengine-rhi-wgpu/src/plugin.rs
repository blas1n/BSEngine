use crate::mesh::GpuMeshRegistry;
use crate::surface::{WgpuSurface, WgpuSurfaceResource};
use crate::texture::GpuTextureRegistry;
use bevy_app::{App, Plugin, Startup, Update};
use bevy_ecs::prelude::{EventReader, ResMut, World};
use bsengine_ecs::Resource;
use bsengine_window::{WindowHandle, WindowResized};
use std::sync::Arc;

/// ECS resource exposing the wgpu command queue, so systems outside this
/// crate (e.g. CPU-side skeletal skinning in `bsengine-gltf`) can call
/// `queue.write_buffer` without needing crate-private access to `WgpuSurface`.
#[derive(Resource)]
pub struct GpuQueueResource(pub Arc<wgpu::Queue>);

/// Where `WgpuRHIPlugin` gets its render target from.
#[derive(Clone, Copy, Debug)]
pub enum SurfaceMode {
    /// Wait for a `WindowHandle` resource (produced by `bsengine_window`'s
    /// winit event loop) and build a swapchain surface from it. A surface
    /// that fails to build only warns -- the windowed runtime keeps running
    /// with no renderer rather than crashing on a bad adapter.
    Windowed,
    /// Build an offscreen render target immediately, no window needed.
    /// Failing to get an adapter is a hard error: the headless test runtime
    /// exists specifically to render and read pixels back, so a silent
    /// no-renderer fallback here would make every pixel query fail with no
    /// clue why.
    Offscreen {
        /// Render target width, in pixels.
        width: u32,
        /// Render target height, in pixels.
        height: u32,
    },
}

/// Bevy plugin that creates the render target (swapchain or offscreen
/// texture) and wires up window-resize handling.
pub struct WgpuRHIPlugin(pub SurfaceMode);

impl WgpuRHIPlugin {
    /// A surface built from a `WindowHandle`, once one appears. The mode
    /// every call site used before offscreen rendering existed.
    pub fn windowed() -> Self {
        Self(SurfaceMode::Windowed)
    }

    /// A surface built immediately from an offscreen texture of `width` x
    /// `height` pixels, no window required. Panics at `Startup` if no
    /// adapter can rasterise -- see `SurfaceMode::Offscreen`.
    pub fn offscreen(width: u32, height: u32) -> Self {
        Self(SurfaceMode::Offscreen { width, height })
    }
}

impl Plugin for WgpuRHIPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<WindowResized>();
        let mode = self.0;
        app.add_systems(Startup, move |world: &mut World| {
            create_surface_system(world, mode)
        });
        app.add_systems(Update, handle_window_resize);
    }
}

fn create_surface_system(world: &mut World, mode: SurfaceMode) {
    let surface = match mode {
        SurfaceMode::Windowed => {
            let handle = world.get_resource::<WindowHandle>().cloned();
            match handle {
                Some(handle) => match pollster::block_on(WgpuSurface::new(handle.0)) {
                    Ok(surface) => Some(surface),
                    Err(e) => {
                        tracing::warn!("wgpu surface not created: {e}");
                        None
                    }
                },
                None => None,
            }
        }
        SurfaceMode::Offscreen { width, height } => {
            match pollster::block_on(WgpuSurface::new_offscreen(width, height)) {
                Ok(surface) => Some(surface),
                Err(e) => panic!(
                    "could not create an offscreen wgpu renderer: {e}\n\
                     The headless test runtime needs an adapter that can actually \
                     rasterise. On Linux CI that is mesa-vulkan-drivers (lavapipe); on \
                     Windows it is normally the D3D12 WARP adapter. If this environment \
                     has neither, that is the finding worth reporting -- do not silence \
                     it by skipping."
                ),
            }
        }
    };

    let Some(surface) = surface else {
        return;
    };
    let registry = GpuMeshRegistry::new(surface.device.clone());
    let tex_registry = GpuTextureRegistry::new(surface.device.clone(), surface.queue.clone());
    world.insert_resource(GpuQueueResource(surface.queue.clone()));
    world.insert_resource(WgpuSurfaceResource(surface));
    world.insert_resource(registry);
    world.insert_resource(tex_registry);
    tracing::info!("wgpu surface, mesh registry, and texture registry ready");
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
    use super::WgpuRHIPlugin;
    use crate::surface::WgpuSurfaceResource;
    use bsengine_app::new_app;

    #[test]
    fn windowed_mode_creates_no_surface_without_a_window_handle() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.update();
        assert!(
            app.world().get_resource::<WgpuSurfaceResource>().is_none(),
            "windowed mode must wait for a WindowHandle before creating a surface"
        );
    }

    #[test]
    fn offscreen_mode_creates_a_surface_with_no_window_handle() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin::offscreen(64, 64));
        app.update();
        assert!(
            app.world().get_resource::<WgpuSurfaceResource>().is_some(),
            "offscreen mode must create a WgpuSurfaceResource without a WindowHandle"
        );
    }
}

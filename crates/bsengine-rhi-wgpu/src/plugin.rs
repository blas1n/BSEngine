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

/// The runtime's lookup for mip cache files shipped inside its pak archive,
/// inserted before the surface is created; the texture registry is given it
/// in place of the loose-package directory lookup. See
/// [`crate::GpuTextureRegistry::set_shipped_mip_cache`].
#[derive(Resource)]
pub struct ShippedMipCacheResource(pub crate::texture::ShippedMipCache);

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
        /// See `WgpuSurface::is_fast_render`.
        fast_render: bool,
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
    pub fn offscreen(width: u32, height: u32, fast_render: bool) -> Self {
        Self(SurfaceMode::Offscreen {
            width,
            height,
            fast_render,
        })
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
        SurfaceMode::Offscreen {
            width,
            height,
            fast_render,
        } => match pollster::block_on(WgpuSurface::new_offscreen(width, height, fast_render)) {
            Ok(surface) => Some(surface),
            Err(e) => panic!(
                "could not create an offscreen wgpu renderer: {e}\n\
                     The headless test runtime needs an adapter that can actually \
                     rasterise. On Linux CI that is mesa-vulkan-drivers (lavapipe); on \
                     Windows it is normally the D3D12 WARP adapter. If this environment \
                     has neither, that is the finding worth reporting -- do not silence \
                     it by skipping."
            ),
        },
    };

    let Some(surface) = surface else {
        return;
    };
    let registry = GpuMeshRegistry::new(surface.device.clone());
    let mut tex_registry = GpuTextureRegistry::new(surface.device.clone(), surface.queue.clone());
    // Streamed textures' mip cache files go under the project, beside the
    // Asset Browser's thumbnail cache (`games/*/.bsengine_cache/` is
    // ignored by git); the working directory when no project is named.
    let project = world
        .get_resource::<bsengine_core::ProjectDir>()
        .filter(|p| !p.0.is_empty())
        .map_or_else(
            || std::path::PathBuf::from("."),
            |p| std::path::PathBuf::from(&p.0),
        );
    let mips = project.join(".bsengine_cache").join("mips");
    // Cache files are keyed by their pixels, so every re-import leaves the
    // previous chain behind as a file nothing opens again. Swept here, once
    // per process and before the registry reads any of them, so a file the
    // sweep removes is never one a texture already streams from.
    crate::cache_sweep::sweep_unused_files(
        &mips,
        crate::cache_sweep::UNUSED_FILE_AGE,
        std::time::SystemTime::now(),
    );
    tex_registry.set_mip_cache_root(Some(mips));
    // What the package shipped: the archive lookup a pak or single-file
    // build's runtime inserted, else the loose package's own directory --
    // which a development project simply does not have, so every lookup
    // misses and the registry encodes and caches as it always did.
    let shipped = world
        .get_resource::<ShippedMipCacheResource>()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| {
            let dir = project.join(crate::texture::SHIPPED_MIP_DIR);
            std::sync::Arc::new(move |name: &str| std::fs::read(dir.join(name)).ok())
        });
    tex_registry.set_shipped_mip_cache(Some(shipped));
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
    use super::{ShippedMipCacheResource, WgpuRHIPlugin};
    use crate::surface::WgpuSurfaceResource;
    use bsengine_app::new_app;
    use std::sync::Arc;

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

    /// Also where the texture registry's mip cache lands -- under the
    /// project's `.bsengine_cache/mips` -- and that startup sweeps it:
    /// a chain nobody has used for longer than the limit is gone before
    /// the first texture uploads, one used recently stays. One test for
    /// the three because each offscreen app costs a wgpu device, and on
    /// Windows CI those run out.
    #[test]
    fn offscreen_mode_creates_a_surface_with_no_window_handle() {
        use crate::cache_sweep::{write_aged, DAY};
        use crate::texture::GpuTextureRegistry;
        use std::time::{Duration, SystemTime};

        let project =
            std::env::temp_dir().join(format!("bse_rhi_plugin_project_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&project);
        let mips = project.join(".bsengine_cache").join("mips");
        std::fs::create_dir_all(&mips).unwrap();
        let now = SystemTime::now();
        write_aged(&mips.join("stale.mips"), &[1u8; 64], now, 30 * DAY);
        write_aged(&mips.join("used.mips"), &[1u8; 64], now, 2 * DAY);
        write_aged(&mips.join("new.mips"), &[1u8; 64], now, Duration::ZERO);

        let mut app = new_app();
        app.insert_resource(bsengine_core::ProjectDir(
            project.to_string_lossy().into_owned(),
        ));
        app.add_plugins(WgpuRHIPlugin::offscreen(64, 64, false));
        app.update();
        assert!(
            app.world().get_resource::<WgpuSurfaceResource>().is_some(),
            "offscreen mode must create a WgpuSurfaceResource without a WindowHandle"
        );
        assert_eq!(
            app.world()
                .get_resource::<GpuTextureRegistry>()
                .and_then(|r| r.mip_cache_root().map(|p| p.to_path_buf())),
            Some(mips.clone()),
            "the mip cache lives under the project's .bsengine_cache"
        );
        assert!(
            !mips.join("stale.mips").exists(),
            "a chain unused for 30 days is swept at startup"
        );
        assert!(
            mips.join("used.mips").exists() && mips.join("new.mips").exists(),
            "chains used within the limit are kept"
        );
        let _ = std::fs::remove_dir_all(&project);
    }

    /// A 64x64 BC1 texture's PNG bytes, decoded pixels and precooked file.
    fn precooked_fixture() -> (
        Vec<u8>,
        bsengine_core::TextureImportSettings,
        String,
        Vec<u8>,
    ) {
        let mut img = image::RgbaImage::new(64, 64);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba([(x * 4) as u8, (y * 4) as u8, 90, 255]);
        }
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let settings = bsengine_core::TextureImportSettings {
            compression: bsengine_core::TextureCompression::Bc1,
            ..Default::default()
        };
        let (name, bytes) = crate::texture::precook_mip_cache(&png, settings).unwrap();
        let rgba = image::load_from_memory(&png).unwrap().to_rgba8().into_raw();
        (rgba, settings, name, bytes)
    }

    /// A loose package's precooked files are found where the packager put
    /// them -- `SHIPPED_MIP_DIR` under the project -- and survive the
    /// startup sweep however old they are: they sit outside the swept cache
    /// directory, so a player back after a month does not lose what shipped
    /// with the game and encode it all again.
    #[test]
    fn a_loose_packages_shipped_mips_are_used_and_never_swept() {
        use crate::cache_sweep::{write_aged, DAY};
        use crate::texture::GpuTextureRegistry;

        let (rgba, settings, name, bytes) = precooked_fixture();
        let project =
            std::env::temp_dir().join(format!("bse_rhi_shipped_project_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&project);
        let shipped = project.join(crate::texture::SHIPPED_MIP_DIR);
        std::fs::create_dir_all(&shipped).unwrap();
        write_aged(
            &shipped.join(&name),
            &bytes,
            std::time::SystemTime::now(),
            40 * DAY,
        );

        let mut app = new_app();
        app.insert_resource(bsengine_core::ProjectDir(
            project.to_string_lossy().into_owned(),
        ));
        app.add_plugins(WgpuRHIPlugin::offscreen(64, 64, false));
        app.update();
        assert!(
            shipped.join(&name).exists(),
            "a shipped file forty days old is not swept"
        );
        let mut registry = app.world_mut().resource_mut::<GpuTextureRegistry>();
        assert!(
            registry.bc_supported(),
            "premise: block compression on this device"
        );
        registry.load_with(64, 64, &rgba, settings);
        assert_eq!(registry.encodes(), 0, "the shipped chain was used");
        let _ = std::fs::remove_dir_all(&project);
    }

    /// A pak or single-file build's runtime inserts its archive lookup
    /// before the surface exists; the registry takes that one, and finds a
    /// file no directory holds.
    #[test]
    fn the_runtimes_archive_lookup_takes_the_place_of_the_directory() {
        use crate::texture::GpuTextureRegistry;

        let (rgba, settings, name, bytes) = precooked_fixture();
        let asked = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let count = Arc::clone(&asked);
        let project =
            std::env::temp_dir().join(format!("bse_rhi_archive_project_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&project);
        let mut app = new_app();
        // A scratch project, so the cache the registry writes lands there
        // and not beside this crate.
        app.insert_resource(bsengine_core::ProjectDir(
            project.to_string_lossy().into_owned(),
        ));
        app.insert_resource(ShippedMipCacheResource(Arc::new(move |n: &str| {
            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            (n == name).then(|| bytes.clone())
        })));
        app.add_plugins(WgpuRHIPlugin::offscreen(64, 64, false));
        app.update();
        let mut registry = app.world_mut().resource_mut::<GpuTextureRegistry>();
        registry.load_with(64, 64, &rgba, settings);
        assert_eq!(
            asked.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "the archive lookup was asked"
        );
        assert_eq!(registry.encodes(), 0, "and its chain used");
        let _ = std::fs::remove_dir_all(&project);
    }
}

//! Chunked terrain: a `Terrain` entity names a heightmap; once it loads,
//! `generate_terrain_chunks` spawns one child entity per chunk (render mesh
//! + Rapier heightfield collider), using `terrain_chunking::generate_chunks`
//! for the actual geometry/heightfield split.
//!
//! The request-once / poll-every-frame / wait-for-both-asset-and-registry /
//! spawn-once control flow mirrors `bsengine-gltf`'s `load_gltf_assets`
//! (`crates/bsengine-gltf/src/plugin.rs`), which solves the identical
//! problem for glTF meshes.

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::ReflectComponent;
use bevy_reflect::Reflect;
use bsengine_asset::{AssetServer, Assets, HeightmapAsset, Polled};
use bsengine_core::{GlobalTransform, Transform};
use bsengine_ecs::{Commands, Component, Entity, Query, Res, ResMut, Without};
use bsengine_physics::{Collider, ColliderShape, PhysicsInput, RigidBody};
use bsengine_render::MeshRenderer;
use bsengine_rhi_wgpu::GpuMeshRegistry;
use glam::{Quat, Vec3};
use tracing::warn;

/// Re-exported from `bsengine-scene` rather than defined here: `Terrain`'s
/// fields are plain, RON-serializable data with no runtime-only handles (the
/// same shape as `PhysicsBodyDesc`/`PrimitiveMesh`, which live in
/// `bsengine-scene` for the identical reason), and `bsengine-editor`'s
/// `terrain_write` MCP tool needs to construct one directly. `bsengine-app`
/// depends on `bsengine-editor`, so defining `Terrain` here would put it out
/// of `bsengine-editor`'s reach without a dependency cycle. See
/// `bsengine_scene::Terrain`'s doc comment for the full rationale. This
/// module keeps `TerrainPlugin`/`generate_terrain_chunks` -- the systems that
/// actually act on the component -- since those depend on
/// `bsengine-asset`/`bsengine-physics`/`bsengine-rhi-wgpu`, which
/// `bsengine-scene` does not.
pub use bsengine_scene::Terrain;

/// Tracks a `Terrain`'s in-flight heightmap load, the same way `PendingGltf`
/// tracks a `GltfAsset`'s load in `bsengine-gltf`. A separate,
/// non-reflected component (not a field on `Terrain` itself) for the same
/// reason `PendingGltf`/`PendingShader` are separate: an `AssetSlot` isn't
/// meaningfully serializable scene state, so keeping it off the reflected
/// component keeps `Terrain`'s RON representation clean.
///
/// Private, matching `PendingGltf`'s exact precedent -- an implementation
/// detail of this module's own load-tracking, not part of any other crate's
/// API (unlike `TerrainChunksGenerated` below, which `bsengine-runtime`'s
/// tests genuinely need to observe). `AssetSlot` doesn't implement `Reflect`
/// (by design -- see above), so this type can't be registered even if it
/// were public; the component catalogue's R1 rule ("every *public*
/// `#[derive(Component)]` type must be registered") only applies to public
/// types for exactly this reason.
#[derive(Component)]
struct PendingTerrain(bsengine_asset::AssetSlot<HeightmapAsset>);

/// Marks a `Terrain` entity whose chunks have already been spawned, so
/// `generate_terrain_chunks` (which still polls every frame while a load is
/// in flight) never double-spawns. Public and reflected (registered in
/// `TerrainPlugin::build` below) because `bsengine-runtime`'s terrain
/// integration test queries for it directly to detect when generation has
/// finished.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct TerrainChunksGenerated;

/// Bevy plugin that loads each `Terrain`'s heightmap and spawns its chunk
/// entities once both the heightmap and a `GpuMeshRegistry` are available.
pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Terrain>()
            .register_type::<TerrainChunksGenerated>()
            .add_systems(Update, generate_terrain_chunks);
    }
}

/// Requests each new `Terrain`'s heightmap exactly once, polls it every
/// frame while loading (mirrors `bsengine-gltf`'s `load_gltf_assets`), and
/// once both the heightmap and a `GpuMeshRegistry` are available, runs
/// `terrain_chunking::generate_chunks` and spawns one entity per chunk.
/// `spawn_bodies` (bsengine-physics) picks up the new `RigidBody`/`Collider`
/// pairs automatically -- no new physics-sync code needed.
fn generate_terrain_chunks(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    heightmaps: Res<Assets<HeightmapAsset>>,
    mut mesh_registry: Option<ResMut<GpuMeshRegistry>>,
    mut query: Query<
        (Entity, &Terrain, &Transform, Option<&mut PendingTerrain>),
        Without<TerrainChunksGenerated>,
    >,
) {
    for (entity, terrain, transform, pending) in query.iter_mut() {
        // Request exactly once, then retain the handle. See `PendingTerrain`.
        let Some(mut pending) = pending else {
            let handle = asset_server.load::<HeightmapAsset>(terrain.heightmap_path.clone());
            commands
                .entity(entity)
                .insert(PendingTerrain(bsengine_asset::AssetSlot::from_handle(
                    handle,
                )));
            continue;
        };

        if let Polled::Failed(e) = pending.0.poll(&asset_server, &heightmaps) {
            // A failed load never resolves, so the path is dropped entirely --
            // otherwise a missing file retries silently forever.
            warn!(
                "[terrain] cannot load heightmap '{}': {e}",
                terrain.heightmap_path
            );
            commands.entity(entity).remove::<PendingTerrain>();
            continue;
        }
        // Deliberately not `Arrived`-only: the heightmap can land before
        // `GpuMeshRegistry` exists (headless test mode, or a frame before a
        // window/surface is up), and this retries every frame until the
        // registry appears too.
        let handle = pending.0.handle().clone();
        let Some(heightmap) = heightmaps.get(&handle) else {
            continue;
        };
        let Some(mesh_reg) = mesh_registry.as_mut() else {
            continue;
        };

        let params = crate::terrain_chunking::ChunkParams {
            chunk_count: terrain.chunk_count,
            chunk_size: terrain.chunk_size,
            height_scale: terrain.height_scale,
        };
        let chunks = crate::terrain_chunking::generate_chunks(heightmap, &params);

        // Rapier's heightfield shape is centered on its own local origin
        // (confirmed against `bsengine-physics`'s
        // `heightfield_collider_supports_a_dynamic_body_at_the_expected_height`
        // test, which drops a body at the horizontal center of a heightfield
        // placed at `Transform::from_position(Vec3::ZERO)` and expects it to
        // land), while `ChunkData::vertices`/`world_offset` describe the
        // chunk's *min corner* in chunk-local space (see
        // `terrain_chunking::generate_chunks`). So the physics body has to
        // sit half a chunk further in +x/+z than the render `Transform`, or
        // collision would happen `chunk_size / 2` away from what is drawn.
        let half_chunk = Vec3::new(params.chunk_size / 2.0, 0.0, params.chunk_size / 2.0);

        for chunk in chunks {
            let mesh_id = mesh_reg.register(&chunk.vertices, &chunk.indices);
            let world_min_corner =
                transform.position.0 + Vec3::new(chunk.world_offset.0, 0.0, chunk.world_offset.1);

            commands.spawn((
                Transform::from_position(world_min_corner),
                GlobalTransform::default(),
                MeshRenderer { mesh_id },
                RigidBody::fixed(),
                Collider {
                    shape: ColliderShape::Heightfield {
                        heights: chunk.heightfield_heights,
                        rows: chunk.heightfield_rows,
                        cols: chunk.heightfield_cols,
                        scale: Vec3::new(params.chunk_size, 1.0, params.chunk_size).into(),
                    },
                    restitution: 0.0,
                    friction: 0.8,
                    density: 1.0,
                    sensor: false,
                },
                PhysicsInput {
                    position: (world_min_corner + half_chunk).into(),
                    rotation: Quat::IDENTITY.into(),
                },
            ));
        }

        commands
            .entity(entity)
            .insert(TerrainChunksGenerated)
            .remove::<PendingTerrain>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_rhi_wgpu::WgpuRHIPlugin;

    /// Encodes `values` (row-major, `width * height` long) as a 16-bit
    /// grayscale PNG, the format `HeightmapAssetLoader` decodes. Mirrors
    /// `heightmap_loader`'s own `make_test_png_16bit_gray` test helper.
    fn make_test_png_16bit_gray(width: u32, height: u32, values: &[u16]) -> Vec<u8> {
        let img: image::ImageBuffer<image::Luma<u16>, Vec<u16>> =
            image::ImageBuffer::from_raw(width, height, values.to_vec())
                .expect("test fixture dimensions must match values.len()");
        let mut bytes = Vec::new();
        image::DynamicImage::ImageLuma16(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("encoding the test fixture PNG failed");
        bytes
    }

    /// Writes a small real heightmap PNG to a fresh path under the temp
    /// directory (one per call, so parallel tests don't collide) and returns
    /// its path as a `String` -- the same shape `Terrain::heightmap_path`
    /// expects, and the same "write bytes with `image` at test time rather
    /// than committing a binary fixture" approach
    /// `texture_asset_loads_async_and_becomes_available` (bsengine-asset)
    /// uses for its own PNG.
    fn write_test_heightmap(name: &str, width: u32, height: u32, values: &[u16]) -> String {
        let bytes = make_test_png_16bit_gray(width, height, values);
        let path = std::env::temp_dir().join(format!(
            "bsengine_terrain_test_{name}_{}.png",
            std::process::id()
        ));
        std::fs::write(&path, bytes).expect("write test heightmap fixture");
        path.to_str().unwrap().to_owned()
    }

    /// Inserts a real `GpuMeshRegistry`, backed by a real headless `wgpu`
    /// device -- not a stand-in, the same type the renderer uses.
    ///
    /// `WgpuRHIPlugin::windowed()` only builds one once a `WindowHandle`
    /// (created by winit) exists, so a window-less headless test has to
    /// construct it directly. Mirrors `bsengine-gltf`'s
    /// `insert_headless_gpu_registries` test helper, trimmed to only the
    /// registry `generate_terrain_chunks` actually reads -- terrain never
    /// touches textures or the physics queue resource.
    fn insert_headless_mesh_registry(app: &mut bevy_app::App) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("a headless adapter; the rest of this suite already requires one");
        let (device, _queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("bsengine-app terrain test device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .expect("headless device request");
        app.insert_resource(GpuMeshRegistry::new(std::sync::Arc::new(device)));
    }

    /// An app with everything `generate_terrain_chunks` needs: a real
    /// `AssetServer` (`AssetPlugin`), a real headless `GpuMeshRegistry`, and
    /// `TerrainPlugin` itself.
    fn test_app() -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(TerrainPlugin);
        insert_headless_mesh_registry(&mut app);
        app
    }

    /// Steps `app` until `entity` carries `TerrainChunksGenerated`, or panics
    /// after a generous frame budget -- loads finish on a background thread,
    /// so the exact landing frame isn't fixed.
    fn run_until_generated(app: &mut bevy_app::App, entity: Entity) {
        for _ in 0..200 {
            app.update();
            if app.world().get::<TerrainChunksGenerated>(entity).is_some() {
                return;
            }
        }
        panic!("terrain chunks were not generated within 200 frames");
    }

    #[test]
    fn terrain_plugin_builds_and_runs() {
        let mut app = test_app();
        app.update();
    }

    #[test]
    fn no_registry_leaves_terrain_intact() {
        // Mirrors bsengine-gltf's `no_registry_leaves_gltf_asset_intact`:
        // without a GpuMeshRegistry, the system must retry forever rather
        // than silently dropping the Terrain.
        let mut app = crate::new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(TerrainPlugin);
        let path = write_test_heightmap("no-registry", 5, 5, &[0u16; 25]);
        let e = app
            .world_mut()
            .spawn((
                Terrain {
                    heightmap_path: path,
                    chunk_count: (1, 1),
                    chunk_size: 10.0,
                    height_scale: 1.0,
                    layer0_texture_path: String::new(),
                    layer1_texture_path: String::new(),
                    layer2_texture_path: String::new(),
                    layer3_texture_path: String::new(),
                },
                Transform::default(),
            ))
            .id();
        for _ in 0..50 {
            app.update();
        }
        assert!(
            app.world().get::<Terrain>(e).is_some(),
            "Terrain should remain when GpuMeshRegistry is unavailable"
        );
        assert!(
            app.world().get::<TerrainChunksGenerated>(e).is_none(),
            "chunks must never be generated without a GpuMeshRegistry"
        );
    }

    #[test]
    fn missing_heightmap_is_given_up_on_instead_of_retried_forever() {
        let mut app = test_app();
        let e = app
            .world_mut()
            .spawn((
                Terrain {
                    heightmap_path: "definitely/not/a/real/heightmap.png".to_string(),
                    chunk_count: (1, 1),
                    chunk_size: 10.0,
                    height_scale: 1.0,
                    layer0_texture_path: String::new(),
                    layer1_texture_path: String::new(),
                    layer2_texture_path: String::new(),
                    layer3_texture_path: String::new(),
                },
                Transform::default(),
            ))
            .id();

        let mut gave_up = false;
        for _ in 0..200 {
            app.update();
            if app.world().get::<PendingTerrain>(e).is_none()
                && app.world().get::<TerrainChunksGenerated>(e).is_none()
            {
                // PendingTerrain was inserted then removed on failure -- give
                // the request frame a chance to land before checking.
                if app.world().get::<Terrain>(e).is_some() {
                    gave_up = true;
                    break;
                }
            }
        }
        assert!(
            gave_up,
            "a Terrain with an unloadable heightmap path must give up on \
             PendingTerrain, not retry it every frame forever"
        );
    }

    /// The end-to-end property this whole item exists for: a real 16-bit
    /// heightmap PNG, loaded async, turns into real chunk entities carrying a
    /// render mesh and a Rapier heightfield collider -- both derived from the
    /// same sampled height grid, per `terrain_chunking::generate_chunks`.
    #[test]
    fn terrain_spawns_the_expected_chunk_entities_with_render_and_physics_components() {
        let mut app = test_app();

        // A 9x9 heightmap divided into 2x2 chunks -- large enough that the
        // remainder-absorption path in `generate_chunks` is exercised too
        // (9 does not evenly divide by 2).
        let mut values = vec![0u16; 9 * 9];
        for (i, v) in values.iter_mut().enumerate() {
            *v = (i as u16 * 100) % u16::MAX;
        }
        let path = write_test_heightmap("chunks", 9, 9, &values);

        let chunk_count = (2u32, 2u32);
        let terrain_entity = app
            .world_mut()
            .spawn((
                Terrain {
                    heightmap_path: path,
                    chunk_count,
                    chunk_size: 8.0,
                    height_scale: 5.0,
                    layer0_texture_path: String::new(),
                    layer1_texture_path: String::new(),
                    layer2_texture_path: String::new(),
                    layer3_texture_path: String::new(),
                },
                Transform::from_position(Vec3::new(100.0, 0.0, -50.0)),
            ))
            .id();

        run_until_generated(&mut app, terrain_entity);

        let mut query = app
            .world_mut()
            .query_filtered::<Entity, bevy_ecs::prelude::With<MeshRenderer>>();
        let chunk_entities: Vec<Entity> = query.iter(app.world()).collect();

        assert_eq!(
            chunk_entities.len(),
            (chunk_count.0 * chunk_count.1) as usize,
            "expected one chunk entity per (chunk_count.0 * chunk_count.1)"
        );

        for chunk in &chunk_entities {
            assert!(
                app.world().get::<MeshRenderer>(*chunk).is_some(),
                "every chunk must carry a MeshRenderer"
            );
            let rigid_body = app.world().get::<RigidBody>(*chunk);
            assert!(
                rigid_body.is_some(),
                "every chunk must carry a RigidBody so spawn_bodies picks it up"
            );
            let collider = app
                .world()
                .get::<Collider>(*chunk)
                .expect("every chunk must carry a Collider");
            assert!(
                matches!(collider.shape, ColliderShape::Heightfield { .. }),
                "a terrain chunk's collider must be a Heightfield, got {:?}",
                collider.shape
            );
        }
    }

    /// Verifies the centering fix documented on `half_chunk` above: a
    /// dynamic ball dropped above the middle of chunk (0, 0) must come to
    /// rest at that chunk's actual sampled height, not fall through (which
    /// is what happens if the physics body is misplaced by `chunk_size / 2`
    /// relative to where the collider's height data says the surface is).
    #[test]
    fn a_dropped_body_lands_on_the_chunk_it_visually_sits_above() {
        let mut app = test_app();
        app.add_plugins(bsengine_physics::PhysicsPlugin);

        // Flat, single-chunk terrain at a known height so the expected
        // resting position is unambiguous.
        let flat_height_raw: u16 = 32768; // -> (32768 / 65535) * height_scale
        let height_scale = 20.0f32;
        let expected_height = (flat_height_raw as f32 / u16::MAX as f32) * height_scale;

        let values = vec![flat_height_raw; 4 * 4];
        let path = write_test_heightmap("flat-drop", 4, 4, &values);

        let chunk_size = 10.0;
        let terrain_entity = app
            .world_mut()
            .spawn((
                Terrain {
                    heightmap_path: path,
                    chunk_count: (1, 1),
                    chunk_size,
                    height_scale,
                    layer0_texture_path: String::new(),
                    layer1_texture_path: String::new(),
                    layer2_texture_path: String::new(),
                    layer3_texture_path: String::new(),
                },
                Transform::from_position(Vec3::ZERO),
            ))
            .id();
        run_until_generated(&mut app, terrain_entity);

        // Drop a ball above the middle of the (only) chunk -- i.e. at
        // world (chunk_size / 2, y, chunk_size / 2), matching `half_chunk`.
        let radius = 0.5;
        let drop_xz = chunk_size / 2.0;
        let start = Vec3::new(drop_xz, expected_height + 10.0, drop_xz);
        let ball = app
            .world_mut()
            .spawn((
                Transform::from_position(start),
                bsengine_physics::RigidBody::dynamic(),
                bsengine_physics::Collider::ball(radius),
                bsengine_physics::PhysicsInput {
                    position: start.into(),
                    rotation: Quat::IDENTITY.into(),
                },
            ))
            .id();

        for _ in 0..200 {
            app.update();
        }

        let y = app.world().get::<Transform>(ball).unwrap().position.0.y;
        let expected = expected_height + radius;
        assert!(
            (y - expected).abs() < 0.1,
            "expected the ball to rest at y ~= {expected} (terrain height {expected_height} \
             + radius {radius}), but it settled at y={y} -- the heightfield collider is not \
             where the rendered chunk says it is"
        );
    }
}

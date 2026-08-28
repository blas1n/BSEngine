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
use bsengine_asset::{AssetServer, Assets, HeightmapAsset, Polled, TextureAsset};
use bsengine_core::{GlobalTransform, Transform};
use bsengine_ecs::{Commands, Component, Entity, Query, Res, ResMut, Without};
use bsengine_physics::{Collider, ColliderShape, PhysicsInput, RigidBody};
use bsengine_render::components::TerrainSplat;
use bsengine_render::MeshRenderer;
use bsengine_rhi_wgpu::{GpuMeshRegistry, GpuTextureRegistry};
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
///
/// Also tracks the 4 layer texture loads (`layer0..3_texture_path`), so a
/// `Terrain`'s chunks aren't spawned until its heightmap AND all 4 diffuse
/// layers have arrived -- `generate_terrain_chunks` needs all 5 to build the
/// `TerrainSplat` it attaches to each chunk.
#[derive(Component)]
struct PendingTerrain {
    heightmap: bsengine_asset::AssetSlot<HeightmapAsset>,
    layers: [bsengine_asset::AssetSlot<TextureAsset>; 4],
}

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
    textures: Res<Assets<TextureAsset>>,
    mut mesh_registry: Option<ResMut<GpuMeshRegistry>>,
    mut tex_registry: Option<ResMut<GpuTextureRegistry>>,
    mut query: Query<
        (Entity, &Terrain, &Transform, Option<&mut PendingTerrain>),
        Without<TerrainChunksGenerated>,
    >,
) {
    for (entity, terrain, transform, pending) in query.iter_mut() {
        // Request exactly once, then retain the handles. See `PendingTerrain`.
        let Some(mut pending) = pending else {
            let heightmap_handle =
                asset_server.load::<HeightmapAsset>(terrain.heightmap_path.clone());
            let layer_paths = [
                terrain.layer0_texture_path.clone(),
                terrain.layer1_texture_path.clone(),
                terrain.layer2_texture_path.clone(),
                terrain.layer3_texture_path.clone(),
            ];
            let layers = layer_paths.map(|p| {
                bsengine_asset::AssetSlot::from_handle(asset_server.load::<TextureAsset>(p))
            });
            commands.entity(entity).insert(PendingTerrain {
                heightmap: bsengine_asset::AssetSlot::from_handle(heightmap_handle),
                layers,
            });
            continue;
        };

        let heightmap_failed = matches!(
            pending.heightmap.poll(&asset_server, &heightmaps),
            Polled::Failed(_)
        );
        let mut any_layer_failed = false;
        for slot in pending.layers.iter_mut() {
            if matches!(slot.poll(&asset_server, &textures), Polled::Failed(_)) {
                any_layer_failed = true;
            }
        }
        if heightmap_failed || any_layer_failed {
            // A failed load never resolves, so the path is dropped entirely --
            // otherwise a missing file retries silently forever.
            warn!(
                "[terrain] cannot load heightmap or a layer texture for '{}'",
                terrain.heightmap_path
            );
            commands.entity(entity).remove::<PendingTerrain>();
            continue;
        }
        // Deliberately not `Arrived`-only: the assets can land before
        // `GpuMeshRegistry`/`GpuTextureRegistry` exist (headless test mode,
        // or a frame before a window/surface is up), and this retries every
        // frame until the registries appear too.
        let heightmap_handle = pending.heightmap.handle().clone();
        let Some(heightmap) = heightmaps.get(&heightmap_handle) else {
            continue;
        };
        let Some(tex0) = textures.get(pending.layers[0].handle()) else {
            continue;
        };
        let Some(tex1) = textures.get(pending.layers[1].handle()) else {
            continue;
        };
        let Some(tex2) = textures.get(pending.layers[2].handle()) else {
            continue;
        };
        let Some(tex3) = textures.get(pending.layers[3].handle()) else {
            continue;
        };
        let Some(tex_reg) = tex_registry.as_mut() else {
            continue;
        };
        let Some(mesh_reg) = mesh_registry.as_mut() else {
            continue;
        };

        // Shared by every chunk of this `Terrain` entity -- uploaded once per
        // frame this branch is reached, which only happens once (chunks are
        // spawned and `TerrainChunksGenerated`/`PendingTerrain` are updated
        // before the next frame's query would see this entity again).
        let layer_ids: [u64; 4] = [
            tex_reg.load_from_rgba(tex0.width, tex0.height, &tex0.data),
            tex_reg.load_from_rgba(tex1.width, tex1.height, &tex1.data),
            tex_reg.load_from_rgba(tex2.width, tex2.height, &tex2.data),
            tex_reg.load_from_rgba(tex3.width, tex3.height, &tex3.data),
        ];

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
            let weight_bytes: Vec<u8> = chunk.splat_weights.iter().flatten().copied().collect();
            let weight_id = tex_reg.load_from_rgba(
                chunk.heightfield_cols as u32,
                chunk.heightfield_rows as u32,
                &weight_bytes,
            );
            let world_min_corner =
                transform.position.0 + Vec3::new(chunk.world_offset.0, 0.0, chunk.world_offset.1);

            commands.spawn((
                Transform::from_position(world_min_corner),
                GlobalTransform::default(),
                MeshRenderer { mesh_id },
                TerrainSplat {
                    weight_texture_id: weight_id,
                    layer_texture_ids: layer_ids,
                },
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

    /// Writes a tiny 2x2 solid-color PNG to a fresh path under the temp
    /// directory (one per call, so parallel tests don't collide -- `name`
    /// must be distinct per call site, the same convention
    /// `write_test_heightmap` uses) and returns its path as a `String`, the
    /// same shape `Terrain::layer0..3_texture_path` expect.
    fn write_test_texture(name: &str, rgba: [u8; 4]) -> String {
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba(rgba));
        let path = std::env::temp_dir().join(format!(
            "bsengine_terrain_test_tex_{name}_{}.png",
            std::process::id()
        ));
        img.save(&path).expect("write test texture fixture");
        path.to_str().unwrap().to_owned()
    }

    /// Inserts a real `GpuMeshRegistry`, backed by a real headless `wgpu`
    /// device -- not a stand-in, the same type the renderer uses.
    ///
    /// `WgpuRHIPlugin::windowed()` only builds one once a `WindowHandle`
    /// (created by winit) exists, so a window-less headless test has to
    /// construct it directly. Mirrors `bsengine-gltf`'s
    /// `insert_headless_gpu_registries` test helper, trimmed to only the
    /// registry `generate_terrain_chunks` actually reads for meshes -- see
    /// `insert_headless_texture_registry` below for its texture counterpart.
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

    /// Inserts a real `GpuTextureRegistry`, backed by a real headless `wgpu`
    /// device/queue -- not a stand-in, the same type the renderer uses.
    /// Sibling to `insert_headless_mesh_registry` above, needed now that
    /// `generate_terrain_chunks` uploads layer/weight textures too.
    fn insert_headless_texture_registry(app: &mut bevy_app::App) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("a headless adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("bsengine-app terrain test texture device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .expect("headless device request");
        let device = std::sync::Arc::new(device);
        let queue = std::sync::Arc::new(queue);
        app.insert_resource(GpuTextureRegistry::new(device, queue));
    }

    /// An app with everything `generate_terrain_chunks` needs: a real
    /// `AssetServer` (`AssetPlugin`), real headless `GpuMeshRegistry`/
    /// `GpuTextureRegistry`s, and `TerrainPlugin` itself.
    fn test_app() -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(TerrainPlugin);
        insert_headless_mesh_registry(&mut app);
        insert_headless_texture_registry(&mut app);
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
                    layer0_texture_path: write_test_texture("no-registry-l0", [50, 200, 50, 255]),
                    layer1_texture_path: write_test_texture("no-registry-l1", [120, 120, 120, 255]),
                    layer2_texture_path: write_test_texture("no-registry-l2", [110, 80, 40, 255]),
                    layer3_texture_path: write_test_texture("no-registry-l3", [240, 240, 250, 255]),
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
                    layer0_texture_path: write_test_texture(
                        "missing-heightmap-l0",
                        [50, 200, 50, 255],
                    ),
                    layer1_texture_path: write_test_texture(
                        "missing-heightmap-l1",
                        [120, 120, 120, 255],
                    ),
                    layer2_texture_path: write_test_texture(
                        "missing-heightmap-l2",
                        [110, 80, 40, 255],
                    ),
                    layer3_texture_path: write_test_texture(
                        "missing-heightmap-l3",
                        [240, 240, 250, 255],
                    ),
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
                    layer0_texture_path: write_test_texture("chunks-l0", [50, 200, 50, 255]),
                    layer1_texture_path: write_test_texture("chunks-l1", [120, 120, 120, 255]),
                    layer2_texture_path: write_test_texture("chunks-l2", [110, 80, 40, 255]),
                    layer3_texture_path: write_test_texture("chunks-l3", [240, 240, 250, 255]),
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
                    layer0_texture_path: write_test_texture("flat-drop-l0", [50, 200, 50, 255]),
                    layer1_texture_path: write_test_texture("flat-drop-l1", [120, 120, 120, 255]),
                    layer2_texture_path: write_test_texture("flat-drop-l2", [110, 80, 40, 255]),
                    layer3_texture_path: write_test_texture("flat-drop-l3", [240, 240, 250, 255]),
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

    /// The property this task adds: once a `Terrain`'s heightmap and all 4
    /// layer textures have loaded, every spawned chunk entity carries a
    /// `TerrainSplat` whose weight texture and all 4 layer textures are real,
    /// registered `GpuTextureRegistry` ids -- not the zero-valued default a
    /// forgotten field would leave behind (`GpuTextureRegistry` ids start at
    /// 1; 0 is never issued, see `GpuTextureRegistry::new`/`load_from_rgba`).
    #[test]
    fn terrain_chunks_carry_a_terrain_splat_with_real_texture_ids() {
        let mut app = test_app();

        let values = vec![10_000u16; 5 * 5];
        let path = write_test_heightmap("splat", 5, 5, &values);

        let chunk_count = (2u32, 1u32);
        let terrain_entity = app
            .world_mut()
            .spawn((
                Terrain {
                    heightmap_path: path,
                    chunk_count,
                    chunk_size: 8.0,
                    height_scale: 5.0,
                    layer0_texture_path: write_test_texture("splat-l0", [50, 200, 50, 255]),
                    layer1_texture_path: write_test_texture("splat-l1", [120, 120, 120, 255]),
                    layer2_texture_path: write_test_texture("splat-l2", [110, 80, 40, 255]),
                    layer3_texture_path: write_test_texture("splat-l3", [240, 240, 250, 255]),
                },
                Transform::default(),
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
            let splat = app
                .world()
                .get::<TerrainSplat>(*chunk)
                .expect("every chunk must carry a TerrainSplat");
            assert!(
                splat.weight_texture_id > 0,
                "weight_texture_id must be a real registered id, got {}",
                splat.weight_texture_id
            );
            for (i, id) in splat.layer_texture_ids.iter().enumerate() {
                assert!(
                    *id > 0,
                    "layer_texture_ids[{i}] must be a real registered id, got {id}"
                );
            }
        }
    }
}

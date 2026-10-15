//! `bsengine-runtime --bake <project>`: bakes a scene's lightmaps.
//!
//! The scene is loaded exactly as `--test` loads it (`build_test_app`: the
//! real plugin stack, prefabs and glTF resolved, transforms propagated), with
//! two differences: scripts do not run -- a bake is of the scene as authored,
//! not of wherever a script moved things to -- and the mesh registry keeps a
//! CPU copy of every mesh ([`RetainCpuMeshCopies`]), so the bake traces the
//! very triangles and `uv2` the renderer draws.
//!
//! What goes in, each converted the way the render plugin converts it for the
//! GPU (`bsengine-render`'s `point_light_entry` and friends): every
//! [`LightmapStatic`] entity's mesh and material, every light whose
//! [`LightBakeMode`] is not `Realtime` (the first directional light only, as
//! the renderer shades only the first), and the sky -- the skybox panorama if
//! there is one, else the directional light's ambient, which is what the
//! renderer lights with in either case.
//!
//! What comes out, under the project:
//! * `assets/lightmaps/<scene>/lightmap_<n>.png` -- one RGBM layer each, with a
//!   `.meta` that says `srgb: false` (the encoded values are not colours);
//! * `<scene>.lightmaps` beside the scene file (`bsengine_scene::lightmaps`),
//!   which loading the scene reads back to give each entity its `Lightmap`.

use bevy_ecs::prelude::{Entity, World};
use bsengine_bake::{
    BakeInput, BakeLight, BakeSettings, BakeTriangle, Contribution, LightmapBaker, RendererInfo,
    Sky, TraceSettings,
};
use bsengine_core::{
    DirectionalLight, GlobalTransform, LightBakeMode, LightmapStatic, Material, PointLight,
    RectLight, SkyboxPath, SpotLight, Transform,
};
use bsengine_rhi_wgpu::{GpuMeshRegistry, RetainCpuMeshCopies};
use glam::{Mat3, Mat4, Vec2, Vec3};

/// What `--bake` was asked to do.
#[derive(Debug, Clone)]
pub struct BakeArgs {
    pub project_dir: String,
    /// Project-relative scene path; the manifest's entry scene when absent.
    pub scene: Option<String>,
    /// `cpu`, or `auto` (cpu until a GPU backend exists).
    pub backend: String,
    pub samples: Option<u32>,
    pub bounces: Option<u32>,
    pub texels_per_unit: Option<f32>,
}

/// What a bake produced, for the summary line and for tests.
#[derive(Debug, Clone, PartialEq)]
pub struct BakeReport {
    pub renderers: usize,
    pub layers: usize,
    pub texels_per_unit: f32,
    pub data_path: String,
    pub layer_paths: Vec<String>,
}

/// Frames to wait for every static entity's mesh to load before giving up.
const MAX_LOAD_FRAMES: u32 = 600;

pub fn run_bake(args: &BakeArgs) -> Result<BakeReport, String> {
    let baker: Box<dyn LightmapBaker> = match args.backend.as_str() {
        "cpu" | "auto" => Box::new(bsengine_bake::CpuBaker),
        other => {
            return Err(format!(
                "--backend {other}: only `cpu` (or `auto`) exists so far"
            ))
        }
    };
    let pak = crate::open_pak(&args.project_dir);
    let manifest = crate::read_manifest(&args.project_dir, pak.as_deref());
    let relative_scene = args
        .scene
        .clone()
        .unwrap_or_else(|| manifest.project.entry_scene.clone());
    let scene_path = format!("{}/{relative_scene}", args.project_dir);
    let scene_text = std::fs::read_to_string(&scene_path)
        .map_err(|e| format!("cannot read {scene_path}: {e}"))?;

    let mut app = crate::test_mode::build_test_app(&args.project_dir, args.scene.as_deref(), true);
    app.insert_resource(RetainCpuMeshCopies);
    // Scripts stay stopped: the bake is of the scene as written.
    app.world_mut()
        .resource_mut::<bsengine_core::InspectorState>()
        .play_state = bsengine_core::EditorPlayState::Stopped;

    let mut ready = false;
    for _ in 0..MAX_LOAD_FRAMES {
        app.update();
        if missing_meshes(app.world_mut()).is_empty() {
            ready = true;
            break;
        }
    }
    // Two more, so the transforms of anything that arrived last are propagated.
    app.update();
    app.update();
    if !ready {
        let missing = missing_meshes(app.world_mut());
        return Err(format!(
            "these LightmapStatic entities never got a mesh: {}",
            missing.join(", ")
        ));
    }

    let world = app.world_mut();
    let gathered = gather(world)?;
    let lights = gather_lights(world);
    let sky = gather_sky(world);

    let defaults = BakeSettings::default();
    let settings = BakeSettings {
        trace: TraceSettings {
            samples: args.samples.unwrap_or(defaults.trace.samples),
            bounces: args.bounces.unwrap_or(defaults.trace.bounces),
            seed: 0,
        },
        texels_per_unit: args.texels_per_unit.unwrap_or(defaults.texels_per_unit),
        layer_size: bsengine_rhi_wgpu::lightmap::LIGHTMAP_SIZE,
        max_layers: bsengine_rhi_wgpu::lightmap::MAX_LIGHTMAPS as u32,
        // `Baked` lights' direct light and shadows go into the atlas: the mesh
        // shader skips those lights on every lightmapped draw, so they are not
        // drawn twice. (`Indirect` lights stay the runtime's to draw.)
        bake_direct: true,
    };
    let started = std::time::Instant::now();
    let output = baker.bake(
        &BakeInput {
            triangles: &gathered.triangles,
            renderers: &gathered.renderers,
            lights: &lights,
            sky: &sky,
        },
        &settings,
    );
    tracing::info!(
        "lightmap bake ({}): {} renderers, {} layers at {:.1} texels/unit, {:.1}s",
        baker.name(),
        gathered.names.len(),
        output.layers.len(),
        output.texels_per_unit,
        started.elapsed().as_secs_f32()
    );
    if output.texels_per_unit < settings.texels_per_unit {
        tracing::warn!(
            "lightmap bake: the scene did not fit in {} layers at {} texels per unit; baked at {:.1}",
            settings.max_layers,
            settings.texels_per_unit,
            output.texels_per_unit
        );
    }

    write_output(
        args,
        &relative_scene,
        &scene_path,
        &scene_text,
        &gathered.names,
        &output,
    )
}

/// Static entities with no CPU mesh yet, by name. (A skinned mesh counts as
/// arrived: it has no copy to wait for, and `gather` refuses it by name.)
fn missing_meshes(world: &mut World) -> Vec<String> {
    let mut q = world.query::<(
        &LightmapStatic,
        Option<&bsengine_scene::Name>,
        Option<&bsengine_render::MeshRenderer>,
    )>();
    let wanted: Vec<(String, Option<u64>)> = q
        .iter(world)
        .map(|(_, name, mesh)| {
            (
                name.map(|n| n.0.clone())
                    .unwrap_or_else(|| "<unnamed>".into()),
                mesh.map(|m| m.mesh_id),
            )
        })
        .collect();
    let registry = world.get_resource::<GpuMeshRegistry>();
    wanted
        .into_iter()
        .filter(|(_, id)| match (registry, id) {
            (Some(r), Some(id)) => r.cpu_mesh(*id).is_none() && !r.is_skinned(*id),
            _ => true,
        })
        .map(|(name, _)| name)
        .collect()
}

struct Gathered {
    triangles: Vec<BakeTriangle>,
    renderers: Vec<RendererInfo>,
    /// The entity name of each renderer, by renderer index.
    names: Vec<String>,
}

fn model_matrix(t: &Transform, gt: Option<&GlobalTransform>) -> Mat4 {
    gt.map(|g| g.to_matrix()).unwrap_or_else(|| t.to_matrix())
}

fn gather(world: &mut World) -> Result<Gathered, String> {
    let mut rows: Vec<(Entity, String, f32, u64, Mat4, Vec3, Vec3)> = Vec::new();
    let mut q = world.query::<(
        Entity,
        &LightmapStatic,
        Option<&bsengine_scene::Name>,
        &bsengine_render::MeshRenderer,
        &Transform,
        Option<&GlobalTransform>,
        Option<&Material>,
    )>();
    for (e, s, name, mesh, t, gt, mat) in q.iter(world) {
        let name = name.map(|n| n.0.clone()).ok_or_else(|| {
            format!("a LightmapStatic entity ({e:?}) has no name to record it by")
        })?;
        let (albedo, emissive) = mat
            .map(|m| (*m.base_color, *m.emissive))
            .unwrap_or((Vec3::ONE, Vec3::ZERO));
        rows.push((
            e,
            name,
            s.resolution_scale,
            mesh.mesh_id,
            model_matrix(t, gt),
            albedo,
            emissive,
        ));
    }
    // Entity order is the world's; sort by name so the bake (and its files)
    // do not depend on spawn order.
    rows.sort_by(|a, b| a.1.cmp(&b.1));

    let mut seen = std::collections::BTreeMap::<&str, usize>::new();
    for r in &rows {
        *seen.entry(&r.1).or_default() += 1;
    }
    let dupes: Vec<&str> = seen
        .iter()
        .filter(|(_, n)| **n > 1)
        .map(|(k, _)| *k)
        .collect();
    if !dupes.is_empty() {
        return Err(format!(
            "LightmapStatic entities must have unique names, since the lightmap data is keyed by \
             name; these repeat: {}",
            dupes.join(", ")
        ));
    }

    let registry = world.resource::<GpuMeshRegistry>();
    let mut out = Gathered {
        triangles: Vec::new(),
        renderers: Vec::new(),
        names: Vec::new(),
    };
    for (_, name, scale, mesh_id, model, albedo, emissive) in rows {
        if registry.is_skinned(mesh_id) {
            return Err(format!(
                "'{name}' is skinned; a lightmap is for static geometry (its uv2 is empty)"
            ));
        }
        let (vertices, indices) = registry
            .cpu_mesh(mesh_id)
            .ok_or_else(|| format!("'{name}' has no mesh copy to bake"))?;
        if vertices.iter().all(|v| v.uv2 == [0.0, 0.0]) {
            return Err(format!(
                "'{name}' has no lightmap UVs (uv2 is zero everywhere)"
            ));
        }
        let renderer = out.renderers.len() as u32;
        let normal_matrix = Mat3::from_mat4(model).inverse().transpose();
        for tri in indices.chunks_exact(3) {
            let v = |k: usize| &vertices[tri[k] as usize];
            out.triangles.push(BakeTriangle {
                positions: [0, 1, 2].map(|k| model.transform_point3(Vec3::from(v(k).position))),
                normals: [0, 1, 2]
                    .map(|k| (normal_matrix * Vec3::from(v(k).normal)).normalize_or_zero()),
                uv2: [0, 1, 2].map(|k| Vec2::from(v(k).uv2)),
                albedo,
                emissive,
                renderer,
            });
        }
        out.renderers.push(RendererInfo {
            resolution_scale: scale,
        });
        out.names.push(name);
    }
    Ok(out)
}

fn contribution(mode: LightBakeMode) -> Option<Contribution> {
    match mode {
        LightBakeMode::Realtime => None,
        LightBakeMode::Indirect => Some(Contribution::Indirect),
        LightBakeMode::Baked => Some(Contribution::Baked),
    }
}

/// Position and rotation of a light, as the renderer reads them.
fn pose(t: &Transform, gt: Option<&GlobalTransform>) -> (Vec3, glam::Quat) {
    gt.map(|g| {
        let (_, r, p) = g.to_matrix().to_scale_rotation_translation();
        (p, r)
    })
    .unwrap_or((t.position.0, t.rotation.0))
}

fn gather_lights(world: &mut World) -> Vec<BakeLight> {
    let mut lights = Vec::new();
    // The first directional light only: the renderer shades no other.
    let mut suns = world.query::<(&DirectionalLight, &Transform, Option<&GlobalTransform>)>();
    if let Some((l, t, gt)) = suns.iter(world).next() {
        if let Some(contribution) = contribution(l.bake_mode) {
            let direction = gt
                .map(|g| -Mat3::from_mat4(g.to_matrix()).z_axis)
                .unwrap_or_else(|| t.rotation.0 * Vec3::NEG_Z);
            lights.push(BakeLight::Directional {
                direction: direction.normalize_or(Vec3::NEG_Y),
                color: *l.color,
                contribution,
            });
        }
    }
    let mut points = world.query::<(&PointLight, &Transform, Option<&GlobalTransform>)>();
    for (l, t, gt) in points.iter(world) {
        if let Some(contribution) = contribution(l.bake_mode) {
            lights.push(BakeLight::Point {
                position: pose(t, gt).0,
                color: *l.color,
                intensity: l.intensity,
                range: l.range,
                contribution,
            });
        }
    }
    let mut spots = world.query::<(&SpotLight, &Transform, Option<&GlobalTransform>)>();
    for (l, t, gt) in spots.iter(world) {
        if let Some(contribution) = contribution(l.bake_mode) {
            let direction = gt
                .map(|g| -Mat3::from_mat4(g.to_matrix()).z_axis)
                .unwrap_or_else(|| t.rotation.0 * Vec3::NEG_Z);
            lights.push(BakeLight::Spot {
                position: pose(t, gt).0,
                direction: direction.normalize_or(Vec3::NEG_Y),
                color: *l.color,
                intensity: l.intensity,
                range: l.range,
                inner_cos: l.inner_angle_degrees.to_radians().cos(),
                outer_cos: l.outer_angle_degrees.to_radians().cos(),
                contribution,
            });
        }
    }
    let mut rects = world.query::<(&RectLight, &Transform, Option<&GlobalTransform>)>();
    for (l, t, gt) in rects.iter(world) {
        if let Some(contribution) = contribution(l.bake_mode) {
            let (position, rotation) = pose(t, gt);
            lights.push(BakeLight::Rect {
                position,
                half_width: rotation * Vec3::X * (l.width * 0.5),
                half_height: rotation * Vec3::Y * (l.height * 0.5),
                radiance: *l.color * l.intensity,
                range: l.range,
                contribution,
            });
        }
    }
    lights
}

/// The renderer's own default ambient, when there is no directional light
/// (`LightData::default`).
const DEFAULT_AMBIENT: Vec3 = Vec3::splat(0.15);

fn gather_sky(world: &mut World) -> Sky {
    let ambient = {
        let mut suns = world.query::<&DirectionalLight>();
        suns.iter(world)
            .next()
            .map(|l| *l.ambient)
            .unwrap_or(DEFAULT_AMBIENT)
    };
    let Some(path) = world.get_resource::<SkyboxPath>().and_then(|p| p.0.clone()) else {
        return Sky::Uniform(ambient);
    };
    match image::open(&path) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            // Decoded from sRGB, as the skybox texture is sampled at run time.
            let linear = |c: u8| {
                let c = c as f32 / 255.0;
                if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            };
            let texels = rgba
                .pixels()
                .map(|p| Vec3::new(linear(p[0]), linear(p[1]), linear(p[2])))
                .collect();
            Sky::Equirect {
                width,
                height,
                texels,
            }
        }
        Err(e) => {
            tracing::warn!(
                "lightmap bake: cannot read the skybox {path} ({e}); baking the ambient"
            );
            Sky::Uniform(ambient)
        }
    }
}

fn scene_stem(relative_scene: &str) -> String {
    std::path::Path::new(relative_scene)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "scene".into())
}

fn write_output(
    args: &BakeArgs,
    relative_scene: &str,
    scene_path: &str,
    scene_text: &str,
    names: &[String],
    output: &bsengine_bake::BakeOutput,
) -> Result<BakeReport, String> {
    let stem = scene_stem(relative_scene);
    let relative_dir = format!("assets/lightmaps/{stem}");
    let dir = std::path::Path::new(&args.project_dir).join(&relative_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

    let mut layer_paths = Vec::new();
    for (n, layer) in output.layers.iter().enumerate() {
        let file = dir.join(format!("lightmap_{n}.png"));
        let bytes = bsengine_bake::encode_rgbm(layer, bsengine_core::LIGHTMAP_RGBM_RANGE);
        image::RgbaImage::from_raw(output.layer_size, output.layer_size, bytes)
            .expect("a layer is layer_size squared")
            .save(&file)
            .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
        write_sidecar(&file)?;
        layer_paths.push(format!("{relative_dir}/lightmap_{n}.png"));
    }
    // Layers a previous, larger bake left behind would otherwise linger.
    for n in output.layers.len()..bsengine_rhi_wgpu::lightmap::MAX_LIGHTMAPS + 8 {
        let file = dir.join(format!("lightmap_{n}.png"));
        if file.exists() {
            let _ = std::fs::remove_file(&file);
            let _ = std::fs::remove_file(bsengine_asset::identity::sidecar_path(&file));
        }
    }

    let data = bsengine_scene::lightmaps::LightmapData {
        scene_fingerprint: bsengine_scene::lightmaps::fingerprint(scene_text),
        entries: names
            .iter()
            .zip(&output.placements)
            .map(|(name, p)| bsengine_scene::lightmaps::LightmapEntry {
                entity: name.clone(),
                texture: layer_paths[p.layer as usize].clone(),
                scale_offset: p.scale_offset(output.layer_size),
            })
            .collect(),
    };
    let data_path = bsengine_scene::lightmaps::data_path(scene_path);
    std::fs::write(
        &data_path,
        ron::ser::to_string_pretty(&data, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("cannot encode the lightmap data: {e}"))?,
    )
    .map_err(|e| format!("cannot write {data_path}: {e}"))?;

    Ok(BakeReport {
        renderers: names.len(),
        layers: output.layers.len(),
        texels_per_unit: output.texels_per_unit,
        data_path,
        layer_paths,
    })
}

/// The layer's `.meta`: linear, no mips. Keeps an existing identity, so a
/// re-bake changes the hash and nothing else.
fn write_sidecar(file: &std::path::Path) -> Result<(), String> {
    use bsengine_asset::identity::{measure_file, sidecar_path, ImportSettings, Sidecar};
    let (hash, size) =
        measure_file(file).map_err(|e| format!("cannot hash {}: {e}", file.display()))?;
    let existing = Sidecar::read(sidecar_path(file)).ok().flatten();
    let sidecar = Sidecar {
        guid: existing
            .as_ref()
            .map(|s| s.guid)
            .unwrap_or_else(bsengine_asset::AssetGuid::new),
        hash,
        size: Some(size),
        former_paths: existing.map(|s| s.former_paths).unwrap_or_default(),
        import: Some(ImportSettings::Texture(
            bsengine_core::TextureImportSettings {
                srgb: false,
                mipmaps: false,
                ..Default::default()
            },
        )),
    };
    sidecar
        .write(sidecar_path(file))
        .map_err(|e| format!("cannot write the sidecar for {}: {e}", file.display()))
}

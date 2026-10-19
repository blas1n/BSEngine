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

use bevy_ecs::prelude::World;
use bsengine_bake::{
    BakeInput, BakeLight, BakeSettings, BakeTriangle, LightmapBaker, RendererInfo, Sky,
    TraceSettings,
};
use bsengine_core::LightmapStatic;
use bsengine_rhi_wgpu::{GpuMeshRegistry, RetainCpuMeshCopies};

/// What `--bake` was asked to do.
#[derive(Debug, Clone)]
pub struct BakeArgs {
    pub project_dir: String,
    /// Project-relative scene path; the manifest's entry scene when absent.
    pub scene: Option<String>,
    /// `cpu`, `gpu` or `auto` (see [`select_baker`]).
    pub backend: String,
    pub samples: Option<u32>,
    pub bounces: Option<u32>,
    pub texels_per_unit: Option<f32>,
}

/// What a bake produced, for the summary line, `--json` and tests.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct BakeReport {
    /// The backend that ran -- `cpu` or `gpu` -- which for `auto` is the only
    /// place the choice is visible.
    pub backend: String,
    pub renderers: usize,
    pub layers: usize,
    pub texels_per_unit: f32,
    pub data_path: String,
    pub layer_paths: Vec<String>,
}

/// The baker `--backend` names. `gpu` insists -- any adapter, a software one
/// included, and an error when there is none, since asking for it by name
/// means a CPU bake is not what was wanted. `auto` (the default, and what the
/// editor's button and MCP send) takes a hardware GPU when there is one and
/// the CPU otherwise, saying why on the log; the report's `backend` says
/// which ran.
fn select_baker(backend: &str) -> Result<Box<dyn LightmapBaker>, String> {
    match backend {
        "cpu" => Ok(Box::new(bsengine_bake::CpuBaker)),
        "gpu" => bsengine_bake::GpuBaker::new()
            .map(|b| Box::new(b) as Box<dyn LightmapBaker>)
            .map_err(|e| format!("--backend gpu: {e}")),
        "auto" => Ok(match bsengine_bake::GpuBaker::new_hardware() {
            Ok(b) => Box::new(b),
            Err(why) => {
                tracing::info!("lightmap bake: on the CPU ({why})");
                Box::new(bsengine_bake::CpuBaker)
            }
        }),
        other => Err(format!("--backend {other}: expected cpu, gpu or auto")),
    }
}

/// Frames to wait for every static entity's mesh to load before giving up.
const MAX_LOAD_FRAMES: u32 = 600;

pub fn run_bake(args: &BakeArgs) -> Result<BakeReport, String> {
    let baker = select_baker(&args.backend)?;
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
        baker.name(),
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

fn gather(world: &mut World) -> Result<Gathered, String> {
    let mut rows: Vec<(String, bsengine_render::gi_scene::StaticRow)> = Vec::new();
    for row in bsengine_render::gi_scene::static_rows(world) {
        let name = world
            .get::<bsengine_scene::Name>(row.entity)
            .map(|n| n.0.clone())
            .ok_or_else(|| {
                format!(
                    "a LightmapStatic entity ({:?}) has no name to record it by",
                    row.entity
                )
            })?;
        rows.push((name, row));
    }
    // Entity order is the world's; sort by name so the bake (and its files)
    // do not depend on spawn order.
    rows.sort_by(|a, b| a.0.cmp(&b.0));

    let mut seen = std::collections::BTreeMap::<&str, usize>::new();
    for r in &rows {
        *seen.entry(&r.0).or_default() += 1;
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
    for (name, row) in rows {
        let mesh_id = row.mesh_id;
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
        bsengine_render::gi_scene::world_triangles(
            vertices,
            indices,
            &row,
            renderer,
            &mut out.triangles,
        );
        out.renderers.push(RendererInfo {
            resolution_scale: row.resolution_scale,
        });
        out.names.push(name);
    }
    Ok(out)
}

fn gather_lights(world: &mut World) -> Vec<BakeLight> {
    use bsengine_render::gi_scene;
    gi_scene::lights(world, gi_scene::bake_contribution)
}

/// The skybox panorama if there is one, else the directional light's ambient:
/// what the renderer lights with in either case.
fn gather_sky(world: &mut World) -> Sky {
    use bsengine_render::gi_scene;
    let ambient = gi_scene::ambient(world);
    gi_scene::sky(gi_scene::skybox_path(world).as_deref(), ambient)
}

fn scene_stem(relative_scene: &str) -> String {
    std::path::Path::new(relative_scene)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "scene".into())
}

fn write_output(
    args: &BakeArgs,
    backend: &str,
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
        backend: backend.to_string(),
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

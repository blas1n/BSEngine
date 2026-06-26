use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bsengine_core::{GlobalTransform, Material, Transform};
use bsengine_render::MeshRenderer;
use bsengine_rhi_wgpu::{GpuMeshRegistry, GpuTextureRegistry};
use tracing::warn;

use crate::loader::GltfLoader;

#[derive(Component, Clone, Debug)]
pub struct GltfAsset {
    pub path: String,
}

impl GltfAsset {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

pub struct GltfPlugin;

impl Plugin for GltfPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, load_gltf_assets);
    }
}

fn load_gltf_assets(
    mut commands: Commands,
    query: Query<(Entity, &GltfAsset, Option<&Transform>), Without<MeshRenderer>>,
    mesh_registry: Option<ResMut<GpuMeshRegistry>>,
    tex_registry: Option<ResMut<GpuTextureRegistry>>,
) {
    let Some(mut mesh_reg) = mesh_registry else {
        return;
    };
    let mut tex_reg = tex_registry;

    for (entity, asset, existing_transform) in query.iter() {
        match GltfLoader::load_full(&asset.path) {
            Ok(loaded) => {
                let tex_ids: Vec<Option<u64>> = if let Some(ref mut tr) = tex_reg {
                    loaded
                        .images
                        .iter()
                        .map(|img| Some(tr.load_from_rgba(img.width, img.height, &img.rgba)))
                        .collect()
                } else {
                    vec![None; loaded.images.len()]
                };

                let mut first = true;
                for (mesh_data, tex_idx) in loaded
                    .meshes
                    .into_iter()
                    .zip(loaded.mesh_tex_indices.iter())
                {
                    let mesh_id = mesh_reg.register(&mesh_data.vertices, &mesh_data.indices);
                    let texture_id = tex_idx.and_then(|i| tex_ids.get(i).copied().flatten());
                    let mat = Material {
                        texture_id,
                        ..Default::default()
                    };

                    if first {
                        let mut e = commands.entity(entity);
                        e.insert((MeshRenderer { mesh_id }, mat));
                        e.remove::<GltfAsset>();
                        if existing_transform.is_none() {
                            e.insert((Transform::default(), GlobalTransform::default()));
                        }
                        first = false;
                    } else {
                        let t = existing_transform.cloned().unwrap_or_default();
                        commands.spawn((
                            MeshRenderer { mesh_id },
                            mat,
                            t,
                            GlobalTransform::default(),
                        ));
                    }
                }

                if first {
                    commands.entity(entity).remove::<GltfAsset>();
                    warn!("GLTF {} has no meshes", asset.path);
                }
            }
            Err(e) => {
                warn!("Failed to load GLTF {}: {e}", asset.path);
                commands.entity(entity).remove::<GltfAsset>();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_app::new_app;
    use bsengine_rhi_wgpu::WgpuRHIPlugin;

    #[test]
    fn gltf_plugin_builds_and_runs() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(GltfPlugin);
        app.update();
    }

    #[test]
    fn no_registry_leaves_gltf_asset_intact() {
        let mut app = new_app();
        app.add_plugins(GltfPlugin);
        let e = app.world_mut().spawn(GltfAsset::new("missing.gltf")).id();
        app.update();
        assert!(
            app.world().get::<GltfAsset>(e).is_some(),
            "GltfAsset should remain when GpuMeshRegistry is unavailable"
        );
    }

    #[test]
    fn with_rhi_plugin_but_no_window_gltf_asset_stays() {
        // WgpuRHIPlugin creates headless RHI but GpuMeshRegistry requires a
        // WindowHandle (created by winit). Without a window, load_gltf_assets
        // returns early and the GltfAsset marker stays on the entity.
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(GltfPlugin);
        let e = app.world_mut().spawn(GltfAsset::new("bad.gltf")).id();
        app.update();
        assert!(app.world().get::<GltfAsset>(e).is_some());
    }
}

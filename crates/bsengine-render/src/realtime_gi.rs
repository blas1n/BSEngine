//! Feeds the renderer's realtime light-probe volume
//! (`bsengine_rhi_wgpu::realtime_gi`) from the world, every frame: the
//! volume, the static set, the lights as they are now, and the sky.

use crate::gi_scene;
use bevy_ecs::prelude::{Local, World};
use bsengine_core::{GlobalTransform, LightProbeVolume, ProbeUpdate, Transform};
use bsengine_rhi_wgpu::realtime_gi::{GiStatic, RealtimeGiInput};
use std::sync::Arc;

/// The sky last decoded, kept until the skybox or the ambient changes: a
/// panorama is decoded once, not every frame.
#[derive(Default)]
pub(crate) struct SkyCache(Option<(Option<String>, glam::Vec3, Arc<bsengine_bake::Sky>)>);

/// The volume the renderer uses -- the first, as `render_frame` takes it --
/// and its world box, when that volume is realtime.
fn realtime_volume(world: &mut World) -> Option<(LightProbeVolume, glam::Vec3)> {
    let mut q = world.query::<(&LightProbeVolume, &Transform, Option<&GlobalTransform>)>();
    let (v, t, gt) = q.iter(world).next()?;
    (v.update == ProbeUpdate::Realtime).then(|| {
        let centre = gt
            .map(|g| g.to_matrix().w_axis.truncate())
            .unwrap_or(t.position.0);
        (*v, centre)
    })
}

/// Hands the surface this frame's realtime volume, or `None` when the scene
/// has none -- which gives the probe grid back to the baked path.
pub(crate) fn feed_realtime_gi(world: &mut World, mut sky_cache: Local<SkyCache>) {
    if !world.contains_resource::<bsengine_rhi_wgpu::WgpuSurfaceResource>() {
        return;
    }
    let input = realtime_volume(world).map(|(volume, centre)| {
        let half = *volume.half_extents;
        let statics = gi_scene::static_rows(world)
            .into_iter()
            .map(|r| GiStatic {
                mesh_id: r.mesh_id,
                model: r.model,
                albedo: r.albedo,
                emissive: r.emissive,
            })
            .collect();
        let lights = gi_scene::lights(world, gi_scene::realtime_contribution);
        let ambient = gi_scene::ambient(world);
        let path = gi_scene::skybox_path(world);
        let fresh = sky_cache
            .0
            .as_ref()
            .is_some_and(|(p, a, _)| *p == path && *a == ambient);
        if !fresh {
            let sky = Arc::new(gi_scene::sky(path.as_deref(), ambient));
            sky_cache.0 = Some((path, ambient, sky));
        }
        let sky = Arc::clone(&sky_cache.0.as_ref().expect("filled above").2);
        RealtimeGiInput {
            // Axis-aligned in world units, as the baked volume is: see the
            // light-probe comment in `render_frame`.
            volume: bsengine_rhi_wgpu::ProbeVolumeParams {
                origin: centre - half,
                extent: half * 2.0,
                resolution: volume.clamped_resolution(),
            },
            rays_per_probe: volume.clamped_rays_per_probe(),
            hysteresis: volume.clamped_hysteresis(),
            rays_per_frame: world
                .get_resource::<bsengine_core::GiSettings>()
                .copied()
                .unwrap_or_default()
                .rays_per_frame,
            statics,
            lights,
            sky,
        }
    });
    world
        .resource_mut::<bsengine_rhi_wgpu::WgpuSurfaceResource>()
        .0
        .set_realtime_gi(input);
}

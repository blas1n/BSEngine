//! The scene as global illumination sees it: the static geometry that bounces
//! light, the lights, and the sky -- converted the way the renderer converts
//! them for the GPU, into `bsengine-bake`'s types.
//!
//! Two consumers read this, and that is why it is one module: the lightmap
//! bake (`bsengine-runtime --bake`) and the realtime probe volume
//! (`LightProbeVolume { update: Realtime }`). Converted twice, a light's
//! direction or a skybox's colour space could differ between the two; here
//! they cannot.
//!
//! "Static" is [`LightmapStatic`]: Unity's single "Contribute GI" flag is the
//! model -- one marker says an object is baked, and is one tree for realtime
//! GI. Realtime GI also traces every other renderer where it is each frame
//! ([`dynamic_rows`]) unless it is [`ExcludeFromGi`].

use bevy_ecs::prelude::{Entity, Without, World};
use bsengine_bake::{BakeLight, BakeTriangle, Contribution, Sky};
use bsengine_core::{
    DirectionalLight, ExcludeFromGi, GlobalTransform, LightBakeMode, LightmapStatic, Material,
    PointLight, RectLight, SkyboxPath, SpotLight, Transform, Visible,
};
use glam::{Mat3, Mat4, Vec3};

/// One static entity, as GI needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct StaticRow {
    /// The entity, for a caller that needs more of it (the bake's name).
    pub entity: Entity,
    /// `MeshRenderer::mesh_id`.
    pub mesh_id: u64,
    /// Local to world, as the renderer draws it.
    pub model: Mat4,
    /// The material's base colour (white without a material): the albedo
    /// light bounces with. Textures are not sampled.
    pub albedo: Vec3,
    /// The material's emissive radiance.
    pub emissive: Vec3,
    /// `LightmapStatic::resolution_scale`, for the bake's packing.
    pub resolution_scale: f32,
}

/// Every [`LightmapStatic`] entity with a mesh, in entity order.
pub fn static_rows(world: &mut World) -> Vec<StaticRow> {
    let mut q = world.query::<(
        Entity,
        &LightmapStatic,
        &crate::MeshRenderer,
        &Transform,
        Option<&GlobalTransform>,
        Option<&Material>,
    )>();
    q.iter(world)
        .map(|(entity, s, mesh, t, gt, mat)| {
            let (albedo, emissive) = mat
                .map(|m| (*m.base_color, *m.emissive))
                .unwrap_or((Vec3::ONE, Vec3::ZERO));
            StaticRow {
                entity,
                mesh_id: mesh.mesh_id,
                model: gt.map(|g| g.to_matrix()).unwrap_or_else(|| t.to_matrix()),
                albedo,
                emissive,
                resolution_scale: s.resolution_scale,
            }
        })
        .collect()
}

/// One moving entity, as realtime GI traces it.
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicRow {
    /// `MeshRenderer::mesh_id`.
    pub mesh_id: u64,
    /// Local to world, this frame.
    pub model: Mat4,
    /// As [`StaticRow::albedo`].
    pub albedo: Vec3,
    /// As [`StaticRow::emissive`].
    pub emissive: Vec3,
}

/// Every other renderer realtime GI traces, in entity order: each one with a
/// mesh that is not [`LightmapStatic`], not hidden ([`Visible`]), and not
/// [`ExcludeFromGi`]. The default is that a renderer takes part -- Lumen's
/// and RTXGI's, and Unreal's "Affect Dynamic Indirect Lighting" -- so a crate
/// a script slides across the room shadows and lights the probes where it is
/// now. (The lightmap bake reads [`static_rows`] alone: a bake cannot follow
/// a moving thing.)
pub fn dynamic_rows(world: &mut World) -> Vec<DynamicRow> {
    let mut q = world.query_filtered::<(
        &crate::MeshRenderer,
        &Transform,
        Option<&GlobalTransform>,
        Option<&Material>,
        Option<&Visible>,
    ), (Without<LightmapStatic>, Without<ExcludeFromGi>)>();
    q.iter(world)
        .filter(|(.., vis)| vis.map(|v| v.is_visible).unwrap_or(true))
        .map(|(mesh, t, gt, mat, _)| {
            let (albedo, emissive) = mat
                .map(|m| (*m.base_color, *m.emissive))
                .unwrap_or((Vec3::ONE, Vec3::ZERO));
            DynamicRow {
                mesh_id: mesh.mesh_id,
                model: gt.map(|g| g.to_matrix()).unwrap_or_else(|| t.to_matrix()),
                albedo,
                emissive,
            }
        })
        .collect()
}

/// `row`'s mesh in world space, appended to `out` as `renderer`'s triangles
/// -- [`bsengine_rhi_wgpu::realtime_gi::world_triangles`], the conversion the
/// realtime update makes of the same mesh.
pub fn world_triangles(
    vertices: &[bsengine_rhi_wgpu::mesh::Vertex],
    indices: &[u32],
    row: &StaticRow,
    renderer: u32,
    out: &mut Vec<BakeTriangle>,
) {
    bsengine_rhi_wgpu::realtime_gi::world_triangles(
        vertices,
        indices,
        row.model,
        row.albedo,
        row.emissive,
        renderer,
        out,
    );
}

/// The bake's rule: a `Realtime` light is not baked at all, `Indirect` only
/// bounces, `Baked` also has its direct light and shadow written.
pub fn bake_contribution(mode: LightBakeMode) -> Option<Contribution> {
    match mode {
        LightBakeMode::Realtime => None,
        LightBakeMode::Indirect => Some(Contribution::Indirect),
        LightBakeMode::Baked => Some(Contribution::Baked),
    }
}

/// Realtime GI's rule: every light bounces, whatever its bake mode -- the
/// probes are lit by the scene as it is now -- and none is direct, since a
/// probe stores only the light arriving from elsewhere and the mesh shader
/// draws every light's direct term itself.
pub fn realtime_contribution(_mode: LightBakeMode) -> Option<Contribution> {
    Some(Contribution::Indirect)
}

/// Position and rotation of a light, as the renderer reads them.
fn pose(t: &Transform, gt: Option<&GlobalTransform>) -> (Vec3, glam::Quat) {
    gt.map(|g| {
        let (_, r, p) = g.to_matrix().to_scale_rotation_translation();
        (p, r)
    })
    .unwrap_or((t.position.0, t.rotation.0))
}

/// Every light `contribution` keeps, converted as the renderer converts it.
pub fn lights(
    world: &mut World,
    contribution: impl Fn(LightBakeMode) -> Option<Contribution>,
) -> Vec<BakeLight> {
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
pub const DEFAULT_AMBIENT: Vec3 = Vec3::splat(0.15);

/// The scene's ambient: the first directional light's, else the renderer's
/// default. What lights a miss when there is no skybox.
pub fn ambient(world: &mut World) -> Vec3 {
    let mut suns = world.query::<&DirectionalLight>();
    suns.iter(world)
        .next()
        .map(|l| *l.ambient)
        .unwrap_or(DEFAULT_AMBIENT)
}

/// The skybox's path, if the scene has one.
pub fn skybox_path(world: &World) -> Option<String> {
    world.get_resource::<SkyboxPath>().and_then(|p| p.0.clone())
}

/// The sky a ray that hits nothing sees: the skybox panorama, decoded from
/// sRGB as the renderer samples it, or `ambient` when there is none or it
/// cannot be read. Decoding a panorama is not cheap; callers that run every
/// frame keep the result until [`skybox_path`] or the ambient changes.
pub fn sky(path: Option<&str>, ambient: Vec3) -> Sky {
    let Some(path) = path else {
        return Sky::Uniform(ambient);
    };
    match image::open(path) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
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
                "global illumination: cannot read the skybox {path} ({e}); using the ambient"
            );
            Sky::Uniform(ambient)
        }
    }
}

//! Spot light shadows: one perspective depth map per spot light, looking down
//! its cone.
//!
//! All three reference engines shadow spot lights this way -- a single
//! perspective view per light is exactly what a cone needs, where a point
//! light needs six. Unity URP packs them into its "additional lights" shadow
//! atlas (2048 square by default, shared), Godot into its positional shadow
//! atlas, Unreal gives each light its own map. This follows the atlas engines'
//! budget: a fixed [`MAX_SPOT_SHADOWS`]-layer array at [`SPOT_SHADOW_MAP_SIZE`],
//! 8 MiB of `Depth32Float` in all, rather than the directional map's 1024 --
//! a spot light covers a cone a few metres across, not the camera's whole
//! shadow distance.
//!
//! The maps are drawn by the directional shadow pipeline (it is a depth-only
//! pass through a view-projection, which is all a spot shadow is) and read
//! through the directional comparison sampler.

use glam::{Mat4, Vec3};

/// How many spot lights cast shadows: every one the renderer shades.
pub const MAX_SPOT_SHADOWS: usize = 8;

/// Edge length of one spot light's shadow map, in texels. 8 x 512 x 512 x 4
/// bytes is 8 MiB -- half what the directional cascades cost together.
pub const SPOT_SHADOW_MAP_SIZE: u32 = 512;

/// The view-projection a spot light's shadow map is drawn and read with: from
/// the light, along `direction`, a square frustum whose half-angle is the
/// cone's -- the cone is inscribed in it, so every point the light reaches is
/// on the map -- out to `range`, where the light reaches nothing.
///
/// The near plane is a hundredth of the range (at least 5 cm): close enough
/// that a caster just in front of the light still casts, far enough that a
/// 32-bit float depth keeps its precision across the rest.
pub fn spot_shadow_view_proj(position: Vec3, direction: Vec3, outer_angle: f32, range: f32) -> Mat4 {
    let direction = direction.normalize_or(Vec3::NEG_Z);
    let (_, up) = crate::cookie::cookie_basis(direction);
    let fov = (2.0 * outer_angle).clamp(1e-3, std::f32::consts::PI - 1e-3);
    let far = range.max(0.1);
    let near = (far * 0.01).max(0.05).min(far * 0.5);
    Mat4::perspective_rh(fov, 1.0, near, far) * Mat4::look_at_rh(position, position + direction, up)
}

/// Every spot shadow's view-projection, as the lighting shaders read them.
/// Matches `SpotShadowUniform` in `spot_shadow_wgsl!`.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SpotShadowUniform {
    pub(crate) view_proj: [[[f32; 4]; 4]; MAX_SPOT_SHADOWS],
}

/// The spot shadow map and its lookup, appended to every shader that lights
/// with spot lights. `spot_shadow` returns how much of the light reaches
/// `world_pos`: 1 lit, 0 shadowed, between at a filtered edge.
///
/// The lookup is offset rather than biased in depth. The directional cascades
/// subtract a constant 0.003 from the depth, which suits an orthographic map
/// whose depth is linear; under this perspective a constant in depth is a
/// world distance that grows with the square of the distance -- 0.74 units at
/// 5 m with a 10 m range -- and would lift every shadow off its caster.
/// Instead the point moves toward the light and off the surface by about a
/// texel's width at its distance (more at grazing angles, where acne starts),
/// which is a fixed fraction of the map wherever the point is.
macro_rules! spot_shadow_wgsl {
    () => {
        r#"
struct SpotShadowUniform {
    view_proj: array<mat4x4<f32>, 8>,
};
@group(2) @binding(20) var spot_shadow_map: texture_depth_2d_array;
@group(2) @binding(21) var<uniform> spot_shadows: SpotShadowUniform;

fn spot_shadow(sl: SpotLightEntry, world_pos: vec3<f32>, n: vec3<f32>, l: vec3<f32>, soft: u32) -> f32 {
    if (sl.shadow_layer < 0.0) {
        return 1.0;
    }
    let layer = i32(sl.shadow_layer);
    let size = vec2<f32>(textureDimensions(spot_shadow_map));
    // One texel's width on a surface facing the light at this distance.
    let along = max(dot(world_pos - sl.position, sl.direction), 0.0);
    let tan_outer = sqrt(max(1.0 - sl.outer_cos * sl.outer_cos, 0.0)) / max(sl.outer_cos, 1e-4);
    let texel = 2.0 * along * tan_outer / size.x;
    let grazing = 1.0 - clamp(dot(n, l), 0.0, 1.0);
    let p = world_pos + n * texel * (0.5 + 1.5 * grazing) + l * texel;
    let lsp = spot_shadows.view_proj[layer] * vec4<f32>(p, 1.0);
    if (lsp.w <= 0.0) {
        return 1.0;
    }
    let proj = lsp.xyz / lsp.w;
    let uv = proj.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || proj.z < 0.0 || proj.z > 1.0) {
        return 1.0;
    }
    // The directional filter, on this map: hard reads one texel, soft a 3x3
    // tent of the comparison sampler's bilinear lookups (`shadow_filtered`).
    // `...Level` for the reason `shadow_sample` gives: this is non-uniform
    // control flow.
    if (soft == 0u) {
        let centre = (floor(uv * size) + vec2<f32>(0.5, 0.5)) / size;
        return textureSampleCompareLevel(spot_shadow_map, shadow_sampler, centre, layer, proj.z);
    }
    let step = vec2<f32>(1.0, 1.0) / size;
    var sum = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            sum += textureSampleCompareLevel(spot_shadow_map, shadow_sampler, uv + vec2<f32>(f32(x), f32(y)) * step, layer, proj.z);
        }
    }
    return sum / 9.0;
}
"#
    };
}
pub(crate) use spot_shadow_wgsl;

#[cfg(test)]
mod tests {
    use super::*;

    /// The cone fits the map: a point on the cone's edge at any distance
    /// projects inside the square, on its edge -- and the light's own axis
    /// to the centre. A frustum built from the inner angle, or from the
    /// half-angle as if it were the whole, would put the edge off the map
    /// (or in its middle).
    #[test]
    fn the_cone_is_inscribed_in_the_map() {
        let (position, direction) = (Vec3::new(1.0, 4.0, 2.0), Vec3::new(0.3, -1.0, 0.1).normalize());
        let outer = 30f32.to_radians();
        let vp = spot_shadow_view_proj(position, direction, outer, 10.0);
        let (_, up) = crate::cookie::cookie_basis(direction);
        let centre = vp.project_point3(position + direction * 5.0);
        assert!(centre.x.abs() < 1e-4 && centre.y.abs() < 1e-4, "{centre}");
        let edge = position + (direction * outer.cos() + up * outer.sin()) * 5.0;
        let p = vp.project_point3(edge);
        assert!((p.y.abs() - 1.0).abs() < 1e-3 && p.x.abs() < 1e-3, "{p}");
        assert!(p.z > 0.0 && p.z < 1.0, "inside the depth range: {}", p.z);
    }

    /// Depth runs from the near plane to the range: just past the light is
    /// at the front, the range at the back, beyond it outside.
    #[test]
    fn depth_spans_the_range() {
        let vp = spot_shadow_view_proj(Vec3::ZERO, Vec3::NEG_Y, 0.5, 10.0);
        assert!(vp.project_point3(Vec3::new(0.0, -0.11, 0.0)).z < 0.1);
        assert!((vp.project_point3(Vec3::new(0.0, -10.0, 0.0)).z - 1.0).abs() < 1e-4);
        assert!(vp.project_point3(Vec3::new(0.0, -10.5, 0.0)).z > 1.0);
    }

    #[test]
    fn the_uniform_matches_the_shader() {
        assert_eq!(std::mem::size_of::<SpotShadowUniform>(), 8 * 64);
    }
}

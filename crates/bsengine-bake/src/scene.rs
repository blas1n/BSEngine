//! What the baker is given: triangles, lights and a sky, in world space.
//!
//! Every light's irradiance is the mesh shader's own formula moved to the CPU
//! (`MESH_WGSL` in `bsengine-rhi-wgpu`): `color * N.L` for the sun,
//! `color * intensity * (1 - d/range)^2 * N.L` for a point light -- a range
//! falloff, not an inverse square --, the same with the cone's smoothstep for a
//! spot, and `radiance * integral(cos)` under the `(1 - (d/range)^4)^2`
//! window for a rect light. A baked light that disagreed with its realtime
//! self would change brightness the moment a surface got a lightmap.
//!
//! Not baked, as the runtime has them: cookies and IES profiles, and each
//! light's `shadows` switch -- in a bake every light is occluded by the static
//! geometry, which is the point of baking it.

use crate::bvh::Bvh;
use crate::sample::Rng;
use glam::{Vec2, Vec3};
use std::f32::consts::PI;

/// One static triangle, with what the path tracer needs to shade it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BakeTriangle {
    /// World-space corners.
    pub positions: [Vec3; 3],
    /// World-space vertex normals. Texels interpolate them; the path tracer
    /// shades hits with the flat geometric normal.
    pub normals: [Vec3; 3],
    /// Lightmap coordinates of the renderer's own `[0,1]²` space.
    pub uv2: [Vec2; 3],
    /// Diffuse reflectance, linear.
    pub albedo: Vec3,
    /// Emitted radiance, linear, as the material's `emissive`.
    pub emissive: Vec3,
    /// Which renderer (index into the bake's renderer list) it belongs to.
    pub renderer: u32,
}

/// Whether a light's direct light is baked as well as its bounces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contribution {
    /// Bounces only: the runtime still draws its direct light.
    Indirect,
    /// Direct light and shadows too; the runtime skips it on lightmapped
    /// surfaces.
    Baked,
}

/// A light, in the units the mesh shader uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BakeLight {
    Directional {
        /// The direction the light travels (the shader's `light.direction`).
        direction: Vec3,
        /// Colour times intensity.
        color: Vec3,
        contribution: Contribution,
    },
    Point {
        position: Vec3,
        color: Vec3,
        intensity: f32,
        range: f32,
        contribution: Contribution,
    },
    Spot {
        position: Vec3,
        /// The way the cone points.
        direction: Vec3,
        color: Vec3,
        intensity: f32,
        range: f32,
        inner_cos: f32,
        outer_cos: f32,
        contribution: Contribution,
    },
    Rect {
        /// Centre of the panel.
        position: Vec3,
        /// Half the panel along its width and height, in world space. It
        /// shines out of the side `half_width x half_height` points away from.
        half_width: Vec3,
        half_height: Vec3,
        /// `color * intensity`: the radiance the panel emits.
        radiance: Vec3,
        range: f32,
        contribution: Contribution,
    },
}

impl BakeLight {
    pub fn contribution(&self) -> Contribution {
        match *self {
            Self::Directional { contribution, .. }
            | Self::Point { contribution, .. }
            | Self::Spot { contribution, .. }
            | Self::Rect { contribution, .. } => contribution,
        }
    }
}

/// Points on a rect light per irradiance estimate, stratified 4x4.
const RECT_SAMPLES: u32 = 4;
/// How far a shadow ray starts off the surface, and stops short of a light.
pub(crate) const EPS: f32 = 1e-3;

/// Irradiance `E` from `light` at `p` on a surface facing `n`, shadowed by
/// `bvh`. The mesh shader's diffuse is `kd * albedo / PI * E`.
pub(crate) fn irradiance(light: &BakeLight, p: Vec3, n: Vec3, bvh: &Bvh, rng: &mut Rng) -> Vec3 {
    match *light {
        BakeLight::Directional {
            direction, color, ..
        } => {
            let l = -direction.normalize();
            let n_dot_l = n.dot(l);
            if n_dot_l <= 0.0 || bvh.occluded(p, l, f32::INFINITY) {
                return Vec3::ZERO;
            }
            color * n_dot_l
        }
        BakeLight::Point {
            position,
            color,
            intensity,
            range,
            ..
        } => {
            let to = position - p;
            let d = to.length();
            if d >= range || d <= 0.0 {
                return Vec3::ZERO;
            }
            let l = to / d;
            let n_dot_l = n.dot(l);
            if n_dot_l <= 0.0 || bvh.occluded(p, l, d - EPS) {
                return Vec3::ZERO;
            }
            let t = 1.0 - d / range;
            color * (intensity * t * t) * n_dot_l
        }
        BakeLight::Spot {
            position,
            direction,
            color,
            intensity,
            range,
            inner_cos,
            outer_cos,
            ..
        } => {
            let to = position - p;
            let d = to.length();
            if d >= range || d <= 0.0 {
                return Vec3::ZERO;
            }
            let l = to / d;
            let spot = smoothstep(outer_cos, inner_cos, (-l).dot(direction.normalize()));
            let n_dot_l = n.dot(l);
            if spot <= 0.0 || n_dot_l <= 0.0 || bvh.occluded(p, l, d - EPS) {
                return Vec3::ZERO;
            }
            let t = 1.0 - d / range;
            color * (intensity * t * t * spot) * n_dot_l
        }
        BakeLight::Rect {
            position,
            half_width,
            half_height,
            radiance,
            range,
            ..
        } => {
            let d = (position - p).length() / range.max(1e-4);
            let w = (1.0 - d * d * d * d).clamp(0.0, 1.0);
            let window = w * w;
            if window <= 0.0 {
                return Vec3::ZERO;
            }
            let cross = half_width.cross(half_height);
            let area = 4.0 * cross.length();
            if area <= 0.0 {
                return Vec3::ZERO;
            }
            let light_n = -cross.normalize();
            // Stratified points on the panel: E = L * integral(cos_s * cos_l / r^2 dA).
            let mut sum = 0.0;
            for i in 0..RECT_SAMPLES {
                for j in 0..RECT_SAMPLES {
                    let u = (i as f32 + rng.next()) / RECT_SAMPLES as f32 * 2.0 - 1.0;
                    let v = (j as f32 + rng.next()) / RECT_SAMPLES as f32 * 2.0 - 1.0;
                    let q = position + half_width * u + half_height * v;
                    let to = q - p;
                    let r2 = to.length_squared();
                    if r2 <= 1e-12 {
                        continue;
                    }
                    let r = r2.sqrt();
                    let l = to / r;
                    let cos_s = n.dot(l);
                    let cos_l = light_n.dot(-l);
                    if cos_s <= 0.0 || cos_l <= 0.0 || bvh.occluded(p, l, r - EPS) {
                        continue;
                    }
                    sum += cos_s * cos_l / r2;
                }
            }
            radiance * (sum * area / (RECT_SAMPLES * RECT_SAMPLES) as f32) * window
        }
    }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What a ray that leaves the scene sees.
#[derive(Debug, Clone, PartialEq)]
pub enum Sky {
    /// The same radiance from every direction: the runtime's flat `ambient`,
    /// which it adds as `ambient * albedo` -- exactly a uniform sky's
    /// `E / PI`.
    Uniform(Vec3),
    /// An equirectangular panorama, linear and already multiplied by the
    /// skybox's intensity, read by direction exactly as the sky shader reads
    /// it: `u = atan2(z, x) / 2pi + 1/2`, `v = 1/2 - asin(y) / pi`.
    Equirect {
        width: u32,
        height: u32,
        texels: Vec<Vec3>,
    },
}

impl Sky {
    pub fn radiance(&self, dir: Vec3) -> Vec3 {
        match self {
            Self::Uniform(l) => *l,
            Self::Equirect {
                width,
                height,
                texels,
            } => {
                if texels.is_empty() {
                    return Vec3::ZERO;
                }
                let u = dir.z.atan2(dir.x) / (2.0 * PI) + 0.5;
                let v = 0.5 - dir.y.clamp(-1.0, 1.0).asin() / PI;
                let x = ((u * *width as f32) as u32).min(width - 1);
                let y = ((v * *height as f32) as u32).min(height - 1);
                texels[(y * width + x) as usize]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nothing() -> Bvh {
        Bvh::build(&[])
    }

    /// The point light's falloff is the shader's `(1 - d/range)^2`, not an
    /// inverse square: at half the range a quarter of the intensity, facing it.
    #[test]
    fn a_point_light_falls_off_as_the_shader_does() {
        let light = BakeLight::Point {
            position: Vec3::new(0.0, 2.0, 0.0),
            color: Vec3::ONE,
            intensity: 3.0,
            range: 4.0,
            contribution: Contribution::Indirect,
        };
        let e = irradiance(&light, Vec3::ZERO, Vec3::Y, &nothing(), &mut Rng::new(0));
        assert!((e.x - 3.0 * 0.25).abs() < 1e-5, "{e}");
        let beyond = irradiance(
            &light,
            Vec3::new(0.0, -3.0, 0.0),
            Vec3::Y,
            &nothing(),
            &mut Rng::new(0),
        );
        assert_eq!(beyond, Vec3::ZERO, "past the range, nothing");
    }

    /// Closed form: a small square panel of side `s` straight above a patch
    /// at height `h`, facing it, gives `E ~= L * s^2 / h^2` (the far-field
    /// limit, where cos ~= 1 on both ends). Pins the area, the sign of the
    /// panel's normal and the sample weighting.
    #[test]
    fn a_rect_light_gives_the_far_field_irradiance() {
        let (s, h) = (0.2f32, 2.0f32);
        let light = BakeLight::Rect {
            position: Vec3::new(0.0, h, 0.0),
            // half_width x half_height = X x -Z = +Y, so it shines down (-Y).
            half_width: Vec3::X * (s / 2.0),
            half_height: Vec3::NEG_Z * (s / 2.0),
            radiance: Vec3::splat(10.0),
            range: 1000.0,
            contribution: Contribution::Baked,
        };
        let e = irradiance(&light, Vec3::ZERO, Vec3::Y, &nothing(), &mut Rng::new(0));
        let want = 10.0 * s * s / (h * h);
        assert!((e.x / want - 1.0).abs() < 0.01, "{} vs {want}", e.x);
        // Turned to shine up, it lights nothing below it.
        let up = BakeLight::Rect {
            position: Vec3::new(0.0, h, 0.0),
            half_width: Vec3::X * (s / 2.0),
            half_height: Vec3::Z * (s / 2.0),
            radiance: Vec3::splat(10.0),
            range: 1000.0,
            contribution: Contribution::Baked,
        };
        assert_eq!(
            irradiance(&up, Vec3::ZERO, Vec3::Y, &nothing(), &mut Rng::new(0)),
            Vec3::ZERO
        );
    }

    /// An occluder between the surface and the sun blocks it.
    #[test]
    fn a_shadow_ray_blocks_the_sun() {
        let roof = [
            Vec3::new(-5.0, 1.0, -5.0),
            Vec3::new(5.0, 1.0, -5.0),
            Vec3::new(0.0, 1.0, 5.0),
        ];
        let sun = BakeLight::Directional {
            direction: Vec3::NEG_Y,
            color: Vec3::ONE,
            contribution: Contribution::Baked,
        };
        let open = irradiance(&sun, Vec3::ZERO, Vec3::Y, &nothing(), &mut Rng::new(0));
        let shaded = irradiance(
            &sun,
            Vec3::ZERO,
            Vec3::Y,
            &Bvh::build(&[roof]),
            &mut Rng::new(0),
        );
        assert_eq!(open, Vec3::ONE, "premise: unshaded it is lit");
        assert_eq!(shaded, Vec3::ZERO);
    }

    /// The panorama is read with the sky shader's mapping: straight up is
    /// the top row, and +X is the middle column's right-hand neighbour
    /// boundary (u = 0.5).
    #[test]
    fn the_sky_is_read_as_the_sky_shader_reads_it() {
        let (w, h) = (4u32, 2u32);
        let mut texels = vec![Vec3::ZERO; 8];
        texels[2] = Vec3::X; // top row, u in [0.5, 0.75)
        let sky = Sky::Equirect {
            width: w,
            height: h,
            texels,
        };
        // Up and slightly toward +X, +Z: u just above 0.5, v near 0.
        let dir = Vec3::new(0.2, 0.9, 0.05).normalize();
        assert_eq!(sky.radiance(dir), Vec3::X);
        assert_eq!(
            sky.radiance(Vec3::NEG_Y),
            Vec3::ZERO,
            "the bottom row is black"
        );
    }
}

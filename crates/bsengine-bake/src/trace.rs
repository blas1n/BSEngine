//! The path tracer: what one lightmap texel receives.
//!
//! A texel stores `E / PI` -- irradiance over pi, the units the runtime's
//! irradiance cube and probe grid store, so the mesh shader's
//! `lightmap * albedo * kd` replaces either without a conversion. With
//! cosine-weighted directions that is the plain mean of the radiance arriving
//! along them (`sample::cosine_hemisphere`).
//!
//! The radiance leaving a surface the ray hits is `emissive + albedo *
//! (E_direct / PI + incoming)`, `incoming` being the next bounce: a
//! Lambertian surface, which is all the runtime's diffuse term assumes.
//! Every non-realtime light lights those surfaces -- an `Indirect` light's
//! bounce is baked even though its direct light is not.

use crate::bvh::Bvh;
use crate::sample::{cosine_hemisphere, Rng};
use crate::scene::{irradiance, BakeLight, BakeTriangle, Contribution, Sky, EPS};
use glam::Vec3;
use std::f32::consts::PI;

/// What one texel bakes to: its irradiance and where that light came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Texel {
    /// `E / PI`, linear.
    pub irradiance: Vec3,
    /// The luminance-weighted mean direction the light arrived from, in world
    /// space; at most unit length, zero for a texel that received none
    /// ([`World::texel_directional`]).
    pub direction: Vec3,
}

/// The luminance a directional lightmap weighs directions by: Rec. 709, as
/// the renderer's own luma.
pub fn luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// How much work a texel gets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceSettings {
    /// Rays per texel. Noise falls as `1/sqrt(samples)`.
    pub samples: u32,
    /// Surface interactions a path may have after leaving the texel; 1 means
    /// light that bounced once (the most visible part of indirect light).
    pub bounces: u32,
    /// Seed for every texel's generator; a fixed seed is a fixed bake.
    pub seed: u64,
}

impl Default for TraceSettings {
    fn default() -> Self {
        Self {
            samples: 128,
            bounces: 3,
            seed: 0,
        }
    }
}

/// The scene as the tracer reads it.
pub struct World<'a> {
    pub bvh: &'a Bvh,
    pub triangles: &'a [BakeTriangle],
    pub lights: &'a [BakeLight],
    pub sky: &'a Sky,
    pub settings: TraceSettings,
}

impl World<'_> {
    /// `E / PI` at a texel at `p` facing `n`: the light arriving from the sky
    /// and from every surface it can see, plus -- when `direct` -- the direct
    /// light of every `Baked` light.
    pub fn texel(&self, p: Vec3, n: Vec3, direct: bool, rng: &mut Rng) -> Vec3 {
        self.texel_directional(p, n, direct, rng).irradiance
    }

    /// [`Self::texel`], and where that light came from: the mean of every
    /// sample's direction weighted by the luminance it brought (a baked
    /// light's direct light likewise, from the light's direction). A unit
    /// vector for light from one direction, shorter the more it is spread
    /// (2/3 along `n` under a uniform sky), and zero for none -- Unity's
    /// "dominant direction" lightmap, with its "directionality" as the
    /// length.
    pub fn texel_directional(&self, p: Vec3, n: Vec3, direct: bool, rng: &mut Rng) -> Texel {
        let origin = p + n * EPS;
        let samples = self.settings.samples.max(1);
        let mut sum = Vec3::ZERO;
        let mut toward = Vec3::ZERO;
        let mut weight = 0.0;
        for _ in 0..samples {
            let dir = cosine_hemisphere(n, rng.next_f32(), rng.next_f32());
            let l = self.incoming(origin, dir, 1, rng);
            sum += l;
            toward += dir * luminance(l);
            weight += luminance(l);
        }
        let mut result = sum / samples as f32;
        // In units of the samples' mean, so a direct term weighs what it
        // adds to the irradiance.
        let (mut toward, mut weight) = (toward / samples as f32, weight / samples as f32);
        if direct {
            for light in self.lights {
                if light.contribution() == Contribution::Baked {
                    let e = irradiance(light, origin, n, self.bvh, rng) / PI;
                    result += e;
                    toward += crate::scene::light_direction(light, origin) * luminance(e);
                    weight += luminance(e);
                }
            }
        }
        Texel {
            irradiance: result,
            direction: if weight > 0.0 {
                toward / weight
            } else {
                Vec3::ZERO
            },
        }
    }

    /// Radiance arriving at `origin` from `dir`, bounces and all: what one
    /// ray of a light probe sees. The reference the renderer's realtime probe
    /// volume is held to.
    pub fn radiance(&self, origin: Vec3, dir: Vec3, rng: &mut Rng) -> Vec3 {
        self.incoming(origin, dir, 1, rng)
    }

    /// Radiance arriving at `origin` from `dir`.
    fn incoming(&self, origin: Vec3, dir: Vec3, depth: u32, rng: &mut Rng) -> Vec3 {
        let Some(hit) = self.bvh.closest(origin, dir, f32::INFINITY) else {
            return self.sky.radiance(dir);
        };
        let tri = &self.triangles[hit.triangle as usize];
        let [a, b, c] = tri.positions;
        let mut ng = (b - a).cross(c - a).normalize_or_zero();
        // Shade with the normal facing the side the authored vertex normals
        // face, so a triangle's winding does not decide which side is lit.
        if ng.dot(tri.normals[0] + tri.normals[1] + tri.normals[2]) < 0.0 {
            ng = -ng;
        }
        // The back of a surface is inside something: nothing comes out of it.
        if ng.dot(dir) >= 0.0 {
            return Vec3::ZERO;
        }
        let y = origin + dir * hit.t + ng * EPS;
        let mut e_direct = Vec3::ZERO;
        for light in self.lights {
            e_direct += irradiance(light, y, ng, self.bvh, rng);
        }
        let mut reflected = e_direct / PI;
        if depth < self.settings.bounces {
            let next = cosine_hemisphere(ng, rng.next_f32(), rng.next_f32());
            reflected += self.incoming(y, next, depth + 1, rng);
        }
        tri.emissive + tri.albedo * reflected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn quad(min: Vec3, u: Vec3, v: Vec3, albedo: Vec3) -> [BakeTriangle; 2] {
        let n = u.cross(v).normalize();
        let tri = |p: [Vec3; 3]| BakeTriangle {
            positions: p,
            normals: [n; 3],
            uv2: [Vec2::ZERO; 3],
            albedo,
            emissive: Vec3::ZERO,
            renderer: 0,
        };
        [
            tri([min, min + u, min + u + v]),
            tri([min, min + u + v, min + v]),
        ]
    }

    fn world<'a>(
        bvh: &'a Bvh,
        tris: &'a [BakeTriangle],
        lights: &'a [BakeLight],
        sky: &'a Sky,
        samples: u32,
    ) -> World<'a> {
        World {
            bvh,
            triangles: tris,
            lights,
            sky,
            settings: TraceSettings {
                samples,
                bounces: 3,
                seed: 0,
            },
        }
    }

    fn bvh_of(tris: &[BakeTriangle]) -> Bvh {
        Bvh::build(&tris.iter().map(|t| t.positions).collect::<Vec<_>>())
    }

    /// Closed form: an open surface under a uniform sky of radiance `L`
    /// receives `E = PI * L`, so it stores exactly `L`. Exact, not
    /// statistical -- every cosine-weighted ray sees the same `L`.
    #[test]
    fn an_open_surface_under_a_uniform_sky_stores_the_sky() {
        let sky = Sky::Uniform(Vec3::new(0.2, 0.4, 0.8));
        let bvh = Bvh::build(&[]);
        let w = world(&bvh, &[], &[], &sky, 16);
        let got = w.texel(Vec3::ZERO, Vec3::Y, false, &mut Rng::new(1));
        assert!((got - Vec3::new(0.2, 0.4, 0.8)).length() < 1e-5, "{got}");
    }

    /// Closed form: at the foot of a tall black wall, the wall hides half the
    /// hemisphere in azimuth, and the cosine-weighted half is exactly half
    /// the irradiance. The large finite wall makes it very nearly 0.5.
    #[test]
    fn a_black_wall_beside_the_texel_hides_half_the_sky() {
        let sky = Sky::Uniform(Vec3::ONE);
        // A 200 x 200 wall in the plane x = 0.01, facing -X (towards the texel).
        let wall = quad(
            Vec3::new(0.01, -100.0, -100.0),
            Vec3::new(0.0, 200.0, 0.0),
            Vec3::new(0.0, 0.0, 200.0),
            Vec3::ZERO,
        );
        let bvh = bvh_of(&wall);
        let w = world(&bvh, &wall, &[], &sky, 20_000);
        let got = w.texel(Vec3::ZERO, Vec3::Y, false, &mut Rng::new(2));
        assert!((got.x - 0.5).abs() < 0.02, "half the sky: {got}");
    }

    /// The colour of a lit wall bleeds onto the floor beside it, and only
    /// through the bounce: the floor itself is not lit (the sun is below its
    /// horizon), the sky is black, and the wall is red. With the light
    /// excluded from the bake the floor is black -- the hard side, which a
    /// tracer that ignored lights on hit surfaces would also give.
    #[test]
    fn a_lit_red_wall_bleeds_red_onto_the_floor() {
        let sky = Sky::Uniform(Vec3::ZERO);
        // Wall in x = 1 facing -X, red.
        let wall = quad(
            Vec3::new(1.0, 0.0, -2.0),
            Vec3::new(0.0, 0.0, 4.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.9, 0.1, 0.1),
        );
        // The sun travels +X, onto the wall's -X face. It grazes the floor's
        // horizon exactly (no -Y component), so the floor gets no direct light
        // -- and the texel is asked for its indirect light only anyway.
        let sun = [BakeLight::Directional {
            direction: Vec3::X,
            color: Vec3::splat(3.0),
            contribution: Contribution::Indirect,
        }];
        let bvh = bvh_of(&wall);
        let lit = world(&bvh, &wall, &sun, &sky, 4000);
        let got = lit.texel(Vec3::new(0.5, 0.0, 0.0), Vec3::Y, false, &mut Rng::new(3));
        assert!(
            got.x > 0.05,
            "premise: the wall's bounce reaches the floor: {got}"
        );
        assert!(
            got.x > 5.0 * got.y && got.x > 5.0 * got.z,
            "and it is red: {got}"
        );

        let dark = world(&bvh, &wall, &[], &sky, 4000);
        let none = dark.texel(Vec3::new(0.5, 0.0, 0.0), Vec3::Y, false, &mut Rng::new(3));
        assert_eq!(none, Vec3::ZERO, "no light, no bounce");
    }

    /// An `Indirect` light's direct light is not in the texel -- the runtime
    /// draws it -- while a `Baked` light's is, at exactly its irradiance over
    /// pi. Nothing to bounce off and a black sky, so the direct term is all
    /// there is.
    #[test]
    fn only_a_baked_lights_direct_light_is_in_the_texel() {
        let sky = Sky::Uniform(Vec3::ZERO);
        let bvh = Bvh::build(&[]);
        let sun = |contribution| BakeLight::Directional {
            direction: Vec3::NEG_Y,
            color: Vec3::splat(2.0),
            contribution,
        };
        let indirect = [sun(Contribution::Indirect)];
        let baked = [sun(Contribution::Baked)];
        let i =
            world(&bvh, &[], &indirect, &sky, 8).texel(Vec3::ZERO, Vec3::Y, true, &mut Rng::new(4));
        let b =
            world(&bvh, &[], &baked, &sky, 8).texel(Vec3::ZERO, Vec3::Y, true, &mut Rng::new(4));
        assert_eq!(
            i,
            Vec3::ZERO,
            "an Indirect light's direct light is the runtime's"
        );
        assert!(
            (b.x - 2.0 / PI).abs() < 1e-5,
            "a Baked light's is E / pi: {b}"
        );
    }

    /// Closed form: under a uniform sky the light comes from the whole
    /// hemisphere, and the mean of cosine-weighted directions is `2/3 n` --
    /// a direction along the normal, two thirds long ("directionality" 2/3).
    #[test]
    fn a_uniform_sky_arrives_along_the_normal_two_thirds_strong() {
        let sky = Sky::Uniform(Vec3::splat(0.5));
        let bvh = Bvh::build(&[]);
        let w = world(&bvh, &[], &[], &sky, 40_000);
        let t = w.texel_directional(Vec3::ZERO, Vec3::Y, false, &mut Rng::new(6));
        assert!(
            (t.direction - Vec3::new(0.0, 2.0 / 3.0, 0.0)).length() < 0.01,
            "{}",
            t.direction
        );
        assert_eq!(
            t.irradiance,
            w.texel(Vec3::ZERO, Vec3::Y, false, &mut Rng::new(6))
        );
    }

    /// A baked sun alone -- black sky, nothing to bounce off -- is light from
    /// one direction: the texel's direction is exactly toward the sun, unit
    /// length, slanted as the sun is.
    #[test]
    fn a_baked_sun_alone_points_the_direction_at_the_sun() {
        let sky = Sky::Uniform(Vec3::ZERO);
        let bvh = Bvh::build(&[]);
        let sun = [BakeLight::Directional {
            direction: Vec3::new(1.0, -1.0, 0.0).normalize(),
            color: Vec3::splat(2.0),
            contribution: Contribution::Baked,
        }];
        let t = world(&bvh, &[], &sun, &sky, 8).texel_directional(
            Vec3::ZERO,
            Vec3::Y,
            true,
            &mut Rng::new(7),
        );
        let toward = Vec3::new(-1.0, 1.0, 0.0).normalize();
        assert!((t.direction - toward).length() < 1e-5, "{}", t.direction);
    }

    /// Bounced light carries its direction too: the floor beside the lit red
    /// wall at +x receives its light from that side, so the direction leans
    /// toward +x -- which a uniform sky's (straight up) and a do-nothing
    /// implementation's (zero) both fail.
    #[test]
    fn bounced_light_points_back_at_the_surface_it_came_from() {
        let sky = Sky::Uniform(Vec3::ZERO);
        let wall = quad(
            Vec3::new(1.0, 0.0, -2.0),
            Vec3::new(0.0, 0.0, 4.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.9, 0.1, 0.1),
        );
        let sun = [BakeLight::Directional {
            direction: Vec3::X,
            color: Vec3::splat(3.0),
            contribution: Contribution::Indirect,
        }];
        let bvh = bvh_of(&wall);
        let t = world(&bvh, &wall, &sun, &sky, 4000).texel_directional(
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::Y,
            false,
            &mut Rng::new(3),
        );
        assert!(
            t.direction.x > 0.3 && t.direction.x > 2.0 * t.direction.z.abs(),
            "from the wall's side: {}",
            t.direction
        );
    }

    /// The same seed gives the same texel, bit for bit.
    #[test]
    fn a_texel_is_deterministic() {
        let sky = Sky::Uniform(Vec3::ONE);
        let wall = quad(
            Vec3::new(0.5, 0.0, -1.0),
            Vec3::Z * 2.0,
            Vec3::Y,
            Vec3::splat(0.5),
        );
        let bvh = bvh_of(&wall);
        let w = world(&bvh, &wall, &[], &sky, 64);
        let a = w.texel(Vec3::ZERO, Vec3::Y, false, &mut Rng::for_texel(5, 0, 1, 2));
        let b = w.texel(Vec3::ZERO, Vec3::Y, false, &mut Rng::for_texel(5, 0, 1, 2));
        assert_eq!(
            a.to_array().map(f32::to_bits),
            b.to_array().map(f32::to_bits)
        );
    }
}

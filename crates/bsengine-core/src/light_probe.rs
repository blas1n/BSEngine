//! Light-probe volume for indirect lighting: baked once, or kept up to date
//! by ray tracing every frame.

use crate::ReflectVec3;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Largest number of probes a baked volume may hold. A volume asking for
/// more is clamped rather than rejected -- an over-specified scene should
/// still render.
///
/// The limit is the bake's, not the grid's: every probe is captured by six
/// scene passes into a cube array of `MAX_BAKED_PROBES * 6` layers, and 256
/// layers is the downlevel limit a texture array may have.
pub const MAX_BAKED_PROBES: usize = 32;

/// Largest number of probes the GPU grid holds: the storage buffer the scene
/// shader reads, and its index clamp. Larger than [`MAX_BAKED_PROBES`] so a
/// grid filled by other means than the raster bake is not held to its limit.
pub const MAX_PROBES: usize = 4096;

/// Largest number of probes an adaptive volume ([`ProbePlacement::Adaptive`])
/// holds -- its bricks are refined until they would pass this. The grid's
/// buffer grows to hold them (`bsengine_rhi_wgpu`'s probe buffer starts at
/// [`MAX_PROBES`], 9.7 MiB, and doubles as an adaptive layout needs, to about
/// 78 MiB here), so a scene without one pays nothing for the headroom. Unity's
/// Adaptive Probe Volumes have no fixed cap; this one is under WebGPU's
/// default 128 MiB storage-binding limit.
pub const MAX_ADAPTIVE_PROBES: usize = 32768;

/// Largest number of bricks an adaptive volume ([`ProbePlacement::Adaptive`])
/// holds -- the brick table's size in the same buffer. Even packed as
/// densely as bricks go, sharing every face, each brick brings 27 probes of
/// its own, so a layout within [`MAX_PROBES`] has about a 27th as many bricks;
/// an eighth leaves room for the mixed levels between. The layout holds to
/// both budgets; a buffer of `n` probes holds `n / 8` bricks.
pub const MAX_PROBE_BRICKS: usize = MAX_ADAPTIVE_PROBES / 8;

// Every baked volume has to fit the grid it is uploaded into.
const _: () = assert!(MAX_BAKED_PROBES <= MAX_PROBES);

/// How a probe keeps what it captured: a [`LightProbeVolume`]'s probes, or a
/// [`crate::ReflectionProbe`] (Unity's Reflection Probe "Refresh Mode",
/// Godot's `ReflectionProbe.update_mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
#[reflect(Default)]
pub enum ProbeUpdate {
    /// Captured once, when the probe appears or its own settings change.
    /// Sees everything the camera would, moving objects and textures
    /// included, but not what changes afterwards. A light-probe volume bakes
    /// at most [`MAX_BAKED_PROBES`] probes this way.
    #[default]
    Once,
    /// Kept up to date while the game runs.
    ///
    /// A light-probe volume's probes are ray traced every frame against the
    /// scene's renderers with the current lights and sky, bounce after
    /// bounce: lights switched on, off or moved, the sky changing, and
    /// objects moving show in the indirect light a few frames later.
    /// `LightmapStatic` renderers are one tree, rebuilt when that set
    /// changes; every other one is traced where it is each frame unless it is
    /// `ExcludeFromGi` (skinned meshes are left out). Surfaces bounce their
    /// material's base colour (no textures). At most [`MAX_PROBES`].
    ///
    /// A reflection probe is captured again, one cube face a frame and
    /// filtered once its six faces are in -- Unity's "Individual faces" time
    /// slicing -- so it shows the scene as it is, moving objects included,
    /// twelve frames behind at most: a change made just after its face was
    /// captured shows when the next refresh ends. Realtime probes take turns,
    /// so with n of them it is 12 n.
    Realtime,
}

/// Where a [`LightProbeVolume`] puts its probes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
#[reflect(Default)]
pub enum ProbePlacement {
    /// A regular grid of [`LightProbeVolume::resolution`] probes.
    #[default]
    Grid,
    /// Bricks of 4 x 4 x 4 probes, fine near geometry and coarse in open
    /// space -- Unity's Adaptive Probe Volumes, Unreal's Volumetric Lightmap,
    /// Godot's LightmapGI: between [`LightProbeVolume::min_probe_spacing`]
    /// near the `LightmapStatic` geometry and
    /// [`LightProbeVolume::max_probe_spacing`] away from it, each level three
    /// times finer than the last (`bsengine_bake::probe_bricks`).
    /// `resolution` is not used. A [`ProbeUpdate::Realtime`] volume cuts its
    /// bricks when the static set or the volume changes, which starts its
    /// probes over; a [`ProbeUpdate::Once`] one is baked by
    /// `bsengine-runtime --bake` with the lightmaps ([`BakedLightProbes`]),
    /// and until it is, bakes its grid (the raster bake holds at most
    /// [`MAX_BAKED_PROBES`]) with a warning.
    Adaptive,
}

/// Where an adaptive volume's baked probes are: `assets/lightmaps/<scene>/
/// probes.bin`, written by `bsengine-runtime --bake` beside the lightmaps and
/// given to the volume when its scene loads (`bsengine_scene::lightmaps`).
/// With it, a [`ProbePlacement::Adaptive`] volume baked [`ProbeUpdate::Once`]
/// shades from its bricks; without it, it bakes its grid.
#[derive(Component, Debug, Clone, PartialEq, Default, Reflect)]
#[reflect(Component, Default)]
pub struct BakedLightProbes {
    /// Project-relative, as scene files write paths.
    pub path: String,
}

/// A box filled with light probes, giving position-varying indirect light
/// inside it: a regular grid, or adaptive bricks ([`ProbePlacement`]).
///
/// Probes sit on the grid's **lattice points**, not cell centres: trilinear
/// interpolation is defined by the eight corners of a cell, so corner
/// placement is what makes the interpolation well-formed.
///
/// Absent means no probe GI, and the existing IBL path runs unchanged.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct LightProbeVolume {
    /// Half-extents of the box, relative to the entity's `Transform`.
    pub half_extents: ReflectVec3,
    /// Probe count along each axis. The product is clamped to
    /// [`MAX_BAKED_PROBES`] for a baked volume and [`MAX_PROBES`] for a
    /// realtime one. `[4, 2, 4]` gives exactly 32.
    pub resolution: [u32; 3],
    /// Baked once, or ray traced every frame.
    pub update: ProbeUpdate,
    /// Realtime only: rays each probe traces per update. More is less noise
    /// for more time. Rounded up to a multiple of 64 (one per thread of the
    /// update's workgroup) and clamped to 64..=256.
    pub rays_per_probe: u32,
    /// Realtime only: how much of a probe's previous light each update keeps,
    /// 0..1. Higher is smoother and slower to follow a change; the DDGI
    /// default of 0.97 settles in about a hundred updates.
    pub hysteresis: f32,
    /// A regular grid, or adaptive bricks.
    pub placement: ProbePlacement,
    /// Adaptive only: the probe spacing near geometry, in metres.
    pub min_probe_spacing: f32,
    /// Adaptive only: the probe spacing in open space, in metres. The box is
    /// cut into whole bricks of three spacings, so the spacing used is this or
    /// a little less.
    pub max_probe_spacing: f32,
}

impl Default for LightProbeVolume {
    fn default() -> Self {
        Self {
            half_extents: glam::Vec3::splat(5.0).into(),
            resolution: [4, 2, 4],
            update: ProbeUpdate::Once,
            rays_per_probe: 64,
            hysteresis: 0.97,
            placement: ProbePlacement::Grid,
            // Unity APV's defaults are 1 and 27 m (three levels); a game
            // scene here is rooms, so finer: three levels from 4.5 to 0.5.
            min_probe_spacing: 0.5,
            max_probe_spacing: 4.5,
        }
    }
}

impl LightProbeVolume {
    /// Resolution with each axis at least 2 (one probe per axis cannot
    /// interpolate) and the product clamped to [`Self::max_probes`].
    pub fn clamped_resolution(&self) -> [u32; 3] {
        let max = self.max_probes();
        let mut r = [
            self.resolution[0].max(2),
            self.resolution[1].max(2),
            self.resolution[2].max(2),
        ];
        while (r[0] * r[1] * r[2]) as usize > max {
            // Shrink the largest axis first, keeping the grid as even as
            // possible rather than collapsing one direction.
            let largest = if r[0] >= r[1] && r[0] >= r[2] {
                0
            } else if r[1] >= r[2] {
                1
            } else {
                2
            };
            if r[largest] <= 2 {
                break;
            }
            r[largest] -= 1;
        }
        r
    }

    /// How many probes this volume may hold: the bake's limit, or the grid's.
    pub fn max_probes(&self) -> usize {
        match self.update {
            ProbeUpdate::Once => MAX_BAKED_PROBES,
            ProbeUpdate::Realtime => MAX_PROBES,
        }
    }

    /// [`Self::rays_per_probe`] as the update runs it: a multiple of 64 in
    /// 64..=256.
    pub fn clamped_rays_per_probe(&self) -> u32 {
        self.rays_per_probe.clamp(64, 256).div_ceil(64) * 64
    }

    /// [`Self::hysteresis`] in 0..=0.999: 1 would never let a probe change.
    pub fn clamped_hysteresis(&self) -> f32 {
        if self.hysteresis.is_finite() {
            self.hysteresis.clamp(0.0, 0.999)
        } else {
            0.97
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_resolution_fits_max_probes_exactly() {
        let v = LightProbeVolume::default();
        let r = v.clamped_resolution();
        assert_eq!((r[0] * r[1] * r[2]) as usize, MAX_BAKED_PROBES);
    }

    #[test]
    fn an_oversized_resolution_is_clamped_not_rejected() {
        let v = LightProbeVolume {
            resolution: [16, 16, 16],
            ..Default::default()
        };
        let r = v.clamped_resolution();
        assert!((r[0] * r[1] * r[2]) as usize <= MAX_BAKED_PROBES);
        assert!(
            r.iter().all(|&a| a >= 2),
            "every axis must keep >= 2 probes"
        );
    }

    /// A realtime volume is held to the grid, not to the bake: the same
    /// 16^3 request is 4096 probes realtime and at most 32 baked.
    #[test]
    fn a_realtime_volume_may_hold_the_whole_grid() {
        let realtime = LightProbeVolume {
            resolution: [16, 16, 16],
            update: ProbeUpdate::Realtime,
            ..Default::default()
        };
        assert_eq!(realtime.clamped_resolution(), [16, 16, 16]);
        let baked = LightProbeVolume {
            update: ProbeUpdate::Once,
            ..realtime
        };
        let r = baked.clamped_resolution();
        assert!((r[0] * r[1] * r[2]) as usize <= MAX_BAKED_PROBES, "{r:?}");
        let over = LightProbeVolume {
            resolution: [32, 32, 32],
            ..realtime
        };
        let r = over.clamped_resolution();
        assert!((r[0] * r[1] * r[2]) as usize <= MAX_PROBES, "{r:?}");
    }

    #[test]
    fn rays_and_hysteresis_are_clamped_to_what_the_update_runs() {
        let v = |rays, h| LightProbeVolume {
            rays_per_probe: rays,
            hysteresis: h,
            ..Default::default()
        };
        assert_eq!(v(0, 0.5).clamped_rays_per_probe(), 64);
        assert_eq!(v(65, 0.5).clamped_rays_per_probe(), 128);
        assert_eq!(v(1000, 0.5).clamped_rays_per_probe(), 256);
        assert_eq!(v(64, 2.0).clamped_hysteresis(), 0.999);
        assert_eq!(v(64, -1.0).clamped_hysteresis(), 0.0);
        assert_eq!(v(64, f32::NAN).clamped_hysteresis(), 0.97);
    }

    #[test]
    fn a_degenerate_resolution_is_raised_to_two_per_axis() {
        let v = LightProbeVolume {
            resolution: [1, 0, 1],
            ..Default::default()
        };
        assert_eq!(v.clamped_resolution(), [2, 2, 2]);
    }
}

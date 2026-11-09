//! The editor's light-probe display (`WgpuSurface::set_show_light_probes`),
//! at the pixel: each probe a sphere where it is, lit by its own SH, sized
//! by its spacing.
//!
//! An orthographic camera looks down -z at the volume, so a sphere's size on
//! screen is its size in the world whatever its depth, and the probes
//! nearest the camera (the volume's front face, z = 0.9) are the ones seen.

use bsengine_bake::probe_bake::{BakedProbe, BakedProbes, MOMENT_TEXELS};
use bsengine_bake::probe_bricks::{BrickLayout, BrickSettings};
use bsengine_rhi_wgpu::mesh::GpuMeshRegistry;
use bsengine_rhi_wgpu::surface::WgpuSurface;
use bsengine_rhi_wgpu::LightData;
use glam::{Mat4, Vec3};
use std::sync::Arc;

const W: u32 = 256;
const H: u32 = 128;
/// What the camera sees: x in -2..2, y in -1..1, so a pixel is 1/64 across.
const HALF_W: f32 = 2.0;
const HALF_H: f32 = 1.0;
const PIXEL: f32 = 2.0 * HALF_W / W as f32;

const RED: Vec3 = Vec3::new(0.8, 0.05, 0.05);
const BLUE: Vec3 = Vec3::new(0.05, 0.05, 0.8);

type DrawCall = (
    u64,
    Mat4,
    Option<u64>,
    bsengine_rhi_wgpu::MaterialParams,
    Option<String>,
);

struct Scene {
    surface: WgpuSurface,
    registry: GpuMeshRegistry,
    draw_calls: Vec<DrawCall>,
}

impl Scene {
    fn new() -> Self {
        let surface = WgpuSurface::offscreen_for_testing(W, H)
            .expect("these tests need an adapter; a skip would look like a pass");
        let registry = GpuMeshRegistry::new(surface.device_arc());
        Self {
            surface,
            registry,
            draw_calls: Vec::new(),
        }
    }

    /// Adds a quad, `corners` counter-clockwise from its front, black and
    /// emitting `emissive`.
    fn push_quad(&mut self, corners: [Vec3; 4], emissive: Vec3) {
        let normal = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .normalize();
        let vertices: Vec<bsengine_rhi_wgpu::mesh::Vertex> = corners
            .iter()
            .map(|p| bsengine_rhi_wgpu::mesh::Vertex {
                position: p.to_array(),
                color: [1.0, 1.0, 1.0],
                normal: normal.to_array(),
                uv: [0.0, 0.0],
                uv2: [0.0, 0.0],
                tangent: [0.0; 4],
            })
            .collect();
        let mesh_id = self.registry.register(&vertices, &[0, 1, 2, 0, 2, 3]);
        self.draw_calls.push((
            mesh_id,
            Mat4::IDENTITY,
            None,
            bsengine_rhi_wgpu::MaterialParams {
                roughness: 1.0,
                emissive,
                base_color: Vec3::ZERO,
                ..bsengine_rhi_wgpu::MaterialParams::default()
            },
            None,
        ));
    }

    /// One real `render_frame` of an empty scene through the orthographic
    /// camera, then the frame's pixels (RGBA8, top row first).
    fn render(&mut self) -> Vec<u8> {
        self.render_with(None)
    }

    /// [`Self::render`] with a light-probe volume the surface bakes (a grid).
    fn render_with(&mut self, volume: Option<bsengine_rhi_wgpu::ProbeVolumeParams>) -> Vec<u8> {
        let eye = Vec3::new(0.0, 0.0, 5.0);
        let proj = Mat4::orthographic_rh(-HALF_W, HALF_W, -HALF_H, HALF_H, 0.1, 10.0);
        let view_proj = proj * Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y);
        let ui_state = bsengine_core::UiState::default();
        self.surface
            .render_frame(
                view_proj,
                eye,
                &bsengine_rhi_wgpu::shadow::DirectionalCascades::new(
                    Vec3::new(0.0, -1.0, 0.0),
                    view_proj,
                    bsengine_core::shadow_config::DEFAULT_SHADOW_DISTANCE,
                    bsengine_core::shadow_config::DEFAULT_CASCADES,
                    0.0,
                ),
                None,
                &self.draw_calls,
                &[],
                &[],
                0,
                &self.registry,
                LightData::default(),
                None,
                &std::collections::HashMap::new(),
                &ui_state,
                &std::collections::HashMap::new(),
                0.0,
                0.0,
                false,
                false,
                proj,
                None,
                None,
                None,
                None,
                None,
                &[],
                false,
                false,
                false,
                None,
                None,
                0.0,
                &[],
                None,
                (0.0, 0.0),
                0,
                view_proj,
                volume,
                &[],
                None,
                None,
                None,
                None,
                None,
                None,
                &[],
            )
            .expect("the frame renders");
        self.surface.read_pixels().expect("offscreen readback")
    }
}

/// The pixel a world point lands on.
fn pixel_of(p: Vec3) -> (u32, u32) {
    (
        ((p.x + HALF_W) / PIXEL) as u32,
        ((HALF_H - p.y) / PIXEL) as u32,
    )
}

fn rgb(pixels: &[u8], p: Vec3) -> [i32; 3] {
    let (x, y) = pixel_of(p);
    let i = ((y * W + x) * 4) as usize;
    [pixels[i] as i32, pixels[i + 1] as i32, pixels[i + 2] as i32]
}

fn reddish(c: [i32; 3]) -> bool {
    c[0] > c[2] + 60
}

fn bluish(c: [i32; 3]) -> bool {
    c[2] > c[0] + 60
}

/// Two top-level bricks side by side, x -1.8..1.8, y and z -0.9..0.9; probes
/// 0.6 apart where nothing cut them, 0.2 inside whatever `triangles` cross.
fn layout(triangles: &[[Vec3; 3]]) -> BrickLayout {
    BrickLayout::build(
        &BrickSettings {
            origin: Vec3::new(-1.8, -0.9, -0.9),
            extent: Vec3::new(3.6, 1.8, 1.8),
            min_spacing: 0.2,
            max_spacing: 0.6,
            max_probes: bsengine_core::MAX_ADAPTIVE_PROBES,
            max_bricks: bsengine_core::MAX_PROBE_BRICKS,
        },
        triangles,
    )
}

/// `layout` baked with each probe lit evenly in `colour(probe)`: a constant
/// SH whose irradiance over PI -- what the sphere shows -- is that colour.
fn baked(
    layout: BrickLayout,
    colour: impl Fn(&bsengine_bake::probe_bricks::Probe) -> Vec3,
) -> BakedProbes {
    let probes = layout
        .probes
        .iter()
        .map(|p| {
            let mut sh = [Vec3::ZERO; 9];
            // E / PI = c0 * 0.282095 for a constant SH (band 0's basis times
            // its cosine-lobe factor PI, over PI).
            sh[0] = colour(p) / 0.282_095;
            BakedProbe {
                sh,
                moments: vec![0; MOMENT_TEXELS],
            }
        })
        .collect();
    BakedProbes { layout, probes }
}

/// The probes on the volume's front face, which nothing is in front of.
fn front(layout: &BrickLayout) -> Vec<bsengine_bake::probe_bricks::Probe> {
    layout
        .probes
        .iter()
        .filter(|p| (p.position.z - 0.9).abs() < 1e-4)
        .copied()
        .collect()
}

/// Every baked probe is drawn where it is, in its own SH's colour: a
/// checkerboard of red and blue probes comes out as that checkerboard,
/// probe by probe -- so the spheres are placed by the probe table and shaded
/// from each probe's own coefficients, not a neighbour's or the first's.
/// Off, the frame is the empty scene's again; and it starts off.
#[test]
fn each_baked_probe_is_drawn_where_it_is_in_its_own_colour() {
    let layout = layout(&[]);
    let front_probes = front(&layout);
    assert_eq!(
        front_probes.len(),
        7 * 4,
        "premise: an uncut layout, 7 x 4 probes on the front face"
    );
    let parity = |p: &bsengine_bake::probe_bricks::Probe| {
        let ix = ((p.position.x + 1.8) / 0.6).round() as i32;
        let iy = ((p.position.y + 0.9) / 0.6).round() as i32;
        (ix + iy) % 2 == 0
    };
    let mut s = Scene::new();
    s.surface
        .set_baked_probe_bricks(Some(Arc::new(baked(layout, |p| {
            if parity(p) {
                RED
            } else {
                BLUE
            }
        }))));
    let blank = s.render();
    for p in &front_probes {
        let c = rgb(&blank, p.position);
        assert!(
            !reddish(c) && !bluish(c),
            "premise: nothing is drawn at {} before the display is on: {c:?}",
            p.position
        );
    }

    s.surface.set_show_light_probes(true);
    let shown = s.render();
    for p in &front_probes {
        let c = rgb(&shown, p.position);
        let (want, name) = if parity(p) {
            (reddish(c), "red")
        } else {
            (bluish(c), "blue")
        };
        assert!(want, "the probe at {} is drawn {name}: {c:?}", p.position);
    }

    s.surface.set_show_light_probes(false);
    assert!(s.render() == blank, "turned off, the spheres are gone");
}

/// A sphere is sized by its probe's spacing, so an adaptive volume's levels
/// read as sizes: a probe of the uncut brick (0.6 apart) covers a point
/// 0.06 from its centre; one of the cut brick (0.2 apart) does not -- it is
/// a third the size -- though its centre is drawn.
#[test]
fn finer_bricks_draw_smaller_spheres() {
    // A small triangle in the right-hand brick, which is cut a level finer.
    let layout = layout(&[[
        Vec3::new(0.85, -0.05, 0.0),
        Vec3::new(0.95, -0.05, 0.0),
        Vec3::new(0.9, 0.05, 0.0),
    ]]);
    let coarse = |p: &bsengine_bake::probe_bricks::Probe| p.spacing.x > 0.5;
    let front_probes = front(&layout);
    let near = |probes: &[bsengine_bake::probe_bricks::Probe], fine: bool, at: Vec3| {
        *probes
            .iter()
            .filter(|p| coarse(p) != fine)
            .min_by(|a, b| {
                (a.position - at)
                    .length()
                    .total_cmp(&(b.position - at).length())
            })
            .expect("a probe of that level")
    };
    let big = near(&front_probes, false, Vec3::new(-1.2, 0.0, 0.9));
    let small = near(&front_probes, true, Vec3::new(0.9, 0.1, 0.9));
    assert!(
        (big.spacing.x - 0.6).abs() < 1e-4 && (small.spacing.x - 0.2).abs() < 1e-4,
        "premise: two levels, 0.6 and 0.2 apart: {} and {}",
        big.spacing,
        small.spacing
    );

    let mut s = Scene::new();
    s.surface
        .set_baked_probe_bricks(Some(Arc::new(baked(layout, |p| {
            if coarse(p) {
                RED
            } else {
                BLUE
            }
        }))));
    s.surface.set_show_light_probes(true);
    let shown = s.render();
    let off_centre = Vec3::new(0.06, 0.0, 0.0);
    assert!(
        bluish(rgb(&shown, small.position)),
        "premise: the fine probe's sphere is drawn: {:?}",
        rgb(&shown, small.position)
    );
    assert!(
        reddish(rgb(&shown, big.position + off_centre)),
        "the coarse probe's sphere reaches 0.06 from its centre: {:?}",
        rgb(&shown, big.position + off_centre)
    );
    assert!(
        !bluish(rgb(&shown, small.position + off_centre)),
        "the fine probe's sphere, a third the size, does not: {:?}",
        rgb(&shown, small.position + off_centre)
    );
}

/// A grid volume baked by the surface itself shows too, on its lattice
/// points -- the display reads a grid's probes by index, x fastest, where a
/// baked brick's come from its table -- sized by the lattice spacing: drawn
/// at every front lattice point and a little off it, and not halfway between
/// two, nor further off than a sphere reaches.
#[test]
fn a_grid_volumes_probes_are_drawn_on_its_lattice_points() {
    // 3 x 2 x 2 probes 1.5 apart in x and y: spheres 0.225 across.
    let volume = bsengine_rhi_wgpu::ProbeVolumeParams {
        origin: Vec3::new(-1.5, -0.75, -0.9),
        extent: Vec3::new(3.0, 1.5, 1.8),
        resolution: [3, 2, 2],
    };
    let mut s = Scene::new();
    // A red wall behind the volume. Without it the bake sees only the empty
    // background and every probe comes out its exact grey -- spheres drawn,
    // and indistinguishable from what is behind them. With it the background
    // is red, and a sphere's camera-facing side, lit by the grey in front of
    // it, is not.
    s.push_quad(
        [
            Vec3::new(-3.0, -2.0, -1.5),
            Vec3::new(3.0, -2.0, -1.5),
            Vec3::new(3.0, 2.0, -1.5),
            Vec3::new(-3.0, 2.0, -1.5),
        ],
        Vec3::new(1.0, 0.0, 0.0),
    );
    let blank = s.render_with(Some(volume));
    s.surface.set_show_light_probes(true);
    let shown = s.render_with(Some(volume));
    let drawn = |p: Vec3| rgb(&shown, p) != rgb(&blank, p);
    for x in [-1.5, 0.0, 1.5] {
        for y in [-0.75, 0.75] {
            let lattice = Vec3::new(x, y, 0.9);
            assert!(drawn(lattice), "a sphere on the lattice point {lattice}");
            assert!(
                drawn(lattice + Vec3::new(0.15, 0.0, 0.0)),
                "reaching 0.15 from {lattice}"
            );
            assert!(
                !drawn(lattice + Vec3::new(0.35, 0.0, 0.0)),
                "but not 0.35 from {lattice}"
            );
        }
    }
    for between in [Vec3::new(-0.75, 0.0, 0.9), Vec3::new(0.75, 0.0, 0.9)] {
        assert!(
            !drawn(between),
            "nothing between lattice points, at {between}"
        );
    }
}

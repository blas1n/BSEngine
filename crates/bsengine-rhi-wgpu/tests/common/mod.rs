//! Stands an offscreen renderer up, draws one frame, and answers questions
//! about the pixels.
//!
//! `render_frame` takes twenty-six arguments. Spelling them all out in every
//! test would bury what each test is actually about, so `Scene` carries
//! defaults and a test fills in only what it cares about.
//!
//! The scene description here is deliberately the harness's own rather than the
//! engine's `LightData`/`MaterialParams`. Those are plain data with no `Clone`,
//! because the real caller builds them fresh each frame from ECS state. Rather
//! than derive `Clone` on engine types for a test's convenience, the harness
//! keeps its own description and converts at the call.

#![allow(dead_code)] // each test binary uses a different part of this

use std::collections::HashMap;

use bsengine_rhi_wgpu::surface::{LightData, MaterialParams, PointLightEntry, WgpuSurface};
use bsengine_rhi_wgpu::{cube_vertices, plane_vertices, GpuMeshRegistry, GpuTextureRegistry};
use glam::{Mat4, Quat, Vec3};

/// Width is not a multiple of 64, so `read_pixels` must go through its row
/// padding path: `200 * 4 = 800`, and `800 % 256 = 32`.
pub const WIDTH: u32 = 200;
pub const HEIGHT: u32 = 150;

/// One object in a frame.
pub struct Draw {
    pub mesh: u64,
    pub transform: Mat4,
    pub texture: Option<u64>,
    pub base_color: Vec3,
    pub emissive: Vec3,
    pub metallic: f32,
    pub roughness: f32,
    pub opacity: f32,
    pub custom_shader: Option<String>,
}

impl Draw {
    pub fn new(mesh: u64, position: Vec3) -> Self {
        Self {
            mesh,
            transform: Mat4::from_translation(position),
            texture: None,
            base_color: Vec3::ONE,
            emissive: Vec3::ZERO,
            metallic: 0.0,
            roughness: 0.5,
            opacity: 1.0,
            custom_shader: None,
        }
    }

    pub fn colour(mut self, rgb: Vec3) -> Self {
        self.base_color = rgb;
        self
    }

    pub fn emissive(mut self, rgb: Vec3) -> Self {
        self.emissive = rgb;
        self
    }

    pub fn scaled(mut self, scale: Vec3, position: Vec3) -> Self {
        self.transform = Mat4::from_scale_rotation_translation(scale, Quat::IDENTITY, position);
        self
    }

    pub fn textured(mut self, id: u64) -> Self {
        self.texture = Some(id);
        self
    }

    /// Below 1.0 sends the draw to the transparent pass.
    pub fn opacity(mut self, a: f32) -> Self {
        self.opacity = a;
        self
    }

    pub fn shader(mut self, path: &str) -> Self {
        self.custom_shader = Some(path.to_string());
        self
    }

    fn material(&self) -> MaterialParams {
        MaterialParams {
            metallic: self.metallic,
            roughness: self.roughness,
            emissive: self.emissive,
            base_color: self.base_color,
            opacity: self.opacity,
        }
    }
}

/// A point light, in the harness's own terms so a `Scene` can be built twice.
#[derive(Clone)]
pub struct PointLight {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
}

/// Lighting for a frame.
#[derive(Clone)]
pub struct Light {
    pub direction: Vec3,
    pub color: Vec3,
    pub ambient: Vec3,
    pub points: Vec<PointLight>,
}

impl Default for Light {
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.4, -0.8, -0.4).normalize(),
            color: Vec3::ONE,
            ambient: Vec3::splat(0.15),
            points: Vec::new(),
        }
    }
}

impl Light {
    fn to_engine(&self) -> LightData {
        LightData {
            direction: self.direction,
            color: self.color,
            ambient: self.ambient,
            point_lights: self
                .points
                .iter()
                .map(|p| PointLightEntry {
                    position: p.position,
                    color: p.color,
                    intensity: p.intensity,
                    range: p.range,
                })
                .collect(),
            spot_lights: Vec::new(),
        }
    }
}

/// Everything that goes into one frame. The default is "nothing in front of
/// the camera".
pub struct Scene {
    pub draws: Vec<Draw>,
    pub light: Light,
    pub camera_pos: Vec3,
    pub look_at: Vec3,
    pub bloom: Option<bsengine_core::Bloom>,
    pub tone_map: Option<bsengine_core::ToneMap>,
    pub ssao: Option<bsengine_core::AmbientOcclusion>,
    pub hud: HashMap<String, String>,
    pub with_skybox: bool,
    /// Particle batches for the pass that runs after transparency.
    pub particles: Vec<bsengine_rhi_wgpu::particles::ParticleBatch>,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            draws: Vec::new(),
            light: Light::default(),
            camera_pos: Vec3::new(0.0, 0.0, 5.0),
            look_at: Vec3::ZERO,
            bloom: None,
            tone_map: None,
            ssao: None,
            hud: HashMap::new(),
            with_skybox: false,
            particles: Vec::new(),
        }
    }
}

/// One frame, read back.
pub struct Pixels {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl Pixels {
    pub fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }

    pub fn centre(&self) -> [u8; 4] {
        self.at(self.width / 2, self.height / 2)
    }

    /// Perceptual brightness. Used for "brighter than" comparisons, never for
    /// asserting an exact colour.
    pub fn luma(&self, x: u32, y: u32) -> f32 {
        let p = self.at(x, y);
        0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32
    }

    pub fn centre_luma(&self) -> f32 {
        self.luma(self.width / 2, self.height / 2)
    }

    /// Whether any pixel differs from another frame's.
    pub fn differs_from(&self, other: &Pixels) -> bool {
        self.data != other.data
    }

    /// Whether every pixel is this colour, within `tolerance` per channel.
    /// Alpha is ignored.
    pub fn is_uniformly(&self, rgb: [u8; 3], tolerance: u8) -> bool {
        self.data.chunks_exact(4).all(|p| {
            p[0].abs_diff(rgb[0]) <= tolerance
                && p[1].abs_diff(rgb[1]) <= tolerance
                && p[2].abs_diff(rgb[2]) <= tolerance
        })
    }

    /// The colour most pixels have. Handy when a test needs to report what it
    /// actually saw.
    pub fn describe(&self) -> String {
        format!(
            "centre {:?}, corner {:?}, edge {:?}",
            self.centre(),
            self.at(0, 0),
            self.at(self.width - 1, self.height / 2)
        )
    }
}

/// An offscreen renderer and the meshes registered on it.
pub struct Harness {
    surface: WgpuSurface,
    registry: GpuMeshRegistry,
    textures: GpuTextureRegistry,
}

impl Harness {
    /// Panics when no adapter can be had.
    ///
    /// Skipping instead is the tempting move and the wrong one: a skipped GPU
    /// test reads exactly like a passing one, and a suite that goes quiet on
    /// the machines that matter is not a suite.
    pub fn new() -> Self {
        Self::build(false)
    }

    /// Same as [`Self::new`], but with `fast_render` set — see
    /// `WgpuSurface::is_fast_render`.
    pub fn new_fast() -> Self {
        Self::build(true)
    }

    fn build(fast_render: bool) -> Self {
        let surface = pollster::block_on(WgpuSurface::new_offscreen(WIDTH, HEIGHT, fast_render))
            .unwrap_or_else(|e| {
                panic!(
                    "could not create an offscreen renderer: {e}\n\
                     These tests need an adapter that can actually rasterise. On Linux CI \
                     that is mesa-vulkan-drivers (lavapipe); on Windows it is normally the \
                     D3D12 WARP adapter. If this environment has neither, that is the \
                     finding worth reporting -- do not silence it by skipping."
                )
            });
        let registry = GpuMeshRegistry::new(surface.device_arc());
        let textures = GpuTextureRegistry::new(surface.device_arc(), surface.queue_arc());
        Self {
            surface,
            registry,
            textures,
        }
    }

    pub fn cube(&mut self) -> u64 {
        let (v, i) = cube_vertices();
        self.registry.register(&v, &i)
    }

    pub fn plane(&mut self) -> u64 {
        let (v, i) = plane_vertices();
        self.registry.register(&v, &i)
    }

    /// A 2x1 texture, `left` in one texel and `right` in the other.
    ///
    /// Two colours rather than one on purpose: a flat texture reads the same
    /// whether or not the UVs are right, so it cannot tell a working sampler
    /// from a broken one.
    pub fn two_colour_texture(&mut self, left: [u8; 4], right: [u8; 4]) -> u64 {
        let mut rgba = Vec::with_capacity(8);
        rgba.extend_from_slice(&left);
        rgba.extend_from_slice(&right);
        self.textures.load_from_rgba(2, 1, &rgba)
    }

    /// Compiles a custom shader that ignores lighting and returns one colour,
    /// and hands back the path key a `Draw` refers to it by.
    ///
    /// The uniform declarations have to mirror the standard mesh shader's
    /// exactly: custom shaders are compiled against the same pipeline layout,
    /// so a mismatch in field order or padding fails pipeline creation.
    pub fn constant_colour_shader(&mut self, rgb: [f32; 3], key: &str) -> String {
        let [r, g, b] = rgb;
        let wgsl = format!(
            r#"
struct CameraUniform {{
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    cam_pos: vec3<f32>,
    time: f32,
}};
struct ModelUniform {{
    model: mat4x4<f32>,
    metallic: f32,
    roughness: f32,
    _pad0: f32,
    _pad1: f32,
    emissive: vec3<f32>,
    _pad2: f32,
    base_color: vec3<f32>,
    _pad3: f32,
}};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> model_data: ModelUniform;

struct VertIn {{
    @location(0) pos: vec3<f32>,
    @location(1) col: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
}};
struct VertOut {{
    @builtin(position) clip: vec4<f32>,
}};

@vertex fn vs_main(in: VertIn) -> VertOut {{
    var out: VertOut;
    out.clip = camera.view_proj * model_data.model * vec4<f32>(in.pos, 1.0);
    return out;
}}
@fragment fn fs_main(in: VertOut) -> @location(0) vec4<f32> {{
    return vec4<f32>({r:?}, {g:?}, {b:?}, 1.0);
}}
"#
        );
        self.surface
            .compile_and_store_shader(key, &wgsl)
            .expect("the test shader should compile");
        key.to_string()
    }

    /// Puts a single flat colour in the skybox.
    pub fn set_test_skybox(&mut self, rgba: [u8; 4]) {
        self.surface.set_skybox_from_rgba(1, 1, &rgba);
    }

    /// Draws one frame and reads it back.
    pub fn render(&mut self, scene: &Scene) -> Pixels {
        let aspect = WIDTH as f32 / HEIGHT as f32;
        let proj = Mat4::perspective_rh(60.0_f32.to_radians(), aspect, 0.1, 100.0);
        let view = Mat4::look_at_rh(scene.camera_pos, scene.look_at, Vec3::Y);
        let view_proj = proj * view;

        let draw_calls: Vec<(u64, Mat4, Option<u64>, MaterialParams, Option<String>)> = scene
            .draws
            .iter()
            .map(|d| {
                (
                    d.mesh,
                    d.transform,
                    d.texture,
                    d.material(),
                    d.custom_shader.clone(),
                )
            })
            .collect();

        let ui_state = bsengine_core::UiState::default();
        let sky_vp_inv = if scene.with_skybox {
            Some(view_proj.inverse())
        } else {
            None
        };

        self.surface
            .render_frame(
                view_proj,
                scene.camera_pos,
                light_view_proj(scene.light.direction),
                sky_vp_inv,
                &draw_calls,
                &[],
                &self.registry,
                scene.light.to_engine(),
                Some(&self.textures),
                &scene.hud,
                &ui_state,
                0.0,
                0.0,
                false,
                false,
                proj,
                scene.bloom,
                scene.tone_map,
                scene.ssao,
                None,
                &[],
                false,
                false,
                false,
                None,
                None,
                0.0,
                &scene.particles,
            )
            .expect("render_frame failed");

        Pixels {
            data: self.surface.read_pixels().expect("read_pixels failed"),
            width: WIDTH,
            height: HEIGHT,
        }
    }

    /// The most recently rendered frame's profiler stats. Panics if
    /// `render()` hasn't been called yet -- every test using this calls
    /// `render()` first, so a `None` here would be a real bug, not an
    /// expected state to handle quietly.
    pub fn frame_stats(&self) -> bsengine_rhi_wgpu::profiler::FrameStats {
        self.surface
            .latest_frame_stats()
            .expect("render() should have populated frame stats")
    }

    /// Shared handle to the rolling frame-stats history.
    pub fn frame_stats_history(
        &self,
    ) -> std::sync::Arc<
        std::sync::Mutex<std::collections::VecDeque<bsengine_rhi_wgpu::profiler::FrameStats>>,
    > {
        self.surface.frame_stats_history()
    }
}

/// The directional shadow map's view-projection.
///
/// This is the same calculation as `compute_light_view_proj` in
/// `bsengine-render`. That function is private, and `bsengine-rhi-wgpu` does
/// not depend on `bsengine-render` -- the dependency runs the other way -- so
/// it is repeated here.
///
/// Worth being clear about what that costs: the shadow tests prove that the
/// shadow pipeline darkens what *this* matrix says is occluded. They do not
/// prove the runtime picks a good matrix.
pub fn light_view_proj(light_dir: Vec3) -> Mat4 {
    let dir = light_dir.normalize();
    let up = if dir.y.abs() < 0.999 {
        Vec3::Y
    } else {
        Vec3::Z
    };
    let eye = -dir * 50.0;
    let view = Mat4::look_at_rh(eye, Vec3::ZERO, up);
    let proj = Mat4::orthographic_rh(-30.0, 30.0, -30.0, 30.0, 0.1, 200.0);
    proj * view
}

//! Spot light cookies: a texture projected through the cone, so a light can
//! throw a window frame, foliage or a coloured gel instead of a plain disc.
//!
//! Unity calls it a cookie (`Light.cookie`), Godot a projector
//! (`Light3D.light_projector`); both take an ordinary colour texture and
//! stretch it over the cone's square, and Godot keeps every projector in one
//! atlas the shader indexes. This follows both: [`CookieAtlas`] is a
//! [`MAX_COOKIES`]-layer array, each layer [`COOKIE_SIZE`] square, and every
//! frame each light's texture is drawn into a layer. Unreal's light
//! *functions* are materials rather than textures, which is more than a
//! cookie asks for.
//!
//! Why copy at all instead of binding each light's texture: WGSL cannot index
//! a variable number of separately bound textures without binding arrays,
//! which WebGPU does not have, and eight more texture bindings would put the
//! mesh shader at 19 of the 16 sampled textures `downlevel_defaults` allows.
//! One array is one binding.
//!
//! Why every frame instead of when the texture changes: a video frame
//! (`TextureCache::set_frame`) rewrites its texture in place without a new
//! generation, so a cache keyed on the registry's generation would freeze a
//! video cookie on its first picture. A layer is 256x256 -- eight of them are
//! less than one shadow-map clear.

use glam::Vec3;

/// How many distinct cookie textures a frame can use: one per spot light the
/// renderer shades, so no light ever loses its cookie to another.
pub const MAX_COOKIES: usize = 8;

/// The side of one atlas layer, in texels. A cookie bigger than this is
/// downsampled into it (through its mips); smaller is stretched.
pub const COOKIE_SIZE: u32 = 256;

/// The format a layer is stored in: sRGB, so the dark end of a cookie -- most
/// of a window frame's shadow -- keeps its precision through the copy.
const ATLAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The basis a spot light's cookie is projected along, before its roll.
///
/// The GPU light entry has three spare floats, not the six a full `right`
/// axis plus a layer would take, so the CPU sends only the light's roll
/// about its own direction, measured from this basis, and the shader builds
/// the same basis from the direction alone. Both sides must therefore agree
/// on it exactly: `cookie_wgsl!`'s `cookie_basis` is this function, line for
/// line, and `the_wgsl_basis_matches_this_one` holds them together.
///
/// The reference up is world Y, except for a light pointing (almost) straight
/// up or down, where Y is (almost) parallel to the direction and the cross
/// product would be noise; Z stands in there.
pub fn cookie_basis(direction: Vec3) -> (Vec3, Vec3) {
    let up_ref = if direction.y.abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let right = direction.cross(up_ref).normalize();
    let up = right.cross(direction);
    (right, up)
}

/// The roll that turns [`cookie_basis`]'s right axis into the light's own
/// (`right`, its local +X in world space): the angle the shader turns the
/// basis by. `right` need not be exactly perpendicular to `direction`.
pub fn cookie_roll(direction: Vec3, right: Vec3) -> f32 {
    let (r0, u0) = cookie_basis(direction);
    right.dot(u0).atan2(right.dot(r0))
}

/// The cookies' declarations and the projection, appended to every shader
/// that lights with spot lights (a macro so `concat!` can join it to their
/// literals). `spot_cookie` returns what the light's colour is multiplied by
/// at a point: white without a cookie.
///
/// The image is projected as a slide projector would: seen from behind the
/// light looking along it, the texture's top is the light's local +Y and its
/// left the local -X, so a picture reads the right way round on the surface.
macro_rules! cookie_wgsl {
    () => {
        r#"
@group(2) @binding(18) var spot_cookies: texture_2d_array<f32>;
@group(2) @binding(19) var spot_cookie_sampler: sampler;

// `cookie::cookie_basis`, line for line: the CPU measured the light's roll
// from this basis, so any difference here turns every cookie.
fn cookie_basis(d: vec3<f32>) -> mat2x3<f32> {
    var up_ref = vec3<f32>(0.0, 1.0, 0.0);
    if abs(d.y) > 0.99 {
        up_ref = vec3<f32>(0.0, 0.0, 1.0);
    }
    let right = normalize(cross(d, up_ref));
    let up = cross(right, d);
    return mat2x3<f32>(right, up);
}

fn spot_cookie(sl: SpotLightEntry, world_pos: vec3<f32>) -> vec3<f32> {
    if sl.cookie_layer < 0.0 {
        return vec3<f32>(1.0);
    }
    let basis = cookie_basis(sl.direction);
    let c = cos(sl.cookie_roll);
    let s = sin(sl.cookie_roll);
    let right = basis[0] * c + basis[1] * s;
    let up = cross(right, sl.direction);
    let rel = world_pos - sl.position;
    let depth = dot(rel, sl.direction);
    if depth <= 0.0 {
        return vec3<f32>(0.0);
    }
    // The cone's edge lands on the square's edge: tan(outer) at unit depth.
    let tan_outer = sqrt(max(1.0 - sl.outer_cos * sl.outer_cos, 0.0)) / max(sl.outer_cos, 1e-4);
    let p = vec2<f32>(dot(rel, right), dot(rel, up)) / (depth * tan_outer);
    let uv = vec2<f32>(0.5 + 0.5 * p.x, 0.5 - 0.5 * p.y);
    // An explicit level: this runs inside the light loop, under branches no
    // derivative is defined across, and every layer has only the one level.
    return textureSampleLevel(spot_cookies, spot_cookie_sampler, uv, i32(sl.cookie_layer), 0.0).rgb;
}
"#
    };
}
pub(crate) use cookie_wgsl;

/// Draws one texture into one layer: a fullscreen triangle sampling it.
const BLIT_WGSL: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> Out {
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: Out;
    out.pos = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    // Implicit level: a cookie larger than the layer is read through its
    // mips rather than point-sampled into shimmering.
    return textureSample(src, src_sampler, in.uv);
}
"#;

/// The array every spot light's cookie is drawn into, and the pass that
/// draws them.
pub(crate) struct CookieAtlas {
    /// Kept alive for the views; never read directly.
    _texture: wgpu::Texture,
    /// All layers, as the light bind group's binding 18 sees them.
    pub(crate) array_view: wgpu::TextureView,
    /// One render-target view per layer, for the blit.
    layer_views: Vec<wgpu::TextureView>,
    /// Binding 19: bilinear, clamped (outside the square is outside the cone
    /// anyway, and a repeat would bleed the opposite edge into the rim).
    pub(crate) sampler: wgpu::Sampler,
    blit_pipeline: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    /// Trilinear, so a large cookie's mips are used on the way down.
    blit_sampler: wgpu::Sampler,
}

impl CookieAtlas {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("spot cookie atlas"),
            size: wgpu::Extent3d {
                width: COOKIE_SIZE,
                height: COOKIE_SIZE,
                depth_or_array_layers: MAX_COOKIES as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ATLAS_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let array_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("spot cookie atlas view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let layer_views = (0..MAX_COOKIES as u32)
            .map(|layer| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("spot cookie layer"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let clamped = |label, mipmap_filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter,
                ..Default::default()
            })
        };
        let sampler = clamped("spot cookie sampler", wgpu::FilterMode::Nearest);
        let blit_sampler = clamped("spot cookie blit sampler", wgpu::FilterMode::Linear);

        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("spot cookie blit bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("spot cookie blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("spot cookie blit layout"),
            bind_group_layouts: &[&blit_layout],
            push_constant_ranges: &[],
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("spot cookie blit"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "vs",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: "fs",
                targets: &[Some(wgpu::ColorTargetState {
                    format: ATLAS_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self {
            _texture: texture,
            array_view,
            layer_views,
            sampler,
            blit_pipeline,
            blit_layout,
            blit_sampler,
        }
    }

    /// Draws this frame's cookies into the atlas and returns each light's
    /// layer, `-1.0` for none -- the value the shader reads as "no cookie".
    ///
    /// Lights sharing a texture share its layer. A cookie whose texture is
    /// not on the GPU (still loading, or a bad path) is no cookie: the light
    /// shines plain rather than going dark while it loads, as Unity's does.
    /// Submitted on its own, before the frame's passes and the probe bakes
    /// that run ahead of them, so both see this frame's cookies.
    pub(crate) fn update(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        registry: Option<&crate::texture::GpuTextureRegistry>,
        cookies: &[Option<u64>],
    ) -> Vec<f32> {
        let mut layers = vec![-1.0; cookies.len()];
        let Some(registry) = registry else {
            return layers;
        };
        let mut assigned: Vec<u64> = Vec::new();
        let mut encoder = None;
        for (slot, id) in cookies.iter().enumerate() {
            let Some(id) = *id else { continue };
            let Some(view) = registry.get_view(id) else {
                continue;
            };
            if let Some(layer) = assigned.iter().position(|a| *a == id) {
                layers[slot] = layer as f32;
                continue;
            }
            if assigned.len() == MAX_COOKIES {
                continue;
            }
            let layer = assigned.len();
            assigned.push(id);
            layers[slot] = layer as f32;
            let encoder = encoder.get_or_insert_with(|| {
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("spot cookie blit"),
                })
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("spot cookie blit bg"),
                layout: &self.blit_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
                    },
                ],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("spot cookie blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.layer_views[layer],
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        if let Some(encoder) = encoder {
            queue.submit(Some(encoder.finish()));
        }
        layers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A light whose right axis *is* the basis's has no roll, and one turned
    /// a quarter about its direction has a quarter -- for a light pointing
    /// sideways and for one pointing straight down, which takes the other
    /// reference axis.
    #[test]
    fn roll_is_measured_from_the_basis() {
        for direction in [
            Vec3::NEG_Z,
            Vec3::NEG_Y,
            Vec3::new(1.0, -1.0, 0.3).normalize(),
        ] {
            let (r0, u0) = cookie_basis(direction);
            assert!(r0.dot(direction).abs() < 1e-5 && u0.dot(direction).abs() < 1e-5);
            assert!((r0.length() - 1.0).abs() < 1e-5 && (u0.length() - 1.0).abs() < 1e-5);
            assert!(cookie_roll(direction, r0).abs() < 1e-5, "{direction}");
            let quarter = cookie_roll(direction, u0);
            assert!(
                (quarter - std::f32::consts::FRAC_PI_2).abs() < 1e-5,
                "{direction}: {quarter}"
            );
        }
    }

    /// The light's own up (local +Y), rebuilt from its direction and roll as
    /// the shader rebuilds it, is the up the transform says -- the claim the
    /// whole three-float encoding rests on. A camera-like light at an
    /// arbitrary orientation, so neither axis is a world axis.
    #[test]
    fn the_roll_rebuilds_the_lights_own_axes() {
        let rotation = glam::Quat::from_euler(glam::EulerRot::YXZ, 0.7, -0.9, 0.4);
        let direction = rotation * Vec3::NEG_Z;
        let right = rotation * Vec3::X;
        let up = rotation * Vec3::Y;
        let roll = cookie_roll(direction, right);
        let (r0, u0) = cookie_basis(direction);
        let rebuilt_right = r0 * roll.cos() + u0 * roll.sin();
        let rebuilt_up = rebuilt_right.cross(direction);
        assert!(
            rebuilt_right.distance(right) < 1e-4,
            "{rebuilt_right} vs {right}"
        );
        assert!(rebuilt_up.distance(up) < 1e-4, "{rebuilt_up} vs {up}");
    }

    /// The WGSL basis is the Rust one: same reference axes, same threshold,
    /// same cross-product order. A drift in either turns every cookie by the
    /// angle between the two bases, which a pixel test with one light
    /// pointing straight down would not notice for the threshold.
    #[test]
    fn the_wgsl_basis_matches_this_one() {
        let wgsl = cookie_wgsl!();
        for needle in [
            "if abs(d.y) > 0.99 {",
            "up_ref = vec3<f32>(0.0, 0.0, 1.0);",
            "var up_ref = vec3<f32>(0.0, 1.0, 0.0);",
            "let right = normalize(cross(d, up_ref));",
            "let up = cross(right, d);",
        ] {
            assert!(wgsl.contains(needle), "cookie_wgsl! lost `{needle}`");
        }
        let src = include_str!("cookie.rs");
        for needle in [
            "if direction.y.abs() > 0.99 {",
            "let right = direction.cross(up_ref).normalize();",
            "let up = right.cross(direction);",
        ] {
            assert!(src.contains(needle), "cookie_basis lost `{needle}`");
        }
    }
}

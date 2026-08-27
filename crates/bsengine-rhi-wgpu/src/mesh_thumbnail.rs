//! Minimal glTF parsing + a small dedicated render pipeline for the Asset
//! Browser's mesh thumbnails. Deliberately does not use `bsengine-gltf`'s
//! full loader: `bsengine-gltf` depends on this crate (for `Vertex`), so
//! calling into it from here would be a dependency cycle. This module only
//! extracts what a thumbnail needs -- base color + geometry -- using the
//! raw `gltf` crate directly. See
//! `docs/superpowers/specs/2026-08-27-mesh-3d-thumbnails-design.md`.

use crate::mesh::Vertex;
use std::path::Path;

/// A decoded RGBA8 image, already resolved from whatever format glTF stored
/// it in (matches `bsengine-gltf`'s own `GltfImageData` shape, duplicated
/// rather than imported for the same reason the whole module exists).
pub struct DecodedImage {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Raw pixel data in RGBA8 order, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// One primitive's geometry plus its base color: a decoded texture when the
/// primitive's material has one, or just the flat `base_color_factor`
/// otherwise.
pub struct ThumbnailPrimitive {
    /// Vertex buffer data (position, color, normal, uv).
    pub vertices: Vec<Vertex>,
    /// Index buffer data, referencing into `vertices`.
    pub indices: Vec<u32>,
    /// The material's flat base color, RGBA in 0..=1. Used directly when
    /// `base_color_texture` is `None`, and as a tint on top of it otherwise.
    pub base_color_factor: [f32; 4],
    /// The material's base color texture, decoded to RGBA8, if it has one.
    pub base_color_texture: Option<DecodedImage>,
}

/// Every primitive belonging to a glTF document's first mesh, ready to
/// render as a thumbnail. Only the first `mesh` entry in the document is
/// used -- see the design doc's "Out of scope" section.
pub struct ThumbnailMesh {
    /// This mesh's primitives, each with its own geometry and material.
    pub primitives: Vec<ThumbnailPrimitive>,
}

/// Loads `path` and extracts its first mesh's geometry + base color
/// material for thumbnail rendering. `None` on any failure: the file
/// doesn't parse, or the document has no meshes, or a primitive is missing
/// position data. Skinning, animation, and every non-base-color material
/// slot are ignored -- see the design doc.
pub fn load_thumbnail_mesh(path: &Path) -> Option<ThumbnailMesh> {
    let (doc, buffers, images) = gltf::import(path).ok()?;
    let mesh = doc.meshes().next()?;

    let mut primitives = Vec::new();
    for primitive in mesh.primitives() {
        let reader = primitive.reader(|b| Some(&buffers[b.index()]));
        let positions: Vec<[f32; 3]> = reader.read_positions()?.collect();
        let indices: Vec<u32> = match reader.read_indices() {
            Some(indices) => indices.into_u32().collect(),
            None => (0..positions.len() as u32).collect(),
        };
        let normals: Vec<[f32; 3]> = reader
            .read_normals()
            .map(|n| n.collect())
            .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
        let uvs: Vec<[f32; 2]> = reader
            .read_tex_coords(0)
            .map(|t| t.into_f32().collect())
            .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

        let vertices: Vec<Vertex> = positions
            .into_iter()
            .zip(normals)
            .zip(uvs)
            .map(|((position, normal), uv)| Vertex {
                position,
                color: [1.0, 1.0, 1.0],
                normal,
                uv,
            })
            .collect();

        let pbr = primitive.material().pbr_metallic_roughness();
        let base_color_factor = pbr.base_color_factor();
        let base_color_texture = pbr.base_color_texture().and_then(|info| {
            let image = images.get(info.texture().source().index())?;
            Some(DecodedImage {
                width: image.width,
                height: image.height,
                rgba: gltf_pixels_to_rgba(&image.pixels, image.format, image.width, image.height),
            })
        });

        primitives.push(ThumbnailPrimitive {
            vertices,
            indices,
            base_color_factor,
            base_color_texture,
        });
    }

    if primitives.is_empty() {
        return None;
    }
    Some(ThumbnailMesh { primitives })
}

/// Mirrors `bsengine-gltf`'s private helper of the same shape (`loader.rs`'s
/// `gltf_pixels_to_rgba`) -- duplicated rather than shared, since sharing it
/// would mean depending on `bsengine-gltf`, which is exactly the cycle this
/// module exists to avoid.
fn gltf_pixels_to_rgba(
    pixels: &[u8],
    format: gltf::image::Format,
    width: u32,
    height: u32,
) -> Vec<u8> {
    match format {
        gltf::image::Format::R8G8B8A8 => pixels.to_vec(),
        gltf::image::Format::R8G8B8 => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for chunk in pixels.chunks(3) {
                out.extend_from_slice(chunk);
                out.push(255);
            }
            out
        }
        _ => vec![255u8; (width * height * 4) as usize],
    }
}

/// Square resolution every mesh thumbnail renders at -- matches the texture
/// thumbnail's own 64x64 (`asset_browser.rs`'s `thumbnail_for`).
pub const THUMBNAIL_SIZE: u32 = 64;

const THUMBNAIL_WGSL: &str = r#"
struct Camera {
    view_proj: mat4x4<f32>,
    light_dir: vec3<f32>,
    _pad0: f32,
    light_color: vec3<f32>,
    ambient: f32,
};
@group(0) @binding(0) var<uniform> camera: Camera;

struct MaterialUniform {
    base_color_factor: vec4<f32>,
    has_texture: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};
@group(1) @binding(0) var<uniform> material: MaterialUniform;
@group(1) @binding(1) var base_color_tex: texture_2d<f32>;
@group(1) @binding(2) var base_color_sampler: sampler;

struct VertIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
};
struct VertOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertIn) -> VertOut {
    var out: VertOut;
    out.clip_pos = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    var base = material.base_color_factor;
    if (material.has_texture != 0u) {
        base = base * textureSample(base_color_tex, base_color_sampler, in.uv);
    }
    let n = normalize(in.normal);
    let ndotl = max(dot(n, -camera.light_dir), 0.0);
    let lit = camera.light_color * ndotl + vec3<f32>(camera.ambient, camera.ambient, camera.ambient);
    return vec4<f32>(base.rgb * lit, base.a);
}
"#;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ThumbnailCameraUniform {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 3],
    _pad0: f32,
    light_color: [f32; 3],
    ambient: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ThumbnailMaterialUniform {
    base_color_factor: [f32; 4],
    has_texture: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

/// Renders `mesh` to a `THUMBNAIL_SIZE`x`THUMBNAIL_SIZE` image: one fixed
/// directional light, flat Lambertian shading, no shadows, no
/// post-processing. Deliberately not a reuse of `WgpuSurface::render_frame`
/// -- see the design doc for why a dedicated pipeline is simpler here.
pub fn render_thumbnail(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mesh: &ThumbnailMesh,
) -> image::RgbaImage {
    use glam::{Mat4, Vec3};
    use wgpu::util::DeviceExt;

    let all_vertices: Vec<Vertex> = mesh
        .primitives
        .iter()
        .flat_map(|p| p.vertices.iter().copied())
        .collect();
    let (center, radius) = crate::mesh::compute_bounding_sphere(&all_vertices);
    let radius = radius.max(0.01);
    let eye = center + Vec3::new(1.0, 0.8, 1.2).normalize() * (radius * 2.5);
    let view = Mat4::look_at_rh(eye, center, Vec3::Y);
    let proj = Mat4::perspective_rh(45.0_f32.to_radians(), 1.0, radius * 0.1, radius * 10.0);
    let view_proj = proj * view;

    let camera_uniform = ThumbnailCameraUniform {
        view_proj: view_proj.to_cols_array_2d(),
        light_dir: Vec3::new(-0.4, -0.8, -0.4).normalize().to_array(),
        _pad0: 0.0,
        light_color: [1.0, 1.0, 1.0],
        ambient: 0.35,
    };
    let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("thumbnail camera"),
        contents: bytemuck::cast_slice(&[camera_uniform]),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("thumbnail camera bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let camera_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("thumbnail camera bg"),
        layout: &camera_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: camera_buffer.as_entire_binding(),
        }],
    });

    let material_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("thumbnail material bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("thumbnail shader"),
        source: wgpu::ShaderSource::Wgsl(THUMBNAIL_WGSL.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("thumbnail pipeline layout"),
        bind_group_layouts: &[&camera_bgl, &material_bgl],
        push_constant_ranges: &[],
    });
    let vertex_buffer_layout = wgpu::VertexBufferLayout {
        array_stride: 44,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 24,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 36,
                shader_location: 3,
            },
        ],
    };
    let color_target_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("thumbnail pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[vertex_buffer_layout],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: color_target_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let color_texture =
        crate::output::create_offscreen_texture(device, THUMBNAIL_SIZE, THUMBNAIL_SIZE);
    let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let depth_texture = crate::profiler::create_tracked_texture(
        device,
        &wgpu::TextureDescriptor {
            label: Some("thumbnail depth"),
            size: wgpu::Extent3d {
                width: THUMBNAIL_SIZE,
                height: THUMBNAIL_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
    );
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let white_texture = crate::profiler::create_tracked_texture_with_data(
        device,
        queue,
        &wgpu::TextureDescriptor {
            label: Some("thumbnail white fallback"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &[255, 255, 255, 255],
    );
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("thumbnail encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("thumbnail pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.1,
                        b: 0.1,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &camera_bg, &[]);

        for primitive in &mesh.primitives {
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("thumbnail vertex buffer"),
                contents: bytemuck::cast_slice(&primitive.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("thumbnail index buffer"),
                contents: bytemuck::cast_slice(&primitive.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let (texture_view, has_texture) = match &primitive.base_color_texture {
                Some(img) => {
                    let texture = crate::profiler::create_tracked_texture_with_data(
                        device,
                        queue,
                        &wgpu::TextureDescriptor {
                            label: Some("thumbnail base color"),
                            size: wgpu::Extent3d {
                                width: img.width,
                                height: img.height,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            usage: wgpu::TextureUsages::TEXTURE_BINDING,
                            view_formats: &[],
                        },
                        wgpu::util::TextureDataOrder::LayerMajor,
                        &img.rgba,
                    );
                    (
                        texture.create_view(&wgpu::TextureViewDescriptor::default()),
                        1u32,
                    )
                }
                None => (
                    white_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    0u32,
                ),
            };

            let material_uniform = ThumbnailMaterialUniform {
                base_color_factor: primitive.base_color_factor,
                has_texture,
                _pad0: 0,
                _pad1: 0,
                _pad2: 0,
            };
            let material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("thumbnail material"),
                contents: bytemuck::cast_slice(&[material_uniform]),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let material_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("thumbnail material bg"),
                layout: &material_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: material_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });

            pass.set_bind_group(1, &material_bg, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..primitive.indices.len() as u32, 0, 0..1);
        }
    }
    queue.submit(std::iter::once(encoder.finish()));
    device.poll(wgpu::Maintain::Wait);

    let pixels = crate::output::read_pixels(
        device,
        queue,
        &color_texture,
        THUMBNAIL_SIZE,
        THUMBNAIL_SIZE,
    );
    image::RgbaImage::from_raw(THUMBNAIL_SIZE, THUMBNAIL_SIZE, pixels)
        .expect("read_pixels always returns exactly width*height*4 bytes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Writes a minimal, valid `.gltf` (JSON, not binary `.glb`) file
    /// containing exactly one mesh with one triangle primitive, using
    /// `material_index` as its `material` reference. The vertex buffer is
    /// built and base64-encoded at test-run time rather than hand-typed, to
    /// avoid an unverifiable hand-computed base64 string.
    fn write_test_gltf_triangle(path: &Path, base_color_factor: [f32; 4]) {
        let positions: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let indices: [u32; 3] = [0, 1, 2];

        let mut buf = Vec::new();
        for p in &positions {
            buf.extend_from_slice(&p[0].to_le_bytes());
            buf.extend_from_slice(&p[1].to_le_bytes());
            buf.extend_from_slice(&p[2].to_le_bytes());
        }
        let position_bytes = buf.len();
        for i in &indices {
            buf.extend_from_slice(&i.to_le_bytes());
        }
        let index_bytes = buf.len() - position_bytes;

        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&buf);
        let [r, g, b, a] = base_color_factor;

        let json = format!(
            r#"{{
  "asset": {{"version": "2.0"}},
  "scenes": [{{"nodes": [0]}}],
  "nodes": [{{"mesh": 0}}],
  "meshes": [{{
    "primitives": [{{
      "attributes": {{"POSITION": 0}},
      "indices": 1,
      "material": 0
    }}]
  }}],
  "materials": [{{
    "pbrMetallicRoughness": {{"baseColorFactor": [{r}, {g}, {b}, {a}]}}
  }}],
  "accessors": [
    {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0]}},
    {{"bufferView": 1, "componentType": 5125, "count": 3, "type": "SCALAR"}}
  ],
  "bufferViews": [
    {{"buffer": 0, "byteOffset": 0, "byteLength": {position_bytes}, "target": 34962}},
    {{"buffer": 0, "byteOffset": {position_bytes}, "byteLength": {index_bytes}, "target": 34963}}
  ],
  "buffers": [
    {{"byteLength": {total}, "uri": "data:application/octet-stream;base64,{encoded}"}}
  ]
}}"#,
            total = position_bytes + index_bytes,
        );
        std::fs::write(path, json).unwrap();
    }

    #[test]
    fn load_thumbnail_mesh_parses_a_minimal_triangle() {
        let tmp = std::env::temp_dir().join("bse_mesh_thumb_test_triangle.gltf");
        write_test_gltf_triangle(&tmp, [1.0, 0.0, 0.0, 1.0]);

        let mesh = load_thumbnail_mesh(&tmp).expect("a valid minimal glTF should parse");
        assert_eq!(mesh.primitives.len(), 1);
        let prim = &mesh.primitives[0];
        assert_eq!(prim.vertices.len(), 3);
        assert_eq!(prim.indices, vec![0, 1, 2]);
        assert_eq!(prim.base_color_factor, [1.0, 0.0, 0.0, 1.0]);
        assert!(
            prim.base_color_texture.is_none(),
            "this fixture has no texture, only a flat factor"
        );

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_thumbnail_mesh_fills_in_default_normals_and_uvs_when_absent() {
        let tmp = std::env::temp_dir().join("bse_mesh_thumb_test_defaults.gltf");
        write_test_gltf_triangle(&tmp, [0.5, 0.5, 0.5, 1.0]);

        let mesh = load_thumbnail_mesh(&tmp).unwrap();
        let prim = &mesh.primitives[0];
        for v in &prim.vertices {
            assert_eq!(
                v.normal,
                [0.0, 1.0, 0.0],
                "missing NORMAL accessor should default to up"
            );
            assert_eq!(
                v.uv,
                [0.0, 0.0],
                "missing TEXCOORD_0 accessor should default to origin"
            );
        }

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_thumbnail_mesh_returns_none_for_a_nonexistent_file() {
        assert!(load_thumbnail_mesh(Path::new("definitely_does_not_exist.gltf")).is_none());
    }

    #[test]
    fn load_thumbnail_mesh_returns_none_for_a_document_with_no_meshes() {
        let tmp = std::env::temp_dir().join("bse_mesh_thumb_test_no_meshes.gltf");
        std::fs::write(&tmp, r#"{"asset": {"version": "2.0"}}"#).unwrap();

        assert!(load_thumbnail_mesh(&tmp).is_none());

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn load_thumbnail_mesh_parses_the_real_fox_fixture() {
        let path = PathBuf::from(format!(
            "{}/../../games/mini-arena/assets/models/fox.glb",
            env!("CARGO_MANIFEST_DIR")
        ));
        let mesh = load_thumbnail_mesh(&path)
            .expect("the real fox.glb fixture used elsewhere in this workspace should parse");
        assert!(
            !mesh.primitives.is_empty(),
            "fox.glb should have at least one primitive"
        );
        assert!(
            mesh.primitives.iter().any(|p| !p.vertices.is_empty()),
            "at least one primitive should have real geometry"
        );
    }

    #[test]
    fn render_thumbnail_produces_a_non_black_image_for_a_lit_triangle() {
        let tmp = std::env::temp_dir().join("bse_mesh_thumb_test_render.gltf");
        write_test_gltf_triangle(&tmp, [1.0, 1.0, 1.0, 1.0]);
        let mesh = load_thumbnail_mesh(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);

        let surface = pollster::block_on(crate::surface::WgpuSurface::new_offscreen(16, 16, false))
            .expect("these tests need an adapter; a skip here would look like a pass");
        let device = surface.device_arc();
        let queue = surface.queue_arc();

        let image = render_thumbnail(&device, &queue, &mesh);

        assert_eq!(image.width(), THUMBNAIL_SIZE);
        assert_eq!(image.height(), THUMBNAIL_SIZE);
        let has_lit_pixel = image
            .pixels()
            .any(|p| p.0[0] > 20 || p.0[1] > 20 || p.0[2] > 20);
        assert!(
            has_lit_pixel,
            "a white-material triangle lit by the fixed directional light should produce at \
             least some visibly non-black pixels"
        );
    }
}

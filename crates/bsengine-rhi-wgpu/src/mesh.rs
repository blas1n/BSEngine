use bsengine_ecs::Resource;
use glam::Vec3;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
/// A single GPU mesh vertex: position, vertex color, normal, and UV, packed
/// for direct upload via `bytemuck`.
pub struct Vertex {
    /// Local-space position.
    pub position: [f32; 3],
    /// Per-vertex RGB tint, multiplied with material color at render time.
    pub color: [f32; 3],
    /// Local-space surface normal.
    pub normal: [f32; 3],
    /// Texture coordinates.
    pub uv: [f32; 2],
}

/// Computes a bounding sphere (center, radius) in local mesh space.
pub fn compute_bounding_sphere(vertices: &[Vertex]) -> (Vec3, f32) {
    if vertices.is_empty() {
        return (Vec3::ZERO, 0.0);
    }
    let center = vertices
        .iter()
        .map(|v| Vec3::from(v.position))
        .fold(Vec3::ZERO, |a, p| a + p)
        / vertices.len() as f32;
    let radius = vertices
        .iter()
        .map(|v| (Vec3::from(v.position) - center).length())
        .fold(0.0_f32, f32::max);
    (center, radius)
}

/// A mesh's GPU-resident buffers plus its precomputed bounding sphere.
pub struct GpuMesh {
    /// GPU buffer holding this mesh's `Vertex` data.
    pub vertex_buffer: wgpu::Buffer,
    /// GPU buffer holding this mesh's triangle indices.
    pub index_buffer: wgpu::Buffer,
    /// Number of indices to draw.
    pub index_count: u32,
    /// Local-space bounding sphere (center, radius).
    pub bounds: (Vec3, f32),
}

/// Owns every GPU mesh uploaded for the running app, keyed by a registry-assigned id.
#[derive(Resource)]
pub struct GpuMeshRegistry {
    device: Arc<wgpu::Device>,
    meshes: HashMap<u64, GpuMesh>,
    next_id: u64,
}

impl GpuMeshRegistry {
    /// Creates an empty registry bound to the given wgpu device.
    pub fn new(device: Arc<wgpu::Device>) -> Self {
        Self {
            device,
            meshes: HashMap::new(),
            next_id: 1,
        }
    }

    /// Uploads vertex/index data as a new GPU mesh and returns its assigned id.
    pub fn register(&mut self, vertices: &[Vertex], indices: &[u32]) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mesh = self.build(vertices, indices);
        self.meshes.insert(id, mesh);
        id
    }

    /// Rebuilds an already-registered mesh's buffers from new data, keeping its
    /// id. Returns whether `id` was registered.
    ///
    /// Hot reload uses this rather than `register` because `MeshRenderer` stores
    /// the id: replacing contents under the same id updates every entity drawing
    /// that mesh at once, including the extra entities a multi-mesh glTF spawns.
    /// `register` would also leak, since the registry never frees.
    ///
    /// Unlike [`GpuMeshRegistry::update_vertices`] this handles a changed vertex
    /// or index count, and recomputes bounds.
    ///
    /// The returned flag is `#[must_use]` because `false` means an id a caller
    /// recorded at load time is no longer registered — the exact invariant
    /// replace-in-place hot reload rests on. Dropping it turns that into a
    /// reload that appears to work and silently keeps the old geometry.
    #[must_use]
    pub fn replace(&mut self, id: u64, vertices: &[Vertex], indices: &[u32]) -> bool {
        if !self.meshes.contains_key(&id) {
            return false;
        }
        let mesh = self.build(vertices, indices);
        self.meshes.insert(id, mesh);
        true
    }

    fn build(&self, vertices: &[Vertex], indices: &[u32]) -> GpuMesh {
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh vbo"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh ibo"),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        GpuMesh {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            bounds: compute_bounding_sphere(vertices),
        }
    }

    /// Looks up a previously registered mesh by id.
    pub fn get(&self, id: u64) -> Option<&GpuMesh> {
        self.meshes.get(&id)
    }

    /// Looks up a previously registered mesh's local-space bounding sphere by id.
    pub fn get_bounds(&self, id: u64) -> Option<(Vec3, f32)> {
        self.meshes.get(&id).map(|m| m.bounds)
    }

    /// Overwrites a previously registered mesh's vertex buffer contents in
    /// place, without resizing it. Used by CPU-side skeletal skinning to
    /// re-upload deformed vertex positions/normals each frame without
    /// re-registering a new mesh id. Returns whether the upload happened.
    ///
    /// `vertices` must be exactly as long as the buffer the mesh *currently*
    /// holds — the count from the most recent `register` or
    /// [`GpuMeshRegistry::replace`], which after a hot reload need not be the
    /// original one. A mismatch is refused with a warning rather than handed to
    /// wgpu: a longer upload is a `BufferOverrun`, and since nothing in this
    /// codebase installs an error scope or `on_uncaptured_error`, that reaches
    /// wgpu's default handler, which panics the process. A shorter one would
    /// leave stale vertices in the tail. Use [`GpuMeshRegistry::replace`] when
    /// the count genuinely changed.
    pub fn update_vertices(&mut self, queue: &wgpu::Queue, id: u64, vertices: &[Vertex]) -> bool {
        let Some(mesh) = self.meshes.get(&id) else {
            return false;
        };
        if vertices.is_empty() {
            // Nothing to upload, and not a caller error worth warning about:
            // `create_buffer_init` never allocates below COPY_BUFFER_ALIGNMENT,
            // so an empty mesh's buffer can never match an empty upload anyway.
            return false;
        }
        let incoming = std::mem::size_of_val(vertices) as wgpu::BufferAddress;
        if incoming != mesh.vertex_buffer.size() {
            warn!(
                "refusing to upload {incoming} bytes of vertex data into mesh {id}'s \
                 {} byte buffer; the mesh was rebuilt at a different vertex count \
                 and whatever holds this data has not caught up",
                mesh.vertex_buffer.size()
            );
            return false;
        }
        queue.write_buffer(&mesh.vertex_buffer, 0, bytemuck::cast_slice(vertices));
        true
    }
}

/// Vertices/indices for a single centered, unit-sized triangle.
pub fn triangle_vertices() -> (Vec<Vertex>, Vec<u32>) {
    let vertices = vec![
        Vertex {
            position: [0.0, 0.5, 0.0],
            color: [1.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.5, 0.0],
        },
        Vertex {
            position: [-0.5, -0.5, 0.0],
            color: [0.0, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 1.0],
        },
        Vertex {
            position: [0.5, -0.5, 0.0],
            color: [0.0, 0.0, 1.0],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 1.0],
        },
    ];
    let indices = vec![0, 1, 2];
    (vertices, indices)
}

/// Vertices/indices for a unit cube, with one distinctly colored face per axis
/// direction and per-face (non-shared) normals/UVs.
pub fn cube_vertices() -> (Vec<Vertex>, Vec<u32>) {
    let v = |pos: [f32; 3], col: [f32; 3], normal: [f32; 3], uv: [f32; 2]| Vertex {
        position: pos,
        color: col,
        normal,
        uv,
    };
    #[rustfmt::skip]
    let vertices = vec![
        // front face (+Z) — red
        v([-0.5, -0.5,  0.5], [1.0, 0.2, 0.2], [ 0.0,  0.0,  1.0], [0.0, 1.0]),
        v([ 0.5, -0.5,  0.5], [1.0, 0.2, 0.2], [ 0.0,  0.0,  1.0], [1.0, 1.0]),
        v([ 0.5,  0.5,  0.5], [1.0, 0.2, 0.2], [ 0.0,  0.0,  1.0], [1.0, 0.0]),
        v([-0.5,  0.5,  0.5], [1.0, 0.2, 0.2], [ 0.0,  0.0,  1.0], [0.0, 0.0]),
        // back face (-Z) — green
        v([ 0.5, -0.5, -0.5], [0.2, 1.0, 0.2], [ 0.0,  0.0, -1.0], [0.0, 1.0]),
        v([-0.5, -0.5, -0.5], [0.2, 1.0, 0.2], [ 0.0,  0.0, -1.0], [1.0, 1.0]),
        v([-0.5,  0.5, -0.5], [0.2, 1.0, 0.2], [ 0.0,  0.0, -1.0], [1.0, 0.0]),
        v([ 0.5,  0.5, -0.5], [0.2, 1.0, 0.2], [ 0.0,  0.0, -1.0], [0.0, 0.0]),
        // top face (+Y) — blue
        v([-0.5,  0.5,  0.5], [0.2, 0.2, 1.0], [ 0.0,  1.0,  0.0], [0.0, 1.0]),
        v([ 0.5,  0.5,  0.5], [0.2, 0.2, 1.0], [ 0.0,  1.0,  0.0], [1.0, 1.0]),
        v([ 0.5,  0.5, -0.5], [0.2, 0.2, 1.0], [ 0.0,  1.0,  0.0], [1.0, 0.0]),
        v([-0.5,  0.5, -0.5], [0.2, 0.2, 1.0], [ 0.0,  1.0,  0.0], [0.0, 0.0]),
        // bottom face (-Y) — yellow
        v([-0.5, -0.5, -0.5], [1.0, 1.0, 0.2], [ 0.0, -1.0,  0.0], [0.0, 1.0]),
        v([ 0.5, -0.5, -0.5], [1.0, 1.0, 0.2], [ 0.0, -1.0,  0.0], [1.0, 1.0]),
        v([ 0.5, -0.5,  0.5], [1.0, 1.0, 0.2], [ 0.0, -1.0,  0.0], [1.0, 0.0]),
        v([-0.5, -0.5,  0.5], [1.0, 1.0, 0.2], [ 0.0, -1.0,  0.0], [0.0, 0.0]),
        // right face (+X) — magenta
        v([ 0.5, -0.5,  0.5], [1.0, 0.2, 1.0], [ 1.0,  0.0,  0.0], [0.0, 1.0]),
        v([ 0.5, -0.5, -0.5], [1.0, 0.2, 1.0], [ 1.0,  0.0,  0.0], [1.0, 1.0]),
        v([ 0.5,  0.5, -0.5], [1.0, 0.2, 1.0], [ 1.0,  0.0,  0.0], [1.0, 0.0]),
        v([ 0.5,  0.5,  0.5], [1.0, 0.2, 1.0], [ 1.0,  0.0,  0.0], [0.0, 0.0]),
        // left face (-X) — cyan
        v([-0.5, -0.5, -0.5], [0.2, 1.0, 1.0], [-1.0,  0.0,  0.0], [0.0, 1.0]),
        v([-0.5, -0.5,  0.5], [0.2, 1.0, 1.0], [-1.0,  0.0,  0.0], [1.0, 1.0]),
        v([-0.5,  0.5,  0.5], [0.2, 1.0, 1.0], [-1.0,  0.0,  0.0], [1.0, 0.0]),
        v([-0.5,  0.5, -0.5], [0.2, 1.0, 1.0], [-1.0,  0.0,  0.0], [0.0, 0.0]),
    ];
    #[rustfmt::skip]
    let indices: Vec<u32> = (0..6u32)
        .flat_map(|face| {
            let b = face * 4;
            [b, b + 1, b + 2, b, b + 2, b + 3]
        })
        .collect();
    (vertices, indices)
}

/// Vertices/indices for a UV sphere of unit diameter, built as a stack/slice grid.
pub fn sphere_vertices() -> (Vec<Vertex>, Vec<u32>) {
    const STACKS: u32 = 16;
    const SLICES: u32 = 32;

    let mut verts = Vec::new();
    let mut idx = Vec::new();

    for i in 0..=STACKS {
        let phi = std::f32::consts::PI * i as f32 / STACKS as f32;
        let (sin_p, cos_p) = phi.sin_cos();
        for j in 0..=SLICES {
            let theta = 2.0 * std::f32::consts::PI * j as f32 / SLICES as f32;
            let (sin_t, cos_t) = theta.sin_cos();
            let nx = sin_p * cos_t;
            let ny = cos_p;
            let nz = sin_p * sin_t;
            verts.push(Vertex {
                position: [nx * 0.5, ny * 0.5, nz * 0.5],
                color: [1.0, 1.0, 1.0],
                normal: [nx, ny, nz],
                uv: [j as f32 / SLICES as f32, i as f32 / STACKS as f32],
            });
        }
    }

    let row = SLICES + 1;
    for i in 0..STACKS {
        for j in 0..SLICES {
            let a = i * row + j;
            let b = a + row;
            idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }

    (verts, idx)
}

/// Vertices/indices for a flat unit-sized quad in the XZ plane, facing +Y.
pub fn plane_vertices() -> (Vec<Vertex>, Vec<u32>) {
    let verts = vec![
        Vertex {
            position: [-0.5, 0.0, -0.5],
            color: [1.0; 3],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.0, -0.5],
            color: [1.0; 3],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 0.0],
        },
        Vertex {
            position: [0.5, 0.0, 0.5],
            color: [1.0; 3],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 1.0],
        },
        Vertex {
            position: [-0.5, 0.0, 0.5],
            color: [1.0; 3],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 1.0],
        },
    ];
    let idx = vec![0, 2, 1, 0, 3, 2];
    (verts, idx)
}

/// Vertices/indices for a capsule: a cylindrical body capped with two
/// hemispheres, built as stacked rings of vertices.
pub fn capsule_vertices() -> (Vec<Vertex>, Vec<u32>) {
    const SLICES: u32 = 24;
    const CAP_STACKS: u32 = 8;
    const RADIUS: f32 = 0.25;
    const HALF_CYL: f32 = 0.25;

    let mut verts = Vec::new();
    let mut idx = Vec::new();
    let row = SLICES + 1;

    let push_ring = |verts: &mut Vec<Vertex>, y: f32, r: f32, ny: f32, uv_v: f32| {
        let horiz = (1.0f32 - ny * ny).max(0.0).sqrt();
        for j in 0..=SLICES {
            let theta = 2.0 * std::f32::consts::PI * j as f32 / SLICES as f32;
            let (s, c) = theta.sin_cos();
            verts.push(Vertex {
                position: [r * c, y, r * s],
                color: [1.0; 3],
                normal: [horiz * c, ny, horiz * s],
                uv: [j as f32 / SLICES as f32, uv_v],
            });
        }
    };

    // top hemisphere: phi 0→PI/2 (top pole → top equator)
    for i in 0..=CAP_STACKS {
        let phi = std::f32::consts::FRAC_PI_2 * i as f32 / CAP_STACKS as f32;
        let (sin_p, cos_p) = phi.sin_cos();
        let uv_v = i as f32 / (2 * CAP_STACKS + 2) as f32;
        push_ring(
            &mut verts,
            HALF_CYL + RADIUS * cos_p,
            RADIUS * sin_p,
            cos_p,
            uv_v,
        );
    }

    // bottom hemisphere: i=0→CAP_STACKS maps equator→bottom pole
    for i in 0..=CAP_STACKS {
        let phi = std::f32::consts::FRAC_PI_2 * i as f32 / CAP_STACKS as f32;
        let (sin_p, cos_p) = phi.sin_cos();
        let uv_v = (CAP_STACKS + 1 + i) as f32 / (2 * CAP_STACKS + 2) as f32;
        push_ring(
            &mut verts,
            -HALF_CYL - RADIUS * sin_p,
            RADIUS * cos_p,
            -sin_p,
            uv_v,
        );
    }

    let total_rings = 2 * CAP_STACKS + 2;
    for i in 0..total_rings {
        for j in 0..SLICES {
            let a = i * row + j;
            let b = a + row;
            idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }

    (verts, idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_has_three_verts() {
        let (verts, indices) = triangle_vertices();
        assert_eq!(verts.len(), 3);
        assert_eq!(indices.len(), 3);
    }

    #[test]
    fn cube_has_24_verts_and_36_indices() {
        let (verts, indices) = cube_vertices();
        assert_eq!(verts.len(), 24);
        assert_eq!(indices.len(), 36);
    }

    #[test]
    fn cube_vertices_have_unit_normals() {
        let (verts, _) = cube_vertices();
        for v in &verts {
            let len = (v.normal[0].powi(2) + v.normal[1].powi(2) + v.normal[2].powi(2)).sqrt();
            assert!((len - 1.0).abs() < 1e-5, "normal not unit: {:?}", v.normal);
        }
    }

    #[test]
    fn cube_vertices_have_uvs_in_0_1_range() {
        let (verts, _) = cube_vertices();
        for v in &verts {
            assert!(
                v.uv[0] >= 0.0 && v.uv[0] <= 1.0,
                "u out of range: {}",
                v.uv[0]
            );
            assert!(
                v.uv[1] >= 0.0 && v.uv[1] <= 1.0,
                "v out of range: {}",
                v.uv[1]
            );
        }
    }

    fn make_vert(pos: [f32; 3]) -> Vertex {
        Vertex {
            position: pos,
            color: [1.0; 3],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0; 2],
        }
    }

    #[test]
    fn bounding_sphere_center_is_centroid() {
        let verts = vec![
            make_vert([1.0, 0.0, 0.0]),
            make_vert([-1.0, 0.0, 0.0]),
            make_vert([0.0, 0.0, 0.0]),
        ];
        let (center, _) = compute_bounding_sphere(&verts);
        assert!(
            center.length() < 1e-5,
            "center should be origin, got {:?}",
            center
        );
    }

    #[test]
    fn bounding_sphere_radius_covers_all_verts() {
        let verts = vec![
            make_vert([2.0, 0.0, 0.0]),
            make_vert([-2.0, 0.0, 0.0]),
            make_vert([0.0, 0.0, 0.0]),
        ];
        let (center, radius) = compute_bounding_sphere(&verts);
        for v in &verts {
            let d = (Vec3::from(v.position) - center).length();
            assert!(
                d <= radius + 1e-5,
                "vertex outside sphere: d={d} radius={radius}"
            );
        }
    }

    #[test]
    fn cube_bounding_sphere_center_near_origin() {
        let (verts, _) = cube_vertices();
        let (center, radius) = compute_bounding_sphere(&verts);
        assert!(center.length() < 1e-4, "cube center should be origin");
        assert!(radius > 0.0, "radius should be positive");
        // cube goes from -0.5 to 0.5, max distance is sqrt(3)*0.5 ≈ 0.866
        assert!(radius < 1.0, "radius for unit cube should be < 1.0");
    }

    #[test]
    fn update_vertices_overwrites_existing_buffer_contents() {
        use crate::surface::WgpuSurface;
        let (device, queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let mut registry = GpuMeshRegistry::new(device);
        let (verts, indices) = triangle_vertices();
        let id = registry.register(&verts, &indices);

        let mut moved = verts.clone();
        moved[0].position[1] += 5.0;
        registry.update_vertices(&queue, id, &moved);

        // No direct GPU read-back API exists; this test's job is to prove
        // update_vertices doesn't panic against a real buffer/queue and that
        // the mesh is still registered afterward with the same id.
        assert!(registry.get(id).is_some());
    }

    /// The hot-reload path can hand `update_vertices` a rest pose that no longer
    /// matches the buffer, because `replace` resizes and `update_vertices` does
    /// not. wgpu turns a too-long `write_buffer` into a validation error, and
    /// with no error scope or `on_uncaptured_error` anywhere in this codebase
    /// that reaches wgpu's default handler, which *panics* — so without this
    /// guard a mesh that loses vertices on reload kills the running game.
    /// Defence in depth: the glTF rebuild is supposed to keep the two in step,
    /// but "supposed to" is not an acceptable distance from a dead process.
    #[test]
    fn update_vertices_refuses_a_size_mismatch_instead_of_overrunning_the_buffer() {
        use crate::surface::WgpuSurface;
        let (device, queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let mut reg = GpuMeshRegistry::new(device);
        let (verts, indices) = triangle_vertices();
        let id = reg.register(&verts, &indices);

        assert!(
            reg.update_vertices(&queue, id, &verts),
            "an exactly-sized upload must still go through, or skinning stops \
             uploading anything at all"
        );

        let mut too_many = verts.clone();
        too_many.extend_from_slice(&verts);
        assert!(
            !reg.update_vertices(&queue, id, &too_many),
            "an oversized upload must be refused here rather than handed to \
             wgpu, which panics the process on BufferOverrun"
        );

        assert!(
            !reg.update_vertices(&queue, id, &verts[..1]),
            "an undersized upload must be refused too -- it would write past \
             nothing but leave the tail holding stale vertices"
        );
        assert!(
            !reg.update_vertices(&queue, id, &[]),
            "an empty upload has nothing to write"
        );
        assert!(
            !reg.update_vertices(&queue, 9999, &verts),
            "an unknown id must report failure, not silently succeed"
        );

        // The guard must track `replace`, not the original register: after a
        // reload shrinks the mesh, the *new* count is what fits.
        let (cube_v, cube_i) = cube_vertices();
        assert!(reg.replace(id, &cube_v, &cube_i));
        assert!(
            !reg.update_vertices(&queue, id, &verts),
            "the pre-replace count must no longer fit"
        );
        assert!(
            reg.update_vertices(&queue, id, &cube_v),
            "the post-replace count must fit; a guard keyed to the register-time \
             count would reject every upload for the rest of the mesh's life"
        );
        assert!(
            reg.get(id).is_some(),
            "a refused upload must not drop the mesh"
        );
    }

    #[test]
    fn replace_swaps_geometry_under_the_same_id() {
        use crate::surface::WgpuSurface;
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let mut reg = GpuMeshRegistry::new(device);

        let (tri_v, tri_i) = triangle_vertices();
        let id = reg.register(&tri_v, &tri_i);
        let before = reg.get_bounds(id).expect("just registered");

        let (cube_v, cube_i) = cube_vertices();
        assert!(
            reg.replace(id, &cube_v, &cube_i),
            "replace must report success for an id that exists"
        );

        assert_eq!(
            reg.get(id).map(|m| m.index_count),
            Some(cube_i.len() as u32),
            "replace must update index_count, or draws would use the old count"
        );
        let after = reg.get_bounds(id).expect("still registered after replace");
        assert_ne!(
            before, after,
            "replace must recompute bounds -- stale bounds silently break culling"
        );
    }

    #[test]
    fn replace_reports_failure_for_an_unknown_id() {
        use crate::surface::WgpuSurface;
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let mut reg = GpuMeshRegistry::new(device);
        let (v, i) = triangle_vertices();
        assert!(
            !reg.replace(9999, &v, &i),
            "replace must not silently create a mesh under an id nobody allocated"
        );
        assert!(reg.get(9999).is_none());
    }
}

use bsengine_ecs::Resource;
use glam::Vec3;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub normal: [f32; 3],
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

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    /// Local-space bounding sphere (center, radius).
    pub bounds: (Vec3, f32),
}

#[derive(Resource)]
pub struct GpuMeshRegistry {
    device: Arc<wgpu::Device>,
    meshes: HashMap<u64, GpuMesh>,
    next_id: u64,
}

impl GpuMeshRegistry {
    pub fn new(device: Arc<wgpu::Device>) -> Self {
        Self {
            device,
            meshes: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn register(&mut self, vertices: &[Vertex], indices: &[u32]) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh vbo"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh ibo"),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        let bounds = compute_bounding_sphere(vertices);
        self.meshes.insert(
            id,
            GpuMesh {
                vertex_buffer,
                index_buffer,
                index_count: indices.len() as u32,
                bounds,
            },
        );
        id
    }

    pub fn get(&self, id: u64) -> Option<&GpuMesh> {
        self.meshes.get(&id)
    }

    pub fn get_bounds(&self, id: u64) -> Option<(Vec3, f32)> {
        self.meshes.get(&id).map(|m| m.bounds)
    }
}

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

pub fn plane_vertices() -> (Vec<Vertex>, Vec<u32>) {
    let verts = vec![
        Vertex { position: [-0.5, 0.0, -0.5], color: [1.0; 3], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] },
        Vertex { position: [ 0.5, 0.0, -0.5], color: [1.0; 3], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0] },
        Vertex { position: [ 0.5, 0.0,  0.5], color: [1.0; 3], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0] },
        Vertex { position: [-0.5, 0.0,  0.5], color: [1.0; 3], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0] },
    ];
    let idx = vec![0, 2, 1, 0, 3, 2];
    (verts, idx)
}

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
        push_ring(&mut verts, HALF_CYL + RADIUS * cos_p, RADIUS * sin_p, cos_p, uv_v);
    }

    // bottom hemisphere: i=0→CAP_STACKS maps equator→bottom pole
    for i in 0..=CAP_STACKS {
        let phi = std::f32::consts::FRAC_PI_2 * i as f32 / CAP_STACKS as f32;
        let (sin_p, cos_p) = phi.sin_cos();
        let uv_v = (CAP_STACKS + 1 + i) as f32 / (2 * CAP_STACKS + 2) as f32;
        push_ring(&mut verts, -HALF_CYL - RADIUS * sin_p, RADIUS * cos_p, -sin_p, uv_v);
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
}

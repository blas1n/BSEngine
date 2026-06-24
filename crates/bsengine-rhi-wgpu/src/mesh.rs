use bsengine_ecs::Resource;
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

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
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

        self.meshes.insert(
            id,
            GpuMesh {
                vertex_buffer,
                index_buffer,
                index_count: indices.len() as u32,
            },
        );
        id
    }

    pub fn get(&self, id: u64) -> Option<&GpuMesh> {
        self.meshes.get(&id)
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
}

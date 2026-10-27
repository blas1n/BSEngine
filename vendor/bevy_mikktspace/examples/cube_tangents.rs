//! This example demonstrates how to generate a mesh.

#![allow(
    clippy::bool_assert_comparison,
    clippy::useless_conversion,
    reason = "Crate auto-generated with many non-idiomatic decisions. See #7372 for details."
)]
#![expect(clippy::print_stdout, reason = "Allowed in examples.")]

type Face = [u32; 3];

#[derive(Debug)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    tex_coord: [f32; 2],
}

struct Mesh {
    faces: Vec<Face>,
    vertices: Vec<Vertex>,
}

fn vertex(mesh: &Mesh, face: usize, vert: usize) -> &Vertex {
    let vs: &[u32; 3] = &mesh.faces[face];
    &mesh.vertices[vs[vert] as usize]
}

impl bevy_mikktspace::Geometry for Mesh {
    fn num_faces(&self) -> usize {
        self.faces.len()
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        vertex(self, face, vert).position
    }

    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        vertex(self, face, vert).normal
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        vertex(self, face, vert).tex_coord
    }

    fn set_tangent(
        &mut self,
        tangent: Option<bevy_mikktspace::TangentSpace>,
        face: usize,
        vert: usize,
    ) {
        println!(
            "{face}-{vert}: v: {v:?}, vn: {vn:?}, vt: {vt:?}, vx: {vx:?}",
            face = face,
            vert = vert,
            v = vertex(self, face, vert).position,
            vn = vertex(self, face, vert).normal,
            vt = vertex(self, face, vert).tex_coord,
            vx = tangent.map(|tangent| tangent.tangent_encoded()),
        );
    }
}

fn make_cube() -> Mesh {
    struct ControlPoint {
        uv: [f32; 2],
        dir: [f32; 3],
    }
    let mut faces = Vec::new();
    let mut ctl_pts = Vec::new();
    let mut vertices = Vec::new();

    // +x plane
    {
        let base = ctl_pts.len() as u32;
        faces.push([base, base + 1, base + 4]);
        faces.push([base + 1, base + 2, base + 4]);
        faces.push([base + 2, base + 3, base + 4]);
        faces.push([base + 3, base, base + 4]);
        ctl_pts.push(ControlPoint {
            uv: [0.0, 0.0],
            dir: [1.0, -1.0, 1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 1.0],
            dir: [1.0, -1.0, -1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [1.0, 1.0],
            dir: [1.0, 1.0, -1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [1.0, 0.0],
            dir: [1.0, 1.0, 1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.5, 0.5],
            dir: [1.0, 0.0, 0.0],
        });
    }

    // -x plane
    {
        let base = ctl_pts.len() as u32;
        faces.push([base, base + 1, base + 4]);
        faces.push([base + 1, base + 2, base + 4]);
        faces.push([base + 2, base + 3, base + 4]);
        faces.push([base + 3, base, base + 4]);
        ctl_pts.push(ControlPoint {
            uv: [1.0, 0.0],
            dir: [-1.0, 1.0, 1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [1.0, 1.0],
            dir: [-1.0, 1.0, -1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 1.0],
            dir: [-1.0, -1.0, -1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 0.0],
            dir: [-1.0, -1.0, 1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.5, 0.5],
            dir: [-1.0, 0.0, 0.0],
        });
    }

    // +y plane
    {
        let base = ctl_pts.len() as u32;
        faces.push([base, base + 1, base + 4]);
        faces.push([base + 1, base + 2, base + 4]);
        faces.push([base + 2, base + 3, base + 4]);
        faces.push([base + 3, base, base + 4]);
        ctl_pts.push(ControlPoint {
            uv: [0.0, 0.0],
            dir: [1.0, 1.0, 1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 1.0],
            dir: [1.0, 1.0, -1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 1.0],
            dir: [-1.0, 1.0, -1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 0.0],
            dir: [-1.0, 1.0, 1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 0.5],
            dir: [0.0, 1.0, 0.0],
        });
    }

    // -y plane
    {
        let base = ctl_pts.len() as u32;
        faces.push([base, base + 1, base + 4]);
        faces.push([base + 1, base + 2, base + 4]);
        faces.push([base + 2, base + 3, base + 4]);
        faces.push([base + 3, base, base + 4]);
        ctl_pts.push(ControlPoint {
            uv: [0.0, 0.0],
            dir: [-1.0, -1.0, 1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 1.0],
            dir: [-1.0, -1.0, -1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 1.0],
            dir: [1.0, -1.0, -1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 0.0],
            dir: [1.0, -1.0, 1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 0.5],
            dir: [0.0, -1.0, 0.0],
        });
    }

    // +z plane
    {
        let base = ctl_pts.len() as u32;
        faces.push([base, base + 1, base + 4]);
        faces.push([base + 1, base + 2, base + 4]);
        faces.push([base + 2, base + 3, base + 4]);
        faces.push([base + 3, base, base + 4]);
        ctl_pts.push(ControlPoint {
            uv: [0.0, 0.0],
            dir: [-1.0, 1.0, 1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 1.0],
            dir: [-1.0, -1.0, 1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [1.0, 1.0],
            dir: [1.0, -1.0, 1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [1.0, 0.0],
            dir: [1.0, 1.0, 1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.5, 0.5],
            dir: [0.0, 0.0, 1.0],
        });
    }

    // -z plane
    {
        let base = ctl_pts.len() as u32;
        faces.push([base, base + 1, base + 4]);
        faces.push([base + 1, base + 2, base + 4]);
        faces.push([base + 2, base + 3, base + 4]);
        faces.push([base + 3, base, base + 4]);
        ctl_pts.push(ControlPoint {
            uv: [1.0, 0.0],
            dir: [1.0, 1.0, -1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [1.0, 1.0],
            dir: [1.0, -1.0, -1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 1.0],
            dir: [-1.0, -1.0, -1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.0, 0.0],
            dir: [-1.0, 1.0, -1.0],
        });
        ctl_pts.push(ControlPoint {
            uv: [0.5, 0.5],
            dir: [0.0, 0.0, -1.0],
        });
    }

    for pt in ctl_pts {
        let p = pt.dir;
        let n = normalize(p);
        let t = pt.uv;
        vertices.push(Vertex {
            position: p.map(|x| x / 2.0),
            normal: n,
            tex_coord: t,
        });
    }

    Mesh { faces, vertices }
}

fn main() {
    let mut cube = make_cube();
    let _ = bevy_mikktspace::generate_tangents(&mut cube);
}

fn normalize([ax, ay, az]: [f32; 3]) -> [f32; 3] {
    let f = 1.0 / (ax * ax + ay * ay + az * az).sqrt();
    [ax, ay, az].map(|x| f * x)
}

//! Vertex tangents: the frame a normal map is read in.
//!
//! A tangent-space normal map stores, per texel, a normal relative to the
//! surface's tangent (`+u`), bitangent (`+v`) and normal. It is only right
//! under the very tangents it was baked against, so the convention is not a
//! free choice: MikkTSpace is what glTF prescribes for a model that brings
//! none ("client implementations SHOULD calculate tangents using default
//! MikkTSpace algorithms"), and what Blender, Substance and the three
//! reference engines bake and shade with. [`generate`] is that.
//!
//! The layout is glTF's: `xyz` the unit tangent, `w` the handedness (±1), with
//! the bitangent `cross(normal, tangent) * w`. A `w` of exactly 0 means "no
//! tangent yet": every hand-built mesh in the engine starts that way, and
//! [`crate::mesh::GpuMeshRegistry::register`] fills such a mesh in, so no
//! caller has to remember to.
//!
//! [`accumulate`] is the cheap per-frame stand-in for a mesh deformed on the
//! CPU every frame (cloth): Lengyel's per-face `dP/du` summed per vertex, as
//! its normals are. On a regular grid it is MikkTSpace's answer; on anything
//! else it is close, and never NaN.

use crate::mesh::Vertex;
use glam::{Vec2, Vec3};

/// Whether `vertices` still need their tangents generated: any one of them
/// carries the "none yet" `w` of 0. One is enough -- a mesh is generated whole.
pub fn missing(vertices: &[Vertex]) -> bool {
    vertices.iter().any(|v| v.tangent[3] == 0.0)
}

/// MikkTSpace tangents for every vertex of the triangle list `indices`,
/// written in place.
///
/// MikkTSpace answers per triangle corner; a vertex shared by several
/// triangles is given the normalised sum of its corners' tangents and the
/// majority of their handedness. Re-indexing the corners instead (as
/// three.js's `computeMikkTSpaceTangents` does) would split the few vertices
/// whose corners disagree, and a registered mesh must keep its vertex count:
/// cloth and CPU skinning rewrite the buffer in place by index
/// (`GpuMeshRegistry::update_vertices`). Corners disagree only where
/// MikkTSpace itself splits the frame, which for a mesh exported with the
/// tangents it was baked with does not happen -- that mesh brings its own and
/// is never generated.
///
/// A vertex no triangle reaches, or one whose triangles have no usable UVs,
/// gets a tangent built from its normal alone (any direction perpendicular to
/// it), so the frame is never degenerate.
pub fn generate(vertices: &mut [Vertex], indices: &[u32]) {
    struct Mesh<'a> {
        vertices: &'a [Vertex],
        indices: &'a [u32],
        /// Per vertex: the summed tangent, and the handedness votes.
        sums: Vec<(Vec3, i32)>,
    }
    impl Mesh<'_> {
        fn vertex(&self, face: usize, vert: usize) -> &Vertex {
            &self.vertices[self.indices[face * 3 + vert] as usize]
        }
    }
    impl bevy_mikktspace::Geometry for Mesh<'_> {
        fn num_faces(&self) -> usize {
            self.indices.len() / 3
        }
        fn num_vertices_of_face(&self, _face: usize) -> usize {
            3
        }
        fn position(&self, face: usize, vert: usize) -> [f32; 3] {
            self.vertex(face, vert).position
        }
        fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
            self.vertex(face, vert).normal
        }
        fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
            self.vertex(face, vert).uv
        }
        fn set_tangent(
            &mut self,
            tangent_space: Option<bevy_mikktspace::TangentSpace>,
            face: usize,
            vert: usize,
        ) {
            let Some(t) = tangent_space else {
                return;
            };
            let [x, y, z, w] = t.tangent_encoded();
            let index = self.indices[face * 3 + vert] as usize;
            let (sum, votes) = &mut self.sums[index];
            *sum += Vec3::new(x, y, z);
            *votes += if w < 0.0 { -1 } else { 1 };
        }
    }

    // An index past the end would panic inside the generator; such a mesh is
    // malformed, and the frame built from the normal alone is what it gets.
    let valid = indices.iter().all(|&i| (i as usize) < vertices.len());
    let mut sums = vec![(Vec3::ZERO, 0); vertices.len()];
    if valid {
        let mut mesh = Mesh {
            vertices,
            indices: &indices[..indices.len() / 3 * 3],
            sums,
        };
        // The error type has no variants: generation cannot fail.
        let _ = bevy_mikktspace::generate_tangents(&mut mesh);
        sums = mesh.sums;
    }
    for (v, (sum, votes)) in vertices.iter_mut().zip(sums) {
        let w = if votes < 0 { -1.0 } else { 1.0 };
        v.tangent = finish(Vec3::from(v.normal), sum, w);
    }
}

/// Lengyel's tangents, for a mesh deformed on the CPU every frame: each
/// triangle's `dP/du` (and its handedness, from `dP/dv`) summed into its
/// corners, then made perpendicular to the vertex normal. Read the normals
/// first -- this orthogonalises against whatever `normal` holds.
pub fn accumulate(vertices: &mut [Vertex], indices: &[u32]) {
    let mut sums = vec![(Vec3::ZERO, Vec3::ZERO); vertices.len()];
    for tri in indices.chunks_exact(3) {
        let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        if a >= vertices.len() || b >= vertices.len() || c >= vertices.len() {
            continue;
        }
        let p = |i: usize| Vec3::from(vertices[i].position);
        let uv = |i: usize| Vec2::from(vertices[i].uv);
        let (e1, e2) = (p(b) - p(a), p(c) - p(a));
        let (d1, d2) = (uv(b) - uv(a), uv(c) - uv(a));
        let det = d1.x * d2.y - d2.x * d1.y;
        if det.abs() < 1e-12 {
            continue;
        }
        let r = 1.0 / det;
        let t = (e1 * d2.y - e2 * d1.y) * r;
        let s = (e2 * d1.x - e1 * d2.x) * r;
        for i in [a, b, c] {
            sums[i].0 += t;
            sums[i].1 += s;
        }
    }
    for (v, (t, s)) in vertices.iter_mut().zip(sums) {
        let n = Vec3::from(v.normal);
        let w = if n.cross(t).dot(s) < 0.0 { -1.0 } else { 1.0 };
        v.tangent = finish(n, t, w);
    }
}

/// `t` made a unit vector perpendicular to `n` (Gram-Schmidt), with
/// handedness `w`; any perpendicular when `t` has nothing left of it.
fn finish(n: Vec3, t: Vec3, w: f32) -> [f32; 4] {
    let n = n.try_normalize().unwrap_or(Vec3::Y);
    let t = (t - n * n.dot(t))
        .try_normalize()
        .unwrap_or_else(|| n.any_orthonormal_vector());
    [t.x, t.y, t.z, w]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(position: [f32; 3], uv: [f32; 2]) -> Vertex {
        Vertex {
            position,
            color: [1.0; 3],
            normal: [0.0, 0.0, 1.0],
            uv,
            uv2: [0.0; 2],
            tangent: [0.0; 4],
        }
    }

    /// A unit quad in the XY plane facing +z, two triangles, `uv` laid on it
    /// by `uv_of`.
    fn quad(uv_of: impl Fn([f32; 2]) -> [f32; 2]) -> (Vec<Vertex>, Vec<u32>) {
        let corners = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        (
            corners
                .iter()
                .map(|&[x, y]| vertex([x, y, 0.0], uv_of([x, y])))
                .collect(),
            vec![0, 1, 2, 0, 2, 3],
        )
    }

    fn close(a: [f32; 4], b: [f32; 4]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    /// Closed form: `u` along +x and `v` along +y on a face whose normal is
    /// +z is the right-handed frame, tangent +x and `w` = +1 (the bitangent
    /// `cross(+z, +x) = +y` is `+v` already). Every vertex, shared or not.
    #[test]
    fn u_along_x_and_v_along_y_give_tangent_x_right_handed() {
        let (mut v, i) = quad(|p| p);
        assert!(missing(&v), "premise: none yet");
        generate(&mut v, &i);
        for (k, v) in v.iter().enumerate() {
            assert!(
                close(v.tangent, [1.0, 0.0, 0.0, 1.0]),
                "vertex {k}: {:?}",
                v.tangent
            );
        }
        assert!(!missing(&v));
    }

    /// The other two cases a normal map can be laid on: `u` running along
    /// +y turns the tangent to +y (the frame follows the UVs, not the
    /// geometry), and `u` mirrored -- a texture flipped on this half of a
    /// symmetric model -- flips the tangent and the handedness with it.
    #[test]
    fn the_tangent_follows_u_and_a_mirrored_u_flips_the_handedness() {
        let (mut turned, i) = quad(|[x, y]| [y, 1.0 - x]);
        generate(&mut turned, &i);
        assert!(
            close(turned[0].tangent, [0.0, 1.0, 0.0, 1.0]),
            "{:?}",
            turned[0].tangent
        );

        let (mut mirrored, i) = quad(|[x, y]| [1.0 - x, y]);
        generate(&mut mirrored, &i);
        assert!(
            close(mirrored[0].tangent, [-1.0, 0.0, 0.0, -1.0]),
            "{:?}",
            mirrored[0].tangent
        );
    }

    /// A mesh with no usable UVs (every one alike) and a vertex no triangle
    /// uses still get a unit tangent perpendicular to their normal: the frame
    /// is never zero, which would turn a normal map's every texel into NaN.
    #[test]
    fn no_uvs_and_unused_vertices_still_get_a_perpendicular_unit_tangent() {
        let (mut v, i) = quad(|_| [0.5, 0.5]);
        v.push(vertex([5.0, 5.0, 5.0], [0.0, 0.0]));
        generate(&mut v, &i);
        for (k, v) in v.iter().enumerate() {
            let t = Vec3::new(v.tangent[0], v.tangent[1], v.tangent[2]);
            assert!((t.length() - 1.0).abs() < 1e-5, "vertex {k}: {t}");
            assert!(t.dot(Vec3::from(v.normal)).abs() < 1e-5, "vertex {k}: {t}");
            assert!(v.tangent[3].abs() == 1.0, "vertex {k}");
        }
    }

    /// The per-frame stand-in agrees with MikkTSpace where cloth lives -- a
    /// regular grid -- including the handedness, and stays perpendicular to
    /// the normal once the sheet is bent (which MikkTSpace would also give,
    /// but at a cost cloth pays every frame).
    #[test]
    fn accumulating_agrees_with_mikktspace_on_a_grid() {
        let (mut a, i) = quad(|[x, y]| [x, 1.0 - y]);
        let mut b = a.clone();
        generate(&mut a, &i);
        accumulate(&mut b, &i);
        for k in 0..4 {
            assert!(
                close(a[k].tangent, b[k].tangent),
                "{k}: {:?} {:?}",
                a[k].tangent,
                b[k].tangent
            );
        }
        assert_eq!(a[0].tangent[3], -1.0, "premise: v runs against +y here");
        // Bend one corner's normal: the tangent stays perpendicular to it.
        b[2].normal = Vec3::new(0.6, 0.0, 0.8).to_array();
        accumulate(&mut b, &i);
        let t = Vec3::new(b[2].tangent[0], b[2].tangent[1], b[2].tangent[2]);
        assert!(t.dot(Vec3::from(b[2].normal)).abs() < 1e-5, "{t}");
    }
}

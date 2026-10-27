//! # Copyright
//!
//! This code is a Rust reimplementation of <https://github.com/mmikk/MikkTSpace>.
//! The copyright notice below reflects that history, and should not be removed.
//!
//! > Copyright (C) 2011 by Morten S. Mikkelsen
//! >
//! > This software is provided 'as-is', without any express or implied
//! > warranty.  In no event will the authors be held liable for any damages
//! > arising from the use of this software.
//! >
//! > Permission is granted to anyone to use this software for any purpose,
//! > including commercial applications, and to alter it and redistribute it
//! > freely, subject to the following restrictions:
//! >
//! > 1. The origin of this software must not be misrepresented; you must not
//! >    claim that you wrote the original software. If you use this software
//! >    in a product, an acknowledgment in the product documentation would be
//! >    appreciated but is not required.
//! > 2. Altered source versions must be plainly marked as such, and must not be
//! >    misrepresented as being the original software.
//! > 3. This notice may not be removed or altered from any source distribution.

mod face_vertex;
#[cfg_attr(
    not(feature = "corrected-edge-sorting"),
    path = "mikktspace/quick_sort_legacy.rs"
)]
mod quick_sort;
mod raw_tangent_space;
#[cfg_attr(
    not(feature = "corrected-vertex-welding"),
    path = "mikktspace/weld_vertices_legacy.rs"
)]
mod weld_vertices;

use alloc::{vec, vec::Vec};

use self::{
    face_vertex::FaceVertex, quick_sort::quick_sort_edges, raw_tangent_space::RawTangentSpace,
    weld_vertices::weld_vertices,
};
use crate::{GenerateTangentSpaceError, Geometry, math::*};

pub(crate) fn generate_tangent_space_and_write<I: Geometry<O>, O: Ops>(
    context: &mut I,
    linear_threshold: f32,
) -> Result<(), GenerateTangentSpaceError> {
    let faces_total = context.num_faces();

    let tangent_spaces = generate_tangent_space(&*context, linear_threshold, faces_total)?;

    let mut tangent_spaces_iter = tangent_spaces.iter();
    for f in 0..faces_total {
        let vertices = context.num_vertices_of_face(f);
        if vertices == 3 || vertices == 4 {
            // I've decided to let degenerate triangles and group-with-anythings
            // vary between left/right hand coordinate systems at the vertices.
            // All healthy triangles on the other hand are built to always be either or.
            // set data
            for v in 0..vertices {
                // Under certain circumstances a tangent space may not be calculated
                // for a particular vertex on a face.
                // For consistency with the C implementation, we provide the default.
                let tangent_space = tangent_spaces_iter
                    .next()
                    .unwrap()
                    .map(crate::TangentSpace::from);

                context.set_tangent(tangent_space, f, v);
            }
        }
    }

    Ok(())
}

/// Generate [`TangentSpace`] values for the provided [`Geometry`].
///
/// This is separated from [`generate_tangent_space_and_write`] to highlight this
/// step does not require mutable access to the provided [`Geometry`].
fn generate_tangent_space<I: Geometry<O>, O: Ops>(
    context: &I,
    linear_threshold: f32,
    faces_total: usize,
) -> Result<Vec<Option<TangentSpace<O>>>, GenerateTangentSpaceError> {
    // This set of operations can be done on-line.
    // That is, they do not require an intermediatory buffer.
    let mut triangle_info_list = generate_triangle_info_list(context, faces_total);

    // Below here, we require buffered triangle and vertex information.

    // Want to partition by degeneracy
    let (triangles_good, triangles_degenerate) = {
        triangle_info_list.sort_by_key(|face| (face.is_degenerate, face.tangent_spaces_offset));
        let partition = triangle_info_list.partition_point(|face| !face.is_degenerate);
        triangle_info_list.split_at_mut(partition)
    };

    // Make a welded index list of identical positions and attributes (pos, norm, texc).
    // By creating this list after segregating good and degenerate triangles, this list
    // is also automatically segregated as well.
    let mut triangle_vertex_list = triangles_good
        .iter()
        .chain(triangles_degenerate.iter())
        .flat_map(|info| {
            info.vertex_indices()
                .map(|t| FaceVertex::new(info.original_face_index, t))
        })
        .collect::<Vec<_>>();
    weld_vertices(context, &mut triangle_vertex_list);

    let tangent_spaces = generate_tangent_spaces(
        triangles_good,
        &*triangles_degenerate,
        &triangle_vertex_list,
        linear_threshold,
        context,
    );

    Ok(tangent_spaces)
}

struct TangentSpace<O: Ops> {
    /// [`RawTangentSpace`] value this [`TangentSpace`] contains.
    value: RawTangentSpace<O>,
    /// Indicates this value has already been [combined](RawTangentSpace::combine)
    /// with another.
    /// This only occurs with quads, where each triangle's tangent values
    /// will be combined to produce the final result.
    is_combined: bool,
    orientation_preserving: bool,
}

impl<O: Ops> Copy for TangentSpace<O> {}

impl<O: Ops> Clone for TangentSpace<O> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, O: Ops> From<&'a TangentSpace<O>> for crate::TangentSpace {
    fn from(value: &'a TangentSpace<O>) -> Self {
        crate::TangentSpace {
            tangent: [value.value.s.x, value.value.s.y, value.value.s.z],
            bi_tangent: [value.value.t.x, value.value.t.y, value.value.t.z],
            mag_s: value.value.s_magnitude,
            mag_t: value.value.t_magnitude,
            is_orientation_preserving: value.orientation_preserving,
        }
    }
}

impl<O: Ops> From<TangentSpace<O>> for crate::TangentSpace {
    fn from(value: TangentSpace<O>) -> Self {
        crate::TangentSpace::from(&value)
    }
}

struct TriangleInfo<O: Ops> {
    /// Stores the index of each neighboring triangle to this one, if any.
    /// Indices correspond to _edges_ of this triangle, rather than _vertices_.
    ///
    /// In other words, `face_neighbors[n] = a` implies the nth edge is shared with
    /// a [triangle](TriangleInfo) with index `a`.
    face_neighbors: [Option<usize>; 3],
    /// Stores which [`Group`] each vertex on this triangle is a member of, if any.
    ///
    /// In other words, `assigned_group[n] = a` implies the nth vertex is a part
    /// of the [`Group`] indexed by `a`.
    assigned_group: [Option<usize>; 3],
    /// First-order tangent and bi-tangent value for this triangle.
    tangent: RawTangentSpace<O>,
    /// Determines if the current and the next triangle are a quad.
    original_face_index: usize,
    /// Index into a list of [`TangentSpace`] values.
    /// As each [`TangentSpace`] value corresponds to a vertex on this triangle,
    /// [`tangent_spaces_offset`](TriangleInfo::tangent_spaces_offset) corresponds
    /// to the 0th vertex, `tangent_spaces_offset + 1` to the 1st, and likewise
    /// `tangent_spaces_offset + 2` to the 2nd.
    tangent_spaces_offset: usize,
    /// Indicates which vertex this triangle does not contain from its original face.
    /// For triangles, this will be [`None`].
    /// For quads, it will be a value in the range `0..=3`.
    missing_vertex: Option<u8>,
    /// Indicates that one or more vertices of this triangle are coincident,
    /// reducing the triangle to either a line or point.
    is_degenerate: bool,
    /// Indicates this triangle is a member of a quad where one of the triangles
    /// is degenerate, but the other is not.
    quad_with_one_degenerate_triangle: bool,
    /// Indicates that this triangle can be [grouped](Group) with any of its
    /// neighbors, regardless of typical merging rules.
    /// This typically happens when a triangle is poorly defined and must rely on
    /// a neighbor for a meaningful tangent value.
    group_with_any: bool,
    orientation_preserving: bool,
}

impl<O: Ops> Copy for TriangleInfo<O> {}

impl<O: Ops> Clone for TriangleInfo<O> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<O: Ops> TriangleInfo<O> {
    const fn vertex_indices(&self) -> [u8; 3] {
        match self.missing_vertex {
            None | Some(3) => [0, 1, 2],
            Some(0) => [1, 2, 3],
            Some(1) => [0, 2, 3],
            Some(2) => [0, 1, 3],
            _ => unreachable!(),
        }
    }
}

#[derive(Clone)]
struct Group {
    id: usize,
    face_indices: Vec<usize>,
    vertex_representative: FaceVertex,
    orientation_preserving: bool,
}

fn generate_triangle_info_list<I: Geometry<O>, O: Ops>(
    context: &I,
    faces_total: usize,
) -> Vec<TriangleInfo<O>> {
    #[expect(clippy::map_flatten, reason = "provides clarity")]
    (0..faces_total)
        // Only support triangles and quads
        .filter_map(|f| match context.num_vertices_of_face(f) {
            verts @ (3 | 4) => Some((f, verts)),
            _ => None,
        })
        // Track `tangent_space_offset` state
        .scan(0, |tangent_space_offset, (f, verts)| {
            let result = (f, verts, *tangent_space_offset);
            *tangent_space_offset += verts;
            Some(result)
        })
        // Triangulate supported faces and map them to `TriangleInfo`s
        .map(|(f, verts, tangent_spaces_offset)| {
            let info = TriangleInfo {
                face_neighbors: [None; 3],
                assigned_group: [None; 3],
                tangent: RawTangentSpace::ZERO,
                original_face_index: f,
                tangent_spaces_offset,
                missing_vertex: None,
                is_degenerate: false,
                quad_with_one_degenerate_triangle: false,
                group_with_any: false,
                orientation_preserving: false,
            };

            if verts == 3 {
                [Some(info), None]
            } else if verts == 4 {
                let mut info_a = info;
                let mut info_b = info;

                // need an order independent way to evaluate
                // tspace on quads. This is done by splitting
                // along the shortest diagonal.
                let i = [0, 1, 2, 3].map(|i| FaceVertex::new(f, i));
                let tx = i.map(|i| get_texture_coordinate_from_index(context, i));
                let d = [(2, 0), (3, 1)].map(|(a, b)| [0, 1].map(|i| tx[a][i] - tx[b][i]));
                let l = d.map(|d| d[0] * d[0] + d[1] * d[1]);

                let quad_diagonal_is_02 = match l[0].partial_cmp(&l[1]) {
                    Some(core::cmp::Ordering::Less) => true,
                    Some(core::cmp::Ordering::Greater) => false,
                    _ => {
                        let p = i.map(|i| get_position_from_index(context, i));
                        let d = [(2, 0), (3, 1)].map(|(a, b)| (p[a] - p[b]).length_squared());
                        d[1] >= d[0]
                    }
                };

                if quad_diagonal_is_02 {
                    info_a.missing_vertex = Some(3);
                    info_b.missing_vertex = Some(1);
                } else {
                    info_a.missing_vertex = Some(2);
                    info_b.missing_vertex = Some(0);
                }

                [Some(info_a), Some(info_b)]
            } else {
                unreachable!()
            }
        })
        // Mark faces as degenerate.
        // Done at the chunk stage to simplify marking partially degenerate quads.
        .map(|mut chunk| {
            for face in &mut chunk {
                let Some(face) = face else { continue };
                let p = face
                    .vertex_indices()
                    .map(|i| context.position(face.original_face_index, i as usize));
                let iter = p.iter().cycle();
                if iter.clone().zip(iter.skip(1)).take(3).any(|(a, b)| a == b) {
                    face.is_degenerate = true
                }
            }
            chunk
        })
        // Mark quads with exactly one degenerate triangle
        .map(|mut chunk| {
            let [Some(a), Some(b)] = &mut chunk else {
                return chunk;
            };

            if a.is_degenerate ^ b.is_degenerate {
                a.quad_with_one_degenerate_triangle = true;
                b.quad_with_one_degenerate_triangle = true;
            }

            chunk
        })
        // Computes a first-order solution for `tangent`.
        .map(|mut chunk| {
            chunk
                .iter_mut()
                .flatten()
                .filter(|info| !info.is_degenerate)
                .map(|info| {
                    // initial values
                    let v = info
                        .vertex_indices()
                        .map(|i| context.position(info.original_face_index, i as usize))
                        .map(Vec3::<O>::from);
                    let tx = info
                        .vertex_indices()
                        .map(|i| context.tex_coord(info.original_face_index, i as usize));

                    let d_tx = [1, 2].map(|t| [0, 1].map(|i| tx[t][i] - tx[0][i]));
                    let d_v = [1, 2].map(|i| v[i] - v[0]);

                    let signed_area_double = d_tx[0][0] * d_tx[1][1] - d_tx[0][1] * d_tx[1][0];
                    let area_double = fabsf(signed_area_double);

                    let s = (d_tx[1][1] * d_v[0]) - (d_tx[0][1] * d_v[1]); // eq 18
                    let t = (-d_tx[1][0] * d_v[0]) + (d_tx[0][0] * d_v[1]); // eq 19

                    // assumed bad
                    info.group_with_any = true;

                    if signed_area_double > 0f32 {
                        info.orientation_preserving = true;
                    }

                    (info, s, t, area_double)
                })
                .filter(|(_, _, _, area_double)| not_zero(*area_double))
                .map(|(info, s, t, area_double)| {
                    // multiplying by `sign` ensures NaN byte compatibility with the C
                    // implementation, compared to conditional negation.
                    let sign = if info.orientation_preserving {
                        1.0f32
                    } else {
                        -1.0f32
                    };

                    info.tangent = RawTangentSpace {
                        s: s.normalized_or_zero() * sign,
                        t: t.normalized_or_zero() * sign,
                        s_magnitude: s.length() / area_double,
                        t_magnitude: t.length() / area_double,
                    };

                    info
                })
                .filter(|info| {
                    not_zero(info.tangent.s_magnitude) && not_zero(info.tangent.t_magnitude)
                })
                .for_each(|info| {
                    // if this is a good triangle
                    info.group_with_any = false;
                });

            chunk
        })
        // Force otherwise healthy quads to a fixed orientation.
        .map(|mut chunk| {
            let [Some(a), Some(b)] = &mut chunk else {
                return chunk;
            };

            if a.is_degenerate || b.is_degenerate {
                return chunk;
            }

            // if this happens the quad has extremely bad mapping!!
            if a.orientation_preserving == b.orientation_preserving {
                return chunk;
            }

            let tx_area_a = calculate_texture_area(context, &*a);
            let tx_area_b = calculate_texture_area(context, &*b);

            // force match
            let [a, b] = if b.group_with_any || tx_area_a >= tx_area_b {
                [a, b]
            } else {
                [b, a]
            };

            b.orientation_preserving = a.orientation_preserving;

            chunk
        })
        .flatten()
        .flatten()
        .collect::<Vec<_>>()
}

fn get_position_from_index<I: Geometry<O>, O: Ops>(context: &I, index: FaceVertex) -> Vec3<O> {
    context
        .position(index.face(), index.vertex() as usize)
        .into()
}

fn get_normal_from_index<I: Geometry<O>, O: Ops>(context: &I, index: FaceVertex) -> Vec3<O> {
    context.normal(index.face(), index.vertex() as usize).into()
}

fn get_texture_coordinate_from_index<I: Geometry<O>, O: Ops>(
    context: &I,
    index: FaceVertex,
) -> [f32; 2] {
    context.tex_coord(index.face(), index.vertex() as usize)
}

/// Returns the texture area times 2.
fn calculate_texture_area<I: Geometry<O>, O: Ops>(context: &I, info: &TriangleInfo<O>) -> f32 {
    let tx = info
        .vertex_indices()
        .map(|i| context.tex_coord(info.original_face_index, i as usize));
    let d_tx = [1, 2].map(|t| [0, 1].map(|i| tx[t][i] - tx[0][i]));

    let signed_area_double = d_tx[0][0] * d_tx[1][1] - d_tx[0][1] * d_tx[1][0];

    fabsf(signed_area_double)
}

/// Based on the 4 rules, identify [groups](Group) based on connectivity.
fn build_4_rule_groups<O: Ops>(
    triangle_info_list: &mut [TriangleInfo<O>],
    triangle_vertex_list: &[FaceVertex],
) -> Vec<Group> {
    let (triangle_vertex_list, _) = triangle_vertex_list.split_at(triangle_info_list.len() * 3);

    let mut ids = 0..;
    (0..triangle_info_list.len())
        .zip(triangle_vertex_list.chunks_exact(3))
        .flat_map(|(f, chunk)| chunk.iter().enumerate().map(move |(i, &vert)| (f, i, vert)))
        .filter_map(|(f, i, vertex_representative)| {
            let info = &mut triangle_info_list[f];

            if info.group_with_any || info.assigned_group[i].is_some() {
                return None;
            }

            let orientation_preserving = info.orientation_preserving;

            let mut group = Group {
                id: ids.next().unwrap(),
                vertex_representative,
                orientation_preserving,
                face_indices: vec![f],
            };

            info.assigned_group[i] = Some(group.id);

            let neighbors = [2, 0].map(|t| (i + t) % 3).map(|i| info.face_neighbors[i]);

            for &neighbor in neighbors.iter().flatten() {
                // neighbor
                let result = assign_to_group_recursive(
                    triangle_vertex_list,
                    triangle_info_list,
                    neighbor,
                    &mut group,
                );

                debug_assert!({
                    let orientation_preserving_neighbor =
                        triangle_info_list[neighbor].orientation_preserving;
                    let different = orientation_preserving != orientation_preserving_neighbor;

                    result || different
                });
            }

            Some(group)
        })
        .collect::<Vec<_>>()
}

fn assign_to_group_recursive<O: Ops>(
    triangle_vertex_list: &[FaceVertex],
    triangle_infos: &mut [TriangleInfo<O>],
    triangle_index: usize,
    group: &mut Group,
) -> bool {
    let triangle_info = &mut triangle_infos[triangle_index];

    // track down vertex
    let i = triangle_vertex_list
        .chunks_exact(3)
        .nth(triangle_index)
        .unwrap()
        .iter()
        .position(|&v| v == group.vertex_representative)
        .unwrap();

    // early out
    if let Some(id) = triangle_info.assigned_group[i] {
        return id == group.id;
    }

    if triangle_info.group_with_any
        && triangle_info.assigned_group[0].is_none()
        && triangle_info.assigned_group[1].is_none()
        && triangle_info.assigned_group[2].is_none()
    {
        // first to group with a group-with-anything triangle
        // determines it's orientation.
        // This is the only existing order dependency in the code!!
        triangle_info.orientation_preserving = false;
        if group.orientation_preserving {
            triangle_info.orientation_preserving = true;
        }
    }

    if triangle_info.orientation_preserving != group.orientation_preserving {
        return false;
    }

    group.face_indices.push(triangle_index);
    triangle_info.assigned_group[i] = Some(group.id);

    let neighbors = [2, 0]
        .map(|t| (i + t) % 3)
        .map(|i| triangle_info.face_neighbors[i]);
    for &neighbor in neighbors.iter().flatten() {
        assign_to_group_recursive(triangle_vertex_list, triangle_infos, neighbor, group);
    }

    true
}

/// Generate a list of [`TangentSpace`]s.
/// Each [`Group`] is split up into subgroups if necessary based on `threshold_cos`.
/// Finally a [`TangentSpace`] is made for every resulting subgroup
fn generate_tangent_spaces<I: Geometry<O>, O: Ops>(
    triangle_info_list: &mut [TriangleInfo<O>],
    triangles_degenerate: &[TriangleInfo<O>],
    triangle_vertex_list: &[FaceVertex],
    linear_threshold: f32,
    context: &I,
) -> Vec<Option<TangentSpace<O>>> {
    // Get the total number of tangent space values to be computed
    let tangent_spaces_total = triangle_info_list
        .last()
        .into_iter()
        .chain(triangles_degenerate.last())
        .max_by_key(|info| info.tangent_spaces_offset)
        .map(|info| {
            info.tangent_spaces_offset + context.num_vertices_of_face(info.original_face_index)
        })
        .unwrap_or(0);

    build_neighbors(triangle_info_list, triangle_vertex_list);

    let groups = build_4_rule_groups(triangle_info_list, triangle_vertex_list);

    let triangle_info_list = &*triangle_info_list;

    let mut tangent_spaces = vec![None; tangent_spaces_total];

    let Some(faces_max_count) = groups.iter().map(|group| group.face_indices.len()).max() else {
        return tangent_spaces;
    };

    // make initial allocations
    let mut sub_group_tangent_spaces = Vec::<RawTangentSpace<O>>::with_capacity(faces_max_count);
    let mut unified_sub_groups = Vec::<Vec<usize>>::with_capacity(faces_max_count);
    for (g, group) in groups.iter().enumerate() {
        // triangles
        for &f in group.face_indices.iter() {
            let a = &triangle_info_list[f];

            // triangle number
            let index = a
                .assigned_group
                .iter()
                .position(|group| group == &Some(g))
                .unwrap();

            let vertex_index = triangle_vertex_list[f * 3 + index];
            debug_assert!(vertex_index == group.vertex_representative);

            // is normalized already
            let n = get_normal_from_index(context, vertex_index);

            // project
            let s_a = (a.tangent.s - ((n.dot(a.tangent.s)) * n)).normalized_or_zero();
            let t_a = (a.tangent.t - ((n.dot(a.tangent.t)) * n)).normalized_or_zero();

            let mut tmp_group = group
                .face_indices
                .iter()
                .copied()
                .filter(|&t| {
                    let b = &triangle_info_list[t];

                    let meets_threshold = {
                        // project
                        let s_b = (b.tangent.s - ((n.dot(b.tangent.s)) * n)).normalized_or_zero();
                        let t_b = (b.tangent.t - ((n.dot(b.tangent.t)) * n)).normalized_or_zero();

                        let s_cos = s_a.dot(s_b);
                        let t_cos = t_a.dot(t_b);

                        s_cos > linear_threshold && t_cos > linear_threshold
                    };

                    let any = a.group_with_any || b.group_with_any;

                    // make sure triangles which belong to the same quad are joined.
                    let same_original_face = a.original_face_index == b.original_face_index;

                    any || same_original_face || meets_threshold
                })
                .collect::<Vec<_>>();

            // sort pTmpMembers
            tmp_group.sort_unstable();

            // look for an existing match
            let l = unified_sub_groups
                .iter()
                .position(|g| g == &tmp_group)
                .unwrap_or_else(|| {
                    // if no match was found we allocate a new subgroup
                    let l = sub_group_tangent_spaces.len();
                    sub_group_tangent_spaces.push(evaluate_tangent_space(
                        &tmp_group,
                        triangle_vertex_list,
                        triangle_info_list,
                        context,
                        group.vertex_representative,
                    ));
                    unified_sub_groups.push(tmp_group);
                    l
                });

            // output tspace
            let index = a.tangent_spaces_offset + a.vertex_indices()[index] as usize;
            let tangent_space = &mut tangent_spaces[index];

            debug_assert!(a.orientation_preserving == group.orientation_preserving);

            if let Some(tangent_space) = tangent_space {
                debug_assert!(!tangent_space.is_combined);
                *tangent_space = TangentSpace {
                    value: tangent_space.value.combine(sub_group_tangent_spaces[l]),
                    is_combined: true,
                    orientation_preserving: group.orientation_preserving,
                };
            } else {
                *tangent_space = Some(TangentSpace {
                    value: sub_group_tangent_spaces[l],
                    is_combined: false,
                    orientation_preserving: group.orientation_preserving,
                });
            }
        }

        sub_group_tangent_spaces.clear();
        unified_sub_groups.clear();
    }

    generate_tangent_spaces_for_degenerate_triangles(
        &mut tangent_spaces,
        triangle_info_list,
        triangles_degenerate,
        triangle_vertex_list,
    );

    generate_tangent_spaces_for_partially_degenerate_quads(
        &mut tangent_spaces,
        triangle_info_list,
        context,
    );

    tangent_spaces
}

fn evaluate_tangent_space<I: Geometry<O>, O: Ops>(
    face_indices: &[usize],
    triangle_vertex_list: &[FaceVertex],
    triangle_info_list: &[TriangleInfo<O>],
    context: &I,
    vertex_representative: FaceVertex,
) -> RawTangentSpace<O> {
    let (angle_sum, mut res) = face_indices
        .iter()
        .map(|&f| (&triangle_vertex_list[3 * f..][..3], &triangle_info_list[f]))
        // only valid triangles get to add their contribution
        .filter(|(_vertices, info)| !info.group_with_any)
        .fold(
            (0f32, RawTangentSpace::ZERO),
            |(angle_sum, mut res), (vertices, info)| {
                let i = (0..=2)
                    .find(|&i| vertices[i] == vertex_representative)
                    .unwrap();

                let n = get_normal_from_index(context, vertices[i]);
                let p = [1, 0, 2]
                    .map(|j| (i + j) % 3)
                    .map(|i| get_position_from_index(context, vertices[i]));
                let v = [p[0] - p[1], p[2] - p[1]]
                    .map(|v| v - ((n.dot(v)) * n))
                    .map(Vec3::normalized_or_zero);

                // weight contribution by the angle
                // between the two edge vectors
                let cos = v[0].dot(v[1]).clamp(-1f32, 1f32);
                let angle = O::acos(cos);

                let t = [info.tangent.s, info.tangent.t]
                    .map(|t| t - (n.dot(t) * n))
                    .map(Vec3::normalized_or_zero)
                    .map(|t| angle * t);
                let t_mag = [info.tangent.s_magnitude, info.tangent.t_magnitude].map(|t| angle * t);

                res.s = res.s + t[0];
                res.t = res.t + t[1];
                res.s_magnitude += t_mag[0];
                res.t_magnitude += t_mag[1];

                (angle_sum + angle, res)
            },
        );

    // normalize
    res.s.normalize_or_zero();
    res.t.normalize_or_zero();
    if angle_sum > 0f32 {
        res.s_magnitude /= angle_sum;
        res.t_magnitude /= angle_sum;
    }

    res
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Edge {
    i0: FaceVertex,
    i1: FaceVertex,
    f: usize,
}

/// Populates [`face_neighbors`](TriangleInfo::face_neighbors) based on matching edges.
fn build_neighbors<O: Ops>(triangles: &mut [TriangleInfo<O>], vertices: &[FaceVertex]) {
    let (vertices, _) = vertices.split_at(triangles.len() * 3);

    // build array of edges
    let mut edges = vertices
        .chunks_exact(3)
        .enumerate()
        .flat_map(|(f, chunk)| {
            chunk
                .iter()
                .zip(chunk.iter().cycle().skip(1))
                .take(3)
                .map(move |(a, b)| (f, a.min(b), a.max(b)))
        })
        .map(|(f, &i0, &i1)| Edge { i0, i1, f })
        .collect::<Vec<_>>();

    // Sort over all edges by i0, i1, then f.
    // This is the pricy one.
    quick_sort_edges(&mut edges);

    // pair up, adjacent triangles
    for (index, a) in edges.iter().enumerate() {
        // resolve index ordering and edge_num
        let (n_a, (i0_a, i1_a)) = get_edge(&vertices[a.f * 3..][..3], a.i0, a.i1).unwrap();

        if triangles[a.f].face_neighbors[n_a].is_some() {
            continue;
        }

        // get true index ordering
        let found = edges
            .iter()
            .skip(index + 1)
            // If `edges` is improperly sorted, neighbors may not be found for
            // faces with the largest `i0`.
            .take_while(|b| (a.i0, a.i1) == (b.i0, b.i1))
            .find_map(|b| {
                // flip i0_B and i1_B
                // resolve index ordering and edge_num
                let (n_b, (i1_b, i0_b)) = get_edge(&vertices[b.f * 3..][..3], b.i0, b.i1).unwrap();
                let unassigned_b = triangles[b.f].face_neighbors[n_b].is_none();

                ((i0_a, i1_a) == (i0_b, i1_b) && unassigned_b).then_some((b, n_b))
            });

        let Some((b, n_b)) = found else {
            continue;
        };

        triangles[a.f].face_neighbors[n_a] = Some(b.f);
        triangles[b.f].face_neighbors[n_b] = Some(a.f);
    }
}

/// Finds the index of the edge `(i0_in, i1_in)` within `indices`, additionally
/// returning `i0_in` and `i1_in` in the same order as they are stored within `indices`.
fn get_edge(
    indices: &[FaceVertex],
    i0: FaceVertex,
    i1: FaceVertex,
) -> Option<(usize, (FaceVertex, FaceVertex))> {
    let iter = indices.iter().copied();
    iter.clone()
        .zip(iter.cycle().skip(1))
        .enumerate()
        .find(|&(_, (a, b))| (a.min(b), a.max(b)) == (i0.min(i1), i0.max(i1)))
}

/// Populate tangent space values for degenerate triangles from adjacent "good" triangles.
fn generate_tangent_spaces_for_degenerate_triangles<O: Ops>(
    // Using `impl Copy` to highlight this function doesn't interact with tangent
    // space values.
    tangent_spaces: &mut [impl Copy],
    triangles_good: &[TriangleInfo<O>],
    triangle_degenerate: &[TriangleInfo<O>],
    triangle_vertices: &[FaceVertex],
) {
    let (vertices_good, vertices_degenerate) = triangle_vertices.split_at(triangles_good.len() * 3);

    // deal with degenerate triangles
    // punishment for degenerate triangles is O(N^2)
    triangle_degenerate
        .iter()
        .zip(vertices_degenerate.chunks_exact(3))
        .filter(|(triangle, _chunk)| {
            // degenerate triangles on a quad with one good triangle are skipped
            // here but processed in the next loop
            !triangle.quad_with_one_degenerate_triangle
        })
        .flat_map(|(a, chunk)| chunk.iter().enumerate().map(move |(i, av)| (a, av, i)))
        .filter_map(|(a, av, i)| {
            // search through the good triangles
            triangles_good
                .iter()
                .zip(vertices_good.chunks_exact(3))
                .flat_map(|(b, chunk)| chunk.iter().enumerate().map(move |(j, bv)| (b, bv, j)))
                .find_map(|(b, bv, j)| (av == bv).then_some((a, i, b, j)))
        })
        .map(|(a, i, b, j)| {
            (
                a.tangent_spaces_offset + a.vertex_indices()[i] as usize,
                b.tangent_spaces_offset + b.vertex_indices()[j] as usize,
            )
        })
        .for_each(|(dst, src)| {
            tangent_spaces[dst] = tangent_spaces[src];
        });
}

/// Degenerate quads with one good triangle will be fixed by copying a space from
/// the good triangle to the coinciding vertex.
fn generate_tangent_spaces_for_partially_degenerate_quads<I: Geometry<O>, O: Ops>(
    // Using `impl Copy` to highlight this function doesn't interact with tangent
    // space values.
    tangent_spaces: &mut [impl Copy],
    triangle_info_list: &[TriangleInfo<O>],
    context: &I,
) {
    // deal with degenerate quads with one good triangle
    triangle_info_list
        .iter()
        // this triangle belongs to a quad where the
        // other triangle is degenerate
        .filter(|triangle_info| triangle_info.quad_with_one_degenerate_triangle)
        .map(|triangle_info| {
            let dst = triangle_info.missing_vertex.unwrap() as usize;

            let offset = triangle_info.tangent_spaces_offset;
            let face = triangle_info.original_face_index;
            let missing_position = context.position(face, dst);

            let src = triangle_info
                .vertex_indices()
                .iter()
                .find_map(|&vertex| {
                    let vertex = vertex as usize;
                    let source_position = context.position(face, vertex);
                    (source_position == missing_position).then_some(vertex)
                })
                .unwrap();

            (dst + offset, src + offset)
        })
        .for_each(|(dst, src)| {
            tangent_spaces[dst] = tangent_spaces[src];
        });
}

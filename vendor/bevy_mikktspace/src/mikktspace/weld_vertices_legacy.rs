//! Provides [`weld_vertices`]; a method to deduplicate [`FaceVertex`] values in
//! a list.
//! This implementation is overly complex and has poor `NaN` handling _deliberately_
//! to match the original C implementation.
//!
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

use alloc::vec::Vec;

use crate::{
    Geometry, Ops,
    math::Vec3,
    mikktspace::{
        face_vertex::FaceVertex, get_normal_from_index, get_position_from_index,
        get_texture_coordinate_from_index,
    },
};

pub(super) fn weld_vertices<I: Geometry<O>, O: Ops>(
    context: &I,
    triangle_vertices: &mut [FaceVertex],
) {
    // make bbox
    let Some((min, max)) = triangle_vertices
        .iter()
        .map(|&i| get_position_from_index(context, i))
        .fold(None, |state, v| {
            let (mut min, mut max) = state.unwrap_or((v, v));

            for c in 0..3 {
                min[c] = min[c].min(v[c]);
                max[c] = max[c].max(v[c]);
            }

            Some((min, max))
        })
    else {
        // If we cannot generate a bounding box then there are no vertices to weld.
        return;
    };

    let d = max - min;

    let c_max = if d.y > d.x && d.y > d.z {
        1
    } else if d.z > d.x {
        2
    } else {
        0
    };

    let mut temporary_vertices = triangle_vertices
        .iter()
        .map(|&v| get_position_from_index(context, v))
        .enumerate()
        .map(|(index, position)| TemporaryVertex {
            bucket: {
                const GROUPS: u16 = 2048;
                let t = (position[c_max] - min[c_max]) / d[c_max];
                let group = (GROUPS as f32 * t.clamp(0., 1.)) as u16;
                group.clamp(0, GROUPS - 1)
            },
            position,
            original_index: index,
        })
        .collect::<Vec<_>>();

    temporary_vertices.sort_by_key(|v| v.bucket);

    for chunk in temporary_vertices.chunk_by_mut(|a, b| a.bucket == b.bucket) {
        merge_verts_fast(context, triangle_vertices, chunk);
    }
}

fn merge_verts_fast<I: Geometry<O>, O: Ops>(
    context: &I,
    vertices: &mut [FaceVertex],
    buffer: &mut [TemporaryVertex<O>],
) {
    // If there is only a single element (or no elements), merging is complete.
    if buffer.len() < 2 {
        return;
    }

    // make bbox
    let (min, max) = buffer
        .iter()
        .map(|t| t.position)
        .fold(None, |state, v| {
            let (mut min, mut max) = state.unwrap_or((v, v));

            for c in 0..3 {
                min[c] = min[c].min(v[c]);
                max[c] = max[c].max(v[c]);
            }

            Some((min, max))
        })
        .unwrap();

    let d = max - min;

    let c = if d.y > d.x && d.y > d.z {
        1
    } else if d.z > d.x {
        2
    } else {
        0
    };

    let sep = 0.5f32 * (max[c] + min[c]);

    // stop if all vertices are NaNs
    if !sep.is_finite() {
        return;
    }

    // terminate recursion when the separation/average value
    // is no longer strictly between fMin and fMax values.
    if !(min[c] < sep && sep < max[c]) {
        // complete the weld
        for (l, v_a) in buffer.iter().enumerate() {
            let i = v_a.original_index;
            let index = vertices[i];

            let a = (
                v_a.position,
                get_normal_from_index(context, index),
                get_texture_coordinate_from_index(context, index),
            );

            let j = buffer.iter().take(l).find_map(|v_b| {
                let j = v_b.original_index;
                let index = vertices[j];

                let b = (
                    v_b.position,
                    get_normal_from_index(context, index),
                    get_texture_coordinate_from_index(context, index),
                );

                (a == b).then_some(j)
            });

            // merge if previously found
            if let Some(j) = j {
                vertices[i] = vertices[j];
            }
        }

        return;
    }

    // separate into vertices either left or right of the separation plane by
    // swapping pairs.
    let mut unsorted = 0..buffer.len();
    while unsorted.len() >= 2 {
        let a = unsorted.find(|&i| buffer[i].position[c] >= sep);
        let b = (&mut unsorted).rev().find(|&i| buffer[i].position[c] < sep);

        unsorted = match (a, b) {
            (Some(a), Some(b)) => {
                buffer.swap(a, b);
                (a + 1)..b
            }
            (None, Some(b)) => unsorted.start..(b + 1),
            (Some(a), None) => a..(a + 1),
            (None, None) => unsorted,
        };
    }

    // separation above only operates on pairs, so there may be a single unsorted
    // entry left.
    let partition = if !unsorted.is_empty() && buffer[unsorted.start].position[c] < sep {
        unsorted.start + 1
    } else {
        unsorted.start
    };

    // merge vertices in each separated buffer
    let (lesser, greater) = buffer.split_at_mut(partition);
    merge_verts_fast(context, vertices, lesser);
    merge_verts_fast(context, vertices, greater);
}

struct TemporaryVertex<O: Ops> {
    position: Vec3<O>,
    original_index: usize,
    bucket: u16,
}

impl<O: Ops> Copy for TemporaryVertex<O> {}

impl<O: Ops> Clone for TemporaryVertex<O> {
    fn clone(&self) -> Self {
        *self
    }
}

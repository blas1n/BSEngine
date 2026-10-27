//! Provides [`weld_vertices`]; a method to deduplicate [`FaceVertex`] values in
//! a list.
//! This implementation is based on [`BTreeMap`] and provides good performance
//! characteristics and predicable behavior.
//! When presented with two identical vertices, the lowest index value will be used.
//! This essentially means the first vertices on the first faces will be kept as
//! the originals, while later ones will be replaced.

use alloc::collections::BTreeMap;

use crate::{
    Geometry, Ops,
    mikktspace::{
        FaceVertex, get_normal_from_index, get_position_from_index,
        get_texture_coordinate_from_index,
    },
};

/// De-duplicate [`FaceVertex`]'s by comparing the position, normal, and texture
/// coordinate values from the provided [context](Geometry).
pub(super) fn weld_vertices<I: Geometry<O>, O: Ops>(
    context: &I,
    triangle_vertices: &mut [FaceVertex],
) {
    let mut map = BTreeMap::<Key, FaceVertex>::new();

    for index in triangle_vertices {
        *index = *map.entry(Key::new(context, *index)).or_insert(*index);
    }
}

/// Compares two vertices using [`f32::total_cmp`] for each component of its position,
/// normal, and texture coordinate.
#[derive(Clone, Copy)]
struct Key([f32; 3 + 3 + 2]);

impl Key {
    fn new<I: Geometry<O>, O: Ops>(context: &I, index: FaceVertex) -> Self {
        let p: [f32; 3] = get_position_from_index(context, index).into();
        let n: [f32; 3] = get_normal_from_index(context, index).into();
        let t: [f32; 2] = get_texture_coordinate_from_index(context, index);

        Self([p[0], p[1], p[2], n[0], n[1], n[2], t[0], t[1]])
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        use core::cmp::Ordering::Equal;

        self.0
            .iter()
            .zip(other.0)
            .map(|(a, b)| a.total_cmp(&b))
            .find(|o| *o != Equal)
            .unwrap_or(Equal)
    }
}

// Defer to the `Ord` implementation to ensure `total_cmp` is used for all methods.

impl Eq for Key {}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == core::cmp::Ordering::Equal
    }
}

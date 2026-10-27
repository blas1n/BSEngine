//! This is a deliberately _broken_ implementation of quick-sort.
//! It exists to provide _identical_ sorting to the original C implementation
//! of `mikktspace` when sorting edges.
//!
//! Details on how this implementation is broken can be found
//! [on GitHub](https://github.com/mmikk/MikkTSpace/issues/5) and in the documentation
//! of [`quick_sort_edges`].
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

use super::Edge;

/// Sorts [`Edge`]s while preserving an off-by-one error in the original C implementation.
/// The observable effect of this bug is that `edges` will be _mostly_ sorted depending
/// on the largest value of [`i0`](Edge::i0).
///
/// 1. The list is quick-sorted by [`i0`](Edge::i0) with an _unstable_ implementation.
/// 2. All edges with an [`i0`](Edge::i0) _not_ equal to the largest will be correctly
///    sorted.
///    This section will be equivalent to the built in `[Edge]::sort` result.
/// 3. All edges with [`i0`](Edge::i0) equal to the largest will be incorrectly
///    sorted into a pseudo-random state.
///
/// Because the first step relies on an unstable sort, the final state of the
/// "bad" section of `edges` is deterministic but incoherent.
pub(super) fn quick_sort_edges(edges: &mut [Edge]) {
    // This initial sort is valid.
    // Unfortunately, because it is an unstable sort _and_ the subsequent sorts
    // are wrong, we cannot use [T]::sort_by_key even for this first step.
    quick_sort_by_key_with_seed(edges, |e| e.i0, INTERNAL_RND_SORT_SEED);
    #[cfg(mikktspace_rs_more_assertions)]
    debug_assert!(edges.is_sorted_by_key(|e| e.i0));

    // The last chunk is incorrectly sorted in the C implementation.
    let last_chunk_len = edges
        .chunk_by(|a, b| a.i0 == b.i0)
        .next_back()
        .map(<[_]>::len)
        .unwrap_or(edges.len());

    // We split `edges` into a section that _can_ be correctly sorted (`good`),
    // and a section which must be incorrectly sorted to match the original C
    // implementation (`bad`).
    let (good, bad) = edges.split_at_mut(edges.len() - last_chunk_len);

    // As this is a sort of an `Eq` type, instability is not a concern
    good.sort_unstable();

    // The off-by-one is replicated here with the `skip(1)` adapter.
    for chunk in bad
        // Within `bad`, all `i0` values are identical
        .chunk_by_mut(|a, b| a.i1 == b.i1)
        .rev()
        .skip(1)
    {
        quick_sort_by_key_with_seed(chunk, |e| e.f, INTERNAL_RND_SORT_SEED);

        // Each `chunk` within `bad` will be correctly sorted.
        // But this is unhelpful, as chunks themselves will be in an unsorted
        // order.
        #[cfg(mikktspace_rs_more_assertions)]
        debug_assert!(chunk.is_sorted());
    }
}

/// This implementation of quick sort is valid.
/// _However_, because it is an unstable sort, and it is improperly used for sorting
/// [`Edge`]'s across multiple fields, the state it leaves the buffer in is required
/// for byte-compatibility with the C implementation.
fn quick_sort_by_key_with_seed<T, K: Ord>(sort_buffer: &mut [T], key: fn(&T) -> K, seed: u32) {
    if sort_buffer.len() <= 2 {
        sort_buffer.sort_unstable_by_key(key);
        return;
    }

    let seed = {
        let t = seed & 31;
        let t = seed.wrapping_shl(t) | seed.wrapping_shr(32_u32.wrapping_sub(t));
        seed.wrapping_add(t).wrapping_add(3)
    };

    let pivot_index = seed.wrapping_rem(sort_buffer.len() as u32) as usize;
    let pivot = key(&sort_buffer[pivot_index]);

    let (mut a, mut b) = (0, sort_buffer.len().saturating_sub(1));
    while a <= b {
        a = (a..sort_buffer.len())
            .find(|&left| key(&sort_buffer[left]) >= pivot)
            .unwrap();

        b = (0..=b)
            .rev()
            .find(|&right| key(&sort_buffer[right]) <= pivot)
            .unwrap();

        if a <= b {
            sort_buffer.swap(a, b);
            a = a.saturating_add(1);
            b = b.saturating_sub(1);
        }
    }

    debug_assert!(b < a);

    let (lesser, rest) = sort_buffer.split_at_mut(b + 1);
    let (_sorted, greater) = rest.split_at_mut(a - lesser.len());

    // everything in lesser should be less than or equal to sorted, and likewise
    // for sorted and greater.
    #[cfg(debug_assertions)]
    if let (Some(x), Some(y)) = (_sorted.first(), _sorted.last()) {
        debug_assert!(lesser.iter().all(|t| key(t) <= key(x)));
        debug_assert!(greater.iter().all(|t| key(t) >= key(y)));
    } else {
        debug_assert!(_sorted.is_empty());
        debug_assert!(lesser.iter().all(|t| key(t) <= pivot));
        debug_assert!(greater.iter().all(|t| key(t) >= pivot));
    }

    quick_sort_by_key_with_seed(lesser, key, seed);
    #[cfg(mikktspace_rs_more_assertions)]
    debug_assert!(_sorted.is_sorted_by_key(key));
    quick_sort_by_key_with_seed(greater, key, seed);
}

const INTERNAL_RND_SORT_SEED: u32 = 39871946;

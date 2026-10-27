//! Implements edge sorting using `[T]::sort`.

use super::Edge;

/// Correctly sorts [`Edge`]s using a built-in sorting algorithm.
pub(super) fn quick_sort_edges(edges: &mut [Edge]) {
    edges.sort_unstable();
}

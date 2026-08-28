//! Pure hysteresis LOD (level-of-detail) level selection.
//!
//! Picking a LOD level from a raw distance-vs-threshold comparison alone
//! flickers whenever the camera sits almost exactly at a threshold —
//! jitter, orbiting, or a slowly approaching camera all cross a single
//! boundary many times per second. [`select_lod_level`] avoids that by
//! taking the *current* level as an input and only switching once the
//! distance has clearly crossed into the next level's territory, not
//! merely touched the boundary.

/// Picks which LOD level should be active given the entity's current level
/// (`None` = LOD 0, `Some(i)` = `mesh_ids[i]`, matching `LodLevels`'
/// own field), the current camera distance, the per-transition switch
/// distances, and a hysteresis band width.
///
/// A raw distance-vs-threshold check alone would flicker whenever the
/// entity sits almost exactly at a threshold (camera jitter, orbiting,
/// etc.) -- moving `hysteresis_band` around each threshold makes the
/// entity only switch once it has clearly crossed into the next level's
/// territory, not merely touched the boundary. This is why the *current*
/// level is an input, not just distance: the function needs to know which
/// side of the (now-widened) boundary it's already committed to.
pub fn select_lod_level(
    current: Option<usize>,
    distance: f32,
    switch_distances: &[f32],
    hysteresis_band: f32,
) -> Option<usize> {
    if switch_distances.is_empty() {
        return None;
    }
    let half_band = hysteresis_band / 2.0;
    let current_level = current.map(|i| i + 1).unwrap_or(0); // 0 = LOD0, 1 = LOD1 (mesh_ids[0]), ...

    // Consider moving up a level (farther away) first, then down.
    let mut level = current_level;
    while level < switch_distances.len() && distance > switch_distances[level] + half_band {
        level += 1;
    }
    while level > 0 && distance < switch_distances[level - 1] - half_band {
        level -= 1;
    }

    if level == 0 {
        None
    } else {
        Some(level - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_at_lod_zero_within_the_first_switch_distance() {
        let distances = [10.0, 30.0];
        assert_eq!(select_lod_level(None, 5.0, &distances, 2.0), None);
    }

    #[test]
    fn switches_to_lod_one_once_past_the_first_threshold() {
        let distances = [10.0, 30.0];
        assert_eq!(select_lod_level(None, 12.0, &distances, 2.0), Some(0));
    }

    #[test]
    fn switches_to_lod_two_once_past_the_second_threshold() {
        let distances = [10.0, 30.0];
        assert_eq!(select_lod_level(Some(0), 32.0, &distances, 2.0), Some(1));
    }

    #[test]
    fn switches_back_down_once_distance_drops_well_below_a_threshold() {
        let distances = [10.0, 30.0];
        // Currently at LOD 1 (index 0), moving back inside the hysteresis
        // band's lower edge (10.0 - 2.0/2 = 9.0) should drop to LOD 0 (None).
        assert_eq!(select_lod_level(Some(0), 8.0, &distances, 2.0), None);
    }

    #[test]
    fn jitter_within_the_hysteresis_band_does_not_flip_the_level() {
        let distances = [10.0, 30.0];
        // At LOD 0 (None), a distance of 10.5 is past the raw 10.0 threshold
        // but still within the 2.0-wide band around it (9.0..=11.0) -- must
        // NOT switch, since jitter right at a boundary is exactly what
        // hysteresis exists to suppress.
        assert_eq!(select_lod_level(None, 10.5, &distances, 2.0), None);
        // Symmetrically, already at LOD 1 (index 0) and drifting to 9.5
        // (still within the band) must stay at LOD 1, not drop back.
        assert_eq!(select_lod_level(Some(0), 9.5, &distances, 2.0), Some(0));
    }

    #[test]
    fn empty_switch_distances_always_selects_lod_zero() {
        assert_eq!(select_lod_level(None, 1000.0, &[], 2.0), None);
    }
}

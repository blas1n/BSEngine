//! Deciding whether a streamed scene should be in the world, from how far the
//! camera is from it.
//!
//! Pure, and separate from the system that acts on it, the same way
//! `bsengine_render::lod` is separate from `render_frame`: the rule is a
//! function of a distance and a threshold, and it is worth being able to ask it
//! without a world to put an answer into.
//!
//! # What the three engines do
//!
//! Only Unreal has this built in. Its two spellings are a *streaming volume*
//! (a box you place; entering it loads the level) and a *distance* (a cell has
//! a position and a loading range, which is what World Partition uses). Unity
//! has additive loading and leaves the triggering to you; Godot has neither.
//!
//! The distance form is the one taken here. A volume has to be authored and
//! kept in step with the chunk it guards; a radius travels with the chunk, and
//! it degenerates to the same thing for the common case of a roughly round
//! piece of level.

/// Whether a streamed scene should be loaded right now.
///
/// `loaded` is the state it is already in, and that is what makes this
/// hysteresis rather than a threshold: the band is applied to whichever edge
/// the current state is committed to, so a camera sitting exactly on the
/// boundary keeps its answer instead of flipping.
///
/// ⚠️ Without the band this thrashes, and thrashing is expensive in a way
/// LOD-level flicker is not: each load spends about 4.8µs per entity reading,
/// parsing and spawning, and each unload despawns all of it again. A camera
/// hovering on the boundary of a 1,200-entity chunk would pay ~6ms every frame,
/// alternately building and destroying the same scene.
///
/// Mirrors `bsengine_render::lod::select_lod_level`, which takes the current
/// level for the same reason and widens its thresholds by the same half-band.
pub fn should_be_loaded(
    loaded: bool,
    distance: f32,
    load_distance: f32,
    hysteresis_band: f32,
) -> bool {
    // A negative band would invert the two edges and make the thing it exists
    // to prevent worse than having none at all.
    let half = (hysteresis_band.max(0.0)) / 2.0;
    if loaded {
        // Already in: stay until clearly outside.
        distance <= load_distance + half
    } else {
        // Not in yet: come in only once clearly inside.
        distance < load_distance - half
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chunk_comes_in_when_the_camera_is_clearly_inside() {
        assert!(should_be_loaded(false, 40.0, 100.0, 10.0));
        assert!(!should_be_loaded(false, 160.0, 100.0, 10.0));
    }

    #[test]
    fn a_chunk_goes_out_when_the_camera_is_clearly_outside() {
        assert!(!should_be_loaded(true, 160.0, 100.0, 10.0));
        assert!(should_be_loaded(true, 40.0, 100.0, 10.0));
    }

    #[test]
    fn the_band_is_where_the_current_state_wins() {
        // ⚠️ The whole point, and the only region where the two answers differ.
        // A test that only checked "near loads, far unloads" would pass with
        // the band deleted -- there would be nothing left to observe.
        for distance in [96.0, 100.0, 104.0] {
            assert!(
                should_be_loaded(true, distance, 100.0, 10.0),
                "a loaded chunk stays loaded inside the band at {distance}"
            );
            assert!(
                !should_be_loaded(false, distance, 100.0, 10.0),
                "an unloaded chunk stays unloaded inside the band at {distance}"
            );
        }
    }

    #[test]
    fn a_band_of_zero_is_a_plain_threshold() {
        // Authored that way it thrashes, which is the author's business; what
        // it must not do is behave as though the band were still there.
        assert!(should_be_loaded(false, 99.0, 100.0, 0.0));
        assert!(!should_be_loaded(true, 101.0, 100.0, 0.0));
    }

    #[test]
    fn a_negative_band_is_treated_as_none_rather_than_inverted() {
        // ⚠️ Measured at the threshold itself, which is the only distance that
        // tells the two apart. Negated, the edges swap: an unloaded chunk comes
        // in at anything under 125 and a loaded one leaves at anything over 75,
        // so a chunk sitting at 100 loads, unloads, loads... every frame --
        // exactly the thrash the band exists to prevent. Clamped, 100 is
        // stable whichever state it is in.
        //
        // An earlier version of this test used 99 and 101, where both spellings
        // agree, and passed with the clamp deleted.
        assert!(!should_be_loaded(false, 100.0, 100.0, -50.0));
        assert!(should_be_loaded(true, 100.0, 100.0, -50.0));
    }
}

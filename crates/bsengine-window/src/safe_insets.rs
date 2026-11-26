//! The safe area as insets, from the two rectangles winit gives on iOS.
//!
//! winit has no call for the insets themselves. On iOS its `outer_position`
//! and `outer_size` are the window's frame and its `inner_position` and
//! `inner_size` the frame less the UIWindow's `safeAreaInsets` (the status
//! bar or Dynamic Island, the home indicator, a landscape notch), both in
//! physical pixels in screen space. The insets are the gaps between them.
//! Plain arithmetic, so it is here rather than behind `cfg(target_os =
//! "ios")`, where no host test could reach it.

/// A rectangle as winit reports one: position, then size, in physical pixels.
pub type Frame = ((i32, i32), (u32, u32));

/// How far the safe area `inner` sits inside the window `outer` from each
/// edge: `(left, top, right, bottom)`, the shape of
/// [`bsengine_core::SafeArea`]'s insets.
///
/// Never negative. winit rounds each of the four numbers to pixels on its
/// own, so a side with no inset can come out a pixel short of the window's
/// edge, and an inner rectangle reaching past it (a rotation caught half way)
/// is no inset either.
pub fn between(outer: Frame, inner: Frame) -> (f32, f32, f32, f32) {
    let ((ox, oy), (ow, oh)) = outer;
    let ((ix, iy), (iw, ih)) = inner;
    let left = (ix - ox).max(0) as f32;
    let top = (iy - oy).max(0) as f32;
    let right = ((ox + ow as i32) - (ix + iw as i32)).max(0) as f32;
    let bottom = ((oy + oh as i32) - (iy + ih as i32)).max(0) as f32;
    (left, top, right, bottom)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An iPhone with a Dynamic Island held upright, at 3x: 402x874 points
    /// with 62 points over the top and 34 under the bottom for the home
    /// indicator. Each edge is checked on its own, so swapping two of them
    /// (top for bottom, say) fails here.
    #[test]
    fn a_portrait_phone_is_covered_at_the_top_and_bottom() {
        let outer = ((0, 0), (1206, 2622));
        let inner = ((0, 186), (1206, 2622 - 186 - 102));
        assert_eq!(between(outer, inner), (0.0, 186.0, 0.0, 102.0));
    }

    /// Turned on its side the island is at the left, and the home indicator
    /// still at the bottom; the right edge gets the left's inset as well
    /// (UIKit keeps landscape symmetric).
    #[test]
    fn a_landscape_phone_is_covered_at_the_sides() {
        let outer = ((0, 0), (2622, 1206));
        let inner = ((186, 0), (2622 - 2 * 186, 1206 - 63));
        assert_eq!(between(outer, inner), (186.0, 0.0, 186.0, 63.0));
    }

    /// The window need not sit at the screen's origin (an iPad's Split
    /// View): the insets are measured from the window, not from the screen.
    #[test]
    fn a_window_away_from_the_origin_is_measured_from_its_own_edges() {
        let outer = ((500, 40), (800, 1000));
        let inner = ((500, 100), (800, 900));
        assert_eq!(between(outer, inner), (0.0, 60.0, 0.0, 40.0));
    }

    /// A safe area that is the whole window -- an iPhone without a notch in
    /// landscape, where the status bar is hidden -- is no insets at all, and
    /// a pixel of rounding past an edge is not a negative one.
    #[test]
    fn rounding_past_an_edge_is_no_inset() {
        let outer = ((0, 0), (1334, 750));
        assert_eq!(between(outer, outer), (0.0, 0.0, 0.0, 0.0));
        let a_pixel_over = ((0, -1), (1335, 752));
        assert_eq!(between(outer, a_pixel_over), (0.0, 0.0, 0.0, 0.0));
    }
}

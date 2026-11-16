use bevy_ecs::prelude::Resource;

/// The part of the window nothing covers: how far the status bar, the
/// navigation bar and a camera cutout reach into it from each edge, in window
/// pixels. Zero on desktop and in a browser; on Android the window layer reads
/// it from the system (`WindowInsets`) when the window appears and whenever it
/// is resized, rotation included.
///
/// Unity's `Screen.safeArea`, Godot's `DisplayServer.get_display_safe_area()`
/// and Unreal's `GetSafeZonePadding` are the same numbers. None of the three
/// applies them to UI by itself (Unreal does inside a `SafeZone` widget); here
/// the HUD lines always sit inside them, and anchored root widgets do too
/// unless the project sets `[ui] safe_area = false` -- see
/// [`SafeArea::applies_to_ui`]. That default differs from the reference
/// engines on purpose: a desktop window's insets are zero, so it changes no
/// existing game, and on a phone it keeps a game that did nothing about the
/// status bar from drawing its UI under it.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct SafeArea {
    /// Pixels covered at the left edge.
    pub left: f32,
    /// Pixels covered at the top edge (status bar, cutout).
    pub top: f32,
    /// Pixels covered at the right edge.
    pub right: f32,
    /// Pixels covered at the bottom edge (navigation bar).
    pub bottom: f32,
    /// Whether anchored root UI widgets resolve against the safe area rather
    /// than the whole window: `project.toml`'s `[ui] safe_area`, on by default.
    pub applies_to_ui: bool,
}

impl Default for SafeArea {
    fn default() -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            applies_to_ui: true,
        }
    }
}

impl SafeArea {
    /// The safe area as `(x, y, width, height)` in a window of this size --
    /// Unity's `Screen.safeArea` shape. Never negative: insets larger than the
    /// window (a window being resized past them) leave an empty area rather
    /// than one with a negative size.
    pub fn rect(&self, window_w: f32, window_h: f32) -> (f32, f32, f32, f32) {
        let x = self.left.min(window_w);
        let y = self.top.min(window_h);
        let w = (window_w - self.left - self.right).max(0.0);
        let h = (window_h - self.top - self.bottom).max(0.0);
        (x, y, w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rect_is_the_window_less_its_insets() {
        let area = SafeArea {
            left: 10.0,
            top: 128.0,
            right: 20.0,
            bottom: 48.0,
            applies_to_ui: true,
        };
        assert_eq!(area.rect(1080.0, 2400.0), (10.0, 128.0, 1050.0, 2224.0));
        assert_eq!(
            SafeArea::default().rect(1280.0, 720.0),
            (0.0, 0.0, 1280.0, 720.0)
        );
    }

    #[test]
    fn insets_larger_than_the_window_leave_an_empty_area() {
        let area = SafeArea {
            top: 500.0,
            bottom: 500.0,
            ..SafeArea::default()
        };
        assert_eq!(area.rect(800.0, 600.0), (0.0, 500.0, 800.0, 0.0));
    }
}

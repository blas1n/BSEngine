use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::Resource;

/// Where a widget attaches to the screen, in normalised 0..1 coordinates.
///
/// `(0, 0)` is the top-left corner and `(1, 1)` the bottom-right, which is the
/// same convention Unity's `RectTransform`, Unreal's UMG anchors and Godot's
/// `Control` anchors all use. Following them is deliberate: a UI author
/// arriving from any of the three already knows what these numbers mean.
///
/// # Point anchors and stretch anchors
///
/// When `min` equals `max` on an axis, the anchor is a **point**: the widget's
/// offset is a position relative to it and its size is used literally. A
/// bottom-right anchor with `x = -110, width = 100` sits 110px in from the
/// right edge and is 100px wide, at any resolution.
///
/// When they differ, the anchor is a **stretch**: the widget spans that
/// fraction of the screen, the offset insets its near edge, and the size is an
/// *adjustment* to the span rather than the span itself. A horizontal stretch
/// with `x = 20, width = -40` leaves a 20px margin on both sides.
///
/// # Why this is backwards compatible by construction
///
/// The default is `(0, 0, 0, 0)` — a point anchor at the top-left. That is
/// exactly what absolute pixel coordinates already meant, so every widget
/// written before anchors existed resolves to the position it always had. The
/// compatibility is structural, not a special case in the resolver.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiAnchor {
    /// Left edge, 0.0 = screen left, 1.0 = screen right.
    pub min_x: f32,
    /// Top edge, 0.0 = screen top, 1.0 = screen bottom.
    pub min_y: f32,
    /// Right edge; equal to `min_x` for a point anchor.
    pub max_x: f32,
    /// Bottom edge; equal to `min_y` for a point anchor.
    pub max_y: f32,
}

impl Default for UiAnchor {
    fn default() -> Self {
        Self::TOP_LEFT
    }
}

impl UiAnchor {
    /// The top-left point anchor: what absolute pixel coordinates have always
    /// meant here.
    pub const TOP_LEFT: Self = Self {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 0.0,
        max_y: 0.0,
    };

    /// A point anchor at the given normalised position.
    pub fn point(x: f32, y: f32) -> Self {
        Self {
            min_x: x,
            min_y: y,
            max_x: x,
            max_y: y,
        }
    }

    /// Whether this anchor stretches horizontally.
    pub fn stretches_x(&self) -> bool {
        self.min_x != self.max_x
    }

    /// Whether this anchor stretches vertically.
    pub fn stretches_y(&self) -> bool {
        self.min_y != self.max_y
    }

    /// Resolves this anchor plus a widget's offsets into screen pixels.
    ///
    /// Returns `(x, y, width, height)`. Pure: no GPU, no window, no egui — it
    /// is arithmetic on the numbers above, which is what lets every anchoring
    /// rule be tested on a machine with no display.
    ///
    /// The two axes are resolved independently, so a horizontal stretch with a
    /// vertical point anchor behaves the way both halves say it should.
    pub fn resolve(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        screen_w: f32,
        screen_h: f32,
    ) -> (f32, f32, f32, f32) {
        let (rx, rw) = Self::resolve_axis(self.min_x, self.max_x, x, width, screen_w);
        let (ry, rh) = Self::resolve_axis(self.min_y, self.max_y, y, height, screen_h);
        (rx, ry, rw, rh)
    }

    /// One axis of [`resolve`](Self::resolve).
    fn resolve_axis(min: f32, max: f32, offset: f32, size: f32, screen: f32) -> (f32, f32) {
        let near = min * screen;
        if min == max {
            // Point anchor: offset positions, size is literal.
            (near + offset, size)
        } else {
            // Stretch: offset insets the near edge, size adjusts the span.
            let span = (max - min) * screen;
            (near + offset, span + size)
        }
    }
}

/// A single immediate-mode UI element rendered by the HUD/UI system.
#[derive(Clone, Debug)]
pub enum UiWidget {
    /// A static or dynamic text label.
    Label {
        /// Unique widget identifier.
        id: String,
        /// Text content to display.
        text: String,
        /// X position, in screen-space pixels.
        x: f32,
        /// Y position, in screen-space pixels.
        y: f32,
        /// Font size, in pixels.
        font_size: f32,
        /// Where this widget attaches to the screen; the default is the
        /// top-left point anchor, which is what plain pixel coordinates
        /// have always meant.
        anchor: UiAnchor,
    },
    /// A clickable button.
    Button {
        /// Unique widget identifier.
        id: String,
        /// Text shown on the button.
        label: String,
        /// X position, in screen-space pixels.
        x: f32,
        /// Y position, in screen-space pixels.
        y: f32,
        /// Button width, in pixels.
        width: f32,
        /// Button height, in pixels.
        height: f32,
        /// Where this widget attaches to the screen; the default is the
        /// top-left point anchor, which is what plain pixel coordinates
        /// have always meant.
        anchor: UiAnchor,
    },
    /// A rectangular container with a title bar.
    Panel {
        /// Unique widget identifier.
        id: String,
        /// Text shown in the panel's title bar.
        title: String,
        /// X position, in screen-space pixels.
        x: f32,
        /// Y position, in screen-space pixels.
        y: f32,
        /// Panel width, in pixels.
        width: f32,
        /// Panel height, in pixels.
        height: f32,
        /// Where this widget attaches to the screen; the default is the
        /// top-left point anchor, which is what plain pixel coordinates
        /// have always meant.
        anchor: UiAnchor,
    },
    /// An editable single-line text field.
    TextInput {
        /// Unique widget identifier.
        id: String,
        /// Placeholder text shown when the field is empty.
        hint: String,
        /// X position, in screen-space pixels.
        x: f32,
        /// Y position, in screen-space pixels.
        y: f32,
        /// Field width, in pixels.
        width: f32,
        /// Where this widget attaches to the screen; the default is the
        /// top-left point anchor, which is what plain pixel coordinates
        /// have always meant.
        anchor: UiAnchor,
    },
    /// A texture displayed at a fixed screen position.
    Image {
        /// Unique widget identifier.
        id: String,
        /// Path to the texture asset to display.
        texture_path: String,
        /// X position, in screen-space pixels.
        x: f32,
        /// Y position, in screen-space pixels.
        y: f32,
        /// Image width, in pixels.
        width: f32,
        /// Image height, in pixels.
        height: f32,
        /// Where this widget attaches to the screen; the default is the
        /// top-left point anchor, which is what plain pixel coordinates
        /// have always meant.
        anchor: UiAnchor,
    },
    /// A horizontal fill bar, e.g. for health/progress display.
    ProgressBar {
        /// Unique widget identifier.
        id: String,
        /// X position, in screen-space pixels.
        x: f32,
        /// Y position, in screen-space pixels.
        y: f32,
        /// Bar width, in pixels.
        width: f32,
        /// Bar height, in pixels.
        height: f32,
        /// Fill fraction, expected in 0.0..=1.0 (egui clamps out-of-range values).
        fraction: f32,
        /// Where this widget attaches to the screen; the default is the
        /// top-left point anchor, which is what plain pixel coordinates
        /// have always meant.
        anchor: UiAnchor,
    },
}

impl UiWidget {
    /// This widget's anchor, mutably, regardless of its variant.
    pub fn anchor_mut(&mut self) -> &mut UiAnchor {
        match self {
            Self::Label { anchor, .. }
            | Self::Button { anchor, .. }
            | Self::Panel { anchor, .. }
            | Self::TextInput { anchor, .. }
            | Self::Image { anchor, .. }
            | Self::ProgressBar { anchor, .. } => anchor,
        }
    }

    /// Returns this widget's unique identifier, regardless of its variant.
    pub fn id(&self) -> &str {
        match self {
            Self::Label { id, .. }
            | Self::Button { id, .. }
            | Self::Panel { id, .. }
            | Self::TextInput { id, .. }
            | Self::Image { id, .. }
            | Self::ProgressBar { id, .. } => id,
        }
    }
}

/// Resource holding the current tree of immediate-mode UI widgets and their
/// per-frame interaction state.
#[derive(Resource, Default, Clone)]
pub struct UiState {
    /// All widgets currently registered for rendering.
    pub widgets: Vec<UiWidget>,
    /// Buttons whose click was registered this frame (cleared next render).
    pub clicked: HashSet<String>,
    /// Current text content for TextInput widgets, keyed by widget id.
    pub text_values: HashMap<String, String>,
}

impl UiState {
    /// Inserts a widget, or replaces the existing widget with the same id.
    pub fn set_widget(&mut self, widget: UiWidget) {
        let id = widget.id().to_string();
        if let Some(pos) = self.widgets.iter().position(|w| w.id() == id) {
            self.widgets[pos] = widget;
        } else {
            self.widgets.push(widget);
        }
    }

    /// Sets an existing widget's anchor. Returns whether the widget existed.
    ///
    /// Separate from the widget setters so those keep a usable arity: a
    /// script's `setButton(..., { anchor: "bottom-right" })` becomes two
    /// queued commands, applied in order, rather than five extra positional
    /// arguments on every setter.
    pub fn set_anchor(&mut self, id: &str, anchor: UiAnchor) -> bool {
        for widget in &mut self.widgets {
            if widget.id() == id {
                *widget.anchor_mut() = anchor;
                return true;
            }
        }
        false
    }

    /// Removes the widget with the given id, along with any stored text input value.
    pub fn remove_widget(&mut self, id: &str) {
        self.widgets.retain(|w| w.id() != id);
        self.text_values.remove(id);
    }

    /// Removes all widgets and clears click/text-input state.
    pub fn clear(&mut self) {
        self.widgets.clear();
        self.text_values.clear();
        self.clicked.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_widget_inserts_progress_bar() {
        let mut state = UiState::default();
        state.set_widget(UiWidget::ProgressBar {
            id: "hp".into(),
            x: 10.0,
            y: 10.0,
            width: 200.0,
            height: 20.0,
            fraction: 0.75,
            anchor: UiAnchor::default(),
        });
        assert_eq!(state.widgets.len(), 1);
        if let UiWidget::ProgressBar { fraction, .. } = &state.widgets[0] {
            assert_eq!(*fraction, 0.75);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn progress_bar_id_returns_its_id() {
        let widget = UiWidget::ProgressBar {
            id: "hp".into(),
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 10.0,
            fraction: 0.5,
            anchor: UiAnchor::default(),
        };
        assert_eq!(widget.id(), "hp");
    }

    #[test]
    fn set_widget_inserts_new() {
        let mut state = UiState::default();
        state.set_widget(UiWidget::Label {
            id: "lbl".into(),
            text: "Hello".into(),
            x: 0.0,
            y: 0.0,
            font_size: 16.0,
            anchor: UiAnchor::default(),
        });
        assert_eq!(state.widgets.len(), 1);
    }

    #[test]
    fn set_widget_replaces_existing() {
        let mut state = UiState::default();
        state.set_widget(UiWidget::Label {
            id: "lbl".into(),
            text: "A".into(),
            x: 0.0,
            y: 0.0,
            font_size: 16.0,
            anchor: UiAnchor::default(),
        });
        state.set_widget(UiWidget::Label {
            id: "lbl".into(),
            text: "B".into(),
            x: 0.0,
            y: 0.0,
            font_size: 16.0,
            anchor: UiAnchor::default(),
        });
        assert_eq!(state.widgets.len(), 1);
        if let UiWidget::Label { text, .. } = &state.widgets[0] {
            assert_eq!(text, "B");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn remove_widget_removes_it() {
        let mut state = UiState::default();
        state.set_widget(UiWidget::Button {
            id: "btn".into(),
            label: "Click".into(),
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 40.0,
            anchor: UiAnchor::default(),
        });
        state.remove_widget("btn");
        assert!(state.widgets.is_empty());
    }

    #[test]
    fn clear_empties_all() {
        let mut state = UiState::default();
        state.set_widget(UiWidget::Panel {
            id: "p".into(),
            title: "T".into(),
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 150.0,
            anchor: UiAnchor::default(),
        });
        state.clicked.insert("btn".into());
        state.text_values.insert("inp".into(), "val".into());
        state.clear();
        assert!(state.widgets.is_empty());
        assert!(state.clicked.is_empty());
        assert!(state.text_values.is_empty());
    }
}

#[cfg(test)]
mod anchor_tests {
    use super::*;

    /// A deliberately non-square screen.
    ///
    /// 1000x1000 would let a bug that swaps the x and y axes read as correct;
    /// 1920x1080 cannot.
    const W: f32 = 1920.0;
    const H: f32 = 1080.0;

    #[test]
    fn the_default_anchor_is_what_plain_pixels_always_meant() {
        // The compatibility claim, asserted rather than reasoned about: a
        // widget written before anchors existed must land exactly where it
        // always did, at any resolution.
        let a = UiAnchor::default();
        assert_eq!(
            a.resolve(30.0, 40.0, 100.0, 50.0, W, H),
            (30.0, 40.0, 100.0, 50.0)
        );
        assert_eq!(
            a.resolve(30.0, 40.0, 100.0, 50.0, 640.0, 480.0),
            (30.0, 40.0, 100.0, 50.0),
            "a top-left anchored widget does not move with the screen"
        );
    }

    #[test]
    fn a_bottom_right_anchor_tracks_the_corner() {
        let a = UiAnchor::point(1.0, 1.0);
        // 110 in from the right, 50 up from the bottom, 100x40.
        assert_eq!(
            a.resolve(-110.0, -50.0, 100.0, 40.0, W, H),
            (1810.0, 1030.0, 100.0, 40.0)
        );
        // Same offsets, smaller screen: still the same distance from the
        // corner, which is the entire point of anchoring.
        assert_eq!(
            a.resolve(-110.0, -50.0, 100.0, 40.0, 1280.0, 720.0),
            (1170.0, 670.0, 100.0, 40.0)
        );
    }

    #[test]
    fn a_centre_anchor_stays_centred() {
        let a = UiAnchor::point(0.5, 0.5);
        // Offsets of half the size centre a widget exactly.
        assert_eq!(
            a.resolve(-50.0, -20.0, 100.0, 40.0, W, H),
            (910.0, 520.0, 100.0, 40.0)
        );
        assert_eq!(
            a.resolve(-50.0, -20.0, 100.0, 40.0, 1280.0, 720.0),
            (590.0, 340.0, 100.0, 40.0)
        );
    }

    #[test]
    fn a_horizontal_stretch_leaves_the_offsets_as_margins() {
        let a = UiAnchor {
            min_x: 0.0,
            max_x: 1.0,
            min_y: 0.0,
            max_y: 0.0,
        };
        // 20px in from each side: x = 20, width = -40.
        let (x, y, w, h) = a.resolve(20.0, 8.0, -40.0, 24.0, W, H);
        assert_eq!((x, w), (20.0, 1880.0), "spans 20..1900 on a 1920 screen");
        assert_eq!(
            (y, h),
            (8.0, 24.0),
            "the vertical axis is a point anchor and must be untouched by the horizontal stretch"
        );

        let (x, _, w, _) = a.resolve(20.0, 8.0, -40.0, 24.0, 1280.0, 720.0);
        assert_eq!((x, w), (20.0, 1240.0), "the margins hold at another width");
    }

    #[test]
    fn a_full_stretch_fills_the_screen_minus_its_insets() {
        let a = UiAnchor {
            min_x: 0.0,
            max_x: 1.0,
            min_y: 0.0,
            max_y: 1.0,
        };
        assert_eq!(
            a.resolve(10.0, 20.0, -20.0, -40.0, W, H),
            (10.0, 20.0, 1900.0, 1040.0)
        );
    }

    #[test]
    fn the_two_axes_resolve_independently() {
        // Asymmetric on purpose: a bug that shares one axis's result with the
        // other, or swaps min_x with min_y, cannot survive this.
        let a = UiAnchor {
            min_x: 1.0,
            max_x: 1.0,
            min_y: 0.0,
            max_y: 1.0,
        };
        let (x, y, w, h) = a.resolve(-200.0, 30.0, 150.0, -60.0, W, H);
        assert_eq!(
            (x, w),
            (1720.0, 150.0),
            "x is a point anchor at the right edge"
        );
        assert_eq!(
            (y, h),
            (30.0, 1020.0),
            "y stretches the full height less 60"
        );
    }

    #[test]
    fn stretch_detection_is_per_axis() {
        let a = UiAnchor {
            min_x: 0.0,
            max_x: 1.0,
            min_y: 0.5,
            max_y: 0.5,
        };
        assert!(a.stretches_x());
        assert!(!a.stretches_y());
    }
}

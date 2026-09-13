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

/// Which way a [`UiWidget::Container`] stacks its children.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UiDirection {
    /// Left to right.
    #[default]
    Horizontal,
    /// Top to bottom.
    Vertical,
}

/// How a container places its children across the axis it does *not* stack
/// along.
///
/// Defaults to [`Stretch`](Self::Stretch), following the reference engines:
/// Unity's layout groups control the cross axis by default, Unreal's box slots
/// default to `Fill` alignment, and Godot's size flags default to `Fill`. A
/// child that wants to keep its own size opts out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UiAlign {
    /// Children span the container's cross axis.
    #[default]
    Stretch,
    /// Children keep their own size, packed against the near edge.
    Start,
    /// Children keep their own size, centred.
    Center,
    /// Children keep their own size, packed against the far edge.
    End,
}

/// A resolved on-screen rectangle, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiRect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
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
    /// A box that positions its children in a row or a column.
    ///
    /// Containers draw nothing themselves; they exist to give their children
    /// rectangles. What they do is the intersection of what Unity's layout
    /// groups, Unreal's `Horizontal`/`VerticalBox` and Godot's
    /// `HBox`/`VBoxContainer` all do: stack along one axis with a fixed gap
    /// between children, inset by a padding, and hand out leftover space to
    /// children that asked to fill.
    Container {
        /// Unique widget identifier. Children name this in their parent field.
        id: String,
        /// X position, in screen-space pixels.
        x: f32,
        /// Y position, in screen-space pixels.
        y: f32,
        /// Container width, in pixels.
        width: f32,
        /// Container height, in pixels.
        height: f32,
        /// Where this container attaches to the screen.
        anchor: UiAnchor,
        /// Which way children stack.
        direction: UiDirection,
        /// Gap between adjacent children, in pixels.
        spacing: f32,
        /// Inset on all four sides, in pixels.
        ///
        /// On the container rather than in a separate widget: Unity and Unreal
        /// both put padding on the box itself, and only Godot splits it into a
        /// `MarginContainer`.
        padding: f32,
        /// How children are placed across the non-stacking axis.
        align: UiAlign,
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
            | Self::Container { anchor, .. }
            | Self::Button { anchor, .. }
            | Self::Panel { anchor, .. }
            | Self::TextInput { anchor, .. }
            | Self::Image { anchor, .. }
            | Self::ProgressBar { anchor, .. } => anchor,
        }
    }

    /// This widget's anchor, regardless of its variant.
    pub fn anchor(&self) -> UiAnchor {
        match self {
            Self::Label { anchor, .. }
            | Self::Container { anchor, .. }
            | Self::Button { anchor, .. }
            | Self::Panel { anchor, .. }
            | Self::TextInput { anchor, .. }
            | Self::Image { anchor, .. }
            | Self::ProgressBar { anchor, .. } => *anchor,
        }
    }

    /// This widget's authored X offset.
    pub fn x(&self) -> f32 {
        match self {
            Self::Label { x, .. }
            | Self::Container { x, .. }
            | Self::Button { x, .. }
            | Self::Panel { x, .. }
            | Self::TextInput { x, .. }
            | Self::Image { x, .. }
            | Self::ProgressBar { x, .. } => *x,
        }
    }

    /// This widget's authored Y offset.
    pub fn y(&self) -> f32 {
        match self {
            Self::Label { y, .. }
            | Self::Container { y, .. }
            | Self::Button { y, .. }
            | Self::Panel { y, .. }
            | Self::TextInput { y, .. }
            | Self::Image { y, .. }
            | Self::ProgressBar { y, .. } => *y,
        }
    }

    /// This widget's authored width.
    ///
    /// Zero for a [`Label`](Self::Label), which sizes itself to its text and
    /// has no width to author. A label inside a container therefore occupies
    /// no space along a horizontal stack unless it is given a fill weight --
    /// the same thing Unity reports for a text element with no
    /// `LayoutElement`.
    pub fn width(&self) -> f32 {
        match self {
            Self::Container { width, .. }
            | Self::Button { width, .. }
            | Self::Panel { width, .. }
            | Self::TextInput { width, .. }
            | Self::Image { width, .. }
            | Self::ProgressBar { width, .. } => *width,
            Self::Label { .. } => 0.0,
        }
    }

    /// This widget's authored height.
    ///
    /// A [`Label`](Self::Label) reports its font size, which is the closest
    /// thing it has to an authored height, and a [`TextInput`](Self::TextInput)
    /// reports zero for the same reason [`width`](Self::width) does for labels.
    pub fn height(&self) -> f32 {
        match self {
            Self::Container { height, .. }
            | Self::Button { height, .. }
            | Self::Panel { height, .. }
            | Self::Image { height, .. }
            | Self::ProgressBar { height, .. } => *height,
            Self::Label { font_size, .. } => *font_size,
            Self::TextInput { .. } => 0.0,
        }
    }

    /// Returns this widget's unique identifier, regardless of its variant.
    pub fn id(&self) -> &str {
        match self {
            Self::Label { id, .. }
            | Self::Container { id, .. }
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
    /// Which container each widget sits in, keyed by child id.
    ///
    /// A side table rather than a field on all seven variants, and a *name*
    /// rather than a nested tree, on two existing precedents: anchoring is
    /// already applied by a separate op rather than by five more positional
    /// arguments on every setter, and scene entities already express hierarchy
    /// as a `parent:` name. A widget with no entry here is a root, which is
    /// what every widget was before containers existed.
    pub parents: HashMap<String, String>,
    /// Share of a container's leftover space each child claims, keyed by id.
    ///
    /// Zero — the default — means the child keeps its own size. Non-zero
    /// weights split whatever the fixed-size children left over, in
    /// proportion. This is Unity's `flexibleWidth`, Unreal's slot `Fill`
    /// weight and Godot's `stretch_ratio` under one name.
    pub fills: HashMap<String, f32>,
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

    /// Sets a widget's container. An empty `parent` detaches it to the root.
    ///
    /// Stored whether or not either widget exists yet, because scripts set
    /// properties in whatever order they like and a container is often
    /// declared after the children that name it. An entry naming a container
    /// that never appears leaves the child a root, which is what it would have
    /// been anyway.
    pub fn set_parent(&mut self, id: &str, parent: &str) {
        if parent.is_empty() {
            self.parents.remove(id);
        } else {
            self.parents.insert(id.to_string(), parent.to_string());
        }
    }

    /// Sets how much of a container's leftover space a child claims.
    pub fn set_fill(&mut self, id: &str, weight: f32) {
        if weight > 0.0 && weight.is_finite() {
            self.fills.insert(id.to_string(), weight);
        } else {
            self.fills.remove(id);
        }
    }

    /// Removes all widgets and clears click/text-input state.
    pub fn clear(&mut self) {
        self.widgets.clear();
        self.text_values.clear();
        self.clicked.clear();
        self.parents.clear();
        self.fills.clear();
    }

    /// Resolves every widget to a screen rectangle.
    ///
    /// One pass, returning one rectangle per widget id, rather than each draw
    /// site resolving its own anchor. Containers make that necessary — a
    /// child's position comes from its parent, not from its own coordinates —
    /// but it is worth having regardless: the placement rule now exists in
    /// exactly one place instead of once per widget kind at the draw site.
    ///
    /// Pure: no GPU, no window, no egui. Every layout rule is testable on a
    /// machine with no display, which is how [`UiAnchor::resolve`] is tested
    /// and for the same reason.
    pub fn layout(&self, screen_w: f32, screen_h: f32) -> HashMap<String, UiRect> {
        let mut out = HashMap::new();
        // Children grouped by parent, in declaration order. Declaration order
        // is the layout order: a script that calls `setButton` three times
        // gets those three buttons in that order, which is the only ordering a
        // caller can predict.
        let mut children: HashMap<&str, Vec<&UiWidget>> = HashMap::new();
        for w in &self.widgets {
            if let Some(parent) = self.effective_parent(w.id()) {
                children.entry(parent).or_default().push(w);
            }
        }
        for w in &self.widgets {
            if self.effective_parent(w.id()).is_none() {
                let (x, y, width, height) =
                    w.anchor()
                        .resolve(w.x(), w.y(), w.width(), w.height(), screen_w, screen_h);
                self.place(
                    w,
                    UiRect {
                        x,
                        y,
                        width,
                        height,
                    },
                    &children,
                    &mut out,
                );
            }
        }
        out
    }

    /// The container a widget sits in, or `None` if it is a root.
    ///
    /// `None` when the named parent does not exist, and `None` when following
    /// the chain revisits a widget already on it. A cycle is a script bug, but
    /// an unguarded one would hang the frame rather than report anything, so
    /// the cycle's members are treated as roots and still drawn. The prefab
    /// system guards its own nesting the same way.
    fn effective_parent(&self, id: &str) -> Option<&str> {
        let parent = self.parents.get(id)?.as_str();
        if !self.widgets.iter().any(|w| w.id() == parent) {
            return None;
        }
        let mut seen = vec![id];
        let mut cursor = parent;
        loop {
            if seen.contains(&cursor) {
                return None;
            }
            seen.push(cursor);
            match self.parents.get(cursor) {
                Some(next) if self.widgets.iter().any(|w| w.id() == next.as_str()) => {
                    cursor = next.as_str();
                }
                _ => return Some(parent),
            }
        }
    }

    /// Records `rect` for `widget`, then lays out its children inside it.
    fn place<'a>(
        &'a self,
        widget: &'a UiWidget,
        rect: UiRect,
        children: &HashMap<&'a str, Vec<&'a UiWidget>>,
        out: &mut HashMap<String, UiRect>,
    ) {
        out.insert(widget.id().to_string(), rect);
        let UiWidget::Container {
            direction,
            spacing,
            padding,
            align,
            ..
        } = widget
        else {
            return;
        };
        let Some(kids) = children.get(widget.id()) else {
            return;
        };
        let inner = UiRect {
            x: rect.x + padding,
            y: rect.y + padding,
            width: (rect.width - 2.0 * padding).max(0.0),
            height: (rect.height - 2.0 * padding).max(0.0),
        };
        let horizontal = *direction == UiDirection::Horizontal;
        let main_span = if horizontal {
            inner.width
        } else {
            inner.height
        };
        // Fixed children keep their own size; the rest split what is left in
        // proportion to their weights. Gaps come out of the budget first, so a
        // filling child never overruns the container by the space between its
        // siblings.
        let gaps = spacing * kids.len().saturating_sub(1) as f32;
        let fixed: f32 = kids
            .iter()
            .filter(|k| self.fill_of(k.id()) == 0.0)
            .map(|k| if horizontal { k.width() } else { k.height() })
            .sum();
        let total_weight: f32 = kids.iter().map(|k| self.fill_of(k.id())).sum();
        let leftover = (main_span - gaps - fixed).max(0.0);
        let mut cursor = if horizontal { inner.x } else { inner.y };
        for kid in kids {
            let weight = self.fill_of(kid.id());
            let own = if horizontal {
                kid.width()
            } else {
                kid.height()
            };
            let main = if weight > 0.0 && total_weight > 0.0 {
                leftover * (weight / total_weight)
            } else {
                own
            };
            let cross_span = if horizontal {
                inner.height
            } else {
                inner.width
            };
            let own_cross = if horizontal {
                kid.height()
            } else {
                kid.width()
            };
            let (cross_offset, cross) = match align {
                UiAlign::Stretch => (0.0, cross_span),
                UiAlign::Start => (0.0, own_cross),
                UiAlign::Center => (((cross_span - own_cross) / 2.0).max(0.0), own_cross),
                UiAlign::End => ((cross_span - own_cross).max(0.0), own_cross),
            };
            let kid_rect = if horizontal {
                UiRect {
                    x: cursor,
                    y: inner.y + cross_offset,
                    width: main,
                    height: cross,
                }
            } else {
                UiRect {
                    x: inner.x + cross_offset,
                    y: cursor,
                    width: cross,
                    height: main,
                }
            };
            cursor += main + spacing;
            self.place(kid, kid_rect, children, out);
        }
    }

    /// A widget's fill weight; zero means it keeps its own size.
    fn fill_of(&self, id: &str) -> f32 {
        self.fills.get(id).copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A non-square screen, deliberately. A square one lets an x/y swap read
    /// as correct -- the same reason the anchor tests use 1920x1080.
    const W: f32 = 1920.0;
    const H: f32 = 1080.0;

    fn button(id: &str, w: f32, h: f32) -> UiWidget {
        UiWidget::Button {
            id: id.into(),
            label: id.into(),
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
            anchor: UiAnchor::TOP_LEFT,
        }
    }

    fn container(id: &str, direction: UiDirection) -> UiWidget {
        UiWidget::Container {
            id: id.into(),
            x: 100.0,
            y: 50.0,
            width: 400.0,
            height: 300.0,
            anchor: UiAnchor::TOP_LEFT,
            direction,
            spacing: 0.0,
            padding: 0.0,
            align: UiAlign::Stretch,
        }
    }

    /// Builds a container holding `kids`, applying `tweak` to the container.
    fn row_of(kids: &[UiWidget], direction: UiDirection, tweak: impl Fn(&mut UiWidget)) -> UiState {
        let mut c = container("box", direction);
        tweak(&mut c);
        let mut st = UiState::default();
        st.set_widget(c);
        for k in kids {
            st.set_widget(k.clone());
            st.set_parent(k.id(), "box");
        }
        st
    }

    #[test]
    fn a_widget_with_no_parent_is_placed_exactly_as_before() {
        // The backwards-compatibility property. Every existing game declares
        // widgets with no container at all, and they must resolve through the
        // anchor alone, as they did before containers existed.
        let mut st = UiState::default();
        st.set_widget(button("solo", 120.0, 40.0));
        let rects = st.layout(W, H);
        let r = rects.get("solo").expect("root widget must be laid out");
        let (x, y, w, h) = UiAnchor::TOP_LEFT.resolve(0.0, 0.0, 120.0, 40.0, W, H);
        assert_eq!(
            *r,
            UiRect {
                x,
                y,
                width: w,
                height: h
            },
            "a parentless widget must resolve exactly as its anchor alone says"
        );
    }

    #[test]
    fn a_row_places_children_left_to_right_and_a_column_top_to_bottom() {
        // Asymmetric child sizes, so an axis swap or a reversed order shows.
        let kids = [button("a", 50.0, 20.0), button("b", 70.0, 30.0)];
        let row = row_of(&kids, UiDirection::Horizontal, |_| {}).layout(W, H);
        let col = row_of(&kids, UiDirection::Vertical, |_| {}).layout(W, H);

        assert_eq!(
            row["a"].x, 100.0,
            "first child starts at the container's edge"
        );
        assert_eq!(row["b"].x, 150.0, "second child follows the first's width");
        assert_eq!(row["a"].y, row["b"].y, "a row does not stagger vertically");

        assert_eq!(col["a"].y, 50.0);
        assert_eq!(col["b"].y, 70.0, "second child follows the first's height");
        assert_eq!(
            col["a"].x, col["b"].x,
            "a column does not stagger horizontally"
        );
    }

    #[test]
    fn spacing_separates_children_without_being_added_after_the_last() {
        let kids = [button("a", 50.0, 20.0), button("b", 70.0, 30.0)];
        let r = row_of(&kids, UiDirection::Horizontal, |c| {
            if let UiWidget::Container { spacing, .. } = c {
                *spacing = 10.0;
            }
        })
        .layout(W, H);
        assert_eq!(
            r["b"].x, 160.0,
            "one gap between two children, not two gaps"
        );
        assert_eq!(
            r["b"].x + r["b"].width,
            230.0,
            "trailing space must not be added past the last child"
        );
    }

    #[test]
    fn padding_insets_children_on_every_side() {
        let kids = [button("a", 50.0, 20.0)];
        let r = row_of(&kids, UiDirection::Horizontal, |c| {
            if let UiWidget::Container { padding, .. } = c {
                *padding = 12.0;
            }
        })
        .layout(W, H);
        assert_eq!(r["a"].x, 112.0, "padded from the left edge");
        assert_eq!(r["a"].y, 62.0, "and from the top edge");
        // Stretch is the default cross alignment, so the child spans the
        // padded height -- both sides inset, not just one.
        assert_eq!(
            r["a"].height,
            300.0 - 24.0,
            "padding must come off both sides of the cross axis, not one"
        );
    }

    #[test]
    fn fill_weights_split_what_the_fixed_children_leave() {
        // One fixed child and two filling ones with *different* weights: equal
        // weights would let a bug that ignores the weight read as correct.
        let mut st = row_of(
            &[
                button("fixed", 100.0, 20.0),
                button("one", 0.0, 20.0),
                button("three", 0.0, 20.0),
            ],
            UiDirection::Horizontal,
            |_| {},
        );
        st.set_fill("one", 1.0);
        st.set_fill("three", 3.0);
        let r = st.layout(W, H);
        // 400 wide, 100 taken by the fixed child, 300 to split 1:3.
        assert_eq!(r["fixed"].width, 100.0);
        assert_eq!(r["one"].width, 75.0);
        assert_eq!(r["three"].width, 225.0);
        assert_eq!(
            r["fixed"].width + r["one"].width + r["three"].width,
            400.0,
            "the children must exactly fill the container, leaving no gap"
        );
    }

    #[test]
    fn spacing_comes_out_of_the_fill_budget_not_out_of_the_container() {
        let mut st = row_of(
            &[button("one", 0.0, 20.0), button("two", 0.0, 20.0)],
            UiDirection::Horizontal,
            |c| {
                if let UiWidget::Container { spacing, .. } = c {
                    *spacing = 40.0;
                }
            },
        );
        st.set_fill("one", 1.0);
        st.set_fill("two", 1.0);
        let r = st.layout(W, H);
        assert_eq!(r["one"].width, 180.0, "(400 - 40 gap) / 2");
        assert_eq!(
            r["two"].x + r["two"].width,
            500.0,
            "filling children must not overrun the container by the gap between them"
        );
    }

    #[test]
    fn cross_alignment_places_children_across_the_stacking_axis() {
        let kids = [button("a", 50.0, 100.0)];
        let at = |align: UiAlign| {
            row_of(&kids, UiDirection::Horizontal, move |c| {
                if let UiWidget::Container { align: a, .. } = c {
                    *a = align;
                }
            })
            .layout(W, H)["a"]
        };
        // Container is y=50 h=300; the child is 100 tall.
        assert_eq!(at(UiAlign::Stretch).height, 300.0, "stretch spans the axis");
        assert_eq!(at(UiAlign::Start).y, 50.0);
        assert_eq!(at(UiAlign::Start).height, 100.0, "start keeps its own size");
        assert_eq!(at(UiAlign::Center).y, 150.0, "(300 - 100) / 2 past the top");
        assert_eq!(at(UiAlign::End).y, 250.0, "flush with the bottom edge");
    }

    #[test]
    fn a_container_inside_a_container_lays_out_within_its_own_rectangle() {
        let mut st = UiState::default();
        st.set_widget(container("outer", UiDirection::Vertical));
        let mut inner = container("inner", UiDirection::Horizontal);
        if let UiWidget::Container { width, height, .. } = &mut inner {
            *width = 200.0;
            *height = 80.0;
        }
        st.set_widget(inner);
        st.set_parent("inner", "outer");
        st.set_widget(button("leaf", 30.0, 10.0));
        st.set_parent("leaf", "inner");
        let r = st.layout(W, H);
        // outer is at (100, 50) and stacks vertically with stretch, so inner
        // spans outer's width and sits at its top.
        assert_eq!(r["inner"].x, 100.0);
        assert_eq!(r["inner"].y, 50.0);
        assert_eq!(
            r["leaf"].x, 100.0,
            "the leaf is placed inside inner, not against the screen"
        );
        assert_eq!(r["leaf"].y, 50.0);
    }

    #[test]
    fn a_parent_that_does_not_exist_leaves_the_child_a_root() {
        let mut st = UiState::default();
        st.set_widget(button("orphan", 120.0, 40.0));
        st.set_parent("orphan", "no-such-container");
        let r = st.layout(W, H);
        let (x, y, w, h) = UiAnchor::TOP_LEFT.resolve(0.0, 0.0, 120.0, 40.0, W, H);
        assert_eq!(
            r["orphan"],
            UiRect {
                x,
                y,
                width: w,
                height: h
            },
            "naming a container that was never declared must leave the widget \
             where it would have been, not drop it from the frame"
        );
    }

    #[test]
    fn a_parent_cycle_is_survived_and_every_widget_is_still_drawn() {
        // Two containers naming each other. Unguarded this loops forever;
        // the frame must still be drawn, with the cycle's members as roots.
        //
        // Note the detection mode: deleting the guard makes this test *hang*
        // rather than fail (verified). In CI that is a job timeout, not a red
        // assertion, so if this ever starts running long the guard is the first
        // thing to look at.
        let mut st = UiState::default();
        st.set_widget(container("a", UiDirection::Horizontal));
        st.set_widget(container("b", UiDirection::Horizontal));
        st.set_parent("a", "b");
        st.set_parent("b", "a");
        let r = st.layout(W, H);
        assert!(r.contains_key("a") && r.contains_key("b"), "{r:?}");
    }

    #[test]
    fn a_widget_is_laid_out_even_when_its_container_is_declared_after_it() {
        // Scripts set properties in whatever order they please, and a menu is
        // commonly built children-first.
        let mut st = UiState::default();
        st.set_widget(button("kid", 50.0, 20.0));
        st.set_parent("kid", "box");
        st.set_widget(container("box", UiDirection::Horizontal));
        let r = st.layout(W, H);
        assert_eq!(
            r["kid"].x, 100.0,
            "declaration order must not decide whether the container applies"
        );
    }

    #[test]
    fn detaching_a_child_returns_it_to_the_root() {
        let mut st = row_of(&[button("a", 50.0, 20.0)], UiDirection::Horizontal, |_| {});
        assert_eq!(st.layout(W, H)["a"].x, 100.0);
        st.set_parent("a", "");
        assert_eq!(
            st.layout(W, H)["a"].x,
            0.0,
            "an empty parent must detach, not be stored as a container named \"\""
        );
    }

    #[test]
    fn layout_is_resolution_independent_for_an_anchored_container() {
        // The #1841 property, now through a container: the same declaration at
        // two resolutions must put children in the same place relative to the
        // anchor it is pinned to.
        let mut st = row_of(&[button("a", 50.0, 20.0)], UiDirection::Horizontal, |c| {
            if let UiWidget::Container { anchor, x, y, .. } = c {
                *anchor = UiAnchor {
                    min_x: 1.0,
                    min_y: 1.0,
                    max_x: 1.0,
                    max_y: 1.0,
                };
                *x = -500.0;
                *y = -400.0;
            }
        });
        let small = st.layout(1280.0, 720.0)["a"];
        let big = st.layout(W, H)["a"];
        assert_eq!(
            (small.x - 1280.0, small.y - 720.0),
            (big.x - W, big.y - H),
            "a bottom-right container must hold the same offset from the corner \
             at any resolution"
        );
        let _ = &mut st;
    }

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

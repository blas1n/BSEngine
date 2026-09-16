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
    /// Left to right, wrapping to a new row every `columns` children.
    ///
    /// Columns split the container's width equally rather than taking a fixed
    /// cell size. That is Godot's model (name the column count, let the
    /// container size them) with Unreal's uniform cells, and it is a choice
    /// rather than a lookup: the three reference engines genuinely disagree
    /// here — Unity takes a fixed `cellSize`, Unreal sizes cells to the largest
    /// child, Godot sizes each column to its own widest child.
    ///
    /// Unity's fixed cell was rejected on two grounds: it silently overrides
    /// the width and height the author just wrote on the child, and a cell
    /// pinned to a pixel count undoes the resolution independence anchors
    /// exist to provide. Godot's per-column content sizing has nothing to
    /// measure here, because widgets carry explicit sizes rather than
    /// content-derived ones.
    Grid {
        /// Children per row. Clamped to at least 1 where it is used, so a
        /// zero cannot divide by zero or silently drop every child.
        columns: u32,
    },
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

/// Where a widget ended up, and what it is clipped to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiPlacement {
    /// The widget's rectangle, in screen pixels.
    pub rect: UiRect,
    /// The rectangle outside which this widget must not draw.
    ///
    /// `None` for anything not inside a scroll container. Clipping is opt-in
    /// per container rather than universal: children of an ordinary container
    /// are allowed to overflow it today -- a label wider than its cell, a
    /// button in a container sized smaller than its content -- and clipping
    /// every container would change that silently.
    pub clip: Option<UiRect>,
}

/// The result of one layout pass.
#[derive(Clone, Debug, Default)]
pub struct UiLayout {
    /// Every widget's placement, keyed by id.
    pub placements: HashMap<String, UiPlacement>,
    /// How far each scroll container *can* scroll, keyed by container id.
    ///
    /// Zero on an axis whose content fits. Reported rather than applied so the
    /// thing that changes the offset -- a wheel, a script, a scrollbar -- can
    /// clamp what it stores, instead of every reader having to re-derive the
    /// content extent to know whether an offset is reachable.
    pub scroll_max: HashMap<String, (f32, f32)>,
}

impl std::ops::Index<&str> for UiLayout {
    type Output = UiRect;

    /// The rectangle of the widget with this id.
    ///
    /// A convenience for the common case -- most callers want where a widget
    /// went, not what it is clipped to. The clip is reached through
    /// [`placements`](UiLayout::placements).
    ///
    /// # Panics
    ///
    /// If no widget with that id was laid out. That is a caller bug: the pass
    /// places every widget it was given, so a missing id means the id itself
    /// is wrong.
    fn index(&self, id: &str) -> &UiRect {
        &self
            .placements
            .get(id)
            .unwrap_or_else(|| panic!("no widget laid out with id {id:?}"))
            .rect
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
        /// Whether children are offset by this container's scroll position and
        /// clipped to its rectangle.
        ///
        /// Opt-in, so every container written before scrolling existed keeps
        /// letting its children overflow exactly as it did. Unity's
        /// `ScrollRect`, Unreal's `ScrollBox` and Godot's `ScrollContainer` are
        /// all likewise a distinct thing you reach for rather than a property
        /// every container has.
        scrollable: bool,
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
    /// Scroll offset of each scroll container, keyed by container id.
    ///
    /// A side table for the same reason `parents` and `fills` are: it is
    /// per-widget state that arrives from its own op, and it changes at
    /// runtime while the container's own declaration does not.
    pub scroll_offsets: HashMap<String, (f32, f32)>,
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

    /// Sets a scroll container's offset. Negative values clamp to zero.
    ///
    /// Not clamped against the content here, because the content extent is a
    /// layout result and this is called before layout runs. `UiLayout`'s
    /// `scroll_max` is what a caller clamps against once it knows.
    pub fn set_scroll(&mut self, id: &str, x: f32, y: f32) {
        let sane = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        self.scroll_offsets
            .insert(id.to_string(), (sane(x), sane(y)));
    }

    /// A container's scroll offset, or `(0, 0)` if it has never been set.
    pub fn scroll_of(&self, id: &str) -> (f32, f32) {
        self.scroll_offsets.get(id).copied().unwrap_or((0.0, 0.0))
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
        self.scroll_offsets.clear();
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
    pub fn layout(&self, screen_w: f32, screen_h: f32) -> UiLayout {
        let mut out = UiLayout::default();
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
                    // A root is clipped by nothing; a scroll container passes
                    // its own rectangle down to its descendants.
                    None,
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
        clip: Option<UiRect>,
        children: &HashMap<&'a str, Vec<&'a UiWidget>>,
        out: &mut UiLayout,
    ) {
        out.placements
            .insert(widget.id().to_string(), UiPlacement { rect, clip });
        let UiWidget::Container {
            direction,
            spacing,
            padding,
            align,
            scrollable,
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
        // Everything below a scroll container is clipped to it, and nested
        // scroll containers intersect rather than replace: an inner list inside
        // an outer one must not paint outside the outer one when the outer is
        // scrolled away.
        let child_clip = if *scrollable {
            Some(match clip {
                Some(c) => Self::intersect(c, rect),
                None => rect,
            })
        } else {
            clip
        };
        let (off_x, off_y) = if *scrollable {
            self.scroll_of(widget.id())
        } else {
            (0.0, 0.0)
        };
        if let UiDirection::Grid { columns } = direction {
            self.place_grid(
                *columns,
                kids,
                inner,
                *spacing,
                *align,
                (off_x, off_y),
                child_clip,
                widget.id(),
                children,
                out,
            );
            return;
        }
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
        // Content is only larger than the viewport when nothing fills: a
        // filling child takes exactly the leftover, so a container whose
        // children fill can never overflow and never scrolls.
        let content = fixed + gaps + if total_weight > 0.0 { leftover } else { 0.0 };
        if *scrollable {
            let over = (content - main_span).max(0.0);
            out.scroll_max.insert(
                widget.id().to_string(),
                if horizontal { (over, 0.0) } else { (0.0, over) },
            );
        }
        let mut cursor = if horizontal {
            inner.x - off_x
        } else {
            inner.y - off_y
        };
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
            self.place(kid, kid_rect, child_clip, children, out);
        }
    }

    /// The overlap of two rectangles, empty when they do not meet.
    fn intersect(a: UiRect, b: UiRect) -> UiRect {
        let x0 = a.x.max(b.x);
        let y0 = a.y.max(b.y);
        let x1 = (a.x + a.width).min(b.x + b.width);
        let y1 = (a.y + a.height).min(b.y + b.height);
        UiRect {
            x: x0,
            y: y0,
            width: (x1 - x0).max(0.0),
            height: (y1 - y0).max(0.0),
        }
    }

    /// Lays children out in rows of `columns`, inside `inner`.
    ///
    /// Column width is the container's width split equally, so a grid stays
    /// proportional at any resolution. Row height is the tallest child in that
    /// row, which is the one place a cell is content-derived: rows are not
    /// bounded by anything the way columns are bounded by the container's
    /// width, so splitting the height equally would depend on how many children
    /// happen to exist.
    #[allow(clippy::too_many_arguments)]
    fn place_grid<'a>(
        &'a self,
        columns: u32,
        kids: &[&'a UiWidget],
        inner: UiRect,
        spacing: f32,
        align: UiAlign,
        scroll: (f32, f32),
        clip: Option<UiRect>,
        id: &str,
        children: &HashMap<&'a str, Vec<&'a UiWidget>>,
        out: &mut UiLayout,
    ) {
        let columns = columns.max(1) as usize;
        let gaps = spacing * columns.saturating_sub(1) as f32;
        let cell_width = ((inner.width - gaps) / columns as f32).max(0.0);
        let mut row_top = inner.y - scroll.1;
        for row in kids.chunks(columns) {
            // The tallest child decides the row, so a short widget beside a
            // tall one does not clip it.
            let row_height = row
                .iter()
                .map(|k| k.height())
                .fold(0.0_f32, f32::max)
                .max(0.0);
            for (col, kid) in row.iter().enumerate() {
                let cell_x = inner.x - scroll.0 + col as f32 * (cell_width + spacing);
                let own_w = kid.width();
                let own_h = kid.height();
                // `align` means the same thing it does for a row or a column:
                // stretch fills the cell, anything else keeps the child's own
                // size. Applied to both axes here, because a grid cell bounds
                // the child on both.
                let (w, h, dx, dy) = match align {
                    UiAlign::Stretch => (cell_width, row_height, 0.0, 0.0),
                    UiAlign::Start => (own_w, own_h, 0.0, 0.0),
                    UiAlign::Center => (
                        own_w,
                        own_h,
                        ((cell_width - own_w) / 2.0).max(0.0),
                        ((row_height - own_h) / 2.0).max(0.0),
                    ),
                    UiAlign::End => (
                        own_w,
                        own_h,
                        (cell_width - own_w).max(0.0),
                        (row_height - own_h).max(0.0),
                    ),
                };
                self.place(
                    kid,
                    UiRect {
                        x: cell_x + dx,
                        y: row_top + dy,
                        width: w,
                        height: h,
                    },
                    clip,
                    children,
                    out,
                );
            }
            row_top += row_height + spacing;
        }
        // Rows grow downward without bound, so a grid's reachable scroll is
        // whatever its rows overran the viewport by.
        let consumed = row_top + scroll.1 - inner.y;
        out.scroll_max
            .insert(id.to_string(), (0.0, (consumed - inner.height).max(0.0)));
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
            scrollable: false,
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

    /// Three columns of a 400-wide container with a 10px gap: each column is
    /// (400 - 20) / 3 = 126.666.
    #[test]
    fn a_grid_splits_the_container_width_equally_across_its_columns() {
        let kids = [
            button("a", 10.0, 20.0),
            button("b", 10.0, 20.0),
            button("c", 10.0, 20.0),
        ];
        let r = row_of(&kids, UiDirection::Grid { columns: 3 }, |c| {
            if let UiWidget::Container { spacing, .. } = c {
                *spacing = 10.0;
            }
        })
        .layout(W, H);
        let expected = (400.0 - 20.0) / 3.0;
        for id in ["a", "b", "c"] {
            assert!(
                (r[id].width - expected).abs() < 1e-3,
                "every column must be {expected} wide, but {id} is {}",
                r[id].width
            );
        }
        assert!((r["a"].x - 100.0).abs() < 1e-3);
        assert!(
            (r["b"].x - (100.0 + expected + 10.0)).abs() < 1e-3,
            "the second column must follow the first plus one gap, not two"
        );
        assert!(
            (r["c"].x + r["c"].width - 500.0).abs() < 1e-3,
            "the last column must end exactly at the container's right edge"
        );
    }

    /// The property that distinguishes a grid from a row.
    #[test]
    fn a_grid_wraps_to_a_new_row_after_the_column_count() {
        // Four children in two columns: two rows. Asymmetric heights so a row
        // that took the wrong child's height shows.
        let kids = [
            button("a", 10.0, 20.0),
            button("b", 10.0, 50.0),
            button("c", 10.0, 30.0),
            button("d", 10.0, 30.0),
        ];
        let r = row_of(&kids, UiDirection::Grid { columns: 2 }, |_| {}).layout(W, H);
        assert_eq!(r["a"].y, r["b"].y, "a and b share the first row");
        assert_eq!(r["c"].y, r["d"].y, "c and d share the second row");
        assert!(
            r["c"].y > r["a"].y,
            "the second row must sit below the first, not beside it"
        );
        assert_eq!(
            r["c"].y,
            50.0 + 50.0,
            "the second row starts after the tallest child of the first (50), \
             not after the first child's own height (20)"
        );
        assert_eq!(r["a"].x, r["c"].x, "column 0 is one x for every row");
    }

    #[test]
    fn a_grid_stretches_children_to_their_cell_by_default() {
        let kids = [button("a", 10.0, 20.0), button("b", 10.0, 50.0)];
        let r = row_of(&kids, UiDirection::Grid { columns: 2 }, |_| {}).layout(W, H);
        assert_eq!(r["a"].width, 200.0, "half of a 400-wide container");
        assert_eq!(
            r["a"].height, 50.0,
            "stretch fills the row, whose height is the tallest child's"
        );
    }

    #[test]
    fn a_grid_alignment_other_than_stretch_keeps_the_childs_own_size() {
        let kids = [button("a", 40.0, 20.0), button("b", 10.0, 60.0)];
        let at = |align: UiAlign| {
            row_of(&kids, UiDirection::Grid { columns: 2 }, move |c| {
                if let UiWidget::Container { align: a, .. } = c {
                    *a = align;
                }
            })
            .layout(W, H)["a"]
        };
        let start = at(UiAlign::Start);
        assert_eq!(
            (start.width, start.height),
            (40.0, 20.0),
            "start keeps the child's own size in both axes"
        );
        assert_eq!((start.x, start.y), (100.0, 50.0));
        // Cell is 200 wide and the row is 60 tall (b is the tallest).
        let centre = at(UiAlign::Center);
        assert_eq!(centre.x, 100.0 + (200.0 - 40.0) / 2.0);
        assert_eq!(centre.y, 50.0 + (60.0 - 20.0) / 2.0);
        let end = at(UiAlign::End);
        assert_eq!(end.x, 100.0 + 200.0 - 40.0);
        assert_eq!(end.y, 50.0 + 60.0 - 20.0);
    }

    #[test]
    fn a_grid_with_zero_columns_still_lays_every_child_out() {
        // Zero would divide by zero and could silently drop every child; it is
        // clamped to one column instead.
        let kids = [button("a", 10.0, 20.0), button("b", 10.0, 20.0)];
        let r = row_of(&kids, UiDirection::Grid { columns: 0 }, |_| {}).layout(W, H);
        assert!(
            r.placements.contains_key("a") && r.placements.contains_key("b"),
            "{r:?}"
        );
        assert!(
            r["a"].width.is_finite() && r["a"].width > 0.0,
            "a clamped single column must still have a real width, got {}",
            r["a"].width
        );
        assert!(r["b"].y > r["a"].y, "one column means one child per row");
    }

    /// A grid inside a row, to prove the branch composes with the rest of the
    /// pass rather than being a separate path that only works at the top.
    #[test]
    fn a_grid_nested_in_a_column_lays_out_inside_its_own_rectangle() {
        let mut st = UiState::default();
        st.set_widget(container("outer", UiDirection::Vertical));
        let mut grid = container("grid", UiDirection::Grid { columns: 2 });
        if let UiWidget::Container { width, height, .. } = &mut grid {
            *width = 200.0;
            *height = 80.0;
        }
        st.set_widget(grid);
        st.set_parent("grid", "outer");
        for id in ["a", "b"] {
            st.set_widget(button(id, 10.0, 20.0));
            st.set_parent(id, "grid");
        }
        let r = st.layout(W, H);
        // outer stacks vertically with stretch, so the grid spans outer's width
        // (400) at outer's top-left.
        assert_eq!(r["a"].x, 100.0, "the grid's first cell starts at the grid");
        assert_eq!(r["a"].y, 50.0);
        assert!(
            r["b"].x > r["a"].x,
            "two columns inside a nested grid still sit side by side"
        );
    }

    /// A scrollable column of three fixed-height buttons, taller than its box.
    fn scroll_column() -> UiState {
        let mut c = container("list", UiDirection::Vertical);
        if let UiWidget::Container {
            height, scrollable, ..
        } = &mut c
        {
            *height = 100.0; // shorter than the 3 x 50 of content
            *scrollable = true;
        }
        let mut st = UiState::default();
        st.set_widget(c);
        for id in ["a", "b", "c"] {
            st.set_widget(button(id, 40.0, 50.0));
            st.set_parent(id, "list");
        }
        st
    }

    #[test]
    fn a_container_is_not_scrollable_or_clipped_unless_asked() {
        // The backwards-compatibility property: every container written before
        // scrolling existed still lets its children overflow, and clips
        // nothing.
        let kids = [button("a", 40.0, 50.0)];
        let st = row_of(&kids, UiDirection::Vertical, |_| {});
        let l = st.layout(W, H);
        assert!(
            l.placements["a"].clip.is_none(),
            "an ordinary container must not clip its children"
        );
        assert!(
            l.scroll_max.is_empty(),
            "a container that is not scrollable reports no scroll range"
        );
    }

    #[test]
    fn scrolling_shifts_children_up_by_the_offset() {
        let mut st = scroll_column();
        let before = st.layout(W, H)["a"].y;
        st.set_scroll("list", 0.0, 30.0);
        let after = st.layout(W, H)["a"].y;
        assert_eq!(
            after,
            before - 30.0,
            "scrolling down by 30 must move the first child up by 30"
        );
    }

    #[test]
    fn a_scroll_container_clips_its_children_to_itself() {
        let st = scroll_column();
        let l = st.layout(W, H);
        let list = l["list"];
        for id in ["a", "b", "c"] {
            let clip = l.placements[id].clip.expect("a child must be clipped");
            assert_eq!(
                clip, list,
                "every child of a scroll container is clipped to the container"
            );
        }
        // The third child starts past the bottom of a 100-tall box: it is laid
        // out, and the clip is what keeps it off screen.
        assert!(
            l["c"].y >= list.y + list.height,
            "the overflowing child should still be placed, at {} vs box bottom {}",
            l["c"].y,
            list.y + list.height
        );
    }

    #[test]
    fn scroll_max_is_the_overflow_and_zero_when_content_fits() {
        let st = scroll_column();
        let over = st.layout(W, H).scroll_max["list"];
        // 3 x 50 of content in a 100-tall box overflows by 50.
        assert_eq!(over, (0.0, 50.0), "vertical overflow only");

        // The same container, tall enough for its content, cannot scroll.
        let mut st2 = scroll_column();
        if let Some(UiWidget::Container { height, .. }) =
            st2.widgets.iter_mut().find(|w| w.id() == "list")
        {
            *height = 500.0;
        }
        assert_eq!(
            st2.layout(W, H).scroll_max["list"],
            (0.0, 0.0),
            "content that fits leaves nothing to scroll"
        );
    }

    #[test]
    fn a_nested_scroll_container_clips_to_the_overlap_of_both() {
        // An inner list inside an outer one must not paint outside the outer
        // box when the outer is scrolled away.
        let mut st = UiState::default();
        let mut outer = container("outer", UiDirection::Vertical);
        if let UiWidget::Container {
            height, scrollable, ..
        } = &mut outer
        {
            *height = 60.0;
            *scrollable = true;
        }
        st.set_widget(outer);
        let mut inner = container("inner", UiDirection::Vertical);
        if let UiWidget::Container {
            height, scrollable, ..
        } = &mut inner
        {
            *height = 200.0;
            *scrollable = true;
        }
        st.set_widget(inner);
        st.set_parent("inner", "outer");
        st.set_widget(button("leaf", 40.0, 50.0));
        st.set_parent("leaf", "inner");

        let l = st.layout(W, H);
        let clip = l.placements["leaf"].clip.expect("clipped");
        assert!(
            clip.height <= 60.0 + 1e-3,
            "the leaf's clip must be bounded by the 60-tall outer box, not the \
             200-tall inner one; got {clip:?}"
        );
    }

    #[test]
    fn a_negative_or_nonsense_scroll_offset_is_ignored() {
        let mut st = scroll_column();
        let base = st.layout(W, H)["a"].y;
        st.set_scroll("list", 0.0, -100.0);
        assert_eq!(
            st.layout(W, H)["a"].y,
            base,
            "scrolling above the top must clamp to zero rather than pushing \
             content down into empty space"
        );
        st.set_scroll("list", f32::NAN, f32::INFINITY);
        assert!(
            st.layout(W, H)["a"].y.is_finite(),
            "a nonsense offset must not poison the layout"
        );
    }

    #[test]
    fn a_scrollable_grid_scrolls_its_rows() {
        let mut st = UiState::default();
        let mut g = container("grid", UiDirection::Grid { columns: 2 });
        if let UiWidget::Container {
            height, scrollable, ..
        } = &mut g
        {
            *height = 60.0;
            *scrollable = true;
        }
        st.set_widget(g);
        for id in ["a", "b", "c", "d"] {
            st.set_widget(button(id, 40.0, 50.0));
            st.set_parent(id, "grid");
        }
        let before = st.layout(W, H)["c"].y;
        st.set_scroll("grid", 0.0, 20.0);
        let after = st.layout(W, H)["c"].y;
        assert_eq!(after, before - 20.0, "a grid's second row scrolls too");
    }

    #[test]
    fn a_widget_with_no_parent_is_placed_exactly_as_before() {
        // The backwards-compatibility property. Every existing game declares
        // widgets with no container at all, and they must resolve through the
        // anchor alone, as they did before containers existed.
        let mut st = UiState::default();
        st.set_widget(button("solo", 120.0, 40.0));
        let rects = st.layout(W, H);
        let r = &rects["solo"];
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
        assert!(
            r.placements.contains_key("a") && r.placements.contains_key("b"),
            "{r:?}"
        );
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

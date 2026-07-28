use crate::panels::reflect_ui::{
    draw_reflect_ui, is_hidden_reflected_type, short_component_name, validate_after_edit,
    ReflectUiCtx,
};
use bsengine_core::{EditorPanel, EditorPanelContext, InspectorCmd};

/// The Inspector panel: renders a header (entity name + Visible toggle), a
/// generic component list (no wrapping section label -- each attached
/// component is its own block, separated by a thin rule, matching Unity's
/// Inspector convention) that renders and edits every reflectable
/// component currently attached to the selected entity via
/// `draw_reflect_ui`, showing each one's short type name rather than its
/// full namespace-qualified path, and finally an Add Component button that
/// opens a picker of every registered, reflectable component not already
/// present. No component type gets bespoke, hand-built UI here anymore --
/// Transform, Tags, Script, and Mesh (the former hardcoded sections) all
/// now render exclusively through this generic list, same as every other
/// reflected component.
pub struct InspectorPanel;

impl EditorPanel for InspectorPanel {
    fn id(&self) -> &str {
        "inspector"
    }

    fn title(&self) -> String {
        "Inspector".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorPanelContext) {
        // Hoisted for readability, not because the borrow checker demands
        // it: Rust 2021 captures disjoint fields individually, so the
        // ScrollArea closure below could name `ctx.type_registry` and
        // `ctx.entities_snapshot` directly alongside its use of
        // `ctx.insp`, and dropping this hoist in favour of the fields at
        // all six use sites compiles. Naming them once here just spares
        // the closure body six repetitions of `ctx.`. Both are Copy.
        let type_registry = ctx.type_registry;
        let entities_snapshot = ctx.entities_snapshot;
        let insp = &mut *ctx.insp;

        let Some(sel_id) = insp.selected_id else {
            ui.label("No entity selected.");
            return;
        };
        let sel_info = entities_snapshot
            .iter()
            .find(|e| e.id == sel_id)
            .cloned()
            .unwrap_or_default();
        // Mirrors `hierarchy.rs`'s row text exactly (`[{id}] {label}`, with
        // the same "(unnamed)" placeholder) so the Inspector header always
        // reads as the same entity the Hierarchy selection highlighted —
        // this used to say "Entity {id}" here but "(unnamed)" in the tree,
        // two different placeholder strings for the same "no name" state.
        let label = sel_info.name.as_deref().unwrap_or("(unnamed)");
        let entity_name = format!("[{sel_id}] {label}");

        // The panel had no scrolling at all until now (unlike hierarchy.rs
        // and asset_browser.rs, which both wrap their bodies this way, and
        // unlike the tab host in dock.rs, which adds none). That was
        // survivable while Add Component sat at the top and was always
        // visible; with it moved below the component list, scrolling is
        // what keeps it reachable on an entity with many components.
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(&entity_name);
            ui.separator();

            // Visible toggle
            if ui.checkbox(&mut insp.edit_visible, "Visible").changed() {
                insp.cmd_queue.push(InspectorCmd::SetVisible {
                    id: sel_id,
                    visible: insp.edit_visible,
                });
            }
            ui.separator();

            // Whether any component will actually render. Drives both the
            // list itself and whether the Add Component button needs a rule
            // above it -- deliberately the same `is_hidden_reflected_type`
            // predicate the list filters on, not `reflected_components
            // .is_empty()`. PR #1728 fixed exactly that mismatch: an entity
            // holding only hidden components satisfied `!is_empty()` but
            // rendered nothing, leaving a separator with nothing under it.
            let has_visible_components = insp
                .reflected_components
                .iter()
                .any(|(p, _)| !is_hidden_reflected_type(p));

            if has_visible_components {
                let reflect_ctx = ReflectUiCtx {
                    entities: entities_snapshot,
                    type_registry,
                };
                let mut to_apply: Vec<(String, Box<dyn bevy_reflect::Reflect>)> = Vec::new();
                let mut to_remove: Option<String> = None;
                for (i, (type_path, value)) in insp
                    .reflected_components
                    .iter_mut()
                    .filter(|(p, _)| !is_hidden_reflected_type(p))
                    .enumerate()
                {
                    if i > 0 {
                        ui.separator();
                    }
                    let header_id = component_header_id(type_path.as_str());
                    egui::containers::collapsing_header::CollapsingState::load_with_default_open(
                        ui.ctx(),
                        header_id,
                        true,
                    )
                    .show_header(ui, |ui| {
                        ui.colored_label(
                            crate::theme::TEXT,
                            short_component_name(type_path.as_str(), type_registry),
                        );
                        ui.menu_button(egui_phosphor::regular::DOTS_THREE, |ui| {
                            if ui.button("Remove Component").clicked() {
                                to_remove = Some(type_path.clone());
                                ui.close_menu();
                            }
                        });
                    })
                    .body(|ui| {
                        if draw_reflect_ui(ui, value.as_mut(), &reflect_ctx) {
                            validate_after_edit(type_path, value.as_mut(), type_registry);
                            to_apply.push((type_path.clone(), value.clone_value()));
                        }
                    });
                }
                for (type_path, value) in to_apply {
                    insp.cmd_queue.push(InspectorCmd::ApplyReflectedComponent {
                        id: sel_id,
                        type_path,
                        value,
                    });
                }
                if let Some(type_path) = to_remove {
                    insp.cmd_queue.push(InspectorCmd::RemoveComponentByType {
                        id: sel_id,
                        type_path,
                    });
                }
            }

            // Add Component -- a centred, inset button at the very bottom
            // (Unity's placement) that opens a picker listing every
            // registered, ReflectDefault-constructible component type not
            // already attached (filtering prevents a confusing
            // duplicate-attach).
            if let Some(registry) = type_registry {
                // Only when the list above actually drew something; the
                // Visible toggle's own separator already precedes the button
                // otherwise, and a second one would stack two rules.
                if has_visible_components {
                    ui.separator();
                }

                if let Some(type_path) =
                    draw_add_component(ui, registry, &insp.reflected_components)
                {
                    insp.cmd_queue.push(InspectorCmd::AttachComponentByType {
                        id: sel_id,
                        type_path,
                    });
                }
            }
        });
    }
}

/// Placeholder shown in the Add Component picker's search field while it
/// is empty.
///
/// A `const` rather than a literal at the one use site because it is doing
/// double duty: it is the user-facing placeholder *and* the only thing a
/// test can use to locate the field, since an empty `TextEdit` paints no
/// content of its own. Sharing the constant makes that coupling a
/// compile-time one instead of a documentary one.
const SEARCH_HINT: &str = "Search";

/// Shown in the Add Component picker in place of the row list when the
/// search text matched no component type.
///
/// A `const` for the same reason as [`SEARCH_HINT`]: it is what the tests
/// look for in the rendered output, and a duplicated literal there could
/// drift from this one without anything failing.
const NO_MATCHES_LABEL: &str = "No matches";

/// Shown in the Add Component picker in place of the row list when nothing
/// is listed and nothing was typed -- i.e. every registered, constructible
/// component type is already attached to this entity, so there is nothing
/// left to add. Distinct from [`NO_MATCHES_LABEL`] because the two states
/// call for different reactions: clear the search, versus nothing to do.
const ALL_ATTACHED_LABEL: &str = "All components attached";

/// Draws the Add Component button and, when it is open, the picker popup
/// on whichever side of the button has more room (see the direction choice
/// at the `popup_above_or_below_widget` call). Returns the `type_path` of
/// the component type the user chose this frame, if any; the caller is what
/// turns that into an `InspectorCmd`.
///
/// `attached` is the selected entity's current component list, used only to
/// filter already-present types out of the picker (offering them would
/// invite a confusing duplicate-attach).
///
/// Extracted from [`InspectorPanel::ui`] to match this module's neighbours
/// -- `hierarchy.rs` and `asset_browser.rs` each pull five helpers out of
/// their panel body -- and to bring the picker's row loop back from ten
/// levels of nesting. A free fn rather than an associated one:
/// `InspectorPanel` is a unit struct with no `&self` state to reach for
/// (the other panels use associated fns precisely because they need
/// `&mut self`), and `component_header_id` below is this file's existing
/// free-fn precedent.
///
/// That the component-list block above stays inline is history, not a
/// judgement that it resists the same treatment: it already defers its
/// `cmd_queue` pushes until after the iteration via `to_apply`/
/// `to_remove`, so it would factor out on a shape much like this one (a
/// mutable slice in, collected edits out). It simply was not in the scope
/// of the change that moved and rebuilt the Add Component control.
///
/// Two things worth knowing about this signature:
///
/// - **`attached` is narrower than `&mut InspectorState` by choice, not by
///   necessity.** Passing the whole state compiles fine: the caller's
///   `&insp.reflected_components` borrows a place expression, so no
///   temporary exists for the `if let` scrutinee rule to extend into the
///   body, and the returned `Option<String>` carries no lifetime, so NLL
///   ends the borrow when the call returns. The reason for the narrow
///   parameter is design -- this helper has no business touching
///   `cmd_queue`, and handing it a read-only slice of exactly what it
///   filters on makes that contract obvious and the function reasonable to
///   read on its own.
/// - **The popup's ids stay stable only because this takes the caller's own
///   `ui`.** `make_persistent_id` derives from that `Ui`'s id, so wrapping
///   this call in a child `Ui` (a `group`, a `Frame`, a `horizontal`)
///   silently shifts every id below, which would reset the popup's
///   open/closed state and break the tests that read these ids out of
///   egui's memory from outside the panel.
fn draw_add_component(
    ui: &mut egui::Ui,
    registry: &bevy_reflect::TypeRegistry,
    attached: &[(String, Box<dyn bevy_reflect::Reflect>)],
) -> Option<String> {
    let popup_id = ui.make_persistent_id("add_component_popup");
    let filter_id = ui.make_persistent_id("add_component_filter");
    let focus_search_id = ui.make_persistent_id("add_component_focus_search");
    let direction_id = ui.make_persistent_id("add_component_popup_direction");
    // Unity insets this button rather than filling the panel width. The
    // explicit width also keeps `popup_above_or_below_widget`'s
    // debug_assert satisfied: it sizes the popup as `button.rect.width() -
    // Frame::popup's margin` and requires that to be >= 0 (egui-0.29.1
    // popup.rs:410 -> ui.rs:896). A shrink-wrapped button would measure
    // only `2 * button_padding.x` (~8px) against a ~12px margin in these
    // headless tests, where `FontDefinitions::empty()` gives every label
    // zero width -- a real font never gets near that, but the tests would
    // panic.
    //
    // 160 approximates Unity's inset button width; 48 is roughly 4x that
    // ~12px frame margin, i.e. comfortably clear of it. Note the bounds
    // conflict once available width drops below 48, and the lower one
    // deliberately wins: the button overflows rather than shrinking
    // further. That is cosmetic, and preferable to slipping under the
    // popup's margin, which panics.
    //
    // Read on the caller's `Ui` -- outside `vertical_centered`'s closure,
    // but still within the panel's scroll body, so it is net of the
    // scrollbar once one appears.
    let button_width = ui.available_width().clamp(48.0, 160.0);
    let button_response = ui
        .vertical_centered(|ui| {
            ui.add_sized(
                [button_width, ui.spacing().interact_size.y],
                egui::Button::new(format!("{} Add Component", egui_phosphor::regular::PLUS)),
            )
        })
        .inner;

    if button_response.clicked() {
        // Checked before toggling: if it was closed, this click opens it,
        // and the picker should start with an empty search box (matching
        // Unity). Clearing unconditionally would also fire on the click
        // that *closes* it, which is harmless but muddles the intent.
        let was_open = ui.memory(|m| m.is_popup_open(popup_id));
        ui.memory_mut(|m| m.toggle_popup(popup_id));
        if !was_open {
            let direction_at_open = picker_direction(ui.ctx().screen_rect(), button_response.rect);
            ui.memory_mut(|m| {
                m.data.insert_temp(filter_id, String::new());
                // Unity focuses the search box on open so you can type
                // immediately. Only a *request* is recorded here, for the
                // popup closure below to consume -- see the comment there
                // for why focus cannot be granted from this point.
                m.data.insert_temp(focus_search_id, true);
                // The side is decided once, here, and held for as long as
                // the popup stays up. It cannot simply be recomputed at
                // render time: `popup_above_or_below_widget` rebuilds
                // `Area::fixed_pos` from the button's rect every frame, so
                // an open popup already tracks the button as the Inspector
                // scrolls, and a per-frame direction would make it *flip*
                // from one side of the button to the other the moment the
                // button crossed the screen's midpoint. Sliding is the
                // pre-existing behaviour; jumping across the button would
                // be new. Open time is also the only moment the choice has
                // to be right, and the button's rect is already in hand.
                m.data.insert_temp(direction_id, direction_at_open);
            });
        }
    }

    // Falls back to a fresh measurement only if no latched side is found,
    // which normally cannot happen -- the popup is opened by the branch
    // above, which always stores one. It matters if egui's temp store is
    // ever cleared under an open popup, where recomputing is a better
    // answer than defaulting to a fixed side.
    let direction = ui
        .memory(|m| m.data.get_temp::<egui::AboveOrBelow>(direction_id))
        .unwrap_or_else(|| picker_direction(ui.ctx().screen_rect(), button_response.rect));

    let mut to_attach: Option<String> = None;
    egui::popup::popup_above_or_below_widget(
        ui,
        popup_id,
        &button_response,
        direction,
        // Not CloseOnClick (what ComboBox uses) -- the search field below
        // must survive being clicked and typed into. Escape still closes.
        egui::popup::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(200.0);
            // Transient, panel-local UI state: egui memory keeps it out of
            // InspectorState (a cross-crate type that PRs #1727/#1728
            // deliberately pruned of UI-only fields) and off
            // InspectorPanel (a unit struct built at ~26 sites, nearly all
            // tests).
            let mut filter: String = ui.memory(|m| m.data.get_temp(filter_id).unwrap_or_default());
            // The hint doubles as the field's only painted content while
            // empty, which is what lets a test locate it -- an empty
            // TextEdit draws no text of its own.
            let search_response =
                ui.add(egui::TextEdit::singleline(&mut filter).hint_text(SEARCH_HINT));
            // Consume the focus request the open-click recorded.
            //
            // This detour looks removable, and the obvious simplification
            // is wrong in a way that no test failure points at unless you
            // keep the one below. `TextEdit::id` can pin this widget's id,
            // so the click site could name it and call
            // `Memory::request_focus` directly, dropping the flag
            // entirely. That compiles and reads correctly -- and silently
            // breaks autofocus: a request issued through `Memory` back
            // there is dropped before this field ever registers. Pinning
            // the id is harmless on its own; routing the request through
            // `Memory` rather than the widget's own `Response` is what
            // fails. So it has to be issued here, against the response
            // `add` just returned.
            if ui.memory(|m| m.data.get_temp::<bool>(focus_search_id) == Some(true)) {
                search_response.request_focus();
                ui.memory_mut(|m| m.data.insert_temp(focus_search_id, false));
            }
            if search_response.changed() {
                ui.memory_mut(|m| m.data.insert_temp(filter_id, filter.clone()));
            }
            let needle = filter.to_lowercase();
            egui::ScrollArea::vertical()
                .max_height(240.0)
                .show(ui, |ui| {
                    // Whether the loop below drew a single row. Tracked
                    // rather than pre-computed: the three `continue`s are
                    // the only definition of "listable", and a separate
                    // count would be a second copy of that predicate, free
                    // to drift from it.
                    let mut listed_any = false;
                    for registration in registry.iter() {
                        if registration
                            .data::<bevy_ecs::reflect::ReflectComponent>()
                            .is_none()
                        {
                            continue;
                        }
                        if registration
                            .data::<bevy_reflect::std_traits::ReflectDefault>()
                            .is_none()
                        {
                            continue;
                        }
                        let type_path = registration.type_info().type_path().to_string();
                        let already_attached = attached
                            .iter()
                            .any(|(existing_path, _)| existing_path == &type_path);
                        if already_attached {
                            continue;
                        }
                        let short_name = short_component_name(&type_path, Some(registry));
                        // Match on the short name -- what the user actually
                        // sees -- not the full type_path.
                        //
                        // The `is_empty` guard is an optimization, not a
                        // correctness condition: `str::contains("")` is
                        // always true, so an empty needle could never
                        // `continue` anyway. What it buys is skipping a
                        // per-row `to_lowercase` allocation every frame in
                        // the common case where nothing has been typed.
                        // Deleting it as redundant would silently
                        // reintroduce that cost.
                        if !needle.is_empty() && !short_name.to_lowercase().contains(&needle) {
                            continue;
                        }
                        listed_any = true;
                        if ui.selectable_label(false, &short_name).clicked() {
                            to_attach = Some(type_path);
                        }
                    }
                    // Without this the popup is a search box over blank
                    // space, which reads as a rendering failure rather than
                    // as an answer. Dimmed and drawn as a plain label, so it
                    // is visibly not a row you can click.
                    //
                    // `TEXT_DIM` is the palette's own "placeholder /
                    // disabled text" entry (theme.rs), reached the way the
                    // component headers above reach `TEXT` -- via
                    // `colored_label`, since this workspace's panels take
                    // their colours from that one table rather than from
                    // egui's derived weak-text grey.
                    if !listed_any {
                        ui.colored_label(
                            crate::theme::TEXT_DIM,
                            if needle.is_empty() {
                                ALL_ATTACHED_LABEL
                            } else {
                                NO_MATCHES_LABEL
                            },
                        );
                    }
                });
        },
    );

    if to_attach.is_some() {
        ui.memory_mut(|m| m.close_popup());
    }
    to_attach
}

/// Which side of `button_rect` the Add Component picker opens on: the side
/// with more room against `screen`, ties going to `Above`.
///
/// The popup's `Area` clamps itself to `ctx.screen_rect()` (`area.rs`
/// defaults `constrain: true`), so one that does not fit on the side it was
/// told to use gets pushed back across the button, covering it and
/// everything past it. Both fixed choices fail that way at their own edge,
/// which is why the side is measured from the geometry instead of being
/// written into the source. It is measured once per opening, not once per
/// frame -- see the `insert_temp` that latches it in `draw_add_component`.
///
/// **What this works out to in practice is not what the tie rule suggests.**
/// The button's y is set by the component list, not by the window: it lands
/// at roughly 59, 152, 245 and 338 px for zero through three components, and
/// does not move as the window grows. So on any real editor window -- 720px
/// tall and up -- `space_below` wins for every entity anyone will select,
/// and the picker opens *downward*. That is the opposite of the fixed
/// `Above` that PR #1729 shipped, and it is correct: there is genuinely more
/// room below, and the ~290px popup fits in it. `Above` is now the rare
/// case, reached only when the button sits past the screen's midpoint --
/// a window around 400px tall, or a short docked Inspector.
///
/// One consequence worth knowing when reading the tests: the `Above` branch
/// is only reachable in a geometry that does not occur at production window
/// sizes, so `picker_opens_above_a_button_that_sits_low` is a valid unit
/// test of this rule but does not guard the common case.
///
/// This is also only a best effort, because it compares free space without
/// knowing what has to fit in it: `popup_above_or_below_widget` never
/// exposes the popup's measured height. When neither side has room the
/// popup is still clamped -- choosing the larger side minimises the overlap
/// rather than removing it. The 400px test screen with two components is
/// exactly that: `Above` wins with 245px of room and is clamped anyway.
fn picker_direction(screen: egui::Rect, button_rect: egui::Rect) -> egui::AboveOrBelow {
    let space_above = button_rect.top() - screen.top();
    let space_below = screen.bottom() - button_rect.bottom();
    if space_above >= space_below {
        egui::AboveOrBelow::Above
    } else {
        egui::AboveOrBelow::Below
    }
}

/// The persistent id of one component block's collapsing header.
///
/// Derived from the type path alone rather than via
/// `ui.make_persistent_id`, which mixes in the enclosing `Ui`'s own id:
/// the panel body now sits inside a `ScrollArea`, whose child `Ui`
/// generates an id of its own, so a `Ui`-relative id would silently change
/// whenever the surrounding container structure changes. That would both
/// reset every header's expanded/collapsed state and break the tests that
/// verify which components rendered, since they look this id up in egui's
/// memory from outside the panel. The id is process-global, so what makes
/// it unique is not per-entity attachment but that `dock.rs` builds exactly
/// one `InspectorPanel` (an `or_insert_with` keyed by panel id) -- no
/// second panel can render a competing header for the same type. The
/// tuple's constant prefix keeps it from colliding with any other id built
/// from the same string.
fn component_header_id(type_path: &str) -> egui::Id {
    egui::Id::new(("inspector_component_header", type_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{InspectorEntityInfo, InspectorState};

    /// Headless single-frame egui harness, mirroring `reflect_ui.rs`'s own
    /// `with_test_ui` helper.
    fn with_test_ui<R>(add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::empty());
        let mut add_contents = Some(add_contents);
        let mut result = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if let Some(f) = add_contents.take() {
                    result = Some(f(ui));
                }
            });
        });
        result.expect("add_contents must run exactly once per test frame")
    }

    /// Returns every literal string egui actually rendered as text in one
    /// frame's output shapes, paired with the position it was drawn at.
    ///
    /// Walking the output shapes is how these tests assert on rendered
    /// content directly (e.g. "the header shows 'Transform', never the
    /// full type_path") without egui's `accesskit` feature, which this
    /// workspace does not enable. `Shape::Text`'s `galley.text()` gives
    /// the exact rendered string; `Shape::Vec` nests and must be walked
    /// recursively.
    ///
    /// The position lets tests assert on vertical ordering and, more
    /// usefully, derive click coordinates from where a widget *actually*
    /// rendered this frame instead of hardcoding pixel positions -- those
    /// had to be re-measured by hand three times over the course of
    /// PR #1727 whenever a section moved.
    ///
    /// **Click the returned `pos` as-is; do not add a half-row offset to
    /// "reach the centre".** These tests build fonts from
    /// `FontDefinitions::empty()`, which gives every galley `row_height:
    /// 0.0` (`epaint`'s `Font::new` early-returns on empty fonts) and so
    /// zero size. A button places its text at
    /// `align_size_within_rect(galley.size(), rect.shrink2(padding)).min`,
    /// and a top-down `Ui` aligns vertically with `Align::Center`, so a
    /// zero-sized galley lands on the *vertical centre* of the padded
    /// rect, not its top edge. Widgets here are only as tall as their
    /// padding, so adding an offset like `reflect_ui.rs`'s
    /// `row_half_height = 9.0` would land outside the widget entirely.
    /// For the same reason the galley carries no usable height, so a
    /// row's extent cannot be derived from it.
    ///
    /// Returns `Pos2` rather than a `Rect` deliberately:
    /// `TextShape::visual_bounding_rect()` is `galley.mesh_bounds`
    /// translated by `pos`, and with no glyphs to mesh `mesh_bounds` is
    /// `Rect::NOTHING`, whose `center()` is `NaN` -- the click-swallowing
    /// trap documented at length in `reflect_ui.rs`.
    ///
    /// Note that `ClippedShape::clip_rect` is ignored: a shape scrolled
    /// out of view inside a `ScrollArea` still reports a position, and a
    /// coordinate taken from one would mis-click.
    fn collect_rendered_texts_with_pos(
        shapes: &[egui::epaint::ClippedShape],
    ) -> Vec<(String, egui::Pos2)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                egui::Shape::Text(text_shape) => {
                    out.push((text_shape.galley.text().to_string(), text_shape.pos))
                }
                egui::Shape::Vec(nested) => {
                    for s in nested {
                        walk(s, out);
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for clipped in shapes {
            walk(&clipped.shape, &mut out);
        }
        out
    }

    /// String-only view of [`collect_rendered_texts_with_pos`], for tests
    /// that assert on rendered content without caring where it landed.
    fn collect_rendered_texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
        collect_rendered_texts_with_pos(shapes)
            .into_iter()
            .map(|(text, _)| text)
            .collect()
    }

    /// Drives `InspectorPanel::ui` frame by frame against a fixed-size
    /// screen with a type registry attached, so the Add Component picker is
    /// live. This is the setup every picker test needs, and which eight of
    /// them each spelled out in full: the same ~60 lines of `run_frame` and
    /// `click_events` closures, differing only in what was registered and
    /// what was attached.
    ///
    /// Those two are therefore the constructor's arguments. Everything the
    /// copies held in common -- the empty-font context, the 400x400 screen,
    /// the empty entity snapshot, entity 1 being the selected one -- lives
    /// here instead, so a test body shows only what that test uniquely does.
    ///
    /// The two `generic_reflected_list_can_edit_*` tests deliberately do not
    /// build on this. They run with `type_registry: None`, so no picker
    /// exists at all, and drive keyboard focus rather than the pointer;
    /// folding them in would mean parameterising away the very things that
    /// make them different tests.
    struct PickerHarness {
        egui_ctx: egui::Context,
        screen_rect: egui::Rect,
        registry: bevy_reflect::TypeRegistry,
        insp: InspectorState,
        entities_snapshot: Vec<InspectorEntityInfo>,
        panel: InspectorPanel,
    }

    /// The pair of frames [`PickerHarness::open_picker`] captures.
    struct PickerFrames {
        /// The frame drawn with the picker still closed, before the button
        /// was clicked.
        ///
        /// Kept rather than discarded because several assertions are only
        /// sound as a *difference* against it: an attached component draws
        /// its own header, so its short name is legitimately on screen with
        /// the picker open or shut, and only the change between these two
        /// frames says whether the picker offered it as a row.
        closed: egui::FullOutput,
        /// The settle frame -- the first one that paints the popup's own
        /// content. See [`PickerHarness::open_picker`].
        open: egui::FullOutput,
    }

    impl PickerHarness {
        /// `registry` decides what the picker can offer; `attached` is what
        /// the selected entity already has, which is both what the
        /// component list renders and what the picker filters out.
        fn new(
            registry: bevy_reflect::TypeRegistry,
            attached: Vec<(String, Box<dyn bevy_reflect::Reflect>)>,
        ) -> Self {
            let egui_ctx = egui::Context::default();
            egui_ctx.set_fonts(egui::FontDefinitions::empty());

            let mut insp = InspectorState::default();
            insp.selected_id = Some(1);
            insp.reflected_components = attached;

            Self {
                egui_ctx,
                screen_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0)),
                registry,
                insp,
                entities_snapshot: Vec::new(),
                panel: InspectorPanel,
            }
        }

        /// The screen the panel is laid out in, so a test can phrase a
        /// position claim relative to it ("in the lower half") instead of
        /// naming a pixel.
        fn screen_rect(&self) -> egui::Rect {
            self.screen_rect
        }

        /// Resizes the screen for every later frame, i.e. the user dragging
        /// the editor window's edge.
        ///
        /// Only interesting between frames: it moves the button relative to
        /// the screen's midpoint without moving it in the panel, which is
        /// how a test can change the answer the direction rule would give
        /// while a popup is already open.
        fn set_screen_rect(&mut self, screen_rect: egui::Rect) {
            self.screen_rect = screen_rect;
        }

        /// Runs one frame with `events` delivered to it.
        fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
            self.egui_ctx.run(
                egui::RawInput {
                    screen_rect: Some(self.screen_rect),
                    events,
                    ..Default::default()
                },
                |egui_ctx| {
                    egui::CentralPanel::default().show(egui_ctx, |ui| {
                        let mut ctx = EditorPanelContext {
                            insp: &mut self.insp,
                            entities_snapshot: &self.entities_snapshot,
                            cursor_pos: (0.0, 0.0),
                            type_registry: Some(&self.registry),
                        };
                        self.panel.ui(ui, &mut ctx);
                    });
                },
            )
        }

        /// Runs one frame with no input.
        fn draw(&mut self) -> egui::FullOutput {
            self.frame(Vec::new())
        }

        /// Runs one frame carrying a full primary-button click at `pos`.
        ///
        /// `pos` must come from [`collect_rendered_texts_with_pos`] and be
        /// used exactly as returned -- see that fn's doc comment for why
        /// adding a half-row offset to "reach the centre" lands outside the
        /// widget in this zero-sized-galley harness.
        fn click(&mut self, pos: egui::Pos2) -> egui::FullOutput {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
            ])
        }

        /// Runs one frame delivering `text` as typed input, which reaches
        /// whichever widget currently holds focus.
        fn type_text(&mut self, text: &str) -> egui::FullOutput {
            self.frame(vec![egui::Event::Text(text.to_string())])
        }

        /// Opens the picker the way a user does -- draw, click the Add
        /// Component button, let the popup settle -- and returns the frame
        /// before the click alongside the first frame that actually paints
        /// the popup.
        ///
        /// Three details here are the reason this sequence is centralised
        /// rather than copied, since each is silently wrong in a way no
        /// assertion points at directly:
        ///
        /// - **The click coordinate is read out of the render**, never
        ///   hardcoded. Hand-measured constants had to be re-measured three
        ///   times over PR #1727 as sections moved; this instead asks the
        ///   frame where the button's label actually landed, which also
        ///   tracks the button's x as the panel width changes (it is
        ///   centred).
        /// - **`contains`, not `==`.** The button's galley is the PLUS icon
        ///   glyph followed by the words, so it never equals
        ///   "Add Component" exactly.
        /// - **The settle frame is mandatory.** A popup's first frame sizes
        ///   its `Area` from a placeholder and paints none of its content;
        ///   the frame after it is the first with real rows. Confirmed
        ///   empirically, not just reasoned: capturing that opening frame's
        ///   own output finds no row text at all -- not even a full
        ///   type_path -- so a test reading it could not tell "the picker
        ///   rendered the wrong thing" from "the picker never opened".
        ///   `reflect_ui.rs` carries the same note for its own combo-box
        ///   popup.
        fn open_picker(&mut self) -> PickerFrames {
            let closed = self.draw();
            let button_pos = collect_rendered_texts_with_pos(&closed.shapes)
                .into_iter()
                .find(|(text, _)| text.contains("Add Component"))
                .map(|(_, pos)| pos)
                .expect("the Add Component button must render");
            self.click(button_pos);
            let open = self.draw();
            PickerFrames { closed, open }
        }
    }

    /// Counts how many of `texts` are exactly `needle`.
    ///
    /// Counts rather than a set membership test, because the interesting
    /// question is usually how many times a name rendered, not whether it
    /// did: with the already-attached filter regressed, an attached type
    /// renders twice on an open frame (its component header, plus the picker
    /// row that should not exist), and a set difference against the closed
    /// frame would cancel the extra against the header and pass.
    fn occurrences(texts: &[String], needle: &str) -> usize {
        texts.iter().filter(|text| text.as_str() == needle).count()
    }

    #[test]
    fn component_header_shows_short_type_name_not_full_path() {
        // The header used to render `type_path.as_str()` directly (e.g.
        // "bsengine_core::transform::Transform"). It must now render only
        // the short name ("Transform"), via short_component_name -- verified
        // here by walking the actual rendered egui shapes (not just calling
        // short_component_name directly in isolation), so this would catch
        // a regression back to raw type_path display.
        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        insp.reflected_components = vec![(
            "bsengine_core::transform::Transform".to_string(),
            Box::new(bsengine_core::Transform::default()) as Box<dyn bevy_reflect::Reflect>,
        )];

        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::Transform>();

        let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
        let mut panel = InspectorPanel;

        let egui_ctx = egui::Context::default();
        egui_ctx.set_fonts(egui::FontDefinitions::empty());
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));

        let full_output = egui_ctx.run(
            egui::RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            },
            |egui_ctx| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    let mut ctx = EditorPanelContext {
                        insp: &mut insp,
                        entities_snapshot: &entities_snapshot,
                        cursor_pos: (0.0, 0.0),
                        type_registry: Some(&registry),
                    };
                    panel.ui(ui, &mut ctx);
                });
            },
        );

        let rendered_texts = collect_rendered_texts(&full_output.shapes);

        assert!(
            rendered_texts.iter().any(|t| t == "Transform"),
            "expected the short name \"Transform\" among rendered texts, got: {rendered_texts:?}"
        );
        assert!(
            !rendered_texts
                .iter()
                .any(|t| t.contains("bsengine_core::transform::Transform")),
            "the full, namespace-qualified type_path must never be rendered as visible text, \
             got: {rendered_texts:?}"
        );
    }

    #[test]
    fn reflected_fields_section_renders_without_panicking_for_a_real_camera_clone() {
        // The manual smoke test (launching the editor with no entity
        // selected) never exercises this panel's component-list branch
        // at all, since it's gated on `has_visible_components` -- the
        // non-hidden subset of `reflected_components` -- and an empty scene
        // has nothing selected. This test closes that gap
        // by feeding the panel a real, populated `reflected_components`
        // entry (mirroring what `populate_reflected_component_snapshot`
        // would produce for a selected Camera) and confirming the whole
        // `InspectorPanel::ui()` call — not just `draw_reflect_ui` in
        // isolation, which already has its own unit tests — renders one
        // frame without panicking.
        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        insp.reflected_components = vec![(
            "bsengine_core::camera::Camera".to_string(),
            Box::new(bsengine_core::Camera::default()) as Box<dyn bevy_reflect::Reflect>,
        )];

        let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
        let mut panel = InspectorPanel;

        with_test_ui(|ui| {
            let mut ctx = EditorPanelContext {
                insp: &mut insp,
                entities_snapshot: &entities_snapshot,
                cursor_pos: (0.0, 0.0),
                type_registry: None,
            };
            panel.ui(ui, &mut ctx);
        });

        // No synthetic pointer input was injected (headless, single frame,
        // no drag/click simulated), so nothing should have been pushed to
        // the command queue — this test's purpose is proving the render
        // path is panic-free with real data, not exercising the edit path
        // (already covered end-to-end by the backend's
        // reflect_command_apply_component_value_mutates_attached_component
        // and inspector_cmd_apply_reflected_component_reaches_reflect_queue
        // tests in bsengine-editor).
        assert!(insp.cmd_queue.is_empty());
    }

    #[test]
    fn reflected_fields_section_renders_without_panicking_for_the_pr1_batch() {
        // Same rationale as the Camera-only test above (avoid the gap where
        // a manual "launch with nothing selected" smoke test never exercises
        // this branch at all) but covering all 14 components added in PR 1
        // of the bevy_reflect remaining-components work in a single frame,
        // rather than one near-identical test per component. If this panics,
        // remove entries from the Vec below one at a time to bisect which
        // component's generic field rendering is at fault.
        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        insp.reflected_components = vec![
            (
                "bsengine_core::ambient_occlusion::AmbientOcclusion".to_string(),
                Box::new(bsengine_core::AmbientOcclusion::default())
                    as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::animation_player::AnimationPlayer".to_string(),
                Box::new(bsengine_core::AnimationPlayer::default())
                    as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::bloom::Bloom".to_string(),
                Box::new(bsengine_core::Bloom::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::custom_shader::CustomShader".to_string(),
                Box::new(bsengine_core::CustomShader::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::damping::Damping".to_string(),
                Box::new(bsengine_core::Damping::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::gravity::GravityScale".to_string(),
                Box::new(bsengine_core::GravityScale::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::lifetime::Lifetime".to_string(),
                Box::new(bsengine_core::Lifetime::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::mass::Mass".to_string(),
                Box::new(bsengine_core::Mass::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::network_id::NetworkId".to_string(),
                Box::new(bsengine_core::NetworkId::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::shield::Shield".to_string(),
                Box::new(bsengine_core::Shield::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::skybox::Skybox".to_string(),
                Box::new(bsengine_core::Skybox::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::timer::Timer".to_string(),
                Box::new(bsengine_core::Timer::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::tone_map::ToneMap".to_string(),
                Box::new(bsengine_core::ToneMap::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::visible::Visible".to_string(),
                Box::new(bsengine_core::Visible::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
        ];

        let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
        let mut panel = InspectorPanel;

        with_test_ui(|ui| {
            let mut ctx = EditorPanelContext {
                insp: &mut insp,
                entities_snapshot: &entities_snapshot,
                cursor_pos: (0.0, 0.0),
                type_registry: None,
            };
            panel.ui(ui, &mut ctx);
        });

        assert!(insp.cmd_queue.is_empty());
    }

    #[test]
    fn reflected_fields_section_renders_without_panicking_for_the_pr2_batch() {
        // Same rationale as the PR1 batch test. Follow/LookAt are included
        // here even though they have no ReflectDefault (so they'd never
        // appear via the Inspector's own Add Component flow) -- this test
        // exercises the read/render path directly with a hand-constructed
        // instance instead, to prove the generic field renderer handles an
        // Entity field (via new(Entity::PLACEHOLDER)) without panicking.
        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        insp.reflected_components = vec![
            (
                "bsengine_core::angular_velocity::AngularVelocity".to_string(),
                Box::new(bsengine_core::AngularVelocity::default())
                    as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::external_impulse::ExternalImpulse".to_string(),
                Box::new(bsengine_core::ExternalImpulse::default())
                    as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::follow::Follow".to_string(),
                Box::new(bsengine_core::Follow::new(
                    bevy_ecs::prelude::Entity::PLACEHOLDER,
                )) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::follow::LookAt".to_string(),
                Box::new(bsengine_core::LookAt::new(
                    bevy_ecs::prelude::Entity::PLACEHOLDER,
                )) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::nav_mesh_agent::NavMeshAgent".to_string(),
                Box::new(bsengine_core::NavMeshAgent::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::velocity::Velocity".to_string(),
                Box::new(bsengine_core::Velocity::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
        ];

        let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
        let mut panel = InspectorPanel;

        with_test_ui(|ui| {
            let mut ctx = EditorPanelContext {
                insp: &mut insp,
                entities_snapshot: &entities_snapshot,
                cursor_pos: (0.0, 0.0),
                type_registry: None,
            };
            panel.ui(ui, &mut ctx);
        });

        assert!(insp.cmd_queue.is_empty());
    }

    #[test]
    fn reflected_fields_section_renders_without_panicking_for_the_pr3_batch() {
        // Same rationale as the PR1/PR2 batch tests. Parent and Tween have no
        // ReflectDefault (Parent needs an Entity with no sensible default;
        // Tween needs a TweenTarget with no natural default variant) -- both
        // are hand-constructed here to exercise the read/render path directly,
        // including a Mat4 field (GlobalTransform) and an enum-with-glam-fields
        // field (Tween's TweenTarget) without panicking.
        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        insp.reflected_components = vec![
            (
                "bsengine_core::transform::Transform".to_string(),
                Box::new(bsengine_core::Transform::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::global_transform::GlobalTransform".to_string(),
                Box::new(bsengine_core::GlobalTransform::default())
                    as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::parent::Parent".to_string(),
                Box::new(bsengine_core::Parent(
                    bevy_ecs::prelude::Entity::PLACEHOLDER,
                )) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::animation_state_machine::AnimationStateMachine".to_string(),
                Box::new(bsengine_core::AnimationStateMachine::default())
                    as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::tween::Tween".to_string(),
                Box::new(bsengine_core::Tween::new(
                    bsengine_core::TweenTarget::Translation {
                        from: glam::Vec3::ZERO.into(),
                        to: glam::Vec3::ONE.into(),
                    },
                    1.0,
                )) as Box<dyn bevy_reflect::Reflect>,
            ),
        ];

        let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
        let mut panel = InspectorPanel;

        with_test_ui(|ui| {
            let mut ctx = EditorPanelContext {
                insp: &mut insp,
                entities_snapshot: &entities_snapshot,
                cursor_pos: (0.0, 0.0),
                type_registry: None,
            };
            panel.ui(ui, &mut ctx);
        });

        assert!(insp.cmd_queue.is_empty());
    }

    #[test]
    fn reflected_fields_list_hides_global_transform_and_visible() {
        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        insp.reflected_components = vec![
            (
                "bsengine_core::transform::Transform".to_string(),
                Box::new(bsengine_core::Transform::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::global_transform::GlobalTransform".to_string(),
                Box::new(bsengine_core::GlobalTransform::default())
                    as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_core::visible::Visible".to_string(),
                Box::new(bsengine_core::Visible::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
        ];

        let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
        let mut panel = InspectorPanel;
        let shown_type_paths = with_test_ui(|ui| {
            let mut ctx = EditorPanelContext {
                insp: &mut insp,
                entities_snapshot: &entities_snapshot,
                cursor_pos: (0.0, 0.0),
                type_registry: None,
            };
            panel.ui(ui, &mut ctx);
            // Re-derive which type paths actually rendered a collapsible
            // header by checking egui's per-id "open" memory state -- each
            // reflected entry's header uses `component_header_id(type_path)`
            // as its collapsing header id (see the production code above),
            // so a header genuinely rendered iff that persistent id has
            // recorded open/closed state in memory.
            [
                "bsengine_core::transform::Transform",
                "bsengine_core::global_transform::GlobalTransform",
                "bsengine_core::visible::Visible",
            ]
            .into_iter()
            .filter(|type_path| {
                let id = component_header_id(type_path);
                egui::containers::collapsing_header::CollapsingState::load(ui.ctx(), id).is_some()
            })
            .collect::<Vec<_>>()
        });

        assert_eq!(
            shown_type_paths,
            vec!["bsengine_core::transform::Transform"],
            "GlobalTransform (derived, no lasting effect if edited) and Visible (already \
             shown as the header checkbox) must not also render as Reflected Fields entries -- \
             only Transform, which has neither exclusion, should show"
        );
    }

    #[test]
    fn validate_after_edit_clamps_an_out_of_range_spot_light() {
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::SpotLight>();

        let mut sl = bsengine_core::SpotLight {
            inner_angle_degrees: 60.0.into(),
            outer_angle_degrees: 20.0.into(),
            ..bsengine_core::SpotLight::default()
        };
        let as_reflect: &mut dyn bevy_reflect::Reflect = &mut sl;

        super::validate_after_edit(
            "bsengine_core::light::SpotLight",
            as_reflect,
            Some(&registry),
        );

        assert!(
            (sl.inner_angle_degrees.0 - 20.0).abs() < 1e-6,
            "inner should have been clamped down to outer via the generic Validate hook"
        );
    }

    #[test]
    fn add_component_picker_click_only_offers_the_not_yet_attached_type() {
        // Regression test that genuinely drives `InspectorPanel::ui()`'s Add
        // Component picker, mirroring the click-simulation technique in
        // `reflect_ui.rs`'s `enum_variant_combo_switches_to_a_default_instance_
        // of_the_chosen_variant` test (open the popup, settle frame, click a
        // row) adapted to `panel.ui(ui, &mut ctx)`'s call shape instead of
        // `draw_reflect_ui(ui, value, &ctx)`.
        //
        // Camera is registered AND already attached (in reflected_components);
        // PointLight is registered and NOT attached. With the real
        // `already_attached` filter in place, the picker has exactly one
        // candidate row (PointLight) -- clicking it must queue
        // `AttachComponentByType` for PointLight, never Camera. If the filter
        // were a no-op, Camera would also be offered as a row -- caught below
        // by asserting on the open popup's rendered row text directly (the
        // invariant itself) as well as on the queued command's type_path.
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::Camera>();
        registry.register::<bsengine_core::PointLight>();

        let mut harness = PickerHarness::new(
            registry,
            vec![(
                "bsengine_core::camera::Camera".to_string(),
                Box::new(bsengine_core::Camera::default()) as Box<dyn bevy_reflect::Reflect>,
            )],
        );

        let frames = harness.open_picker();

        // The filter's actual effect, asserted directly rather than probed
        // by clicking where a filtered-out row would have been: opening the
        // picker must add a PointLight row and no Camera row. Checking the
        // render can't fail open the way a hand-measured "click empty space
        // and expect nothing" coordinate can.
        //
        // This compares the closed frame against the open one instead of
        // just scanning the open one, because "Camera" is legitimately on
        // screen either way: it is already attached, so the component list
        // above the button draws its component header (and its fields)
        // every frame. A bare `!open.contains("Camera")` would fire on that
        // header and fail a correct build. What the filter guarantees is
        // narrower -- that *opening the picker contributes* no Camera row.
        //
        // The PointLight assertion doubles as the vacuity guard for the
        // Camera one: if the picker ever stops opening, PointLight's +1
        // fails first, so "Camera gained no row" can never pass merely
        // because nothing rendered.
        //
        // These hold every string the frame rendered -- entity heading,
        // "Visible", field labels, the attached component's header -- not
        // just picker rows.
        let closed_texts = collect_rendered_texts(&frames.closed.shapes);
        let open_texts = collect_rendered_texts(&frames.open.shapes);
        assert_eq!(
            occurrences(&open_texts, "PointLight"),
            occurrences(&closed_texts, "PointLight") + 1,
            "opening the picker must add exactly one PointLight row -- it is registered and \
             not yet attached, so it must be offered. closed: {closed_texts:?}, open: \
             {open_texts:?}"
        );
        assert_eq!(
            occurrences(&open_texts, "Camera"),
            occurrences(&closed_texts, "Camera"),
            "Camera is already attached, so opening the picker must add no Camera row -- an \
             extra one here means the already_attached filter regressed. closed: \
             {closed_texts:?}, open: {open_texts:?}"
        );

        // Click the picker's only row, at the position the settle frame --
        // the first one to paint rows at all -- says it rendered at.
        let point_light_row_pos = collect_rendered_texts_with_pos(&frames.open.shapes)
            .into_iter()
            .find(|(text, _)| text == "PointLight")
            .map(|(_, pos)| pos)
            .expect("the popup's PointLight row must render on the settle frame");
        harness.click(point_light_row_pos);

        assert_eq!(
            harness.insp.cmd_queue.len(),
            1,
            "clicking the popup's only row should queue exactly one attach command; \
             a queue of 0 means the click missed"
        );
        match &harness.insp.cmd_queue[0] {
            InspectorCmd::AttachComponentByType { id, type_path } => {
                assert_eq!(*id, 1);
                assert_eq!(
                    type_path, "bsengine_core::light::PointLight",
                    "the only clickable row must be PointLight -- Camera is already \
                     attached and must never be offered again"
                );
            }
            other => panic!(
                "expected AttachComponentByType, got a different InspectorCmd variant instead: {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn add_component_picker_shows_short_names_not_full_type_paths() {
        // Mirrors add_component_picker_click_only_offers_the_not_yet_attached_type's
        // setup (Camera already attached, PointLight registered and not
        // attached) but checks *rendered text* after opening the picker,
        // rather than the resulting command -- this test would fail if the
        // picker's selectable rows went back to showing the raw type_path.
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::Camera>();
        registry.register::<bsengine_core::PointLight>();

        let mut harness = PickerHarness::new(
            registry,
            vec![(
                "bsengine_core::camera::Camera".to_string(),
                Box::new(bsengine_core::Camera::default()) as Box<dyn bevy_reflect::Reflect>,
            )],
        );

        // The settle frame is the one that carries the rows -- see
        // `PickerHarness::open_picker`, whose doc records the empirical
        // check behind that: the opening frame paints no row text at all,
        // not even a full type_path, so reading it could not tell a
        // regressed row label from a popup that never opened.
        let rendered_texts = collect_rendered_texts(&harness.open_picker().open.shapes);

        assert!(
            rendered_texts.iter().any(|t| t == "PointLight"),
            "expected the short name \"PointLight\" among rendered texts once the popup is \
             open, got: {rendered_texts:?}"
        );
        assert!(
            !rendered_texts
                .iter()
                .any(|t| t.contains("bsengine_core::light::PointLight")),
            "the full, namespace-qualified type_path must never be rendered as visible text \
             in the picker, got: {rendered_texts:?}"
        );
    }

    #[test]
    fn add_component_button_renders_below_the_component_list() {
        // Unity puts Add Component at the very bottom, after every
        // component. Verified positionally against the real render rather
        // than by assuming source order, since the two are only the same
        // if the panel actually lays out top-down as intended.
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::Transform>();

        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        insp.reflected_components = vec![(
            "bsengine_core::transform::Transform".to_string(),
            Box::new(bsengine_core::Transform::default()) as Box<dyn bevy_reflect::Reflect>,
        )];

        let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
        let mut panel = InspectorPanel;

        let egui_ctx = egui::Context::default();
        egui_ctx.set_fonts(egui::FontDefinitions::empty());
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));

        let full_output = egui_ctx.run(
            egui::RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            },
            |egui_ctx| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    let mut ctx = EditorPanelContext {
                        insp: &mut insp,
                        entities_snapshot: &entities_snapshot,
                        cursor_pos: (0.0, 0.0),
                        type_registry: Some(&registry),
                    };
                    panel.ui(ui, &mut ctx);
                });
            },
        );

        let texts = collect_rendered_texts_with_pos(&full_output.shapes);
        let transform_y = texts
            .iter()
            .find(|(text, _)| text == "Transform")
            .map(|(_, pos)| pos.y)
            .expect("the Transform component's header must render");
        // `contains`, not `==`: the button's label is the PLUS icon glyph
        // followed by the words, so the galley text isn't exactly
        // "Add Component".
        let button_y = texts
            .iter()
            .find(|(text, _)| text.contains("Add Component"))
            .map(|(_, pos)| pos.y)
            .expect("the Add Component button must render");

        assert!(
            button_y > transform_y,
            "Add Component must render below the component list, got button y={button_y} \
             vs Transform y={transform_y}"
        );
    }

    #[test]
    fn component_picker_only_appears_after_clicking_the_button() {
        // The old UI was an always-visible ComboBox. Unity shows a plain
        // button and only reveals the picker on click -- so before any
        // click, no component-type name may render at all.
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::PointLight>();

        let mut harness = PickerHarness::new(registry, Vec::new());

        let frames = harness.open_picker();

        // The frame before the click: the button shows, the picker doesn't.
        let closed_texts = collect_rendered_texts_with_pos(&frames.closed.shapes);
        assert!(
            closed_texts
                .iter()
                .any(|(text, _)| text.contains("Add Component")),
            "the Add Component button must render, got: {closed_texts:?}"
        );
        assert!(
            !closed_texts.iter().any(|(text, _)| text == "PointLight"),
            "no component-type name may render before the button is clicked, got: \
             {closed_texts:?}"
        );

        let opened_texts = collect_rendered_texts(&frames.open.shapes);

        assert!(
            opened_texts.iter().any(|text| text == "PointLight"),
            "clicking the button must reveal the picker's rows, got: {opened_texts:?}"
        );
    }

    #[test]
    fn picker_search_field_filters_the_component_list() {
        // Unity's Add Component picker has a search box. Typing must narrow
        // the list by short name -- the text the user actually sees -- and
        // must not dismiss the popup (which is why the popup uses
        // CloseOnClickOutside rather than ComboBox's CloseOnClick).
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::PointLight>();
        registry.register::<bsengine_core::Camera>();

        let mut harness = PickerHarness::new(registry, Vec::new());

        let opened = harness.open_picker().open;
        let unfiltered = collect_rendered_texts(&opened.shapes);
        assert!(
            unfiltered.iter().any(|text| text == "PointLight")
                && unfiltered.iter().any(|text| text == "Camera"),
            "both types must be listed before filtering, got: {unfiltered:?}"
        );

        // Click the search field, located by its hint text. An empty
        // TextEdit paints no content of its own, so the hint is the only
        // thing that marks where the field is -- which is exactly why the
        // implementation gives it one (Unity's picker shows a placeholder
        // too). Deriving the position from a neighbouring row instead
        // would mean guessing a row height.
        let search_pos = collect_rendered_texts_with_pos(&opened.shapes)
            .into_iter()
            .find(|(text, _)| text == SEARCH_HINT)
            .map(|(_, pos)| pos)
            .expect("the search field's hint text must render while the picker is open");
        harness.click(search_pos);
        harness.type_text("point");
        let filtered = collect_rendered_texts(&harness.draw().shapes);

        assert!(
            filtered.iter().any(|text| text == "PointLight"),
            "a case-insensitive match on the short name must survive filtering, got: \
             {filtered:?}"
        );
        assert!(
            !filtered.iter().any(|text| text == "Camera"),
            "a non-matching type must be filtered out, got: {filtered:?}"
        );
    }

    #[test]
    fn picker_search_field_is_focused_on_open() {
        // Unity focuses the picker's search box the moment it opens, so you
        // can type straight away. Proven by never clicking the field: the
        // text events below go nowhere unless opening the popup granted it
        // focus on its own.
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::PointLight>();
        registry.register::<bsengine_core::Camera>();

        let mut harness = PickerHarness::new(registry, Vec::new());

        let opened = harness.open_picker().open;
        let unfiltered = collect_rendered_texts(&opened.shapes);
        assert!(
            unfiltered.iter().any(|text| text == "PointLight")
                && unfiltered.iter().any(|text| text == "Camera"),
            "both types must be listed before typing, got: {unfiltered:?}"
        );

        // Type without ever clicking the search field. This is the whole
        // point of the test -- the sibling test clicks it first, which would
        // mask a missing autofocus.
        harness.type_text("point");
        let filtered = collect_rendered_texts(&harness.draw().shapes);

        // `Camera` vanishing is the assertion that actually proves focus:
        // with no focus the keystrokes go nowhere, nothing filters, and
        // both types stay listed. It therefore carries the explanation.
        // The `PointLight` check below cannot fail that way -- it renders
        // either way -- so it only guards against over-filtering.
        assert!(
            !filtered.iter().any(|text| text == "Camera"),
            "typing straight after opening the picker, with no click on the search field, \
             must filter the list -- so the field was never focused on open; got: {filtered:?}"
        );
        assert!(
            filtered.iter().any(|text| text == "PointLight"),
            "the matching type must survive filtering, got: {filtered:?}"
        );
    }

    #[test]
    fn generic_reflected_list_can_edit_transform_translation() {
        // Proves the generic Reflected Fields list can perform the same
        // edit the hardcoded Transform DragValues used to, via a real
        // keyboard interaction through the actual InspectorPanel::ui() --
        // not just a render-without-panicking smoke test. Reuses the
        // Tab-to-focus + ArrowUp technique from reflect_ui.rs's
        // `reflect_quat_leaf_edits_only_the_dragged_euler_axis_via_keyboard`
        // test (Phase 1, Task 5): egui's Memory::interested_in_focus grants
        // focus to the first focus-wanting widget on Tab when nothing is
        // focused yet, and each further Tab in its own frame (Focus::
        // begin_pass/end_pass processes at most one FocusDirection step per
        // frame, so multiple Tab events queued in a single frame do not
        // stack) advances focus to the next one; DragValue reads
        // ArrowUp/ArrowDown directly while keyboard-focused, bumping its
        // bound value by `speed` per press.
        //
        // Unlike that isolated reflect_ui.rs test -- which calls
        // draw_reflect_ui directly on a bare ReflectQuat with nothing else
        // in the UI tree, so the very first Tab reaches the first DragValue
        // -- this test drives the *whole* InspectorPanel::ui(), which draws
        // several other focusable (`Sense::click()`, which sets
        // `focusable: true` -- see egui's sense.rs) widgets before the
        // Reflected Fields list: the Visible checkbox, then this Transform
        // entry's own collapsing-header toggle button and "..." menu button
        // (both added by CollapsingState::show_header itself, not just its
        // closure). That's 3 focusable widgets ahead of translation.x's
        // DragValue, so 4 Tab presses (one per frame) are needed, not 1.
        // This was confirmed empirically with a throwaway diagnostic test
        // that swept tab_count from 0 to 14 and printed the resulting
        // queued command after 3x ArrowUp at each count: queue_len stayed 0
        // through tab_count=3, became 1 at tab_count=4 with translation
        // moved to (0.15, 0, 0) and rotation/scale untouched, then 5/6 hit
        // translation.y/z, and 7+ found no more focus-wanting widgets (or
        // hit rotation's raw quaternion components, which ArrowUp doesn't
        // move the same way), until tab_count=13 wrapped focus back to the
        // start of the chain and queue_len returned to 0.
        //
        // (Prior to Task 3 removing the hardcoded Tags section's new-tag
        // text edit + "Add" button -- 2 focusable widgets -- this count was
        // 9, dropping to 7 as a direct consequence of that removal; Task 4
        // removing the hardcoded Script section's text edit + "Attach"
        // button -- 2 more focusable widgets -- dropped it again, from 7
        // to 5; Task 5 removing the hardcoded Mesh section's primitive
        // combo box -- 1 more focusable widget -- dropped it again, from 5
        // to 4.)
        //
        // Separately, `value` here is NOT a concrete `Transform`: derived
        // `Reflect` structs' `clone_value()` (see bevy_reflect_derive's
        // `impls/structs.rs`) returns `Box::new(Struct::clone_dynamic(self))`
        // -- a `DynamicStruct`, not `Box<Self>` -- so `downcast_ref::
        // <Transform>()` on the queued value always returns `None`. This
        // mirrors exactly how production applies it too: `apply_inspector_
        // cmds` (bsengine-editor/src/plugin.rs) routes `value` through
        // `ReflectComponent::apply_or_insert`, not a downcast. This test
        // does the same by patching a fresh `Transform::default()` via
        // `Reflect::apply`.
        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        insp.reflected_components = vec![(
            "bsengine_core::transform::Transform".to_string(),
            Box::new(bsengine_core::Transform::default()) as Box<dyn bevy_reflect::Reflect>,
        )];

        let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
        let mut panel = InspectorPanel;

        let egui_ctx = egui::Context::default();
        egui_ctx.set_fonts(egui::FontDefinitions::empty());
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));

        let run_frame = |egui_ctx: &egui::Context,
                         events: Vec<egui::Event>,
                         insp: &mut InspectorState,
                         entities_snapshot: &[InspectorEntityInfo],
                         panel: &mut InspectorPanel| {
            let _ = egui_ctx.run(
                egui::RawInput {
                    screen_rect: Some(screen_rect),
                    events,
                    ..Default::default()
                },
                |egui_ctx| {
                    egui::CentralPanel::default().show(egui_ctx, |ui| {
                        let mut ctx = EditorPanelContext {
                            insp,
                            entities_snapshot,
                            cursor_pos: (0.0, 0.0),
                            type_registry: None,
                        };
                        panel.ui(ui, &mut ctx);
                    });
                },
            );
        };

        // Frame 1: draw once so the widget tree/focus-interest exists.
        run_frame(&egui_ctx, vec![], &mut insp, &entities_snapshot, &mut panel);

        // Frames 2-5: one Tab per frame, walking focus through the 3
        // focusable widgets that precede translation.x (Visible checkbox;
        // Transform's collapsing-header toggle button; Transform's "..."
        // menu button) until the 4th Tab lands on translation.x's
        // DragValue -- see the empirical sweep described above.
        let tab_event = || egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        for _ in 0..4 {
            run_frame(
                &egui_ctx,
                vec![tab_event()],
                &mut insp,
                &entities_snapshot,
                &mut panel,
            );
        }

        // Final frame: 3x ArrowUp while translation.x's DragValue is
        // focused -- bumps it by 3 * speed(0.05) = 0.15.
        let arrow_up_events = (0..3)
            .flat_map(|_| {
                vec![egui::Event::Key {
                    key: egui::Key::ArrowUp,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::default(),
                }]
            })
            .collect();
        run_frame(
            &egui_ctx,
            arrow_up_events,
            &mut insp,
            &entities_snapshot,
            &mut panel,
        );

        assert_eq!(
            insp.cmd_queue.len(),
            1,
            "editing translation.x via the generic list should queue exactly one \
             ApplyReflectedComponent command"
        );
        match &insp.cmd_queue[0] {
            InspectorCmd::ApplyReflectedComponent {
                id,
                type_path,
                value,
            } => {
                assert_eq!(*id, 1);
                assert_eq!(type_path, "bsengine_core::transform::Transform");
                // `value` is a `DynamicStruct` (see the comment above this
                // test), not a concrete `Transform` -- patch it onto a real
                // default `Transform` via `Reflect::apply`, the same
                // mechanism production uses (`ReflectComponent::
                // apply_or_insert`), to get a concrete value to assert on.
                let mut transform = bsengine_core::Transform::default();
                bevy_reflect::Reflect::apply(&mut transform, value.as_ref());
                assert!(
                    (transform.translation.0.x - 0.15).abs() < 1e-4,
                    "translation.x should have moved by 3 * speed(0.05) = 0.15, got {}",
                    transform.translation.0.x
                );
                assert_eq!(
                    transform.translation.0.y, 0.0,
                    "only translation.x was edited -- y must be untouched"
                );
                assert_eq!(
                    transform.rotation.0,
                    glam::Quat::IDENTITY,
                    "only translation.x was edited -- rotation must be untouched"
                );
            }
            other => panic!(
                "expected ApplyReflectedComponent, got a different InspectorCmd variant: {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn generic_reflected_list_can_edit_script_path() {
        // ScriptPath(String) is a tuple struct -- its TupleStruct arm in
        // draw_reflect_ui recurses directly into the String field with no
        // wrapping label (Phase 1, Task 2's design), so the String leaf's
        // `ui.text_edit_singleline` is the sole, first-and-only
        // Tab-focusable widget *for a ScriptPath entry itself*. TextEdit
        // reads pending `Event::Text(String)` events while keyboard-focused
        // and appends each to its bound buffer.
        //
        // With the hardcoded Script section removed (Task 4) and the
        // hardcoded Mesh section also removed (this task), the focusable
        // widgets preceding the ScriptPath entry's own leaf text field are:
        // the Visible checkbox(1), then this ScriptPath entry's own
        // collapsing-header toggle(2) and "..." menu button(3) (both added
        // by `CollapsingState::show_header` itself), before the 4th Tab
        // finally lands on the leaf. Confirmed empirically with a
        // throwaway diagnostic test that swept tab_count from 0 to 14:
        // queue_len was 0 through tab_count=3, became 1 at tab_count=4,
        // then 0 again through tab_count=8 before becoming 1 again at
        // tab_count=9 (egui's focus wraps back around to the start of the
        // chain once it runs out of focus-wanting widgets, so a much larger
        // tab_count re-lands on the same leaf on a later lap; the wrap
        // period shrank from 6 Tabs to 5 as a direct consequence of
        // removing one focusable widget from the chain).
        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        insp.reflected_components = vec![(
            "bsengine_scene::types::ScriptPath".to_string(),
            Box::new(bsengine_scene::ScriptPath(String::new())) as Box<dyn bevy_reflect::Reflect>,
        )];

        let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
        let mut panel = InspectorPanel;

        let egui_ctx = egui::Context::default();
        egui_ctx.set_fonts(egui::FontDefinitions::empty());
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));

        let run_frame = |egui_ctx: &egui::Context,
                         events: Vec<egui::Event>,
                         insp: &mut InspectorState,
                         entities_snapshot: &[InspectorEntityInfo],
                         panel: &mut InspectorPanel| {
            let _ = egui_ctx.run(
                egui::RawInput {
                    screen_rect: Some(screen_rect),
                    events,
                    ..Default::default()
                },
                |egui_ctx| {
                    egui::CentralPanel::default().show(egui_ctx, |ui| {
                        let mut ctx = EditorPanelContext {
                            insp,
                            entities_snapshot,
                            cursor_pos: (0.0, 0.0),
                            type_registry: None,
                        };
                        panel.ui(ui, &mut ctx);
                    });
                },
            );
        };

        // Frame 1: draw once so the widget tree/focus-interest exists.
        run_frame(&egui_ctx, vec![], &mut insp, &entities_snapshot, &mut panel);

        // Frames 2-5: one Tab per frame, walking focus through the 3
        // focusable widgets that precede the ScriptPath entry's leaf text
        // field (see the comment above) until the 4th Tab lands on it.
        let tab_event = || egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        for _ in 0..4 {
            run_frame(
                &egui_ctx,
                vec![tab_event()],
                &mut insp,
                &entities_snapshot,
                &mut panel,
            );
        }

        // Frame 6: type "foo.js" into the focused text field.
        run_frame(
            &egui_ctx,
            vec![egui::Event::Text("foo.js".to_string())],
            &mut insp,
            &entities_snapshot,
            &mut panel,
        );

        assert_eq!(
            insp.cmd_queue.len(),
            1,
            "typing into ScriptPath's text field via the generic list should queue \
             exactly one ApplyReflectedComponent command"
        );
        match &insp.cmd_queue[0] {
            InspectorCmd::ApplyReflectedComponent {
                id,
                type_path,
                value,
            } => {
                assert_eq!(*id, 1);
                assert_eq!(type_path, "bsengine_scene::types::ScriptPath");
                // `value` is a `DynamicTupleStruct` (see the comment in
                // `generic_reflected_list_can_edit_transform_translation` for
                // why `clone_value()` on a derived `Reflect` type never
                // downcasts back to `Self`), not a concrete `ScriptPath` --
                // patch it onto a real `ScriptPath` via `Reflect::apply`, the
                // same mechanism production uses, to get a concrete value.
                let mut script_path = bsengine_scene::ScriptPath(String::new());
                bevy_reflect::Reflect::apply(&mut script_path, value.as_ref());
                assert_eq!(script_path.0, "foo.js");
            }
            other => panic!(
                "expected ApplyReflectedComponent, got a different InspectorCmd variant: {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn generic_reflected_list_shows_tags_as_an_editable_entry() {
        // Tags' List-editing mechanics (the +/x row UI) and its full
        // command-pipeline round-trip are already thoroughly tested in
        // reflect_ui.rs and bsengine-editor (Phase 1) -- this test is
        // narrower and Inspector-panel-specific: proving Tags genuinely
        // renders as an interactive entry (a real CollapsingHeader with a
        // body, not just present-but-inert) once the hardcoded Tags
        // section is gone.
        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        insp.reflected_components = vec![(
            "bsengine_editor::snapshot::Tags".to_string(),
            Box::new(bsengine_editor::snapshot::Tags(vec!["enemy".to_string()]))
                as Box<dyn bevy_reflect::Reflect>,
        )];

        let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
        let mut panel = InspectorPanel;

        let shown = with_test_ui(|ui| {
            let mut ctx = EditorPanelContext {
                insp: &mut insp,
                entities_snapshot: &entities_snapshot,
                cursor_pos: (0.0, 0.0),
                type_registry: None,
            };
            panel.ui(ui, &mut ctx);
            let id = component_header_id("bsengine_editor::snapshot::Tags");
            egui::containers::collapsing_header::CollapsingState::load(ui.ctx(), id).is_some()
        });

        assert!(
            shown,
            "Tags must render as a genuine Reflected Fields entry once the hardcoded \
             Tags section is removed"
        );
    }

    #[test]
    fn generic_reflected_list_shows_primitive_mesh_as_an_editable_entry() {
        // PrimitiveMesh's enum-variant-switching mechanics are already
        // thoroughly tested in reflect_ui.rs (Phase 1, Task 4) -- this test
        // is narrower and Inspector-panel-specific: proving PrimitiveMesh
        // genuinely renders as an interactive entry once the hardcoded Mesh
        // section is gone.
        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        insp.reflected_components = vec![(
            "bsengine_scene::types::PrimitiveMesh".to_string(),
            Box::new(bsengine_scene::PrimitiveMesh(
                bsengine_scene::Primitive::Cube,
            )) as Box<dyn bevy_reflect::Reflect>,
        )];

        let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
        let mut panel = InspectorPanel;

        let shown = with_test_ui(|ui| {
            let mut ctx = EditorPanelContext {
                insp: &mut insp,
                entities_snapshot: &entities_snapshot,
                cursor_pos: (0.0, 0.0),
                type_registry: None,
            };
            panel.ui(ui, &mut ctx);
            let id = component_header_id("bsengine_scene::types::PrimitiveMesh");
            egui::containers::collapsing_header::CollapsingState::load(ui.ctx(), id).is_some()
        });

        assert!(
            shown,
            "PrimitiveMesh must render as a genuine Reflected Fields entry once the \
             hardcoded Mesh section is removed"
        );
    }

    #[test]
    fn validate_after_edit_is_a_no_op_without_a_type_registry() {
        // Mirrors the shape of `reflected_fields_section_renders_without_panicking_for_a_
        // real_camera_clone`'s `type_registry: None` case — confirms the helper degrades
        // gracefully (no panic) rather than assuming a registry is always present.
        let mut sl = bsengine_core::SpotLight {
            inner_angle_degrees: 60.0.into(),
            outer_angle_degrees: 20.0.into(),
            ..bsengine_core::SpotLight::default()
        };
        let as_reflect: &mut dyn bevy_reflect::Reflect = &mut sl;

        super::validate_after_edit("bsengine_core::light::SpotLight", as_reflect, None);

        assert!(
            (sl.inner_angle_degrees.0 - 60.0).abs() < 1e-6,
            "with no type registry available, the value should be left untouched, not panic"
        );
    }

    #[test]
    fn reflected_fields_banner_label_is_never_rendered() {
        // The "Reflected Fields" section banner (icon + text row that used
        // to precede the component list) is being removed entirely --
        // Unity's Inspector has no equivalent wrapping label, each
        // component is just its own block directly. This is a straight
        // regression guard: the literal string must never appear in
        // rendered output again, regardless of how many components exist.
        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        insp.reflected_components = vec![
            (
                "bsengine_core::transform::Transform".to_string(),
                Box::new(bsengine_core::Transform::default()) as Box<dyn bevy_reflect::Reflect>,
            ),
            (
                "bsengine_editor::snapshot::Tags".to_string(),
                Box::new(bsengine_editor::snapshot::Tags(vec!["enemy".to_string()]))
                    as Box<dyn bevy_reflect::Reflect>,
            ),
        ];

        let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
        let mut panel = InspectorPanel;

        let egui_ctx = egui::Context::default();
        egui_ctx.set_fonts(egui::FontDefinitions::empty());
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));

        let full_output = egui_ctx.run(
            egui::RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            },
            |egui_ctx| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    let mut ctx = EditorPanelContext {
                        insp: &mut insp,
                        entities_snapshot: &entities_snapshot,
                        cursor_pos: (0.0, 0.0),
                        type_registry: None,
                    };
                    panel.ui(ui, &mut ctx);
                });
            },
        );

        let rendered_texts = collect_rendered_texts(&full_output.shapes);

        assert!(
            !rendered_texts.iter().any(|t| t == "Reflected Fields"),
            "the \"Reflected Fields\" banner must be gone entirely, got: {rendered_texts:?}"
        );
    }

    #[test]
    fn an_entity_with_only_hidden_components_renders_identically_to_no_components() {
        // Found during the final whole-branch review: the section guard
        // `if !insp.reflected_components.is_empty()` checked the *unfiltered*
        // vec, but the loop below only iterates the *filtered* (non-hidden)
        // list. An entity whose only reflected_components entries are
        // GlobalTransform/Visible (both hidden by is_hidden_reflected_type)
        // would satisfy the unfiltered emptiness check, draw the leading
        // ui.separator(), then iterate zero times -- a dangling separator
        // with nothing below it. Real-world reachability is low
        // (GlobalTransform is normally paired with a present, non-hidden
        // Transform), but nothing in the ECS structurally prevents it.
        //
        // Fixed by checking emptiness on the filtered set instead
        // (`has_visible_components`). Verified here by rendering two
        // scenarios -- reflected_components empty vs. containing only hidden
        // entries -- and asserting their rendered shape counts are
        // identical, proving the hidden-only case draws nothing extra (no
        // dangling separator) beyond what the truly-empty case draws.
        //
        // Run twice, because the predicate is consulted at two sites and
        // only one of them is still observable.
        //
        // The component-list block it originally guarded no longer opens
        // with a separator -- that rule was deleted when Add Component moved
        // to the bottom, the Visible toggle's own separator having taken
        // over the job -- and the loop inside it was always filtered. So
        // with a hidden-only entity that block now renders nothing either
        // way, and the two predicate forms are genuinely indistinguishable
        // there: mutating it changes no rendered shape and fails no test,
        // because there is nothing left to observe.
        //
        // The rule above the Add Component button is where the choice still
        // decides something, and reaching it needs a type_registry -- without
        // one the whole `if let Some(registry) = type_registry` block is
        // skipped. That is exactly how this site went uncovered until a
        // mutation check caught it: regressing it to `!is_empty()` changed
        // no test result at all. `None` keeps the original scenario on the
        // record; `Some(&registry)` is the case that actually pins the
        // invariant.
        fn render_shape_count(
            reflected_components: Vec<(String, Box<dyn bevy_reflect::Reflect>)>,
            type_registry: Option<&bevy_reflect::TypeRegistry>,
        ) -> usize {
            let mut insp = InspectorState::default();
            insp.selected_id = Some(1);
            insp.reflected_components = reflected_components;

            let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
            let mut panel = InspectorPanel;

            let egui_ctx = egui::Context::default();
            egui_ctx.set_fonts(egui::FontDefinitions::empty());
            let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));

            let full_output = egui_ctx.run(
                egui::RawInput {
                    screen_rect: Some(screen_rect),
                    ..Default::default()
                },
                |egui_ctx| {
                    egui::CentralPanel::default().show(egui_ctx, |ui| {
                        let mut ctx = EditorPanelContext {
                            insp: &mut insp,
                            entities_snapshot: &entities_snapshot,
                            cursor_pos: (0.0, 0.0),
                            type_registry,
                        };
                        panel.ui(ui, &mut ctx);
                    });
                },
            );

            full_output.shapes.len()
        }

        // Only `Some` vs `None` matters here: this test never clicks, so the
        // popup never opens and the picker closure never runs, which means
        // the registry's *contents* cannot reach the rendered shapes. What
        // `Some` buys is entry into the `if let Some(registry)` block that
        // holds the separator under test. A type is registered anyway so the
        // scenario resembles a real one.
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::PointLight>();

        fn hidden_only() -> Vec<(String, Box<dyn bevy_reflect::Reflect>)> {
            vec![
                (
                    "bsengine_core::global_transform::GlobalTransform".to_string(),
                    Box::new(bsengine_core::GlobalTransform::default())
                        as Box<dyn bevy_reflect::Reflect>,
                ),
                (
                    "bsengine_core::visible::Visible".to_string(),
                    Box::new(bsengine_core::Visible::default()) as Box<dyn bevy_reflect::Reflect>,
                ),
            ]
        }

        for type_registry in [None, Some(&registry)] {
            let empty_count = render_shape_count(vec![], type_registry);
            let hidden_only_count = render_shape_count(hidden_only(), type_registry);

            assert_eq!(
                empty_count,
                hidden_only_count,
                "an entity with only hidden reflected components must render exactly like an \
                 entity with none -- no dangling separator or empty section left behind \
                 (with a type_registry: {})",
                type_registry.is_some()
            );
        }
    }

    #[test]
    fn component_list_handles_multiple_counts_without_panicking() {
        // Visual boundary design (approved via the visual companion,
        // 2026-07-27 brainstorm): no framed box per component, just a thin
        // ui.separator() between each pair -- drawn *before* each component
        // except the first, since a separator already precedes the whole
        // list (the Visible toggle's), which serves as the rule before the
        // first component. With N components there must be exactly N-1
        // *additional* separators drawn by the loop itself.
        //
        // egui doesn't expose "shapes drawn by ui.separator()" as a
        // distinct, directly-queryable shape variant (a separator paints a
        // Shape::Line/Rect, indistinguishable at this level from any other
        // thin rect this panel might draw) -- so instead of counting
        // separator shapes precisely, this test asserts the weaker but
        // still meaningful invariant that CAN be checked without
        // over-fitting to egui's internal paint representation: the panel
        // renders without panicking for 1, 2, and 3 reflected_components
        // entries (0 separators, 1 separator, 2 separators respectively),
        // proving the enumerate()-based conditional doesn't panic or
        // corrupt state at any component count, including the boundary
        // case of exactly one component (i == 0, no separator drawn at
        // all).
        for count in 1..=3 {
            let mut insp = InspectorState::default();
            insp.selected_id = Some(1);
            insp.reflected_components = (0..count)
                .map(|i| {
                    (
                        format!("test_module::Component{i}"),
                        Box::new(bsengine_core::Transform::default())
                            as Box<dyn bevy_reflect::Reflect>,
                    )
                })
                .collect();

            let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
            let mut panel = InspectorPanel;

            with_test_ui(|ui| {
                let mut ctx = EditorPanelContext {
                    insp: &mut insp,
                    entities_snapshot: &entities_snapshot,
                    cursor_pos: (0.0, 0.0),
                    type_registry: None,
                };
                panel.ui(ui, &mut ctx);
            });
        }
    }

    /// `count` `Transform` values under invented type paths, for a test
    /// that needs the component list to occupy space rather than to hold
    /// anything in particular.
    ///
    /// Follows `component_list_handles_multiple_counts_without_panicking`:
    /// distinct paths are what give each entry its own collapsing-header
    /// id, and none of them is ever registered, so none can also turn up as
    /// a picker row.
    fn filler_components(count: usize) -> Vec<(String, Box<dyn bevy_reflect::Reflect>)> {
        (0..count)
            .map(|i| {
                (
                    format!("test_module::Component{i}"),
                    Box::new(bsengine_core::Transform::default()) as Box<dyn bevy_reflect::Reflect>,
                )
            })
            .collect()
    }

    /// Where the Add Component button's own label and the open picker's
    /// search hint rendered in `output`.
    ///
    /// Both come from the one frame, so the two are comparable; taking one
    /// from an earlier frame would quietly assume the button had not moved.
    /// The hint is what marks where the popup went -- an empty `TextEdit`
    /// paints nothing of its own, so its placeholder is the only text
    /// drawn there.
    fn button_and_hint_ys(output: &egui::FullOutput) -> (f32, f32) {
        let texts = collect_rendered_texts_with_pos(&output.shapes);
        let button_y = texts
            .iter()
            .find(|(text, _)| text.contains("Add Component"))
            .map(|(_, pos)| pos.y)
            .expect("the Add Component button must still render while the picker is open");
        let hint_y = texts
            .iter()
            .find(|(text, _)| text == SEARCH_HINT)
            .map(|(_, pos)| pos.y)
            .expect("the search field's hint text must render while the picker is open");
        (button_y, hint_y)
    }

    #[test]
    fn picker_opens_below_a_button_that_sits_high() {
        // An entity with no components leaves the button just under the
        // header, with almost the whole screen free below it and next to
        // nothing above. A hardcoded `AboveOrBelow::Above` cannot honour
        // that: the popup doesn't fit above, so `Area`'s screen-clamping
        // pushes it back down over the button and everything above it --
        // still clickable, but it reads as broken. So the direction has to
        // be chosen from the space actually available.
        //
        // Asserted through the render rather than by inspecting the chosen
        // `AboveOrBelow` (a local inside the panel): the search hint is the
        // empty field's only painted content, so its position is where the
        // popup went.
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::PointLight>();
        registry.register::<bsengine_core::Camera>();
        // No components at all: this is what leaves the button high.
        let mut harness = PickerHarness::new(registry, Vec::new());

        let frames = harness.open_picker();
        let (button_y, hint_y) = button_and_hint_ys(&frames.open);
        let screen = harness.screen_rect();

        assert!(
            button_y < screen.center().y,
            "this scenario is only meaningful with the button in the upper half of the \
             screen; got button y={button_y} against screen {screen:?}"
        );
        assert!(
            hint_y > button_y,
            "with the button high on screen the picker must open downward, so the search \
             field must render below the button's label; got hint y={hint_y} vs button \
             y={button_y}"
        );
    }

    #[test]
    fn picker_opens_above_a_button_that_sits_low() {
        // The common case: the button follows the component list, so on an
        // entity with a few components it sits near the bottom edge and the
        // room is all above it. Three filler components is what puts it
        // there on this 400x400 screen -- measured with a throwaway sweep of
        // counts 0..=6, which put the button at y=59, 152, 245 and 338, and
        // then stopped rendering it at all from 4 on (the panel's ScrollArea
        // clips it once the content outgrows the viewport, and a button that
        // never renders can't be clicked or measured).
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::PointLight>();
        registry.register::<bsengine_core::Camera>();
        let mut harness = PickerHarness::new(registry, filler_components(3));

        let frames = harness.open_picker();
        let (button_y, hint_y) = button_and_hint_ys(&frames.open);
        let screen = harness.screen_rect();

        assert!(
            button_y > screen.center().y,
            "this scenario is only meaningful with the button in the lower half of the \
             screen; got button y={button_y} against screen {screen:?}"
        );
        assert!(
            hint_y < button_y,
            "with the button low on screen the picker must open upward, so the search \
             field must render above the button's label; got hint y={hint_y} vs button \
             y={button_y}"
        );
    }

    #[test]
    fn picker_keeps_its_side_when_the_geometry_changes_while_open() {
        // The side is latched when the popup opens rather than recomputed
        // per frame, so an open picker cannot jump across the button.
        //
        // Driven by a window resize, not a scroll, and that is worth
        // explaining because scrolling is the case the latch was written
        // for. Scrolling cannot show it *in this harness*: the button is
        // the panel's last widget, so whenever the content is tall enough
        // to scroll at all, the button sits at or below the viewport's
        // bottom edge and is never in the screen's upper half. Probed
        // rather than assumed -- a throwaway sweep drove wheel events at
        // 4, 6 and 8 components and the button reported y=383 of 400 at
        // every offset where it rendered, and was culled entirely at the
        // rest, so the rule returns `Above` throughout and a scroll test
        // would pass with or without the latch. (The flip is still real in
        // the shipped editor, where the Inspector is a dock tab whose
        // viewport is a sub-rect of the screen: a short tab docked high
        // can hold the button in the screen's upper half and still
        // scroll.) A resize reaches the same invariant here -- the
        // direction rule's answer changes underneath an open popup -- with
        // no second harness.
        //
        // Three components put the button low on the 400px screen, so it
        // opens `Above`; growing the screen to 800 leaves the button where
        // it is (its y comes from the component list, not the window) but
        // gives `space_below` the larger value, so a per-frame rule would
        // now answer `Below`.
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::PointLight>();
        registry.register::<bsengine_core::Camera>();
        let mut harness = PickerHarness::new(registry, filler_components(3));

        let frames = harness.open_picker();
        let (button_y, hint_y) = button_and_hint_ys(&frames.open);
        assert!(
            hint_y < button_y,
            "the picker must start out opening upward for this test to be about anything; \
             got hint y={hint_y} vs button y={button_y}"
        );

        harness.set_screen_rect(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(400.0, 800.0),
        ));
        let resized = harness.draw();
        let (resized_button_y, resized_hint_y) = button_and_hint_ys(&resized);

        // The resize has to actually change what the rule would answer,
        // otherwise the assertion below proves nothing. Measured off the
        // button's label, which sits at the centre of its rect in this
        // zero-sized-galley harness (see `collect_rendered_texts_with_pos`),
        // so it stands in for the rect either side is measured from.
        let screen = harness.screen_rect();
        assert!(
            screen.bottom() - resized_button_y > resized_button_y - screen.top(),
            "the enlarged screen must be one where the rule now prefers opening downward, \
             or this test cannot tell a latched side from a recomputed one; got button \
             y={resized_button_y} in screen {screen:?}"
        );
        assert!(
            resized_hint_y < resized_button_y,
            "the side is chosen when the picker opens and held while it stays open, so a \
             resize that would now favour opening downward must not move the popup across \
             the button; got hint y={resized_hint_y} vs button y={resized_button_y}"
        );
    }

    #[test]
    fn picker_explains_itself_when_every_component_is_already_attached() {
        // The picker filters out types the entity already has, so on an
        // entity holding all of them it listed nothing at all -- a search
        // box over blank space, with no way to tell "nothing to add" from
        // "the list failed to draw".
        //
        // Both types are attached under the registry's own type paths,
        // asked of `TypePath` rather than spelled out, since matching those
        // exact strings is what makes the already-attached filter fire.
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::PointLight>();
        registry.register::<bsengine_core::Camera>();
        let point_light_path =
            <bsengine_core::PointLight as bevy_reflect::TypePath>::type_path().to_string();
        let camera_path =
            <bsengine_core::Camera as bevy_reflect::TypePath>::type_path().to_string();

        let mut harness = PickerHarness::new(
            registry,
            vec![
                (
                    point_light_path,
                    Box::new(bsengine_core::PointLight::default())
                        as Box<dyn bevy_reflect::Reflect>,
                ),
                (
                    camera_path,
                    Box::new(bsengine_core::Camera::default()) as Box<dyn bevy_reflect::Reflect>,
                ),
            ],
        );

        let frames = harness.open_picker();
        let closed_texts = collect_rendered_texts(&frames.closed.shapes);
        let opened_texts = collect_rendered_texts(&frames.open.shapes);

        assert!(
            !closed_texts.iter().any(|text| text == ALL_ATTACHED_LABEL),
            "the empty-state label belongs to the picker, so it must not render before the \
             picker is open, got: {closed_texts:?}"
        );
        assert!(
            opened_texts.iter().any(|text| text == ALL_ATTACHED_LABEL),
            "with every registered type already attached the picker must say so rather than \
             show an empty list, got: {opened_texts:?}"
        );

        // Both types render as component headers in the list above, so
        // their mere presence proves nothing. What must hold is that
        // opening the picker added no further occurrence of either -- i.e.
        // listed neither as a row.
        for short_name in ["PointLight", "Camera"] {
            assert_eq!(
                occurrences(&opened_texts, short_name),
                occurrences(&closed_texts, short_name),
                "an already-attached type must not be offered as a picker row, so opening the \
                 picker must not add an occurrence of {short_name}; got open: {opened_texts:?} \
                 vs closed: {closed_texts:?}"
            );
        }
    }

    #[test]
    fn picker_says_no_matches_when_the_search_matches_nothing() {
        // The other half of the empty state: the list can also come up
        // empty because of what was typed, and that needs a different
        // answer -- clear the search, rather than nothing to do.
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::PointLight>();
        registry.register::<bsengine_core::Camera>();

        let mut harness = PickerHarness::new(registry, Vec::new());

        let unfiltered = collect_rendered_texts(&harness.open_picker().open.shapes);

        // Nothing is attached here, so both types must be listed first --
        // otherwise the assertions below would pass on an empty picker.
        assert!(
            unfiltered.iter().any(|text| text == "PointLight")
                && unfiltered.iter().any(|text| text == "Camera"),
            "both types must be listed before typing, got: {unfiltered:?}"
        );
        assert!(
            !unfiltered.iter().any(|text| text == NO_MATCHES_LABEL),
            "a picker with rows in it must not claim there are no matches, got: {unfiltered:?}"
        );

        // Type a needle no short name contains, relying on the search
        // field's autofocus the same way
        // `picker_search_field_is_focused_on_open` does.
        harness.type_text("zzz");
        let filtered = collect_rendered_texts(&harness.draw().shapes);

        assert!(
            filtered.iter().any(|text| text == NO_MATCHES_LABEL),
            "a search that matches nothing must say so rather than leave the list blank, \
             got: {filtered:?}"
        );
        assert!(
            !filtered.iter().any(|text| text == "PointLight")
                && !filtered.iter().any(|text| text == "Camera"),
            "neither type matches the typed text, so neither may still be listed, got: \
             {filtered:?}"
        );
        assert!(
            !filtered.iter().any(|text| text == ALL_ATTACHED_LABEL),
            "nothing is attached here -- an empty list caused by the search must not be \
             explained as everything being attached, got: {filtered:?}"
        );
    }
}

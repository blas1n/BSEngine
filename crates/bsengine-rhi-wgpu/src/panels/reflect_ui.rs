use bevy_reflect::Reflect;

/// Read-only context threaded through every `draw_reflect_ui`/`draw_leaf_ui`
/// call in a single Inspector frame — bundled into one struct rather than
/// separate positional parameters, mirroring `hierarchy.rs`'s `TreeCtx` (same
/// rationale: keeps the recursive calls' argument lists from growing every
/// time a new leaf case needs more context).
pub struct ReflectUiCtx<'a> {
    /// Every entity in the current scene, for rendering an `Entity`-typed
    /// field's current target and the picker's search list. Empty is a
    /// valid, safe value (the picker just has nothing to show/pick).
    pub entities: &'a [bsengine_core::InspectorEntityInfo],
    /// Used to look up `ReflectDefault` for: (a) a `List`'s item type when
    /// appending a new element, (b) an enum variant's field types when
    /// switching to a variant that isn't currently active. `None` is a
    /// valid, safe value (those two features just become no-ops).
    pub type_registry: Option<&'a bevy_reflect::TypeRegistry>,
}

/// Returns whether `path` (a component's `type_path()`) should be hidden from
/// user-facing reflected-component lists. Shared by the docked Inspector
/// panel and the overlay-mode fallback UI so the exclusion list only needs
/// to be maintained in one place.
///
/// `GlobalTransform` is a derived value fully recomputed every frame by the
/// transform-propagation system -- editing it has no lasting effect, so it's
/// hidden entirely rather than shown read-only. `Visible` is already
/// surfaced by the header's checkbox; showing it again in the list would be
/// a confusing duplicate.
pub(crate) fn is_hidden_reflected_type(path: &str) -> bool {
    path == "bsengine_core::global_transform::GlobalTransform"
        || path == "bsengine_core::visible::Visible"
}

/// Returns the short, human-readable name for `type_path` (e.g. `"Transform"`
/// for `"bsengine_core::transform::Transform"`), used to display a component
/// in the Inspector without its full namespace. Falls back to the original
/// `type_path` string when no registry is available, or when the type isn't
/// registered in it (both are safe, valid states — the display just becomes
/// less friendly, nothing breaks).
///
/// Uses `bevy_reflect`'s own `TypePathTable::short_path()` rather than
/// manually splitting `type_path` on `"::"`, since that would mis-handle any
/// type with `::` inside its generic parameters (e.g. `Vec<other::Type>`).
pub(crate) fn short_component_name(
    type_path: &str,
    type_registry: Option<&bevy_reflect::TypeRegistry>,
) -> String {
    type_registry
        .and_then(|registry| registry.get_with_type_path(type_path))
        .map(|registration| {
            registration
                .type_info()
                .type_path_table()
                .short_path()
                .to_string()
        })
        .unwrap_or_else(|| type_path.to_string())
}

/// Looks up the `ReflectValidate` type data for `type_path` (if the
/// component's `#[derive(Reflect)]` registered one via
/// `#[reflect(..., Validate)]`) and calls it on `value` in place. A no-op
/// for any component that doesn't implement `Validate` — most components
/// have no cross-field invariants to enforce, so this only ever does
/// something for the (currently one) type that opts in.
///
/// Shared by the docked Inspector panel and the overlay-mode fallback UI so
/// both apply the same cross-field clamp (e.g. `SpotLight`'s inner/outer
/// angle) after an edit, rather than the overlay silently skipping it.
pub(crate) fn validate_after_edit(
    type_path: &str,
    value: &mut dyn Reflect,
    type_registry: Option<&bevy_reflect::TypeRegistry>,
) {
    let Some(registry) = type_registry else {
        return;
    };
    let Some(registration) = registry.get_with_type_path(type_path) else {
        return;
    };
    let Some(reflect_validate) = registration.data::<bsengine_core::ReflectValidate>() else {
        return;
    };
    if let Some(validate) = reflect_validate.get_mut(value) {
        validate.validate();
    }
}

/// Recursively renders an egui editor for any `Reflect` value. Returns
/// whether anything changed. Handles:
/// - `Struct`: iterate fields, label + recurse.
/// - `TupleStruct`: iterate fields and recurse, with no label wrapper
///   (tuple struct fields have no name to show).
/// - `Enum`: a combo box listing every variant (from `EnumInfo`); picking a
///   different one builds a `ReflectDefault`-based `DynamicEnum` for it and
///   applies it, then recurses into the (possibly new) active variant's
///   fields.
/// - `List`: one row per element (recursively-rendered widget + a remove
///   button), plus an append row that pushes a `ReflectDefault`-constructed
///   item via the type registry.
/// - Opaque `Value`: glam Vec2/Vec3/Vec4/Quat get dedicated multi-DragValue
///   rows; everything else falls through to primitive widgets.
pub fn draw_reflect_ui(ui: &mut egui::Ui, value: &mut dyn Reflect, ctx: &ReflectUiCtx) -> bool {
    // Enum variant switching is decided in a read-only pass first: building
    // the DynamicEnum needs `value.apply(..)`, a *mutable* borrow, but the
    // combo box itself is drawn while holding `e: &mut dyn Enum` (an
    // existing mutable borrow of `value` from `value.reflect_mut()`) --
    // those two mutable borrows can't coexist. Deciding the switch via
    // `value.reflect_ref()` (immutable) first, entirely separate from the
    // `reflect_mut()` match below, sidesteps the conflict: by the time
    // `value.apply(..)` runs, this `if let` block (and its immutable
    // borrows) has already gone out of scope.
    let variant_switch =
        if let (bevy_reflect::ReflectRef::Enum(e), Some(bevy_reflect::TypeInfo::Enum(enum_info))) =
            (value.reflect_ref(), value.get_represented_type_info())
        {
            let current_variant = e.variant_name().to_string();
            let mut target_variant: Option<&str> = None;
            // `ui.id()` is egui's *stable* id, which is shared across sibling
            // children of the same parent `Ui` (e.g. every field in a Struct's
            // field loop gets a child `Ui` via `new_child`, and unless an
            // explicit id_salt is given, that always resolves to the same
            // `.id()`). Deriving the combo's id from it would collide when a
            // component has 2+ sibling enum fields (e.g. `Tween`'s
            // target/easing/repeat), making their popups share open/closed
            // state. `ui.next_auto_id()` is the per-call-site auto-id counter,
            // which *does* vary between sibling calls, so it's captured here
            // -- before any other widget call in this block consumes it --
            // and used as the combo's id salt instead.
            let combo_salt = ui.next_auto_id();
            ui.push_id(combo_salt, |ui| {
                egui::ComboBox::from_id_salt("variant")
                    .selected_text(&current_variant)
                    .show_ui(ui, |ui| {
                        for name in enum_info.variant_names() {
                            if ui
                                .selectable_label(*name == current_variant, *name)
                                .clicked()
                                && *name != current_variant
                            {
                                target_variant = Some(name);
                            }
                        }
                    });
            });
            target_variant.and_then(|name| {
                ctx.type_registry
                    .and_then(|registry| build_default_variant(enum_info, name, registry))
            })
        } else {
            None
        };
    if let Some(dyn_enum) = variant_switch {
        value.apply(&dyn_enum);
        return true;
    }

    // `l` (below) mutably borrows value for the rest of this match, so this lookup (which
    // needs &value) has to happen first.
    let list_item_type_id = match value.get_represented_type_info() {
        Some(bevy_reflect::TypeInfo::List(list_info)) => Some(list_info.item_type_id()),
        _ => None,
    };
    match value.reflect_mut() {
        bevy_reflect::ReflectMut::Struct(s) => {
            let mut changed = false;
            for i in 0..s.field_len() {
                let name = s.name_at(i).unwrap_or("?").to_string();
                if let Some(field) = s.field_at_mut(i) {
                    ui.horizontal(|ui| {
                        ui.label(&name);
                        changed |= draw_reflect_ui(ui, field, ctx);
                    });
                }
            }
            changed
        }
        bevy_reflect::ReflectMut::TupleStruct(ts) => {
            let mut changed = false;
            for i in 0..ts.field_len() {
                if let Some(field) = ts.field_mut(i) {
                    changed |= draw_reflect_ui(ui, field, ctx);
                }
            }
            changed
        }
        bevy_reflect::ReflectMut::List(l) => {
            let mut changed = false;
            let mut remove_at: Option<usize> = None;
            for i in 0..l.len() {
                if let Some(element) = l.get_mut(i) {
                    ui.horizontal(|ui| {
                        changed |= draw_reflect_ui(ui, element, ctx);
                        if ui.small_button("×").clicked() {
                            remove_at = Some(i);
                        }
                    });
                }
            }
            if let Some(i) = remove_at {
                l.remove(i);
                changed = true;
            }
            ui.horizontal(|ui| {
                if ui.small_button("+").clicked() {
                    if let (Some(item_type_id), Some(registry)) =
                        (list_item_type_id, ctx.type_registry)
                    {
                        if let Some(default) = registry
                            .get_type_data::<bevy_reflect::std_traits::ReflectDefault>(item_type_id)
                        {
                            l.push(default.default());
                            changed = true;
                        }
                    }
                }
            });
            changed
        }
        bevy_reflect::ReflectMut::Enum(e) => {
            ui.label(format!("({})", e.variant_name()));
            let mut changed = false;
            for i in 0..e.field_len() {
                if let Some(field) = e.field_at_mut(i) {
                    changed |= draw_reflect_ui(ui, field, ctx);
                }
            }
            changed
        }
        _ => draw_leaf_ui(ui, value, ctx),
    }
}

/// Builds a `DynamicEnum` representing `variant_name` with every field set
/// to its `ReflectDefault`-constructed value, or `None` if `variant_name`
/// doesn't exist on this enum or any of its fields lack a registered
/// `ReflectDefault` (a safe no-op switch in that case, rather than a partial
/// / panicking one).
fn build_default_variant(
    enum_info: &bevy_reflect::EnumInfo,
    variant_name: &str,
    registry: &bevy_reflect::TypeRegistry,
) -> Option<bevy_reflect::DynamicEnum> {
    let variant_info = enum_info.variant(variant_name)?;
    let dynamic_variant = match variant_info {
        bevy_reflect::VariantInfo::Unit(_) => bevy_reflect::DynamicVariant::Unit,
        bevy_reflect::VariantInfo::Tuple(tuple_info) => {
            let mut t = bevy_reflect::DynamicTuple::default();
            for field in tuple_info.iter() {
                let default = registry
                    .get_type_data::<bevy_reflect::std_traits::ReflectDefault>(field.type_id())?
                    .default();
                t.insert_boxed(default);
            }
            bevy_reflect::DynamicVariant::Tuple(t)
        }
        bevy_reflect::VariantInfo::Struct(struct_info) => {
            let mut s = bevy_reflect::DynamicStruct::default();
            for field in struct_info.iter() {
                let default = registry
                    .get_type_data::<bevy_reflect::std_traits::ReflectDefault>(field.type_id())?
                    .default();
                s.insert_boxed(field.name(), default);
            }
            bevy_reflect::DynamicVariant::Struct(s)
        }
    };
    Some(bevy_reflect::DynamicEnum::new(
        variant_name,
        dynamic_variant,
    ))
}

fn draw_leaf_ui(ui: &mut egui::Ui, value: &mut dyn Reflect, ctx: &ReflectUiCtx) -> bool {
    // `Entity`-typed fields (e.g. Follow/LookAt/Parent's target) get a
    // dedicated picker: the current target is shown as "[id] name", a drop
    // zone accepts the same u64 drag payload Hierarchy rows already emit
    // (`row_response.dnd_set_drag_payload(info.id)` in hierarchy.rs) for
    // reparenting, and a searchable ComboBox is offered as a fallback for
    // when drag-and-drop isn't convenient.
    if let Some(entity) = value.downcast_mut::<bevy_ecs::prelude::Entity>() {
        // `ui.next_auto_id()` (NOT `ui.id()`) as the ComboBox salt -- `ui.id()`
        // is egui's *stable* id, identical across sibling fields under the
        // same parent `Ui`, which caused a real ComboBox id-collision bug
        // fixed earlier in this file's history (see the `combo_salt` comment
        // in the Enum arm of `draw_reflect_ui` above, for sibling enum-typed
        // fields). `next_auto_id()` varies correctly per call site even when
        // siblings share the same stable `.id()`, because each child `Ui`'s
        // auto-id counter increments per creation.
        //
        // Captured first, before any other call in this block that would
        // consume an id -- including `allocate_exact_size` below, which
        // (via `allocate_response`/`allocate_space`) unconditionally
        // advances the `Ui`'s auto-id counter regardless of `Sense`, so it
        // is itself an id-consuming call, not just the `ComboBox` further
        // down.
        let picker_salt = ui.next_auto_id();
        let mut changed = false;
        let current_label = if *entity == bevy_ecs::prelude::Entity::PLACEHOLDER {
            "(none)".to_string()
        } else {
            ctx.entities
                .iter()
                .find(|e| e.id == entity.index() as u64)
                .map(|e| format!("[{}] {}", e.id, e.name.as_deref().unwrap_or("(unnamed)")))
                .unwrap_or_else(|| format!("[{}] (not found)", entity.index()))
        };
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width().min(160.0), 20.0),
            egui::Sense::hover(),
        );
        ui.painter().text(
            rect.left_center(),
            egui::Align2::LEFT_CENTER,
            &current_label,
            egui::FontId::default(),
            ui.visuals().text_color(),
        );
        if let Some(dropped_id) = response.dnd_release_payload::<u64>() {
            *entity = bevy_ecs::prelude::Entity::from_raw(*dropped_id as u32);
            changed = true;
        }
        egui::ComboBox::from_id_salt(picker_salt)
            .selected_text("Search…")
            .show_ui(ui, |ui| {
                for info in ctx.entities {
                    let label = format!(
                        "[{}] {}",
                        info.id,
                        info.name.as_deref().unwrap_or("(unnamed)")
                    );
                    if ui.selectable_label(false, label).clicked() {
                        *entity = bevy_ecs::prelude::Entity::from_raw(info.id as u32);
                        changed = true;
                    }
                }
            });
        return changed;
    }
    // Fields of type `glam::Vec2/Vec3/Vec4/Quat` are never `Reflect` themselves (Task 1: Rust's
    // orphan rule blocks that impl from bsengine-core) — reflected components store these as the
    // local `ReflectVec2`/`ReflectVec3`/`ReflectVec4`/`ReflectQuat` wrapper types instead
    // (`Deref<Target = glam::TheRealType>`), so that's what shows up here at runtime.
    if let Some(v) = value.downcast_mut::<bsengine_core::ReflectVec3>() {
        let mut arr = v.to_array();
        let mut changed = false;
        ui.horizontal(|ui| {
            for a in arr.iter_mut() {
                changed |= ui.add(egui::DragValue::new(a).speed(0.05)).changed();
            }
        });
        if changed {
            v.0 = glam::Vec3::from(arr);
        }
        return changed;
    }
    if let Some(v) = value.downcast_mut::<bsengine_core::ReflectColor>() {
        let mut arr = v.to_array();
        let changed = ui.color_edit_button_rgb(&mut arr).changed();
        if changed {
            v.0 = glam::Vec3::from(arr);
        }
        return changed;
    }
    if let Some(v) = value.downcast_mut::<bsengine_core::ReflectDegrees>() {
        return ui
            .add(egui::DragValue::new(&mut v.0).speed(0.5).suffix("°"))
            .changed();
    }
    if let Some(v) = value.downcast_mut::<bsengine_core::ReflectVec2>() {
        let mut arr = v.to_array();
        let mut changed = false;
        ui.horizontal(|ui| {
            for a in arr.iter_mut() {
                changed |= ui.add(egui::DragValue::new(a).speed(0.05)).changed();
            }
        });
        if changed {
            v.0 = glam::Vec2::from(arr);
        }
        return changed;
    }
    if let Some(v) = value.downcast_mut::<bsengine_core::ReflectVec4>() {
        let mut arr = v.to_array();
        let mut changed = false;
        ui.horizontal(|ui| {
            for a in arr.iter_mut() {
                changed |= ui.add(egui::DragValue::new(a).speed(0.05)).changed();
            }
        });
        if changed {
            v.0 = glam::Vec4::from(arr);
        }
        return changed;
    }
    if let Some(v) = value.downcast_mut::<bsengine_core::ReflectQuat>() {
        // Same to_euler/from_euler(EulerRot::XYZ) convention already used by
        // the hardcoded Transform panel (inspector.rs) and hierarchy.rs --
        // showing/editing raw quaternion x/y/z/w here would be far less
        // usable than the degrees a user actually thinks in.
        let (rx, ry, rz) = v.0.to_euler(glam::EulerRot::XYZ);
        let mut degrees = [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()];
        let mut changed = false;
        ui.horizontal(|ui| {
            for d in degrees.iter_mut() {
                changed |= ui
                    .add(egui::DragValue::new(d).speed(0.5).suffix("°"))
                    .changed();
            }
        });
        if changed {
            v.0 = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                degrees[0].to_radians(),
                degrees[1].to_radians(),
                degrees[2].to_radians(),
            );
        }
        return changed;
    }
    if let Some(v) = value.downcast_mut::<f32>() {
        return ui.add(egui::DragValue::new(v).speed(0.05)).changed();
    }
    if let Some(v) = value.downcast_mut::<f64>() {
        return ui.add(egui::DragValue::new(v).speed(0.05)).changed();
    }
    if let Some(v) = value.downcast_mut::<bool>() {
        return ui.checkbox(v, "").changed();
    }
    if let Some(v) = value.downcast_mut::<String>() {
        return ui.text_edit_singleline(v).changed();
    }
    ui.label("(unsupported field type)");
    false
}

#[cfg(test)]
mod tests {
    use super::{draw_reflect_ui, short_component_name, ReflectUiCtx};
    use bevy_reflect::Reflect;

    fn empty_ctx() -> ReflectUiCtx<'static> {
        ReflectUiCtx {
            entities: &[],
            type_registry: None,
        }
    }

    #[derive(Reflect, Debug, PartialEq, Clone)]
    struct SampleStruct {
        speed: f32,
        offset: bsengine_core::ReflectVec3,
        enabled: bool,
    }

    #[derive(Reflect, Debug, PartialEq, Clone)]
    struct SampleTupleStruct(f32, bool);

    /// Runs `add_contents` against a real (headless) `egui::Ui` inside a single frame and
    /// returns whatever it returns. This mirrors the pattern egui's own crate uses internally
    /// for its widget doctests (see `egui::__run_test_ui`/`__run_test_ctx` in egui 0.29's
    /// `src/lib.rs`), reimplemented locally so the closure can be `FnOnce`, mutably capture
    /// the value under test, and receive the `&Context` alongside the `&mut Ui` (the public
    /// `__run_test_ui` helper only hands out the `Ui`, and requires `Fn` rather than `FnOnce`,
    /// neither of which this test module can work with).
    fn with_test_ui<R>(add_contents: impl FnOnce(&egui::Context, &mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::empty()); // skip font loading, saves CPU time
        let mut add_contents = Some(add_contents);
        let mut result = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if let Some(f) = add_contents.take() {
                    result = Some(f(ctx, ui));
                }
            });
        });
        result.expect("add_contents must run exactly once per test frame")
    }

    /// `egui::Id` values are content-hashes, not linear counters, so two `Ui::next_auto_id()`
    /// snapshots can't be subtracted to recover a widget count. `Ui::skip_ahead_auto_ids` is
    /// egui's own public, documented way to "pretend `n` widgets were allocated" — used here to
    /// compute, on a *fresh* top-level `Ui` (i.e. the first content added to a `CentralPanel` in
    /// a new headless frame — the same starting point every test below uses), what
    /// `next_auto_id()` would read after exactly `n` top-level widgets/groups had been drawn.
    /// Comparing a real post-`draw_reflect_ui` `next_auto_id()` against this tells us how many
    /// top-level auto-ids were actually consumed, without hardcoding opaque hash constants.
    ///
    /// (Verified empirically against egui 0.29.1: a bare `ui.add(DragValue::new(..))` and a
    /// `ui.horizontal(|ui| { ..3 DragValues.. })` each consume exactly 1 top-level auto-id on
    /// the *parent* — the group's own internal widgets live on a separately-salted child `Ui`
    /// and don't show up in the parent's count. So distinguishing "1 bare widget" from "1
    /// wrapped group" needs a second signal — see `top_level_response_exists_at` below.)
    fn auto_id_after_n_top_level_widgets(n: usize) -> egui::Id {
        with_test_ui(|_ctx, ui| {
            ui.skip_ahead_auto_ids(n);
            ui.next_auto_id()
        })
    }

    /// Whether *some* widget response is registered at exactly the id a fresh top-level `Ui`
    /// would hand out first (i.e. `id == ui.next_auto_id()` captured before anything was drawn).
    ///
    /// A bare, unwrapped widget (`ui.add(..)`, `ui.checkbox(..)`, `ui.label(..)`, …) claims
    /// exactly that id, so this reads `true`. A `ui.horizontal(|ui| { .. })` group's member
    /// widgets live on an independently-salted *child* `Ui` and never claim the parent's id, so
    /// this reads `false` for any group, no matter what it contains (verified empirically: for
    /// `ReflectVec3`'s 3-`DragValue` `ui.horizontal` group, `read_response` at the parent's
    /// pre-call id returns `None`, whereas a direct `f32` `DragValue` or the fallback
    /// `ui.label(..)` — both bare, unwrapped calls — return `Some(..)`).
    fn top_level_response_exists_at(ctx: &egui::Context, id: egui::Id) -> Option<egui::Response> {
        ctx.read_response(id)
    }

    #[test]
    fn f32_leaf_renders_a_focusable_dragvalue_not_the_fallback_label() {
        let mut speed: f32 = 3.5;
        let (changed, widget_count, is_focusable) = with_test_ui(|ctx, ui| {
            let before = ui.next_auto_id();
            let changed = draw_reflect_ui(ui, &mut speed, &empty_ctx());
            let after = ui.next_auto_id();
            let is_focusable = top_level_response_exists_at(ctx, before)
                .map(|r| r.sense.focusable)
                .unwrap_or(false);
            (changed, after, is_focusable)
        });
        assert!(!changed, "no interaction happened, so nothing changed");
        assert!(
            (speed - 3.5).abs() < f32::EPSILON,
            "value must be untouched"
        );
        assert_eq!(
            widget_count,
            auto_id_after_n_top_level_widgets(1),
            "expected exactly one top-level widget to be drawn for an f32 leaf"
        );
        // DragValue is focusable (keyboard-navigable); a plain `ui.label(..)` is not. This is
        // what actually catches "dispatch silently fell through to the fallback label" — the
        // widget count alone is 1 either way, so it can't distinguish the two paths.
        assert!(
            is_focusable,
            "expected a focusable DragValue at the field's position — a non-focusable result \
             means dispatch fell through to the \"(unsupported field type)\" fallback label \
             instead of rendering a DragValue"
        );
    }

    #[test]
    fn reflect_vec3_leaf_renders_a_wrapped_dragvalue_group_not_the_fallback_label() {
        let mut offset: bsengine_core::ReflectVec3 = glam::Vec3::new(1.0, 2.0, 3.0).into();
        let (changed, widget_count, is_wrapped_group) = with_test_ui(|ctx, ui| {
            let before = ui.next_auto_id();
            let changed = draw_reflect_ui(ui, &mut offset, &empty_ctx());
            let after = ui.next_auto_id();
            let is_wrapped_group = top_level_response_exists_at(ctx, before).is_none();
            (changed, after, is_wrapped_group)
        });
        assert!(!changed, "no interaction happened, so nothing changed");
        assert_eq!(
            offset.0,
            glam::Vec3::new(1.0, 2.0, 3.0),
            "value must be untouched"
        );
        assert_eq!(
            widget_count,
            auto_id_after_n_top_level_widgets(1),
            "expected exactly one top-level group (the ui.horizontal wrapping the 3 DragValues)"
        );
        // The correct path wraps its 3 DragValues in `ui.horizontal`, so the parent's
        // pre-call id is never claimed directly (`is_wrapped_group == true`). If the
        // ReflectVec3 downcast were swapped for e.g. ReflectVec2 (a real type-confusion bug),
        // the real Vec3 value would fail every downcast check and fall through to the bare,
        // unwrapped fallback label instead — which *does* claim the parent's id directly
        // (`is_wrapped_group == false`). This is the signal that catches that class of bug;
        // the widget count alone (1 in both cases — see `auto_id_after_n_top_level_widgets`'s
        // doc comment) cannot.
        assert!(
            is_wrapped_group,
            "expected the 3 DragValues to live inside a ui.horizontal group (not claiming the \
             field's own id directly) — finding a bare widget at the field's exact id means \
             dispatch fell through to the single-widget fallback label instead"
        );
    }

    #[test]
    fn reflect_degrees_leaf_renders_a_focusable_dragvalue_not_the_fallback_label() {
        let mut angle: bsengine_core::ReflectDegrees = 45.0_f32.into();
        let (changed, widget_count, is_focusable) = with_test_ui(|ctx, ui| {
            let before = ui.next_auto_id();
            let changed = draw_reflect_ui(ui, &mut angle, &empty_ctx());
            let after = ui.next_auto_id();
            let is_focusable = top_level_response_exists_at(ctx, before)
                .map(|r| r.sense.focusable)
                .unwrap_or(false);
            (changed, after, is_focusable)
        });
        assert!(!changed, "no interaction happened, so nothing changed");
        assert_eq!(angle.0, 45.0, "value must be untouched");
        assert_eq!(
            widget_count,
            auto_id_after_n_top_level_widgets(1),
            "expected exactly one top-level widget to be drawn for a ReflectDegrees leaf"
        );
        assert!(
            is_focusable,
            "expected a focusable DragValue at the field's position — a non-focusable result \
             means dispatch fell through to the \"(unsupported field type)\" fallback label \
             instead of rendering a DragValue"
        );
    }

    #[test]
    fn reflect_color_leaf_renders_a_focusable_color_button_not_the_fallback_label() {
        // `ui.color_edit_button_rgb` bottoms out in egui's internal `color_button`,
        // which calls `ui.allocate_exact_size(size, Sense::click())` directly — a bare,
        // unwrapped top-level call, not wrapped in `ui.horizontal` like ReflectVec3's 3
        // DragValues. `Sense::click()` has `focusable: true` (verified directly against
        // egui 0.29.1's `sense.rs`). So this leaf has the same "1 bare focusable widget"
        // shape as the `f32`/`ReflectDegrees` leaves, not the "1 wrapped group" shape of
        // `ReflectVec3`.
        let mut color: bsengine_core::ReflectColor = glam::Vec3::new(1.0, 0.5, 0.0).into();
        let (changed, widget_count, is_focusable) = with_test_ui(|ctx, ui| {
            let before = ui.next_auto_id();
            let changed = draw_reflect_ui(ui, &mut color, &empty_ctx());
            let after = ui.next_auto_id();
            let is_focusable = top_level_response_exists_at(ctx, before)
                .map(|r| r.sense.focusable)
                .unwrap_or(false);
            (changed, after, is_focusable)
        });
        assert!(!changed, "no interaction happened, so nothing changed");
        assert_eq!(
            color.0,
            glam::Vec3::new(1.0, 0.5, 0.0),
            "value must be untouched"
        );
        assert_eq!(
            widget_count,
            auto_id_after_n_top_level_widgets(1),
            "expected exactly one top-level widget to be drawn for a ReflectColor leaf"
        );
        assert!(
            is_focusable,
            "expected a focusable color button at the field's position — a non-focusable \
             result means dispatch fell through to the \"(unsupported field type)\" fallback \
             label instead of rendering a color button"
        );
    }

    #[test]
    fn struct_recursion_renders_three_field_groups_not_a_single_fallback_label() {
        let mut s = SampleStruct {
            speed: 2.0,
            offset: glam::Vec3::new(1.0, 2.0, 3.0).into(),
            enabled: true,
        };
        let expected = s.clone();
        let (changed, widget_count) = with_test_ui(|_ctx, ui| {
            let changed = draw_reflect_ui(ui, &mut s, &empty_ctx());
            let after = ui.next_auto_id();
            (changed, after)
        });
        assert!(!changed, "no interaction happened, so nothing changed");
        assert_eq!(s, expected, "no field should have been touched");
        // Each of the 3 struct fields is drawn inside its own `ui.horizontal(..)`, and each
        // such group consumes exactly 1 top-level auto-id on the struct's own Ui (see
        // `auto_id_after_n_top_level_widgets`'s doc comment) — so 3 fields drawn correctly
        // means a widget count of 3. A single fallback label for the whole (unrecognized)
        // struct would instead show a widget count of 1, so this catches e.g. the
        // `ReflectMut::Struct` dispatch arm being removed or the field loop stopping early.
        assert_eq!(
            widget_count,
            auto_id_after_n_top_level_widgets(3),
            "expected exactly 3 top-level field groups (one ui.horizontal per struct field)"
        );
    }

    #[test]
    fn tuple_struct_recursion_renders_two_field_widgets_not_a_single_fallback_label() {
        let mut s = SampleTupleStruct(2.0, true);
        let expected = s.clone();
        let (changed, widget_count) = with_test_ui(|_ctx, ui| {
            let changed = draw_reflect_ui(ui, &mut s, &empty_ctx());
            let after = ui.next_auto_id();
            (changed, after)
        });
        assert!(!changed, "no interaction happened, so nothing changed");
        assert_eq!(s, expected, "no field should have been touched");
        // Unlike SampleStruct's fields, a tuple struct's fields have no
        // `ui.horizontal` label wrapper of their own in this implementation
        // (there's no field name to show) -- each field is drawn directly by
        // recursing into draw_reflect_ui, so a bare f32 leaf claims 1
        // top-level id and the bool leaf claims another: 2 total.
        assert_eq!(
            widget_count,
            auto_id_after_n_top_level_widgets(2),
            "expected exactly 2 top-level widgets (one f32 DragValue, one bool checkbox) -- \
             a widget count of 1 would mean this fell through to the single fallback label \
             instead of iterating the tuple struct's fields"
        );
    }

    #[test]
    fn list_leaf_renders_one_row_per_element_plus_an_append_button() {
        let mut tags: Vec<String> = vec!["enemy".to_string(), "boss".to_string()];
        let (changed, widget_count) = with_test_ui(|_ctx, ui| {
            let changed = draw_reflect_ui(ui, &mut tags, &empty_ctx());
            let after = ui.next_auto_id();
            (changed, after)
        });
        assert!(!changed, "no interaction happened, so nothing changed");
        assert_eq!(tags, vec!["enemy".to_string(), "boss".to_string()]);
        // One ui.horizontal group per existing element (text field + "x"
        // button), plus one more group for the "+" append row: 2 elements
        // + 1 append row = 3 top-level groups.
        assert_eq!(
            widget_count,
            auto_id_after_n_top_level_widgets(3),
            "expected 2 element rows + 1 append row"
        );
    }

    #[test]
    fn list_append_button_pushes_a_default_item_using_the_type_registry() {
        let mut tags: Vec<String> = vec![];
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<String>();
        let ctx = ReflectUiCtx {
            entities: &[],
            type_registry: Some(&registry),
        };

        // Simulate a click on the append ("+") button by calling
        // draw_reflect_ui inside a frame where the button's id is pressed.
        let egui_ctx = egui::Context::default();
        egui_ctx.set_fonts(egui::FontDefinitions::empty());
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));

        // Frame 1: measure where the append row starts via
        // `next_widget_position()` -- the same technique used by
        // `enum_variant_combo_switches_to_a_default_instance_of_the_chosen_variant`
        // above. This deliberately does NOT use
        // `ui.ctx().read_response(before)`: that only resolves for bare,
        // unwrapped top-level widgets, and the "+" button lives inside its
        // own `ui.horizontal` wrapper (see the `ReflectMut::List` arm),
        // which puts it on an independently-salted *child* `Ui` that never
        // claims the parent's pre-call id (see `top_level_response_exists_at`'s
        // doc comment above). With `read_response` that silently collapsed
        // to `Rect::NOTHING`, whose `center()` is `(NaN, NaN)`.
        //
        // That NaN position didn't just harmlessly miss the button --
        // verified directly against egui 0.29.1's own
        // `emath::Rect::distance_sq_to_pos` and `egui::hit_test::hit_test`:
        // a NaN pointer position makes every `if`/`else if` distance
        // comparison evaluate to `false`, so *every* widget's
        // distance-to-pointer falls through to `distance_sq_to_pos`'s final
        // `else { 0.0 }` arm. Every widget therefore ties at distance zero,
        // and `hit_test`'s own "in a tie, pick last = topmost" rule means a
        // NaN-position click always resolves to whichever click/drag-sense
        // widget was drawn *last* in the frame -- not "whatever's under the
        // (nonexistent) pointer". The old test only ever passed because the
        // "+" button happened to be both the last and only such widget on
        // screen. The trailing "decoy" button below is a permanent
        // regression guard against exactly that: if this test ever
        // regressed back to the `read_response` technique, the click would
        // land on "decoy" instead and the assertion below would fail (this
        // was verified by temporarily reverting to `read_response` with
        // this same decoy in place).
        let mut append_top = egui::Pos2::ZERO;
        let _ = egui_ctx.run(
            egui::RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            },
            |egui_ctx| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    append_top = ui.next_widget_position();
                    draw_reflect_ui(ui, &mut tags, &ctx);
                    let _ = ui.button("decoy");
                });
            },
        );

        // A click safely inside the "+" `small_button`'s row (top-left of
        // the append row's own `ui.horizontal`, nudged in from its corner).
        let pos = egui::Pos2::new(append_top.x + 8.0, append_top.y + 8.0);
        let click_events = vec![
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
        ];
        let _ = egui_ctx.run(
            egui::RawInput {
                screen_rect: Some(screen_rect),
                events: click_events,
                ..Default::default()
            },
            |egui_ctx| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    draw_reflect_ui(ui, &mut tags, &ctx);
                    let _ = ui.button("decoy");
                });
            },
        );

        assert_eq!(
            tags,
            vec!["".to_string()],
            "clicking append should push one default (empty-string) item"
        );
    }

    #[derive(Reflect, Debug, PartialEq, Clone, Default)]
    enum SampleEnum {
        #[default]
        Unit,
        Tuple(f32),
        Named {
            x: f32,
        },
    }

    #[test]
    fn enum_variant_combo_switches_to_a_default_instance_of_the_chosen_variant() {
        let mut value = SampleEnum::Unit;
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<f32>();
        let ctx = ReflectUiCtx {
            entities: &[],
            type_registry: Some(&registry),
        };

        let egui_ctx = egui::Context::default();
        egui_ctx.set_fonts(egui::FontDefinitions::empty());
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));

        let run_frame =
            |egui_ctx: &egui::Context, events: Vec<egui::Event>, value: &mut SampleEnum| {
                let _ = egui_ctx.run(
                    egui::RawInput {
                        screen_rect: Some(screen_rect),
                        events,
                        ..Default::default()
                    },
                    |egui_ctx| {
                        egui::CentralPanel::default().show(egui_ctx, |ui| {
                            draw_reflect_ui(ui, value, &ctx);
                        });
                    },
                );
            };
        let click_events = |pos: egui::Pos2| {
            vec![
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
            ]
        };

        // Frame 1: draw the combo box (closed) to learn where it sits.
        // `SampleEnum::Unit` has no fields, so the combo box is the *only*
        // thing drawn -- the cursor position before and after fully brackets
        // its row (CentralPanel's content Ui doesn't shrink-to-fit its
        // `min_rect`, it pre-allocates the full available area, so
        // `ui.min_rect()` can't be used for this; `next_widget_position()`
        // tracks the actual layout cursor instead).
        let mut combo_top = egui::Pos2::ZERO;
        let mut combo_bottom = egui::Pos2::ZERO;
        let _ = egui_ctx.run(
            egui::RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            },
            |egui_ctx| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    combo_top = ui.next_widget_position();
                    draw_reflect_ui(ui, &mut value, &ctx);
                    combo_bottom = ui.next_widget_position();
                });
            },
        );

        // Frame 2: click the combo box (safely inside its row) to open the popup.
        let open_pos = egui::Pos2::new(combo_top.x + 20.0, combo_top.y + 8.0);
        run_frame(&egui_ctx, click_events(open_pos), &mut value);

        // Frame 3 ("settle"): redraw with no input. The popup's first-ever
        // frame (frame 2) sizes its `Area`/`ScrollArea` from a placeholder
        // default (it hasn't measured real content yet), which shifts how
        // many internal child `Ui`s get created versus every frame after --
        // and that shift changes the auto-generated `Id`s of the entries
        // inside it. A click sent in frame 2's immediate next frame gets
        // hit-tested against frame 2's (pre-settle) ids, but resolved
        // against the newly-drawn (post-settle) ones, so it never lands.
        // One extra no-op redraw lets the popup's cached size (and thus its
        // id sequence) stabilize *before* anything tries to click into it;
        // every frame from here on reproduces the same ids/positions.
        run_frame(&egui_ctx, vec![], &mut value);

        // Frame 4: click the settled popup's 2nd entry ("Tuple", index 1 in
        // `EnumInfo::variant_names()` declaration order: Unit, Tuple, Named).
        // Its row starts exactly at `combo_bottom.y` (the popup is anchored
        // to the combo box's bottom-left) and each row after that is a fixed
        // stride down; both the stride and horizontal center were measured
        // empirically against this popup (button padding + a bare/unstyled
        // `FontDefinitions::empty()` row height) and are stable across
        // frames once settled.
        let row_stride = 21.0;
        let row_half_height = 9.0;
        let tuple_variant_index = 1.0;
        let tuple_pos = egui::Pos2::new(
            combo_top.x + 50.0,
            combo_bottom.y + row_stride * tuple_variant_index + row_half_height,
        );
        run_frame(&egui_ctx, click_events(tuple_pos), &mut value);

        assert_eq!(
            value,
            SampleEnum::Tuple(0.0),
            "selecting 'Tuple' in the variant combo should switch to Tuple with a default f32 field"
        );
    }

    #[derive(Reflect, Debug, PartialEq, Clone, Default)]
    enum SampleSiblingEnum {
        #[default]
        A,
        B,
    }

    #[derive(Reflect, Debug, PartialEq, Clone, Default)]
    struct SampleTwoEnumFields {
        first: SampleSiblingEnum,
        second: SampleSiblingEnum,
    }

    #[test]
    fn opening_a_sibling_enum_fields_combo_actually_switches_its_variant() {
        // Regression test for the sibling-id-collision bug fixed alongside this
        // test (see the `combo_salt = ui.next_auto_id()` comment in
        // `draw_reflect_ui`'s Enum arm) -- mirrors `bsengine_core::Tween`'s
        // shape (2+ sibling enum-typed fields on one struct) and drives the
        // *real* `draw_reflect_ui`, through its actual `ReflectMut::Struct`
        // arm, which recurses into each field's own `ui.horizontal(..)` --
        // exactly the call shape that made `first` and `second`'s combos
        // collide on one id pre-fix.
        //
        // Empirically (verified by temporarily reverting *only* the
        // production `combo_salt` line back to `ui.id().with(..)`, see the
        // task notes): with the id collision, `first`'s combo becomes
        // entirely unclickable when `second` is also present. The reason is
        // structural, not incidental to this test's specific click
        // coordinates: `draw_reflect_ui`'s Struct arm draws `first` then
        // `second` in the *same frame*; since both fields' combo buttons
        // register under the identical id, egui's global per-id "was this
        // clicked this frame" flag is shared, so *both* fields' ComboBox
        // internals independently see the click and each call
        // `toggle_popup` on the (shared) popup id -- once each, i.e. twice
        // total, which cancels back to closed. So opening a field's combo
        // silently does nothing whenever a sibling enum field is drawn
        // alongside it. With the fix, each field's combo has its own id, so
        // only the clicked field's `toggle_popup` fires and the popup
        // actually opens.
        //
        // This test opens `first`'s combo and selects "B" (the non-default
        // 2nd variant); it is red on the pre-fix code (the click has no
        // effect -- `first` stays `A`) and green on the fix.
        let mut value = SampleTwoEnumFields::default();
        let registry = bevy_reflect::TypeRegistry::default(); // Unit variants need no ReflectDefault lookups.
        let ctx = ReflectUiCtx {
            entities: &[],
            type_registry: Some(&registry),
        };

        let egui_ctx = egui::Context::default();
        egui_ctx.set_fonts(egui::FontDefinitions::empty());
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));

        let run_frame = |events: Vec<egui::Event>, value: &mut SampleTwoEnumFields| {
            let _ = egui_ctx.run(
                egui::RawInput {
                    screen_rect: Some(screen_rect),
                    events,
                    ..Default::default()
                },
                |egui_ctx| {
                    egui::CentralPanel::default()
                        .show(egui_ctx, |ui| draw_reflect_ui(ui, value, &ctx));
                },
            );
        };
        let click_events = |pos: egui::Pos2| {
            vec![
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
            ]
        };

        // Layout is deterministic (fixed screen size, no fonts, identical
        // default field values) and was measured empirically: `first`'s row
        // spans y=[8, 29], `second`'s spans y=[29, 50] directly below it,
        // with no gap. x=60 lands inside either row's combo regardless of
        // the (near-zero-width, fontless) preceding field-name label --
        // verified directly by scanning x in [20..120] and confirming every
        // candidate opens the popup.
        //
        // Frame 1: draw once (closed) to establish prior-frame widget rects.
        run_frame(vec![], &mut value);

        // Frame 2: click inside `first`'s row to open its combo.
        run_frame(click_events(egui::Pos2::new(60.0, 15.0)), &mut value);

        // Frame 3 ("settle"): redraw with no input, so the popup's cached
        // size/id sequence stabilizes before anything clicks into it (same
        // reason as the single-enum combo test above).
        run_frame(vec![], &mut value);

        // Frame 4: click the settled popup's 2nd entry ("B", index 1 of
        // `SampleSiblingEnum`'s 2 variants). Its row starts exactly at
        // `first`'s row bottom (29) with the same ~21px stride + ~9px
        // half-height measured for the single-enum combo test.
        run_frame(click_events(egui::Pos2::new(60.0, 60.0)), &mut value);

        assert_eq!(
            value.first,
            SampleSiblingEnum::B,
            "opening and selecting in `first`'s combo should switch it to B -- if this \
             is still A, the click had no effect, which is exactly the symptom of the \
             sibling-id-collision bug (both fields' ComboBoxes toggling the same shared \
             popup id back closed within the same frame)"
        );
        assert_eq!(
            value.second,
            SampleSiblingEnum::A,
            "only `first` was interacted with -- `second` must stay at its default"
        );
    }

    #[test]
    fn genuinely_unsupported_leaf_type_falls_through_to_a_non_focusable_label() {
        // i32 is `Reflect` out of the box (bevy_reflect's built-in std impls), but
        // `draw_leaf_ui` has no downcast branch for it, so this must hit the final
        // `ui.label("(unsupported field type)")` fallback arm — this is the confirmed-fallback
        // baseline the other tests' "not the fallback" assertions are contrasted against, and
        // it's also the only test in this file that actually exercises that fallback arm.
        let mut value: i32 = 7;
        let (changed, widget_count, is_focusable) = with_test_ui(|ctx, ui| {
            let before = ui.next_auto_id();
            let changed = draw_reflect_ui(ui, &mut value, &empty_ctx());
            let after = ui.next_auto_id();
            let is_focusable = top_level_response_exists_at(ctx, before)
                .map(|r| r.sense.focusable)
                .unwrap_or(false);
            (changed, after, is_focusable)
        });
        assert!(!changed, "no interaction happened, so nothing changed");
        assert_eq!(value, 7, "value must be untouched");
        assert_eq!(
            widget_count,
            auto_id_after_n_top_level_widgets(1),
            "the fallback path draws exactly one bare label"
        );
        assert!(
            !is_focusable,
            "the fallback label must be a plain, non-focusable label — if this becomes \
             focusable, a live interactive widget is being silently drawn for an unhandled type"
        );
    }

    #[test]
    fn reflect_quat_leaf_renders_three_euler_degree_dragvalues_not_four_raw_components() {
        let mut rot: bsengine_core::ReflectQuat = glam::Quat::IDENTITY.into();
        let (changed, widget_count) = with_test_ui(|_ctx, ui| {
            let changed = draw_reflect_ui(ui, &mut rot, &empty_ctx());
            let after = ui.next_auto_id();
            (changed, after)
        });
        assert!(!changed, "no interaction happened, so nothing changed");
        assert_eq!(rot.0, glam::Quat::IDENTITY, "value must be untouched");
        // Previously rendered 4 bare DragValues (x, y, z, w) inside one
        // ui.horizontal (1 top-level group). Now renders 3 (Euler XYZ
        // degrees) inside the same kind of group -- still 1 top-level
        // group either way, so the widget-count signal alone can't
        // distinguish "3 DragValues" from "4 DragValues". This is why the
        // keyboard-interaction test below (not this one) is the real
        // regression guard; this test only proves the leaf still dispatches
        // to a wrapped group, not the unsupported-type fallback.
        assert_eq!(
            widget_count,
            auto_id_after_n_top_level_widgets(1),
            "expected exactly one top-level group wrapping the Euler DragValues"
        );
    }

    #[test]
    fn reflect_quat_euler_edit_roundtrips_through_the_quaternion() {
        // A quaternion built from known Euler XYZ degrees, converted back
        // to Euler degrees via the exact same to_euler/from_euler(XYZ)
        // convention draw_leaf_ui must use -- proves the conversion formula
        // is lossless within float tolerance, independent of any UI code.
        let original_degrees = [30.0_f32, 45.0, 60.0];
        let quat = glam::Quat::from_euler(
            glam::EulerRot::XYZ,
            original_degrees[0].to_radians(),
            original_degrees[1].to_radians(),
            original_degrees[2].to_radians(),
        );
        let (rx, ry, rz) = quat.to_euler(glam::EulerRot::XYZ);
        let degrees_via_conversion = [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()];
        for (a, b) in original_degrees.iter().zip(degrees_via_conversion.iter()) {
            assert!(
                (a - b).abs() < 1e-3,
                "Euler XYZ round-trip must be lossless within float tolerance: {a} vs {b}"
            );
        }
    }

    #[test]
    fn reflect_quat_leaf_edits_only_the_dragged_euler_axis_via_keyboard() {
        // Real regression gate for "3 Euler-degree DragValues, not 4 raw
        // x/y/z/w components": tabs keyboard focus onto the *first* widget
        // draw_leaf_ui's ReflectQuat branch creates (order-based, so no
        // pixel-position guessing is needed -- see egui 0.29.1's
        // `Memory::interested_in_focus`: "nothing has focus and the user
        // pressed tab -- give focus to the first widget that wants it"),
        // then presses ArrowUp 3 times while it's focused. egui's DragValue
        // reads ArrowUp/Down directly off the keyboard while focused
        // (`is_kb_editing`, drag_value.rs) and bumps its bound value by
        // `speed` per press -- no mouse/pixel interaction needed at all.
        //
        // On the current (post-Step-2) code the first DragValue drawn is
        // bound to the X Euler-degree with speed(0.5), so 3 presses must
        // move it by exactly 1.5 degrees, leaving Y and Z untouched. On the
        // pre-Step-2 code (4 raw quaternion x/y/z/w components, speed(0.05))
        // the first DragValue is bound to the raw quaternion x component
        // instead -- 3 presses would add 0.15 to it directly (no Euler
        // conversion at all), and `Quat::from_array` doesn't renormalize,
        // producing an entirely different (and non-unit) quaternion whose
        // Euler decomposition would not show "X moved by ~1.5°, Y/Z at 0" --
        // so this test is red on that code, not just tautologically green.
        let mut rot: bsengine_core::ReflectQuat = glam::Quat::IDENTITY.into();

        let egui_ctx = egui::Context::default();
        egui_ctx.set_fonts(egui::FontDefinitions::empty());
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));

        // Frame 1: Tab with nothing focused yet. egui grants focus, within
        // this very same frame, to the first widget that registers interest
        // in it -- whichever DragValue draw_leaf_ui's loop creates first.
        let _ = egui_ctx.run(
            egui::RawInput {
                screen_rect: Some(screen_rect),
                events: vec![egui::Event::Key {
                    key: egui::Key::Tab,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::default(),
                }],
                ..Default::default()
            },
            |egui_ctx| {
                egui::CentralPanel::default()
                    .show(egui_ctx, |ui| draw_reflect_ui(ui, &mut rot, &empty_ctx()));
            },
        );

        // Frame 2: 3x ArrowUp while that first DragValue still holds
        // keyboard focus. DragValue consumes these directly and bumps its
        // bound value by `speed` per press.
        let mut changed = false;
        let _ = egui_ctx.run(
            egui::RawInput {
                screen_rect: Some(screen_rect),
                events: vec![
                    egui::Event::Key {
                        key: egui::Key::ArrowUp,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::default(),
                    };
                    3
                ],
                ..Default::default()
            },
            |egui_ctx| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    changed = draw_reflect_ui(ui, &mut rot, &empty_ctx());
                });
            },
        );

        assert!(
            changed,
            "3 ArrowUp presses on the focused first DragValue should have registered a change"
        );
        let (rx, ry, rz) = rot.0.to_euler(glam::EulerRot::XYZ);
        let (rx_deg, ry_deg, rz_deg) = (rx.to_degrees(), ry.to_degrees(), rz.to_degrees());
        assert!(
            (rx_deg - 1.5).abs() < 1e-2,
            "expected the X Euler degree to have moved by exactly 3 * speed(0.5) = 1.5 -- got \
             {rx_deg}"
        );
        assert!(
            ry_deg.abs() < 1e-2 && rz_deg.abs() < 1e-2,
            "only the first (X) DragValue was interacted with -- Y and Z must stay at 0, got \
             ({ry_deg}, {rz_deg})"
        );
    }

    #[test]
    fn entity_leaf_shows_target_name_and_accepts_a_hierarchy_drag_payload() {
        let entities = vec![
            bsengine_core::InspectorEntityInfo {
                id: 1,
                name: Some("Player".to_string()),
                ..Default::default()
            },
            bsengine_core::InspectorEntityInfo {
                id: 2,
                name: Some("Boss".to_string()),
                ..Default::default()
            },
        ];
        let ctx = ReflectUiCtx {
            entities: &entities,
            type_registry: None,
        };
        let mut target = bevy_ecs::prelude::Entity::PLACEHOLDER;

        let egui_ctx = egui::Context::default();
        egui_ctx.set_fonts(egui::FontDefinitions::empty());
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));

        // Frame 1: draw with no target set yet -- establishes the field's
        // drop-zone rect and confirms the "(none)" placeholder shows.
        let mut field_rect = egui::Rect::NOTHING;
        let _ = egui_ctx.run(
            egui::RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            },
            |egui_ctx| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    let before = ui.next_auto_id();
                    draw_reflect_ui(ui, &mut target, &ctx);
                    field_rect = ui
                        .ctx()
                        .read_response(before)
                        .map(|r| r.rect)
                        .unwrap_or(egui::Rect::NOTHING);
                });
            },
        );

        // Frame 2: set a drag payload directly -- the same call
        // `Response::dnd_set_drag_payload` makes internally on
        // `drag_started()` (egui's `response.rs`), so this is equivalent to
        // a real drag having started elsewhere (e.g. a Hierarchy row, which
        // calls `.dnd_set_drag_payload(info.id)` in `hierarchy.rs`) without
        // needing to simulate the full drag gesture -- then release the
        // pointer over the field's rect. `Response::dnd_release_payload`
        // only requires `contains_pointer()` (true once the field's
        // `Sense::hover()` rect contains the pointer position) and
        // `pointer.any_released()` (true from a single `PointerButton
        // {pressed: false}` event, unconditional on any prior `pressed:
        // true` in this test -- confirmed directly against egui 0.29.1's
        // `input_state/mod.rs`: every `pressed: false` event unconditionally
        // pushes a `PointerEvent::Released`).
        egui::DragAndDrop::set_payload(&egui_ctx, 2u64);
        let drop_pos = field_rect.center();
        let _ = egui_ctx.run(
            egui::RawInput {
                screen_rect: Some(screen_rect),
                events: vec![
                    egui::Event::PointerMoved(drop_pos),
                    egui::Event::PointerButton {
                        pos: drop_pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::default(),
                    },
                ],
                ..Default::default()
            },
            |egui_ctx| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    draw_reflect_ui(ui, &mut target, &ctx);
                });
            },
        );

        assert_eq!(
            target.index(),
            2,
            "dropping the id-2 Hierarchy payload should set the field's target to that raw index"
        );
    }

    #[derive(Reflect, Debug, Clone)]
    struct SampleTwoEntityFields {
        first: bevy_ecs::prelude::Entity,
        second: bevy_ecs::prelude::Entity,
    }

    #[test]
    fn opening_a_sibling_entity_fields_picker_only_affects_that_field() {
        // Regression test for the same class of sibling-id-collision bug
        // fixed in Task 4 (see `combo_salt = ui.next_auto_id()` in
        // `draw_reflect_ui`'s Enum arm, and its `opening_a_sibling_enum_..`
        // regression test above) -- but for the Entity picker's own
        // `picker_salt = ui.next_auto_id()` line in `draw_leaf_ui`. Mirrors
        // a component with 2+ sibling `Entity`-typed fields (e.g. a future
        // component with 2 `Entity` fields, or `Follow`+`LookAt` ending up
        // as siblings under one parent `Ui` in a future refactor) and
        // drives the *real* `draw_reflect_ui`, through its actual
        // `ReflectMut::Struct` arm, exactly the call shape that would make
        // `first` and `second`'s picker ComboBoxes collide on one id if the
        // salt regressed back to `ui.id()`.
        //
        // If `picker_salt` regressed to `ui.id()` (stable, identical across
        // sibling fields), both fields' ComboBox internals would register
        // under the same id, so opening one field's popup while a sibling
        // Entity field is also present would toggle it right back closed
        // within the same frame (the exact mechanism documented on the
        // enum regression test above) -- silently no-op'ing the selection
        // click below. This test opens `first`'s picker and selects the
        // 2nd entity ("Boss", id 2) while `second` is also present and
        // untouched; it is red on the pre-fix `ui.id()` salt and green on
        // the `next_auto_id()` fix already in production code.
        //
        // Coordinates below were measured empirically against this exact
        // scene (2 Entity fields, 2 registered entities, fontless/400x400
        // headless canvas) the same way the enum sibling test's were: by
        // instrumenting the real production path with a scratch diagnostic
        // (not committed) that scanned candidate click points and printed
        // which one actually flipped `first`/`second`, then hard-coding the
        // ones that worked.
        let entities = vec![
            bsengine_core::InspectorEntityInfo {
                id: 1,
                name: Some("Player".to_string()),
                ..Default::default()
            },
            bsengine_core::InspectorEntityInfo {
                id: 2,
                name: Some("Boss".to_string()),
                ..Default::default()
            },
        ];
        let ctx = ReflectUiCtx {
            entities: &entities,
            type_registry: None,
        };
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));
        let click_events = |pos: egui::Pos2| {
            vec![
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
            ]
        };

        // --- Scenario A: interact with `first`'s picker; `second` must stay untouched. ---
        {
            let mut value = SampleTwoEntityFields {
                first: bevy_ecs::prelude::Entity::PLACEHOLDER,
                second: bevy_ecs::prelude::Entity::PLACEHOLDER,
            };
            let egui_ctx = egui::Context::default();
            egui_ctx.set_fonts(egui::FontDefinitions::empty());
            let run = |events: Vec<egui::Event>, v: &mut SampleTwoEntityFields| {
                let _ = egui_ctx.run(
                    egui::RawInput {
                        screen_rect: Some(screen_rect),
                        events,
                        ..Default::default()
                    },
                    |c| {
                        egui::CentralPanel::default().show(c, |ui| draw_reflect_ui(ui, v, &ctx));
                    },
                );
            };
            run(vec![], &mut value); // frame 1: establish prior-frame widget rects
            run(click_events(egui::Pos2::new(180.0, 15.0)), &mut value); // frame 2: open `first`'s combo
            run(vec![], &mut value); // frame 3: settle (popup's cached size/id sequence stabilizes)
            run(click_events(egui::Pos2::new(200.0, 60.0)), &mut value); // frame 4: select "Boss" (id 2)

            assert_eq!(
                value.first,
                bevy_ecs::prelude::Entity::from_raw(2),
                "opening and selecting in `first`'s picker should set it to entity id 2 -- if \
                 this is still PLACEHOLDER, the click had no effect, which is exactly the \
                 symptom of the sibling-id-collision bug (both fields' picker ComboBoxes \
                 toggling the same shared popup id back closed within the same frame)"
            );
            assert_eq!(
                value.second,
                bevy_ecs::prelude::Entity::PLACEHOLDER,
                "only `first`'s picker was interacted with -- `second` must stay untouched"
            );
        }

        // --- Scenario B: interact with `second`'s picker; `first` must stay untouched. ---
        {
            let mut value = SampleTwoEntityFields {
                first: bevy_ecs::prelude::Entity::PLACEHOLDER,
                second: bevy_ecs::prelude::Entity::PLACEHOLDER,
            };
            let egui_ctx = egui::Context::default();
            egui_ctx.set_fonts(egui::FontDefinitions::empty());
            let run = |events: Vec<egui::Event>, v: &mut SampleTwoEntityFields| {
                let _ = egui_ctx.run(
                    egui::RawInput {
                        screen_rect: Some(screen_rect),
                        events,
                        ..Default::default()
                    },
                    |c| {
                        egui::CentralPanel::default().show(c, |ui| draw_reflect_ui(ui, v, &ctx));
                    },
                );
            };
            run(vec![], &mut value); // frame 1: establish prior-frame widget rects
            run(click_events(egui::Pos2::new(180.0, 40.0)), &mut value); // frame 2: open `second`'s combo
            run(vec![], &mut value); // frame 3: settle
            run(click_events(egui::Pos2::new(200.0, 85.0)), &mut value); // frame 4: select "Boss" (id 2)

            assert_eq!(
                value.second,
                bevy_ecs::prelude::Entity::from_raw(2),
                "opening and selecting in `second`'s picker should set it to entity id 2"
            );
            assert_eq!(
                value.first,
                bevy_ecs::prelude::Entity::PLACEHOLDER,
                "only `second`'s picker was interacted with -- `first` must stay untouched"
            );
        }
    }

    #[test]
    fn short_component_name_resolves_a_registered_types_short_path() {
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::Transform>();

        let short = short_component_name("bsengine_core::transform::Transform", Some(&registry));

        assert_eq!(
            short, "Transform",
            "a registered type's short_path() must be used instead of the full type_path"
        );
    }

    #[test]
    fn short_component_name_falls_back_to_the_full_path_without_a_registry() {
        let short = short_component_name("bsengine_core::transform::Transform", None);

        assert_eq!(
            short, "bsengine_core::transform::Transform",
            "with no type registry available, the full type_path must be returned unchanged \
             (existing tests that pass type_registry: None rely on this fallback)"
        );
    }

    #[test]
    fn short_component_name_falls_back_to_the_full_path_when_the_type_isnt_registered() {
        let registry = bevy_reflect::TypeRegistry::default();

        let short = short_component_name("some_crate::not_registered::Thing", Some(&registry));

        assert_eq!(
            short, "some_crate::not_registered::Thing",
            "an unregistered type_path has no TypeRegistration to look up a short name from, \
             so it must fall back to the original string rather than panicking"
        );
    }
}

use bevy_reflect::Reflect;

/// Recursively renders an egui editor for any `Reflect` value. Returns
/// whether anything changed. Handles:
/// - `Struct`: iterate fields, label + recurse.
/// - `Enum`: display the current variant's name (read-only — switching
///   variants generically is out of scope for this pass, see design doc)
///   and recurse into its fields.
/// - Opaque `Value`: glam Vec2/Vec3/Vec4/Quat get dedicated multi-DragValue
///   rows; everything else falls through to primitive widgets.
pub fn draw_reflect_ui(ui: &mut egui::Ui, value: &mut dyn Reflect) -> bool {
    match value.reflect_mut() {
        bevy_reflect::ReflectMut::Struct(s) => {
            let mut changed = false;
            for i in 0..s.field_len() {
                let name = s.name_at(i).unwrap_or("?").to_string();
                if let Some(field) = s.field_at_mut(i) {
                    ui.horizontal(|ui| {
                        ui.label(&name);
                        changed |= draw_reflect_ui(ui, field);
                    });
                }
            }
            changed
        }
        bevy_reflect::ReflectMut::Enum(e) => {
            ui.label(format!("({})", e.variant_name()));
            let mut changed = false;
            for i in 0..e.field_len() {
                if let Some(field) = e.field_at_mut(i) {
                    changed |= draw_reflect_ui(ui, field);
                }
            }
            changed
        }
        _ => draw_leaf_ui(ui, value),
    }
}

fn draw_leaf_ui(ui: &mut egui::Ui, value: &mut dyn Reflect) -> bool {
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
        let mut arr = v.to_array();
        let mut changed = false;
        ui.horizontal(|ui| {
            for a in arr.iter_mut() {
                changed |= ui.add(egui::DragValue::new(a).speed(0.05)).changed();
            }
        });
        if changed {
            v.0 = glam::Quat::from_array(arr);
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
    use super::draw_reflect_ui;
    use bevy_reflect::Reflect;

    #[derive(Reflect, Debug, PartialEq, Clone)]
    struct SampleStruct {
        speed: f32,
        offset: bsengine_core::ReflectVec3,
        enabled: bool,
    }

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
            let changed = draw_reflect_ui(ui, &mut speed);
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
            let changed = draw_reflect_ui(ui, &mut offset);
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
            let changed = draw_reflect_ui(ui, &mut angle);
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
            let changed = draw_reflect_ui(ui, &mut color);
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
            let changed = draw_reflect_ui(ui, &mut s);
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
    fn genuinely_unsupported_leaf_type_falls_through_to_a_non_focusable_label() {
        // i32 is `Reflect` out of the box (bevy_reflect's built-in std impls), but
        // `draw_leaf_ui` has no downcast branch for it, so this must hit the final
        // `ui.label("(unsupported field type)")` fallback arm — this is the confirmed-fallback
        // baseline the other tests' "not the fallback" assertions are contrasted against, and
        // it's also the only test in this file that actually exercises that fallback arm.
        let mut value: i32 = 7;
        let (changed, widget_count, is_focusable) = with_test_ui(|ctx, ui| {
            let before = ui.next_auto_id();
            let changed = draw_reflect_ui(ui, &mut value);
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
}

use crate::panels::reflect_ui::{
    draw_reflect_ui, is_hidden_reflected_type, validate_after_edit, ReflectUiCtx,
};
use bsengine_core::{EditorPanel, EditorPanelContext, InspectorCmd, PRIMITIVE_KINDS};

/// The Inspector panel: shows and edits the selected entity's transform, tags, and components.
pub struct InspectorPanel;

impl EditorPanel for InspectorPanel {
    fn id(&self) -> &str {
        "inspector"
    }

    fn title(&self) -> String {
        "Inspector".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorPanelContext) {
        let insp = &mut *ctx.insp;
        let Some(sel_id) = insp.selected_id else {
            ui.label("No entity selected.");
            return;
        };
        let sel_info = ctx
            .entities_snapshot
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

        // Mesh
        ui.horizontal(|ui| {
            ui.colored_label(crate::theme::ACCENT, egui_phosphor::regular::CUBE);
            ui.colored_label(crate::theme::TEXT, "Mesh");
        });
        ui.horizontal(|ui| {
            let current_label = sel_info.primitive.as_deref().unwrap_or("None");
            let mut chosen: Option<&str> = None;
            egui::ComboBox::from_id_salt("mesh_primitive_combo")
                .selected_text(current_label)
                .show_ui(ui, |ui| {
                    for p in PRIMITIVE_KINDS {
                        if ui.selectable_label(false, p).clicked() {
                            chosen = Some(p);
                        }
                    }
                });
            if let Some(primitive) = chosen {
                insp.cmd_queue.push(InspectorCmd::AttachPrimitiveMesh {
                    id: sel_id,
                    primitive: primitive.to_string(),
                });
            }
            if sel_info.primitive.is_some() && ui.button("Remove").clicked() {
                insp.cmd_queue
                    .push(InspectorCmd::DetachPrimitiveMesh { id: sel_id });
            }
        });
        ui.separator();

        // Add Component -- a single menu listing every registered,
        // ReflectDefault-constructible component type not already attached
        // to this entity (filtering prevents a confusing duplicate-attach).
        if let Some(registry) = ctx.type_registry {
            ui.separator();
            ui.horizontal(|ui| {
                ui.colored_label(crate::theme::ACCENT, egui_phosphor::regular::PLUS);
                ui.colored_label(crate::theme::TEXT, "Add Component");
            });
            let mut to_attach: Option<String> = None;
            egui::ComboBox::from_id_salt("reflect_add_component")
                .selected_text("Select type...")
                .show_ui(ui, |ui| {
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
                        let already_attached = insp
                            .reflected_components
                            .iter()
                            .any(|(existing_path, _)| existing_path == &type_path);
                        if already_attached {
                            continue;
                        }
                        if ui.selectable_label(false, &type_path).clicked() {
                            to_attach = Some(type_path);
                        }
                    }
                });
            if let Some(type_path) = to_attach {
                insp.cmd_queue.push(InspectorCmd::AttachComponentByType {
                    id: sel_id,
                    type_path,
                });
            }
        }

        if !insp.reflected_components.is_empty() {
            ui.separator();
            ui.horizontal(|ui| {
                ui.colored_label(crate::theme::ACCENT, egui_phosphor::regular::LIST);
                ui.colored_label(crate::theme::TEXT, "Reflected Fields");
            });
            let type_registry = ctx.type_registry;
            let reflect_ctx = ReflectUiCtx {
                entities: ctx.entities_snapshot,
                type_registry,
            };
            let mut to_apply: Vec<(String, Box<dyn bevy_reflect::Reflect>)> = Vec::new();
            let mut to_remove: Option<String> = None;
            for (type_path, value) in insp
                .reflected_components
                .iter_mut()
                .filter(|(p, _)| !is_hidden_reflected_type(p))
            {
                let header_id = ui.make_persistent_id(type_path.as_str());
                egui::containers::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    header_id,
                    true,
                )
                .show_header(ui, |ui| {
                    ui.colored_label(crate::theme::TEXT, type_path.as_str());
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
    }
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

    #[test]
    fn reflected_fields_section_renders_without_panicking_for_a_real_camera_clone() {
        // The manual smoke test (launching the editor with no entity
        // selected) never exercises this panel's "Reflected Fields" branch
        // at all, since it's gated on `!insp.reflected_components.is_empty()`
        // and an empty scene has nothing selected. This test closes that gap
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
            // reflected entry's header uses
            // `ui.make_persistent_id(type_path.as_str())` as its collapsing
            // header id (see the production code below), so a header
            // genuinely rendered iff that persistent id has recorded
            // open/closed state in memory.
            [
                "bsengine_core::transform::Transform",
                "bsengine_core::global_transform::GlobalTransform",
                "bsengine_core::visible::Visible",
            ]
            .into_iter()
            .filter(|type_path| {
                let id = ui.make_persistent_id(*type_path);
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
    fn add_component_menu_filters_out_already_attached_types() {
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::Camera>();
        registry.register::<bsengine_core::PointLight>();

        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        // Camera is already attached (present in reflected_components) --
        // it must not also appear as a pickable entry in the Add Component
        // menu, or picking it would be a confusing no-op duplicate-attach.
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
                type_registry: Some(&registry),
            };
            panel.ui(ui, &mut ctx);
        });

        // No interaction was simulated (headless single frame), so this
        // doesn't test clicking the menu -- it's a smoke test that the
        // panel renders without panicking with a registry containing an
        // already-attached type.
        assert!(insp.cmd_queue.is_empty());
    }

    #[test]
    fn add_component_menu_click_only_offers_the_not_yet_attached_type() {
        // Regression test that genuinely drives `InspectorPanel::ui()`'s Add
        // Component combo, mirroring the click-simulation technique in
        // `reflect_ui.rs`'s `enum_variant_combo_switches_to_a_default_instance_
        // of_the_chosen_variant` test (open combo, settle frame, click a row)
        // adapted to `panel.ui(ui, &mut ctx)`'s call shape instead of
        // `draw_reflect_ui(ui, value, &ctx)`.
        //
        // Camera is registered AND already attached (in reflected_components);
        // PointLight is registered and NOT attached. With the real
        // `already_attached` filter in place, the combo has exactly one
        // candidate row (PointLight) -- clicking it must queue
        // `AttachComponentByType` for PointLight, never Camera. If the filter
        // were a no-op, Camera would also be offered as a row, which this
        // test's row-count and type_path assertions below would catch.
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<bsengine_core::Camera>();
        registry.register::<bsengine_core::PointLight>();

        let mut insp = InspectorState::default();
        insp.selected_id = Some(1);
        insp.reflected_components = vec![(
            "bsengine_core::camera::Camera".to_string(),
            Box::new(bsengine_core::Camera::default()) as Box<dyn bevy_reflect::Reflect>,
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
                            type_registry: Some(&registry),
                        };
                        panel.ui(ui, &mut ctx);
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

        // Frame 1: draw the panel once (combo closed) so its widgets exist
        // in egui's id/layout cache before anything is clicked.
        run_frame(&egui_ctx, vec![], &mut insp, &entities_snapshot, &mut panel);

        // Frame 2: click the combo box to open its popup. Its position was
        // found empirically for this exact scenario (fixed 400x400 headless
        // screen rect, `FontDefinitions::empty()`, this panel's fixed
        // section order up to "Add Component") by scanning candidate y
        // values and observing `ctx.memory(|mem| mem.any_popup_open())`
        // flip to true -- with the hardcoded Script section also removed
        // (Task 4), there are still two combo boxes ahead of it (the mesh
        // primitive combo opens a popup for y=[66,94]; the Add Component
        // combo -- the one this test wants -- opens one for y=[126,154]),
        // so its vertical center (140) is used here. (Before Task 4 removed
        // the Script section's text edit + "Attach" button -- 2 focusable
        // widgets -- this was y=190; it shifted up because that section's
        // two rows are now gone.)
        let combo_pos = egui::Pos2::new(20.0, 140.0);
        run_frame(
            &egui_ctx,
            click_events(combo_pos),
            &mut insp,
            &entities_snapshot,
            &mut panel,
        );

        // Frame 3 ("settle"): redraw with no input so the popup's cached
        // size/id sequence stabilizes before anything tries to click into
        // it (see reflect_ui.rs's identical settle-frame comment for why
        // this is needed).
        run_frame(&egui_ctx, vec![], &mut insp, &entities_snapshot, &mut panel);

        // Frame 4: click the popup's only row (y=165, likewise found
        // empirically: with the real `already_attached` filter active,
        // clicking anywhere in y=[156,172] queues PointLight, and nothing at
        // all is queued outside that range -- there is no second row).
        let point_light_row_pos = egui::Pos2::new(30.0, 165.0);
        run_frame(
            &egui_ctx,
            click_events(point_light_row_pos),
            &mut insp,
            &entities_snapshot,
            &mut panel,
        );

        assert_eq!(
            insp.cmd_queue.len(),
            1,
            "clicking the popup's only row should queue exactly one attach command; \
             a queue of 0 means the click missed"
        );
        match &insp.cmd_queue[0] {
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

        // Frame 5: reopen the combo and click y=183 -- just past the
        // PointLight row's range (y=[156,172]), the row position Camera
        // would occupy as a second entry if the `already_attached` filter
        // regressed to a no-op. With the real filter active, there is no
        // second row there, so this must add nothing to the queue: the
        // length must stay at 1 from frame 4, not grow to 2.
        run_frame(
            &egui_ctx,
            click_events(combo_pos),
            &mut insp,
            &entities_snapshot,
            &mut panel,
        );
        run_frame(&egui_ctx, vec![], &mut insp, &entities_snapshot, &mut panel);
        let camera_row_pos = egui::Pos2::new(30.0, 183.0);
        run_frame(
            &egui_ctx,
            click_events(camera_row_pos),
            &mut insp,
            &entities_snapshot,
            &mut panel,
        );
        assert_eq!(
            insp.cmd_queue.len(),
            1,
            "clicking where Camera's row would sit if it weren't filtered out must not \
             queue a second command -- a length of 2 here means the already_attached \
             filter regressed and Camera became clickable again"
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
        // Reflected Fields list: the Visible checkbox and the mesh primitive
        // combo box, then this Transform entry's own collapsing-header
        // toggle button and "..." menu button (both added by
        // CollapsingState::show_header itself, not just its closure).
        // That's 4 focusable widgets ahead of translation.x's DragValue, so
        // 5 Tab presses (one per frame) are needed, not 1. This was
        // confirmed empirically with a throwaway diagnostic test that swept
        // tab_count from 0 to 14 and printed the resulting queued command
        // after 3x ArrowUp at each count: queue_len stayed 0 through
        // tab_count=4, became 1 at tab_count=5 with translation moved to
        // (0.15, 0, 0) and rotation/scale untouched, then 6/7 hit
        // translation.y/z, and 8+ found no more focus-wanting widgets (or
        // hit rotation's raw quaternion components, which ArrowUp doesn't
        // move the same way), until tab_count=14 wrapped focus back to the
        // start of the chain and queue_len returned to 0.
        //
        // (Prior to Task 3 removing the hardcoded Tags section's new-tag
        // text edit + "Add" button -- 2 focusable widgets -- this count was
        // 9, dropping to 7 as a direct consequence of that removal; Task 4
        // removing the hardcoded Script section's text edit + "Attach"
        // button -- 2 more focusable widgets -- dropped it again, from 7
        // to 5.)
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

        // Frames 2-6: one Tab per frame, walking focus through the 4
        // focusable widgets that precede translation.x (Visible checkbox;
        // mesh combo box; Transform's collapsing-header toggle button;
        // Transform's "..." menu button) until the 5th Tab lands on
        // translation.x's DragValue -- see the empirical sweep described
        // above.
        let tab_event = || egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        for _ in 0..5 {
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
        // With the hardcoded Script section removed (this task), the
        // focusable widgets preceding the ScriptPath entry's own leaf text
        // field are: the Visible checkbox(1), the mesh primitive combo
        // box(2), then this ScriptPath entry's own collapsing-header
        // toggle(3) and "..." menu button(4) (both added by
        // `CollapsingState::show_header` itself), before the 5th Tab
        // finally lands on the leaf. Confirmed empirically with a
        // throwaway diagnostic test that swept tab_count from 0 to 14:
        // queue_len was 0 through tab_count=4, became 1 at tab_count=5,
        // then 0 again through tab_count=10 before becoming 1 again at
        // tab_count=11 (egui's focus wraps back around to the start of the
        // chain once it runs out of focus-wanting widgets, so a much larger
        // tab_count re-lands on the same leaf on a later lap).
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

        // Frames 2-6: one Tab per frame, walking focus through the 4
        // focusable widgets that precede the ScriptPath entry's leaf text
        // field (see the comment above) until the 5th Tab lands on it.
        let tab_event = || egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        for _ in 0..5 {
            run_frame(
                &egui_ctx,
                vec![tab_event()],
                &mut insp,
                &entities_snapshot,
                &mut panel,
            );
        }

        // Frame 7: type "foo.js" into the focused text field.
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
            let id = ui.make_persistent_id("bsengine_editor::snapshot::Tags");
            egui::containers::collapsing_header::CollapsingState::load(ui.ctx(), id).is_some()
        });

        assert!(
            shown,
            "Tags must render as a genuine Reflected Fields entry once the hardcoded \
             Tags section is removed"
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
}

use crate::panels::reflect_ui::draw_reflect_ui;
use bsengine_core::{EditorPanel, EditorPanelContext, InspectorCmd, PRIMITIVE_KINDS};

pub struct InspectorPanel;

/// Looks up the `ReflectValidate` type data for `type_path` (if the
/// component's `#[derive(Reflect)]` registered one via
/// `#[reflect(..., Validate)]`) and calls it on `value` in place. A no-op
/// for any component that doesn't implement `Validate` — most components
/// have no cross-field invariants to enforce, so this only ever does
/// something for the (currently one) type that opts in.
fn validate_after_edit(
    type_path: &str,
    value: &mut dyn bevy_reflect::Reflect,
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
        let entity_name = sel_info
            .name
            .as_deref()
            .map(String::from)
            .unwrap_or_else(|| format!("Entity {sel_id}"));
        let has_transform = sel_info.position.is_some();
        let light_type = sel_info.light_type.clone();
        let has_camera = sel_info.camera_fov.is_some();

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

        // Transform
        if has_transform {
            ui.horizontal(|ui| {
                ui.colored_label(
                    crate::theme::ACCENT,
                    egui_phosphor::regular::ARROWS_OUT_CARDINAL,
                );
                ui.colored_label(crate::theme::TEXT, "Transform");
            });
            let mut pos_changed = false;
            ui.horizontal(|ui| {
                ui.label("Pos");
                pos_changed |= ui
                    .add(egui::DragValue::new(&mut insp.edit_pos[0]).speed(0.05))
                    .changed();
                pos_changed |= ui
                    .add(egui::DragValue::new(&mut insp.edit_pos[1]).speed(0.05))
                    .changed();
                pos_changed |= ui
                    .add(egui::DragValue::new(&mut insp.edit_pos[2]).speed(0.05))
                    .changed();
            });
            if pos_changed {
                insp.cmd_queue.push(InspectorCmd::SetPosition {
                    id: sel_id,
                    x: insp.edit_pos[0],
                    y: insp.edit_pos[1],
                    z: insp.edit_pos[2],
                });
            }

            let mut rot_changed = false;
            ui.horizontal(|ui| {
                ui.label("Rot°");
                rot_changed |= ui
                    .add(egui::DragValue::new(&mut insp.edit_rot[0]).speed(0.5))
                    .changed();
                rot_changed |= ui
                    .add(egui::DragValue::new(&mut insp.edit_rot[1]).speed(0.5))
                    .changed();
                rot_changed |= ui
                    .add(egui::DragValue::new(&mut insp.edit_rot[2]).speed(0.5))
                    .changed();
            });
            if rot_changed {
                insp.cmd_queue.push(InspectorCmd::SetRotation {
                    id: sel_id,
                    rx: insp.edit_rot[0],
                    ry: insp.edit_rot[1],
                    rz: insp.edit_rot[2],
                });
            }

            let mut scale_changed = false;
            ui.horizontal(|ui| {
                ui.label("Scale");
                scale_changed |= ui
                    .add(egui::DragValue::new(&mut insp.edit_scale[0]).speed(0.01))
                    .changed();
                scale_changed |= ui
                    .add(egui::DragValue::new(&mut insp.edit_scale[1]).speed(0.01))
                    .changed();
                scale_changed |= ui
                    .add(egui::DragValue::new(&mut insp.edit_scale[2]).speed(0.01))
                    .changed();
            });
            if scale_changed {
                insp.cmd_queue.push(InspectorCmd::SetScale {
                    id: sel_id,
                    sx: insp.edit_scale[0],
                    sy: insp.edit_scale[1],
                    sz: insp.edit_scale[2],
                });
            }
            ui.separator();
        }

        // Tags
        ui.horizontal(|ui| {
            ui.colored_label(crate::theme::ACCENT, egui_phosphor::regular::TAG);
            ui.colored_label(crate::theme::TEXT, "Tags");
        });
        ui.horizontal_wrapped(|ui| {
            for tag in &sel_info.tags {
                ui.horizontal(|ui| {
                    ui.label(tag);
                    if ui.small_button("×").clicked() {
                        insp.cmd_queue.push(InspectorCmd::UntagEntity {
                            id: sel_id,
                            tag: tag.clone(),
                        });
                    }
                });
            }
        });
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut insp.edit_new_tag);
            if ui.button("Add").clicked() && !insp.edit_new_tag.is_empty() {
                insp.cmd_queue.push(InspectorCmd::TagEntity {
                    id: sel_id,
                    tag: insp.edit_new_tag.clone(),
                });
                insp.edit_new_tag.clear();
            }
        });
        ui.separator();

        // Script
        ui.horizontal(|ui| {
            ui.colored_label(crate::theme::ACCENT, egui_phosphor::regular::CODE);
            ui.colored_label(crate::theme::TEXT, "Script");
        });
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut insp.edit_script_path);
            if ui.button("Attach").clicked() && !insp.edit_script_path.is_empty() {
                insp.cmd_queue.push(InspectorCmd::AttachScript {
                    id: sel_id,
                    path: insp.edit_script_path.clone(),
                });
            }
            if sel_info.script_path.is_some() && ui.button("Clear").clicked() {
                insp.cmd_queue
                    .push(InspectorCmd::DetachScript { id: sel_id });
                insp.edit_script_path.clear();
            }
        });
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

        // Add Component
        ui.horizontal(|ui| {
            ui.colored_label(crate::theme::ACCENT, egui_phosphor::regular::PLUS);
            ui.colored_label(crate::theme::TEXT, "Add Component");
        });
        ui.horizontal(|ui| {
            if light_type.is_none() && ui.button("Point Light").clicked() {
                insp.cmd_queue
                    .push(InspectorCmd::AddPointLight { id: sel_id });
            }
            if !has_camera && ui.button("Camera").clicked() {
                insp.cmd_queue.push(InspectorCmd::AddCamera { id: sel_id });
            }
        });

        if let Some(registry) = ctx.type_registry {
            ui.separator();
            ui.horizontal(|ui| {
                ui.colored_label(crate::theme::ACCENT, egui_phosphor::regular::PLUS);
                ui.colored_label(crate::theme::TEXT, "Add Component (reflected)");
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
                        let type_path = registration.type_info().type_path().to_string();
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
            let mut to_apply: Vec<(String, Box<dyn bevy_reflect::Reflect>)> = Vec::new();
            let mut to_remove: Option<String> = None;
            for (type_path, value) in insp.reflected_components.iter_mut() {
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
                    if draw_reflect_ui(ui, value.as_mut()) {
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

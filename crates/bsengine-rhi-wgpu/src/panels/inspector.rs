use crate::panels::reflect_ui::draw_reflect_ui;
use bsengine_core::{EditorPanel, EditorPanelContext, InspectorCmd, PRIMITIVE_KINDS};

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
        let entity_name = sel_info
            .name
            .as_deref()
            .map(String::from)
            .unwrap_or_else(|| format!("Entity {sel_id}"));
        let has_transform = sel_info.position.is_some();
        let light_type = sel_info.light_type.clone();
        let has_camera = sel_info.camera_fov.is_some();
        let has_material = sel_info.material_base_color.is_some();

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
            ui.strong("Transform");
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

        // Light
        if let Some(lt) = &light_type {
            ui.strong(format!("Light ({})", lt));
            let mut light_changed = false;
            ui.horizontal(|ui| {
                ui.label("Color");
                light_changed |= ui
                    .color_edit_button_rgb(&mut insp.edit_light_color)
                    .changed();
            });
            ui.horizontal(|ui| {
                ui.label("Intensity");
                light_changed |= ui
                    .add(
                        egui::DragValue::new(&mut insp.edit_light_intensity)
                            .speed(0.05)
                            .range(0.0..=f32::MAX),
                    )
                    .changed();
            });
            if lt != "directional" {
                ui.horizontal(|ui| {
                    ui.label("Range");
                    light_changed |= ui
                        .add(
                            egui::DragValue::new(&mut insp.edit_light_range)
                                .speed(0.1)
                                .range(0.1..=f32::MAX),
                        )
                        .changed();
                });
            }
            if lt == "spot" {
                ui.horizontal(|ui| {
                    ui.label("Inner Angle°");
                    light_changed |= ui
                        .add(
                            egui::DragValue::new(&mut insp.edit_spot_inner_angle)
                                .speed(0.5)
                                .range(0.0..=insp.edit_spot_outer_angle),
                        )
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Outer Angle°");
                    light_changed |= ui
                        .add(
                            egui::DragValue::new(&mut insp.edit_spot_outer_angle)
                                .speed(0.5)
                                .range(insp.edit_spot_inner_angle..=89.0),
                        )
                        .changed();
                });
            }
            if light_changed {
                insp.cmd_queue.push(InspectorCmd::UpdateLight {
                    id: sel_id,
                    light_type: lt.clone(),
                    color: Some(insp.edit_light_color),
                    intensity: Some(insp.edit_light_intensity),
                    range: Some(insp.edit_light_range),
                    inner_angle: if lt == "spot" {
                        Some(insp.edit_spot_inner_angle.to_radians())
                    } else {
                        None
                    },
                    outer_angle: if lt == "spot" {
                        Some(insp.edit_spot_outer_angle.to_radians())
                    } else {
                        None
                    },
                });
            }
            ui.separator();
        }

        // Camera
        if has_camera {
            ui.horizontal(|ui| {
                ui.strong("Camera");
                if ui.small_button("✕").clicked() {
                    insp.cmd_queue.push(InspectorCmd::RemoveComponentByType {
                        id: sel_id,
                        type_path: "bsengine_core::camera::Camera".to_string(),
                    });
                }
            });
            let mut cam_fov_changed = false;
            ui.horizontal(|ui| {
                ui.label("FOV°");
                cam_fov_changed |= ui
                    .add(
                        egui::DragValue::new(&mut insp.edit_camera_fov)
                            .speed(0.5)
                            .range(10.0..=170.0),
                    )
                    .changed();
            });
            if cam_fov_changed {
                insp.cmd_queue.push(InspectorCmd::UpdateCamera {
                    id: sel_id,
                    fov_y_degrees: Some(insp.edit_camera_fov),
                });
            }
            ui.separator();
        }

        // Material
        if has_material {
            ui.horizontal(|ui| {
                ui.strong("Material");
                if ui.small_button("✕").clicked() {
                    insp.cmd_queue.push(InspectorCmd::RemoveComponentByType {
                        id: sel_id,
                        type_path: "bsengine_core::material::Material".to_string(),
                    });
                }
            });
            let mut mat_changed = false;
            ui.horizontal(|ui| {
                ui.label("Base Color");
                mat_changed |= ui
                    .color_edit_button_rgb(&mut insp.edit_mat_base_color)
                    .changed();
            });
            ui.horizontal(|ui| {
                ui.label("Metallic");
                mat_changed |= ui
                    .add(
                        egui::DragValue::new(&mut insp.edit_mat_metallic)
                            .speed(0.01)
                            .range(0.0..=1.0),
                    )
                    .changed();
            });
            ui.horizontal(|ui| {
                ui.label("Roughness");
                mat_changed |= ui
                    .add(
                        egui::DragValue::new(&mut insp.edit_mat_roughness)
                            .speed(0.01)
                            .range(0.0..=1.0),
                    )
                    .changed();
            });
            ui.horizontal(|ui| {
                ui.label("Emissive");
                mat_changed |= ui
                    .color_edit_button_rgb(&mut insp.edit_mat_emissive)
                    .changed();
            });
            if mat_changed {
                insp.cmd_queue.push(InspectorCmd::UpdateMaterial {
                    id: sel_id,
                    base_color: Some(insp.edit_mat_base_color),
                    metallic: Some(insp.edit_mat_metallic),
                    roughness: Some(insp.edit_mat_roughness),
                    emissive: Some(insp.edit_mat_emissive),
                });
            }
            ui.separator();
        }

        // Tags
        ui.strong("Tags");
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
        ui.strong("Script");
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
        ui.strong("Mesh");
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
        ui.strong("Add Component");
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
            ui.strong("Add Component (reflected)");
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
            ui.strong("Reflected Fields");
            let mut to_apply: Vec<(String, Box<dyn bevy_reflect::Reflect>)> = Vec::new();
            for (type_path, value) in insp.reflected_components.iter_mut() {
                egui::CollapsingHeader::new(type_path.as_str())
                    .id_salt(type_path.as_str())
                    .default_open(true)
                    .show(ui, |ui| {
                        if draw_reflect_ui(ui, value.as_mut()) {
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
        }
    }
}

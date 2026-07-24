use bsengine_core::{EditorPanel, EditorPanelContext, InspectorCmd};

pub struct ViewportPanel;

impl EditorPanel for ViewportPanel {
    fn id(&self) -> &str {
        "viewport"
    }

    fn title(&self) -> String {
        "Viewport".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorPanelContext) {
        let insp = &mut *ctx.insp;
        let entities_snapshot = ctx.entities_snapshot;
        let (cursor_x, cursor_y) = ctx.cursor_pos;

        let panel_rect = ui.max_rect();
        insp.viewport_size = [panel_rect.width(), panel_rect.height()];
        insp.viewport_contains_cursor = panel_rect.contains(egui::Pos2::new(cursor_x, cursor_y));
        let response = ui.allocate_rect(panel_rect, egui::Sense::click_and_drag());

        if let Some(payload) = response.dnd_release_payload::<crate::panels::AssetDragPayload>() {
            if payload.kind == crate::panels::AssetKind::Mesh {
                let name = payload
                    .path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Mesh".to_string());
                insp.cmd_queue.push(InspectorCmd::SpawnMeshAsset {
                    name,
                    path: payload.path.to_string_lossy().to_string(),
                });
            }
        }

        // Gizmo overlays only make sense while editing: once Play starts,
        // the viewport shows the game's own Camera entity feed (see
        // bsengine-render's render_frame), which the editor-orbit-relative
        // view_proj no longer matches.
        let is_stopped = insp.play_state == bsengine_core::EditorPlayState::Stopped;

        if is_stopped && insp.show_grid {
            if let Some(view_proj) = insp.editor_view_proj {
                let lines = crate::gizmo::ground_grid_lines(&view_proj, panel_rect, 1.0, 10);
                crate::gizmo::draw_ground_grid(ui.painter(), &lines);
            }
        }

        if is_stopped {
            if let Some(view_proj) = insp.editor_view_proj {
                for info in entities_snapshot {
                    if let (Some(fov_deg), Some(pos), Some(rot)) =
                        (info.camera_fov, info.position, info.rotation)
                    {
                        let rotation = glam::Quat::from_euler(
                            glam::EulerRot::XYZ,
                            rot[0].to_radians(),
                            rot[1].to_radians(),
                            rot[2].to_radians(),
                        );
                        crate::gizmo::draw_camera_frustum(
                            ui.painter(),
                            glam::Vec3::from(pos),
                            rotation,
                            fov_deg.to_radians(),
                            &view_proj,
                            panel_rect,
                            info.selected,
                        );
                    }
                }
            }

            if let (Some(sel_id), Some(view_proj)) = (insp.selected_id, insp.editor_view_proj) {
                let has_transform = entities_snapshot
                    .iter()
                    .find(|e| e.id == sel_id)
                    .is_some_and(|e| e.position.is_some());
                if has_transform {
                    let pos = glam::Vec3::from(insp.edit_pos);
                    let cam_pos = glam::Vec3::from(insp.editor_cam_pos);
                    let handle_len = crate::gizmo::handle_length(pos, cam_pos);

                    match insp.gizmo_mode {
                        bsengine_core::GizmoMode::Translate => {
                            if response.drag_started() {
                                if let Some(mp) = response.interact_pointer_pos() {
                                    if let Some(axis) = crate::gizmo::hit_test(
                                        pos, handle_len, &view_proj, panel_rect, mp,
                                    ) {
                                        insp.gizmo_drag_axis = Some(axis);
                                        insp.gizmo_drag_start_world = insp.edit_pos;
                                        insp.gizmo_drag_start_mouse = [mp.x, mp.y];
                                    }
                                }
                            }

                            let mut pos_changed = false;
                            if let Some(axis) = insp.gizmo_drag_axis {
                                if response.dragged() {
                                    if let (Some((dir2d, px_per_unit)), Some(mp)) = (
                                        crate::gizmo::axis_screen_dir_and_scale(
                                            glam::Vec3::from(insp.gizmo_drag_start_world),
                                            axis,
                                            handle_len.max(0.01),
                                            &view_proj,
                                            panel_rect,
                                        ),
                                        response.interact_pointer_pos(),
                                    ) {
                                        let start = egui::Pos2::new(
                                            insp.gizmo_drag_start_mouse[0],
                                            insp.gizmo_drag_start_mouse[1],
                                        );
                                        let screen_delta = mp - start;
                                        let world_delta = screen_delta.dot(dir2d) / px_per_unit;
                                        let new_pos = glam::Vec3::from(insp.gizmo_drag_start_world)
                                            + crate::gizmo::axis_dir(axis) * world_delta;
                                        insp.edit_pos = new_pos.to_array();
                                        pos_changed = true;
                                    }
                                } else if response.drag_stopped() {
                                    insp.gizmo_drag_axis = None;
                                }
                            }
                            if pos_changed {
                                insp.cmd_queue.push(InspectorCmd::SetPosition {
                                    id: sel_id,
                                    x: insp.edit_pos[0],
                                    y: insp.edit_pos[1],
                                    z: insp.edit_pos[2],
                                });
                            }

                            let hovered = response.hover_pos().and_then(|mp| {
                                crate::gizmo::hit_test(pos, handle_len, &view_proj, panel_rect, mp)
                            });
                            crate::gizmo::draw(
                                ui.painter(),
                                pos,
                                handle_len,
                                &view_proj,
                                panel_rect,
                                hovered,
                                insp.gizmo_drag_axis,
                            );
                        }
                        bsengine_core::GizmoMode::Rotate => {
                            let radius = handle_len;

                            if response.drag_started() {
                                if let Some(mp) = response.interact_pointer_pos() {
                                    if let Some(axis) = crate::gizmo::hit_test_rotate(
                                        pos, radius, &view_proj, panel_rect, mp,
                                    ) {
                                        if let Some(center) = crate::gizmo::world_to_screen(
                                            pos, &view_proj, panel_rect,
                                        ) {
                                            insp.gizmo_rotate_axis = Some(axis);
                                            insp.gizmo_rotate_start_deg = insp.edit_rot;
                                            insp.gizmo_rotate_start_angle =
                                                crate::gizmo::screen_angle(center, mp);
                                        }
                                    }
                                }
                            }

                            let mut rot_changed = false;
                            if let Some(axis) = insp.gizmo_rotate_axis {
                                if response.dragged() {
                                    if let (Some(center), Some(mp)) = (
                                        crate::gizmo::world_to_screen(pos, &view_proj, panel_rect),
                                        response.interact_pointer_pos(),
                                    ) {
                                        let current_angle = crate::gizmo::screen_angle(center, mp);
                                        let delta = current_angle - insp.gizmo_rotate_start_angle;
                                        let deg = insp.gizmo_rotate_start_deg;
                                        let start_rot = glam::Quat::from_euler(
                                            glam::EulerRot::XYZ,
                                            deg[0].to_radians(),
                                            deg[1].to_radians(),
                                            deg[2].to_radians(),
                                        );
                                        let delta_rot = glam::Quat::from_axis_angle(
                                            crate::gizmo::axis_dir(axis),
                                            delta,
                                        );
                                        let (rx, ry, rz) =
                                            (delta_rot * start_rot).to_euler(glam::EulerRot::XYZ);
                                        insp.edit_rot =
                                            [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()];
                                        rot_changed = true;
                                    }
                                } else if response.drag_stopped() {
                                    insp.gizmo_rotate_axis = None;
                                }
                            }
                            if rot_changed {
                                insp.cmd_queue.push(InspectorCmd::SetRotation {
                                    id: sel_id,
                                    rx: insp.edit_rot[0],
                                    ry: insp.edit_rot[1],
                                    rz: insp.edit_rot[2],
                                });
                            }

                            let hovered = response.hover_pos().and_then(|mp| {
                                crate::gizmo::hit_test_rotate(
                                    pos, radius, &view_proj, panel_rect, mp,
                                )
                            });
                            crate::gizmo::draw_rotate_gizmo(
                                ui.painter(),
                                pos,
                                radius,
                                &view_proj,
                                panel_rect,
                                hovered,
                                insp.gizmo_rotate_axis,
                            );
                        }
                    }
                }
            }
        }

        egui::Area::new(egui::Id::new("viewport_mini_toolbar"))
            .fixed_pos(panel_rect.min + egui::vec2(8.0, 8.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                insp.gizmo_mode == bsengine_core::GizmoMode::Translate,
                                egui_phosphor::regular::ARROWS_OUT_CARDINAL,
                            )
                            .on_hover_text("Move (W)")
                            .clicked()
                        {
                            insp.gizmo_mode = bsengine_core::GizmoMode::Translate;
                        }
                        if ui
                            .selectable_label(
                                insp.gizmo_mode == bsengine_core::GizmoMode::Rotate,
                                egui_phosphor::regular::ARROWS_CLOCKWISE,
                            )
                            .on_hover_text("Rotate (E)")
                            .clicked()
                        {
                            insp.gizmo_mode = bsengine_core::GizmoMode::Rotate;
                        }
                        if ui
                            .selectable_label(insp.show_grid, egui_phosphor::regular::GRID_FOUR)
                            .on_hover_text("Toggle Grid")
                            .clicked()
                        {
                            insp.show_grid = !insp.show_grid;
                        }
                    });
                });
            });

        egui::Area::new(egui::Id::new("viewport_stats_overlay"))
            .fixed_pos(egui::Pos2::new(
                panel_rect.max.x - 90.0,
                panel_rect.min.y + 8.0,
            ))
            .show(ui.ctx(), |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    let fps = 1.0 / ui.ctx().input(|i| i.stable_dt.max(1e-6));
                    ui.colored_label(crate::theme::TEXT_DIM, format!("{fps:.0} FPS"));
                });
            });
    }
}

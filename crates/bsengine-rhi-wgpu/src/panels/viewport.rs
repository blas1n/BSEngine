use bsengine_core::{EditorPanel, EditorPanelContext, InspectorCmd};

/// The Viewport panel: renders the editor gizmo/grid/frustum overlays on top of the 3D scene.
pub struct ViewportPanel {
    /// Exponential-moving-average of the instantaneous per-frame FPS
    /// (`1.0 / stable_dt`), so the stats overlay doesn't visibly jitter —
    /// raw per-frame dt varies enough, even at a steady refresh rate, that
    /// displaying it unsmoothed makes the readout look like it's flickering.
    smoothed_fps: f32,
}

impl Default for ViewportPanel {
    fn default() -> Self {
        Self { smoothed_fps: 60.0 }
    }
}

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
        insp.viewport_pos = [panel_rect.min.x, panel_rect.min.y];
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
            } else if payload.kind == crate::panels::AssetKind::Prefab {
                // Same origin-drop simplification SpawnMeshAsset already
                // makes -- neither reads the drop location, since there's
                // no cursor-to-world raycast wired into this panel yet.
                insp.cmd_queue.push(InspectorCmd::InstantiatePrefab {
                    path: payload.path.to_string_lossy().to_string(),
                    name: None,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    parent_id: None,
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
                        bsengine_core::GizmoMode::Scale => {
                            if response.drag_started() {
                                if let Some(mp) = response.interact_pointer_pos() {
                                    if let Some(handle) = crate::gizmo::hit_test_scale(
                                        pos, handle_len, &view_proj, panel_rect, mp,
                                    ) {
                                        match handle {
                                            crate::gizmo::ScaleHandle::Axis(axis) => {
                                                insp.gizmo_scale_axis = Some(axis);
                                                insp.gizmo_scale_uniform = false;
                                            }
                                            crate::gizmo::ScaleHandle::Uniform => {
                                                insp.gizmo_scale_axis = None;
                                                insp.gizmo_scale_uniform = true;
                                            }
                                        }
                                        insp.gizmo_scale_start_world = insp.edit_scale;
                                        insp.gizmo_scale_start_mouse = [mp.x, mp.y];
                                    }
                                }
                            }

                            let mut scale_changed = false;
                            if let Some(axis) = insp.gizmo_scale_axis {
                                if response.dragged() {
                                    if let (Some((dir2d, px_per_unit)), Some(mp)) = (
                                        crate::gizmo::axis_screen_dir_and_scale(
                                            pos,
                                            axis,
                                            handle_len.max(0.01),
                                            &view_proj,
                                            panel_rect,
                                        ),
                                        response.interact_pointer_pos(),
                                    ) {
                                        let start = egui::Pos2::new(
                                            insp.gizmo_scale_start_mouse[0],
                                            insp.gizmo_scale_start_mouse[1],
                                        );
                                        let screen_delta = mp - start;
                                        let world_delta = screen_delta.dot(dir2d) / px_per_unit;
                                        let delta_frac = world_delta / handle_len.max(0.01);
                                        let mut new_scale = insp.gizmo_scale_start_world;
                                        new_scale[axis as usize] =
                                            (new_scale[axis as usize] + delta_frac).max(0.01);
                                        insp.edit_scale = new_scale;
                                        scale_changed = true;
                                    }
                                } else if response.drag_stopped() {
                                    insp.gizmo_scale_axis = None;
                                }
                            } else if insp.gizmo_scale_uniform {
                                if response.dragged() {
                                    if let (Some(origin), Some(mp)) = (
                                        crate::gizmo::world_to_screen(pos, &view_proj, panel_rect),
                                        response.interact_pointer_pos(),
                                    ) {
                                        let start_mouse = egui::Pos2::new(
                                            insp.gizmo_scale_start_mouse[0],
                                            insp.gizmo_scale_start_mouse[1],
                                        );
                                        let radial_start = (start_mouse - origin).length();
                                        let radial_now = (mp - origin).length();
                                        let delta_frac =
                                            (radial_now - radial_start) / handle_len.max(0.01);
                                        let scale_factor = (1.0 + delta_frac).max(0.01);
                                        insp.edit_scale = insp
                                            .gizmo_scale_start_world
                                            .map(|s| (s * scale_factor).max(0.01));
                                        scale_changed = true;
                                    }
                                } else if response.drag_stopped() {
                                    insp.gizmo_scale_uniform = false;
                                }
                            }
                            if scale_changed {
                                insp.cmd_queue.push(InspectorCmd::SetScale {
                                    id: sel_id,
                                    sx: insp.edit_scale[0],
                                    sy: insp.edit_scale[1],
                                    sz: insp.edit_scale[2],
                                });
                            }

                            let hovered = response.hover_pos().and_then(|mp| {
                                crate::gizmo::hit_test_scale(
                                    pos, handle_len, &view_proj, panel_rect, mp,
                                )
                            });
                            let dragging = if insp.gizmo_scale_uniform {
                                Some(crate::gizmo::ScaleHandle::Uniform)
                            } else {
                                insp.gizmo_scale_axis.map(crate::gizmo::ScaleHandle::Axis)
                            };
                            crate::gizmo::draw_scale_gizmo(
                                ui.painter(),
                                pos,
                                handle_len,
                                &view_proj,
                                panel_rect,
                                hovered,
                                dragging,
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
                            .selectable_label(
                                insp.gizmo_mode == bsengine_core::GizmoMode::Scale,
                                egui_phosphor::regular::CORNERS_OUT,
                            )
                            .on_hover_text("Scale (R)")
                            .clicked()
                        {
                            insp.gizmo_mode = bsengine_core::GizmoMode::Scale;
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

        let instantaneous_fps = 1.0 / ui.ctx().input(|i| i.stable_dt.max(1e-6));
        self.smoothed_fps += (instantaneous_fps - self.smoothed_fps) * 0.1;

        egui::Area::new(egui::Id::new("viewport_stats_overlay"))
            .fixed_pos(egui::Pos2::new(
                panel_rect.max.x - 8.0,
                panel_rect.min.y + 8.0,
            ))
            .pivot(egui::Align2::RIGHT_TOP)
            .show(ui.ctx(), |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("{:.0} FPS", self.smoothed_fps))
                            .size(11.0)
                            .color(crate::theme::ACCENT),
                    )
                    .extend(),
                );
            });
    }
}

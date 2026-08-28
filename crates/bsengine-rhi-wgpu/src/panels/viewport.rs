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

        // Terrain brush cursor + drag handling. Not nested inside
        // `is_stopped` above -- `bsengine-app`'s picking system (the writer
        // of `terrain_pick`) doesn't gate on play state either, so this
        // stays consistent with the backend it drives rather than
        // introducing a restriction Task 8 never imposed. All of the actual
        // terrain mutation lives downstream (`bsengine-app`'s
        // preview/commit systems reading `terrain_brush_stroke`); this
        // block only turns a drag into that one shared field.
        if insp.terrain_brush_active {
            if let Some((terrain_id, world_pos)) = insp.terrain_pick {
                if let Some(view_proj) = insp.editor_view_proj {
                    // Ground-plane (Y=0 relative to the pick) ring of points
                    // around the brush center, reusing the same
                    // ring_points/world_to_screen helpers the rotate gizmo
                    // already uses for its axis rings -- there's no
                    // dedicated "draw a world-space circle" helper in
                    // `gizmo` yet, so this reuses the two pieces of math it
                    // does expose rather than duplicating the projection
                    // logic here.
                    let radius = insp.terrain_brush_settings.radius.max(0.01);
                    let ring = crate::gizmo::ring_points(
                        glam::Vec3::from(world_pos),
                        crate::gizmo::AXIS_Y,
                        radius,
                    );
                    let stroke = egui::Stroke::new(2.0, crate::theme::ACCENT);
                    for i in 0..ring.len() {
                        let j = (i + 1) % ring.len();
                        if let (Some(a), Some(b)) = (
                            crate::gizmo::world_to_screen(ring[i], &view_proj, panel_rect),
                            crate::gizmo::world_to_screen(ring[j], &view_proj, panel_rect),
                        ) {
                            ui.painter().line_segment([a, b], stroke);
                        }
                    }
                }

                if response.dragged() {
                    insp.terrain_brush_stroke = Some(bsengine_core::TerrainBrushStroke {
                        terrain_entity_id: terrain_id,
                        world_pos,
                    });
                } else if response.drag_stopped() {
                    insp.terrain_brush_stroke = None;
                }
            } else {
                insp.terrain_brush_stroke = None;
            }
        }

        egui::Area::new(egui::Id::new("viewport_mini_toolbar"))
            .fixed_pos(panel_rect.min + egui::vec2(8.0, 8.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    // Toggling the tool and opening its settings popup share
                    // the same click on purpose -- there's no separate gear
                    // icon, so `toggle_popup` is called in lockstep with the
                    // active flag: turning the brush on surfaces its
                    // settings immediately, turning it off tucks them away
                    // again rather than leaving a popup for an inactive tool.
                    let terrain_brush_popup_id =
                        ui.make_persistent_id("viewport_terrain_brush_settings_popup");
                    let mut terrain_brush_button: Option<egui::Response> = None;
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

                        // Explicit size, not a shrink-wrapped
                        // `selectable_label` like its sibling buttons above --
                        // this is the one button in the row paired with a
                        // `popup_below_widget`, which derives the popup's
                        // width from this response's rect and
                        // `debug_assert!`s it non-negative, which a
                        // near-zero shrink-wrapped width can violate under
                        // the headless empty-font `Context` this panel is
                        // tested with (same reasoning as `hierarchy.rs`'s
                        // "Create Terrain" button).
                        let brush_button_size = ui.spacing().interact_size;
                        let response = ui
                            .add_sized(
                                brush_button_size,
                                egui::SelectableLabel::new(
                                    insp.terrain_brush_active,
                                    egui_phosphor::regular::PAINT_BRUSH,
                                ),
                            )
                            .on_hover_text("Terrain Brush");
                        if response.clicked() {
                            insp.terrain_brush_active = !insp.terrain_brush_active;
                            ui.memory_mut(|m| m.toggle_popup(terrain_brush_popup_id));
                        }
                        terrain_brush_button = Some(response);
                    });

                    if let Some(button_response) = &terrain_brush_button {
                        egui::popup::popup_below_widget(
                            ui,
                            terrain_brush_popup_id,
                            button_response,
                            egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                            |ui| {
                                ui.set_min_width(180.0);
                                ui.label("Terrain Brush");
                                ui.separator();

                                let settings = &mut insp.terrain_brush_settings;
                                let is_height = matches!(
                                    settings.kind,
                                    bsengine_core::TerrainBrushKind::Height { .. }
                                );
                                ui.horizontal(|ui| {
                                    if ui.selectable_label(is_height, "Height").clicked()
                                        && !is_height
                                    {
                                        settings.kind =
                                            bsengine_core::TerrainBrushKind::Height { raise: true };
                                    }
                                    if ui.selectable_label(!is_height, "Paint").clicked()
                                        && is_height
                                    {
                                        settings.kind =
                                            bsengine_core::TerrainBrushKind::Paint { layer: 0 };
                                    }
                                });

                                match &mut settings.kind {
                                    bsengine_core::TerrainBrushKind::Height { raise } => {
                                        ui.horizontal(|ui| {
                                            if ui.selectable_label(*raise, "Raise").clicked() {
                                                *raise = true;
                                            }
                                            if ui.selectable_label(!*raise, "Lower").clicked() {
                                                *raise = false;
                                            }
                                        });
                                    }
                                    bsengine_core::TerrainBrushKind::Paint { layer } => {
                                        ui.horizontal(|ui| {
                                            for l in 0u8..4 {
                                                if ui
                                                    .selectable_label(*layer == l, l.to_string())
                                                    .clicked()
                                                {
                                                    *layer = l;
                                                }
                                            }
                                        });
                                    }
                                }

                                ui.add(
                                    egui::Slider::new(&mut settings.radius, 0.1..=50.0)
                                        .text("Radius"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut settings.strength, 0.0..=1.0)
                                        .text("Strength"),
                                );
                            },
                        );
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{InspectorEntityInfo, InspectorState};

    /// Headless multi-frame harness for `ViewportPanel::ui`, following
    /// `hierarchy.rs`'s `HierarchyHarness` exactly (same empty-font
    /// `Context`, same `frame`/`click` shape) -- no equivalent harness
    /// existed in this file before this task. `entities_snapshot` is always
    /// empty: none of these tests exercise the camera-frustum overlay,
    /// which is the only thing here that reads it.
    struct ViewportHarness {
        egui_ctx: egui::Context,
        screen_rect: egui::Rect,
        insp: InspectorState,
        entities_snapshot: Vec<InspectorEntityInfo>,
        panel: ViewportPanel,
    }

    impl ViewportHarness {
        fn new() -> Self {
            let egui_ctx = egui::Context::default();
            egui_ctx.set_fonts(egui::FontDefinitions::empty());
            Self {
                egui_ctx,
                screen_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0)),
                insp: InspectorState::default(),
                entities_snapshot: Vec::new(),
                panel: ViewportPanel::default(),
            }
        }

        /// Runs one frame with `events` delivered to it. Same "hit-tests
        /// against the *previous* frame's widget rects" caveat as
        /// `HierarchyHarness::frame` applies to any position read from this
        /// call's own return value.
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
                            type_registry: None,
                        };
                        self.panel.ui(ui, &mut ctx);
                    });
                },
            )
        }

        /// A frame that delivers a single press+release click at `pos`.
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

        /// A frame that presses the primary button at `pos`, deliberately
        /// *not* combined with any subsequent move in the same event list.
        /// egui's own `InputState::is_decidedly_dragging` doc comment says
        /// it "can return true on the same frame the drag is released, but
        /// NOT on the first frame it was started", and its body explicitly
        /// requires `!self.any_pressed()` -- so a press-and-move sent
        /// together in one `RawInput` never reports `dragged() == true`,
        /// confirmed empirically: the press must land on its own frame
        /// first (its effect only visible to the *next* frame's snapshot,
        /// which is itself computed from the *previous* frame's widget
        /// rects per `interaction.rs`'s own `InteractionSnapshot` doc
        /// comment -- "Calculated at the start of each frame based on:
        /// Widget rects from previous frame").
        fn press(&mut self, pos: egui::Pos2) -> egui::FullOutput {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ])
        }

        /// A frame that moves the pointer to `pos` while the primary button
        /// is still held down from an earlier `press()` call -- no new
        /// `PointerButton` event, since egui's own button-down state
        /// persists across frames until a release event arrives. This is
        /// the frame on which `dragged()` actually first turns true (see
        /// `press`'s doc comment).
        fn drag_to(&mut self, pos: egui::Pos2) -> egui::FullOutput {
            self.frame(vec![egui::Event::PointerMoved(pos)])
        }

        /// A frame that releases the primary button at `pos`, with no
        /// preceding move -- for continuing an already-started drag from a
        /// prior `drag()` call into a release.
        fn release(&mut self, pos: egui::Pos2) -> egui::FullOutput {
            self.frame(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            }])
        }
    }

    /// Every literal string egui rendered as text in one frame, paired with
    /// the position it was drawn at. Ported from `hierarchy.rs`'s identical
    /// helper (itself ported from `inspector.rs`) -- see that copy's doc
    /// comment for the full rationale. Toolbar icon buttons render their
    /// `egui_phosphor` glyph as ordinary text (icon fonts are just unicode
    /// characters), so this same helper locates them too: click the
    /// returned `pos` as-is, with no "reach the centre" offset, since
    /// `FontDefinitions::empty()` gives every galley zero size and a
    /// top-down `Ui` centers a zero-sized widget's content on the padded
    /// rect's own centre.
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

    #[test]
    fn clicking_the_brush_button_toggles_terrain_brush_active() {
        let mut harness = ViewportHarness::new();
        assert!(!harness.insp.terrain_brush_active);

        // The toolbar lives inside an `egui::Area`, which -- unlike
        // `HierarchyPanel`'s toolbar buttons, drawn directly on the panel's
        // own `Ui` -- renders invisibly on the very first frame it's shown.
        // That's egui's own "sizing pass" (see `Prepared::content_ui` in
        // egui's `area.rs`: `if self.sizing_pass { ui_builder =
        // ui_builder.sizing_pass().invisible(); }`), used to compute the
        // Area's size before positioning it for real; it paints nothing
        // that frame. One extra settle frame -- discarded here -- clears
        // that before this test reads back a real button position.
        harness.frame(vec![]);
        let layout = harness.frame(vec![]);
        let (_, brush_pos) = collect_rendered_texts_with_pos(&layout.shapes)
            .into_iter()
            .find(|(t, _)| t == egui_phosphor::regular::PAINT_BRUSH)
            .expect("toolbar must render the terrain brush button's icon glyph");

        harness.click(brush_pos);

        assert!(
            harness.insp.terrain_brush_active,
            "clicking the brush button must activate it"
        );

        // Clicking again toggles it back off.
        let layout2 = harness.frame(vec![]);
        let (_, brush_pos2) = collect_rendered_texts_with_pos(&layout2.shapes)
            .into_iter()
            .find(|(t, _)| t == egui_phosphor::regular::PAINT_BRUSH)
            .expect("brush button must still render after activation");
        harness.click(brush_pos2);
        assert!(
            !harness.insp.terrain_brush_active,
            "clicking the brush button again must deactivate it"
        );
    }

    #[test]
    fn dragging_while_a_terrain_pick_is_present_sets_a_brush_stroke() {
        let mut harness = ViewportHarness::new();
        harness.insp.terrain_brush_active = true;
        harness.insp.terrain_pick = Some((42, [1.0, 0.0, 2.0]));

        // Two settle frames, not one, are needed before the press -- the
        // toolbar/stats-overlay `egui::Area`s each render their first-ever
        // frame as an invisible "sizing pass" (see the comment in
        // `clicking_the_brush_button_toggles_terrain_brush_active`), and
        // during THAT pass their placeholder interact rect is egui's
        // built-in `default_area_size`, which is large enough to cover
        // most of this 400x400 test screen -- including (150,150)/
        // (250,250) below. Since hit-testing a press always uses the
        // *previous* frame's widget rects (see `HierarchyHarness::frame`'s
        // doc comment in `hierarchy.rs`), pressing on settle frame 1's
        // heels would hit-test against that oversized placeholder and
        // misattribute the press to the toolbar Area instead of the
        // viewport's own `response`. A second settle frame lets the Areas
        // shrink to their real (small, corner-hugging) size first.
        harness.frame(vec![]);
        harness.frame(vec![]);

        let from = egui::Pos2::new(150.0, 150.0);
        let to = egui::Pos2::new(250.0, 250.0);
        // The press and the move-while-held are deliberately two separate
        // frames -- see `ViewportHarness::press`'s doc comment for why a
        // single frame combining both never reports `dragged() == true`.
        harness.press(from);
        harness.drag_to(to);

        let stroke = harness
            .insp
            .terrain_brush_stroke
            .expect("dragging while a terrain pick is present must set a brush stroke");
        assert_eq!(stroke.terrain_entity_id, 42);
        assert_eq!(stroke.world_pos, [1.0, 0.0, 2.0]);
    }

    #[test]
    fn releasing_the_drag_clears_the_brush_stroke() {
        let mut harness = ViewportHarness::new();
        harness.insp.terrain_brush_active = true;
        harness.insp.terrain_pick = Some((42, [1.0, 0.0, 2.0]));

        // See the equivalent two-settle-frame comment in
        // `dragging_while_a_terrain_pick_is_present_sets_a_brush_stroke`.
        harness.frame(vec![]);
        harness.frame(vec![]);

        let from = egui::Pos2::new(150.0, 150.0);
        let to = egui::Pos2::new(250.0, 250.0);
        harness.press(from);
        harness.drag_to(to);
        assert!(
            harness.insp.terrain_brush_stroke.is_some(),
            "precondition: the drag must have set a stroke before release can clear it"
        );

        harness.release(to);

        assert_eq!(
            harness.insp.terrain_brush_stroke, None,
            "releasing the drag must clear the brush stroke"
        );
    }
}

use bsengine_core::{EditorPanel, EditorPanelContext, InspectorCmd, InspectorEntityInfo};

pub struct HierarchyPanel;

/// Which row is currently being renamed inline (double-click target), and
/// the in-progress edit buffer. `None` when no row is being renamed.
#[derive(Clone, Default)]
struct RenameState {
    entity_id: u64,
    buffer: String,
}

impl EditorPanel for HierarchyPanel {
    fn id(&self) -> &str {
        "hierarchy"
    }

    fn title(&self) -> String {
        "Hierarchy".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorPanelContext) {
        let insp = &mut *ctx.insp;
        let entities_snapshot = ctx.entities_snapshot;
        let current_sel = insp.selected_id;

        let mut spawn_entity = false;
        let mut despawn_entity = false;
        let mut new_selection: Option<Vec<u64>> = None;
        let mut new_sel = insp.selected_id;
        let mut set_parent: Option<(u64, u64)> = None;
        let mut remove_parent: Option<u64> = None;
        let mut duplicate: Option<u64> = None;
        let mut rename_commit: Option<(u64, String)> = None;
        let mut despawn_ids: Vec<u64> = Vec::new();

        ui.horizontal(|ui| {
            if ui.button("＋").clicked() {
                spawn_entity = true;
            }
            if ui
                .add_enabled(current_sel.is_some(), egui::Button::new("－"))
                .clicked()
            {
                despawn_entity = true;
            }
        });
        ui.label("Ctrl+click: toggle · Shift+click: range · double-click: rename · right-click: menu · drag onto a row to reparent");
        ui.separator();

        // Egui's own per-widget memory persists the rename-edit buffer across
        // frames using a fixed Id — we don't need our own InspectorState field
        // for this, matching how CollapsingHeader persists its own open/closed
        // state without any app-level bookkeeping.
        let rename_id = egui::Id::new("hierarchy_rename_state");
        let mut rename_state: Option<RenameState> = ui.data(|d| d.get_temp(rename_id));

        egui::ScrollArea::vertical().show(ui, |ui| {
            for root in entities_snapshot.iter().filter(|e| e.parent_id.is_none()) {
                Self::draw_row(
                    ui,
                    root,
                    entities_snapshot,
                    current_sel,
                    &mut new_selection,
                    &mut new_sel,
                    &mut set_parent,
                    &mut remove_parent,
                    &mut duplicate,
                    &mut despawn_ids,
                    &mut rename_state,
                    &mut rename_commit,
                    0,
                );
            }

            // Root drop zone: drag a row here to unparent it. Occupies the
            // remaining empty space below the tree.
            let (_, root_drop_response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 40.0),
                egui::Sense::hover(),
            );
            if let Some(dropped_id) = root_drop_response.dnd_release_payload::<u64>() {
                remove_parent = Some(*dropped_id);
            }
        });

        ui.data_mut(|d| {
            if let Some(state) = rename_state {
                d.insert_temp(rename_id, state);
            } else {
                d.remove_temp::<RenameState>(rename_id);
            }
        });

        if spawn_entity {
            insp.cmd_queue.push(InspectorCmd::SpawnEntity {
                name: format!("Entity {}", entities_snapshot.len()),
            });
        }
        if despawn_entity {
            let ids: Vec<u64> = entities_snapshot
                .iter()
                .filter(|e| e.selected)
                .map(|e| e.id)
                .collect();
            let ids: Vec<u64> = if ids.is_empty() {
                current_sel.into_iter().collect()
            } else {
                ids
            };
            for id in ids {
                insp.cmd_queue.push(InspectorCmd::Despawn { id });
            }
            insp.selected_id = None;
            insp.cmd_queue
                .push(InspectorCmd::SetSelection { ids: vec![] });
        }
        if let Some(ids) = new_selection {
            insp.cmd_queue.push(InspectorCmd::SetSelection { ids });
        }
        if new_sel != insp.selected_id {
            insp.selected_id = new_sel;
            insp.sync_selection();
        }
        if let Some((child_id, parent_id)) = set_parent {
            if child_id != parent_id {
                insp.cmd_queue
                    .push(InspectorCmd::SetParent { id: child_id, parent_id });
            }
        }
        if let Some(id) = remove_parent {
            insp.cmd_queue.push(InspectorCmd::RemoveParent { id });
        }
        if let Some(id) = duplicate {
            insp.cmd_queue.push(InspectorCmd::Duplicate { id });
        }
        if !despawn_ids.is_empty() {
            for id in despawn_ids {
                insp.cmd_queue.push(InspectorCmd::Despawn { id });
            }
            insp.selected_id = None;
            insp.cmd_queue
                .push(InspectorCmd::SetSelection { ids: vec![] });
        }
        if let Some((id, name)) = rename_commit {
            if !name.is_empty() {
                insp.cmd_queue.push(InspectorCmd::RenameEntity { id, name });
            }
        }
    }
}

impl HierarchyPanel {
    #[allow(clippy::too_many_arguments)]
    fn draw_row(
        ui: &mut egui::Ui,
        info: &InspectorEntityInfo,
        all_entities: &[InspectorEntityInfo],
        current_sel: Option<u64>,
        new_selection: &mut Option<Vec<u64>>,
        new_sel: &mut Option<u64>,
        set_parent: &mut Option<(u64, u64)>,
        remove_parent: &mut Option<u64>,
        duplicate: &mut Option<u64>,
        despawn_ids: &mut Vec<u64>,
        rename_state: &mut Option<RenameState>,
        rename_commit: &mut Option<(u64, String)>,
        depth: usize,
    ) {
        let children: Vec<&InspectorEntityInfo> = all_entities
            .iter()
            .filter(|e| e.parent_id == Some(info.id))
            .collect();
        let label = info.name.as_deref().unwrap_or("(unnamed)");
        let text = format!("[{}] {}", info.id, label);
        let is_renaming = rename_state.as_ref().is_some_and(|r| r.entity_id == info.id);

        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 16.0);

            let row_response = if is_renaming {
                let state = rename_state.as_mut().expect("checked by is_renaming");
                let edit_response =
                    ui.add(egui::TextEdit::singleline(&mut state.buffer).id_salt(info.id));
                if edit_response.lost_focus()
                    && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter))
                {
                    *rename_commit = Some((info.id, state.buffer.clone()));
                    *rename_state = None;
                } else if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
                    *rename_state = None;
                }
                edit_response
            } else if children.is_empty() {
                ui.selectable_label(info.selected, text)
            } else {
                egui::CollapsingHeader::new(text)
                    .id_salt(info.id)
                    .default_open(true)
                    .show(ui, |ui| {
                        for child in &children {
                            Self::draw_row(
                                ui,
                                child,
                                all_entities,
                                current_sel,
                                new_selection,
                                new_sel,
                                set_parent,
                                remove_parent,
                                duplicate,
                                despawn_ids,
                                rename_state,
                                rename_commit,
                                depth + 1,
                            );
                        }
                    })
                    .header_response
            };

            if is_renaming {
                return;
            }

            if row_response.clicked() {
                let mods = ui.ctx().input(|i| i.modifiers);
                if mods.shift {
                    let idx = all_entities.iter().position(|e| e.id == info.id).unwrap_or(0);
                    let anchor_idx = current_sel
                        .and_then(|id| all_entities.iter().position(|e| e.id == id))
                        .unwrap_or(idx);
                    let (lo, hi) = (anchor_idx.min(idx), anchor_idx.max(idx));
                    *new_selection = Some(all_entities[lo..=hi].iter().map(|e| e.id).collect());
                } else if mods.ctrl {
                    let mut ids: Vec<u64> = all_entities
                        .iter()
                        .filter(|e| e.selected)
                        .map(|e| e.id)
                        .collect();
                    if let Some(pos) = ids.iter().position(|&id| id == info.id) {
                        ids.remove(pos);
                    } else {
                        ids.push(info.id);
                    }
                    *new_selection = Some(ids);
                } else {
                    *new_selection = Some(vec![info.id]);
                }
                *new_sel = Some(info.id);
            }

            if row_response.double_clicked() {
                *rename_state = Some(RenameState {
                    entity_id: info.id,
                    buffer: info.name.clone().unwrap_or_default(),
                });
            }

            row_response.dnd_set_drag_payload(info.id);
            if let Some(dropped_id) = row_response.dnd_release_payload::<u64>() {
                *set_parent = Some((*dropped_id, info.id));
            }

            row_response.context_menu(|ui| {
                if ui.button("Rename").clicked() {
                    *rename_state = Some(RenameState {
                        entity_id: info.id,
                        buffer: info.name.clone().unwrap_or_default(),
                    });
                    ui.close_menu();
                }
                if ui.button("Duplicate").clicked() {
                    *duplicate = Some(info.id);
                    ui.close_menu();
                }
                if ui.button("Delete").clicked() {
                    despawn_ids.push(info.id);
                    ui.close_menu();
                }
                if info.parent_id.is_some() && ui.button("Unparent").clicked() {
                    *remove_parent = Some(info.id);
                    ui.close_menu();
                }
            });
        });
    }
}

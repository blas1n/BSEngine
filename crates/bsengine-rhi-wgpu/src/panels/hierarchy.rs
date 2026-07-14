use bsengine_core::{EditorPanel, EditorPanelContext, InspectorCmd, InspectorEntityInfo};

pub struct HierarchyPanel;

/// Which row is currently being renamed inline (double-click target), and
/// the in-progress edit buffer. `None` when no row is being renamed.
#[derive(Clone, Default)]
struct RenameState {
    entity_id: u64,
    buffer: String,
}

/// Read-only, whole-tree context threaded through the `draw_row` recursion
/// unchanged at every depth — bundled into one struct rather than three
/// separate positional parameters to keep `draw_row`'s already-long
/// argument list from growing further.
struct TreeCtx<'a> {
    all_entities: &'a [InspectorEntityInfo],
    current_sel: Option<u64>,
    /// Entity ids in depth-first rendered order (same traversal `draw_row`
    /// itself performs: roots in snapshot order, each subtree's children
    /// immediately after their parent). Shift-click range-select uses this
    /// instead of `all_entities`' raw snapshot-array order, so the
    /// highlighted range actually matches what's visually between the two
    /// clicked rows in the tree — snapshot order and render order are not
    /// the same thing once entities have parents.
    order: &'a [u64],
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

        let order = Self::dfs_order(entities_snapshot);
        let tree = TreeCtx {
            all_entities: entities_snapshot,
            current_sel,
            order: &order,
        };

        egui::ScrollArea::vertical().show(ui, |ui| {
            for root in entities_snapshot.iter().filter(|e| e.parent_id.is_none()) {
                Self::draw_row(
                    ui,
                    root,
                    &tree,
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
            let (_, root_drop_response) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::hover());
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
                insp.cmd_queue.push(InspectorCmd::SetParent {
                    id: child_id,
                    parent_id,
                });
            }
        }
        if let Some(id) = remove_parent {
            insp.cmd_queue.push(InspectorCmd::RemoveParent { id });
        }
        if let Some(id) = duplicate {
            insp.cmd_queue.push(InspectorCmd::Duplicate { id });
        }
        if !despawn_ids.is_empty() {
            for &id in &despawn_ids {
                insp.cmd_queue.push(InspectorCmd::Despawn { id });
            }
            // Only touch selection for ids that were actually deleted here —
            // a context-menu Delete on an unselected row must not silently
            // clear whatever else was selected (unlike the toolbar "－"
            // button above, which always despawns exactly the selected set).
            let selected_ids: Vec<u64> = entities_snapshot
                .iter()
                .filter(|e| e.selected)
                .map(|e| e.id)
                .collect();
            let remaining_selected: Vec<u64> = selected_ids
                .iter()
                .copied()
                .filter(|id| !despawn_ids.contains(id))
                .collect();
            if remaining_selected.len() != selected_ids.len() {
                insp.cmd_queue.push(InspectorCmd::SetSelection {
                    ids: remaining_selected.clone(),
                });
            }
            if insp.selected_id.is_some_and(|id| despawn_ids.contains(&id)) {
                insp.selected_id = remaining_selected.first().copied();
                insp.sync_selection();
            }
        }
        if let Some((id, name)) = rename_commit {
            if !name.is_empty() {
                insp.cmd_queue.push(InspectorCmd::RenameEntity { id, name });
            }
        }
    }
}

impl HierarchyPanel {
    /// Would setting `dropped_id`'s parent to `target_id` create a cycle in
    /// the `parent_id` graph? True if `dropped_id` is `target_id` itself or
    /// one of `target_id`'s existing ancestors (walking up via `parent_id`).
    /// Bounded by `all_entities.len()` steps so a pre-existing cycle in the
    /// snapshot (which should never happen, but this is UI code reacting to
    /// a live snapshot, not the source of truth) can't spin forever.
    fn would_create_cycle(
        all_entities: &[InspectorEntityInfo],
        dropped_id: u64,
        target_id: u64,
    ) -> bool {
        let mut current = Some(target_id);
        let mut steps = 0;
        while let Some(id) = current {
            if id == dropped_id {
                return true;
            }
            steps += 1;
            if steps > all_entities.len() {
                return true;
            }
            current = all_entities
                .iter()
                .find(|e| e.id == id)
                .and_then(|e| e.parent_id);
        }
        false
    }

    /// Entity ids in depth-first rendered order — same traversal `draw_row`
    /// performs (roots in snapshot order, then each subtree's children
    /// immediately after their parent). A `visited` guard makes this safe
    /// against a pre-existing cycle in the live snapshot (external data,
    /// not something the UI's own drag-and-drop can create — see
    /// `would_create_cycle`), unlike `draw_row`'s own unguarded recursion.
    fn dfs_order(all_entities: &[InspectorEntityInfo]) -> Vec<u64> {
        let mut order = Vec::with_capacity(all_entities.len());
        let mut visited = std::collections::HashSet::with_capacity(all_entities.len());
        for root in all_entities.iter().filter(|e| e.parent_id.is_none()) {
            Self::push_dfs(root, all_entities, &mut order, &mut visited);
        }
        order
    }

    fn push_dfs(
        info: &InspectorEntityInfo,
        all_entities: &[InspectorEntityInfo],
        order: &mut Vec<u64>,
        visited: &mut std::collections::HashSet<u64>,
    ) {
        if !visited.insert(info.id) {
            return;
        }
        order.push(info.id);
        for child in all_entities.iter().filter(|e| e.parent_id == Some(info.id)) {
            Self::push_dfs(child, all_entities, order, visited);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_row(
        ui: &mut egui::Ui,
        info: &InspectorEntityInfo,
        tree: &TreeCtx,
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
        let children: Vec<&InspectorEntityInfo> = tree
            .all_entities
            .iter()
            .filter(|e| e.parent_id == Some(info.id))
            .collect();
        let label = info.name.as_deref().unwrap_or("(unnamed)");
        let text = format!("[{}] {}", info.id, label);
        let is_renaming = rename_state
            .as_ref()
            .is_some_and(|r| r.entity_id == info.id);

        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 16.0);

            let row_response = if is_renaming {
                let state = rename_state.as_mut().expect("checked by is_renaming");
                let edit_response =
                    ui.add(egui::TextEdit::singleline(&mut state.buffer).id_salt(info.id));
                if edit_response.lost_focus() && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter))
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
                                tree,
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

            // `selectable_label`/`CollapsingHeader` only allocate with
            // `Sense::click()`, so `row_response.drag_started()` (which
            // `dnd_set_drag_payload` gates on) would never fire on its own.
            // Mirror `Ui::dnd_drag_source`'s own internals: union in a
            // second same-rect interact that senses drags, so the row
            // becomes an actual DnD source without disturbing its existing
            // click/double-click behavior (`Response::union` ORs
            // `clicked`/`double_clicked`/`drag_started`/`dragged`).
            let drag_id = ui.id().with(("hierarchy_row_drag", info.id));
            let drag_response = ui.interact(row_response.rect, drag_id, egui::Sense::drag());
            let row_response = drag_response | row_response;

            if row_response.clicked() {
                let mods = ui.ctx().input(|i| i.modifiers);
                if mods.shift {
                    // Range-select by *rendered* (depth-first) order, not
                    // raw snapshot-array order — the two diverge once
                    // entities have parents, and using array order here
                    // would select a range with no visual relationship to
                    // what's actually between the two clicked rows.
                    let idx = tree.order.iter().position(|&id| id == info.id).unwrap_or(0);
                    let anchor_idx = tree
                        .current_sel
                        .and_then(|id| tree.order.iter().position(|&oid| oid == id))
                        .unwrap_or(idx);
                    let (lo, hi) = (anchor_idx.min(idx), anchor_idx.max(idx));
                    *new_selection = Some(tree.order[lo..=hi].to_vec());
                } else if mods.ctrl {
                    let mut ids: Vec<u64> = tree
                        .all_entities
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
                let dropped_id = *dropped_id;
                // Refuse reparents that would create a cycle (e.g. dragging a
                // parent row onto one of its own descendants, which renders
                // right below it and is an easy accidental drop target). A
                // cycle would make `draw_row`'s root filter
                // (`parent_id.is_none()`) unable to ever reach that subtree
                // again — the entity and everything under it would silently
                // vanish from the panel with no error.
                if !Self::would_create_cycle(tree.all_entities, dropped_id, info.id) {
                    *set_parent = Some((dropped_id, info.id));
                }
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

use bsengine_core::{EditorPanel, EditorPanelContext, InspectorCmd, InspectorEntityInfo};

/// The Hierarchy panel: shows the entity tree with selection, drag-to-reparent, and inline rename.
pub struct HierarchyPanel;

/// Which row is currently being renamed inline (double-click target), and
/// the in-progress edit buffer. `None` when no row is being renamed.
#[derive(Clone, Default)]
struct RenameState {
    entity_id: u64,
    buffer: String,
}

/// Which row's "Create Prefab" name prompt is currently open (opened via
/// the context menu), and the in-progress name buffer. `None` when no
/// prompt is open. Mirrors `RenameState` exactly -- same temp-storage
/// mechanism, same inline-row-replacement rendering -- just commits to
/// `InspectorCmd::CreatePrefab` instead of `InspectorCmd::RenameEntity`.
#[derive(Clone, Default)]
struct CreatePrefabState {
    entity_id: u64,
    buffer: String,
}

/// Default terrain params the "Create Terrain" toolbar button queues
/// alongside the user-entered heightmap path. Unlike `CreatePrefabState`'s
/// name prompt, the popup below only collects the one field a fresh terrain
/// can't have a sane default for; chunk grid/size/height scale get fixed
/// defaults instead of more text fields, matching this crate's own
/// `terrain_write` MCP tool test fixtures (`bsengine-editor/src/plugin.rs`)
/// rather than being invented here.
const DEFAULT_TERRAIN_CHUNK_COUNT: (u32, u32) = (4, 4);
const DEFAULT_TERRAIN_CHUNK_SIZE: f32 = 32.0;
const DEFAULT_TERRAIN_HEIGHT_SCALE: f32 = 20.0;

/// Key under which the "Create Terrain" popup's heightmap-path buffer is
/// stored in egui's own per-frame data store (`ui.data`/`ui.data_mut`,
/// unscoped by widget hierarchy -- same mechanism `rename_id`/
/// `create_prefab_id` below already rely on). A named constant, not an
/// inline literal at the one production use site, so the test module can
/// seed the buffer directly -- driving a real text edit via simulated
/// keystrokes is unnecessary ceremony here -- without the two copies of
/// the string ever being able to drift apart.
const CREATE_TERRAIN_BUFFER_ID_STR: &str = "hierarchy_create_terrain_buffer";

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
        let mut apply_to_prefab_ids: Vec<u64> = Vec::new();
        let mut attach_script: Option<(u64, String)> = None;
        let mut create_terrain_commit: Option<String> = None;

        // "Create Terrain" popup state: unlike `RenameState`/
        // `CreatePrefabState`, this isn't keyed to a row -- Create Terrain
        // spawns a brand-new root entity, the same shape as the "Spawn
        // Entity" button next to it, not a per-row context-menu action --
        // so egui's own popup-open memory (`toggle_popup`/`is_popup_open`,
        // the same mechanism `draw_add_component` in inspector.rs uses)
        // drives visibility, and only the text buffer needs our own
        // temp-storage slot.
        let create_terrain_popup_id = ui.make_persistent_id("hierarchy_create_terrain_popup");
        let create_terrain_buffer_id = egui::Id::new(CREATE_TERRAIN_BUFFER_ID_STR);
        let mut create_terrain_buffer: String = ui
            .data(|d| d.get_temp(create_terrain_buffer_id))
            .unwrap_or_default();

        let mut create_terrain_button: Option<egui::Response> = None;
        ui.horizontal(|ui| {
            if ui
                .button(egui_phosphor::regular::PLUS)
                .on_hover_text("Spawn Entity")
                .clicked()
            {
                spawn_entity = true;
            }
            if ui
                .add_enabled(
                    current_sel.is_some(),
                    egui::Button::new(egui_phosphor::regular::MINUS),
                )
                .on_hover_text("Despawn Selected")
                .clicked()
            {
                despawn_entity = true;
            }
            // Explicit size, not a shrink-wrapped `ui.button` -- mirrors
            // `draw_add_component`'s own button in inspector.rs, and for
            // the same reason: `popup_below_widget` derives the popup's
            // width from this response's rect and `debug_assert!`s it
            // non-negative, which a near-zero shrink-wrapped width can
            // violate under the headless empty-font `Context` these panels
            // are tested with.
            let button_width = ui.available_width().clamp(48.0, 160.0);
            let response = ui.add_sized(
                [button_width, ui.spacing().interact_size.y],
                egui::Button::new(format!(
                    "{} Create Terrain",
                    egui_phosphor::regular::MOUNTAINS
                )),
            );
            if response.clicked() {
                let was_open = ui.memory(|m| m.is_popup_open(create_terrain_popup_id));
                ui.memory_mut(|m| m.toggle_popup(create_terrain_popup_id));
                if !was_open {
                    create_terrain_buffer.clear();
                }
            }
            create_terrain_button = Some(response);
        });

        if let Some(button_response) = &create_terrain_button {
            egui::popup::popup_below_widget(
                ui,
                create_terrain_popup_id,
                button_response,
                egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    ui.set_min_width(220.0);
                    ui.label("Heightmap path:");
                    let edit_response = ui.text_edit_singleline(&mut create_terrain_buffer);
                    let commit_by_enter = edit_response.lost_focus()
                        && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter));
                    let commit_by_button = ui.button("Create").clicked();
                    if (commit_by_enter || commit_by_button) && !create_terrain_buffer.is_empty() {
                        create_terrain_commit = Some(create_terrain_buffer.clone());
                        ui.memory_mut(|m| m.close_popup());
                    }
                },
            );
        }

        ui.data_mut(|d| d.insert_temp(create_terrain_buffer_id, create_terrain_buffer));
        ui.horizontal(|ui| {
            ui.label(egui_phosphor::regular::MAGNIFYING_GLASS);
            ui.text_edit_singleline(&mut insp.hierarchy_search);
        });
        ui.label("Ctrl+click: toggle · Shift+click: range · double-click: rename · right-click: menu · drag onto a row to reparent");
        ui.separator();

        // Egui's own per-widget memory persists the rename-edit buffer across
        // frames using a fixed Id — we don't need our own InspectorState field
        // for this, matching how CollapsingHeader persists its own open/closed
        // state without any app-level bookkeeping.
        let rename_id = egui::Id::new("hierarchy_rename_state");
        let mut rename_state: Option<RenameState> = ui.data(|d| d.get_temp(rename_id));

        let create_prefab_id = egui::Id::new("hierarchy_create_prefab_state");
        let mut create_prefab_state: Option<CreatePrefabState> =
            ui.data(|d| d.get_temp(create_prefab_id));
        let mut create_prefab_commit: Option<(u64, String)> = None;

        let order = Self::dfs_order(entities_snapshot);
        let tree = TreeCtx {
            all_entities: entities_snapshot,
            current_sel,
            order: &order,
        };

        egui::ScrollArea::vertical().show(ui, |ui| {
            if insp.hierarchy_search.is_empty() {
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
                        &mut apply_to_prefab_ids,
                        &mut rename_state,
                        &mut rename_commit,
                        &mut attach_script,
                        &mut create_prefab_state,
                        &mut create_prefab_commit,
                        0,
                    );
                }

                // Root drop zone: drag a row here to unparent it. Occupies
                // the remaining empty space below the tree.
                let (_, root_drop_response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 40.0),
                    egui::Sense::hover(),
                );
                if let Some(dropped_id) = root_drop_response.dnd_release_payload::<u64>() {
                    remove_parent = Some(*dropped_id);
                }
            } else {
                // Filtered mode: a flat list of matches, no tree/DnD/rename —
                // clear the search box to return to the full tree.
                for info in entities_snapshot
                    .iter()
                    .filter(|e| Self::matches_search(e.name.as_deref(), &insp.hierarchy_search))
                {
                    let label = info.name.as_deref().unwrap_or("(unnamed)");
                    let text = format!("{} [{}] {}", Self::icon_for(info), info.id, label);
                    if ui.selectable_label(info.selected, text).clicked() {
                        new_selection = Some(vec![info.id]);
                        new_sel = Some(info.id);
                    }
                }
            }
        });

        ui.data_mut(|d| {
            if let Some(state) = rename_state {
                d.insert_temp(rename_id, state);
            } else {
                d.remove_temp::<RenameState>(rename_id);
            }
        });

        ui.data_mut(|d| {
            if let Some(state) = create_prefab_state {
                d.insert_temp(create_prefab_id, state);
            } else {
                d.remove_temp::<CreatePrefabState>(create_prefab_id);
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
        for &id in &apply_to_prefab_ids {
            insp.cmd_queue
                .push(InspectorCmd::ApplyToPrefab { entity_id: id });
        }
        if let Some((id, name)) = rename_commit {
            if !name.is_empty() {
                insp.cmd_queue.push(InspectorCmd::RenameEntity { id, name });
            }
        }
        if let Some((id, path)) = attach_script {
            insp.cmd_queue.push(InspectorCmd::AttachScript { id, path });
        }
        if let Some((id, name)) = create_prefab_commit {
            if !name.is_empty() {
                insp.cmd_queue.push(InspectorCmd::CreatePrefab {
                    entity_id: id,
                    name,
                });
            }
        }
        if let Some(heightmap_path) = create_terrain_commit {
            insp.cmd_queue.push(InspectorCmd::SpawnTerrain {
                heightmap_path,
                chunk_count: DEFAULT_TERRAIN_CHUNK_COUNT,
                chunk_size: DEFAULT_TERRAIN_CHUNK_SIZE,
                height_scale: DEFAULT_TERRAIN_HEIGHT_SCALE,
            });
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
    /// immediately after their parent). The `visited` guard is defensive
    /// rather than load-bearing: each entity has exactly one `parent_id`, so
    /// any cycle in the graph is necessarily a component with no root-reachable
    /// entity (same reasoning as `would_create_cycle`'s "vanishes from the
    /// panel" doc comment, not a hang risk for `draw_row` either) — but the
    /// guard costs nothing and keeps this function correct even if that
    /// invariant is ever violated by future changes.
    fn dfs_order(all_entities: &[InspectorEntityInfo]) -> Vec<u64> {
        let mut order = Vec::with_capacity(all_entities.len());
        let mut visited = std::collections::HashSet::with_capacity(all_entities.len());
        for root in all_entities.iter().filter(|e| e.parent_id.is_none()) {
            Self::push_dfs(root, all_entities, &mut order, &mut visited);
        }
        order
    }

    /// Case-insensitive substring match used by the Hierarchy search box. An
    /// empty `query` matches every entity (including unnamed ones), so the
    /// panel shows the full tree when the search box is empty.
    fn matches_search(name: Option<&str>, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        name.unwrap_or("")
            .to_lowercase()
            .contains(&query.to_lowercase())
    }

    /// Icon shown next to each Hierarchy row, chosen by the first matching
    /// component on the entity: camera, light, primitive mesh, else a
    /// generic node icon (used for group/empty entities like "Environment").
    fn icon_for(info: &InspectorEntityInfo) -> &'static str {
        if info.camera_fov.is_some() {
            egui_phosphor::regular::CAMERA
        } else if info.light_type.is_some() {
            egui_phosphor::regular::LIGHTBULB
        } else if info.primitive.is_some() {
            egui_phosphor::regular::CUBE
        } else {
            egui_phosphor::regular::TREE_STRUCTURE
        }
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
        apply_to_prefab_ids: &mut Vec<u64>,
        rename_state: &mut Option<RenameState>,
        rename_commit: &mut Option<(u64, String)>,
        attach_script: &mut Option<(u64, String)>,
        create_prefab_state: &mut Option<CreatePrefabState>,
        create_prefab_commit: &mut Option<(u64, String)>,
        depth: usize,
    ) {
        let children: Vec<&InspectorEntityInfo> = tree
            .all_entities
            .iter()
            .filter(|e| e.parent_id == Some(info.id))
            .collect();
        let label = info.name.as_deref().unwrap_or("(unnamed)");
        let text = format!("{} [{}] {}", Self::icon_for(info), info.id, label);
        let is_renaming = rename_state
            .as_ref()
            .is_some_and(|r| r.entity_id == info.id);
        let is_creating_prefab = create_prefab_state
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
            } else if is_creating_prefab {
                let state = create_prefab_state
                    .as_mut()
                    .expect("checked by is_creating_prefab");
                let edit_response = ui.add(
                    egui::TextEdit::singleline(&mut state.buffer)
                        .id_salt(("create_prefab", info.id)),
                );
                if edit_response.lost_focus() && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter))
                {
                    *create_prefab_commit = Some((info.id, state.buffer.clone()));
                    *create_prefab_state = None;
                } else if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
                    *create_prefab_state = None;
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
                                apply_to_prefab_ids,
                                rename_state,
                                rename_commit,
                                attach_script,
                                create_prefab_state,
                                create_prefab_commit,
                                depth + 1,
                            );
                        }
                    })
                    .header_response
            };

            if is_renaming || is_creating_prefab {
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
            //
            // This second interact MUST also sense `click` (not `Sense::drag()`
            // alone): it's added after (i.e. on top of, in hit-test z-order)
            // the row's own click-sensing widget on the *exact same rect*.
            // egui's hit-test (`hit_test_on_close` in egui's `hit_test.rs`)
            // deliberately nulls out the click hit whenever the topmost of two
            // perfectly-overlapping widgets senses only drag — "the top thing
            // senses only drags, so we ignore the click-widget below it" — so
            // with `Sense::drag()` here, every row's click was permanently
            // swallowed regardless of `Response::union`'s own (correct) OR
            // logic. `Sense::click_and_drag()` keeps this topmost widget's
            // click hit intact, since egui special-cases "topmost widget
            // senses both" to report both hits.
            let drag_id = ui.id().with(("hierarchy_row_drag", info.id));
            let drag_response =
                ui.interact(row_response.rect, drag_id, egui::Sense::click_and_drag());
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

            if let Some(payload) =
                row_response.dnd_release_payload::<crate::panels::AssetDragPayload>()
            {
                if payload.kind == crate::panels::AssetKind::Script {
                    *attach_script = Some((info.id, payload.path.to_string_lossy().to_string()));
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
                if ui.button("Create Prefab").clicked() {
                    *create_prefab_state = Some(CreatePrefabState {
                        entity_id: info.id,
                        buffer: info.name.clone().unwrap_or_default(),
                    });
                    ui.close_menu();
                }
                if info.is_prefab_instance && ui.button("Apply to Prefab").clicked() {
                    apply_to_prefab_ids.push(info.id);
                    ui.close_menu();
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(id: u64, parent_id: Option<u64>) -> InspectorEntityInfo {
        InspectorEntityInfo {
            id,
            parent_id,
            ..Default::default()
        }
    }

    #[test]
    fn dfs_order_matches_draw_rows_traversal() {
        // Two roots (1, 5); 1 has children 2 and 3 (in that array order); 2
        // has a grandchild 4. Expected order mirrors exactly what draw_row
        // would render: root 1, then its subtree depth-first (2, then 2's
        // child 4, then 3), then root 5.
        let entities = vec![
            entity(1, None),
            entity(2, Some(1)),
            entity(3, Some(1)),
            entity(4, Some(2)),
            entity(5, None),
        ];
        assert_eq!(HierarchyPanel::dfs_order(&entities), vec![1, 2, 4, 3, 5]);
    }

    #[test]
    fn dfs_order_ignores_entities_unreachable_from_any_root() {
        // 10 and 11 form a 2-cycle (parent_id points at each other) with no
        // path to a root — the `parent_id` relation is single-valued per
        // entity, so a cycle member's parent_id can never simultaneously
        // match a root-reachable id, meaning cycle members are simply never
        // visited (consistent with `would_create_cycle`'s "vanishes from the
        // panel" framing, not a hang risk). Only the genuine root (20)
        // should appear in the output.
        let entities = vec![entity(10, Some(11)), entity(11, Some(10)), entity(20, None)];
        assert_eq!(HierarchyPanel::dfs_order(&entities), vec![20]);
    }

    #[test]
    fn dfs_order_visited_guard_prevents_duplicate_output_on_malformed_duplicate_ids() {
        // Defensive case: two array entries sharing the same id (should
        // never happen from a real snapshot, but this is UI code reacting
        // to external data, not the source of truth). Without the
        // `visited` guard, the second occurrence would be walked again as
        // its own root, producing a duplicate entry.
        let entities = vec![entity(1, None), entity(1, None), entity(2, Some(1))];
        let order = HierarchyPanel::dfs_order(&entities);
        assert_eq!(
            order,
            vec![1, 2],
            "duplicate id must not appear twice in the output"
        );
    }

    #[test]
    fn matches_search_is_case_insensitive_substring() {
        assert!(HierarchyPanel::matches_search(
            Some("PlayerCharacter"),
            "player"
        ));
        assert!(HierarchyPanel::matches_search(
            Some("PlayerCharacter"),
            "CHAR"
        ));
        assert!(!HierarchyPanel::matches_search(
            Some("PlayerCharacter"),
            "zzz"
        ));
    }

    #[test]
    fn matches_search_empty_query_matches_everything() {
        assert!(HierarchyPanel::matches_search(Some("Anything"), ""));
        assert!(HierarchyPanel::matches_search(None, ""));
    }

    #[test]
    fn matches_search_unnamed_entity_only_matches_empty_query() {
        assert!(!HierarchyPanel::matches_search(None, "x"));
    }

    #[test]
    fn icon_for_prefers_camera_over_light_and_mesh() {
        let mut info = entity(1, None);
        info.camera_fov = Some(60.0);
        info.light_type = Some("point".to_string());
        info.primitive = Some("cube".to_string());
        assert_eq!(
            HierarchyPanel::icon_for(&info),
            egui_phosphor::regular::CAMERA
        );
    }

    #[test]
    fn icon_for_falls_back_to_generic_node_icon() {
        let info = entity(1, None);
        assert_eq!(
            HierarchyPanel::icon_for(&info),
            egui_phosphor::regular::TREE_STRUCTURE
        );
    }

    /// Regression test for the tree-view row click failing to register.
    ///
    /// egui hit-tests using the *previous* frame's widget rects
    /// (`Context::begin_pass` reads `viewport.prev_pass.widgets`), so this
    /// needs two `Context::run` passes: the first establishes the row's
    /// rect, the second delivers a press+release at that rect and observes
    /// whether `.clicked()` fires.
    ///
    /// `draw_row` unions a same-rect `ui.interact(.., Sense::drag())` onto
    /// the row's own click-sensing response to enable DnD-reparent. Per
    /// egui's `hit_test_on_close` (Some click hit, Some drag hit branch),
    /// when the *topmost* of two perfectly-overlapping widgets senses only
    /// drag, egui deliberately reports `click: None` for the pair ("the top
    /// thing senses only drags, so we ignore the click-widget below it") —
    /// so `.clicked()` never fires, regardless of `Response::union`'s
    /// otherwise-correct OR-merge. Using `Sense::click_and_drag()` for the
    /// unioned interact keeps that topmost widget's own click hit intact.
    #[test]
    fn row_click_registers_despite_unioned_drag_sense_interact() {
        let ctx = egui::Context::default();
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0));

        let mut row_rect = egui::Rect::NOTHING;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let row_response = ui.selectable_label(false, "row");
                    let drag_id = ui.id().with("row_drag");
                    let _drag_response =
                        ui.interact(row_response.rect, drag_id, egui::Sense::click_and_drag());
                    row_rect = row_response.rect;
                });
            },
        );

        let pos = row_rect.center();
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

        let mut clicked = false;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen_rect),
                events: click_events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let row_response = ui.selectable_label(false, "row");
                    let drag_id = ui.id().with("row_drag");
                    let drag_response =
                        ui.interact(row_response.rect, drag_id, egui::Sense::click_and_drag());
                    let row_response = drag_response | row_response;
                    if row_response.clicked() {
                        clicked = true;
                    }
                });
            },
        );

        assert!(
            clicked,
            "row click must register even with a same-rect drag-sense interact unioned in"
        );
    }

    // -- "Create Terrain" toolbar button -----------------------------------
    //
    // Unlike the tests above (which exercise `HierarchyPanel`'s free
    // helper fns, or a standalone egui scenario with no `HierarchyPanel`
    // involved at all), these drive `HierarchyPanel::ui` itself end to
    // end, mirroring `inspector.rs`'s `PickerHarness`: an empty-font
    // `Context` run across as many frames as a test needs, reading click
    // positions back out of what actually rendered rather than hardcoding
    // pixel coordinates (see `collect_rendered_texts_with_pos`'s doc
    // comment below for exactly why that matters with zero-size galleys).

    use bsengine_core::InspectorState;

    /// Every literal string egui rendered as text in one frame, paired
    /// with the position it was drawn at. Ported from `inspector.rs`'s
    /// identical helper -- see that copy's doc comment for the full
    /// rationale (in short: click the returned `pos` as-is, with no "reach
    /// the centre" offset, since `FontDefinitions::empty()` gives every
    /// galley zero size and a top-down `Ui` centers a zero-sized widget's
    /// content on the padded rect's own centre).
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

    /// String-only view of [`collect_rendered_texts_with_pos`], for
    /// assertions that only care whether something rendered, not where.
    fn collect_rendered_texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
        collect_rendered_texts_with_pos(shapes)
            .into_iter()
            .map(|(text, _)| text)
            .collect()
    }

    /// Headless multi-frame harness for `HierarchyPanel::ui`. `entities_snapshot`
    /// is always empty -- these tests exercise the toolbar's "Create Terrain"
    /// button, which (unlike "Create Prefab") doesn't target a row, so no
    /// entity needs to exist.
    struct HierarchyHarness {
        egui_ctx: egui::Context,
        screen_rect: egui::Rect,
        insp: InspectorState,
        entities_snapshot: Vec<InspectorEntityInfo>,
        panel: HierarchyPanel,
    }

    impl HierarchyHarness {
        fn new() -> Self {
            let egui_ctx = egui::Context::default();
            egui_ctx.set_fonts(egui::FontDefinitions::empty());
            Self {
                egui_ctx,
                screen_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 400.0)),
                insp: InspectorState::default(),
                entities_snapshot: Vec::new(),
                panel: HierarchyPanel,
            }
        }

        /// Runs one frame with `events` delivered to it. Note egui hit-tests
        /// against the *previous* frame's widget rects (documented above on
        /// `row_click_registers_despite_unioned_drag_sense_interact`), so a
        /// position read from this call's own return value is only valid
        /// input to the *next* `frame`/`click` call, never to this one.
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
    }

    #[test]
    fn create_terrain_button_renders_in_the_toolbar() {
        let mut harness = HierarchyHarness::new();
        let output = harness.frame(vec![]);
        let texts = collect_rendered_texts(&output.shapes);
        assert!(
            texts.iter().any(|t| t.contains("Create Terrain")),
            "toolbar must render a Create Terrain button, got: {texts:?}"
        );
    }

    #[test]
    fn clicking_create_terrain_opens_the_heightmap_path_popup() {
        let mut harness = HierarchyHarness::new();

        let layout = harness.frame(vec![]);
        let (_, terrain_button_pos) = collect_rendered_texts_with_pos(&layout.shapes)
            .into_iter()
            .find(|(t, _)| t.contains("Create Terrain"))
            .expect("Create Terrain button must render");

        harness.click(terrain_button_pos);
        // The click frame itself only toggles the popup open and sizes its
        // `Area` from a placeholder; it paints none of the popup's own
        // content yet. A settle frame afterwards is what actually paints
        // "Heightmap path:" and the Create button -- the same,
        // independently confirmed quirk `PickerHarness::open_picker`
        // documents for Add Component's identical popup mechanism in
        // inspector.rs ("the frame after [the click] is the first with
        // real rows").
        let settled = harness.frame(vec![]);
        let texts = collect_rendered_texts(&settled.shapes);
        assert!(
            texts.iter().any(|t| t == "Heightmap path:"),
            "clicking Create Terrain must open the heightmap-path popup, got: {texts:?}"
        );
        assert!(
            harness.insp.cmd_queue.is_empty(),
            "opening the popup must not itself queue a command"
        );
    }

    #[test]
    fn committing_the_heightmap_path_queues_spawn_terrain_with_default_params() {
        let mut harness = HierarchyHarness::new();

        let layout = harness.frame(vec![]);
        let (_, terrain_button_pos) = collect_rendered_texts_with_pos(&layout.shapes)
            .into_iter()
            .find(|(t, _)| t.contains("Create Terrain"))
            .expect("Create Terrain button must render");

        harness.click(terrain_button_pos);
        // See `clicking_create_terrain_opens_the_heightmap_path_popup`'s
        // comment: the click frame only toggles the popup open, a settle
        // frame afterwards is what actually paints its content, including
        // the Create button this test needs a position for.
        let settled = harness.frame(vec![]);
        let (_, create_pos) = collect_rendered_texts_with_pos(&settled.shapes)
            .into_iter()
            .find(|(t, _)| t == "Create")
            .expect("popup must render a Create button once open");

        // Seed the buffer directly through egui's own data store rather
        // than simulating individual keystrokes: `ui()` reads this exact
        // Id/store every frame (see `CREATE_TERRAIN_BUFFER_ID_STR`'s doc
        // comment), so this exercises the real read path -- driving actual
        // key-by-key text input would only additionally exercise egui's
        // own `TextEdit`, which isn't this crate's code to verify.
        let buffer_id = egui::Id::new(CREATE_TERRAIN_BUFFER_ID_STR);
        harness
            .egui_ctx
            .data_mut(|d| d.insert_temp(buffer_id, "heightmaps/mountain.png".to_string()));

        harness.click(create_pos);

        assert_eq!(
            harness.insp.cmd_queue.len(),
            1,
            "exactly one command should be queued"
        );
        let cmd = harness.insp.cmd_queue.remove(0);
        let InspectorCmd::SpawnTerrain {
            heightmap_path,
            chunk_count,
            chunk_size,
            height_scale,
        } = cmd
        else {
            panic!("expected InspectorCmd::SpawnTerrain to have been queued");
        };
        assert_eq!(heightmap_path, "heightmaps/mountain.png");
        assert_eq!(chunk_count, DEFAULT_TERRAIN_CHUNK_COUNT);
        assert!((chunk_size - DEFAULT_TERRAIN_CHUNK_SIZE).abs() < f32::EPSILON);
        assert!((height_scale - DEFAULT_TERRAIN_HEIGHT_SCALE).abs() < f32::EPSILON);
    }

    #[test]
    fn clicking_create_with_an_empty_heightmap_path_queues_nothing() {
        let mut harness = HierarchyHarness::new();

        let layout = harness.frame(vec![]);
        let (_, terrain_button_pos) = collect_rendered_texts_with_pos(&layout.shapes)
            .into_iter()
            .find(|(t, _)| t.contains("Create Terrain"))
            .expect("Create Terrain button must render");

        harness.click(terrain_button_pos);
        // See `clicking_create_terrain_opens_the_heightmap_path_popup`'s
        // comment: the click frame only toggles the popup open, a settle
        // frame afterwards is what actually paints its content, including
        // the Create button this test needs a position for.
        let settled = harness.frame(vec![]);
        let (_, create_pos) = collect_rendered_texts_with_pos(&settled.shapes)
            .into_iter()
            .find(|(t, _)| t == "Create")
            .expect("popup must render a Create button once open");

        // No buffer seeding here, unlike the commit test above -- the
        // buffer starts (and stays) empty, which must guard the commit.
        harness.click(create_pos);

        assert!(
            harness.insp.cmd_queue.is_empty(),
            "an empty heightmap path must not queue InspectorCmd::SpawnTerrain"
        );
    }
}

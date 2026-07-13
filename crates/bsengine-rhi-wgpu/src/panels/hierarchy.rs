use bsengine_core::{EditorPanel, EditorPanelContext, InspectorCmd};

pub struct HierarchyPanel;

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
        ui.label("Ctrl+click: toggle · Shift+click: range");
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (idx, info) in entities_snapshot.iter().enumerate() {
                let label = info.name.as_deref().unwrap_or("(unnamed)");
                let text = format!("[{}] {}", info.id, label);
                let resp = ui.selectable_label(info.selected, text);
                if resp.clicked() {
                    let mods = ui.ctx().input(|i| i.modifiers);
                    if mods.shift {
                        let anchor_idx = current_sel
                            .and_then(|id| entities_snapshot.iter().position(|e| e.id == id))
                            .unwrap_or(idx);
                        let (lo, hi) = (anchor_idx.min(idx), anchor_idx.max(idx));
                        new_selection = Some(
                            entities_snapshot[lo..=hi]
                                .iter()
                                .map(|e| e.id)
                                .collect(),
                        );
                    } else if mods.ctrl {
                        let mut ids: Vec<u64> = entities_snapshot
                            .iter()
                            .filter(|e| e.selected)
                            .map(|e| e.id)
                            .collect();
                        if let Some(pos) = ids.iter().position(|&id| id == info.id) {
                            ids.remove(pos);
                        } else {
                            ids.push(info.id);
                        }
                        new_selection = Some(ids);
                    } else {
                        new_selection = Some(vec![info.id]);
                    }
                    new_sel = Some(info.id);
                }
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
    }
}

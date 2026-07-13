use bsengine_core::{EditorPanel, EditorPanelContext, EditorPanelRegistry, InspectorEntityInfo, InspectorState};
use egui_dock::{DockState, Node, NodeIndex, Split, SurfaceIndex};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

/// Where the dock layout is persisted, relative to the current working
/// directory the editor process was launched from.
pub fn layout_path() -> PathBuf {
    PathBuf::from("editor_layout.json")
}

/// Hierarchy (20%) | Viewport (~60%) | Inspector (~20%) — the same visual
/// arrangement the fixed-panel layout used, just now rearrangeable.
pub fn default_dock_state() -> DockState<String> {
    let mut state = DockState::new(vec!["hierarchy".to_string()]);
    let surface = SurfaceIndex::main();
    let [_hierarchy, rest] = state.split(
        (surface, NodeIndex::root()),
        Split::Right,
        0.2,
        Node::leaf("viewport".to_string()),
    );
    let [_viewport, _inspector] = state.split(
        (surface, rest),
        Split::Right,
        0.75,
        Node::leaf("inspector".to_string()),
    );
    state
}

/// Idempotently registers the three built-in panels if they aren't already
/// present (e.g. from a previous frame, or pre-registered by app code).
pub fn ensure_builtin_panels(registry: &EditorPanelRegistry) {
    let mut map = registry.0.lock().unwrap();
    map.entry("hierarchy".to_string())
        .or_insert_with(|| Box::new(crate::panels::HierarchyPanel) as Box<dyn EditorPanel>);
    map.entry("inspector".to_string())
        .or_insert_with(|| Box::new(crate::panels::InspectorPanel) as Box<dyn EditorPanel>);
    map.entry("viewport".to_string())
        .or_insert_with(|| Box::new(crate::panels::ViewportPanel) as Box<dyn EditorPanel>);
}

/// Loads a previously saved layout. Returns `None` if the file doesn't
/// exist (expected on first run) or fails to parse (logged as a warning —
/// this is not the expected case, so it's worth surfacing).
pub fn load_dock_state(path: &Path) -> Option<DockState<String>> {
    let content = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&content) {
        Ok(state) => Some(state),
        Err(e) => {
            tracing::warn!("editor_layout.json: failed to parse, using default layout: {e}");
            None
        }
    }
}

pub fn save_dock_state(path: &Path, state: &DockState<String>) {
    match serde_json::to_string(state) {
        Ok(json) => {
            if let Err(e) = std::fs::write(path, json) {
                tracing::warn!("editor_layout.json: failed to write: {e}");
            }
        }
        Err(e) => tracing::warn!("editor_layout.json: failed to serialize: {e}"),
    }
}

/// Bridges `egui_dock`'s per-tab callbacks to `EditorPanel::ui()`, looking
/// the live panel instance up by id from the registry each call. A tab id
/// with no matching registered panel (e.g. a stale saved id from a custom
/// panel type that's no longer registered) renders a placeholder instead
/// of panicking.
pub struct BseTabViewer<'a> {
    pub insp: &'a mut InspectorState,
    pub entities_snapshot: &'a [InspectorEntityInfo],
    pub cursor_pos: (f32, f32),
    pub panels: &'a mut HashMap<String, Box<dyn EditorPanel>>,
}

impl egui_dock::TabViewer for BseTabViewer<'_> {
    type Tab = String;

    fn title(&mut self, tab: &mut String) -> egui::WidgetText {
        match self.panels.get(tab.as_str()) {
            Some(panel) => panel.title().into(),
            None => tab.clone().into(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut String) {
        match self.panels.get_mut(tab.as_str()) {
            Some(panel) => {
                let mut ctx = EditorPanelContext {
                    insp: &mut *self.insp,
                    entities_snapshot: self.entities_snapshot,
                    cursor_pos: self.cursor_pos,
                };
                panel.ui(ui, &mut ctx);
            }
            None => {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!("⚠ Panel unavailable: {tab}"),
                );
            }
        }
    }
}

/// Renders the toolbar's "Window ▾" menu: a checkbox per registered panel
/// (checked = currently docked somewhere) to reopen closed tabs, plus a
/// "Reset Layout" action.
pub fn window_menu_ui(
    ui: &mut egui::Ui,
    dock_state: &mut DockState<String>,
    registry: &EditorPanelRegistry,
) {
    ui.menu_button("Window ▾", |ui| {
        let panels: MutexGuard<HashMap<String, Box<dyn EditorPanel>>> = registry.0.lock().unwrap();
        let mut ids: Vec<&String> = panels.keys().collect();
        ids.sort();
        for id in ids {
            let title = panels
                .get(id)
                .map(|p| p.title())
                .unwrap_or_else(|| id.clone());
            let is_open = dock_state.iter_all_tabs().any(|(_, tab)| tab == id);
            let mut checked = is_open;
            if ui.checkbox(&mut checked, &title).changed() {
                if checked && !is_open {
                    dock_state.push_to_first_leaf(id.clone());
                } else if !checked && is_open {
                    if let Some(loc) = dock_state.find_tab(id) {
                        dock_state.remove_tab(loc);
                    }
                }
            }
        }
        drop(panels);
        ui.separator();
        if ui.button("Reset Layout").clicked() {
            *dock_state = default_dock_state();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dock_state_has_three_builtin_tabs() {
        let state = default_dock_state();
        let mut ids: Vec<&String> = state.iter_all_tabs().map(|(_, tab)| tab).collect();
        ids.sort();
        assert_eq!(
            ids,
            vec![&"hierarchy".to_string(), &"inspector".to_string(), &"viewport".to_string()]
        );
    }

    #[test]
    fn save_and_load_dock_state_round_trips() {
        let path = std::env::temp_dir().join("bsengine_test_dock_layout.json");
        let mut state = default_dock_state();

        // `Node::leaf()`/`.split()` seed node rects with `Rect::NOTHING`
        // (infinite coordinates) until a `DockArea` actually lays them out
        // during a real UI frame. `serde_json` serializes infinite floats
        // as JSON `null`, which can't deserialize back into `f32`. In
        // production this never bites because layouts are only ever saved
        // after at least one render pass has populated real rects; simulate
        // that here so the round trip reflects realistic persisted state.
        let finite = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        for node in state.main_surface_mut().iter_mut() {
            match node {
                Node::Leaf { rect, viewport, .. } => {
                    *rect = finite;
                    *viewport = finite;
                }
                Node::Vertical { rect, .. } | Node::Horizontal { rect, .. } => {
                    *rect = finite;
                }
                Node::Empty => {}
            }
        }

        save_dock_state(&path, &state);
        let loaded = load_dock_state(&path).expect("failed to load saved layout");

        let mut original_ids: Vec<&String> = state.iter_all_tabs().map(|(_, t)| t).collect();
        let mut loaded_ids: Vec<&String> = loaded.iter_all_tabs().map(|(_, t)| t).collect();
        original_ids.sort();
        loaded_ids.sort();
        assert_eq!(original_ids, loaded_ids);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn load_dock_state_returns_none_for_missing_file() {
        let path = std::env::temp_dir().join("bsengine_test_dock_layout_does_not_exist.json");
        std::fs::remove_file(&path).ok();
        assert!(load_dock_state(&path).is_none());
    }

    #[test]
    fn ensure_builtin_panels_is_idempotent() {
        let registry = EditorPanelRegistry::default();
        ensure_builtin_panels(&registry);
        ensure_builtin_panels(&registry);
        assert_eq!(registry.0.lock().unwrap().len(), 3);
    }
}

use crate::inspector::{InspectorEntityInfo, InspectorState};
use bevy_ecs::prelude::Resource;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A single dockable editor panel. Built-in panels (Hierarchy, Inspector,
/// Viewport, and future ones like Project/Console) and custom panels
/// registered by downstream game projects both implement this uniformly —
/// there is no special-casing of "built-in" vs "custom" anywhere in the
/// dock host.
pub trait EditorPanel: Send {
    /// Stable identifier used as the dock tab id and for layout
    /// persistence. Must not change across versions once shipped, or saved
    /// layouts referencing it silently drop the tab.
    fn id(&self) -> &str;
    /// Human-readable label shown on the panel's dock tab.
    fn title(&self) -> String;
    /// Draws the panel's contents for this frame.
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorPanelContext);
}

/// Shared state every panel's `ui()` call gets access to.
pub struct EditorPanelContext<'a> {
    /// Mutable inspector state (selection, gizmo mode, pending commands, ...).
    pub insp: &'a mut InspectorState,
    /// Read-only snapshot of all entities visible to the editor this frame.
    pub entities_snapshot: &'a [InspectorEntityInfo],
    /// Current cursor position within the viewport, in logical pixels.
    pub cursor_pos: (f32, f32),
    /// Reflection type registry, when available, for generic component editing.
    pub type_registry: Option<&'a bevy_reflect::TypeRegistry>,
}

/// Registry of all panels (built-in and custom) available to dock.
/// Populated by `EditorPlugin` (built-ins) and by downstream app code
/// (custom panels), the same way `McpRegistryResource` is populated by
/// whichever plugin/app code calls `.register()` on it.
#[derive(Resource, Default)]
pub struct EditorPanelRegistry(pub Arc<Mutex<HashMap<String, Box<dyn EditorPanel>>>>);

#[cfg(test)]
mod tests {
    use super::*;

    struct CounterPanel {
        clicks: u32,
    }

    impl EditorPanel for CounterPanel {
        fn id(&self) -> &str {
            "counter"
        }
        fn title(&self) -> String {
            "Counter".to_string()
        }
        fn ui(&mut self, _ui: &mut egui::Ui, _ctx: &mut EditorPanelContext) {
            self.clicks += 1;
        }
    }

    #[test]
    fn registry_stores_and_looks_up_panel_by_id() {
        let registry = EditorPanelRegistry::default();
        registry
            .0
            .lock()
            .unwrap()
            .insert("counter".to_string(), Box::new(CounterPanel { clicks: 0 }));

        let map = registry.0.lock().unwrap();
        let panel = map.get("counter").expect("panel not registered");
        assert_eq!(panel.title(), "Counter");
        assert_eq!(panel.id(), "counter");
    }

    #[test]
    fn registry_default_is_empty() {
        let registry = EditorPanelRegistry::default();
        assert!(registry.0.lock().unwrap().is_empty());
    }
}

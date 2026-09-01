/// The Assets panel: scans and browses the project's asset directory.
pub mod asset_browser;
/// Dock layout state, persistence, and the `egui_dock` tab viewer.
pub mod dock;
/// The Hierarchy panel: entity tree view with selection, reparenting, and rename.
pub mod hierarchy;
/// The Inspector panel: displays and edits the selected entity's components.
pub mod inspector;
/// The Profiler panel: frame timing, per-pass GPU cost, and draw-call/
/// triangle/texture-memory stats.
pub mod profiler;
/// Generic egui widgets for editing `bevy_reflect`-reflected component fields.
pub mod reflect_ui;
/// The Shader Graph panel: a node editor for `bsengine-shadergraph` graphs.
pub mod shadergraph;
/// The Viewport panel: renders the 3D scene plus gizmo overlays.
pub mod viewport;

pub use asset_browser::{AssetBrowserPanel, AssetDragPayload, AssetKind};
pub use dock::{
    default_dock_state, ensure_builtin_panels, layout_path, load_dock_state, save_dock_state,
    window_menu_ui, BseTabViewer,
};
pub use hierarchy::HierarchyPanel;
pub use inspector::InspectorPanel;
pub use profiler::ProfilerPanel;
pub use reflect_ui::draw_reflect_ui;
pub use shadergraph::ShaderGraphPanel;
pub use viewport::ViewportPanel;

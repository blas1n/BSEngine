use bevy_ecs::prelude::Resource;

/// Canonical set of recognized primitive-mesh kind strings, lowercase,
/// shared between the Inspector's Mesh dropdown (`bsengine-rhi-wgpu`) and
/// `InspectorEntityInfo.primitive`/`InspectorCmd::AttachPrimitiveMesh.primitive`
/// below. `bsengine-editor`'s `primitive_to_str`/`str_to_primitive` (the only
/// place these strings convert to/from the real `bsengine_scene::Primitive`
/// enum) must stay in sync with this list — see the doc comments there.
pub const PRIMITIVE_KINDS: [&str; 4] = ["cube", "sphere", "plane", "capsule"];

#[derive(Clone, Default)]
pub struct InspectorEntityInfo {
    pub id: u64,
    pub name: Option<String>,
    pub position: Option<[f32; 3]>,
    pub rotation: Option<[f32; 3]>,
    pub scale: Option<[f32; 3]>,
    // light
    pub light_type: Option<String>,
    pub light_color: Option<[f32; 3]>,
    pub light_intensity: Option<f32>,
    pub light_range: Option<f32>,
    pub spot_inner_angle: Option<f32>,
    pub spot_outer_angle: Option<f32>,
    // camera
    pub camera_fov: Option<f32>,
    // material
    pub material_base_color: Option<[f32; 3]>,
    pub material_metallic: Option<f32>,
    pub material_roughness: Option<f32>,
    pub material_emissive: Option<[f32; 3]>,
    // hierarchy / tags / script / mesh
    pub parent_id: Option<u64>,
    pub tags: Vec<String>,
    pub script_path: Option<String>,
    /// One of [`PRIMITIVE_KINDS`], or `None` if no `PrimitiveMesh` is
    /// attached. A plain `String`, not `bsengine_scene::Primitive`, because
    /// `bsengine-core` cannot depend on `bsengine-scene` (that crate already
    /// depends on `bsengine-core`). Mirrors this same struct's
    /// `light_type: Option<String>` convention.
    pub primitive: Option<String>,
    pub visible: bool,
    pub selected: bool,
}

pub enum InspectorCmd {
    SetPosition {
        id: u64,
        x: f32,
        y: f32,
        z: f32,
    },
    SetRotation {
        id: u64,
        rx: f32,
        ry: f32,
        rz: f32,
    },
    SetScale {
        id: u64,
        sx: f32,
        sy: f32,
        sz: f32,
    },
    SpawnEntity {
        name: String,
    },
    Despawn {
        id: u64,
    },
    SetVisible {
        id: u64,
        visible: bool,
    },
    AddPointLight {
        id: u64,
    },
    AddCamera {
        id: u64,
    },
    SetSelection {
        ids: Vec<u64>,
    },
    Duplicate {
        id: u64,
    },
    SaveScene,
    AttachComponentByType {
        id: u64,
        type_path: String,
    },
    RemoveComponentByType {
        id: u64,
        type_path: String,
    },
    RenameEntity {
        id: u64,
        name: String,
    },
    SetParent {
        id: u64,
        parent_id: u64,
    },
    RemoveParent {
        id: u64,
    },
    TagEntity {
        id: u64,
        tag: String,
    },
    UntagEntity {
        id: u64,
        tag: String,
    },
    AttachScript {
        id: u64,
        path: String,
    },
    DetachScript {
        id: u64,
    },
    AttachPrimitiveMesh {
        id: u64,
        /// One of [`PRIMITIVE_KINDS`] — a plain `String`, not
        /// `bsengine_scene::Primitive`, for the same circular-dependency
        /// reason as `InspectorEntityInfo.primitive`. Parsed back into the
        /// real enum in `apply_inspector_cmds` (`bsengine-editor`), where
        /// `bsengine_scene` is already in scope.
        primitive: String,
    },
    DetachPrimitiveMesh {
        id: u64,
    },
    /// Apply an edited clone of a reflected component's value back onto the
    /// real ECS component. `value` was originally cloned out by
    /// `populate_reflected_component_snapshot`, edited in place in the
    /// Inspector via `draw_reflect_ui`, then re-cloned here for the trip
    /// back through the command queue. Routed through the same
    /// `ReflectCommandQueueResource` `AttachComponentByType`/
    /// `RemoveComponentByType` already use, not the plain `EditorCommand`
    /// queue — see `apply_inspector_cmds`.
    ApplyReflectedComponent {
        id: u64,
        type_path: String,
        value: Box<dyn bevy_reflect::Reflect>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum EditorPlayState {
    #[default]
    Stopped,
    Playing,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum GizmoMode {
    #[default]
    Translate,
    Rotate,
}

#[derive(Resource)]
pub struct InspectorState {
    pub entities: Vec<InspectorEntityInfo>,
    pub selected_id: Option<u64>,
    pub cmd_queue: Vec<InspectorCmd>,
    /// Cloned reflected components currently attached to `selected_id`,
    /// repopulated every frame by `populate_reflected_component_snapshot`
    /// (bsengine-editor). The Inspector's "Reflected Fields" section edits
    /// these clones in place via `draw_reflect_ui`; on change, an edited
    /// clone is pushed back as `InspectorCmd::ApplyReflectedComponent`. Each
    /// entry is `(type_path, cloned value)` — `type_path` matches the
    /// format already used by `AttachComponentByType`/`RemoveComponentByType`
    /// (e.g. `"bsengine_core::camera::Camera"`).
    pub reflected_components: Vec<(String, Box<dyn bevy_reflect::Reflect>)>,
    pub edit_pos: [f32; 3],
    pub edit_rot: [f32; 3],
    pub edit_scale: [f32; 3],
    pub edit_new_tag: String,
    pub edit_script_path: String,
    pub edit_visible: bool,
    prev_selected_id: Option<u64>,

    // Editor mode toggle and play state
    pub editor_mode: bool,
    pub play_state: EditorPlayState,

    // Editor orbit camera parameters
    pub cam_target: [f32; 3],
    pub cam_distance: f32,
    pub cam_yaw: f32,
    pub cam_pitch: f32,

    // Set by the egui viewport panel each frame; read by the camera system
    pub viewport_contains_cursor: bool,
    pub viewport_size: [f32; 2],
    /// Top-left corner of the viewport panel in window screen space, set by
    /// the egui viewport panel each frame. Read by the HUD text overlay so
    /// `Bsengine.setHudText` positions relative to the actual rendered game
    /// view instead of the whole editor window (which would put it under
    /// the toolbar/other dock panels in editor mode).
    pub viewport_pos: [f32; 2],

    // Override view_proj computed by EditorPlugin from orbit state; read by RenderPlugin
    pub editor_view_proj: Option<[[f32; 4]; 4]>,
    pub editor_proj: [[f32; 4]; 4],
    pub editor_cam_pos: [f32; 3],

    // Which viewport gizmo is active for the selected entity.
    pub gizmo_mode: GizmoMode,

    // Translate-gizmo drag state (viewport panel). `gizmo_drag_axis` is
    // 0=X, 1=Y, 2=Z while a handle is being dragged.
    pub gizmo_drag_axis: Option<u8>,
    pub gizmo_drag_start_world: [f32; 3],
    pub gizmo_drag_start_mouse: [f32; 2],

    // Rotate-gizmo drag state. `gizmo_rotate_axis` is 0=X, 1=Y, 2=Z (world
    // axis) while a ring is being dragged; the angle fields are radians of
    // the mouse's angle around the gizmo's screen-space center.
    pub gizmo_rotate_axis: Option<u8>,
    pub gizmo_rotate_start_deg: [f32; 3],
    pub gizmo_rotate_start_angle: f32,

    // Set by the toolbar/keyboard to request an undo/redo; consumed and
    // cleared by EditorPlugin's history system the same frame.
    pub request_undo: bool,
    pub request_redo: bool,

    // Path the scene was loaded from / last saved to, used by Ctrl+S / the
    // Save toolbar button to save in place without prompting for a path.
    pub current_scene_path: Option<String>,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            entities: Vec::new(),
            selected_id: None,
            cmd_queue: Vec::new(),
            reflected_components: Vec::new(),
            edit_pos: [0.0; 3],
            edit_rot: [0.0; 3],
            edit_scale: [1.0, 1.0, 1.0],
            edit_new_tag: String::new(),
            edit_script_path: String::new(),
            edit_visible: true,
            prev_selected_id: None,
            editor_mode: false,
            play_state: EditorPlayState::Stopped,
            cam_target: [0.0; 3],
            cam_distance: 10.0,
            cam_yaw: 0.5,
            cam_pitch: 0.4,
            viewport_contains_cursor: false,
            viewport_size: [1280.0, 720.0],
            viewport_pos: [0.0, 0.0],
            editor_view_proj: None,
            editor_proj: [[0.0; 4]; 4],
            editor_cam_pos: [0.0; 3],
            gizmo_mode: GizmoMode::Translate,
            gizmo_drag_axis: None,
            gizmo_drag_start_world: [0.0; 3],
            gizmo_drag_start_mouse: [0.0; 2],
            gizmo_rotate_axis: None,
            gizmo_rotate_start_deg: [0.0; 3],
            gizmo_rotate_start_angle: 0.0,
            request_undo: false,
            request_redo: false,
            current_scene_path: None,
        }
    }
}

impl InspectorState {
    pub fn editor() -> Self {
        Self {
            editor_mode: true,
            ..Default::default()
        }
    }

    pub fn sync_selection(&mut self) {
        if self.selected_id != self.prev_selected_id {
            self.prev_selected_id = self.selected_id;
            if let Some(id) = self.selected_id {
                if let Some(info) = self.entities.iter().find(|e| e.id == id) {
                    self.edit_pos = info.position.unwrap_or([0.0; 3]);
                    self.edit_rot = info.rotation.unwrap_or([0.0; 3]);
                    self.edit_scale = info.scale.unwrap_or([1.0, 1.0, 1.0]);
                    self.edit_new_tag.clear();
                    self.edit_script_path = info.script_path.clone().unwrap_or_default();
                    self.edit_visible = info.visible;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_no_selection() {
        let s = InspectorState::default();
        assert!(s.selected_id.is_none());
        assert!(s.entities.is_empty());
        assert!(s.cmd_queue.is_empty());
        assert!(!s.editor_mode);
        assert_eq!(s.play_state, EditorPlayState::Stopped);
    }

    #[test]
    fn sync_selection_loads_entity_transform() {
        let mut s = InspectorState::default();
        s.entities.push(InspectorEntityInfo {
            id: 1,
            name: Some("Player".into()),
            position: Some([1.0, 2.0, 3.0]),
            rotation: Some([10.0, 20.0, 30.0]),
            scale: Some([2.0, 2.0, 2.0]),
            ..Default::default()
        });
        s.selected_id = Some(1);
        s.sync_selection();
        assert_eq!(s.edit_pos, [1.0, 2.0, 3.0]);
        assert_eq!(s.edit_rot, [10.0, 20.0, 30.0]);
        assert_eq!(s.edit_scale, [2.0, 2.0, 2.0]);
    }

    #[test]
    fn sync_selection_no_reset_when_same_entity() {
        let mut s = InspectorState::default();
        s.entities.push(InspectorEntityInfo {
            id: 1,
            name: None,
            position: Some([5.0, 0.0, 0.0]),
            ..Default::default()
        });
        s.selected_id = Some(1);
        s.sync_selection();
        assert_eq!(s.edit_pos[0], 5.0);
        s.edit_pos = [99.0, 0.0, 0.0];
        s.sync_selection();
        assert_eq!(s.edit_pos[0], 99.0);
    }

    #[test]
    fn sync_selection_uses_defaults_when_no_transform() {
        let mut s = InspectorState::default();
        s.entities.push(InspectorEntityInfo {
            id: 2,
            ..Default::default()
        });
        s.selected_id = Some(2);
        s.sync_selection();
        assert_eq!(s.edit_pos, [0.0; 3]);
        assert_eq!(s.edit_scale, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn editor_cam_default_distance() {
        let s = InspectorState::default();
        assert!((s.cam_distance - 10.0).abs() < 1e-6);
    }
}

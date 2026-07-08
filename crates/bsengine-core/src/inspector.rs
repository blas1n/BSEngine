use bevy_ecs::prelude::Resource;

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
    // camera
    pub camera_fov: Option<f32>,
    pub visible: bool,
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
    UpdateLight {
        id: u64,
        color: Option<[f32; 3]>,
        intensity: Option<f32>,
        range: Option<f32>,
    },
    UpdateCamera {
        id: u64,
        fov_y_degrees: Option<f32>,
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
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum EditorPlayState {
    #[default]
    Stopped,
    Playing,
}

#[derive(Resource)]
pub struct InspectorState {
    pub entities: Vec<InspectorEntityInfo>,
    pub selected_id: Option<u64>,
    pub cmd_queue: Vec<InspectorCmd>,
    pub edit_pos: [f32; 3],
    pub edit_rot: [f32; 3],
    pub edit_scale: [f32; 3],
    pub edit_light_color: [f32; 3],
    pub edit_light_intensity: f32,
    pub edit_light_range: f32,
    pub edit_camera_fov: f32,
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

    // Override view_proj computed by EditorPlugin from orbit state; read by RenderPlugin
    pub editor_view_proj: Option<[[f32; 4]; 4]>,
    pub editor_proj: [[f32; 4]; 4],
    pub editor_cam_pos: [f32; 3],
}

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            entities: Vec::new(),
            selected_id: None,
            cmd_queue: Vec::new(),
            edit_pos: [0.0; 3],
            edit_rot: [0.0; 3],
            edit_scale: [1.0, 1.0, 1.0],
            edit_light_color: [1.0, 1.0, 1.0],
            edit_light_intensity: 1.0,
            edit_light_range: 10.0,
            edit_camera_fov: 60.0,
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
            editor_view_proj: None,
            editor_proj: [[0.0; 4]; 4],
            editor_cam_pos: [0.0; 3],
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
                    self.edit_light_color = info.light_color.unwrap_or([1.0, 1.0, 1.0]);
                    self.edit_light_intensity = info.light_intensity.unwrap_or(1.0);
                    self.edit_light_range = info.light_range.unwrap_or(10.0);
                    self.edit_camera_fov = info.camera_fov.unwrap_or(60.0);
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

    #[test]
    fn sync_selection_loads_light_params() {
        let mut s = InspectorState::default();
        s.entities.push(InspectorEntityInfo {
            id: 3,
            light_type: Some("point".into()),
            light_color: Some([0.5, 0.8, 1.0]),
            light_intensity: Some(2.5),
            light_range: Some(20.0),
            ..Default::default()
        });
        s.selected_id = Some(3);
        s.sync_selection();
        assert_eq!(s.edit_light_color, [0.5, 0.8, 1.0]);
        assert!((s.edit_light_intensity - 2.5).abs() < 1e-6);
        assert!((s.edit_light_range - 20.0).abs() < 1e-6);
    }

    #[test]
    fn sync_selection_loads_camera_fov() {
        let mut s = InspectorState::default();
        s.entities.push(InspectorEntityInfo {
            id: 4,
            camera_fov: Some(90.0),
            ..Default::default()
        });
        s.selected_id = Some(4);
        s.sync_selection();
        assert!((s.edit_camera_fov - 90.0).abs() < 1e-6);
    }
}

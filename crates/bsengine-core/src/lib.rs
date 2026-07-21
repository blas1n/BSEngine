pub mod ambient_occlusion;
pub mod angular_velocity;
pub mod animation_player;
pub mod animation_state_machine;
pub mod bloom;
pub mod camera;
pub mod cursor_config;
pub mod cursor_pos;
pub mod custom_shader;
pub mod damping;
pub mod editor_panel;
pub mod external_impulse;
pub mod follow;
pub mod global_transform;
pub mod gravity;
pub mod hud_texts;
pub mod inspector;
pub mod lifetime;
pub mod light;
pub mod logging;
pub mod mass;
pub mod material;
pub mod name;
pub mod nav_mesh;
pub mod nav_mesh_agent;
pub mod network_id;
pub mod parent;
pub mod reflect_color;
pub mod reflect_degrees;
pub mod reflect_glam;
pub mod reflect_mat4;
pub mod reflect_validate;
pub mod save_data;
pub mod screen_size;
pub mod shield;
pub mod skybox;
pub mod time;
pub mod timer;
pub mod tone_map;
pub mod transform;
pub mod tween;
pub mod ui_state;
pub mod velocity;
pub mod visible;

pub use ambient_occlusion::AmbientOcclusion;
pub use angular_velocity::AngularVelocity;
pub use animation_player::AnimationPlayer;
pub use animation_state_machine::{
    AnimationStateMachine, AsmState, AsmTransition, TransitionCondition,
};
pub use bloom::Bloom;
pub use camera::Camera;
pub use cursor_config::CursorConfig;
pub use cursor_pos::CursorPos;
pub use custom_shader::CustomShader;
pub use damping::Damping;
pub use editor_panel::{EditorPanel, EditorPanelContext, EditorPanelRegistry};
pub use external_impulse::ExternalImpulse;
pub use follow::{Follow, LookAt};
pub use global_transform::GlobalTransform;
pub use gravity::{Gravity, GravityScale};
pub use hud_texts::HudTexts;
pub use inspector::{
    EditorPlayState, GizmoMode, InspectorCmd, InspectorEntityInfo, InspectorState, PRIMITIVE_KINDS,
};
pub use lifetime::Lifetime;
pub use light::{DirectionalLight, PointLight, SpotLight};
pub use logging::init_logging;
pub use mass::Mass;
pub use material::Material;
pub use name::Name;
pub use nav_mesh::NavMesh;
pub use nav_mesh_agent::{NavAgentState, NavMeshAgent};
pub use network_id::{NetworkAuthority, NetworkId};
pub use parent::Parent;
pub use reflect_color::ReflectColor;
pub use reflect_degrees::ReflectDegrees;
pub use reflect_glam::{ReflectQuat, ReflectVec2, ReflectVec3, ReflectVec4};
pub use reflect_mat4::ReflectMat4;
pub use reflect_validate::{ReflectValidate, Validate};
pub use save_data::SaveData;
pub use screen_size::ScreenSize;
pub use shield::Shield;
pub use skybox::{Skybox, SkyboxPath, SkyboxProjection};
pub use time::Time;
pub use timer::Timer;
pub use tone_map::{ToneMap, ToneMappingMode};
pub use transform::Transform;
pub use tween::{EasingFn, RepeatMode, Tween, TweenTarget};
pub use ui_state::{UiState, UiWidget};
pub use velocity::Velocity;
pub use visible::Visible;

pub fn propagate_global_transforms(world: &mut bevy_ecs::world::World) {
    use bevy_ecs::prelude::Entity;
    use glam::Mat4;
    use std::collections::HashMap;

    let mut query = world.query::<(Entity, &Transform, Option<&Parent>)>();
    let entries: Vec<(Entity, Mat4, Option<Entity>)> = query
        .iter(world)
        .map(|(e, t, p)| (e, t.to_matrix(), p.map(|p| p.0)))
        .collect();

    let mut globals: HashMap<Entity, Mat4> =
        entries.iter().map(|(e, local, _)| (*e, *local)).collect();

    for _ in 0..8 {
        for (e, local, parent) in &entries {
            if let Some(parent_e) = parent {
                if let Some(&parent_global) = globals.get(parent_e) {
                    globals.insert(*e, parent_global * *local);
                }
            }
        }
    }

    let mut gt_query = world.query::<(Entity, &mut GlobalTransform)>();
    for (e, mut gt) in gt_query.iter_mut(world) {
        if let Some(&mat) = globals.get(&e) {
            gt.0 = mat.into();
        }
    }
}

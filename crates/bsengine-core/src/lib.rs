//! Core ECS components and resources shared across BSEngine.
//!
//! Defines the real, actively-used component types (`Transform`, `Camera`,
//! light variants, `Material`, physics-adjacent components like
//! `Velocity`/`Damping`/`Shield`, ...) plus their `bevy_reflect`
//! registrations backing the editor's generic Inspector UI, the
//! `ReflectVec3`/`ReflectColor`/... opaque wrapper types reflection needs,
//! and editor-facing infrastructure (`InspectorState`, `EditorPanelRegistry`).
#![warn(missing_docs)]

/// Screen-space ambient occlusion post-process settings.
pub mod ambient_occlusion;
/// Angular (rotational) velocity component for physics-driven entities.
pub mod angular_velocity;
/// Simple single-clip skeletal/sprite animation playback component.
pub mod animation_player;
/// Graph-based animation state machine with parameterized transitions.
pub mod animation_state_machine;
/// HDR bloom post-process settings.
pub mod bloom;
/// Perspective camera component and projection math.
pub mod camera;
/// Cursor visibility/lock configuration resource.
pub mod cursor_config;
/// Current cursor position resource, in window space.
pub mod cursor_pos;
/// Custom shader override component for materials.
pub mod custom_shader;
/// Linear/angular velocity damping (drag) component.
pub mod damping;
/// Editor-side dockable panel trait and registry.
pub mod editor_panel;
/// One-shot linear/angular impulse component consumed by the physics step.
pub mod external_impulse;
/// Components that make an entity's transform track another entity.
pub mod follow;
/// World-space transform resulting from local transform + parent hierarchy.
pub mod global_transform;
/// Gravitational acceleration resource and per-entity gravity scale.
pub mod gravity;
/// Named on-screen HUD text overlay resource.
pub mod hud_texts;
/// Editor inspector state, entity snapshots, and inspector command protocol.
pub mod inspector;
/// Countdown-to-despawn component for temporary entities.
pub mod lifetime;
/// Directional, point, and spot light components.
pub mod light;
/// Engine-wide logging initialization.
pub mod logging;
/// Physical mass component used by the physics integrator.
pub mod mass;
/// PBR material properties component.
pub mod material;
/// Human-readable entity name component.
pub mod name;
/// Baked navigation mesh resource used for pathfinding.
pub mod nav_mesh;
/// Nav-mesh-driven pathing agent component and its runtime state.
pub mod nav_mesh_agent;
/// Networked entity identity and authority components.
pub mod network_id;
/// Parent-entity reference for building transform hierarchies.
pub mod parent;
/// `bevy_reflect`-friendly wrapper for RGBA color values.
pub mod reflect_color;
/// `bevy_reflect`-friendly wrapper for angles expressed in degrees.
pub mod reflect_degrees;
/// `bevy_reflect`-friendly wrappers for `glam` vector and quaternion types.
pub mod reflect_glam;
/// `bevy_reflect`-friendly wrapper for a `glam::Mat4`.
pub mod reflect_mat4;
/// Validation trait for reflected component field values.
pub mod reflect_validate;
/// Serializable save-game data resource.
pub mod save_data;
/// Current window/render-target size resource.
pub mod screen_size;
/// Depletable shield/absorb-damage component.
pub mod shield;
/// Skybox background component and its projection mode.
pub mod skybox;
/// Frame and elapsed-time resource driven by the app's main loop.
pub mod time;
/// Simple countdown timer component.
pub mod timer;
/// HDR-to-LDR tone mapping post-process settings.
pub mod tone_map;
/// Local position/rotation/scale transform component.
pub mod transform;
/// Time-driven property tween/animation component.
pub mod tween;
/// Immediate-mode UI widget tree and state resource.
pub mod ui_state;
/// Linear velocity component consumed by the physics integrator.
pub mod velocity;
/// Visibility toggle component controlling whether an entity is rendered.
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

/// Recomputes every entity's [`GlobalTransform`] from its local [`Transform`]
/// and [`Parent`] chain, iterating a fixed number of passes to settle deep hierarchies.
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

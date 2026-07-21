use std::collections::{HashMap, HashSet};

use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_ecs::prelude::*;
use bsengine_audio::AudioWorld;
use bsengine_core::{
    CursorConfig, CustomShader, EditorPlayState, GlobalTransform, HudTexts, InspectorState,
    Material, Parent, ScreenSize, SkyboxPath, Transform, UiState, UiWidget, Visible,
};
use bsengine_input::{GamepadButton, GamepadSticks, Input, KeyCode, MouseButton, MouseState};
use bsengine_network::NetworkSession;
use bsengine_physics::CollisionEvent;
use bsengine_physics::PhysicsWorld;
use bsengine_scene::{Name, PendingSceneLoad, Primitive, PrimitiveMesh, ScriptPath};
use glam::{EulerRot, Quat, Vec3};

use crate::ops::{
    ScriptCommand, SpawnParams, AMBIENT_OCCLUSION_SNAPSHOT, ANGULAR_DAMPING_SNAPSHOT,
    ANGULAR_VELOCITY_SNAPSHOT, ANIMATION_SNAPSHOT, ASM_STATE_SNAPSHOT, BLOOM_SNAPSHOT,
    BODY_TYPE_SNAPSHOT, BOOTSTRAP_JS, CHILDREN_SNAPSHOT, COLLIDER_SENSOR_SNAPSHOT,
    COLLISION_SNAPSHOT, COMMAND_BUFFER, ENTITY_NAMES_SNAPSHOT, ENTITY_NAME_MAP, FOLLOW_SNAPSHOT,
    FRICTION_SNAPSHOT, GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT, GAMEPAD_BUTTON_JUST_RELEASED_SNAPSHOT,
    GAMEPAD_BUTTON_SNAPSHOT, GAMEPAD_STICKS_SNAPSHOT, GRAVITY_SCALE_SNAPSHOT, GRAVITY_SNAPSHOT,
    KEY_JUST_PRESSED_SNAPSHOT, KEY_JUST_RELEASED_SNAPSHOT, KEY_SNAPSHOT, LIFETIME_SNAPSHOT,
    LINEAR_DAMPING_SNAPSHOT, LOOK_AT_SNAPSHOT, MASS_SNAPSHOT, MATERIAL_COLOR_SNAPSHOT,
    MATERIAL_EMISSIVE_SNAPSHOT, MATERIAL_METALLIC_SNAPSHOT, MATERIAL_ROUGHNESS_SNAPSHOT,
    MOUSE_DELTA_SNAPSHOT, MOUSE_JUST_PRESSED_SNAPSHOT, MOUSE_JUST_RELEASED_SNAPSHOT,
    MOUSE_POS_SNAPSHOT, MOUSE_PRESSED_SNAPSHOT, NAV_SNAPSHOT, NETWORK_ID_SNAPSHOT,
    NETWORK_STATE_SNAPSHOT, PARENT_SNAPSHOT, PHYSICS_WORLD_PTR, RESTITUTION_SNAPSHOT,
    SCREEN_SIZE_SNAPSHOT, SHIELD_SNAPSHOT, SLEEP_SNAPSHOT, SOUND_POSITION_SNAPSHOT,
    SOUND_STATE_SNAPSHOT, TIMER_SNAPSHOT, TIME_DELTA_SNAPSHOT, TIME_ELAPSED_SNAPSHOT,
    TONE_MAP_SNAPSHOT, TRANSFORM_SNAPSHOT, TWEEN_SNAPSHOT, UI_CLICKED_SNAPSHOT, VELOCITY_SNAPSHOT,
    VISIBLE_SNAPSHOT, WORLD_TRANSFORM_SNAPSHOT,
};
use crate::runtime::ScriptRuntime;

/// Root directory of the current project — used to resolve relative script paths.
#[derive(Resource, Default)]
pub struct ProjectDir(pub String);

/// Loaded JS source for a scripted entity.
#[derive(Component)]
pub struct Script {
    pub source: String,
}

// Not Send/Sync — stored as a non-send resource via insert_non_send_resource.
pub struct ScriptRuntimeResource(pub ScriptRuntime);

/// Stores kira sound handles by script-assigned id for stopSound support.
#[derive(Resource, Default)]
pub struct SoundHandles(pub HashMap<u32, kira::sound::static_sound::StaticSoundHandle>);

#[derive(Resource)]
pub(crate) struct ScriptTimingState {
    startup: std::time::Instant,
    last_frame: std::time::Instant,
}

pub struct ScriptingPlugin {
    pub project_dir: String,
}

impl Default for ScriptingPlugin {
    fn default() -> Self {
        Self {
            project_dir: ".".to_string(),
        }
    }
}

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ProjectDir(self.project_dir.clone()));
        app.insert_resource(HudTexts::default());
        app.insert_resource(SoundHandles::default());
        app.insert_non_send_resource(ScriptRuntimeResource(ScriptRuntime::new_with_ops()));
        let now = std::time::Instant::now();
        app.insert_resource(ScriptTimingState {
            startup: now,
            last_frame: now,
        });
        // Register CollisionEvent so EventReader works even without PhysicsPlugin
        app.add_event::<CollisionEvent>();
        app.add_systems(PostStartup, load_scripts);
        app.add_systems(Update, (capture_collision_events, run_scripts).chain());
    }
}

/// Capture collision events each frame into a thread_local snapshot for scripts.
fn capture_collision_events(
    mut events: EventReader<CollisionEvent>,
    name_query: Query<(Entity, &Name)>,
) {
    let name_map: HashMap<Entity, String> =
        name_query.iter().map(|(e, n)| (e, n.0.clone())).collect();

    let collisions: Vec<(String, String, bool)> = events
        .read()
        .filter_map(|ev| {
            let a = name_map.get(&ev.entity_a)?.clone();
            let b = name_map.get(&ev.entity_b)?.clone();
            Some((a, b, ev.started))
        })
        .collect();

    COLLISION_SNAPSHOT.with(|s| *s.borrow_mut() = collisions);
}

pub fn load_scripts(world: &mut World) {
    let project_dir = world
        .get_resource::<ProjectDir>()
        .map(|pd| pd.0.clone())
        .unwrap_or_default();

    let scripts: Vec<(Entity, String)> = {
        let mut q = world.query::<(Entity, &ScriptPath)>();
        q.iter(world)
            .map(|(e, sp)| {
                let path = if project_dir.is_empty() {
                    sp.0.clone()
                } else {
                    format!("{}/{}", project_dir, sp.0)
                };
                (e, path)
            })
            .collect()
    };

    tracing::info!(
        "[scripting] {} scripted entity/entities found",
        scripts.len()
    );

    if scripts.is_empty() {
        return;
    }

    if let Some(mut rt) = world.get_non_send_resource_mut::<ScriptRuntimeResource>() {
        if let Err(e) = rt.0.exec_source(BOOTSTRAP_JS, "<bootstrap>") {
            tracing::error!("[scripting] bootstrap failed: {e}");
            return;
        }
    }

    for (entity, path) in scripts {
        match std::fs::read_to_string(&path) {
            Ok(source) => {
                let id = entity.to_bits();
                let wrapped = format!(
                    "(function() {{\n{source}\nBsengine._scripts[\"{id}\"] = \
                     {{ onUpdate: typeof onUpdate === 'function' ? onUpdate : null }};\n}})();"
                );
                world.entity_mut(entity).insert(Script { source });
                if let Some(mut rt) = world.get_non_send_resource_mut::<ScriptRuntimeResource>() {
                    match rt.0.exec_source(&wrapped, &path) {
                        Ok(()) => tracing::info!("[scripting] loaded: {path}"),
                        Err(e) => tracing::error!("[scripting] error in {path}: {e}"),
                    }
                }
            }
            Err(e) => tracing::warn!("[scripting] cannot read {path}: {e}"),
        }
    }
}

const KEY_MAPPINGS: &[(KeyCode, &str)] = &[
    (KeyCode::W, "W"),
    (KeyCode::A, "A"),
    (KeyCode::S, "S"),
    (KeyCode::D, "D"),
    (KeyCode::Space, "Space"),
    (KeyCode::Enter, "Enter"),
    (KeyCode::Escape, "Escape"),
    (KeyCode::Up, "Up"),
    (KeyCode::Down, "Down"),
    (KeyCode::Left, "Left"),
    (KeyCode::Right, "Right"),
];

fn run_scripts(world: &mut World) {
    // In editor mode, only run scripts when Play is active
    if let Some(insp) = world.get_resource::<InspectorState>() {
        if insp.editor_mode && insp.play_state == EditorPlayState::Stopped {
            return;
        }
    }

    {
        let mut q = world.query::<&Script>();
        if q.iter(world).next().is_none() {
            return;
        }
    }

    let (scripted, collision_json) = collect_world_snapshots(world);

    if let Some(mut rt) = world.get_non_send_resource_mut::<ScriptRuntimeResource>() {
        // Dispatch collision events to JS before update
        if collision_json != "[]" {
            let call = format!("Bsengine._runCollisions({collision_json});");
            if let Err(e) = rt.0.exec_source(&call, "<run_collisions>") {
                tracing::error!("[scripting] _runCollisions error: {e}");
            }
        }

        let entities_json = serde_json::to_string(&scripted).unwrap_or_else(|_| "[]".to_string());
        let call = format!("Bsengine._runAll({entities_json});");
        if let Err(e) = rt.0.exec_source(&call, "<run_scripts>") {
            tracing::error!("[scripting] _runAll error: {e}");
        }
    }

    // Clear physics pointer — must happen after all V8 execution is complete.
    PHYSICS_WORLD_PTR.with(|p| *p.borrow_mut() = std::ptr::null());

    let commands: Vec<ScriptCommand> = COMMAND_BUFFER.with(|c| c.borrow().clone());
    for cmd in commands {
        match cmd {
            ScriptCommand::SetTransform { name, x, y, z } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation = Vec3::new(x, y, z);
                        break;
                    }
                }
            }
            ScriptCommand::SetRotation {
                name,
                rx,
                ry,
                rz,
                rw,
            } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.rotation = Quat::from_xyzw(rx, ry, rz, rw);
                        break;
                    }
                }
            }
            ScriptCommand::SetRotationEuler {
                name,
                pitch_deg,
                yaw_deg,
                roll_deg,
            } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.rotation = Quat::from_euler(
                            EulerRot::YXZ,
                            yaw_deg.to_radians(),
                            pitch_deg.to_radians(),
                            roll_deg.to_radians(),
                        );
                        break;
                    }
                }
            }
            ScriptCommand::SetScale { name, sx, sy, sz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale = Vec3::new(sx, sy, sz);
                        break;
                    }
                }
            }
            ScriptCommand::AddPosition { name, dx, dy, dz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation += Vec3::new(dx, dy, dz);
                        break;
                    }
                }
            }
            ScriptCommand::AddPositionLocal { name, dx, dy, dz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let rot = t.rotation;
                        t.translation += rot.mul_vec3(Vec3::new(dx, dy, dz));
                        break;
                    }
                }
            }
            ScriptCommand::SetPositionX { name, x } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation.x = x;
                        break;
                    }
                }
            }
            ScriptCommand::SetPositionY { name, y } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation.y = y;
                        break;
                    }
                }
            }
            ScriptCommand::SetPositionZ { name, z } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation.z = z;
                        break;
                    }
                }
            }
            ScriptCommand::AddPositionX { name, dx } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation.x += dx;
                        break;
                    }
                }
            }
            ScriptCommand::AddPositionY { name, dy } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation.y += dy;
                        break;
                    }
                }
            }
            ScriptCommand::AddPositionZ { name, dz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation.z += dz;
                        break;
                    }
                }
            }
            ScriptCommand::SetEmissive { name, r, g, b } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mat) = world.get_mut::<Material>(e) {
                        mat.emissive = Vec3::new(r, g, b).into();
                    } else {
                        world.entity_mut(e).insert(Material {
                            emissive: Vec3::new(r, g, b).into(),
                            ..Default::default()
                        });
                    }
                }
            }
            ScriptCommand::SetColor { name, r, g, b } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mat) = world.get_mut::<Material>(e) {
                        mat.base_color = Vec3::new(r, g, b).into();
                    } else {
                        world.entity_mut(e).insert(Material {
                            base_color: Vec3::new(r, g, b).into(),
                            ..Default::default()
                        });
                    }
                }
            }
            ScriptCommand::SetMetallic { name, value } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mat) = world.get_mut::<Material>(e) {
                        mat.metallic = value;
                    } else {
                        world.entity_mut(e).insert(Material {
                            metallic: value,
                            ..Default::default()
                        });
                    }
                }
            }
            ScriptCommand::SetRoughness { name, value } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mat) = world.get_mut::<Material>(e) {
                        mat.roughness = value;
                    } else {
                        world.entity_mut(e).insert(Material {
                            roughness: value,
                            ..Default::default()
                        });
                    }
                }
            }
            ScriptCommand::SetPointLightColor { name, r, g, b } => {
                use bsengine_core::PointLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<PointLight>(e) {
                        light.color = glam::Vec3::new(r, g, b).into();
                    }
                }
            }
            ScriptCommand::SetPointLightIntensity { name, value } => {
                use bsengine_core::PointLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<PointLight>(e) {
                        light.intensity = value;
                    }
                }
            }
            ScriptCommand::SetPointLightRange { name, value } => {
                use bsengine_core::PointLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<PointLight>(e) {
                        light.range = value;
                    }
                }
            }
            ScriptCommand::SetSpotLightColor { name, r, g, b } => {
                use bsengine_core::SpotLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<SpotLight>(e) {
                        light.color = glam::Vec3::new(r, g, b).into();
                    }
                }
            }
            ScriptCommand::SetSpotLightIntensity { name, value } => {
                use bsengine_core::SpotLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<SpotLight>(e) {
                        light.intensity = value;
                    }
                }
            }
            ScriptCommand::SetSpotLightRange { name, value } => {
                use bsengine_core::SpotLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<SpotLight>(e) {
                        light.range = value;
                    }
                }
            }
            ScriptCommand::SetSpotLightInnerAngle { name, deg } => {
                use bsengine_core::SpotLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<SpotLight>(e) {
                        light.inner_angle_degrees = deg.into();
                    }
                }
            }
            ScriptCommand::SetSpotLightOuterAngle { name, deg } => {
                use bsengine_core::SpotLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<SpotLight>(e) {
                        light.outer_angle_degrees = deg.into();
                    }
                }
            }
            ScriptCommand::SetDirectionalLightColor { name, r, g, b } => {
                use bsengine_core::DirectionalLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<DirectionalLight>(e) {
                        light.color = glam::Vec3::new(r, g, b).into();
                    }
                }
            }
            ScriptCommand::SetDirectionalLightAmbient { name, r, g, b } => {
                use bsengine_core::DirectionalLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<DirectionalLight>(e) {
                        light.ambient = glam::Vec3::new(r, g, b).into();
                    }
                }
            }
            ScriptCommand::SetDirectionalLightDirection { name, x, y, z } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    // Direction lives on Transform.rotation (rotation * -Z), same as SpotLight.
                    let dir = glam::Vec3::new(x, y, z).normalize_or(glam::Vec3::NEG_Z);
                    let rotation = Quat::from_rotation_arc(glam::Vec3::NEG_Z, dir);
                    if let Some(mut t) = world.get_mut::<Transform>(e) {
                        t.rotation = rotation;
                    }
                }
            }
            ScriptCommand::SetCameraFov { name, deg } => {
                use bsengine_core::Camera;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cam) = world.get_mut::<Camera>(e) {
                        cam.fov_y_degrees = deg.into();
                    }
                }
            }
            ScriptCommand::SetCameraNear { name, value } => {
                use bsengine_core::Camera;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cam) = world.get_mut::<Camera>(e) {
                        cam.near = value;
                    }
                }
            }
            ScriptCommand::SetCameraFar { name, value } => {
                use bsengine_core::Camera;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cam) = world.get_mut::<Camera>(e) {
                        cam.far = value;
                    }
                }
            }
            ScriptCommand::SetDamping { name, value } => {
                use bsengine_core::Damping;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Damping>(e) {
                        d.linear = value;
                    }
                }
            }
            ScriptCommand::PlayAnimation { name, clip } => {
                use bsengine_core::AnimationPlayer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ap) = world.get_mut::<AnimationPlayer>(e) {
                        ap.clip = clip;
                        ap.time = 0.0;
                        ap.playing = true;
                    }
                }
            }
            ScriptCommand::PauseAnimation { name } => {
                use bsengine_core::AnimationPlayer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ap) = world.get_mut::<AnimationPlayer>(e) {
                        ap.pause();
                    }
                }
            }
            ScriptCommand::ResumeAnimation { name } => {
                use bsengine_core::AnimationPlayer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ap) = world.get_mut::<AnimationPlayer>(e) {
                        ap.play();
                    }
                }
            }
            ScriptCommand::ResetAnimation { name } => {
                use bsengine_core::AnimationPlayer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ap) = world.get_mut::<AnimationPlayer>(e) {
                        ap.reset();
                    }
                }
            }
            ScriptCommand::SetAnimationSpeed { name, speed } => {
                use bsengine_core::AnimationPlayer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ap) = world.get_mut::<AnimationPlayer>(e) {
                        ap.speed = speed;
                    }
                }
            }
            ScriptCommand::SetAnimationLooping { name, looping } => {
                use bsengine_core::AnimationPlayer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ap) = world.get_mut::<AnimationPlayer>(e) {
                        ap.looping = looping;
                    }
                }
            }
            ScriptCommand::AsmSetTrigger { name, trigger } => {
                use bsengine_core::AnimationStateMachine;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut asm) = world.get_mut::<AnimationStateMachine>(e) {
                        asm.set_trigger(trigger);
                    }
                }
            }
            ScriptCommand::AsmSetFloat { name, param, value } => {
                use bsengine_core::AnimationStateMachine;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut asm) = world.get_mut::<AnimationStateMachine>(e) {
                        asm.set_float(param, value);
                    }
                }
            }
            ScriptCommand::AsmSetBool { name, param, value } => {
                use bsengine_core::AnimationStateMachine;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut asm) = world.get_mut::<AnimationStateMachine>(e) {
                        asm.set_bool(param, value);
                    }
                }
            }
            ScriptCommand::SetLifetime { name, seconds } => {
                use bsengine_core::Lifetime;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lt) = world.get_mut::<Lifetime>(e) {
                        lt.remaining = seconds.max(0.0);
                    }
                }
            }
            ScriptCommand::DamageShield { name, amount } => {
                use bsengine_core::Shield;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<Shield>(e) {
                        sh.absorb(amount);
                    }
                }
            }
            ScriptCommand::RestoreShield { name, amount } => {
                use bsengine_core::Shield;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<Shield>(e) {
                        sh.current = (sh.current + amount.max(0.0)).min(sh.max);
                    }
                }
            }
            ScriptCommand::SetMaxShield { name, value } => {
                use bsengine_core::Shield;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<Shield>(e) {
                        sh.max = value.max(0.0);
                        sh.current = sh.current.min(sh.max);
                    }
                }
            }
            ScriptCommand::ResetTimer { name } => {
                use bsengine_core::Timer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Timer>(e) {
                        t.reset();
                    }
                }
            }
            ScriptCommand::SetNavDestination { name, x, y, z } => {
                use bsengine_core::NavMeshAgent;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<NavMeshAgent>(e) {
                        a.destination = Some(glam::Vec3::new(x, y, z).into());
                    }
                }
            }
            ScriptCommand::ClearNavDestination { name } => {
                use bsengine_core::NavMeshAgent;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<NavMeshAgent>(e) {
                        a.clear_destination();
                    }
                }
            }
            ScriptCommand::SetNavSpeed { name, speed } => {
                use bsengine_core::NavMeshAgent;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<NavMeshAgent>(e) {
                        a.speed = speed.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNavAngularSpeed { name, speed } => {
                use bsengine_core::NavMeshAgent;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<NavMeshAgent>(e) {
                        a.angular_speed = speed.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNavStoppingDistance { name, distance } => {
                use bsengine_core::NavMeshAgent;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<NavMeshAgent>(e) {
                        a.stopping_distance = distance.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNavEnabled { name, enabled } => {
                use bsengine_core::NavMeshAgent;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<NavMeshAgent>(e) {
                        a.enabled = enabled;
                    }
                }
            }
            ScriptCommand::NavmeshInit {
                width,
                depth,
                cell_size,
                origin_x,
                origin_y,
                origin_z,
            } => {
                use bsengine_core::NavMesh;
                world.insert_resource(NavMesh::new(
                    width,
                    depth,
                    cell_size,
                    glam::Vec3::new(origin_x, origin_y, origin_z),
                ));
            }
            ScriptCommand::NavmeshSetWalkable { x, z, walkable } => {
                use bsengine_core::NavMesh;
                if let Some(mut nm) = world.get_resource_mut::<NavMesh>() {
                    nm.set_walkable(x, z, walkable);
                }
            }
            ScriptCommand::SaveGame { path } => {
                if let Err(e) = crate::save::save_world(world, &path) {
                    tracing::warn!("[save] {}", e);
                }
            }
            ScriptCommand::LoadGame { path } => {
                if let Err(e) = crate::save::load_world(world, &path) {
                    tracing::warn!("[load] {}", e);
                }
            }
            ScriptCommand::SetCustomShader { name, path } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    world.entity_mut(e).insert(CustomShader { path });
                }
            }
            ScriptCommand::NetworkStartServer { port } => {
                match bsengine_network::NetworkSession::new_server(port) {
                    Ok(session) => {
                        world.insert_resource(session);
                    }
                    Err(e) => tracing::warn!("[network] start_server failed: {e}"),
                }
            }
            ScriptCommand::NetworkConnect { host, port } => {
                match bsengine_network::NetworkSession::new_client(&host, port) {
                    Ok(session) => {
                        world.insert_resource(session);
                    }
                    Err(e) => tracing::warn!("[network] connect failed: {e}"),
                }
            }
            ScriptCommand::NetworkDisconnect => {
                world.remove_resource::<bsengine_network::NetworkSession>();
            }
            ScriptCommand::ClearCustomShader { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    world.entity_mut(e).remove::<CustomShader>();
                }
            }
            ScriptCommand::SetBloomIntensity { name, intensity } => {
                use bsengine_core::Bloom;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Bloom>(e) {
                        b.intensity = intensity.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBloomThreshold { name, threshold } => {
                use bsengine_core::Bloom;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Bloom>(e) {
                        b.threshold = threshold.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBloomRadius { name, radius } => {
                use bsengine_core::Bloom;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Bloom>(e) {
                        b.radius = radius.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBloomSoftness { name, softness } => {
                use bsengine_core::Bloom;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Bloom>(e) {
                        b.softness = softness.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetBloomEnabled { name, enabled } => {
                use bsengine_core::Bloom;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Bloom>(e) {
                        b.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetAoRadius { name, radius } => {
                use bsengine_core::AmbientOcclusion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ao) = world.get_mut::<AmbientOcclusion>(e) {
                        ao.radius = radius.max(0.0);
                    }
                }
            }
            ScriptCommand::SetAoBias { name, bias } => {
                use bsengine_core::AmbientOcclusion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ao) = world.get_mut::<AmbientOcclusion>(e) {
                        ao.bias = bias.max(0.0);
                    }
                }
            }
            ScriptCommand::SetAoIntensity { name, intensity } => {
                use bsengine_core::AmbientOcclusion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ao) = world.get_mut::<AmbientOcclusion>(e) {
                        ao.intensity = intensity.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetAoSampleCount { name, count } => {
                use bsengine_core::AmbientOcclusion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ao) = world.get_mut::<AmbientOcclusion>(e) {
                        ao.sample_count = count.max(1);
                    }
                }
            }
            ScriptCommand::SetAoEnabled { name, enabled } => {
                use bsengine_core::AmbientOcclusion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ao) = world.get_mut::<AmbientOcclusion>(e) {
                        ao.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetToneMapMode { name, mode } => {
                use bsengine_core::{ToneMap, ToneMappingMode};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tm) = world.get_mut::<ToneMap>(e) {
                        tm.mode = match mode {
                            0 => ToneMappingMode::None,
                            1 => ToneMappingMode::Reinhard,
                            2 => ToneMappingMode::ReinhardLuminance,
                            3 => ToneMappingMode::Aces,
                            4 => ToneMappingMode::Filmic,
                            _ => ToneMappingMode::Aces,
                        };
                    }
                }
            }
            ScriptCommand::SetToneMapExposure { name, exposure } => {
                use bsengine_core::ToneMap;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tm) = world.get_mut::<ToneMap>(e) {
                        tm.exposure = exposure;
                    }
                }
            }
            ScriptCommand::SetToneMapEnabled { name, enabled } => {
                use bsengine_core::ToneMap;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tm) = world.get_mut::<ToneMap>(e) {
                        tm.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetTweenDuration { name, duration } => {
                use bsengine_core::Tween;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tw) = world.get_mut::<Tween>(e) {
                        tw.duration = duration.max(0.0);
                    }
                }
            }
            ScriptCommand::SetTweenEasing { name, easing } => {
                use bsengine_core::{EasingFn, Tween};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tw) = world.get_mut::<Tween>(e) {
                        tw.easing = match easing {
                            1 => EasingFn::EaseInQuad,
                            2 => EasingFn::EaseOutQuad,
                            3 => EasingFn::EaseInOutQuad,
                            _ => EasingFn::Linear,
                        };
                    }
                }
            }
            ScriptCommand::SetTweenRepeat { name, repeat } => {
                use bsengine_core::{RepeatMode, Tween};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tw) = world.get_mut::<Tween>(e) {
                        tw.repeat = match repeat {
                            1 => RepeatMode::Loop,
                            2 => RepeatMode::PingPong,
                            _ => RepeatMode::Once,
                        };
                    }
                }
            }
            ScriptCommand::SetTweenElapsed { name, elapsed } => {
                use bsengine_core::Tween;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tw) = world.get_mut::<Tween>(e) {
                        tw.elapsed = elapsed.clamp(0.0, tw.duration);
                        tw.finished = false;
                    }
                }
            }
            ScriptCommand::SetFollowTarget { name, target } => {
                use bsengine_core::Follow;
                let target_entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == target).map(|(e, _)| e)
                };
                if let Some(te) = target_entity {
                    let entity = {
                        let mut q = world.query::<(Entity, &Name)>();
                        q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                    };
                    if let Some(e) = entity {
                        if let Some(mut f) = world.get_mut::<Follow>(e) {
                            f.target = te;
                        }
                    }
                }
            }
            ScriptCommand::SetFollowOffset { name, x, y, z } => {
                use bsengine_core::Follow;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Follow>(e) {
                        f.offset = Vec3::new(x, y, z).into();
                    }
                }
            }
            ScriptCommand::SetFollowSpeed { name, speed } => {
                use bsengine_core::Follow;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Follow>(e) {
                        f.speed = speed;
                    }
                }
            }
            ScriptCommand::SetLookAtTarget { name, target } => {
                use bsengine_core::LookAt;
                let target_entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == target).map(|(e, _)| e)
                };
                if let Some(te) = target_entity {
                    let entity = {
                        let mut q = world.query::<(Entity, &Name)>();
                        q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                    };
                    if let Some(e) = entity {
                        if let Some(mut la) = world.get_mut::<LookAt>(e) {
                            la.target = te;
                        }
                    }
                }
            }
            ScriptCommand::SetLookAtUp { name, x, y, z } => {
                use bsengine_core::LookAt;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<LookAt>(e) {
                        la.up = Vec3::new(x, y, z).into();
                    }
                }
            }
            ScriptCommand::Spawn(params) => {
                spawn_entity(world, params);
            }
            ScriptCommand::Destroy { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    world.despawn(e);
                }
            }
            ScriptCommand::PlaySound {
                id,
                path,
                volume,
                loop_,
            } => {
                let project_dir = world
                    .get_resource::<ProjectDir>()
                    .map(|pd| pd.0.clone())
                    .unwrap_or_default();
                let full_path = if project_dir.is_empty() {
                    path.clone()
                } else {
                    format!("{}/{}", project_dir, path)
                };
                match kira::sound::static_sound::StaticSoundData::from_file(&full_path) {
                    Ok(data) => {
                        use kira::Decibels;
                        let volume_db = 20.0_f32 * volume.max(1e-10_f32).log10();
                        let data = data.volume(Decibels(volume_db));
                        let data = if loop_ { data.loop_region(..) } else { data };
                        if let Some(mut audio) = world.get_resource_mut::<AudioWorld>() {
                            if let Some(handle) = audio.play(data) {
                                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>()
                                {
                                    handles.0.insert(id, handle);
                                }
                            }
                        }
                    }
                    Err(e) => tracing::warn!("[audio] failed to load {full_path}: {e}"),
                }
            }
            ScriptCommand::StopSound { id } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(mut handle) = handles.0.remove(&id) {
                        use kira::Tween;
                        handle.stop(Tween::default());
                    }
                }
            }
            ScriptCommand::PauseSound { id } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(handle) = handles.0.get_mut(&id) {
                        use kira::Tween;
                        handle.pause(Tween::default());
                    }
                }
            }
            ScriptCommand::ResumeSound { id } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(handle) = handles.0.get_mut(&id) {
                        use kira::Tween;
                        handle.resume(Tween::default());
                    }
                }
            }
            ScriptCommand::SetSoundVolume { id, db } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(handle) = handles.0.get_mut(&id) {
                        use kira::{Decibels, Tween};
                        handle.set_volume(Decibels(db), Tween::default());
                    }
                }
            }
            ScriptCommand::SetSoundPanning { id, panning } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(handle) = handles.0.get_mut(&id) {
                        use kira::{Panning, Tween};
                        handle.set_panning(Panning(panning), Tween::default());
                    }
                }
            }
            ScriptCommand::SetSoundPlaybackRate { id, rate } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(handle) = handles.0.get_mut(&id) {
                        use kira::{PlaybackRate, Tween};
                        handle.set_playback_rate(PlaybackRate(rate as f64), Tween::default());
                    }
                }
            }
            ScriptCommand::SeekSound { id, position } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(handle) = handles.0.get_mut(&id) {
                        handle.seek_to(position);
                    }
                }
            }
            ScriptCommand::SetHudText { id, text } => {
                if let Some(mut hud) = world.get_resource_mut::<HudTexts>() {
                    hud.0.insert(id, text);
                }
            }
            ScriptCommand::ClearHudText { id } => {
                if let Some(mut hud) = world.get_resource_mut::<HudTexts>() {
                    hud.0.remove(&id);
                }
            }
            ScriptCommand::SetUiLabel {
                id,
                text,
                x,
                y,
                font_size,
            } => {
                if let Some(mut ui) = world.get_resource_mut::<UiState>() {
                    ui.set_widget(UiWidget::Label {
                        id,
                        text,
                        x,
                        y,
                        font_size,
                    });
                }
            }
            ScriptCommand::SetUiButton {
                id,
                label,
                x,
                y,
                width,
                height,
            } => {
                if let Some(mut ui) = world.get_resource_mut::<UiState>() {
                    ui.set_widget(UiWidget::Button {
                        id,
                        label,
                        x,
                        y,
                        width,
                        height,
                    });
                }
            }
            ScriptCommand::SetUiPanel {
                id,
                title,
                x,
                y,
                width,
                height,
            } => {
                if let Some(mut ui) = world.get_resource_mut::<UiState>() {
                    ui.set_widget(UiWidget::Panel {
                        id,
                        title,
                        x,
                        y,
                        width,
                        height,
                    });
                }
            }
            ScriptCommand::SetUiTextInput {
                id,
                hint,
                x,
                y,
                width,
            } => {
                if let Some(mut ui) = world.get_resource_mut::<UiState>() {
                    ui.set_widget(UiWidget::TextInput {
                        id,
                        hint,
                        x,
                        y,
                        width,
                    });
                }
            }
            ScriptCommand::RemoveUiWidget { id } => {
                if let Some(mut ui) = world.get_resource_mut::<UiState>() {
                    ui.remove_widget(&id);
                }
            }
            ScriptCommand::ClearUiWidgets => {
                if let Some(mut ui) = world.get_resource_mut::<UiState>() {
                    ui.clear();
                }
            }
            ScriptCommand::LoadScene { path } => {
                world.insert_resource(PendingSceneLoad { path });
            }
            ScriptCommand::SetVisible { name, visible } => {
                let mut q = world.query::<(&Name, &mut Visible)>();
                for (n, mut v) in q.iter_mut(world) {
                    if n.0 == name {
                        v.is_visible = visible;
                        break;
                    }
                }
            }
            ScriptCommand::SetSkybox { path } => {
                let project_dir = world
                    .get_resource::<ProjectDir>()
                    .map(|pd| pd.0.clone())
                    .unwrap_or_default();
                let full_path = if project_dir.is_empty() {
                    path
                } else {
                    format!("{}/{}", project_dir, path)
                };
                world.insert_resource(SkyboxPath(Some(full_path)));
            }
            ScriptCommand::SetParent { child, parent } => {
                let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                let mut child_entity = None;
                let mut parent_entity = None;
                for (e, n) in q.iter(world) {
                    if n.0 == child {
                        child_entity = Some(e);
                    } else if n.0 == parent {
                        parent_entity = Some(e);
                    }
                }
                if let (Some(ce), Some(pe)) = (child_entity, parent_entity) {
                    world.entity_mut(ce).insert(Parent(pe));
                }
            }
            ScriptCommand::ClearParent { child } => {
                let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                let child_entity = q.iter(world).find(|(_, n)| n.0 == child).map(|(e, _)| e);
                if let Some(ce) = child_entity {
                    world.entity_mut(ce).remove::<Parent>();
                }
            }
            ScriptCommand::SetCursorVisible { visible } => {
                if let Some(mut cfg) = world.get_resource_mut::<CursorConfig>() {
                    cfg.visible = visible;
                } else {
                    world.insert_resource(CursorConfig {
                        visible,
                        locked: false,
                    });
                }
            }
            ScriptCommand::SetCursorLocked { locked } => {
                if let Some(mut cfg) = world.get_resource_mut::<CursorConfig>() {
                    cfg.locked = locked;
                } else {
                    world.insert_resource(CursorConfig {
                        visible: true,
                        locked,
                    });
                }
            }
            ScriptCommand::AddImpulse { name, fx, fy, fz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.apply_impulse(e, Vec3::new(fx, fy, fz));
                }
            }
            ScriptCommand::AddImpulseAtPoint {
                name,
                fx,
                fy,
                fz,
                px,
                py,
                pz,
            } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.apply_impulse_at_point(e, Vec3::new(fx, fy, fz), Vec3::new(px, py, pz));
                }
            }
            ScriptCommand::AddForce { name, fx, fy, fz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.apply_force(e, Vec3::new(fx, fy, fz));
                }
            }
            ScriptCommand::AddForceAtPoint {
                name,
                fx,
                fy,
                fz,
                px,
                py,
                pz,
            } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.apply_force_at_point(e, Vec3::new(fx, fy, fz), Vec3::new(px, py, pz));
                }
            }
            ScriptCommand::SetVelocity { name, vx, vy, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_linvel(e, Vec3::new(vx, vy, vz));
                }
            }
            ScriptCommand::SetVelocityX { name, vx } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_linvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_linvel(e, Vec3::new(vx, cur.y, cur.z));
                }
            }
            ScriptCommand::SetVelocityY { name, vy } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_linvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_linvel(e, Vec3::new(cur.x, vy, cur.z));
                }
            }
            ScriptCommand::SetVelocityZ { name, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_linvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_linvel(e, Vec3::new(cur.x, cur.y, vz));
                }
            }
            ScriptCommand::SetGravity { magnitude } => {
                if let Some(mut pw) = world.get_resource_mut::<PhysicsWorld>() {
                    pw.set_gravity(magnitude);
                }
            }
            ScriptCommand::SetAngularVelocity { name, vx, vy, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_angvel(e, Vec3::new(vx, vy, vz));
                }
            }
            ScriptCommand::SetAngularVelocityX { name, vx } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_angvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_angvel(e, Vec3::new(vx, cur.y, cur.z));
                }
            }
            ScriptCommand::SetAngularVelocityY { name, vy } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_angvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_angvel(e, Vec3::new(cur.x, vy, cur.z));
                }
            }
            ScriptCommand::SetAngularVelocityZ { name, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_angvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_angvel(e, Vec3::new(cur.x, cur.y, vz));
                }
            }
            ScriptCommand::AddVelocity { name, vx, vy, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_linvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_linvel(e, cur + Vec3::new(vx, vy, vz));
                }
            }
            ScriptCommand::AddAngularVelocity { name, vx, vy, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_angvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_angvel(e, cur + Vec3::new(vx, vy, vz));
                }
            }
            ScriptCommand::AddAngularImpulse { name, vx, vy, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.apply_torque_impulse(e, Vec3::new(vx, vy, vz));
                }
            }
            ScriptCommand::AddTorque { name, vx, vy, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.add_torque(e, Vec3::new(vx, vy, vz));
                }
            }
            ScriptCommand::SetCCDEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_ccd_enabled(e, enabled);
                }
            }
            ScriptCommand::SetLinearDamping { name, damping } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_linear_damping(e, damping);
                }
            }
            ScriptCommand::SetAngularDamping { name, damping } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_angular_damping(e, damping);
                }
            }
            ScriptCommand::SetMass { name, mass } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_mass(e, mass);
                }
            }
            ScriptCommand::LockRotation {
                name,
                lock_x,
                lock_y,
                lock_z,
            } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.lock_rotations(e, lock_x, lock_y, lock_z);
                }
            }
            ScriptCommand::LockTranslation {
                name,
                lock_x,
                lock_y,
                lock_z,
            } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.lock_translations(e, lock_x, lock_y, lock_z);
                }
            }
            ScriptCommand::WakeUp { name } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.wake_up(e);
                }
            }
            ScriptCommand::PutToSleep { name } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.put_to_sleep(e);
                }
            }
            ScriptCommand::RotateBy {
                name,
                rx,
                ry,
                rz,
                rw,
            } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let delta = Quat::from_xyzw(rx, ry, rz, rw).normalize();
                        t.rotation = (t.rotation * delta).normalize();
                        break;
                    }
                }
            }
            ScriptCommand::RotateAroundAxis {
                name,
                ax,
                ay,
                az,
                angle_deg,
            } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let axis = Vec3::new(ax, ay, az);
                        if axis.length_squared() > 1e-10 {
                            let delta =
                                Quat::from_axis_angle(axis.normalize(), angle_deg.to_radians());
                            t.rotation = (t.rotation * delta).normalize();
                        }
                        break;
                    }
                }
            }
            ScriptCommand::AddRotationEuler {
                name,
                pitch,
                yaw,
                roll,
            } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let delta = Quat::from_euler(
                            glam::EulerRot::XYZ,
                            pitch.to_radians(),
                            yaw.to_radians(),
                            roll.to_radians(),
                        );
                        t.rotation = (t.rotation * delta).normalize();
                        break;
                    }
                }
            }
            ScriptCommand::AddRotationEulerX { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let delta = Quat::from_euler(EulerRot::XYZ, deg.to_radians(), 0.0, 0.0);
                        t.rotation = (t.rotation * delta).normalize();
                        break;
                    }
                }
            }
            ScriptCommand::AddRotationEulerY { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let delta = Quat::from_euler(EulerRot::XYZ, 0.0, deg.to_radians(), 0.0);
                        t.rotation = (t.rotation * delta).normalize();
                        break;
                    }
                }
            }
            ScriptCommand::AddRotationEulerZ { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let delta = Quat::from_euler(EulerRot::XYZ, 0.0, 0.0, deg.to_radians());
                        t.rotation = (t.rotation * delta).normalize();
                        break;
                    }
                }
            }
            ScriptCommand::SetScaleX { name, x } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.x = x;
                        break;
                    }
                }
            }
            ScriptCommand::SetScaleY { name, y } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.y = y;
                        break;
                    }
                }
            }
            ScriptCommand::SetScaleZ { name, z } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.z = z;
                        break;
                    }
                }
            }
            ScriptCommand::AddScaleX { name, dx } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.x += dx;
                        break;
                    }
                }
            }
            ScriptCommand::AddScaleY { name, dy } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.y += dy;
                        break;
                    }
                }
            }
            ScriptCommand::AddScaleZ { name, dz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.z += dz;
                        break;
                    }
                }
            }
            ScriptCommand::AddScale { name, sx, sy, sz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale += Vec3::new(sx, sy, sz);
                        break;
                    }
                }
            }
            ScriptCommand::SetRotationEulerX { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let (_, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                        t.rotation = Quat::from_euler(EulerRot::XYZ, deg.to_radians(), y, z);
                        break;
                    }
                }
            }
            ScriptCommand::SetRotationEulerY { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let (x, _, z) = t.rotation.to_euler(EulerRot::XYZ);
                        t.rotation = Quat::from_euler(EulerRot::XYZ, x, deg.to_radians(), z);
                        break;
                    }
                }
            }
            ScriptCommand::SetRotationEulerZ { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let (x, y, _) = t.rotation.to_euler(EulerRot::XYZ);
                        t.rotation = Quat::from_euler(EulerRot::XYZ, x, y, deg.to_radians());
                        break;
                    }
                }
            }
            ScriptCommand::MultiplyScale { name, sx, sy, sz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale *= Vec3::new(sx, sy, sz);
                        break;
                    }
                }
            }
            ScriptCommand::SetKinematic { name, kinematic } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_body_type(e, kinematic);
                }
            }
            ScriptCommand::SetGravityScale { name, scale } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_gravity_scale(e, scale);
                }
            }
            ScriptCommand::SetColliderSensor { name, sensor } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_collider_sensor(e, sensor);
                }
            }
            ScriptCommand::SetRestitution { name, restitution } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_restitution(e, restitution);
                }
            }
            ScriptCommand::SetFriction { name, friction } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_friction(e, friction);
                }
            }
        }
    }
}

fn spawn_entity(world: &mut World, params: SpawnParams) {
    let prim = match params.primitive.as_str() {
        "Sphere" => Primitive::Sphere,
        "Plane" => Primitive::Plane,
        "Capsule" => Primitive::Capsule,
        _ => Primitive::Cube,
    };

    let transform = Transform {
        translation: Vec3::new(params.x, params.y, params.z),
        rotation: Quat::from_xyzw(params.rx, params.ry, params.rz, params.rw).normalize(),
        scale: Vec3::new(params.sx, params.sy, params.sz),
    };

    let mut cmd = world.spawn((
        Name(params.name.clone()),
        transform,
        GlobalTransform::default(),
        PrimitiveMesh(prim),
    ));

    let has_color = params.color.is_some() || params.emissive.is_some();
    if has_color {
        cmd.insert(Material {
            base_color: params.color.map(Vec3::from).unwrap_or(Vec3::ONE).into(),
            emissive: params.emissive.map(Vec3::from).unwrap_or(Vec3::ZERO).into(),
            ..Default::default()
        });
    }

    if let Some(script) = params.script {
        cmd.insert(ScriptPath(script));
    }
}

fn collect_world_snapshots(world: &mut World) -> (Vec<(String, String)>, String) {
    let transform_snapshot: HashMap<String, (Vec3, Quat, Vec3)> = {
        let mut q = world.query::<(&Name, &Transform)>();
        q.iter(world)
            .map(|(n, t)| (n.0.clone(), (t.translation, t.rotation, t.scale)))
            .collect()
    };

    let world_transform_snapshot: HashMap<String, (Vec3, Quat, Vec3)> = {
        let mut q = world.query::<(&Name, &GlobalTransform)>();
        q.iter(world)
            .map(|(n, gt)| {
                let (scale, rot, pos) = gt.0.to_scale_rotation_translation();
                (n.0.clone(), (pos, rot, scale))
            })
            .collect()
    };

    let visible_snapshot: HashMap<String, bool> = {
        let mut q = world.query::<(&Name, &Visible)>();
        q.iter(world)
            .map(|(n, v)| (n.0.clone(), v.is_visible))
            .collect()
    };

    let material_color_snapshot: HashMap<String, [f32; 3]> = {
        let mut q = world.query::<(&Name, &Material)>();
        q.iter(world)
            .map(|(n, m)| (n.0.clone(), m.base_color.to_array()))
            .collect()
    };

    let material_emissive_snapshot: HashMap<String, [f32; 3]> = {
        let mut q = world.query::<(&Name, &Material)>();
        q.iter(world)
            .map(|(n, m)| (n.0.clone(), m.emissive.to_array()))
            .collect()
    };

    let material_metallic_snapshot: HashMap<String, f32> = {
        let mut q = world.query::<(&Name, &Material)>();
        q.iter(world)
            .map(|(n, m)| (n.0.clone(), m.metallic))
            .collect()
    };

    let material_roughness_snapshot: HashMap<String, f32> = {
        let mut q = world.query::<(&Name, &Material)>();
        q.iter(world)
            .map(|(n, m)| (n.0.clone(), m.roughness))
            .collect()
    };

    let (key_snapshot, key_just_pressed, key_just_released): (
        HashSet<String>,
        HashSet<String>,
        HashSet<String>,
    ) = if let Some(input) = world.get_resource::<Input<KeyCode>>() {
        let pressed = KEY_MAPPINGS
            .iter()
            .filter(|(code, _)| input.is_pressed(code))
            .map(|(_, name)| name.to_string())
            .collect();
        let just_pressed = KEY_MAPPINGS
            .iter()
            .filter(|(code, _)| input.just_pressed(code))
            .map(|(_, name)| name.to_string())
            .collect();
        let just_released = KEY_MAPPINGS
            .iter()
            .filter(|(code, _)| input.just_released(code))
            .map(|(_, name)| name.to_string())
            .collect();
        (pressed, just_pressed, just_released)
    } else {
        (HashSet::new(), HashSet::new(), HashSet::new())
    };

    let (mb_pressed, mb_just_pressed, mb_just_released): (u8, u8, u8) =
        if let Some(buttons) = world.get_resource::<Input<MouseButton>>() {
            let mut p = 0u8;
            let mut jp = 0u8;
            let mut jr = 0u8;
            if buttons.is_pressed(&MouseButton::Left) {
                p |= 1;
            }
            if buttons.is_pressed(&MouseButton::Right) {
                p |= 2;
            }
            if buttons.is_pressed(&MouseButton::Middle) {
                p |= 4;
            }
            if buttons.just_pressed(&MouseButton::Left) {
                jp |= 1;
            }
            if buttons.just_pressed(&MouseButton::Right) {
                jp |= 2;
            }
            if buttons.just_pressed(&MouseButton::Middle) {
                jp |= 4;
            }
            if buttons.just_released(&MouseButton::Left) {
                jr |= 1;
            }
            if buttons.just_released(&MouseButton::Right) {
                jr |= 2;
            }
            if buttons.just_released(&MouseButton::Middle) {
                jr |= 4;
            }
            (p, jp, jr)
        } else {
            (0, 0, 0)
        };

    let (mouse_pos, mouse_delta) = world
        .get_resource::<MouseState>()
        .map(|ms| (ms.position, ms.delta))
        .unwrap_or(((0.0, 0.0), (0.0, 0.0)));

    const GAMEPAD_MAPPINGS: &[(GamepadButton, u32)] = &[
        (GamepadButton::South, 0),
        (GamepadButton::East, 1),
        (GamepadButton::West, 2),
        (GamepadButton::North, 3),
        (GamepadButton::LB, 4),
        (GamepadButton::RB, 5),
        (GamepadButton::LT, 6),
        (GamepadButton::RT, 7),
        (GamepadButton::Select, 8),
        (GamepadButton::Start, 9),
        (GamepadButton::LeftStick, 10),
        (GamepadButton::RightStick, 11),
        (GamepadButton::DPadUp, 12),
        (GamepadButton::DPadDown, 13),
        (GamepadButton::DPadLeft, 14),
        (GamepadButton::DPadRight, 15),
    ];

    let (gpad_pressed, gpad_just_pressed, gpad_just_released): (u16, u16, u16) =
        if let Some(gpad) = world.get_resource::<Input<GamepadButton>>() {
            let mut p = 0u16;
            let mut jp = 0u16;
            let mut jr = 0u16;
            for &(btn, bit) in GAMEPAD_MAPPINGS {
                let mask = 1u16 << bit;
                if gpad.is_pressed(&btn) {
                    p |= mask;
                }
                if gpad.just_pressed(&btn) {
                    jp |= mask;
                }
                if gpad.just_released(&btn) {
                    jr |= mask;
                }
            }
            (p, jp, jr)
        } else {
            (0, 0, 0)
        };

    let gamepad_sticks = world
        .get_resource::<GamepadSticks>()
        .map(|s| {
            (
                s.left.0,
                s.left.1,
                s.right.0,
                s.right.1,
                s.left_trigger,
                s.right_trigger,
            )
        })
        .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0, 0.0));

    let physics_ptr = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| pw as *const PhysicsWorld)
        .unwrap_or(std::ptr::null());

    let entity_name_map: HashMap<u64, String> = {
        let mut q = world.query::<(Entity, &Name)>();
        q.iter(world)
            .map(|(e, n)| (e.to_bits(), n.0.clone()))
            .collect()
    };

    let velocity_snapshot: HashMap<String, Vec3> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_linvel(entity).map(|v| (name.clone(), v))
                })
                .collect()
        })
        .unwrap_or_default();

    let angular_velocity_snapshot: HashMap<String, Vec3> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_angvel(entity).map(|v| (name.clone(), v))
                })
                .collect()
        })
        .unwrap_or_default();

    let parent_map: HashMap<String, String> = {
        let mut q = world.query::<(Entity, &Name, &Parent)>();
        q.iter(world)
            .filter_map(|(_, n, p)| {
                entity_name_map
                    .get(&p.0.to_bits())
                    .map(|pn| (n.0.clone(), pn.clone()))
            })
            .collect()
    };
    let children_map: HashMap<String, Vec<String>> = {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for (child, parent) in &parent_map {
            map.entry(parent.clone()).or_default().push(child.clone());
        }
        map
    };

    let scripted: Vec<(String, String)> = {
        let mut q = world.query::<(Entity, &Name, &Script)>();
        q.iter(world)
            .map(|(e, n, _)| (e.to_bits().to_string(), n.0.clone()))
            .collect()
    };

    let all_names: Vec<String> = {
        let mut q = world.query::<&Name>();
        q.iter(world).map(|n| n.0.clone()).collect()
    };

    let collision_json = COLLISION_SNAPSHOT.with(|s| {
        let evs = s.borrow();
        serde_json::to_string(
            &evs.iter()
                .map(|(a, b, started)| {
                    serde_json::json!({"nameA": a, "nameB": b, "started": started})
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|_| "[]".to_string())
    });

    TRANSFORM_SNAPSHOT.with(|s| *s.borrow_mut() = transform_snapshot);
    WORLD_TRANSFORM_SNAPSHOT.with(|s| *s.borrow_mut() = world_transform_snapshot);
    VISIBLE_SNAPSHOT.with(|s| *s.borrow_mut() = visible_snapshot);
    MATERIAL_COLOR_SNAPSHOT.with(|s| *s.borrow_mut() = material_color_snapshot);
    MATERIAL_EMISSIVE_SNAPSHOT.with(|s| *s.borrow_mut() = material_emissive_snapshot);
    MATERIAL_METALLIC_SNAPSHOT.with(|s| *s.borrow_mut() = material_metallic_snapshot);
    MATERIAL_ROUGHNESS_SNAPSHOT.with(|s| *s.borrow_mut() = material_roughness_snapshot);

    let (elapsed_secs, delta_secs) =
        if let Some(mut timing) = world.get_resource_mut::<ScriptTimingState>() {
            let now = std::time::Instant::now();
            let elapsed = now.duration_since(timing.startup).as_secs_f32();
            let delta = now.duration_since(timing.last_frame).as_secs_f32();
            timing.last_frame = now;
            (elapsed, delta)
        } else {
            (0.0, 0.0)
        };
    TIME_ELAPSED_SNAPSHOT.with(|s| *s.borrow_mut() = elapsed_secs);
    TIME_DELTA_SNAPSHOT.with(|s| *s.borrow_mut() = delta_secs);
    if let Some(ss) = world.get_resource::<ScreenSize>() {
        SCREEN_SIZE_SNAPSHOT.with(|s| *s.borrow_mut() = (ss.width, ss.height));
    }
    KEY_SNAPSHOT.with(|k| *k.borrow_mut() = key_snapshot);
    KEY_JUST_PRESSED_SNAPSHOT.with(|k| *k.borrow_mut() = key_just_pressed);
    KEY_JUST_RELEASED_SNAPSHOT.with(|k| *k.borrow_mut() = key_just_released);
    ENTITY_NAMES_SNAPSHOT.with(|s| *s.borrow_mut() = all_names);
    MOUSE_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = mb_pressed);
    MOUSE_JUST_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = mb_just_pressed);
    MOUSE_JUST_RELEASED_SNAPSHOT.with(|s| *s.borrow_mut() = mb_just_released);
    MOUSE_POS_SNAPSHOT.with(|s| *s.borrow_mut() = mouse_pos);
    MOUSE_DELTA_SNAPSHOT.with(|s| *s.borrow_mut() = mouse_delta);
    let ui_clicked: Vec<String> = world
        .get_resource::<UiState>()
        .map(|ui| ui.clicked.iter().cloned().collect())
        .unwrap_or_default();
    UI_CLICKED_SNAPSHOT.with(|s| *s.borrow_mut() = ui_clicked);
    let mass_snapshot: HashMap<String, f32> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_mass(entity).map(|m| (name.clone(), m))
                })
                .collect()
        })
        .unwrap_or_default();
    let gravity_scale_snapshot: HashMap<String, f32> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_gravity_scale(entity).map(|s| (name.clone(), s))
                })
                .collect()
        })
        .unwrap_or_default();
    let body_type_snapshot: HashMap<String, bool> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.is_kinematic(entity).map(|k| (name.clone(), k))
                })
                .collect()
        })
        .unwrap_or_default();
    let collider_sensor_snapshot: HashMap<String, bool> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.is_collider_sensor(entity).map(|s| (name.clone(), s))
                })
                .collect()
        })
        .unwrap_or_default();
    let linear_damping_snapshot: HashMap<String, f32> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_linear_damping(entity).map(|d| (name.clone(), d))
                })
                .collect()
        })
        .unwrap_or_default();
    let angular_damping_snapshot: HashMap<String, f32> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_angular_damping(entity).map(|d| (name.clone(), d))
                })
                .collect()
        })
        .unwrap_or_default();
    let restitution_snapshot: HashMap<String, f32> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_restitution(entity).map(|v| (name.clone(), v))
                })
                .collect()
        })
        .unwrap_or_default();
    let friction_snapshot: HashMap<String, f32> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_friction(entity).map(|v| (name.clone(), v))
                })
                .collect()
        })
        .unwrap_or_default();
    let sleep_snapshot: HashMap<String, bool> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.is_sleeping(entity).map(|v| (name.clone(), v))
                })
                .collect()
        })
        .unwrap_or_default();
    ENTITY_NAME_MAP.with(|m| *m.borrow_mut() = entity_name_map);
    PARENT_SNAPSHOT.with(|s| *s.borrow_mut() = parent_map);
    CHILDREN_SNAPSHOT.with(|s| *s.borrow_mut() = children_map);
    VELOCITY_SNAPSHOT.with(|s| *s.borrow_mut() = velocity_snapshot);
    ANGULAR_VELOCITY_SNAPSHOT.with(|s| *s.borrow_mut() = angular_velocity_snapshot);
    MASS_SNAPSHOT.with(|s| *s.borrow_mut() = mass_snapshot);
    GRAVITY_SCALE_SNAPSHOT.with(|s| *s.borrow_mut() = gravity_scale_snapshot);
    BODY_TYPE_SNAPSHOT.with(|s| *s.borrow_mut() = body_type_snapshot);
    COLLIDER_SENSOR_SNAPSHOT.with(|s| *s.borrow_mut() = collider_sensor_snapshot);
    LINEAR_DAMPING_SNAPSHOT.with(|s| *s.borrow_mut() = linear_damping_snapshot);
    ANGULAR_DAMPING_SNAPSHOT.with(|s| *s.borrow_mut() = angular_damping_snapshot);
    RESTITUTION_SNAPSHOT.with(|s| *s.borrow_mut() = restitution_snapshot);
    FRICTION_SNAPSHOT.with(|s| *s.borrow_mut() = friction_snapshot);
    SLEEP_SNAPSHOT.with(|s| *s.borrow_mut() = sleep_snapshot);
    let gravity = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| pw.gravity())
        .unwrap_or(9.81);
    GRAVITY_SNAPSHOT.with(|s| *s.borrow_mut() = gravity);
    PHYSICS_WORLD_PTR.with(|p| *p.borrow_mut() = physics_ptr);
    GAMEPAD_BUTTON_SNAPSHOT.with(|s| *s.borrow_mut() = gpad_pressed);
    GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = gpad_just_pressed);
    GAMEPAD_BUTTON_JUST_RELEASED_SNAPSHOT.with(|s| *s.borrow_mut() = gpad_just_released);
    GAMEPAD_STICKS_SNAPSHOT.with(|s| *s.borrow_mut() = gamepad_sticks);
    if let Some(handles) = world.get_resource::<SoundHandles>() {
        use kira::sound::PlaybackState;
        let mut states = std::collections::HashMap::new();
        let mut positions = std::collections::HashMap::new();
        for (id, handle) in &handles.0 {
            let state = match handle.state() {
                PlaybackState::Playing => "playing",
                PlaybackState::Pausing => "pausing",
                PlaybackState::Paused => "paused",
                PlaybackState::WaitingToResume => "waiting_to_resume",
                PlaybackState::Resuming => "resuming",
                PlaybackState::Stopping => "stopping",
                PlaybackState::Stopped => "stopped",
            };
            states.insert(*id, state.to_string());
            positions.insert(*id, handle.position());
        }
        SOUND_STATE_SNAPSHOT.with(|s| *s.borrow_mut() = states);
        SOUND_POSITION_SNAPSHOT.with(|s| *s.borrow_mut() = positions);
    }
    {
        use bsengine_core::AnimationPlayer;
        let mut anim_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &AnimationPlayer)>();
        for (name, ap) in q.iter(world) {
            anim_map.insert(
                name.0.clone(),
                (ap.clip.clone(), ap.time, ap.speed, ap.looping, ap.playing),
            );
        }
        ANIMATION_SNAPSHOT.with(|s| *s.borrow_mut() = anim_map);
    }
    {
        use bsengine_core::AnimationStateMachine;
        let mut asm_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &AnimationStateMachine)>();
        for (name, asm) in q.iter(world) {
            asm_map.insert(name.0.clone(), asm.current_state.clone());
        }
        ASM_STATE_SNAPSHOT.with(|s| *s.borrow_mut() = asm_map);
    }
    {
        use bsengine_core::Lifetime;
        let mut lifetime_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Lifetime)>();
        for (name, lt) in q.iter(world) {
            lifetime_map.insert(name.0.clone(), lt.remaining);
        }
        LIFETIME_SNAPSHOT.with(|s| *s.borrow_mut() = lifetime_map);
    }
    {
        use bsengine_core::Shield;
        let mut shield_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Shield)>();
        for (name, sh) in q.iter(world) {
            shield_map.insert(name.0.clone(), (sh.current, sh.max));
        }
        SHIELD_SNAPSHOT.with(|s| *s.borrow_mut() = shield_map);
    }
    {
        use bsengine_core::Timer;
        let mut timer_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Timer)>();
        for (name, t) in q.iter(world) {
            timer_map.insert(
                name.0.clone(),
                (
                    t.elapsed(),
                    t.duration(),
                    t.fraction(),
                    t.is_finished(),
                    t.just_finished(),
                ),
            );
        }
        TIMER_SNAPSHOT.with(|s| *s.borrow_mut() = timer_map);
    }
    {
        use bsengine_core::{NavAgentState, NavMeshAgent};
        let mut nav_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &NavMeshAgent)>();
        for (name, a) in q.iter(world) {
            let state_u8 = match a.state {
                NavAgentState::Idle => 0u8,
                NavAgentState::Moving => 1u8,
                NavAgentState::Arrived => 2u8,
                NavAgentState::NoPath => 3u8,
            };
            nav_map.insert(
                name.0.clone(),
                (
                    a.speed,
                    a.angular_speed,
                    a.stopping_distance,
                    state_u8,
                    a.enabled,
                ),
            );
        }
        NAV_SNAPSHOT.with(|s| *s.borrow_mut() = nav_map);
    }
    {
        use bsengine_core::Bloom;
        let mut bl_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Bloom)>();
        for (_, name, b) in q.iter(world) {
            bl_map.insert(
                name.0.clone(),
                (b.intensity, b.threshold, b.radius, b.softness, b.enabled),
            );
        }
        BLOOM_SNAPSHOT.with(|s| *s.borrow_mut() = bl_map);
    }
    {
        use bsengine_core::AmbientOcclusion;
        let mut ao_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &AmbientOcclusion)>();
        for (_, name, ao) in q.iter(world) {
            ao_map.insert(
                name.0.clone(),
                (
                    ao.radius,
                    ao.bias,
                    ao.intensity,
                    ao.sample_count,
                    ao.enabled,
                ),
            );
        }
        AMBIENT_OCCLUSION_SNAPSHOT.with(|s| *s.borrow_mut() = ao_map);
    }
    {
        use bsengine_core::{ToneMap, ToneMappingMode};
        let mut tm_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &ToneMap)>();
        for (_, name, tm) in q.iter(world) {
            let mode_u32 = match tm.mode {
                ToneMappingMode::None => 0u32,
                ToneMappingMode::Reinhard => 1,
                ToneMappingMode::ReinhardLuminance => 2,
                ToneMappingMode::Aces => 3,
                ToneMappingMode::Filmic => 4,
            };
            tm_map.insert(name.0.clone(), (mode_u32, tm.exposure, tm.enabled));
        }
        TONE_MAP_SNAPSHOT.with(|s| *s.borrow_mut() = tm_map);
    }
    {
        use bsengine_core::{EasingFn, RepeatMode, Tween, TweenTarget};
        let mut tw_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Tween)>();
        for (_, name, tw) in q.iter(world) {
            let target_type = match tw.target {
                TweenTarget::Translation { .. } => 0u32,
                TweenTarget::Rotation { .. } => 1u32,
                TweenTarget::Scale { .. } => 2u32,
            };
            let easing_u32 = match tw.easing {
                EasingFn::Linear => 0u32,
                EasingFn::EaseInQuad => 1u32,
                EasingFn::EaseOutQuad => 2u32,
                EasingFn::EaseInOutQuad => 3u32,
            };
            let repeat_u32 = match tw.repeat {
                RepeatMode::Once => 0u32,
                RepeatMode::Loop => 1u32,
                RepeatMode::PingPong => 2u32,
            };
            tw_map.insert(
                name.0.clone(),
                (
                    target_type,
                    tw.duration,
                    easing_u32,
                    repeat_u32,
                    tw.elapsed,
                    tw.finished,
                    tw.reversed,
                ),
            );
        }
        TWEEN_SNAPSHOT.with(|s| *s.borrow_mut() = tw_map);
    }
    {
        use bsengine_core::Follow;
        let mut f_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Follow)>();
        for (_, name, f) in q.iter(world) {
            let target_name = ENTITY_NAME_MAP.with(|m| {
                m.borrow()
                    .get(&f.target.to_bits())
                    .cloned()
                    .unwrap_or_default()
            });
            f_map.insert(
                name.0.clone(),
                (target_name, f.offset.x, f.offset.y, f.offset.z, f.speed),
            );
        }
        FOLLOW_SNAPSHOT.with(|s| *s.borrow_mut() = f_map);
    }
    {
        use bsengine_core::LookAt;
        let mut la_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &LookAt)>();
        for (_, name, la) in q.iter(world) {
            let target_name = ENTITY_NAME_MAP.with(|m| {
                m.borrow()
                    .get(&la.target.to_bits())
                    .cloned()
                    .unwrap_or_default()
            });
            la_map.insert(name.0.clone(), (target_name, la.up.x, la.up.y, la.up.z));
        }
        LOOK_AT_SNAPSHOT.with(|s| *s.borrow_mut() = la_map);
    }
    {
        use bsengine_core::{NetworkAuthority, NetworkId};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &NetworkId)>();
        for (_, name, nid) in q.iter(world) {
            let (auth_kind, peer_id_str) = match nid.authority {
                NetworkAuthority::Server => (0u32, String::new()),
                NetworkAuthority::Client { peer_id } => (1u32, peer_id.to_string()),
                NetworkAuthority::Local => (2u32, String::new()),
            };
            map.insert(name.0.clone(), (nid.id.to_string(), auth_kind, peer_id_str));
        }
        NETWORK_ID_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        let (is_server, is_connected, my_peer_id, peer_count) =
            if let Some(session) = world.get_resource::<NetworkSession>() {
                (
                    session.is_server(),
                    session.connected,
                    session.my_peer_id,
                    session.peer_count(),
                )
            } else {
                (false, false, 0, 0)
            };
        NETWORK_STATE_SNAPSHOT.with(|s| {
            *s.borrow_mut() = (is_server, is_connected, my_peer_id, peer_count);
        });
    }
    COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    (scripted, collision_json)
}

#[cfg(test)]
mod tests {
    use super::{ScriptRuntimeResource, ScriptingPlugin};
    use bsengine_app::new_app;

    #[test]
    fn scripting_plugin_registers_runtime() {
        let mut app = new_app();
        app.add_plugins(ScriptingPlugin::default());
        assert!(app
            .world()
            .get_non_send_resource::<ScriptRuntimeResource>()
            .is_some());
    }

    #[test]
    fn scripting_plugin_runtime_can_eval() {
        let mut app = new_app();
        app.add_plugins(ScriptingPlugin::default());

        let result = app
            .world_mut()
            .get_non_send_resource_mut::<ScriptRuntimeResource>()
            .expect("ScriptRuntimeResource not found")
            .0
            .eval("40 + 2");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "42");
    }
}

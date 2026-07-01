use std::collections::{HashMap, HashSet};

use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_ecs::prelude::*;
use bsengine_audio::AudioWorld;
use bsengine_core::{
    GlobalTransform, HudTexts, Material, Parent, ScreenSize, SkyboxPath, Transform, Visible,
};
use bsengine_input::{GamepadButton, GamepadSticks, Input, KeyCode, MouseButton, MouseState};
use bsengine_physics::CollisionEvent;
use bsengine_physics::PhysicsWorld;
use bsengine_scene::{Name, PendingSceneLoad, Primitive, PrimitiveMesh, ScriptPath};
use glam::{Quat, Vec3};

use crate::ops::{
    ScriptCommand, SpawnParams, BOOTSTRAP_JS, COLLISION_SNAPSHOT, COMMAND_BUFFER,
    ENTITY_NAMES_SNAPSHOT, ENTITY_NAME_MAP, GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT,
    GAMEPAD_BUTTON_JUST_RELEASED_SNAPSHOT, GAMEPAD_BUTTON_SNAPSHOT, GAMEPAD_STICKS_SNAPSHOT,
    KEY_JUST_PRESSED_SNAPSHOT, KEY_JUST_RELEASED_SNAPSHOT, KEY_SNAPSHOT, MOUSE_DELTA_SNAPSHOT,
    MOUSE_JUST_PRESSED_SNAPSHOT, MOUSE_JUST_RELEASED_SNAPSHOT, MOUSE_POS_SNAPSHOT,
    MOUSE_PRESSED_SNAPSHOT, PHYSICS_WORLD_PTR, SCREEN_SIZE_SNAPSHOT, TIME_DELTA_SNAPSHOT,
    TIME_ELAPSED_SNAPSHOT, TRANSFORM_SNAPSHOT, VISIBLE_SNAPSHOT,
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
    {
        let mut q = world.query::<&Script>();
        if q.iter(world).next().is_none() {
            return;
        }
    }

    let transform_snapshot: HashMap<String, (Vec3, Quat, Vec3)> = {
        let mut q = world.query::<(&Name, &Transform)>();
        q.iter(world)
            .map(|(n, t)| (n.0.clone(), (t.translation, t.rotation, t.scale)))
            .collect()
    };

    let visible_snapshot: HashMap<String, bool> = {
        let mut q = world.query::<(&Name, &Visible)>();
        q.iter(world)
            .map(|(n, v)| (n.0.clone(), v.is_visible))
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
    VISIBLE_SNAPSHOT.with(|s| *s.borrow_mut() = visible_snapshot);

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
    ENTITY_NAME_MAP.with(|m| *m.borrow_mut() = entity_name_map);
    PHYSICS_WORLD_PTR.with(|p| *p.borrow_mut() = physics_ptr);
    GAMEPAD_BUTTON_SNAPSHOT.with(|s| *s.borrow_mut() = gpad_pressed);
    GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = gpad_just_pressed);
    GAMEPAD_BUTTON_JUST_RELEASED_SNAPSHOT.with(|s| *s.borrow_mut() = gpad_just_released);
    GAMEPAD_STICKS_SNAPSHOT.with(|s| *s.borrow_mut() = gamepad_sticks);
    COMMAND_BUFFER.with(|c| c.borrow_mut().clear());

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
            ScriptCommand::SetScale { name, sx, sy, sz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale = Vec3::new(sx, sy, sz);
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
                        mat.emissive = Vec3::new(r, g, b);
                    } else {
                        world.entity_mut(e).insert(Material {
                            emissive: Vec3::new(r, g, b),
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
                        mat.base_color = Vec3::new(r, g, b);
                    } else {
                        world.entity_mut(e).insert(Material {
                            base_color: Vec3::new(r, g, b),
                            ..Default::default()
                        });
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
                        let _ = handle.stop(Tween::default());
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
            base_color: params.color.map(Vec3::from).unwrap_or(Vec3::ONE),
            emissive: params.emissive.map(Vec3::from).unwrap_or(Vec3::ZERO),
            ..Default::default()
        });
    }

    if let Some(script) = params.script {
        cmd.insert(ScriptPath(script));
    }
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

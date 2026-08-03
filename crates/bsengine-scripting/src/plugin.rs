use std::collections::{HashMap, HashSet};

use bevy_app::{App, AppExit, Plugin, PostStartup, Update};
use bevy_ecs::prelude::*;
use bsengine_audio::AudioWorld;
use bsengine_core::{
    resolve_project_path, CursorConfig, CustomShader, EditorPlayState, GlobalTransform, HudTexts,
    InspectorState, Material, Parent, ProjectDir, ScreenSize, SkyboxPath, Transform, UiState,
    UiWidget, Visible,
};
use bsengine_input::{GamepadButton, GamepadSticks, Input, KeyCode, MouseButton, MouseState};
use bsengine_network::NetworkSession;
use bsengine_physics::CollisionEvent;
use bsengine_physics::PhysicsWorld;
use bsengine_scene::{Name, PendingSceneLoad, Primitive, PrimitiveMesh, ScriptPath};
use glam::{EulerRot, Quat, Vec3};

use crate::ops::{
    render_asset_status, ScriptCommand, SpawnParams, AMBIENT_OCCLUSION_SNAPSHOT,
    ANGULAR_DAMPING_SNAPSHOT, ANGULAR_VELOCITY_SNAPSHOT, ANIMATION_SNAPSHOT, ASM_STATE_SNAPSHOT,
    ASSET_STATUS_SNAPSHOT, BLOOM_SNAPSHOT, BODY_TYPE_SNAPSHOT, BOOTSTRAP_JS, CHILDREN_SNAPSHOT,
    COLLIDER_SENSOR_SNAPSHOT, COLLISION_SNAPSHOT, COMMAND_BUFFER, ENTITY_NAMES_SNAPSHOT,
    ENTITY_NAME_MAP, FOLLOW_SNAPSHOT, FRICTION_SNAPSHOT, GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT,
    GAMEPAD_BUTTON_JUST_RELEASED_SNAPSHOT, GAMEPAD_BUTTON_SNAPSHOT, GAMEPAD_STICKS_SNAPSHOT,
    GRAVITY_SCALE_SNAPSHOT, GRAVITY_SNAPSHOT, KEY_JUST_PRESSED_SNAPSHOT,
    KEY_JUST_RELEASED_SNAPSHOT, KEY_SNAPSHOT, LIFETIME_SNAPSHOT, LINEAR_DAMPING_SNAPSHOT,
    LOOK_AT_SNAPSHOT, MASS_SNAPSHOT, MATERIAL_COLOR_SNAPSHOT, MATERIAL_EMISSIVE_SNAPSHOT,
    MATERIAL_METALLIC_SNAPSHOT, MATERIAL_ROUGHNESS_SNAPSHOT, MOUSE_DELTA_SNAPSHOT,
    MOUSE_JUST_PRESSED_SNAPSHOT, MOUSE_JUST_RELEASED_SNAPSHOT, MOUSE_POS_SNAPSHOT,
    MOUSE_PRESSED_SNAPSHOT, NAV_SNAPSHOT, NETWORK_ID_SNAPSHOT, NETWORK_STATE_SNAPSHOT,
    PARENT_SNAPSHOT, PAUSED_SNAPSHOT, PHYSICS_WORLD_PTR, RESTITUTION_SNAPSHOT, SAVE_DATA_SNAPSHOT,
    SCREEN_SIZE_SNAPSHOT, SHIELD_SNAPSHOT, SLEEP_SNAPSHOT, SOUND_POSITION_SNAPSHOT,
    SOUND_STATE_SNAPSHOT, TIMER_SNAPSHOT, TIME_DELTA_SNAPSHOT, TIME_ELAPSED_SNAPSHOT,
    TONE_MAP_SNAPSHOT, TRANSFORM_SNAPSHOT, TWEEN_SNAPSHOT, UI_CLICKED_SNAPSHOT, VELOCITY_SNAPSHOT,
    VISIBLE_SNAPSHOT, WORLD_TRANSFORM_SNAPSHOT,
};
use crate::runtime::ScriptRuntime;

/// Loaded JS source for a scripted entity.
#[derive(Component)]
pub struct Script {
    /// The full text of the entity's script file.
    pub source: String,
}

/// Non-Send wrapper around the entity's V8 isolate; stored as a non-send
/// resource via `insert_non_send_resource` since `JsRuntime` isn't `Send`/`Sync`.
pub struct ScriptRuntimeResource(pub ScriptRuntime);

/// Stores kira sound handles by script-assigned id for stopSound support.
#[derive(Resource, Default)]
pub struct SoundHandles(pub HashMap<u32, kira::sound::static_sound::StaticSoundHandle>);

/// What the audio consumer knows about one sound path.
#[derive(Debug)]
enum SoundLoad {
    /// Requested; waiting for `Assets<AudioSourceAsset>` to have it.
    Loading(bevy_asset::Handle<bsengine_audio::AudioSourceAsset>),
    /// Decoded and resident. See [`SoundLoads`] for why the handle is kept
    /// rather than dropped once the play that asked for it has started.
    Ready(bevy_asset::Handle<bsengine_audio::AudioSourceAsset>),
    /// The load failed. Kept so the path is never re-requested --
    /// re-requesting a failed path resets it to `Loading` and starts the load
    /// over (`bevy_asset` 0.14.2, `server/info.rs:212-221`), which is why a
    /// re-requesting poll loop can never see the failure.
    GaveUp,
}

/// Every sound path `playSound` has ever asked for, keyed by resolved path.
///
/// Gives audio the request-once/retain-the-handle/poll-the-retained-handle
/// shape the glTF, shader and skybox consumers already have (see
/// `bsengine_render::plugin`'s `PendingShaders` for the closest analogue).
/// Keyed by path rather than by play id because a path is what gets loaded,
/// while several concurrent plays -- each with its own id, volume and loop
/// flag -- can be waiting on the same one.
///
/// A `Ready` path keeps its handle, not just an in-flight one, and that is a
/// deliberate trade rather than an oversight: `Assets::<A>::track_assets`
/// frees an asset the frame after its last strong handle drops, so releasing
/// it here would make *every* repeat play a fresh disk read and decode,
/// several frames late -- not just the first. The cost is that each distinct
/// sound path a game plays stays resident for the process lifetime. For a
/// game engine that is the right way round: SFX are small, and latency on a
/// repeated trigger is the thing players actually notice. Unity and Unreal
/// both keep loaded clips resident by default for the same reason.
#[derive(bevy_ecs::prelude::Resource, Default)]
struct SoundLoads(HashMap<String, SoundLoad>);

/// One `playSound` waiting for its sound to finish decoding.
///
/// `playSound` is fire-and-forget from the script's side, so the request has
/// to outlive the frame it was made on; everything needed to start the play
/// later is captured here at request time.
#[derive(Debug)]
struct PendingSound {
    /// Script-assigned id — what `stopSound`/`pauseSound` name this play by.
    id: u32,
    /// Resolved path this play is waiting on; the key into [`SoundLoads`].
    path: String,
    /// Linear volume captured at request time.
    volume: f32,
    /// Whether the play loops.
    loop_: bool,
    /// A `pauseSound` arrived before the sound started, so there was no kira
    /// handle to pause; pause it the instant it does start.
    paused: bool,
}

/// Plays requested before their sound finished loading.
///
/// Only `stopSound`, `pauseSound` and `resumeSound` reach entries here;
/// `setSoundVolume`, `setSoundPanning`, `setSoundPlaybackRate` and `seekSound`
/// still act on playing sounds only and silently no-op on a queued one. That
/// is deliberate, and the line is what the command does when it is lost: those
/// four *tune* a sound, so an ignored one yields a wrong-sounding sound, which
/// is also what they have always done for an id that is not playing. A stop or
/// a pause instead *suppresses* a sound, so an ignored one actively produces
/// the sound it was meant to prevent — `playSound(); pauseSound(id)` in one
/// `onUpdate` would otherwise be audible.
#[derive(bevy_ecs::prelude::Resource, Default)]
struct PendingSounds(Vec<PendingSound>);

/// Where `Bsengine.getTime()` / `getDeltaTime()` get their numbers.
///
/// Rapier always steps a fixed 1/60s of simulation per frame, no matter how
/// long the frame actually took. In a real window that roughly matches the
/// wall clock, so script-driven and physics-driven motion advance together.
/// Headless, frames run as fast as the CPU allows — under a millisecond —
/// while physics still advances 1/60s, so the two clocks diverge by more than
/// an order of magnitude, and by a different factor on every machine. Anything
/// whose correctness depends on both (a ball rolling past an obstacle that
/// swings on `getTime()`) then behaves differently per machine, which is how
/// `tilt-run`'s level-5 recordings passed locally and failed on CI.
#[derive(bevy_ecs::prelude::Resource)]
pub enum ScriptTimingState {
    /// Real elapsed time — what a player actually experiences.
    Wall {
        /// When the app started; `getTime()` counts from here.
        startup: std::time::Instant,
        /// Previous frame's instant, subtracted to get `getDeltaTime()`.
        last_frame: std::time::Instant,
    },
    /// A fixed step per frame, so N frames always means the same amount of
    /// game time regardless of how fast the machine ran them.
    Fixed {
        /// Seconds of game time each frame advances.
        dt: f32,
        /// Game time accumulated so far; what `getTime()` reports.
        elapsed: f32,
    },
}

impl ScriptTimingState {
    /// Wall-clock timing, for a real running game.
    pub fn wall_clock() -> Self {
        let now = std::time::Instant::now();
        Self::Wall {
            startup: now,
            last_frame: now,
        }
    }

    /// Fixed-step timing, for headless replay.
    ///
    /// Pass the physics timestep (Rapier's default, 1/60) so script-driven and
    /// physics-driven motion stay on the same clock; picking anything else
    /// reintroduces the drift this exists to remove.
    pub fn fixed(dt: f32) -> Self {
        Self::Fixed { dt, elapsed: 0.0 }
    }

    /// Advances one frame, returning `(elapsed_seconds, delta_seconds)`.
    fn tick(&mut self) -> (f32, f32) {
        match self {
            Self::Wall {
                startup,
                last_frame,
            } => {
                let now = std::time::Instant::now();
                let elapsed = now.duration_since(*startup).as_secs_f32();
                let delta = now.duration_since(*last_frame).as_secs_f32();
                *last_frame = now;
                (elapsed, delta)
            }
            Self::Fixed { dt, elapsed } => {
                *elapsed += *dt;
                (*elapsed, *dt)
            }
        }
    }
}

/// Bevy plugin that wires up the JS scripting runtime: loads scripts, runs
/// them each frame, and exposes ECS state to them via the `bsengine_ops` extension.
pub struct ScriptingPlugin {
    /// Root directory used to resolve relative script paths.
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
        use bevy_asset::AssetApp;
        app.init_asset::<bsengine_audio::AudioSourceAsset>()
            .register_asset_loader(bsengine_audio::AudioSourceLoader);

        app.insert_resource(ProjectDir(self.project_dir.clone()));
        app.insert_resource(HudTexts::default());
        app.insert_resource(SoundHandles::default());
        app.init_resource::<PendingSounds>();
        app.init_resource::<SoundLoads>();
        app.insert_non_send_resource(ScriptRuntimeResource(ScriptRuntime::new_with_ops()));
        // Wall clock by default. `bsengine-runtime --test` overwrites this
        // with `ScriptTimingState::fixed` so replays are reproducible.
        app.insert_resource(ScriptTimingState::wall_clock());
        // Register CollisionEvent so EventReader works even without PhysicsPlugin
        app.add_event::<CollisionEvent>();
        app.add_systems(PostStartup, load_scripts);
        app.add_systems(
            Update,
            (capture_collision_events, run_scripts, start_pending_sounds)
                .chain()
                // Pins the frame `Bsengine.getAssetStatus` reports from.
                // `collect_asset_statuses` also runs in `Update`, so without
                // an explicit edge the two are unordered and the answer would
                // lag by zero or one frame depending on how the scheduler
                // happened to sort them on a given run — a difference a
                // replay can see. With it, a load that resolves during frame
                // N is what frame N's scripts read.
                //
                // A no-op in an app that never adds `AssetStatusPlugin`: the
                // constraint names a system type with no instance in the
                // schedule, so there is nothing to order against.
                .after(bsengine_asset::status::collect_asset_statuses),
        );
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

/// Read the JS source for every entity with a `ScriptPath` (resolved against
/// `ProjectDir`) and attach it as a `Script` component.
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

/// Canonical key-name table shared by the scripting snapshot (`Bsengine.isKeyPressed`)
/// and the headless test runtime's `press_key`/`release_key` commands.
pub const KEY_MAPPINGS: &[(KeyCode, &str)] = &[
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
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name, &mut Transform)>();
                    q.iter_mut(world).find_map(|(e, n, mut t)| {
                        (n.0 == name).then(|| {
                            t.translation = Vec3::new(x, y, z).into();
                            e
                        })
                    })
                };
                // Also teleport the actual Rapier body — for a Dynamic body,
                // Transform alone gets overwritten from the physics
                // simulation next frame (see PhysicsWorld::set_translation).
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_translation(e, Vec3::new(x, y, z));
                }
            }
            ScriptCommand::SetRotation {
                name,
                rx,
                ry,
                rz,
                rw,
            } => {
                let rot = Quat::from_xyzw(rx, ry, rz, rw);
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name, &mut Transform)>();
                    q.iter_mut(world).find_map(|(e, n, mut t)| {
                        (n.0 == name).then(|| {
                            t.rotation = rot.into();
                            e
                        })
                    })
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_rotation(e, rot);
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
                        )
                        .into();
                        break;
                    }
                }
            }
            ScriptCommand::SetScale { name, sx, sy, sz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale = Vec3::new(sx, sy, sz).into();
                        break;
                    }
                }
            }
            ScriptCommand::AddPosition { name, dx, dy, dz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation.0 += Vec3::new(dx, dy, dz);
                        break;
                    }
                }
            }
            ScriptCommand::AddPositionLocal { name, dx, dy, dz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let rot = t.rotation;
                        t.translation.0 += rot.0.mul_vec3(Vec3::new(dx, dy, dz));
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
                        t.rotation = rotation.into();
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
            ScriptCommand::Quit => {
                world.send_event(AppExit::Success);
            }
            ScriptCommand::MoveEntity { name, dx, dy, dz } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Transform>(e) {
                        t.translation.x += dx;
                        t.translation.y += dy;
                        t.translation.z += dz;
                    }
                }
            }
            ScriptCommand::SetSaveField { name, key, value } => {
                use bsengine_core::SaveData;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sd) = world.get_mut::<SaveData>(e) {
                        sd.set(key, value.into_bytes());
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
                    let resolved = resolve_project_path(world.get_resource::<ProjectDir>(), &path);
                    world.entity_mut(e).insert(CustomShader { path: resolved });
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
                // What `SoundLoads` already knows decides everything below, so
                // it is consulted *before* anything is requested. Re-calling
                // `load()` for a path whose load has since failed resets it to
                // `Loading` and respawns the filesystem task; because `Failed`
                // is set in `PreUpdate` and this runs in `Update`, a script
                // calling `playSound` on a missing path every frame would
                // queue one entry and spawn one task per frame and never once
                // observe the failure. Re-calling it for a path that already
                // resolved would instead throw away the decode `SoundLoads`
                // exists to keep, making every repeat play a fresh disk read.
                let known = world
                    .get_resource::<SoundLoads>()
                    .and_then(|loads| loads.0.get(&full_path));
                if matches!(known, Some(SoundLoad::GaveUp)) {
                    // Dropped now rather than queued: this play can never
                    // start, and a queue entry that can never resolve is
                    // exactly the unbounded growth this map prevents.
                    tracing::warn!("[audio] not playing {full_path}: its load already failed");
                    continue;
                }
                let already_requested = known.is_some();

                if !already_requested {
                    let asset_server = world.get_resource::<bevy_asset::AssetServer>().cloned();
                    let requested = match asset_server {
                        Some(asset_server) => world
                            .get_resource_mut::<bevy_asset::Assets<bsengine_audio::AudioSourceAsset>>()
                            .map(|mut assets| {
                                bsengine_asset::load(
                                    bsengine_asset::LoadMode::Async,
                                    &asset_server,
                                    &mut assets,
                                    &full_path,
                                    bsengine_audio::load_audio_source,
                                )
                            }),
                        None => None,
                    };
                    let load = match requested {
                        Some(Ok(handle)) => SoundLoad::Loading(handle),
                        Some(Err(e)) => {
                            // Unreachable: `LoadMode::Async` is infallible.
                            // Present only because the shared `load()`
                            // signature returns `Result` for `Sync` callers.
                            tracing::warn!("[audio] failed to request {full_path}: {e}");
                            SoundLoad::GaveUp
                        }
                        None => {
                            tracing::warn!(
                                "[audio] Assets<AudioSourceAsset> resource missing (AssetPlugin not registered?)"
                            );
                            SoundLoad::GaveUp
                        }
                    };
                    let gave_up = matches!(load, SoundLoad::GaveUp);
                    if let Some(mut loads) = world.get_resource_mut::<SoundLoads>() {
                        loads.0.insert(full_path.clone(), load);
                    }
                    if gave_up {
                        continue;
                    }
                }

                // Queued rather than played outright: the sound may not be
                // decoded yet, and `playSound` is fire-and-forget from the
                // script's side, so the request has to outlive this frame.
                // `start_pending_sounds` is chained immediately after
                // `run_scripts`, so a play on a path that already resolved
                // still starts on this very frame — the queue costs it
                // nothing.
                if let Some(mut pending) = world.get_resource_mut::<PendingSounds>() {
                    pending.0.push(PendingSound {
                        id,
                        path: full_path,
                        volume,
                        loop_,
                        paused: false,
                    });
                }
            }
            ScriptCommand::StopSound { id } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(mut handle) = handles.0.remove(&id) {
                        use kira::Tween;
                        handle.stop(Tween::default());
                    }
                }
                // The queue is consulted too because a stop can arrive before
                // the async load resolves, while the id is still only in
                // `PendingSounds` and not yet in `SoundHandles`. Without this
                // the stop would find nothing, and the sound would start once
                // the load finished — the opposite of what was asked.
                if let Some(mut pending) = world.get_resource_mut::<PendingSounds>() {
                    pending.0.retain(|entry| entry.id != id);
                }
            }
            ScriptCommand::PauseSound { id } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(handle) = handles.0.get_mut(&id) {
                        use kira::Tween;
                        handle.pause(Tween::default());
                    }
                }
                // The queue is consulted for the same reason `stopSound`
                // consults it: a pause can arrive before the async load
                // resolves, while the id is still only in `PendingSounds` and
                // not yet in `SoundHandles`. Without this the pause would find
                // nothing, and the sound would start audibly once the load
                // finished — the opposite of what was asked. Recorded rather
                // than acted on because there is nothing to act on yet;
                // `start_pending_sounds` pauses the sound the moment it starts.
                if let Some(mut pending) = world.get_resource_mut::<PendingSounds>() {
                    for entry in pending.0.iter_mut().filter(|entry| entry.id == id) {
                        entry.paused = true;
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
                // Clears a pause recorded above, so `playSound(); pauseSound();
                // resumeSound()` in one `onUpdate` still plays.
                if let Some(mut pending) = world.get_resource_mut::<PendingSounds>() {
                    for entry in pending.0.iter_mut().filter(|entry| entry.id == id) {
                        entry.paused = false;
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
            ScriptCommand::SetUiProgressBar {
                id,
                x,
                y,
                width,
                height,
                fraction,
            } => {
                if let Some(mut ui) = world.get_resource_mut::<UiState>() {
                    ui.set_widget(UiWidget::ProgressBar {
                        id,
                        x,
                        y,
                        width,
                        height,
                        fraction,
                    });
                }
            }
            ScriptCommand::SetPaused { paused } => {
                world.insert_resource(bsengine_core::PauseState { paused });
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
                // `path` is project-relative (e.g. "assets/scenes/level2.ron",
                // matching every other path convention in this engine —
                // ScriptPath, entry_scene, ...), but handle_scene_load reads
                // it directly with std::fs, so it needs the same project_dir
                // prefix ScenePlugin::from_file/InspectorState.current_scene_path
                // already carry. Without this, loadScene only works by
                // accident when the process's CWD happens to equal
                // project_dir.
                let full_path = resolve_project_path(world.get_resource::<ProjectDir>(), &path);
                world.insert_resource(PendingSceneLoad { path: full_path });
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
                let full_path = resolve_project_path(world.get_resource::<ProjectDir>(), &path);
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
            ScriptCommand::ResetForces { name } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.reset_forces(e);
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
                        t.rotation = (t.rotation.0 * delta).normalize().into();
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
                            t.rotation = (t.rotation.0 * delta).normalize().into();
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
                        t.rotation = (t.rotation.0 * delta).normalize().into();
                        break;
                    }
                }
            }
            ScriptCommand::AddRotationEulerX { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let delta = Quat::from_euler(EulerRot::XYZ, deg.to_radians(), 0.0, 0.0);
                        t.rotation = (t.rotation.0 * delta).normalize().into();
                        break;
                    }
                }
            }
            ScriptCommand::AddRotationEulerY { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let delta = Quat::from_euler(EulerRot::XYZ, 0.0, deg.to_radians(), 0.0);
                        t.rotation = (t.rotation.0 * delta).normalize().into();
                        break;
                    }
                }
            }
            ScriptCommand::AddRotationEulerZ { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let delta = Quat::from_euler(EulerRot::XYZ, 0.0, 0.0, deg.to_radians());
                        t.rotation = (t.rotation.0 * delta).normalize().into();
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
                        t.scale.0 += Vec3::new(sx, sy, sz);
                        break;
                    }
                }
            }
            ScriptCommand::SetRotationEulerX { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let (_, y, z) = t.rotation.0.to_euler(EulerRot::XYZ);
                        t.rotation = Quat::from_euler(EulerRot::XYZ, deg.to_radians(), y, z).into();
                        break;
                    }
                }
            }
            ScriptCommand::SetRotationEulerY { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let (x, _, z) = t.rotation.0.to_euler(EulerRot::XYZ);
                        t.rotation = Quat::from_euler(EulerRot::XYZ, x, deg.to_radians(), z).into();
                        break;
                    }
                }
            }
            ScriptCommand::SetRotationEulerZ { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let (x, y, _) = t.rotation.0.to_euler(EulerRot::XYZ);
                        t.rotation = Quat::from_euler(EulerRot::XYZ, x, y, deg.to_radians()).into();
                        break;
                    }
                }
            }
            ScriptCommand::MultiplyScale { name, sx, sy, sz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.0 *= Vec3::new(sx, sy, sz);
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

/// Starts any queued play whose sound has finished decoding.
///
/// Split from the `PlaySound` command handler because an async load is not
/// ready on the frame it is asked for. Entries stay queued until the handle
/// [`SoundLoads`] holds for their path resolves; every entry on a path whose
/// load failed is dropped, and the path is marked [`SoundLoad::GaveUp`] so
/// later plays on it are refused at the command handler instead of piling up
/// here.
///
/// Chained immediately after `run_scripts` (see [`ScriptingPlugin::build`]),
/// so a play queued this frame on a path that already resolved starts this
/// frame rather than the next one.
fn start_pending_sounds(world: &mut World) {
    // Taken rather than cloned: `PendingSound` is not `Clone`, and nothing
    // else can push while this exclusive system runs. Every entry is either
    // started, dropped, or put back below.
    let entries = match world.get_resource_mut::<PendingSounds>() {
        Some(mut pending) if !pending.0.is_empty() => std::mem::take(&mut pending.0),
        _ => return,
    };

    // Decide each entry's fate first, so the borrows of PendingSounds,
    // SoundLoads and Assets end before AudioWorld/SoundHandles are borrowed
    // mutably below.
    let mut still_pending = Vec::new();
    let mut ready = Vec::new();
    let mut resolved_paths = HashSet::new();
    // Keyed by path so N plays queued on one bad path warn once, not N times.
    let mut failed: HashMap<String, String> = HashMap::new();
    {
        let assets = world.resource::<bevy_asset::Assets<bsengine_audio::AudioSourceAsset>>();
        let asset_server = world.resource::<bevy_asset::AssetServer>();
        let loads = world.resource::<SoundLoads>();
        for entry in entries {
            // The handle is read from `SoundLoads`, never re-requested here:
            // `AssetServer::load` on a path already in `Failed` restarts it,
            // which would make the failure below unobservable.
            let handle = match loads.0.get(&entry.path) {
                Some(SoundLoad::Loading(handle) | SoundLoad::Ready(handle)) => handle,
                // Unreachable: `PlaySound` records the path before queueing,
                // and refuses to queue at all once it is `GaveUp`.
                Some(SoundLoad::GaveUp) | None => continue,
            };
            match assets.get(handle) {
                Some(src) => {
                    resolved_paths.insert(entry.path.clone());
                    ready.push((entry, src.0.clone()));
                }
                None => match asset_server.load_state(handle) {
                    bevy_asset::LoadState::Failed(e) => {
                        failed.insert(entry.path.clone(), format!("{e}"));
                    }
                    _ => still_pending.push(entry),
                },
            }
        }
    }

    for (path, e) in &failed {
        tracing::warn!("[audio] failed to load queued sound {path}: {e}");
    }
    if let Some(mut loads) = world.get_resource_mut::<SoundLoads>() {
        // `Loading` -> `Ready`: the handle stays held so the decoded sound
        // stays resident and a repeat play starts instantly (see `SoundLoads`).
        for path in resolved_paths {
            if let Some(slot) = loads.0.get_mut(&path) {
                if let SoundLoad::Loading(handle) = slot {
                    *slot = SoundLoad::Ready(handle.clone());
                }
            }
        }
        // `Loading` -> `GaveUp`: the path is never requested again, so the
        // failure stays observable and later plays on it are refused outright.
        for path in failed.into_keys() {
            loads.0.insert(path, SoundLoad::GaveUp);
        }
    }
    if let Some(mut pending) = world.get_resource_mut::<PendingSounds>() {
        still_pending.append(&mut pending.0);
        pending.0 = still_pending;
    }

    for (entry, data) in ready {
        use kira::Decibels;
        let volume_db = 20.0_f32 * entry.volume.max(1e-10_f32).log10();
        let data = data.volume(Decibels(volume_db));
        let data = if entry.loop_ {
            data.loop_region(..)
        } else {
            data
        };
        if let Some(mut audio) = world.get_resource_mut::<AudioWorld>() {
            if let Some(mut handle) = audio.play(data) {
                // A `pauseSound` that arrived while this play was still queued
                // had no kira handle to act on; apply it now, before the sound
                // is audible for a frame it was asked not to be.
                if entry.paused {
                    handle.pause(kira::Tween::default());
                }
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    handles.0.insert(entry.id, handle);
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
        translation: Vec3::new(params.x, params.y, params.z).into(),
        rotation: Quat::from_xyzw(params.rx, params.ry, params.rz, params.rw)
            .normalize()
            .into(),
        scale: Vec3::new(params.sx, params.sy, params.sz).into(),
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
            .map(|(n, t)| (n.0.clone(), (t.translation.0, t.rotation.0, t.scale.0)))
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
            timing.tick()
        } else {
            (0.0, 0.0)
        };
    TIME_ELAPSED_SNAPSHOT.with(|s| *s.borrow_mut() = elapsed_secs);
    TIME_DELTA_SNAPSHOT.with(|s| *s.borrow_mut() = delta_secs);
    let is_paused = world
        .get_resource::<bsengine_core::PauseState>()
        .map(|p| p.paused)
        .unwrap_or(false);
    PAUSED_SNAPSHOT.with(|p| *p.borrow_mut() = is_paused);
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
    {
        use kira::sound::PlaybackState;
        let mut states = std::collections::HashMap::new();
        let mut positions = std::collections::HashMap::new();
        // Queued plays first, so a real kira handle always wins if an id
        // somehow appears in both. Without this a sound that has been asked
        // for but not yet decoded reports "" — indistinguishable from one
        // that already finished, which is how a script polling
        // `getSoundState` fires its "sound over" branch several frames early.
        // No position is written: `getSoundPosition` falls back to 0.0, which
        // is honest for a sound that has not started.
        if let Some(pending) = world.get_resource::<PendingSounds>() {
            for entry in &pending.0 {
                states.insert(entry.id, "loading".to_string());
            }
        }
        if let Some(handles) = world.get_resource::<SoundHandles>() {
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
        }
        SOUND_STATE_SNAPSHOT.with(|s| *s.borrow_mut() = states);
        SOUND_POSITION_SNAPSHOT.with(|s| *s.borrow_mut() = positions);
    }
    {
        // Mirrors `AssetStatuses` wholesale for `Bsengine.getAssetStatus`,
        // which V8 calls on a thread with no `World` to consult. See
        // `ops::bsengine_get_asset_status` for what the strings mean.
        //
        // # What the refresh costs, said plainly
        //
        // One `String` clone of the path plus one rendered status `String`
        // per recorded path, into a fresh map that replaces last frame's —
        // O(distinct asset paths) allocations every frame, whether or not
        // anything changed. That is a real cost and it is deliberate:
        //
        // * The map is bounded by the number of *distinct paths the project
        //   requests*, and `AssetStatuses`' own docs cover what keeps it
        //   there: it does not grow with entity count, with elapsed time, or
        //   with retries, because the path is the key. Tens of entries for
        //   the games in this repository, not thousands.
        // * `bsengine_asset::collect_asset_statuses` already walks that same
        //   map every frame, doing an `AssetServer::get_path_id` and a
        //   `load_state` — two `RwLock` reads and two hash lookups — per
        //   entry. This adds a constant factor to a per-frame cost that is
        //   already O(recorded paths) and already more expensive per entry
        //   than a `String` clone; it does not introduce a new one.
        // * Every other snapshot in this function is built the same way, and
        //   several of them are O(entities) with a `String` clone each. This
        //   one is smaller than any of those in every project here.
        //
        // Replacing the map wholesale, rather than patching entries in place,
        // is also what keeps the mirror honest when two `App`s share a thread
        // (this workspace's own test runs are exactly that): each refresh
        // publishes only what *this* world knows, so an app can never inherit
        // another app's failure for a path it never requested — the precise
        // wrong answer this API exists to prevent.
        //
        // If a project ever does synthesise fresh asset paths at runtime, the
        // thing to fix is `AssetStatuses` itself — it would be leaking
        // `AssetInfo`s inside `bevy_asset` too — not this mirror.
        let asset_statuses: HashMap<String, String> = world
            .get_resource::<bsengine_asset::AssetStatuses>()
            .map(|statuses| {
                statuses
                    .iter()
                    .map(|(path, status)| (path.to_owned(), render_asset_status(status)))
                    .collect()
            })
            // No `AssetStatusPlugin` in this app, so nothing is known about
            // any path and `getAssetStatus` says exactly that rather than
            // guessing. An empty mirror is also what clears a previous app's
            // entries off this thread.
            .unwrap_or_default();
        ASSET_STATUS_SNAPSHOT.with(|s| *s.borrow_mut() = asset_statuses);
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
        use bsengine_core::SaveData;
        let mut save_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &SaveData)>();
        for (name, sd) in q.iter(world) {
            let fields: std::collections::HashMap<String, String> = sd
                .fields
                .iter()
                .filter_map(|(k, v)| String::from_utf8(v.clone()).ok().map(|s| (k.clone(), s)))
                .collect();
            save_map.insert(name.0.clone(), fields);
        }
        SAVE_DATA_SNAPSHOT.with(|s| *s.borrow_mut() = save_map);
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
    use super::{
        HudTexts, Name, PendingSounds, ScriptPath, ScriptRuntimeResource, ScriptingPlugin,
        SoundLoad, SoundLoads, Transform, Vec3,
    };
    use bsengine_app::new_app;

    /// Collects everything `tracing` emits on this thread into a string, so a
    /// test can assert a warning actually reached the developer rather than
    /// only that the state behind it changed. Thread-local
    /// (`tracing::subscriber::with_default`), so parallel tests can't see each
    /// other's output.
    #[derive(Clone, Default)]
    struct LogSink(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl LogSink {
        fn contents(&self) -> String {
            String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
        }
    }

    impl std::io::Write for LogSink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl tracing_subscriber::fmt::MakeWriter<'_> for LogSink {
        type Writer = Self;
        fn make_writer(&self) -> Self {
            self.clone()
        }
    }

    /// Runs `body` with every `tracing` event on this thread captured.
    fn capture_logs<T>(body: impl FnOnce() -> T) -> (T, String) {
        let sink = LogSink::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(sink.clone())
            .with_ansi(false)
            .with_max_level(tracing::Level::WARN)
            .finish();
        let out = tracing::subscriber::with_default(subscriber, body);
        let logs = sink.contents();
        (out, logs)
    }

    /// The smallest valid FLAC file this workspace knows how to make: mono,
    /// 16-bit, 8kHz, four 192-sample `CONSTANT` (silence) frames — 86 bytes.
    ///
    /// No audio file of any kind is checked into this repo, and only
    /// `.mp3`/`.flac` decode at all with this workspace's `kira` feature set
    /// (its `symphonia` deps bundle those two codecs; the WAV/OGG *container*
    /// readers are present without the PCM/Vorbis *codecs*, so even a real
    /// `.wav` would fail). These bytes are the exact output of
    /// `bsengine_audio::audio_source::tests::minimal_flac_silence()`, which is
    /// the generator of record and documents the encoding field by field;
    /// it is `#[cfg(test)]`-private to that crate, hence the copy of its
    /// result rather than a call. Nothing here asserts the bytes are correct
    /// in isolation — every test using them fails loudly if they stop
    /// decoding, since the sound then never resolves.
    const MINIMAL_FLAC_SILENCE: &[u8] = &[
        0x66, 0x4c, 0x61, 0x43, 0x80, 0x00, 0x00, 0x22, 0x00, 0xc0, 0x00, 0xc0, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x01, 0xf4, 0x00, 0xf0, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xf8, 0x10,
        0x00, 0x00, 0x28, 0x00, 0x00, 0x00, 0x11, 0x11, 0xff, 0xf8, 0x10, 0x00, 0x01, 0x2f, 0x00,
        0x00, 0x00, 0x7d, 0x69, 0xff, 0xf8, 0x10, 0x00, 0x02, 0x26, 0x00, 0x00, 0x00, 0xc9, 0xe1,
        0xff, 0xf8, 0x10, 0x00, 0x03, 0x21, 0x00, 0x00, 0x00, 0xa5, 0x99,
    ];

    #[test]
    fn scripting_plugin_registers_runtime() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin::default());
        assert!(app
            .world()
            .get_non_send_resource::<ScriptRuntimeResource>()
            .is_some());
    }

    #[test]
    fn scripting_plugin_runtime_can_eval() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
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

    #[test]
    fn set_save_field_round_trips_through_world() {
        use bsengine_core::SaveData;

        let script_path = std::env::temp_dir().join(format!(
            "bsengine_test_set_save_field_{}.js",
            std::process::id()
        ));
        std::fs::write(
            &script_path,
            "function onUpdate(name) { Bsengine.setSaveField(name, \"score\", \"99\"); }",
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: String::new(),
        });
        app.world_mut().spawn((
            Name("Hero".to_string()),
            SaveData::new(0),
            ScriptPath(script_path.to_string_lossy().to_string()),
        ));

        app.update();
        app.update();

        let world = app.world_mut();
        let mut q = world.query::<(&Name, &SaveData)>();
        let (_, save) = q
            .iter(world)
            .find(|(n, _)| n.0 == "Hero")
            .expect("Hero entity with SaveData not found");
        assert_eq!(save.get("score"), Some(b"99".as_slice()));

        let _ = std::fs::remove_file(&script_path);
    }

    // `playSound` requests each path exactly once and polls the handle
    // `SoundLoads` keeps for it. Re-requesting the path instead would reset
    // the failed load back to `Loading` and restart it (`bevy_asset` 0.14.2,
    // `server/info.rs:212-221`), so the failure could never be observed and
    // an entry would join the queue every frame forever.
    //
    // The script plays *every frame*, deliberately: that is the case the
    // path-keyed map exists for, and the case an earlier version of this test
    // avoided (it played once, because a re-requesting handler would have
    // refilled the queue and masked the drop — which is precisely the hole
    // being closed here). Playing once is the easy half and is still covered:
    // the first frame's assertion below is the once-only case.
    //
    // Driven through a real script so the actual `PlaySound` command handler
    // runs, not an imitation of it. There is no audio device here
    // (`AudioWorld` no-ops), so the queue, the map and the log are what get
    // asserted on, never audible output.
    #[test]
    fn sound_played_every_frame_on_a_missing_path_is_given_up_on() {
        let script_path = std::env::temp_dir().join(format!(
            "bsengine_test_pending_sound_{}.js",
            std::process::id()
        ));
        std::fs::write(
            &script_path,
            "function onUpdate(name) {\n\
               Bsengine.playSound(\"definitely/not/a/real/sound.wav\", { volume: 0.5 });\n\
             }",
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: String::new(),
        });
        app.world_mut().spawn((
            Name("Speaker".to_string()),
            ScriptPath(script_path.to_string_lossy().to_string()),
        ));

        // The queue is measured after every frame, not just at the end: the
        // defect being fixed grew it by one entry per frame, which a
        // start-and-end comparison alone would miss once the queue drains.
        let (peak_queued, logs) = capture_logs(|| {
            // One frame: `PostStartup` loads the script and `Update` runs
            // `onUpdate`, which reaches the real `PlaySound` handler. The load
            // cannot have failed yet — `LoadState::Failed` is set in
            // `PreUpdate` — so the request must be queued rather than resolved
            // or dropped.
            app.update();
            assert_eq!(
                app.world().resource::<PendingSounds>().0.len(),
                1,
                "playSound must queue the request instead of blocking on the decode"
            );

            let mut peak = 1;
            for _ in 0..200 {
                app.update();
                peak = peak.max(app.world().resource::<PendingSounds>().0.len());
            }
            peak
        });

        // A per-frame `playSound` can only queue entries until the load is
        // seen to fail, which takes a handful of frames at most. Without the
        // path-keyed map it queued one per frame for all 201 frames.
        assert!(
            peak_queued <= 16,
            "a script playing a missing path every frame must not grow the queue \
             without bound; the queue peaked at {peak_queued} entries over 201 frames"
        );
        assert!(
            app.world().resource::<PendingSounds>().0.is_empty(),
            "a sound that can never load must be dropped from the queue, not retried forever"
        );
        assert!(
            matches!(
                app.world()
                    .resource::<SoundLoads>()
                    .0
                    .get("definitely/not/a/real/sound.wav"),
                Some(SoundLoad::GaveUp)
            ),
            "the failed path must be remembered as given-up on, or the next \
             playSound restarts the load and the failure is never observed"
        );
        assert!(
            logs.contains("[audio] failed to load queued sound"),
            "the developer must be told the sound could not be loaded; captured logs were:\n{logs}"
        );

        let _ = std::fs::remove_file(&script_path);
    }

    // Repeat plays of a path that already loaded must not go back to disk.
    // `SoundLoads` is the only holder of a `Handle<AudioSourceAsset>` in the
    // workspace, so if it released the handle once a play resolved,
    // `Assets::<A>::track_assets` would free the decoded sound and *every*
    // repeat play — not just the first — would be a fresh read and decode,
    // several frames late.
    //
    // The script waits for the first play to leave the queue and then idles
    // several frames before playing again, so a released asset would really
    // have been freed by the time the repeat is asked for. Since
    // `start_pending_sounds` is chained immediately after `run_scripts`, the
    // repeat must be gone from the queue by the time that same `app.update()`
    // returns — that is what "starts on the requesting frame" means with no
    // audio device to listen to.
    #[test]
    fn a_repeat_play_reuses_the_resident_sound_and_starts_the_same_frame() {
        use bevy_asset::Assets;
        use bsengine_audio::AudioSourceAsset;

        let dir =
            std::env::temp_dir().join(format!("bsengine_test_repeat_play_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sound_path = dir.join("blip.flac");
        std::fs::write(&sound_path, MINIMAL_FLAC_SILENCE).unwrap();
        // Backslashes would be escapes inside the JS string literal below;
        // Windows accepts forward slashes just as happily.
        let sound_path_js = sound_path.to_string_lossy().replace('\\', "/");

        let script_path = dir.join("repeat.js");
        std::fs::write(
            &script_path,
            format!(
                "var first = -1;\n\
                 var second = -1;\n\
                 var idle = 0;\n\
                 function onUpdate(name) {{\n\
                   if (first < 0) {{\n\
                     first = Bsengine.playSound(\"{sound_path_js}\", {{}});\n\
                     return;\n\
                   }}\n\
                   if (second >= 0) {{ return; }}\n\
                   if (Bsengine.getSoundState(first) === \"loading\") {{ return; }}\n\
                   if (idle < 5) {{ idle += 1; return; }}\n\
                   second = Bsengine.playSound(\"{sound_path_js}\", {{}});\n\
                   Bsengine.setHudText(\"second\", String(second));\n\
                 }}"
            ),
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: String::new(),
        });
        app.world_mut().spawn((
            Name("Speaker".to_string()),
            ScriptPath(script_path.to_string_lossy().to_string()),
        ));

        let mut frames = 0;
        while !app.world().resource::<HudTexts>().0.contains_key("second") {
            app.update();
            frames += 1;
            assert!(
                frames < 300,
                "the first play never resolved, so the repeat was never requested \
                 (is MINIMAL_FLAC_SILENCE still decodable?)"
            );
        }

        // `app.update()` has already run `start_pending_sounds` for the frame
        // that asked for the repeat, so an empty queue here means the repeat
        // started on its requesting frame rather than waiting for a reload.
        assert!(
            app.world().resource::<PendingSounds>().0.is_empty(),
            "a repeat play of an already-loaded sound must start on the frame it \
             is requested, not wait for the file to be read again"
        );

        let loads = app.world().resource::<SoundLoads>();
        assert_eq!(
            loads.0.len(),
            1,
            "both plays name the same path, so it must be requested once"
        );
        // Keyed by exactly the string the script passed, which is what
        // `PlaySound` resolves and `AssetServer` was handed.
        let handle = match loads.0.get(&sound_path_js) {
            Some(SoundLoad::Ready(handle)) => handle.clone(),
            other => panic!("the resolved path must be retained as Ready, got {other:?}"),
        };
        assert!(
            app.world()
                .resource::<Assets<AudioSourceAsset>>()
                .get(&handle)
                .is_some(),
            "the decoded sound must stay resident between plays, or every repeat \
             is a fresh disk read several frames late"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // Audio is the one asset consumer this phase adds no rebuild code to, and
    // that is a claim worth pinning rather than assuming. It holds only
    // because `SoundLoads` already retains a *strong* handle in `Ready` (for
    // repeat plays — see the test above) and `bevy_asset` swaps the newly
    // decoded data in under that same handle, so the next `playSound` picks
    // up the edited file with nothing written here. Break either half and the
    // claim collapses silently: if a reload evicted the entry, or if a future
    // cleanup downgraded the retained handle to a weak one, `track_assets`
    // would free the sound, the next `playSound` would go back to disk, and
    // audio would need the same rebuild system glTF, shaders and the skybox
    // got.
    //
    // No audio device is involved. `AudioWorld` no-ops here as everywhere
    // else in these tests; every assertion is about `SoundLoads`,
    // `Assets<AudioSourceAsset>` and the asset events, never about sound.
    #[test]
    fn reloading_a_resident_sound_replaces_it_rather_than_evicting_it() {
        use bevy_asset::{AssetEvent, AssetServer, Assets};
        use bevy_ecs::event::{Events, ManualEventReader};
        use bsengine_audio::AudioSourceAsset;

        let dir =
            std::env::temp_dir().join(format!("bsengine_test_sound_reload_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sound_path = dir.join("blip.flac");
        std::fs::write(&sound_path, MINIMAL_FLAC_SILENCE).unwrap();
        // Backslashes would be escapes inside the JS string literal below;
        // Windows accepts forward slashes just as happily.
        let sound_path_js = sound_path.to_string_lossy().replace('\\', "/");

        let script_path = dir.join("play_once.js");
        std::fs::write(
            &script_path,
            format!(
                "var played = false;\n\
                 function onUpdate(name) {{\n\
                   if (played) {{ return; }}\n\
                   played = true;\n\
                   Bsengine.playSound(\"{sound_path_js}\", {{}});\n\
                 }}"
            ),
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: String::new(),
        });
        app.world_mut().spawn((
            Name("Speaker".to_string()),
            ScriptPath(script_path.to_string_lossy().to_string()),
        ));

        // Loops until the play has genuinely *resolved*, not merely until the
        // queue looks empty: an empty queue is equally what "the script has
        // not run yet" and "the load gave up" look like, and reloading from
        // either of those states would make every assertion below vacuous.
        let mut frames = 0;
        loop {
            app.update();
            frames += 1;
            let resolved = matches!(
                app.world().resource::<SoundLoads>().0.values().next(),
                Some(SoundLoad::Ready(_))
            );
            if resolved && app.world().resource::<PendingSounds>().0.is_empty() {
                break;
            }
            assert!(
                frames < 300,
                "the sound never resolved (is MINIMAL_FLAC_SILENCE still \
                 decodable?); SoundLoads was {:?}",
                app.world().resource::<SoundLoads>().0
            );
        }

        // Only the *id* is carried across the reload, never a clone of the
        // handle: a strong clone held by the test would keep the asset alive
        // on its own, and the liveness assertion at the end would then pass
        // even if `SoundLoads` had stopped retaining anything at all.
        let (resolved_path, asset_id) = {
            let loads = app.world().resource::<SoundLoads>();
            assert_eq!(
                loads.0.len(),
                1,
                "exactly one path was played, got {:?}",
                loads.0
            );
            let (path, load) = loads.0.iter().next().unwrap();
            let SoundLoad::Ready(handle) = load else {
                unreachable!("the loop above only exits on Ready, got {load:?}")
            };
            (path.clone(), handle.id())
        };
        assert_eq!(
            resolved_path, sound_path_js,
            "the map is keyed by exactly the string handed to AssetServer, so \
             any other key here means the reload below would name a different \
             asset and silently do nothing"
        );
        assert!(
            app.world()
                .resource::<Assets<AudioSourceAsset>>()
                .get(asset_id)
                .is_some(),
            "the sound must be resident before the reload, or nothing measured \
             after it means anything"
        );

        let mut reader: ManualEventReader<AssetEvent<AudioSourceAsset>> = app
            .world_mut()
            .resource_mut::<Events<AssetEvent<AudioSourceAsset>>>()
            .get_reader();
        // Discards the Added/LoadedWithDependencies of the original load, so
        // only what the reload itself emits is counted.
        {
            let events = app
                .world()
                .resource::<Events<AssetEvent<AudioSourceAsset>>>();
            let _ = reader.read(events).count();
        }

        app.world()
            .resource::<AssetServer>()
            .reload(resolved_path.clone());

        // 60 frames is far more than the reload needs; the number is chosen
        // for the other half of this test. `Assets::track_assets` runs every
        // `PreUpdate` and frees an asset the frame after its last strong
        // handle drops, so a merely-weak retained handle gets ~60 chances to
        // be collected before the liveness assertion below runs.
        let mut events_seen: Vec<String> = Vec::new();
        let mut modified_ours = false;
        for _ in 0..60 {
            app.update();
            let events = app
                .world()
                .resource::<Events<AssetEvent<AudioSourceAsset>>>();
            for event in reader.read(events) {
                if matches!(event, AssetEvent::Modified { id } if *id == asset_id) {
                    modified_ours = true;
                }
                events_seen.push(format!("{event:?}"));
            }
        }

        // How we know the reload was not a silent no-op: `Modified` is emitted
        // only when the freshly decoded data is actually inserted under the
        // existing id, and that insertion *is* "the next playSound uses the
        // new file". Without this the remaining assertions would also hold for
        // a reload that never happened.
        assert!(
            modified_ours,
            "reloading the retained sound must emit AssetEvent::Modified for \
             its id -- otherwise the reload did nothing and this test proves \
             nothing; events seen were {events_seen:?}"
        );

        let loads = app.world().resource::<SoundLoads>();
        let Some(SoundLoad::Ready(handle)) = loads.0.get(&resolved_path) else {
            panic!(
                "a reload must leave the path Ready, not evict or downgrade it; \
                 got {:?}",
                loads.0.get(&resolved_path)
            );
        };
        assert_eq!(
            handle.id(),
            asset_id,
            "the reload must replace the data under the existing id rather \
             than mint a second asset the retained handle no longer names"
        );
        // The assertion that actually measures retention. The `Ready` check
        // above is not enough on its own: a weak handle satisfies it while
        // still letting `track_assets` free the sound underneath.
        assert!(
            app.world()
                .resource::<Assets<AudioSourceAsset>>()
                .get(handle)
                .is_some(),
            "the retained handle must still resolve after the reload -- if it \
             does not, the next playSound goes back to disk and audio needs \
             rebuild code after all"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // A sound that has been asked for but has not started yet must report
    // `"loading"`, not `""`. `""` is what `getSoundState` returns for an id
    // that was never played at all, so a script polling for "has my sound
    // finished?" cannot tell the two apart and fires its finished branch
    // several frames early — before the sound has made a sound.
    //
    // The timing is fixed rather than raced: frame 1 queues the play (the
    // load cannot have failed yet, `LoadState::Failed` is set in `PreUpdate`),
    // and frame 2 reads the snapshot `run_scripts` builds before it runs
    // `onUpdate`, while the entry is still queued.
    #[test]
    fn a_queued_sound_reports_loading_not_nothing() {
        let script_path = std::env::temp_dir().join(format!(
            "bsengine_test_queued_sound_state_{}.js",
            std::process::id()
        ));
        std::fs::write(
            &script_path,
            "var id = -1;\n\
             function onUpdate(name) {\n\
               if (id < 0) {\n\
                 id = Bsengine.playSound(\"definitely/not/a/real/queued.wav\", {});\n\
                 return;\n\
               }\n\
               Bsengine.setHudText(\"state\", Bsengine.getSoundState(id));\n\
               Bsengine.setHudText(\"position\", String(Bsengine.getSoundPosition(id)));\n\
             }",
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: String::new(),
        });
        app.world_mut().spawn((
            Name("Speaker".to_string()),
            ScriptPath(script_path.to_string_lossy().to_string()),
        ));

        app.update();
        assert_eq!(
            app.world().resource::<PendingSounds>().0.len(),
            1,
            "the play must still be queued for the state read below to mean anything"
        );
        app.update();

        let hud = &app.world().resource::<HudTexts>().0;
        assert_eq!(
            hud.get("state").map(String::as_str),
            Some("loading"),
            "a queued sound must be distinguishable from one that never played"
        );
        // Deliberately still 0.0: a sound that has not started has no honest
        // position to report.
        assert_eq!(hud.get("position").map(String::as_str), Some("0"));

        let _ = std::fs::remove_file(&script_path);
    }

    // `Bsengine.getAssetStatus` exists to make two answers different that
    // used to be the same one. A path that failed to load and a path nothing
    // ever mentioned both produced silence — at best a `warn!` nobody read —
    // and that ambiguity is what let `games/mini-arena` run with no mesh and
    // no shader across two phases of work.
    //
    // So both directions are asserted here, and the second is not optional
    // padding: an op that returned `"failed: ..."` for every path on earth
    // would satisfy the first assertion alone while being worth nothing.
    //
    // Driven through a real script, a real `AssetServer` and the real
    // per-frame snapshot refresh, because every one of those is a place the
    // wiring can be missing. In particular `AssetStatusPlugin` is added
    // explicitly below — an app without it mirrors nothing, and the whole op
    // answers `"unknown"` forever, which is the failure this test would catch
    // if a host ever stopped registering it.
    #[test]
    fn get_asset_status_tells_a_failed_path_from_one_nothing_requested() {
        // Requested through `playSound`, which goes through
        // `bsengine_asset::load` — the funnel that records the request — so
        // the path is known to `AssetStatuses` from the frame after the call.
        // `project_dir` is empty below, so this string is also the exact
        // spelling the load site uses and therefore the exact key to query.
        const REQUESTED: &str = "definitely/not/a/real/asset-status-probe.wav";
        // Same shape, equally nonexistent; the only difference is that
        // nothing ever asks for it.
        const NEVER_ASKED: &str = "definitely/not/a/real/asset-status-never-asked.wav";

        let script_path = std::env::temp_dir().join(format!(
            "bsengine_test_asset_status_{}.js",
            std::process::id()
        ));
        // Played once, not every frame: re-requesting a failed path resets it
        // to `Loading` and restarts the load (see the `SoundLoads` docs), so
        // a per-frame play could never settle on a failure to report.
        std::fs::write(
            &script_path,
            format!(
                "var played = false;\n\
                 function onUpdate(name) {{\n\
                   if (!played) {{\n\
                     played = true;\n\
                     Bsengine.playSound(\"{REQUESTED}\", {{}});\n\
                   }}\n\
                   Bsengine.setHudText(\"requested\", Bsengine.getAssetStatus(\"{REQUESTED}\"));\n\
                   Bsengine.setHudText(\"never\", Bsengine.getAssetStatus(\"{NEVER_ASKED}\"));\n\
                 }}"
            ),
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(bsengine_asset::AssetStatusPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: String::new(),
        });
        app.world_mut().spawn((
            Name("Probe".to_string()),
            ScriptPath(script_path.to_string_lossy().to_string()),
        ));

        // A hang guard, not a budget: the load is one missing local file.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let (requested, never_asked) = loop {
            app.update();
            let (requested, never_asked) = {
                let hud = &app.world().resource::<HudTexts>().0;
                (
                    hud.get("requested").cloned().unwrap_or_default(),
                    hud.get("never").cloned().unwrap_or_default(),
                )
            };

            // Checked on *every* frame, not only at the end: a path nobody
            // asked for must never read as anything else, not even briefly on
            // its way somewhere. Skipped only before the script's first
            // `onUpdate` has written anything at all.
            if !never_asked.is_empty() {
                assert_eq!(
                    never_asked, "unknown",
                    "a path nothing ever requested must read \"unknown\" on every \
                     frame — anything else means the op is answering about a path \
                     the engine never heard of"
                );
            }

            if requested.starts_with("failed:") {
                break (requested, never_asked);
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the requested load never resolved, so this proves nothing; last \
                 status was {requested:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        };

        let reason = requested.strip_prefix("failed: ").unwrap_or_default();
        assert!(
            !reason.trim().is_empty(),
            "a failure with no reason is no better than the warn! it replaces, got \
             {requested:?}"
        );
        let lowered = reason.to_lowercase();
        assert!(
            lowered.contains("not found") || lowered.contains("asset-status-probe.wav"),
            "the reason must name what went wrong or what it went wrong on, got \
             {requested:?}"
        );
        assert_eq!(
            never_asked, "unknown",
            "a path nothing requested must still read \"unknown\" once the other \
             one has failed"
        );
        assert_ne!(
            requested, never_asked,
            "telling these two apart is the entire point of the op; an \
             implementation that answers the same for both is no better than the \
             log line it replaces"
        );

        let _ = std::fs::remove_file(&script_path);
    }

    // `playSound(); pauseSound(id)` in one `onUpdate` used to yield an
    // audible sound: the play was still queued, so the pause found no kira
    // handle, did nothing, and the sound started anyway — the same inversion
    // `stopSound` had, one notch weaker only because it is recoverable.
    // All three plays and both control commands happen in a single
    // `onUpdate`, so every command is applied in one `run_scripts` pass
    // before any load can resolve; the queue state below is fixed, not a
    // race. The un-paused play pins the other direction (a fix that paused
    // the whole queue would catch it too) and the paused-then-resumed one
    // pins `resumeSound` clearing the flag again. There is no audio device
    // here (`AudioWorld` no-ops), so the queue is the only observable.
    #[test]
    fn pause_sound_pauses_a_still_loading_sound() {
        let script_path = std::env::temp_dir().join(format!(
            "bsengine_test_pause_pending_sound_{}.js",
            std::process::id()
        ));
        std::fs::write(
            &script_path,
            "var played = false;\n\
             function onUpdate(name) {\n\
               if (played) { return; }\n\
               played = true;\n\
               var paused = Bsengine.playSound(\"definitely/not/a/real/paused.wav\", {});\n\
               var resumed = Bsengine.playSound(\"definitely/not/a/real/resumed.wav\", {});\n\
               var untouched = Bsengine.playSound(\"definitely/not/a/real/untouched.wav\", {});\n\
               Bsengine.pauseSound(paused);\n\
               Bsengine.pauseSound(resumed);\n\
               Bsengine.resumeSound(resumed);\n\
               Bsengine.setHudText(\"paused\", String(paused));\n\
               Bsengine.setHudText(\"resumed\", String(resumed));\n\
               Bsengine.setHudText(\"untouched\", String(untouched));\n\
             }",
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: String::new(),
        });
        app.world_mut().spawn((
            Name("Speaker".to_string()),
            ScriptPath(script_path.to_string_lossy().to_string()),
        ));

        app.update();

        let read_id = |key: &str| -> u32 {
            app.world()
                .resource::<HudTexts>()
                .0
                .get(key)
                .unwrap_or_else(|| panic!("script did not report the {key} sound's id"))
                .parse()
                .expect("reported id is not a number")
        };
        let queued_paused = |id: u32| -> bool {
            app.world()
                .resource::<PendingSounds>()
                .0
                .iter()
                .find(|entry| entry.id == id)
                .unwrap_or_else(|| panic!("sound {id} is not queued"))
                .paused
        };

        assert!(
            queued_paused(read_id("paused")),
            "pauseSound must reach a play that is still loading, or the sound \
             becomes audible after being paused"
        );
        assert!(
            !queued_paused(read_id("resumed")),
            "resumeSound must clear a pause recorded on a queued play"
        );
        assert!(
            !queued_paused(read_id("untouched")),
            "pauseSound must reach only the id it was given, not the whole queue"
        );

        let _ = std::fs::remove_file(&script_path);
    }

    // A stop that arrives before the async load resolves must still stop the
    // sound. `playSound` only reaches `SoundHandles` once the decode finishes,
    // so a `SoundHandles`-only stop finds nothing, does nothing, and the sound
    // then starts anyway — the opposite of what the script asked for. Both
    // plays and the stop happen in a single `onUpdate`, so all three commands
    // are applied in one `run_scripts` pass before any load can resolve; the
    // queue state below is therefore fixed, not a race. The ids are reported
    // back through `setHudText` because the op assigns them, not the test.
    // The second, un-stopped sound pins the other direction: a fix that
    // cleared the whole queue would drop it too. There is no audio device here
    // (`AudioWorld` no-ops), so the queue is the only observable.
    #[test]
    fn stop_sound_drops_a_still_loading_sound_from_the_queue() {
        let script_path = std::env::temp_dir().join(format!(
            "bsengine_test_stop_pending_sound_{}.js",
            std::process::id()
        ));
        std::fs::write(
            &script_path,
            "var played = false;\n\
             function onUpdate(name) {\n\
               if (played) { return; }\n\
               played = true;\n\
               var stopped = Bsengine.playSound(\"definitely/not/a/real/stopped.wav\", {});\n\
               var kept = Bsengine.playSound(\"definitely/not/a/real/kept.wav\", {});\n\
               Bsengine.stopSound(stopped);\n\
               Bsengine.setHudText(\"stopped\", String(stopped));\n\
               Bsengine.setHudText(\"kept\", String(kept));\n\
             }",
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: String::new(),
        });
        app.world_mut().spawn((
            Name("Speaker".to_string()),
            ScriptPath(script_path.to_string_lossy().to_string()),
        ));

        app.update();

        let read_id = |key: &str| -> u32 {
            app.world()
                .resource::<HudTexts>()
                .0
                .get(key)
                .unwrap_or_else(|| panic!("script did not report the {key} sound's id"))
                .parse()
                .expect("reported id is not a number")
        };
        let stopped = read_id("stopped");
        let kept = read_id("kept");
        assert_ne!(stopped, kept, "each play must get its own id");

        let queued: Vec<u32> = app
            .world()
            .resource::<PendingSounds>()
            .0
            .iter()
            .map(|entry| entry.id)
            .collect();
        assert!(
            !queued.contains(&stopped),
            "stopSound must cancel a queued play, or the sound starts after being stopped; queue held {queued:?}"
        );
        assert!(
            queued.contains(&kept),
            "stopSound must drop only the id it was given, not the whole queue; queue held {queued:?}"
        );

        let _ = std::fs::remove_file(&script_path);
    }

    #[test]
    fn move_entity_moves_transform_by_delta() {
        let script_path = std::env::temp_dir().join(format!(
            "bsengine_test_move_entity_{}.js",
            std::process::id()
        ));
        std::fs::write(
            &script_path,
            "function onUpdate(name) { Bsengine.moveEntity(name, 1.0, 0.0, 2.0); }",
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: String::new(),
        });
        app.world_mut().spawn((
            Name("Hero".to_string()),
            Transform::from_translation(Vec3::new(5.0, 0.0, 5.0)),
            ScriptPath(script_path.to_string_lossy().to_string()),
        ));

        app.update();
        app.update();

        let world = app.world_mut();
        let mut q = world.query::<(&Name, &Transform)>();
        let (_, t) = q
            .iter(world)
            .find(|(n, _)| n.0 == "Hero")
            .expect("Hero entity with Transform not found");
        // `load_scripts` runs at `PostStartup`, which executes during the *first*
        // `app.update()` call, immediately before that same call's `Update` schedule
        // (which runs `run_scripts`/`onUpdate`). So two `app.update()` calls invoke
        // `onUpdate` twice total (once during update #1, once during update #2), and
        // the (1.0, 0.0, 2.0) delta from `moveEntity` is applied twice: (2.0, 0.0, 4.0).
        assert!(
            (t.translation.x - 7.0).abs() < 1e-4,
            "x: {}",
            t.translation.x
        );
        assert!(
            (t.translation.z - 9.0).abs() < 1e-4,
            "z: {}",
            t.translation.z
        );

        let _ = std::fs::remove_file(&script_path);
    }

    #[test]
    fn pause_then_is_paused_reports_true_next_frame() {
        let script_path =
            std::env::temp_dir().join(format!("bsengine_test_pause_{}.js", std::process::id()));
        std::fs::write(
            &script_path,
            "let called = false;\n\
             function onUpdate(name) {\n\
                 if (!called) { Bsengine.pause(); called = true; return; }\n\
                 Bsengine.setSaveField(name, \"paused\", String(Bsengine.isPaused()));\n\
             }",
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: String::new(),
        });
        app.world_mut().spawn((
            Name("Hero".to_string()),
            bsengine_core::SaveData::new(0),
            ScriptPath(script_path.to_string_lossy().to_string()),
        ));

        app.update();
        app.update();
        app.update();

        let world = app.world_mut();
        let mut q = world.query::<(&Name, &bsengine_core::SaveData)>();
        let (_, save) = q
            .iter(world)
            .find(|(n, _)| n.0 == "Hero")
            .expect("Hero entity with SaveData not found");
        assert_eq!(save.get("paused"), Some(b"true".as_slice()));

        let _ = std::fs::remove_file(&script_path);
    }

    #[test]
    fn quit_sends_app_exit_event() {
        use bevy_app::AppExit;
        use bevy_ecs::event::Events;

        let script_path =
            std::env::temp_dir().join(format!("bsengine_test_quit_{}.js", std::process::id()));
        std::fs::write(&script_path, "function onUpdate(name) { Bsengine.quit(); }").unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: String::new(),
        });
        app.world_mut().spawn((
            Name("Hero".to_string()),
            ScriptPath(script_path.to_string_lossy().to_string()),
        ));

        app.update();
        app.update();

        let events = app.world().resource::<Events<AppExit>>();
        assert!(
            events.iter_current_update_events().next().is_some(),
            "AppExit should have been sent"
        );

        let _ = std::fs::remove_file(&script_path);
    }

    #[test]
    fn set_shader_resolves_path_against_project_dir() {
        use bsengine_core::CustomShader;

        // `load_scripts` also resolves `ScriptPath` against `ProjectDir` (see
        // its doc comment above), so an absolute `ScriptPath` combined with a
        // non-empty `project_dir` would make script loading itself fail
        // (project_dir gets prefixed onto an already-absolute path). Using a
        // real temp directory as `project_dir` with a project-relative
        // `ScriptPath`, mirroring how a real game's `project_dir` +
        // `assets/scripts/...` are related, keeps both resolutions valid.
        let project_dir = std::env::temp_dir().join(format!(
            "bsengine_test_set_shader_project_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::write(
            project_dir.join("set_shader.js"),
            "function onUpdate(name) { Bsengine.setShader(name, \"shaders/glow.wgsl\"); }",
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: project_dir.to_string_lossy().to_string(),
        });
        app.world_mut().spawn((
            Name("Hero".to_string()),
            ScriptPath("set_shader.js".to_string()),
        ));

        app.update();
        app.update();

        let world = app.world_mut();
        let mut q = world.query::<(&Name, &CustomShader)>();
        let (_, shader) = q
            .iter(world)
            .find(|(n, _)| n.0 == "Hero")
            .expect("Hero entity with CustomShader not found");
        assert_eq!(
            shader.path,
            format!("{}/shaders/glow.wgsl", project_dir.to_string_lossy())
        );

        let _ = std::fs::remove_dir_all(&project_dir);
    }
}

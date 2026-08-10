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
    PARENT_SNAPSHOT, PAUSED_SNAPSHOT, PHYSICS_WORLD_PTR, PROJECT_DIR, RESTITUTION_SNAPSHOT,
    SAVE_DATA_SNAPSHOT, SCREEN_SIZE_SNAPSHOT, SHIELD_SNAPSHOT, SLEEP_SNAPSHOT,
    SOUND_POSITION_SNAPSHOT, SOUND_STATE_SNAPSHOT, TIMER_SNAPSHOT, TIME_DELTA_SNAPSHOT,
    TIME_ELAPSED_SNAPSHOT, TONE_MAP_SNAPSHOT, TRANSFORM_SNAPSHOT, TWEEN_SNAPSHOT,
    UI_CLICKED_SNAPSHOT, VELOCITY_SNAPSHOT, VISIBLE_SNAPSHOT, WORLD_TRANSFORM_SNAPSHOT,
};
use crate::runtime::ScriptRuntime;

/// Loaded JS source for a scripted entity.
///
/// Present only once the source has actually arrived and been executed, which
/// is what makes it the marker `run_scripts` and `collect_world_snapshots`
/// filter on: an entity whose script is still loading, or whose script could
/// not be loaded at all, is not offered to `Bsengine._runAll`, so there is
/// never a frame where the engine asks JS to run an `onUpdate` that was never
/// registered.
#[derive(Component, bevy_reflect::Reflect)]
#[reflect(Component)]
pub struct Script {
    /// The full text of the entity's script file.
    pub source: String,
}

/// One entity's script file, as an asset request that outlives the frame it
/// was made on.
///
/// Inserted by [`load_scripts`] (which requests, once) and advanced by
/// [`execute_loaded_scripts`] (which polls, every frame, and never requests).
/// That split is the request-once/retain-the-handle/poll-the-retained-handle
/// shape every asset consumer in this engine uses; see
/// [`bsengine_asset::LoadMode`] for the argument and [`SoundLoads`] above for
/// the closest neighbour.
///
/// # Why the handle is retained after the script has run
///
/// The `Handle<ScriptSource>` here is the only strong handle to a script in
/// the whole engine. `Assets::<A>::track_assets` frees an asset the frame
/// after its last strong handle drops, and `AssetEvent::Modified` is only
/// emitted for an asset that is still tracked -- so dropping the handle once
/// the source had been executed would silently make scripts the one asset
/// class that cannot hot reload, while everything about the load still looked
/// like it worked. Item 30 has three separate cases of exactly that.
///
/// The state is kept for a failed load too, and for the same reason in
/// reverse: the entry is what stops the path being requested a second time.
#[derive(Component, Debug)]
struct ScriptLoad {
    /// The load, and with it the retained strong handle -- see the type docs
    /// for why that outlives the execution it was requested for, and
    /// [`bsengine_asset::AssetSlot`] for why a failed load keeps one too.
    ///
    /// `Ready` here doubles as "already executed": `execute_loaded_scripts`
    /// only polls slots still `Loading`, so the arrival that runs a script is
    /// also what takes it out of that set. Nothing else records having run it.
    slot: bsengine_asset::AssetSlot<crate::script_asset::ScriptSource>,
    /// The path as this engine spelled it when it asked. Kept rather than
    /// re-derived from `AssetServer::get_path`, which can only answer while
    /// the `AssetInfo` exists -- and the message that most needs a path is
    /// the one about a load that failed.
    path: String,
}

/// Non-Send wrapper around the entity's V8 isolate; stored as a non-send
/// resource via `insert_non_send_resource` since `JsRuntime` isn't `Send`/`Sync`.
pub struct ScriptRuntimeResource(pub ScriptRuntime);

/// Stores kira sound handles by script-assigned id for stopSound support.
#[derive(Resource, Default)]
pub struct SoundHandles(pub HashMap<u32, kira::sound::static_sound::StaticSoundHandle>);

/// What the audio consumer knows about one sound path.
///
/// Two cases rather than one, because "the load failed" and "there was never
/// anything to ask" are different facts that used to share a `GaveUp`. Only the
/// first is [`bsengine_asset::AssetSlot`]'s business.
#[derive(Debug)]
enum SoundLoad {
    /// A request was made; this is what became of it.
    ///
    /// The handle is kept even once the sound is resident -- see [`SoundLoads`]
    /// for why -- and even once the load has failed, for the reason
    /// [`bsengine_asset::AssetSlot::GaveUp`] gives.
    Requested(bsengine_asset::AssetSlot<bsengine_audio::AudioSourceAsset>),
    /// The request could not even be made, so there is no handle to hold.
    ///
    /// Not a failed load, which is why this cannot be an
    /// [`AssetSlot`](bsengine_asset::AssetSlot) at all: that type starts from a
    /// handle, and here none was ever produced.
    ///
    /// Both ways in are defensive rather than reachable — `LoadMode::Async` is
    /// infallible, and [`ScriptingPlugin`] registers
    /// `Assets<AudioSourceAsset>` itself, so an app that can call `playSound`
    /// has one. Recorded as a state anyway so that if either ever does happen,
    /// the warning fires once instead of once per call: a script calling
    /// `playSound` every frame would otherwise print sixty lines a second
    /// about a wiring mistake.
    Unrequestable,
}

impl SoundLoad {
    /// Whether a play on this path can never start.
    ///
    /// Both cases are hopeless for the same *caller* reason even though they
    /// are different engine facts: `PlaySound` refuses to queue, and
    /// `start_pending_sounds` drops anything already queued.
    fn hopeless(&self) -> bool {
        match self {
            Self::Requested(slot) => slot.gave_up(),
            Self::Unrequestable => true,
        }
    }
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
    /// Name of the `AudioEmitter` entity this play is positioned at, or `None`
    /// for a non-positional play. Resolved to an entity when the sound
    /// actually starts, not now, because the entity may not exist yet.
    emitter: Option<String>,
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
        // Registered here rather than in `bsengine_asset::AssetPlugin` for the
        // same reason the audio asset is: the type belongs to the crate whose
        // plugin consumes it, so an app that wants scripts cannot end up with
        // the loader missing.
        app.init_asset::<crate::script_asset::ScriptSource>()
            .register_asset_loader(crate::script_asset::ScriptSourceLoader);

        // R1: every public component must be registered for reflection.
        // Not in `bsengine_scene::register_gameplay_reflect_types`, where the
        // rule's message points, because `bsengine-scripting` depends on
        // `bsengine-scene` — the reverse edge would be a cycle. This plugin is
        // in both the windowed runtime and the headless `--test` app, so the
        // registration reaches the same two hosts that function does.
        app.register_type::<Script>();

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
            // `execute_loaded_scripts` before `run_scripts`, as an explicit
            // edge rather than as a consequence of the order they are named
            // in: an unconstrained schedule *sorts* its systems and does not
            // replay registration order, so "added earlier" guarantees
            // nothing. The edge is the one property that stopped being
            // structural when scripts became assets -- reading a file inline
            // meant a script could not exist without already having run,
            // whereas now the frame its source arrives is the frame something
            // has to execute it, and it has to happen before that same
            // frame's `_runAll` or the entity silently misses its first
            // `onUpdate`.
            //
            // `reexecute_modified_scripts` takes the same edge for the same
            // reason, on both sides. After `execute_loaded_scripts`, because
            // that is what owns the `Loading -> Ready` transition the reload
            // reads (ahead of it, an entity whose first load resolved this
            // frame would still be `Loading` and the reload would skip it) --
            // and because the `Added` arm has to be able to tell an ordinary
            // first load from a recovery, which is that same state. Before
            // `run_scripts`, because a reload the frame's `_runAll` does not
            // see is a reload that visibly takes an extra frame, and one that
            // lands *after* an `onUpdate` from the previous revision is worse
            // than that: for one frame the entity runs code the file no longer
            // contains.
            (
                capture_collision_events,
                execute_loaded_scripts,
                reexecute_modified_scripts,
                run_scripts,
                start_pending_sounds,
            )
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

/// Runs `BOOTSTRAP_JS`, then *requests* the JS source for every entity that
/// has a `ScriptPath` (resolved against `ProjectDir`) and has not been asked
/// for yet, recording the retained handle as a [`ScriptLoad`].
///
/// # What this no longer does
///
/// It used to `std::fs::read_to_string` each path and execute it before
/// returning, so "the entity has a script" and "the script is running" were
/// one atomic step. Scripts are `bevy_asset` assets now: this hands the path
/// to `AssetServer` and returns, and [`execute_loaded_scripts`] runs the
/// source once it appears in `Assets<ScriptSource>`. Both callers had that
/// atomicity and neither has it now.
///
/// How long the gap is depends on which schedule the caller runs in, because
/// `bevy_asset` publishes finished loads from `PreUpdate`:
///
/// * **`PostStartup` (this plugin's own registration).** `PostStartup` runs
///   before `PreUpdate`, which runs before `Update`, all within the first
///   `app.update()` -- so the source is published and executed on that same
///   first frame. **Zero frames of visible latency**, and every existing test
///   that counted `app.update()`s still counts the same.
/// * **`Update` (`handle_scene_load` in `bsengine-runtime`, which calls this
///   inline right after respawning a scene).** `PreUpdate` has already run,
///   so the source is published on the *following* frame and executed there.
///   **One frame**, measured -- see that call site's comment for what happens
///   in between, and `scene_systems`'
///   `a_script_loaded_scene_gets_its_own_scripts_running` for the test that
///   pins it.
///
/// Both numbers hold because `bevy_tasks` is built here without its
/// `multi-threaded` feature, so `TaskPool::spawn` blocks on the read rather
/// than handing it to a worker. Turn that feature on and the read becomes
/// genuinely concurrent and both numbers become "one frame or more" -- which
/// is why nothing in this crate's tests asserts a fixed frame count for a
/// load, only that it eventually arrives.
///
/// # What is still atomic, and has to be
///
/// `BOOTSTRAP_JS` runs *here*, synchronously, before anything is requested --
/// not from the polling system. Two things depend on that:
///
/// * It defines the `Bsengine` global every script's body and every wrapper
///   below references, so a script executing before it had run would fail
///   outright with `Bsengine is not defined`.
/// * Re-running it is how a scene load resets `Bsengine._scripts`, the timer
///   queue and the message/collision handlers (see `handle_scene_load`'s
///   comment). That reset has to land on the frame the old entities are
///   despawned, not whenever the new scene's files happen to arrive, or the
///   dead scene's handlers keep firing in between.
///
/// `PostStartup` runs before `Update` within the same `app.update()`, and the
/// scene-load caller invokes this inline, so in both cases the bootstrap is
/// already done by the time [`execute_loaded_scripts`] can see anything.
///
/// # Requesting once
///
/// Entities that already carry a [`ScriptLoad`] are skipped, so calling this
/// again -- which a scene load does -- never re-requests a path. That matters
/// most for a path that *failed*: `AssetServer::load` on a `Failed` path
/// resets it to `Loading` and starts over, which would make the failure
/// permanently unobservable. See [`bsengine_asset::AssetSlot::GaveUp`].
pub fn load_scripts(world: &mut World) {
    let project_dir = world
        .get_resource::<ProjectDir>()
        .map(|pd| pd.0.clone())
        .unwrap_or_default();

    let scripts: Vec<(Entity, String)> = {
        let mut q = world.query_filtered::<(Entity, &ScriptPath), Without<ScriptLoad>>();
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

    let Some(asset_server) = world.get_resource::<bevy_asset::AssetServer>().cloned() else {
        tracing::warn!(
            "[scripting] no AssetServer (bsengine_asset::AssetPlugin not registered?); \
             {} script(s) will never load",
            scripts.len()
        );
        return;
    };

    for (entity, path) in scripts {
        // `load_async` rather than `AssetServer::load`: a path requested the
        // latter way is invisible to `AssetStatuses` until it fails, so a
        // script that loaded and a script nothing ever asked for would read
        // back identically through `Bsengine.getAssetStatus`. It is also what
        // resolves a path an asset has since moved away from.
        let slot = bsengine_asset::AssetSlot::<crate::script_asset::ScriptSource>::requesting(
            &asset_server,
            &path,
        );
        world.entity_mut(entity).insert(ScriptLoad { slot, path });
    }
}

/// Executes each requested script whose source has arrived, and gives up --
/// once, out loud -- on each one that cannot arrive.
///
/// This is the polling half of the pair described on [`load_scripts`]. It
/// advances the [`bsengine_asset::AssetSlot`] that [`ScriptLoad`] already holds
/// and **never requests anything** — see
/// [`AssetSlot::GaveUp`](bsengine_asset::AssetSlot::GaveUp) for why a
/// re-requesting poll loop can never observe a failure at all. Phase 2 of item
/// 24 measured that shape at 200 errors over 200 frames against one for this
/// one.
///
/// Executing a script is what inserts its [`Script`] component, so an entity
/// whose source never arrives is simply never offered to `Bsengine._runAll`
/// (see [`Script`]). Nothing downstream has to special-case it.
fn execute_loaded_scripts(world: &mut World) {
    // Nothing to do on almost every frame: after the first couple, every
    // script has either executed or been given up on, so this collects an
    // empty vec and returns.
    let pending: Vec<Entity> = {
        let mut q = world.query::<(Entity, &ScriptLoad)>();
        q.iter(world)
            .filter(|(_, load)| matches!(load.slot, bsengine_asset::AssetSlot::Loading(_)))
            .map(|(entity, _)| entity)
            .collect()
    };
    if pending.is_empty() {
        return;
    }

    let asset_server = world.resource::<bevy_asset::AssetServer>().clone();
    // Decided first, so the borrows end before the world is mutated below.
    let mut ready: Vec<(Entity, String, String)> = Vec::new();

    // `resource_scope` lifts `Assets` out of the world for the duration. A slot
    // lives on a component, so polling it needs `&mut World` at the same time
    // as the assets it polls against -- which the borrow checker will not give
    // while `Assets` is still a resource in there.
    world.resource_scope(
        |world, assets: bevy_ecs::world::Mut<
            bevy_asset::Assets<crate::script_asset::ScriptSource>,
        >| {
            for entity in pending {
                let Some(mut load) = world.get_mut::<ScriptLoad>(entity) else {
                    continue;
                };
                match load.slot.poll(&asset_server, &assets) {
                    bsengine_asset::Polled::Arrived => {
                        if let Some(source) = assets.get(load.slot.handle()) {
                            ready.push((entity, load.path.clone(), source.0.clone()));
                        }
                    }
                    // Once per entity, because the slot settles once: a
                    // per-frame warning for a path that will never load is how
                    // the log that should carry this line gets buried.
                    bsengine_asset::Polled::Failed(e) => {
                        tracing::warn!("[scripting] giving up on {}: {e}", load.path);
                    }
                    bsengine_asset::Polled::Nothing => {}
                }
            }
        },
    );

    for (entity, path, source) in ready {
        match adopt_script_source(world, entity, &path, source) {
            Some(Ok(())) => tracing::info!("[scripting] loaded: {path}"),
            Some(Err(e)) => tracing::error!("[scripting] error in {path}: {e}"),
            None => {}
        }
    }
}

/// Makes `source` the revision of `path` that `entity` is running: records it
/// as the entity's [`Script`], marks its [`ScriptLoad`] `Ready`, and evaluates
/// it in the shared isolate.
///
/// Shared by [`execute_loaded_scripts`] and [`reexecute_modified_scripts`] so
/// that a first execution and a hot reload cannot drift apart — they are the
/// same operation, and the only difference is what gets logged afterwards.
///
/// # The wrapper, and why re-running it is a replacement rather than a leak
///
/// The source goes into an IIFE that ends by assigning
/// `Bsengine._scripts["<entity bits>"] = { onUpdate }` — an assignment, keyed
/// by *entity*. So evaluating a second revision of a file **overwrites** that
/// entity's registration instead of adding to it, and nothing has to
/// unregister anything first. Two entities sharing one script path keep
/// separate registrations, because the key is the entity and not the path;
/// reloading the file re-runs the IIFE once per entity and replaces both.
///
/// The same shape decides what a reload *costs*: everything the file declares
/// is declared inside that function body, so a fresh evaluation gets fresh
/// bindings. See [`reexecute_modified_scripts`] for what that means for a
/// script that was keeping state in one.
///
/// Returns `None` when the app has no `ScriptRuntimeResource` at all — the
/// component and the state are still updated, since they describe what the
/// entity holds rather than what a runtime did with it.
fn adopt_script_source(
    world: &mut World,
    entity: Entity,
    path: &str,
    source: String,
) -> Option<Result<(), String>> {
    let id = entity.to_bits();
    let wrapped = format!(
        "(function() {{\n{source}\nBsengine._scripts[\"{id}\"] = \
         {{ onUpdate: typeof onUpdate === 'function' ? onUpdate : null }};\n}})();"
    );
    let mut entity_mut = world.get_entity_mut(entity)?;
    entity_mut.insert(Script { source });
    if let Some(mut load) = entity_mut.get_mut::<ScriptLoad>() {
        // A no-op on the ordinary path, where `execute_loaded_scripts` polled
        // the slot to `Ready` moments ago. It matters for a recovery: a script
        // whose file was missing is revived by `reexecute_modified_scripts`
        // from an `AssetEvent`, never by a poll, so nothing else would take its
        // slot out of `GaveUp`.
        load.slot.mark_arrived();
    }
    let mut rt = world.get_non_send_resource_mut::<ScriptRuntimeResource>()?;
    Some(rt.0.exec_source(&wrapped, path))
}

/// Re-runs a script whose file changed on disk while the game was running, so
/// an edit takes effect without a restart.
///
/// This is the half of hot reload that lives on this side of the boundary.
/// `bsengine_asset::AssetWatcherPlugin` answers "which paths changed" and calls
/// `AssetServer::reload`; `bevy_asset` re-reads the file and replaces the
/// `ScriptSource` behind the handle [`ScriptLoad`] retained, which is what
/// emits the [`AssetEvent`](bevy_asset::AssetEvent) read here. None of that
/// makes the *new* source run — re-evaluating it is this system's whole job,
/// and a version that only logged the event would look identical in the log
/// and change nothing on screen.
///
/// # What a reload resets
///
/// **Everything the script declared.** The wrapper described on
/// [`adopt_script_source`] is a fresh function invocation, so a reloaded script
/// gets fresh bindings for every top-level `let`, `var`, `const` and `function`
/// in the file:
///
/// ```js
/// var played = false;                 // back to false on every reload
/// function onUpdate(name) {
///     if (!played) { Bsengine.playSound("assets/sfx/intro.ogg"); played = true; }
/// }
/// ```
///
/// That is the right default and not merely the easy one: the alternative —
/// evaluating the new file with the old file's bindings still in scope — makes
/// a reload's result depend on how long the game had been running when the
/// save happened, which is exactly what makes "works after a restart, not
/// after a reload" bugs so hard to pin down. A script that must survive a
/// reload should keep its state in the world (a component, `SaveData`) rather
/// than in a module variable, where it is durable across scene loads too.
///
/// Because it *is* surprising the first time, the reload's `info!` line says so
/// rather than leaving it to be discovered.
///
/// # Once per edit, not once per frame
///
/// A save that an editor performs in several writes is collapsed twice before
/// it gets here: the watcher debounces, then dedupes by path across everything
/// it drains, so one edit is one `AssetServer::reload`, one `Modified`, and one
/// re-execution per entity holding the handle.
///
/// # `Added` is the failed-then-fixed case, and only that
///
/// A script whose *first* load failed has no asset behind its handle, so the
/// load that finally succeeds `insert`s rather than replaces, and `bevy_asset`
/// reports it as `Added` (`assets.rs`'s `insert_with_index`) — a `Modified`
/// for it never comes. Without that arm a single typo in a filename, or one
/// non-UTF-8 byte, would be permanent for the rest of the run even after the
/// file was fixed: [`execute_loaded_scripts`] deliberately never revisits
/// [`bsengine_asset::AssetSlot::GaveUp`], and it is right not to, since
/// re-requesting is
/// what erases the failure. Nothing here re-requests either; recovery arrives
/// because [`ScriptLoad`] kept the handle even for a load that failed.
///
/// `Added` is therefore acted on **only** for a `GaveUp` entity. On an ordinary
/// first load `execute_loaded_scripts` has already run the source a frame
/// earlier — `bevy_asset` flushes its events in `Last` — so treating that
/// `Added` as a reload would evaluate every script twice, for nothing.
fn reexecute_modified_scripts(
    world: &mut World,
    mut reader: Local<
        bevy_ecs::event::ManualEventReader<
            bevy_asset::AssetEvent<crate::script_asset::ScriptSource>,
        >,
    >,
) {
    let changed: Vec<(bevy_asset::AssetId<crate::script_asset::ScriptSource>, bool)> = {
        let events =
            world.resource::<Events<bevy_asset::AssetEvent<crate::script_asset::ScriptSource>>>();
        reader
            .read(events)
            .filter_map(|event| match event {
                bevy_asset::AssetEvent::Modified { id } => Some((*id, false)),
                bevy_asset::AssetEvent::Added { id } => Some((*id, true)),
                _ => None,
            })
            .collect()
    };
    // The overwhelmingly common frame: nothing changed on disk, so this is one
    // empty event read and a return.
    if changed.is_empty() {
        return;
    }

    // Decided first, so the shared borrows end before the world is mutated --
    // the same split `execute_loaded_scripts` uses, and for the same reason.
    let mut to_run: Vec<(Entity, String, String, bool)> = Vec::new();
    {
        let mut q = world.query::<(Entity, &ScriptLoad)>();
        let assets = world.resource::<bevy_asset::Assets<crate::script_asset::ScriptSource>>();
        for (entity, load) in q.iter(world) {
            let Some(&(_, added)) = changed
                .iter()
                .find(|(id, _)| *id == load.slot.handle().id())
            else {
                continue;
            };
            let recovered = match &load.slot {
                // Fixed on disk, by either spelling of the event.
                bsengine_asset::AssetSlot::GaveUp(_) => true,
                // The reload proper. An `Added` here is the frame-late echo of
                // this entity's own first load; see the type docs.
                bsengine_asset::AssetSlot::Ready(_) if !added => false,
                bsengine_asset::AssetSlot::Ready(_) => continue,
                // `execute_loaded_scripts` owns this transition and has
                // already made it this frame, ahead of this system.
                bsengine_asset::AssetSlot::Loading(_) => continue,
            };
            let Some(source) = assets.get(load.slot.handle()) else {
                continue;
            };
            to_run.push((entity, load.path.clone(), source.0.clone(), recovered));
        }
    }

    for (entity, path, source, recovered) in to_run {
        match adopt_script_source(world, entity, &path, source) {
            Some(Ok(())) if recovered => tracing::info!(
                "[scripting] {path} loads now and has been run; it had been given \
                 up on earlier this session"
            ),
            Some(Ok(())) => tracing::info!(
                "[scripting] reloaded {path}; the script was re-run from the top, \
                 so its top-level variables are back at their initial values"
            ),
            Some(Err(e)) => tracing::error!("[scripting] error in {path}: {e}"),
            None => {}
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
            ScriptCommand::SetPosition { name, x, y, z } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name, &mut Transform)>();
                    q.iter_mut(world).find_map(|(e, n, mut t)| {
                        (n.0 == name).then(|| {
                            t.position = Vec3::new(x, y, z).into();
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
            ScriptCommand::SetTransform {
                name,
                x,
                y,
                z,
                rx,
                ry,
                rz,
                rw,
                sx,
                sy,
                sz,
            } => {
                let position = Vec3::new(x, y, z);
                let rotation = Quat::from_xyzw(rx, ry, rz, rw);
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name, &mut Transform)>();
                    q.iter_mut(world).find_map(|(e, n, mut t)| {
                        (n.0 == name).then(|| {
                            t.position = position.into();
                            t.rotation = rotation.into();
                            t.scale = Vec3::new(sx, sy, sz).into();
                            e
                        })
                    })
                };
                // Both halves teleported for the reason the position-only arm
                // gives: a Dynamic body overwrites `Transform` from the
                // simulation on the next frame. Scale has no Rapier
                // counterpart, so there is nothing to mirror for it.
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_translation(e, position);
                    pw.set_rotation(e, rotation);
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
                        t.position.0 += Vec3::new(dx, dy, dz);
                        break;
                    }
                }
            }
            ScriptCommand::AddPositionLocal { name, dx, dy, dz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let rot = t.rotation;
                        t.position.0 += rot.0.mul_vec3(Vec3::new(dx, dy, dz));
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
                        t.position.x += dx;
                        t.position.y += dy;
                        t.position.z += dz;
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
                emitter,
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
                if known.is_some_and(SoundLoad::hopeless) {
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
                        Some(Ok(handle)) => {
                            SoundLoad::Requested(bsengine_asset::AssetSlot::from_handle(handle))
                        }
                        Some(Err(e)) => {
                            // Unreachable: `LoadMode::Async` is infallible.
                            // Present only because the shared `load()`
                            // signature returns `Result` for `Sync` callers.
                            tracing::warn!("[audio] failed to request {full_path}: {e}");
                            SoundLoad::Unrequestable
                        }
                        None => {
                            tracing::warn!(
                                "[audio] Assets<AudioSourceAsset> resource missing (AssetPlugin not registered?)"
                            );
                            SoundLoad::Unrequestable
                        }
                    };
                    let hopeless = load.hopeless();
                    if let Some(mut loads) = world.get_resource_mut::<SoundLoads>() {
                        loads.0.insert(full_path.clone(), load);
                    }
                    if hopeless {
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
                        emitter,
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
            ScriptCommand::BurstParticles { name } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                // Queued, not emitted here: emission needs the emitter's world
                // position and runs in `ParticlePlugin` on the next tick.
                if let Some(mut emitter) =
                    entity.and_then(|e| world.get_mut::<bsengine_core::ParticleEmitter>(e))
                {
                    emitter.burst();
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
            ScriptCommand::AddScale { name, sx, sy, sz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.0 += Vec3::new(sx, sy, sz);
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
    let asset_server = world.resource::<bevy_asset::AssetServer>().clone();

    // `resource_scope` lifts `Assets` out of the world so the slots in
    // `SoundLoads` can be advanced -- which needs the map mutably -- while the
    // assets they poll against are still readable.
    world.resource_scope(
        |world,
         assets: bevy_ecs::world::Mut<
            bevy_asset::Assets<bsengine_audio::AudioSourceAsset>,
        >| {
            let mut loads = world.resource_mut::<SoundLoads>();
            for entry in entries {
                // The handle is read from `SoundLoads`, never re-requested
                // here: `AssetServer::load` on a path already in `Failed`
                // restarts it, which would make the failure unobservable.
                let Some(SoundLoad::Requested(slot)) = loads.0.get_mut(&entry.path) else {
                    // Unreachable: `PlaySound` records the path before queueing
                    // and refuses to queue at all once it is hopeless.
                    continue;
                };
                // Warned inline rather than collected per path: the slot
                // reports `Failed` once, so N plays queued on one bad path
                // still produce one line between them.
                if let bsengine_asset::Polled::Failed(e) = slot.poll(&asset_server, &assets) {
                    tracing::warn!("[audio] failed to load queued sound {}: {e}", entry.path);
                }
                if slot.gave_up() {
                    // Dropped, not requeued: this play can never start.
                    continue;
                }
                match assets.get(slot.handle()) {
                    Some(src) => ready.push((entry, src.0.clone())),
                    None => still_pending.push(entry),
                }
            }
        },
    );
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
        // Resolved before borrowing `AudioWorld`, since finding the entity
        // needs the world too.
        let emitter_entity = entry.emitter.as_ref().and_then(|name| {
            let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
            let found = q.iter(world).find(|(_, n)| n.0 == *name).map(|(e, _)| e);
            if found.is_none() {
                tracing::warn!(
                    "playSound3D named entity '{name}', which does not exist —                      playing it without a position"
                );
            }
            found
        });
        if let Some(mut audio) = world.get_resource_mut::<AudioWorld>() {
            // `play_at` returns None when the entity has no spatial track yet,
            // which is the normal state before a listener exists. Falling back
            // keeps the sound audible during scene load rather than dropping
            // it; it is simply not positional for that moment.
            let started = match emitter_entity {
                Some(e) => audio.play_at(e, data.clone()).or_else(|| audio.play(data)),
                None => audio.play(data),
            };
            if let Some(mut handle) = started {
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
        position: Vec3::new(params.x, params.y, params.z).into(),
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
            .map(|(n, t)| (n.0.clone(), (t.position.0, t.rotation.0, t.scale.0)))
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
        // # Where this stops being the right trade, honestly
        //
        // Not "a project that synthesises fresh asset paths at runtime" — that
        // case is pathological, and if it happened the thing to fix would be
        // `AssetStatuses` itself, which would be leaking `AssetInfo`s inside
        // `bevy_asset` too. The realistic threshold is ordinary: a few hundred
        // distinct assets, which is a normal size for a shipping project, pays
        // ~2×N `String` allocations *every frame* for a map that most frames
        // nobody reads. Tens of entries is free; hundreds is a real per-frame
        // cost bought for nothing on the frames no script calls
        // `getAssetStatus`.
        //
        // Fixing it is not just adding an `is_changed()` guard, which is why
        // it is not done here: `bsengine_asset::collect_asset_statuses` calls
        // `by_path.iter_mut()` unconditionally, so `AssetStatuses` is marked
        // changed every single frame and change detection would gate on a
        // signal that is always true. A real fix has to make that collector
        // compare-then-write and `bypass_change_detection()` when nothing
        // moved, and only then can this mirror skip a rebuild.
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

        // The prefix those keys carry and a script cannot otherwise learn.
        // Every key above is `format!("{project_dir}/{path}")`, and
        // `project_dir` is a command-line argument — different for the
        // windowed runtime, the editor and an MCP session, and absolute in the
        // last of those. No op reveals it, so without this the only spelling a
        // script can write down (`assets/sounds/hit.wav`, exactly what it just
        // passed `playSound`) missed the mirror and read `"unknown"`: the one
        // answer that means "nothing ever requested that path".
        // `bsengine_get_asset_status` resolves against this on an exact-key
        // miss, so both spellings answer and neither can shadow the other.
        //
        // Written unconditionally, and beside the map rather than at plugin
        // build time, for the same reason the map is replaced wholesale: two
        // `App`s sharing a thread (this workspace's own test runs, and an
        // editor hosting a game) must not resolve each other's paths against
        // the wrong project, and a host with no `ProjectDir` at all must clear
        // whatever the last one left here.
        PROJECT_DIR.with(|pd| {
            *pd.borrow_mut() = world
                .get_resource::<ProjectDir>()
                .cloned()
                .unwrap_or_default()
        });
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
        HudTexts, Name, PendingSounds, Script, ScriptLoad, ScriptPath, ScriptRuntimeResource,
        ScriptingPlugin, SoundLoad, SoundLoads, Transform, Vec3,
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

    // ---- scripts as assets (roadmap item 31) ----
    //
    // Both tests drive the real `ScriptingPlugin` — its real `PostStartup`
    // request, its real `Update` poll, a real `AssetServer` and a real file
    // (or real absence of one) on disk. Nothing here calls `load_scripts` or
    // `execute_loaded_scripts` by hand: what changed in this item is *when*
    // those two run relative to each other and to `run_scripts`, which a
    // hand-driven call cannot observe at all.

    /// A script must actually **run**, not merely have arrived — and it must
    /// run before the very first `_runAll` that could have called its
    /// `onUpdate`.
    ///
    /// Asserted through `setHudText` rather than through the `Script`
    /// component, for two independent reasons. A component only proves the
    /// bytes got here; `HudTexts` proves V8 evaluated the file, registered
    /// `onUpdate` under this entity's id, and that `_runAll` then found it
    /// under that id — the whole chain the entity-keyed IIFE exists for. And
    /// it proves `BOOTSTRAP_JS` ran first: `Bsengine` is defined nowhere else,
    /// so a script executing before the bootstrap would die on `Bsengine is
    /// not defined` and set no HUD text at all.
    ///
    /// The sharp part is *where* the assertion is made: on the first frame the
    /// `Script` component exists, the HUD text must **already** be set. That
    /// is the `execute_loaded_scripts` -> `run_scripts` edge and nothing else.
    /// Drop the edge and the systems sort into some order the schedule picks;
    /// if `run_scripts` sorts first, the script is executed after that frame's
    /// `_runAll` and the HUD text appears one frame later than the component —
    /// a silently skipped first `onUpdate`, which for a script that does its
    /// setup there is a bug that survives to the next scene load.
    #[test]
    fn a_script_runs_on_the_frame_its_source_arrives() {
        let script_path = std::env::temp_dir().join(format!(
            "bsengine_test_script_asset_runs_{}.js",
            std::process::id()
        ));
        std::fs::write(
            &script_path,
            "function onUpdate(name) { Bsengine.setHudText(\"ran\", name); }",
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: String::new(),
        });
        let entity = app
            .world_mut()
            .spawn((
                Name("Hero".to_string()),
                ScriptPath(script_path.to_string_lossy().to_string()),
            ))
            .id();

        // Framed as "run until it arrives", never as a fixed frame count: how
        // many frames a load takes is `bevy_asset`'s business, and pinning a
        // number here would turn a slower filesystem — or `bevy_tasks` being
        // built with `multi-threaded` on, which makes the load genuinely
        // concurrent instead of a blocking `spawn` — into a test failure that
        // says nothing about this code.
        let mut frames = 0;
        loop {
            app.update();
            frames += 1;
            if app.world().get::<Script>(entity).is_some() {
                break;
            }
            assert!(
                frames < 300,
                "the script source never arrived, so nothing here was measured"
            );
        }

        assert_eq!(
            app.world().resource::<HudTexts>().0.get("ran").cloned(),
            Some("Hero".to_string()),
            "on the frame a script's source arrives, its onUpdate must already \
             have been called: `execute_loaded_scripts` has to run before \
             `run_scripts`, and the entity's registration has to be keyed by \
             the id `_runAll` looks it up under"
        );
        assert!(
            app.world()
                .get::<ScriptLoad>(entity)
                .is_some_and(|l| l.slot.is_ready()),
            "an executed script must be recorded as Ready, holding the handle \
             that keeps the source tracked"
        );

        let _ = std::fs::remove_file(&script_path);
    }

    /// A `.js` path that cannot load must be given up on: warned about once,
    /// and then never asked for again.
    ///
    /// The mutation this exists to catch is re-requesting the path from the
    /// poll instead of reading the retained handle. `AssetServer::load` on a
    /// path whose state is `Failed` resets it to `Loading` and respawns the
    /// load (`bevy_asset` 0.14.2, `server/info.rs:212-221`), and `Failed` is
    /// set in `PreUpdate` while the poll runs in `Update` — so a re-requesting
    /// poll never observes the failure at all. It does not warn *too much*, it
    /// warns **not at all**, while spawning a filesystem task every frame
    /// forever. Both assertions below are written to catch that: the warning
    /// count is `== 1` (a re-requesting version scores 0), and the terminal
    /// state must be `GaveUp` (a re-requesting version is stuck in `Loading`).
    ///
    /// 200 frames because that is the length Phase 2 measured the naive shape
    /// at — 200 errors over 200 frames against one.
    #[test]
    fn a_script_that_cannot_load_is_given_up_on_and_warned_about_once() {
        const MISSING: &str = "definitely/not/a/real/script.js";

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: String::new(),
        });
        let entity = app
            .world_mut()
            .spawn((Name("Ghost".to_string()), ScriptPath(MISSING.to_string())))
            .id();

        let (_, logs) = capture_logs(|| {
            for _ in 0..200 {
                app.update();
            }
        });

        assert_eq!(
            logs.matches("[scripting] giving up on").count(),
            1,
            "a script that cannot load must be reported exactly once — zero \
             means the poll re-requested the path and erased the failure before \
             it could be seen, more than one means it is being retried. \
             Captured logs were:\n{logs}"
        );
        assert!(
            logs.contains(MISSING),
            "the warning must name the path that could not be loaded, or a \
             project with fifty scripts learns only that one of them is bad. \
             Captured logs were:\n{logs}"
        );
        assert!(
            app.world()
                .get::<ScriptLoad>(entity)
                .is_some_and(|l| l.slot.gave_up()),
            "the entity must end up recorded as given-up on; still `Loading` \
             after 200 frames means the load is being restarted every frame"
        );
        assert!(
            app.world().get::<Script>(entity).is_none(),
            "a script that never loaded must not leave a Script component \
             behind, or `_runAll` is handed an entity with no registration"
        );
    }

    // ---- script hot reload (roadmap item 31) ----
    //
    // Both tests below drive the whole chain rather than any part of it: a real
    // `AssetWatcherPlugin` over a real project directory, a real file rewritten
    // on disk while frames are running, and the real `ScriptingPlugin`. Nothing
    // calls `AssetServer::reload` by hand and nothing calls
    // `reexecute_modified_scripts` by hand -- the two mistakes this effort keeps
    // finding are a reload that never dispatches and a reload that dispatches
    // and is never *run*, and each of those shortcuts hides one of them.
    //
    // What is asserted is always the script's **behaviour**, through
    // `setHudText`, never that a reload was logged. A test that looked for the
    // log line would pass against an implementation that re-read the file and
    // then did nothing with the new source, which is precisely the silent
    // failure worth guarding.

    /// Hard ceiling on every wait here. A hung test in CI is far worse than a
    /// failing one, so nothing in these tests ever blocks unbounded; the value
    /// matches `bsengine_asset::watcher`'s own probes.
    const HARD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

    /// The watcher's debounce window, used here only as the scale of "long
    /// enough for the OS backend to have started delivering". Copied rather
    /// than shared because `watcher::DEBOUNCE` is private to that module and
    /// its exact value is not this test's subject.
    const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(200);

    /// Runs frames until `done`, or panics naming `what` after
    /// [`HARD_TIMEOUT`]. Bounded by wall clock rather than by a frame count
    /// because what is waited on is a filesystem notification delivered by
    /// another thread on the OS's schedule, not a fixed amount of work.
    fn run_until(
        app: &mut bevy_app::App,
        what: &str,
        mut done: impl FnMut(&bevy_app::App) -> bool,
    ) {
        let deadline = std::time::Instant::now() + HARD_TIMEOUT;
        while std::time::Instant::now() < deadline {
            app.update();
            if done(app) {
                return;
            }
            // Yield rather than spin: the thing being waited on happens on
            // another thread.
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("{what} did not happen within {HARD_TIMEOUT:?}");
    }

    /// What `setHudText(name, phase)` last recorded for the entity called
    /// `who` — the observable both tests below are written against, because it
    /// can only be set by JavaScript that actually ran.
    fn phase(app: &bevy_app::App, who: &str) -> Option<String> {
        app.world().resource::<HudTexts>().0.get(who).cloned()
    }

    /// A project directory under the process CWD (the crate root, under
    /// cargo), holding an `assets/scripts/` for the probe script.
    ///
    /// Relative, and under the CWD, for the same two reasons the watcher's own
    /// tests need it: `bevy_asset`'s root here *is* the working directory, so a
    /// path outside it is not addressable as an asset path at all — and a
    /// relative `ProjectDir` is the shape the engine really uses, the one whose
    /// reconstruction from `notify`'s absolutised path can actually go wrong.
    /// `.gitignore` covers the name `unique` mints.
    fn script_probe(
        tag: &str,
    ) -> (
        String,
        std::path::PathBuf,
        bsengine_asset::test_support::ProbeDir,
    ) {
        let project = bsengine_asset::test_support::unique(tag);
        let root = std::path::PathBuf::from(&project);
        std::fs::create_dir_all(root.join("assets").join("scripts"))
            .expect("create the probe's assets/scripts");
        let guard = bsengine_asset::test_support::ProbeDir(root.clone());
        (project, root, guard)
    }

    /// An app with the watcher and scripting wired together exactly as a host
    /// wires them. `ScriptingPlugin` is what inserts `ProjectDir`, and it does
    /// so at build time, so the watcher's `Startup` system finds it.
    fn app_with_watcher(project: &str) -> bevy_app::App {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(bsengine_asset::AssetWatcherPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: project.to_string(),
        });
        app
    }

    /// The property this whole item exists for: a script edited on disk while
    /// the game is running does something **different**, without a restart.
    ///
    /// Two entities share the one script file, which is not decoration. The
    /// wrapper keys its registration by *entity* — `Bsengine._scripts["<entity
    /// bits>"]` — so a reload has to re-run the source once per entity holding
    /// the handle. An implementation that re-ran it for the first match and
    /// stopped, or that keyed anything by path, leaves the second entity
    /// running the previous revision, and does so silently: the log would show
    /// one reload and the game would show one stale character.
    ///
    /// The two revisions differ only in a string neither file shares, so the
    /// final assertion cannot be satisfied by the original source still
    /// running, by the reload merely having been announced, or by the new
    /// source having been *read* without being evaluated.
    #[test]
    fn editing_a_script_while_the_game_runs_changes_what_it_does() {
        const BEFORE: &str = "function onUpdate(name) { Bsengine.setHudText(name, \"before\"); }";
        const AFTER: &str = "function onUpdate(name) { Bsengine.setHudText(name, \"after\"); }";

        let (project, root, _guard) = script_probe("script-reload");
        let script = root.join("assets").join("scripts").join("hero.js");
        std::fs::write(&script, BEFORE).unwrap();

        let mut app = app_with_watcher(&project);
        let hero = app
            .world_mut()
            .spawn((
                Name("Hero".to_string()),
                ScriptPath("assets/scripts/hero.js".to_string()),
            ))
            .id();
        let sidekick = app
            .world_mut()
            .spawn((
                Name("Sidekick".to_string()),
                ScriptPath("assets/scripts/hero.js".to_string()),
            ))
            .id();

        run_until(&mut app, "both entities ran the original script", |app| {
            phase(app, "Hero").as_deref() == Some("before")
                && phase(app, "Sidekick").as_deref() == Some("before")
        });

        // Let the OS backend actually begin delivering before the edit. A write
        // that lands before the watch is live is a write nothing reports, and
        // the failure would look identical to a reload that never took effect.
        std::thread::sleep(DEBOUNCE * 3);

        // The edit an author would make, mid-session.
        std::fs::write(&script, AFTER).unwrap();

        run_until(
            &mut app,
            "the edited script's new behaviour ran (a reload that re-read the \
             file without re-evaluating it fails exactly here, having logged \
             everything a working one logs)",
            |app| {
                phase(app, "Hero").as_deref() == Some("after")
                    && phase(app, "Sidekick").as_deref() == Some("after")
            },
        );

        for (entity, who) in [(hero, "Hero"), (sidekick, "Sidekick")] {
            assert_eq!(
                app.world().get::<Script>(entity).map(|s| s.source.clone()),
                Some(AFTER.to_string()),
                "{who} is running the new revision, so the source recorded on it \
                 must be the new revision too -- a stale `Script` is how a later \
                 reader concludes the wrong thing about what is running"
            );
            assert!(
                app.world()
                    .get::<ScriptLoad>(entity)
                    .is_some_and(|l| l.slot.is_ready()),
                "{who} must still be Ready after a reload, holding the handle \
                 that makes the *next* edit reloadable too"
            );
        }
    }

    /// A script that could not be loaded at all, then fixed on disk, starts
    /// working — rather than staying given-up-on for the rest of the session.
    ///
    /// This is the case `ScriptLoad` keeps its handle for even when the load
    /// failed, and it is worth a test of its own because it does **not** arrive
    /// as `AssetEvent::Modified`: the failed load inserted nothing, so the load
    /// that finally succeeds is an `Added`. A reload handler that matched only
    /// `Modified` would leave a single mistyped filename permanent until the
    /// game was restarted, while looking completely correct for every ordinary
    /// edit — which is the half of hot reload nobody tests by accident.
    ///
    /// `execute_loaded_scripts` cannot be what recovers this: it deliberately
    /// never revisits `GaveUp`, because re-requesting a `Failed` path resets it
    /// to `Loading` and erases the very failure it was polling for.
    #[test]
    fn a_script_given_up_on_starts_working_once_the_file_appears() {
        const FIXED: &str = "function onUpdate(name) { Bsengine.setHudText(name, \"fixed\"); }";

        let (project, root, _guard) = script_probe("script-recover");
        // Deliberately absent: the entity names a script that is not there.
        let script = root.join("assets").join("scripts").join("late.js");

        let mut app = app_with_watcher(&project);
        let entity = app
            .world_mut()
            .spawn((
                Name("Ghost".to_string()),
                ScriptPath("assets/scripts/late.js".to_string()),
            ))
            .id();

        run_until(&mut app, "the missing script was given up on", |app| {
            app.world()
                .get::<ScriptLoad>(entity)
                .is_some_and(|l| l.slot.gave_up())
        });
        assert!(
            phase(&app, "Ghost").is_none(),
            "nothing can have run yet, or the recovery below would be measuring \
             a script that was working all along"
        );

        std::thread::sleep(DEBOUNCE * 3);
        std::fs::write(&script, FIXED).unwrap();

        run_until(
            &mut app,
            "the script that appeared on disk was loaded and run",
            |app| phase(app, "Ghost").as_deref() == Some("fixed"),
        );
        assert!(
            app.world()
                .get::<ScriptLoad>(entity)
                .is_some_and(|l| l.slot.is_ready()),
            "a recovered script must be recorded as Ready, or the *next* edit to \
             it is filtered out again"
        );
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
                Some(SoundLoad::Requested(slot)) if slot.gave_up()
            ),
            "the failed path must be remembered as a request that gave up -- \
             `Unrequestable` would be the wrong answer here, since the request \
             was made and it is the load that failed -- or the next playSound \
             restarts the load and the failure is never observed"
        );
        assert!(
            logs.contains("[audio] failed to load queued sound"),
            "the developer must be told the sound could not be loaded; captured logs were:\n{logs}"
        );
        // The `PlaySound` handler's own refusal, which is a different line from
        // the one above and reached on a different frame: once the path is
        // known-bad, later plays are turned away at the command rather than
        // queued and dropped a frame later. Nothing else measures that guard --
        // removing it leaves every other assertion here green, because
        // `start_pending_sounds` drops the entry either way.
        assert!(
            logs.contains("[audio] not playing") && logs.contains("its load already failed"),
            "a play on an already-failed path must be refused at the command \
             handler, and say so; captured logs were:\n{logs}"
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
            Some(SoundLoad::Requested(slot)) if slot.is_ready() => slot.handle().clone(),
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
                Some(SoundLoad::Requested(slot)) if slot.is_ready()
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
            let SoundLoad::Requested(slot) = load else {
                unreachable!("the loop above only exits on a requested load, got {load:?}")
            };
            assert!(
                slot.is_ready(),
                "the loop above only exits on Ready, got {slot:?}"
            );
            (path.clone(), slot.handle().id())
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
        let handle = match loads.0.get(&resolved_path) {
            Some(SoundLoad::Requested(slot)) if slot.is_ready() => slot.handle(),
            other => panic!(
                "a reload must leave the path Ready, not evict or downgrade it; \
                 got {other:?}"
            ),
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

    // The Critical regression: the spelling a script uses to *request* an
    // asset must be a spelling it can use to *ask about* one.
    //
    // Every key in `AssetStatuses` is `format!("{project_dir}/{path}")`, and
    // `project_dir` is a command-line argument — different for the windowed
    // runtime, the editor and an MCP session. No op hands a script that
    // prefix, so `getAssetStatus("assets/sounds/blip.flac")` — the same string
    // the same script just gave `playSound` — used to read `"unknown"`, which
    // means "nothing ever requested that path". That is the exact ambiguity
    // this phase exists to remove, answered wrongly to every script author.
    //
    // The sibling MCP surface was fixed first (`bsengine-runtime`'s
    // `test_query::get_asset_status`); this is the same fix on the JS surface,
    // and the same three assertions: the short spelling answers, the
    // fully-qualified one still answers the same thing, and a path nothing
    // requested still reads `"unknown"`.
    //
    // # Why it asserts a *successful* load
    //
    // A failure would prove much less. `UntypedAssetLoadFailedEvent` puts a
    // path into `AssetStatuses` on its own, so a `failed:` answer is reachable
    // without `record_asset_request` ever having run — whereas a `"loaded"` is
    // only ever reachable through the recorded request, which is the half
    // `bevy_asset` provides no other way to get (see
    // `bsengine_asset::status`). A real FLAC on disk under a real project
    // directory is therefore the harder and more honest case.
    //
    // # Why `project_dir` is non-empty and that matters
    //
    // `resolve_project_path` with an empty `ProjectDir` returns its argument
    // unchanged, so an empty one would make the two spellings the same string
    // and this test vacuous. It is a real temp directory here, with a
    // project-relative `ScriptPath` beneath it — the same arrangement
    // `set_shader_resolves_path_against_project_dir` uses, and the same one a
    // real game has.
    #[test]
    fn get_asset_status_accepts_the_project_relative_path_a_script_played() {
        // Project-relative, as a script would write it; never the key.
        const PLAYED: &str = "assets/sounds/blip.flac";
        // Same shape, under the same project, but nothing asks for it.
        const NEVER_ASKED: &str = "assets/sounds/never-asked.flac";

        let project_dir = std::env::temp_dir().join(format!(
            "bsengine_test_asset_status_relative_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(project_dir.join("assets/sounds")).unwrap();
        std::fs::write(project_dir.join(PLAYED), MINIMAL_FLAC_SILENCE).unwrap();

        // Forward-slashed, the way a project directory arrives on the command
        // line, so the key below is the same shape a real host produces.
        // (Backslashes would also be escapes inside the JS string literal.)
        let project_dir_js = project_dir.to_string_lossy().replace('\\', "/");
        let qualified = format!("{project_dir_js}/{PLAYED}");

        // Played once, not every frame: re-requesting resets a load, and this
        // test waits for one to settle.
        std::fs::write(
            project_dir.join("probe.js"),
            format!(
                "var played = false;\n\
                 function onUpdate(name) {{\n\
                   if (!played) {{\n\
                     played = true;\n\
                     Bsengine.playSound(\"{PLAYED}\", {{}});\n\
                   }}\n\
                   Bsengine.setHudText(\"relative\", Bsengine.getAssetStatus(\"{PLAYED}\"));\n\
                   Bsengine.setHudText(\"qualified\", Bsengine.getAssetStatus(\"{qualified}\"));\n\
                   Bsengine.setHudText(\"never\", Bsengine.getAssetStatus(\"{NEVER_ASKED}\"));\n\
                 }}"
            ),
        )
        .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(bsengine_asset::AssetStatusPlugin);
        app.add_plugins(ScriptingPlugin {
            project_dir: project_dir_js.clone(),
        });
        app.world_mut().spawn((
            Name("Probe".to_string()),
            ScriptPath("probe.js".to_string()),
        ));

        // A hang guard, not a budget: the load is one 86-byte local file.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let (relative, qualified_status, never_asked) = loop {
            app.update();
            let (relative, qualified_status, never_asked) = {
                let hud = &app.world().resource::<HudTexts>().0;
                (
                    hud.get("relative").cloned().unwrap_or_default(),
                    hud.get("qualified").cloned().unwrap_or_default(),
                    hud.get("never").cloned().unwrap_or_default(),
                )
            };

            // Checked every frame rather than only at the end. Both are
            // properties of the answer at *every* moment, and a fix that held
            // only once the load had settled would still be wrong while it was
            // in flight — which is exactly when a script polls.
            if !never_asked.is_empty() {
                assert_eq!(
                    never_asked, "unknown",
                    "resolving a project-relative path must not make every path \
                     answerable: a path nothing requested must read \"unknown\" on \
                     every frame"
                );
                assert_eq!(
                    relative, qualified_status,
                    "the short spelling and the engine's own key name one asset, so \
                     they must never disagree — not even mid-load"
                );
            }

            if relative == "loaded" {
                break (relative, qualified_status, never_asked);
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the project-relative spelling never reported the load. \"{relative}\" \
                 for {PLAYED} under {project_dir_js}; the fully-qualified key read \
                 \"{qualified_status}\". (An \"unknown\" here is the defect: the \
                 script requested this very path. A \"failed:\" here instead means \
                 MINIMAL_FLAC_SILENCE stopped decoding.)"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        };

        assert_eq!(
            qualified_status, "loaded",
            "resolution adds a spelling, it does not swap one for another: the \
             engine's own fully-qualified key must keep answering"
        );
        assert_eq!(
            never_asked, "unknown",
            "a path nothing requested must still read \"unknown\" once the other \
             one has loaded"
        );
        assert_ne!(
            relative, never_asked,
            "if a requested path and an unrequested one read the same, the op \
             answers nothing"
        );

        let _ = std::fs::remove_dir_all(&project_dir);
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
            Transform::from_position(Vec3::new(5.0, 0.0, 5.0)),
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
        assert!((t.position.x - 7.0).abs() < 1e-4, "x: {}", t.position.x);
        assert!((t.position.z - 9.0).abs() < 1e-4, "z: {}", t.position.z);

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

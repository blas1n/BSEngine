//! Scene/project loading systems shared by the real-time runtime (`main.rs`)
//! and the headless test runtime (`test_mode.rs`), so both run identical
//! scene-load and physics-resolution behavior.

use bevy_app::{App, PostStartup, Update};
use bevy_ecs::prelude::{IntoSystemConfigs, World};
use bsengine_core::{HudTexts, Transform};
use bsengine_ecs::{Added, Commands, Entity, Query, ResMut};
use bsengine_physics::{Collider, PhysicsInput, RigidBody};
use bsengine_rhi_wgpu::{
    capsule_vertices, cube_vertices, cylinder_vertices, plane_vertices, sphere_vertices,
    GpuMeshRegistry,
};
use bsengine_scene::{
    spawn_scene_entities, ColliderShapeDesc, Name, PendingSceneLoad, PhysicsBodyDesc, Primitive,
    PrimitiveMesh, RigidBodyDesc, SceneDescriptor,
};
use bsengine_scripting::{load_scripts, SoundHandles};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ProjectManifest {
    pub project: ProjectSection,
    #[serde(default)]
    pub window: WindowSection,
    #[serde(default)]
    pub render: RenderSection,
    #[serde(default)]
    pub package: PackageSection,
}

#[derive(Deserialize)]
pub struct ProjectSection {
    pub name: String,
    pub entry_scene: String,
}

#[derive(Deserialize)]
pub struct WindowSection {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_true")]
    pub resizable: bool,
}

// Not `#[derive(Default)]`: a derived impl gives every field its type's
// zero value (width/height: 0, resizable: false), which disagrees with the
// field-level `#[serde(default = "default_width")]` etc. above. The two
// have to agree, because `ProjectManifest.window` is itself
// `#[serde(default)]` -- a `project.toml` that omits `[window]` entirely
// falls back to this impl, while one with a present-but-empty `[window]`
// table falls back to the field-level defaults. Reusing the same functions
// here is what makes "table absent" and "table empty" mean the same thing.
impl Default for WindowSection {
    fn default() -> Self {
        Self {
            title: None,
            width: default_width(),
            height: default_height(),
            resizable: default_true(),
        }
    }
}

/// Rendering switches from `project.toml`'s `[render]` table.
#[derive(Deserialize)]
pub struct RenderSection {
    /// Whether to cull entities hidden behind `Occluder`s. On by default,
    /// matching frustum culling, which has always run unconditionally.
    #[serde(default = "default_true")]
    pub occlusion_culling: bool,
}

// Same rule as `WindowSection` above, and for the same reason: because
// `ProjectManifest.render` is itself `#[serde(default)]`, this impl and
// the field-level `#[serde(default = "...")]` must call the same function,
// or an absent `[render]` table and an empty one would disagree.
impl Default for RenderSection {
    fn default() -> Self {
        Self {
            occlusion_culling: default_true(),
        }
    }
}

/// Packaging switches from `project.toml`'s `[package]` table.
///
/// Unlike [`WindowSection`] and [`RenderSection`] this one takes a plain
/// `#[derive(Default)]`: its only field's zero value (an empty list) is exactly
/// what an absent table should mean, so there is no field-level default for a
/// hand-written impl to have to agree with.
#[derive(Deserialize, Default)]
pub struct PackageSection {
    /// Assets to include in a build that no scene reaches.
    ///
    /// A path a script *builds* rather than spells is invisible to `--package`'s
    /// static walk, and so is a file a build needs for reasons the engine knows
    /// nothing about — the attribution notice a model's licence requires being
    /// the usual one. Listing it here is how those get in. Each entry is
    /// followed like any other reference, so naming a scene brings in what that
    /// scene references, and an entry that resolves to nothing fails the build:
    /// this list is written on purpose, so a typo in it is a mistake, not a
    /// guess.
    #[serde(default)]
    pub extra_assets: Vec<String>,
    /// Whether a build's assets are loose files or one archive.
    ///
    /// Defaults to loose, so upgrading the engine never changes what an
    /// existing project's build looks like.
    #[serde(default)]
    pub mode: bsengine_asset::cook::PackageMode,
}

fn default_width() -> u32 {
    1280
}
fn default_height() -> u32 {
    720
}
fn default_true() -> bool {
    true
}

/// Registers the scene-load/physics-resolution systems shared by every
/// runtime entry point (windowed and headless).
pub fn register_scene_systems(app: &mut App) {
    app.add_systems(
        PostStartup,
        (resolve_primitives, resolve_physics_bodies).chain(),
    )
    .add_systems(Update, handle_scene_load)
    .add_systems(Update, resolve_primitives.after(handle_scene_load))
    .add_systems(Update, resolve_physics_bodies.after(resolve_primitives));
}

pub fn resolve_primitives(
    query: Query<(Entity, &PrimitiveMesh), Added<PrimitiveMesh>>,
    mut commands: Commands,
    registry: Option<ResMut<GpuMeshRegistry>>,
) {
    let Some(mut registry) = registry else { return };

    let mut cube_id: Option<u64> = None;
    let mut sphere_id: Option<u64> = None;
    let mut plane_id: Option<u64> = None;
    let mut capsule_id: Option<u64> = None;
    let mut cylinder_id: Option<u64> = None;

    for (entity, prim) in query.iter() {
        let mesh_id = match &prim.0 {
            Primitive::Cube => *cube_id.get_or_insert_with(|| {
                let (v, i) = cube_vertices();
                registry.register(&v, &i)
            }),
            Primitive::Sphere => *sphere_id.get_or_insert_with(|| {
                let (v, i) = sphere_vertices();
                registry.register(&v, &i)
            }),
            Primitive::Plane => *plane_id.get_or_insert_with(|| {
                let (v, i) = plane_vertices();
                registry.register(&v, &i)
            }),
            Primitive::Capsule => *capsule_id.get_or_insert_with(|| {
                let (v, i) = capsule_vertices();
                registry.register(&v, &i)
            }),
            Primitive::Cylinder => *cylinder_id.get_or_insert_with(|| {
                let (v, i) = cylinder_vertices();
                registry.register(&v, &i)
            }),
        };
        commands
            .entity(entity)
            .insert(bsengine_render::MeshRenderer { mesh_id });
    }
}

pub fn resolve_physics_bodies(
    query: Query<(Entity, &PhysicsBodyDesc), Added<PhysicsBodyDesc>>,
    transforms: Query<&Transform>,
    mut commands: Commands,
) {
    for (entity, desc) in query.iter() {
        let mut rb = match desc.rigidbody {
            RigidBodyDesc::Dynamic => RigidBody::dynamic(),
            RigidBodyDesc::Static => RigidBody::fixed(),
            RigidBodyDesc::Kinematic => RigidBody::kinematic(),
        };
        if let Some(d) = desc.linear_damping {
            rb.linear_damping = d;
        }
        if let Some(d) = desc.angular_damping {
            rb.angular_damping = d;
        }
        let col_base = match &desc.collider.shape {
            ColliderShapeDesc::Box { hx, hy, hz } => Collider::cuboid(*hx, *hy, *hz),
            ColliderShapeDesc::Sphere { radius } => Collider::ball(*radius),
            ColliderShapeDesc::Capsule {
                half_height,
                radius,
            } => Collider::capsule(*half_height, *radius),
        };
        let col = col_base
            .with_restitution(desc.collider.restitution)
            .with_friction(desc.collider.friction)
            .with_sensor(desc.collider.sensor);
        let t = transforms.get(entity).cloned().unwrap_or_default();
        commands.entity(entity).insert((
            rb,
            col,
            PhysicsInput {
                position: t.position,
                rotation: t.rotation,
            },
        ));
    }
}

pub fn handle_scene_load(world: &mut World) {
    let pending = world.remove_resource::<PendingSceneLoad>();
    let Some(pending) = pending else { return };

    // Through the archive when this is a packaged build, from disk otherwise.
    // This is the site a script's `loadScene` reaches, so a second level of a
    // packaged game arrives here and nowhere else.
    let content = match bsengine_asset::pak_source::read_to_string(&pending.path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("[scene] failed to read {}: {e}", pending.path);
            return;
        }
    };
    let scene: SceneDescriptor = match ron::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("[scene] failed to parse {}: {e}", pending.path);
            return;
        }
    };

    // Stop all sounds and clear handles
    if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
        for (_, mut handle) in handles.0.drain() {
            handle.stop(kira::Tween::default());
        }
    }

    // Despawn all named entities
    let named: Vec<Entity> = {
        let mut q = world.query::<(Entity, &Name)>();
        q.iter(world).map(|(e, _)| e).collect()
    };
    for e in named {
        world.despawn(e);
    }

    // Clear HUD
    if let Some(mut hud) = world.get_resource_mut::<HudTexts>() {
        hud.0.clear();
    }

    // Script state (Bsengine._scripts, timers, collision/message handlers,
    // ...) is reset below by re-running BOOTSTRAP_JS via load_scripts, which
    // replaces the whole `Bsengine` object. This deliberately reuses the
    // existing ScriptRuntime/V8 isolate rather than constructing a new one:
    // creating a second V8 isolate while EditorPlugin's stack is active
    // corrupts V8's isolate state (crashes with "Cannot create a handle
    // without a HandleScope" the moment a script next runs) — see
    // BOOTSTRAP_JS's `var Bsengine` comment for the JS-side half of this fix.

    // Spawn scene and resolve physics inline (Added<> won't fire for same-frame spawns)
    spawn_scene_entities(world, &scene.entities);
    resolve_physics_bodies_world(world);
    // Requests the new entities' scripts; it does not run them. Scripts are
    // `bevy_asset` assets, and `bevy_asset` publishes finished loads from
    // `PreUpdate` — which this frame has already passed, since this system is
    // in `Update` — so the new scene's scripts execute in
    // `bsengine_scripting`'s `execute_loaded_scripts` on the *next* frame.
    // Measured at exactly one frame; pinned by this module's
    // `a_script_loaded_scene_gets_its_own_scripts_running`.
    //
    // What that costs, stated plainly, because it used to be atomic: between
    // this frame and the one where the new scripts execute, no entity has a
    // `Script` component, so `run_scripts` early-returns and `Bsengine._runAll`
    // is not called at all. For those few frames script timers do not tick,
    // key/mouse/gamepad edge events are not dispatched to JS, and collisions
    // are not delivered. The old scene's handlers are already gone (the
    // bootstrap above wiped them and every named entity was despawned), so
    // nothing from the dead scene leaks into the new one — the gap is silence,
    // not stale behaviour, and it lasts as long as reading the scripts off
    // disk takes.
    //
    // What is still atomic is the reset itself: `load_scripts` runs
    // BOOTSTRAP_JS synchronously before it requests anything, so the despawn
    // and the JS-side clear land on the same frame, which is what the comment
    // above is about.
    load_scripts(world);
}

pub fn resolve_physics_bodies_world(world: &mut World) {
    let entities: Vec<(Entity, PhysicsBodyDesc)> = {
        let mut q = world.query::<(Entity, &PhysicsBodyDesc)>();
        q.iter(world).map(|(e, d)| (e, d.clone())).collect()
    };
    for (entity, desc) in entities {
        let rb = match desc.rigidbody {
            RigidBodyDesc::Dynamic => RigidBody::dynamic(),
            RigidBodyDesc::Static => RigidBody::fixed(),
            RigidBodyDesc::Kinematic => RigidBody::kinematic(),
        };
        let col_base = match desc.collider.shape {
            ColliderShapeDesc::Box { hx, hy, hz } => Collider::cuboid(hx, hy, hz),
            ColliderShapeDesc::Sphere { radius } => Collider::ball(radius),
            ColliderShapeDesc::Capsule {
                half_height,
                radius,
            } => Collider::capsule(half_height, radius),
        };
        let col = col_base
            .with_restitution(desc.collider.restitution)
            .with_friction(desc.collider.friction)
            .with_sensor(desc.collider.sensor);
        let t = world.get::<Transform>(entity).cloned().unwrap_or_default();
        world.entity_mut(entity).insert((
            rb,
            col,
            PhysicsInput {
                position: t.position,
                rotation: t.rotation,
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::Name;
    use bsengine_core::HudTexts;

    /// `ProjectManifest.window` is `#[serde(default)]`, so a `project.toml`
    /// that omits `[window]` entirely (every hand-written fixture in this
    /// file and most of `test_mode.rs`'s do) falls back to
    /// `WindowSection::default()` rather than the field-level
    /// `#[serde(default = "default_width")]`/etc. those two must agree, or
    /// "no `[window]` table" silently means something different from
    /// "`[window]` table present but empty" -- which is exactly what
    /// happened when `WindowSection` derived its `Default` instead of
    /// defining one: the derived impl gives `width: 0, height: 0`, and once
    /// `build_test_app` started feeding those straight into
    /// `WgpuRHIPlugin::offscreen`, every minimal `project.toml` in this
    /// codebase's own tests crashed wgpu with "Dimension X is zero".
    #[test]
    fn window_section_defaults_to_1280x720_when_window_table_is_absent() {
        let manifest: super::ProjectManifest =
            toml::from_str("[project]\nname = \"Test\"\nentry_scene = \"assets/scenes/a.ron\"\n")
                .expect("a project.toml with no [window] table must still parse");
        assert_eq!(
            manifest.window.width, 1280,
            "width must fall back to the same default a present-but-empty \
             [window] table gets, not the derived Default's 0"
        );
        assert_eq!(
            manifest.window.height, 720,
            "height must fall back to the same default a present-but-empty \
             [window] table gets, not the derived Default's 0"
        );
    }

    /// A two-scene project where scene A's script chains to scene B on its
    /// first `onUpdate`, and each scene's script announces itself through the
    /// HUD. That is `games/tilt-run`'s level chain in miniature — the shape
    /// whose atomicity `handle_scene_load` gave up when scripts became assets.
    fn chained_scene_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
        std::fs::create_dir_all(root.join("assets/scripts")).unwrap();
        std::fs::write(
            root.join("project.toml"),
            "[project]\nname = \"Chain\"\nentry_scene = \"assets/scenes/a.ron\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("assets/scripts/a.js"),
            "var chained = false;\n\
             function onUpdate(name) {\n\
               Bsengine.setHudText(\"a\", \"ran\");\n\
               if (!chained) { chained = true; Bsengine.loadScene(\"assets/scenes/b.ron\"); }\n\
             }",
        )
        .unwrap();
        std::fs::write(
            root.join("assets/scripts/b.js"),
            "function onUpdate(name) { Bsengine.setHudText(\"b\", \"ran\"); }",
        )
        .unwrap();
        std::fs::write(
            root.join("assets/scenes/a.ron"),
            r#"SceneDescriptor(entities: [EntityDescriptor(name: "A", script: Some("assets/scripts/a.js"))])"#,
        )
        .unwrap();
        std::fs::write(
            root.join("assets/scenes/b.ron"),
            r#"SceneDescriptor(entities: [EntityDescriptor(name: "B", script: Some("assets/scripts/b.js"))])"#,
        )
        .unwrap();
        dir
    }

    /// A scene loaded from a script must end up with *its* scripts running,
    /// promptly, without anything asking again.
    ///
    /// This is the property `handle_scene_load` used to get for free by
    /// reading each file inline: "the scene is spawned" and "its scripts are
    /// running" were one step. They are two now — `load_scripts` requests, and
    /// `bsengine_scripting`'s poll executes once `bevy_asset` has the source —
    /// and `games/tilt-run` chains five levels through exactly this path, with
    /// its recordings asserting on what the *next* level's scripts do.
    ///
    /// The bound is deliberately loose and deliberately finite. Loose, because
    /// how many frames a load takes is `bevy_asset`'s business and this test
    /// is not the place to pin it (`bevy_tasks` built with `multi-threaded`
    /// would make it genuinely concurrent rather than a blocking `spawn`).
    /// Finite, because the failure this guards against is not slowness — it is
    /// the new script never running at all, which is what a poll that dropped
    /// the handle, or a request that never happened, would look like.
    #[test]
    fn a_script_loaded_scene_gets_its_own_scripts_running() {
        let dir = chained_scene_project();
        let mut app = crate::test_mode::build_test_app(dir.path().to_str().unwrap(), None, false);

        let mut swapped_at = None;
        let mut b_ran_at = None;
        for frame in 1..=60u32 {
            app.update();
            let swapped = {
                let world = app.world_mut();
                let mut q = world.query::<&Name>();
                q.iter(world).any(|n| n.0 == "B")
            };
            if swapped && swapped_at.is_none() {
                swapped_at = Some(frame);
            }
            if app.world().resource::<HudTexts>().0.contains_key("b") {
                b_ran_at = Some(frame);
                break;
            }
        }

        let swapped_at = swapped_at.expect(
            "scene B was never spawned, so nothing about its scripts was measured — \
             the chain itself is broken, not the script loading",
        );
        let b_ran_at = b_ran_at.expect(
            "scene B spawned but its script never ran: a scene loaded from a script \
             must get its own scripts running, which is what `handle_scene_load` \
             used to guarantee synchronously and now delegates to the asset poll",
        );
        assert!(
            b_ran_at >= swapped_at,
            "sanity: the new scene's script cannot run before the scene exists \
             (spawned frame {swapped_at}, ran frame {b_ran_at})"
        );
        // The gap this item introduced, pinned rather than described: frames
        // in which the new scene exists and no script is running. It is one
        // today. Growing would mean scripts are waiting on something they did
        // not wait on before, which is worth failing over even though the
        // exact number is `bevy_asset`'s to choose.
        assert!(
            b_ran_at - swapped_at <= 8,
            "the new scene ran with no scripts for {} frames (spawned {swapped_at}, \
             ran {b_ran_at}); that gap is silence — no onUpdate, no script timers, \
             no collision or input dispatch — and it used to be zero",
            b_ran_at - swapped_at
        );
    }

    /// The `[render]` table being absent, present-but-empty, and explicit
    /// must all agree -- the same three-way consistency `WindowSection`'s
    /// Default impl comment above exists to protect.
    #[test]
    fn render_section_defaults_agree_whether_the_table_is_absent_or_empty() {
        let absent: super::ProjectManifest =
            toml::from_str("[project]\nname = \"t\"\nentry_scene = \"s.ron\"\n").unwrap();
        let empty: super::ProjectManifest =
            toml::from_str("[project]\nname = \"t\"\nentry_scene = \"s.ron\"\n[render]\n").unwrap();
        assert_eq!(
            absent.render.occlusion_culling, empty.render.occlusion_culling,
            "an absent [render] table and an empty one must mean the same thing"
        );
        assert!(
            absent.render.occlusion_culling,
            "occlusion culling defaults to on"
        );
    }

    #[test]
    fn render_section_can_disable_occlusion_culling() {
        let m: super::ProjectManifest = toml::from_str(
            "[project]\nname = \"t\"\nentry_scene = \"s.ron\"\n[render]\nocclusion_culling = false\n",
        )
        .unwrap();
        assert!(!m.render.occlusion_culling);
    }
}

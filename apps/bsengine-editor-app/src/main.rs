use bsengine_app::{new_app, Startup, Update};
use bsengine_asset::{AssetIdentityPlugin, AssetPlugin, AssetStatusPlugin, AssetWatcherPlugin};
use bsengine_core::{Camera, DirectionalLight, GlobalTransform, InspectorState, Transform};
use bsengine_ecs::{Added, Commands, Entity, Query, ResMut};
use bsengine_editor::EditorPlugin;
use bsengine_gltf::GltfPlugin;
use bsengine_input::InputPlugin;
use bsengine_render::{MeshRenderer, RenderPlugin};
use bsengine_rhi_wgpu::{
    capsule_vertices, cube_vertices, plane_vertices, sphere_vertices, GpuMeshRegistry,
    WgpuRHIPlugin,
};
use bsengine_scene::{Primitive, PrimitiveMesh, ScenePlugin};
use bsengine_scripting::ScriptingPlugin;
use bsengine_window::{WindowDescriptor, WindowPlugin};
use glam::{Quat, Vec3};
use std::env;

// winit requires its EventLoop to be created on the real OS main thread, so
// the extra stack space V8 needs (IsOnCentralStack() requires the isolate to
// be initialized and called from a thread with sufficient stack) comes from
// growing the actual main thread via linker flag (.cargo/config.toml
// `/STACK:67108864`) rather than from spawning a worker thread.
fn main() {
    let args: Vec<String> = env::args().collect();
    let scene_path = args.into_iter().nth(1);

    // Derive the game project root from the scene file path so that relative
    // script paths (e.g. "assets/scripts/player.js") resolve correctly.
    // Convention: scene lives at <project_root>/assets/<subdir>/<file>.ron
    let project_dir = scene_path
        .as_deref()
        .and_then(|p| std::path::Path::new(p).parent())
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .and_then(|p| p.to_str())
        .unwrap_or(".")
        .to_string();

    let mut inspector_state = InspectorState::editor();
    inspector_state.current_scene_path = scene_path.clone();

    let mut app = new_app();
    app.add_plugins(AssetPlugin)
        // Hot reload matters more here than in the runtime — editing an asset
        // is what this application is for. `project_dir` above is derived from
        // the scene path; with no scene argument it falls back to ".", so what
        // gets watched is `./assets` — which is the right directory when the
        // editor is launched from inside a game directory, and simply does not
        // exist when it is launched from the repository root. In that second
        // case the watcher logs why it is idle and starts nothing, rather than
        // falling back to watching the whole repository.
        .add_plugins(AssetWatcherPlugin)
        // Matters here for the same reason the watcher does, one step later:
        // the watcher notices an edited asset, this notices what became of
        // the reload. Without it the resource never exists, so every status
        // query in the process answers `unknown` — including for the asset
        // the user just broke and is looking at.
        .add_plugins(AssetStatusPlugin)
        // Same project directory the two plugins above work from, walked once
        // at Startup so a scene opened here resolves its references by
        // identity rather than by path. This is where an artist renames a
        // mesh, so it is where the reference item 30 exists to keep alive is
        // most likely to break.
        //
        // `project_dir` is derived above from the scene path this editor was
        // launched with, and handed to `ScriptingPlugin` below, which inserts
        // `ProjectDir` at build time — so it is in place before this plugin's
        // Startup scan looks for it. Launched with no scene argument it falls
        // back to ".", whose `assets/` does not exist at the repository root:
        // the scan reports that at info and publishes an empty index, exactly
        // as the watcher above goes idle. Nothing changes `ProjectDir` later,
        // so a one-shot Startup scan sees everything there is to see; if the
        // editor ever grows an open-a-project flow that swaps it, this scan
        // and the watcher both need rerunning, together.
        .add_plugins(AssetIdentityPlugin)
        .add_plugins(WgpuRHIPlugin)
        .add_plugins(WindowPlugin {
            descriptor: WindowDescriptor {
                title: "BSEngine Editor".to_string(),
                width: 1600,
                height: 900,
                resizable: true,
            },
        })
        .add_plugins(InputPlugin)
        .add_plugins(GltfPlugin)
        .add_plugins(RenderPlugin)
        .add_plugins(EditorPlugin)
        .add_plugins(ScriptingPlugin { project_dir })
        .add_systems(Update, resolve_primitives)
        .insert_resource(inspector_state);

    match scene_path {
        Some(path) => {
            app.add_plugins(ScenePlugin::from_file(&path));
        }
        None => {
            app.add_systems(Startup, setup_empty_scene);
        }
    }

    app.run();
}

fn resolve_primitives(
    query: Query<(Entity, &PrimitiveMesh), Added<PrimitiveMesh>>,
    mut commands: Commands,
    registry: Option<ResMut<GpuMeshRegistry>>,
) {
    let Some(mut registry) = registry else {
        return;
    };
    let mut cube_id: Option<u64> = None;
    let mut sphere_id: Option<u64> = None;
    let mut plane_id: Option<u64> = None;
    let mut capsule_id: Option<u64> = None;
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
        };
        commands.entity(entity).insert(MeshRenderer { mesh_id });
    }
}

fn setup_empty_scene(mut commands: Commands, mut registry: Option<ResMut<GpuMeshRegistry>>) {
    commands.spawn((
        Camera::perspective(60.0, 16.0 / 9.0),
        Transform::from_translation(Vec3::new(0.0, 3.0, 10.0)),
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform {
            rotation: Quat::from_rotation_arc(Vec3::NEG_Z, Vec3::new(-0.4, -0.8, -0.4).normalize())
                .into(),
            ..Default::default()
        },
        GlobalTransform::default(),
    ));

    if let Some(ref mut reg) = registry {
        let (verts, indices) = cube_vertices();
        let mesh_id = reg.register(&verts, &indices);
        commands.spawn((
            MeshRenderer { mesh_id },
            Transform {
                translation: Vec3::new(0.0, -0.1, 0.0).into(),
                rotation: Quat::IDENTITY.into(),
                scale: Vec3::new(20.0, 0.2, 20.0).into(),
            },
            GlobalTransform::default(),
        ));
    }
}

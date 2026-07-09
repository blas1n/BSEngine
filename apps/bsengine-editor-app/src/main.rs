use bsengine_app::{new_app, Startup, Update};
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

// V8's IsOnCentralStack() check requires that V8 is both initialized and
// called from the same thread, and that the thread has sufficient stack space.
// run_scripts uses ~13 000 lines of local state; on Windows the default 1 MB
// main-thread stack is exhausted before V8 can compile its per-frame snippet.
// Running everything on a 64 MB thread keeps the SP inside V8's recorded stack
// bounds for the lifetime of the process.
const STACK_SIZE: usize = 64 * 1024 * 1024;

fn main() {
    let args: Vec<String> = env::args().collect();
    std::thread::Builder::new()
        .name("bsengine-main".to_string())
        .stack_size(STACK_SIZE)
        .spawn(move || run(args))
        .expect("failed to spawn main thread")
        .join()
        .expect("main thread panicked");
}

fn run(args: Vec<String>) {
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

    let mut app = new_app();
    app.add_plugins(WgpuRHIPlugin)
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
        .insert_resource(InspectorState::editor());

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

    commands.spawn(DirectionalLight::default());

    if let Some(ref mut reg) = registry {
        let (verts, indices) = cube_vertices();
        let mesh_id = reg.register(&verts, &indices);
        commands.spawn((
            MeshRenderer { mesh_id },
            Transform {
                translation: Vec3::new(0.0, -0.1, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::new(20.0, 0.2, 20.0),
            },
            GlobalTransform::default(),
        ));
    }
}

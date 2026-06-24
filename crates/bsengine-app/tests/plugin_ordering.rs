use bsengine_app::{new_app, App, BsPlugin, Update};
use bsengine_ecs::{IntoSystemConfigs, IntoSystemSetConfigs, ResMut, Resource, SystemSet};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum GameSet {
    Physics,
    Render,
}

#[derive(Resource, Default)]
struct ExecutionLog(Vec<&'static str>);

fn physics_system(mut log: ResMut<ExecutionLog>) {
    log.0.push("physics");
}

fn render_system(mut log: ResMut<ExecutionLog>) {
    log.0.push("render");
}

struct PhysicsPlugin;
struct RenderPlugin;

impl BsPlugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, GameSet::Physics.before(GameSet::Render));
        app.add_systems(Update, physics_system.in_set(GameSet::Physics));
    }
}

impl BsPlugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, render_system.in_set(GameSet::Render));
    }
}

#[test]
fn render_always_runs_after_physics() {
    let mut app = new_app();
    app.insert_resource(ExecutionLog::default());
    app.add_plugins((PhysicsPlugin, RenderPlugin));

    // Run 3 Update cycles
    for _ in 0..3 {
        app.update();
    }

    let log = app.world().resource::<ExecutionLog>();
    // Should have 6 entries: physics, render, physics, render, physics, render
    assert_eq!(log.0.len(), 6, "Expected 6 log entries, got: {:?}", log.0);
    for chunk in log.0.chunks(2) {
        assert_eq!(chunk[0], "physics", "physics must run first in each frame");
        assert_eq!(chunk[1], "render", "render must run second in each frame");
    }
}

//! Loads behaviour tree assets and ticks every entity's tree against its
//! blackboard and its `NavMeshAgent`.

use std::collections::HashMap;

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bsengine_core::behavior_tree::{
    BehaviorTreeAsset, BtContext, BtRuntime, MoveState, MoveTarget,
};
use bsengine_core::{
    BehaviorTree, BehaviorTreeSystems, Blackboard, BtScriptCall, BtScriptQueue, BtScriptRequest,
    BtStatus, GlobalTransform, NavAgentState, NavMeshAgent, Transform,
};
use bsengine_scene::Name;
use glam::Vec3;

/// Behaviour tree assets parsed from disk, keyed by the path a
/// [`BehaviorTree`] names -- read once on first use, a failure remembered
/// rather than retried every frame, as `LoadedTimelines` does -- and read
/// again when the file changes on disk ([`reload_changed_behavior_trees`]).
#[derive(Resource, Default)]
pub struct LoadedBehaviorTrees {
    by_path: HashMap<String, Option<BehaviorTreeAsset>>,
    /// Paths re-read since the last tick, whose running trees restart. Taken
    /// (and so emptied) by every tick.
    reloaded: std::collections::HashSet<String>,
}

/// Reads and parses one tree, saying why when it cannot.
fn read_tree(path: &str) -> Option<BehaviorTreeAsset> {
    // Through the archive in a packaged build, as scenes and timelines are
    // read.
    match bsengine_asset::pak_source::read_to_string(path) {
        Ok(text) => match ron::from_str::<BehaviorTreeAsset>(&text) {
            Ok(tree) => Some(tree),
            Err(e) => {
                tracing::error!("[behavior tree] {path} is not a valid tree: {e}");
                None
            }
        },
        Err(e) => {
            tracing::error!("[behavior tree] cannot read {path}: {e}");
            None
        }
    }
}

impl LoadedBehaviorTrees {
    fn get(&mut self, path: &str) -> Option<&BehaviorTreeAsset> {
        if !self.by_path.contains_key(path) {
            self.by_path.insert(path.to_string(), read_tree(path));
        }
        self.by_path.get(path).and_then(Option::as_ref)
    }

    /// `path` changed on disk. Re-read if this cache holds it; a tree that no
    /// longer parses leaves the last good one in place -- a half-typed edit
    /// should not stop every guard in the level -- and only a tree that does
    /// replaces it and restarts the entities running it.
    fn reload(&mut self, path: &str) {
        let Some(held) = self.by_path.get_mut(path) else {
            return;
        };
        match read_tree(path) {
            Some(tree) => {
                tracing::info!("[behavior tree] {path} changed on disk, restarting its trees");
                *held = Some(tree);
                self.reloaded.insert(path.to_string());
            }
            None if held.is_some() => {
                tracing::warn!("[behavior tree] keeping the last good version of {path}");
            }
            None => {}
        }
    }
}

/// Hot reload: a `.bt.ron` edited while the game runs takes effect without a
/// restart, as a script or a texture already does. The asset watcher says
/// which file changed ([`bsengine_asset::AssetFileChanged`]); the trees read
/// from it restart from their root on the next tick, keeping the blackboard
/// -- the tree's structure changed, so no node's running state can be carried
/// across (a node index may now name a different node), but the blackboard
/// is the entity's data, not the tree's. What was running is aborted first,
/// so a `MoveTo` stops its agent and a script task gets its `onAbort`.
fn reload_changed_behavior_trees(
    mut changes: MessageReader<bsengine_asset::AssetFileChanged>,
    mut trees: ResMut<LoadedBehaviorTrees>,
) {
    for change in changes.read() {
        trees.reload(&change.path);
    }
}

/// Ticks every [`BehaviorTree`] once a frame, in [`BehaviorTreeSystems`] --
/// before navigation, so a `MoveTo` issued this frame is steered this frame --
/// and not while the game is paused.
pub struct BehaviorTreePlugin;

impl Plugin for BehaviorTreePlugin {
    fn build(&self, app: &mut App) {
        // Registered here as well as by the watcher that sends it, so the
        // reader below works in a host with no watcher (a replay, a packaged
        // game) and simply never sees one.
        app.add_message::<bsengine_asset::AssetFileChanged>()
            .init_resource::<LoadedBehaviorTrees>()
            .init_resource::<BtScriptQueue>()
            .configure_sets(
                Update,
                BehaviorTreeSystems.before(crate::nav_mesh::NavAgentSystems),
            )
            .add_systems(
                Update,
                (
                    // Not paused: an edit made while paused is picked up then,
                    // and the restart waits for the first tick after.
                    reload_changed_behavior_trees,
                    tick_behavior_trees.run_if(|paused: Option<Res<bsengine_core::PauseState>>| {
                        !paused.is_some_and(|p| p.paused)
                    }),
                )
                    .chain()
                    .in_set(BehaviorTreeSystems),
            );
    }
}

/// How far a followed entity may drift before its `MoveTo` is re-issued
/// (a new path). Small enough to follow, large enough that a target
/// jittering in place does not re-path every frame.
const RETARGET_DISTANCE: f32 = 0.25;

/// The executor's view of one entity's world.
struct EntityContext<'w> {
    world: &'w mut World,
    entity: Entity,
    now: f64,
    blackboard: Blackboard,
    /// Where the last `move_to` pointed, and its acceptance distance --
    /// read by the `move_state` that follows it in the same tick.
    goal: Option<(Vec3, Option<f32>)>,
}

impl EntityContext<'_> {
    fn position_of(&mut self, target: &MoveTarget) -> Option<Vec3> {
        match target {
            MoveTarget::Position(p) => Some(*p),
            MoveTarget::Entity(name) => {
                let mut q = self
                    .world
                    .query::<(&Name, Option<&GlobalTransform>, Option<&Transform>)>();
                q.iter(self.world)
                    .find(|(n, _, _)| n.0 == *name)
                    .and_then(|(_, g, t)| {
                        g.map(|g| g.0 .0.w_axis.truncate())
                            .or_else(|| t.map(|t| t.position.0))
                    })
            }
        }
    }

    fn own_position(&self) -> Option<Vec3> {
        self.world
            .get::<Transform>(self.entity)
            .map(|t| t.position.0)
    }
}

impl BtContext for EntityContext<'_> {
    fn now(&self) -> f64 {
        self.now
    }

    fn blackboard(&mut self) -> &mut Blackboard {
        &mut self.blackboard
    }

    fn move_to(&mut self, target: &MoveTarget, acceptance: Option<f32>) -> bool {
        let Some(goal) = self.position_of(target) else {
            return false;
        };
        let Some(mut agent) = self.world.get_mut::<NavMeshAgent>(self.entity) else {
            return false;
        };
        let current = agent.destination.map(|d| d.0);
        if current.is_none_or(|c| c.distance(goal) > RETARGET_DISTANCE) {
            agent.destination = Some(goal.into());
        }
        self.goal = Some((goal, acceptance));
        true
    }

    fn move_state(&mut self) -> MoveState {
        let own = self.own_position();
        let Some(mut agent) = self.world.get_mut::<NavMeshAgent>(self.entity) else {
            return MoveState::Failed;
        };
        // An acceptance distance is the task's own arrival test, flat as
        // the agent's is, and it stops the agent there.
        if let (Some((goal, Some(acceptance))), Some(own)) = (self.goal, own) {
            let flat = Vec3::new(goal.x - own.x, 0.0, goal.z - own.z).length();
            if flat <= acceptance {
                agent.clear_destination();
                return MoveState::Arrived;
            }
        }
        match agent.state {
            NavAgentState::Arrived => MoveState::Arrived,
            NavAgentState::NoPath => MoveState::Failed,
            NavAgentState::Moving | NavAgentState::Idle => MoveState::Moving,
        }
    }

    fn stop_move(&mut self) {
        if let Some(mut agent) = self.world.get_mut::<NavMeshAgent>(self.entity) {
            agent.clear_destination();
        }
    }

    fn script_task(&mut self, node: usize, task: &str, first: bool) -> Option<BtStatus> {
        let mut queue = self.world.get_resource_mut::<BtScriptQueue>()?;
        // Always taken, so an answer left over from an earlier run of the
        // same node (an aborted run's last word) cannot outlive it; on a
        // first call the executor does not read it.
        let answer = queue.results.remove(&(self.entity, node));
        if first || answer == Some(BtStatus::Running) {
            queue.requests.push(BtScriptRequest {
                entity: self.entity,
                node,
                task: task.to_string(),
                call: BtScriptCall::Tick { first },
            });
        }
        answer
    }

    fn abort_script_task(&mut self, node: usize, task: &str) {
        if let Some(mut queue) = self.world.get_resource_mut::<BtScriptQueue>() {
            queue.results.remove(&(self.entity, node));
            queue.requests.push(BtScriptRequest {
                entity: self.entity,
                node,
                task: task.to_string(),
                call: BtScriptCall::Abort,
            });
        }
    }
}

/// Ticks each entity's tree: compiles it on first use (and again when the
/// component names a different asset), aborts it when it is disabled, and
/// writes back the blackboard and the root's status.
fn tick_behavior_trees(world: &mut World) {
    // Not while the editor is editing: trees drive agents, and an agent that
    // walks off while its scene is being laid out is gameplay running in the
    // editor -- scripts and particles stop there the same way.
    if let Some(insp) = world.get_resource::<bsengine_core::InspectorState>() {
        if insp.editor_mode && insp.play_state == bsengine_core::EditorPlayState::Stopped {
            return;
        }
    }
    let now = world
        .get_resource::<bsengine_core::Time>()
        .map(|t| f64::from(t.elapsed_seconds))
        .unwrap_or(0.0);
    let project_dir = world.get_resource::<bsengine_core::ProjectDir>().cloned();
    let reloaded = world
        .get_resource_mut::<LoadedBehaviorTrees>()
        .map(|mut t| std::mem::take(&mut t.reloaded))
        .unwrap_or_default();
    let entities: Vec<Entity> = world
        .query_filtered::<Entity, With<BehaviorTree>>()
        .iter(world)
        .collect();
    for entity in entities {
        let Some(tree) = world.get::<BehaviorTree>(entity).cloned() else {
            continue;
        };
        let mut runtime = tree.runtime.clone();
        let blackboard = world.get::<Blackboard>(entity).cloned().unwrap_or_default();
        let mut ctx = EntityContext {
            world: &mut *world,
            entity,
            now,
            blackboard,
            goal: None,
        };

        let resolved = bsengine_core::resolve_project_path(project_dir.as_ref(), &tree.tree);
        // Compiled from a file that has since been re-read: rebuilt, below or
        // -- for a disabled tree -- when it is next enabled. Without this a
        // disabled tree would keep its old runtime past the one tick that
        // carries the news.
        let stale = reloaded.contains(&resolved);
        let status = if !tree.enabled {
            if let Some((_, rt)) = runtime.as_mut() {
                rt.abort(&mut ctx);
            }
            if stale {
                runtime = None;
            }
            tree.last_status
        } else {
            if stale || runtime.as_ref().map(|(path, _)| path.as_str()) != Some(tree.tree.as_str())
            {
                let asset = ctx
                    .world
                    .resource_mut::<LoadedBehaviorTrees>()
                    .get(&resolved)
                    .cloned();
                if let Some((_, old)) = runtime.as_mut() {
                    old.abort(&mut ctx);
                }
                runtime = asset.map(|a| (tree.tree.clone(), BtRuntime::new(&a)));
            }
            match runtime.as_mut() {
                Some((_, rt)) => rt.tick(&mut ctx),
                None => BtStatus::Failure,
            }
        };

        let blackboard = ctx.blackboard;
        let world = ctx.world;
        if let Some(mut bb) = world.get_mut::<Blackboard>(entity) {
            if *bb != blackboard {
                *bb = blackboard;
            }
        } else if !blackboard.values.is_empty() {
            world.entity_mut(entity).insert(blackboard);
        }
        // Empty once disabled without asking: the abort above reset every
        // node, so nothing is running.
        let active_path = runtime
            .as_ref()
            .map(|(_, rt)| rt.running_path())
            .unwrap_or_default();
        if let Some(mut component) = world.get_mut::<BehaviorTree>(entity) {
            component.runtime = runtime;
            component.last_status = status;
            if component.active_path != active_path {
                component.active_path = active_path;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{BbValue, NavMesh};

    fn tree_file(tag: &str, text: &str) -> String {
        let path =
            std::env::temp_dir().join(format!("bsengine_bt_{tag}_{}.bt.ron", std::process::id()));
        std::fs::write(&path, text).unwrap();
        path.to_string_lossy().to_string()
    }

    /// A 20 x 20 walkable floor, a fixed 0.1 s clock, and the real
    /// navigation and behaviour tree plugins.
    fn app() -> App {
        let mut app = crate::new_app();
        // After `TimePlugin`, which inserts a wall-clock `Time` of its own:
        // before it, this would be overwritten and each frame would advance
        // by the microseconds a test frame takes.
        app.add_plugins(crate::TimePlugin);
        app.insert_resource(bsengine_core::Time::fixed(0.1));
        app.insert_resource(NavMesh::new(40, 40, 0.5, Vec3::new(-10.0, 0.0, -10.0)));
        app.add_plugins(crate::NavMeshPlugin);
        app.add_plugins(BehaviorTreePlugin);
        app
    }

    fn guard(app: &mut App, tree: String, blackboard: Blackboard) -> Entity {
        app.world_mut()
            .spawn((
                Name("Guard".to_string()),
                Transform::default(),
                NavMeshAgent::new(4.0),
                blackboard,
                BehaviorTree {
                    tree,
                    ..Default::default()
                },
            ))
            .id()
    }

    fn position(app: &App, e: Entity) -> Vec3 {
        app.world().get::<Transform>(e).unwrap().position.0
    }

    /// A tree that writes `version` once and then waits: which file a guard
    /// is running is visible on its blackboard.
    fn versioned_tree(version: i64) -> String {
        format!(
            r#"(root: Sequence([SetValue(key: "version", value: Int({version})), Wait(seconds: 100.0)]))"#
        )
    }

    fn version(app: &App, e: Entity) -> Option<BbValue> {
        app.world()
            .get::<Blackboard>(e)
            .and_then(|bb| bb.get("version").cloned())
    }

    fn announce(app: &mut App, path: &str) {
        app.world_mut()
            .write_message(bsengine_asset::AssetFileChanged {
                path: path.to_string(),
            });
    }

    /// Hot reload: the file is rewritten and the watcher says so, and the
    /// guard runs the new tree from its root on the next frame -- with the
    /// blackboard it had, since that is the entity's data. The premise is the
    /// other half: rewriting the file *without* the notice changes nothing,
    /// so it is the notice, not a re-read every frame, that reloads.
    #[test]
    fn an_edited_tree_restarts_with_the_new_file_and_the_same_blackboard() {
        let path = tree_file("reload", &versioned_tree(1));
        let mut app = app();
        let mut bb = Blackboard::default();
        bb.set("kept", BbValue::Bool(true));
        let e = guard(&mut app, path.clone(), bb);
        app.update();
        assert_eq!(version(&app, e), Some(BbValue::Int(1)));

        std::fs::write(&path, versioned_tree(2)).unwrap();
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(
            version(&app, e),
            Some(BbValue::Int(1)),
            "premise: without the notice the cached tree keeps running"
        );

        announce(&mut app, &path);
        app.update();
        assert_eq!(version(&app, e), Some(BbValue::Int(2)), "the new tree ran");
        assert_eq!(
            app.world().get::<Blackboard>(e).unwrap().get("kept"),
            Some(&BbValue::Bool(true)),
            "the blackboard survived the restart"
        );
        assert_eq!(
            app.world().get::<BehaviorTree>(e).unwrap().last_status,
            BtStatus::Running,
            "and it is waiting, not failed"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A half-typed edit does not stop every guard: a file that no longer
    /// parses is reported and the last good tree stays -- for the guard
    /// already running it, and for one that starts after the bad edit.
    #[test]
    fn a_tree_that_no_longer_parses_leaves_the_last_good_one() {
        let path = tree_file("broken", &versioned_tree(1));
        let mut app = app();
        let first = guard(&mut app, path.clone(), Blackboard::default());
        app.update();
        assert_eq!(version(&app, first), Some(BbValue::Int(1)));

        std::fs::write(&path, "(root: Sequence([").unwrap();
        announce(&mut app, &path);
        app.update();
        assert_eq!(
            app.world().get::<BehaviorTree>(first).unwrap().last_status,
            BtStatus::Running,
            "the running guard kept its tree"
        );
        let second = guard(&mut app, path.clone(), Blackboard::default());
        app.update();
        assert_eq!(
            version(&app, second),
            Some(BbValue::Int(1)),
            "a guard starting after the bad edit gets the last good tree, not none"
        );

        // And the fix lands: the cache did not give up on the file.
        std::fs::write(&path, versioned_tree(3)).unwrap();
        announce(&mut app, &path);
        app.update();
        assert_eq!(version(&app, first), Some(BbValue::Int(3)));
        let _ = std::fs::remove_file(path);
    }

    /// The whole chain with nothing handed over by hand: the real asset
    /// watcher sees the edit on disk, and the tree it names restarts. What the
    /// tests above cannot see is the spelling -- the watcher builds its path
    /// from `ProjectDir` and what `notify` reports, the cache from
    /// `ProjectDir` and the component's string, and a notice for a path the
    /// cache does not hold by that exact spelling reloads nothing, silently.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_asset_watcher_reloads_a_tree_edited_on_disk() {
        struct RemoveDir(std::path::PathBuf);
        impl Drop for RemoveDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let root = std::env::temp_dir().join(format!(
            "bsengine-bt-watch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _cleanup = RemoveDir(root.clone());
        let tree = root.join("assets").join("ai").join("guard.bt.ron");
        std::fs::create_dir_all(tree.parent().unwrap()).unwrap();
        std::fs::write(&tree, versioned_tree(1)).unwrap();

        let mut app = app();
        app.insert_resource(bsengine_core::ProjectDir(
            root.to_string_lossy().replace('\\', "/"),
        ));
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(bsengine_asset::AssetWatcherPlugin);
        let e = guard(
            &mut app,
            "assets/ai/guard.bt.ron".to_string(),
            Blackboard::default(),
        );
        app.update();
        assert_eq!(
            version(&app, e),
            Some(BbValue::Int(1)),
            "premise: the tree loads through ProjectDir"
        );

        // Past the watcher's 200 ms debounce before the edit, so the write is
        // not folded into whatever the start-up produced.
        std::thread::sleep(std::time::Duration::from_millis(600));
        std::fs::write(&tree, versioned_tree(2)).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while version(&app, e) != Some(BbValue::Int(2)) {
            assert!(
                std::time::Instant::now() < deadline,
                "the edit never reached the running tree"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
            app.update();
        }
    }

    /// A tree switched off when its file changed runs the new file when it is
    /// switched back on. The notice reaches only the one tick after it, so a
    /// disabled tree that kept its old runtime would resume the old file.
    #[test]
    fn a_disabled_tree_picks_up_the_edit_when_enabled() {
        let path = tree_file("disabled", &versioned_tree(1));
        let mut app = app();
        let e = guard(&mut app, path.clone(), Blackboard::default());
        app.update();
        assert_eq!(version(&app, e), Some(BbValue::Int(1)));

        app.world_mut().get_mut::<BehaviorTree>(e).unwrap().enabled = false;
        app.update();
        std::fs::write(&path, versioned_tree(2)).unwrap();
        announce(&mut app, &path);
        app.update();
        app.update();
        assert_eq!(
            version(&app, e),
            Some(BbValue::Int(1)),
            "premise: nothing runs while it is off"
        );

        app.world_mut().get_mut::<BehaviorTree>(e).unwrap().enabled = true;
        app.update();
        assert_eq!(version(&app, e), Some(BbValue::Int(2)));
        let _ = std::fs::remove_file(path);
    }

    /// Through the real navigation: the tree's MoveTo walks the guard to the
    /// blackboard's goal, and the SetValue after it -- reached only when the
    /// move succeeded -- writes back to the entity's blackboard.
    #[test]
    fn move_to_walks_the_agent_to_the_blackboard_goal() {
        let path = tree_file(
            "walk",
            r#"(root: Sequence([MoveTo(key: "goal"), SetValue(key: "arrived", value: Bool(true)), Wait(seconds: 100.0)]))"#,
        );
        let mut app = app();
        let goal = Vec3::new(4.0, 0.0, 0.0);
        let mut bb = Blackboard::default();
        bb.set("goal", BbValue::Vec3(goal.into()));
        let e = guard(&mut app, path.clone(), bb);
        assert!(
            position(&app, e).distance(goal) > 3.9,
            "premise: it starts away from the goal"
        );
        // The tree runs before navigation: the MoveTo issued on the first
        // frame is steered on that same frame. After it, navigation would
        // only see the destination a frame later.
        app.update();
        assert!(
            position(&app, e).x > 0.01,
            "moved on the frame the move was issued: {}",
            position(&app, e)
        );
        let mut frames = 0;
        while app
            .world()
            .get::<Blackboard>(e)
            .unwrap()
            .get("arrived")
            .is_none()
        {
            app.update();
            frames += 1;
            assert!(frames < 200, "never arrived; at {}", position(&app, e));
        }
        assert!(
            position(&app, e).distance(goal) < 0.3,
            "at the goal when the tree moved on: {}",
            position(&app, e)
        );
        let tree = app.world().get::<BehaviorTree>(e).unwrap();
        assert_eq!(tree.last_status, BtStatus::Running, "waiting now");
        let _ = std::fs::remove_file(path);
    }

    /// Chasing an entity by name to an acceptance distance: the guard stops
    /// within it of the player -- not on top of them -- and the agent's
    /// destination is cleared, so it does not keep walking in.
    #[test]
    fn move_to_chases_an_entity_to_the_acceptance_distance() {
        let path = tree_file(
            "chase",
            r#"(root: Sequence([MoveTo(key: "enemy", acceptance: Some(1.5)), SetValue(key: "caught", value: Bool(true)), Wait(seconds: 100.0)]))"#,
        );
        let mut app = app();
        let player = Vec3::new(3.0, 0.0, 4.0);
        app.world_mut().spawn((
            Name("Player".to_string()),
            Transform {
                position: player.into(),
                ..Default::default()
            },
        ));
        let mut bb = Blackboard::default();
        bb.set("enemy", BbValue::Entity("Player".to_string()));
        let e = guard(&mut app, path.clone(), bb);
        let mut frames = 0;
        while app
            .world()
            .get::<Blackboard>(e)
            .unwrap()
            .get("caught")
            .is_none()
        {
            app.update();
            frames += 1;
            assert!(frames < 200, "never caught up; at {}", position(&app, e));
        }
        let d = position(&app, e).distance(player);
        assert!(
            d <= 1.5 + 1e-3 && d > 1.0,
            "stopped at the acceptance distance: {d}"
        );
        assert!(
            app.world()
                .get::<NavMeshAgent>(e)
                .unwrap()
                .destination
                .is_none(),
            "and stopped the agent there"
        );
        let _ = std::fs::remove_file(path);
    }

    /// Disabling the tree mid-walk aborts the MoveTo: the agent's
    /// destination is cleared and the guard stops where it is.
    #[test]
    fn disabling_the_tree_stops_its_move() {
        let path = tree_file("disable", r#"(root: MoveTo(key: "goal"))"#);
        let mut app = app();
        let mut bb = Blackboard::default();
        bb.set("goal", BbValue::Vec3(Vec3::new(8.0, 0.0, 0.0).into()));
        let e = guard(&mut app, path.clone(), bb);
        for _ in 0..3 {
            app.update();
        }
        assert!(
            app.world()
                .get::<NavMeshAgent>(e)
                .unwrap()
                .destination
                .is_some(),
            "premise: walking"
        );
        assert_eq!(
            app.world().get::<BehaviorTree>(e).unwrap().active_path,
            vec!["MoveTo(goal)".to_string()],
            "the component shows what is running"
        );
        app.world_mut().get_mut::<BehaviorTree>(e).unwrap().enabled = false;
        app.update();
        assert!(app
            .world()
            .get::<NavMeshAgent>(e)
            .unwrap()
            .destination
            .is_none());
        assert!(
            app.world()
                .get::<BehaviorTree>(e)
                .unwrap()
                .active_path
                .is_empty(),
            "and nothing once disabled"
        );
        let stopped = position(&app, e);
        for _ in 0..5 {
            app.update();
        }
        assert!(
            position(&app, e).distance(stopped) < 1e-4,
            "and it stays put"
        );
        let _ = std::fs::remove_file(path);
    }

    /// While the editor is editing (not playing) the tree does not tick: its
    /// MoveTo would walk an agent off while the scene is being laid out.
    /// Play starts it.
    #[test]
    fn trees_wait_for_play_in_the_editor() {
        let path = tree_file("editor", r#"(root: MoveTo(key: "goal"))"#);
        let mut app = app();
        let mut editing = bsengine_core::InspectorState::default();
        editing.editor_mode = true;
        editing.play_state = bsengine_core::EditorPlayState::Stopped;
        app.insert_resource(editing);
        let mut bb = Blackboard::default();
        bb.set("goal", BbValue::Vec3(Vec3::new(8.0, 0.0, 0.0).into()));
        let e = guard(&mut app, path.clone(), bb);
        for _ in 0..3 {
            app.update();
        }
        assert!(
            app.world()
                .get::<NavMeshAgent>(e)
                .unwrap()
                .destination
                .is_none(),
            "no move issued while editing"
        );
        app.world_mut()
            .resource_mut::<bsengine_core::InspectorState>()
            .play_state = bsengine_core::EditorPlayState::Playing;
        app.update();
        assert!(
            app.world()
                .get::<NavMeshAgent>(e)
                .unwrap()
                .destination
                .is_some(),
            "and issued once playing"
        );
        let _ = std::fs::remove_file(path);
    }

    /// A tree that cannot be loaded fails, every frame, without panicking --
    /// and without being re-read every frame.
    #[test]
    fn a_missing_tree_fails_quietly() {
        let mut app = app();
        let e = guard(
            &mut app,
            "no/such/tree.bt.ron".to_string(),
            Blackboard::default(),
        );
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(
            app.world().get::<BehaviorTree>(e).unwrap().last_status,
            BtStatus::Failure
        );
        assert_eq!(
            app.world().resource::<LoadedBehaviorTrees>().by_path.len(),
            1
        );
    }
}

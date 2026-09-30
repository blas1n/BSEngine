//! Loads behaviour tree assets and ticks every entity's tree against its
//! blackboard and its `NavMeshAgent`.

use std::collections::HashMap;

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bsengine_core::behavior_tree::{
    BehaviorTreeAsset, BtContext, BtRuntime, MoveState, MoveTarget,
};
use bsengine_core::{
    BehaviorTree, BehaviorTreeSystems, Blackboard, BtStatus, GlobalTransform, NavAgentState,
    NavMeshAgent, Transform,
};
use bsengine_scene::Name;
use glam::Vec3;

/// Behaviour tree assets parsed from disk, keyed by the path a
/// [`BehaviorTree`] names -- read once on first use, a failure remembered
/// rather than retried every frame, as `LoadedTimelines` does.
#[derive(Resource, Default)]
pub struct LoadedBehaviorTrees {
    by_path: HashMap<String, Option<BehaviorTreeAsset>>,
}

impl LoadedBehaviorTrees {
    fn get(&mut self, path: &str) -> Option<&BehaviorTreeAsset> {
        if !self.by_path.contains_key(path) {
            // Through the archive in a packaged build, as scenes and
            // timelines are read.
            let parsed = match bsengine_asset::pak_source::read_to_string(path) {
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
            };
            self.by_path.insert(path.to_string(), parsed);
        }
        self.by_path.get(path).and_then(Option::as_ref)
    }
}

/// Ticks every [`BehaviorTree`] once a frame, in [`BehaviorTreeSystems`] --
/// before navigation, so a `MoveTo` issued this frame is steered this frame --
/// and not while the game is paused.
pub struct BehaviorTreePlugin;

impl Plugin for BehaviorTreePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadedBehaviorTrees>()
            .configure_sets(
                Update,
                BehaviorTreeSystems.before(crate::nav_mesh::NavAgentSystems),
            )
            .add_systems(
                Update,
                tick_behavior_trees
                    .run_if(|paused: Option<Res<bsengine_core::PauseState>>| {
                        !paused.is_some_and(|p| p.paused)
                    })
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
}

/// Ticks each entity's tree: compiles it on first use (and again when the
/// component names a different asset), aborts it when it is disabled, and
/// writes back the blackboard and the root's status.
fn tick_behavior_trees(world: &mut World) {
    let now = world
        .get_resource::<bsengine_core::Time>()
        .map(|t| f64::from(t.elapsed_seconds))
        .unwrap_or(0.0);
    let project_dir = world.get_resource::<bsengine_core::ProjectDir>().cloned();
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

        let status = if !tree.enabled {
            if let Some((_, rt)) = runtime.as_mut() {
                rt.abort(&mut ctx);
            }
            tree.last_status
        } else {
            if runtime.as_ref().map(|(path, _)| path.as_str()) != Some(tree.tree.as_str()) {
                let resolved =
                    bsengine_core::resolve_project_path(project_dir.as_ref(), &tree.tree);
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
        if let Some(mut component) = world.get_mut::<BehaviorTree>(entity) {
            component.runtime = runtime;
            component.last_status = status;
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
        app.world_mut().get_mut::<BehaviorTree>(e).unwrap().enabled = false;
        app.update();
        assert!(app
            .world()
            .get::<NavMeshAgent>(e)
            .unwrap()
            .destination
            .is_none());
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

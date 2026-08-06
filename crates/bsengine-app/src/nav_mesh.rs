use std::collections::HashMap;

use bevy_app::{App, Plugin, Update};
use bsengine_core::{NavAgentState, NavMesh, NavMeshAgent, Time, Transform};
use bsengine_ecs::{Entity, IntoSystemConfigs, Query, Res, ResMut, Resource};
use glam::Vec3;

/// Paths `NavMeshAgent` entities across the `NavMesh` resource, moving them toward
/// their destination each frame with basic separation-based obstacle avoidance.
pub struct NavMeshPlugin;

/// Per-entity cached A* path. Keyed by Entity; value is (waypoints, index, destination_it_was_computed_for).
#[derive(Resource, Default)]
struct NavCache(HashMap<Entity, (Vec<Vec3>, usize, Option<Vec3>)>);

impl Plugin for NavMeshPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NavMesh>()
            .init_resource::<NavCache>()
            .add_systems(
                Update,
                navigate_agents.run_if(
                    |paused: Option<bevy_ecs::prelude::Res<bsengine_core::PauseState>>| {
                        !paused.map(|p| p.paused).unwrap_or(false)
                    },
                ),
            );
    }
}

fn navigate_agents(
    navmesh: Res<NavMesh>,
    time: Res<Time>,
    mut cache: ResMut<NavCache>,
    mut query: Query<(Entity, &mut NavMeshAgent, &mut Transform)>,
    mut physics: Option<ResMut<bsengine_physics::PhysicsWorld>>,
) {
    let dt = time.delta_seconds;

    // Read pass: collect positions for separation (must finish before mutable borrow).
    let all_positions: Vec<(Entity, Vec3, f32)> = query
        .iter()
        .map(|(e, a, t)| (e, t.position.0, a.radius))
        .collect();

    for (entity, mut agent, mut transform) in query.iter_mut() {
        if !agent.enabled {
            if agent.destination.is_none() {
                agent.state = NavAgentState::Idle;
            }
            continue;
        }

        let Some(dest) = agent.destination else {
            agent.state = NavAgentState::Idle;
            cache.0.remove(&entity);
            continue;
        };
        let dest = dest.0;

        // Check arrival before computing paths.
        let flat_pos = Vec3::new(transform.position.x, 0.0, transform.position.z);
        let flat_dest = Vec3::new(dest.x, 0.0, dest.z);
        if (flat_pos - flat_dest).length() <= agent.stopping_distance {
            agent.state = NavAgentState::Arrived;
            continue;
        }

        // Recompute path only when destination has changed.
        let needs_recompute = cache
            .0
            .get(&entity)
            .and_then(|(_, _, for_dest)| *for_dest)
            .is_none_or(|d| (d - dest).length_squared() > 0.0001);

        if needs_recompute {
            match navmesh.find_path(transform.position.0, dest) {
                Some(wp) => {
                    cache.0.insert(entity, (wp, 0, Some(dest)));
                }
                None => {
                    cache.0.insert(entity, (vec![], 0, Some(dest)));
                    agent.state = NavAgentState::NoPath;
                    continue;
                }
            }
        }

        let Some((waypoints, idx, _)) = cache.0.get_mut(&entity) else {
            agent.state = NavAgentState::NoPath;
            continue;
        };

        if waypoints.is_empty() {
            agent.state = NavAgentState::NoPath;
            continue;
        }

        // Advance past already-reached waypoints.
        let wp_threshold = agent.stopping_distance.max(0.15);
        while *idx < waypoints.len() {
            let wp = waypoints[*idx];
            let dx = wp.x - transform.position.x;
            let dz = wp.z - transform.position.z;
            if (dx * dx + dz * dz).sqrt() <= wp_threshold {
                *idx += 1;
            } else {
                break;
            }
        }

        if *idx >= waypoints.len() {
            agent.state = NavAgentState::Arrived;
            continue;
        }

        let wp = waypoints[*idx];
        let dx = wp.x - transform.position.x;
        let dz = wp.z - transform.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist < 0.001 {
            agent.state = NavAgentState::Moving;
            continue;
        }

        let move_dir = Vec3::new(dx / dist, 0.0, dz / dist);

        // Basic separation from nearby agents (dynamic obstacle avoidance).
        let mut sep = Vec3::ZERO;
        for &(other, other_pos, other_radius) in &all_positions {
            if other == entity {
                continue;
            }
            let sx = transform.position.x - other_pos.x;
            let sz = transform.position.z - other_pos.z;
            let min_d = agent.radius + other_radius;
            let sd = (sx * sx + sz * sz).sqrt();
            if sd < min_d && sd > 0.001 {
                let scale = (1.0 - sd / min_d) / sd;
                sep.x += sx * scale;
                sep.z += sz * scale;
            }
        }

        let final_dir = if sep.length_squared() > 0.0001 {
            let cx = move_dir.x + sep.x * 0.5;
            let cz = move_dir.z + sep.z * 0.5;
            let clen = (cx * cx + cz * cz).sqrt();
            if clen > 0.001 {
                Vec3::new(cx / clen, 0.0, cz / clen)
            } else {
                move_dir
            }
        } else {
            move_dir
        };

        match physics
            .as_mut()
            .and_then(|p| p.get_linvel(entity).map(|v| (p, v)))
        {
            // Physics owns this entity's `Transform` (see
            // `sync_transform_from_physics`), so writing it here would be
            // discarded on the same frame. Steer by impulse instead, which also
            // lets knockback survive: an impulse from a script adds to the
            // body's velocity rather than being overwritten by ours.
            Some((physics, current)) => {
                let desired = final_dir * agent.speed;
                let mut dv = Vec3::new(desired.x - current.x, 0.0, desired.z - current.z);
                // `acceleration` caps how fast the agent may change its own
                // velocity. Before this it was a declared-but-unread field.
                let max_dv = agent.acceleration * dt;
                if dv.length() > max_dv {
                    dv = dv.normalize() * max_dv;
                }
                // Impulse, not force: `PhysicsWorld::step` never resets forces
                // and Rapier's `add_force` persists across steps, so a
                // force-driven agent would gain speed every frame. `mass * dv`
                // produces the same velocity change and does not carry over.
                let mass = physics.get_mass(entity).unwrap_or(1.0);
                physics.apply_impulse(entity, dv * mass);
            }
            // No physics body: nothing else owns this `Transform`, so moving it
            // directly is the only option and is correct. This is dispatch on
            // who owns the transform, not a second motion model.
            None => {
                let move_dist = (agent.speed * dt).min(dist);
                transform.position.x += final_dir.x * move_dist;
                transform.position.z += final_dir.z * move_dist;
            }
        }
        agent.state = NavAgentState::Moving;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{NavAgentState, NavMesh, NavMeshAgent, Time, Transform};
    use glam::Vec3;

    fn make_app(nm: NavMesh) -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(NavMeshPlugin);
        app.insert_resource(nm);
        let mut t = Time::default();
        t.set_delta_for_test(0.1);
        app.insert_resource(t);
        app
    }

    fn open_app() -> bevy_app::App {
        make_app(NavMesh::new(20, 20, 1.0, Vec3::new(-10.0, 0.0, -10.0)))
    }

    #[test]
    fn idle_with_no_destination() {
        let mut app = open_app();
        app.world_mut()
            .spawn((NavMeshAgent::new(5.0), Transform::from_position(Vec3::ZERO)));
        app.update();

        let state = app
            .world_mut()
            .query::<&NavMeshAgent>()
            .iter(app.world())
            .next()
            .unwrap()
            .state;
        assert_eq!(state, NavAgentState::Idle);
    }

    #[test]
    fn does_not_move_while_paused() {
        let mut app = open_app();
        app.insert_resource(bsengine_core::PauseState { paused: true });
        app.world_mut().spawn((
            NavMeshAgent::new(5.0).with_destination(Vec3::new(3.0, 0.0, 0.0)),
            Transform::from_position(Vec3::ZERO),
        ));
        app.update();

        let transform = app
            .world_mut()
            .query::<&Transform>()
            .iter(app.world())
            .next()
            .unwrap();
        assert_eq!(
            transform.position.0,
            Vec3::ZERO,
            "expected the agent to stay in place while paused, got {:?}",
            transform.position.0
        );
    }

    #[test]
    fn moves_toward_destination() {
        let mut app = open_app();
        app.world_mut().spawn((
            NavMeshAgent::new(5.0).with_destination(Vec3::new(3.0, 0.0, 0.0)),
            Transform::from_position(Vec3::ZERO),
        ));
        app.update();
        app.update();

        let t = app
            .world_mut()
            .query::<&Transform>()
            .iter(app.world())
            .next()
            .unwrap()
            .clone();
        assert!(
            t.position.x > 0.0,
            "agent should move toward +x destination"
        );
    }

    #[test]
    fn arrives_at_close_destination() {
        let mut app = open_app();
        app.world_mut().spawn((
            NavMeshAgent::new(20.0)
                .with_destination(Vec3::new(0.5, 0.0, 0.0))
                .with_stopping_distance(0.05),
            Transform::from_position(Vec3::ZERO),
        ));
        for _ in 0..3 {
            app.update();
        }

        let state = app
            .world_mut()
            .query::<&NavMeshAgent>()
            .iter(app.world())
            .next()
            .unwrap()
            .state;
        assert_eq!(state, NavAgentState::Arrived);
    }

    #[test]
    fn no_path_through_full_wall() {
        let mut nm = NavMesh::new(10, 10, 1.0, Vec3::new(-5.0, 0.0, -5.0));
        for z in 0..10u32 {
            nm.set_walkable(5, z, false);
        }
        let mut app = make_app(nm);
        app.world_mut().spawn((
            NavMeshAgent::new(5.0).with_destination(Vec3::new(3.0, 0.0, 0.0)),
            Transform::from_position(Vec3::new(-3.0, 0.0, 0.0)),
        ));
        app.update();
        app.update();

        let state = app
            .world_mut()
            .query::<&NavMeshAgent>()
            .iter(app.world())
            .next()
            .unwrap()
            .state;
        assert_eq!(state, NavAgentState::NoPath);
    }

    #[test]
    fn disabled_agent_does_not_move() {
        let mut app = open_app();
        let mut agent = NavMeshAgent::new(5.0).with_destination(Vec3::new(5.0, 0.0, 0.0));
        agent.enabled = false;
        app.world_mut()
            .spawn((agent, Transform::from_position(Vec3::ZERO)));
        app.update();

        let t = app
            .world_mut()
            .query::<&Transform>()
            .iter(app.world())
            .next()
            .unwrap()
            .clone();
        assert_eq!(t.position.x, 0.0, "disabled agent must not move");
    }

    #[test]
    fn path_recomputed_on_destination_change() {
        let mut app = open_app();
        let entity = app
            .world_mut()
            .spawn((
                NavMeshAgent::new(5.0).with_destination(Vec3::new(3.0, 0.0, 0.0)),
                Transform::from_position(Vec3::ZERO),
            ))
            .id();
        app.update();

        // Change destination.
        let mut agent = app.world_mut().get_mut::<NavMeshAgent>(entity).unwrap();
        agent.destination = Some(Vec3::new(-3.0, 0.0, 0.0).into());
        app.update();

        let state = app
            .world_mut()
            .query::<&NavMeshAgent>()
            .iter(app.world())
            .next()
            .unwrap()
            .state;
        assert_ne!(
            state,
            NavAgentState::NoPath,
            "recomputed path should succeed"
        );
    }

    // ---- physics-backed agents (roadmap item 27) -------------------------

    use bsengine_physics::{
        CharacterBody, Collider, PhysicsInput, PhysicsPlugin, PhysicsTransform, PhysicsWorld,
        RigidBody,
    };

    /// An app with both navigation and physics, plus a floor to stand on.
    fn physics_app() -> bevy_app::App {
        let mut app = open_app();
        app.add_plugins(PhysicsPlugin);
        app.world_mut().spawn((
            Transform::from_position(Vec3::new(0.0, -0.5, 0.0)),
            RigidBody::fixed(),
            Collider::cuboid(20.0, 0.5, 20.0),
            PhysicsInput {
                position: Vec3::new(0.0, -0.5, 0.0).into(),
                rotation: glam::Quat::IDENTITY.into(),
            },
            PhysicsTransform::default(),
        ));
        app
    }

    fn spawn_physics_agent(
        app: &mut bevy_app::App,
        agent: NavMeshAgent,
    ) -> bevy_ecs::entity::Entity {
        let at = Vec3::new(0.0, 0.8, 0.0);
        app.world_mut()
            .spawn((
                agent,
                Transform::from_position(at),
                RigidBody {
                    linear_damping: 4.0,
                    ..RigidBody::dynamic()
                },
                Collider::capsule(0.5, 0.3),
                CharacterBody::default(),
                PhysicsInput {
                    position: at.into(),
                    rotation: glam::Quat::IDENTITY.into(),
                },
                PhysicsTransform::default(),
            ))
            .id()
    }

    #[test]
    fn a_physics_backed_agent_moves_toward_its_destination() {
        let mut app = physics_app();
        let entity = spawn_physics_agent(
            &mut app,
            NavMeshAgent::new(5.0).with_destination(Vec3::new(6.0, 0.0, 0.0)),
        );

        for _ in 0..60 {
            app.update();
        }

        let x = app.world().get::<Transform>(entity).unwrap().position.x;
        assert!(
            x > 1.0,
            "agent should have travelled toward +x under its own impulses; x = {x}"
        );
    }

    #[test]
    fn knockback_survives_agent_movement() {
        // The property this whole item exists for: a knockback impulse has to
        // keep affecting the agent instead of being wiped by the agent's own
        // steering on the next frame.
        //
        // Asserting only "it moved backwards once" does not test that -- even
        // an agent that overwrites its velocity outright still gets one frame
        // of displacement from the impulse, and that version of this test
        // passed against a `set_linvel` implementation. So run the same
        // scenario twice, once undisturbed, and compare where each ends up
        // after the knockback has had time to be either carried or erased.
        let mut app = physics_app();
        let entity = spawn_physics_agent(
            &mut app,
            NavMeshAgent::new(5.0).with_destination(Vec3::new(8.0, 0.0, 0.0)),
        );
        for _ in 0..10 {
            app.update();
        }

        // Small enough that the agent's own steering can plausibly answer it
        // within a couple of frames. A huge impulse would make any
        // implementation look like it carried the knockback.
        app.world_mut()
            .resource_mut::<PhysicsWorld>()
            .apply_impulse(entity, Vec3::new(-3.0, 0.0, 0.0));

        // Sample two frames running. Any implementation moves the agent
        // backwards on the *first* frame -- the impulse lands before anything
        // can react to it, so total displacement proves nothing. What separates
        // carrying the knockback from erasing it is whether the agent is *still*
        // going backwards on the second frame, or already walking forward again
        // because its own steering overwrote the velocity.
        app.update();
        let first = app.world().get::<Transform>(entity).unwrap().position.x;
        app.update();
        let second = app.world().get::<Transform>(entity).unwrap().position.x;

        assert!(
            second < first,
            "the agent should still be sliding backwards a frame later, not \
             already walking forward again: {first} -> {second}"
        );
    }

    #[test]
    fn acceleration_changes_how_fast_an_agent_gets_going() {
        // `NavMeshAgent::acceleration` was a declared-but-unread field before
        // this item -- scenes authored a value for it and nothing looked. This
        // pins that it now does something.
        fn distance_after_10_frames(acceleration: f32) -> f32 {
            let mut app = physics_app();
            let mut agent = NavMeshAgent::new(5.0).with_destination(Vec3::new(6.0, 0.0, 0.0));
            agent.acceleration = acceleration;
            let entity = spawn_physics_agent(&mut app, agent);
            for _ in 0..10 {
                app.update();
            }
            app.world().get::<Transform>(entity).unwrap().position.x
        }

        let slow = distance_after_10_frames(1.0);
        let fast = distance_after_10_frames(50.0);
        assert!(
            fast > slow,
            "higher acceleration should cover more ground early: {slow} vs {fast}"
        );
    }
}

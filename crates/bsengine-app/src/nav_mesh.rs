use std::collections::HashMap;

use bevy_app::{App, Plugin, Update};
use bsengine_core::{NavAgentState, NavMesh, NavMeshAgent, Time, Transform};
use bsengine_ecs::{Entity, Query, Res, ResMut, Resource};
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
            .add_systems(Update, navigate_agents);
    }
}

fn navigate_agents(
    navmesh: Res<NavMesh>,
    time: Res<Time>,
    mut cache: ResMut<NavCache>,
    mut query: Query<(Entity, &mut NavMeshAgent, &mut Transform)>,
) {
    let dt = time.delta_seconds;

    // Read pass: collect positions for separation (must finish before mutable borrow).
    let all_positions: Vec<(Entity, Vec3, f32)> = query
        .iter()
        .map(|(e, a, t)| (e, t.translation.0, a.radius))
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
        let flat_pos = Vec3::new(transform.translation.x, 0.0, transform.translation.z);
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
            match navmesh.find_path(transform.translation.0, dest) {
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
            let dx = wp.x - transform.translation.x;
            let dz = wp.z - transform.translation.z;
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
        let dx = wp.x - transform.translation.x;
        let dz = wp.z - transform.translation.z;
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
            let sx = transform.translation.x - other_pos.x;
            let sz = transform.translation.z - other_pos.z;
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

        let move_dist = (agent.speed * dt).min(dist);
        transform.translation.x += final_dir.x * move_dist;
        transform.translation.z += final_dir.z * move_dist;
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
        app.world_mut().spawn((
            NavMeshAgent::new(5.0),
            Transform::from_translation(Vec3::ZERO),
        ));
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
    fn moves_toward_destination() {
        let mut app = open_app();
        app.world_mut().spawn((
            NavMeshAgent::new(5.0).with_destination(Vec3::new(3.0, 0.0, 0.0)),
            Transform::from_translation(Vec3::ZERO),
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
            t.translation.x > 0.0,
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
            Transform::from_translation(Vec3::ZERO),
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
            Transform::from_translation(Vec3::new(-3.0, 0.0, 0.0)),
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
            .spawn((agent, Transform::from_translation(Vec3::ZERO)));
        app.update();

        let t = app
            .world_mut()
            .query::<&Transform>()
            .iter(app.world())
            .next()
            .unwrap()
            .clone();
        assert_eq!(t.translation.x, 0.0, "disabled agent must not move");
    }

    #[test]
    fn path_recomputed_on_destination_change() {
        let mut app = open_app();
        let entity = app
            .world_mut()
            .spawn((
                NavMeshAgent::new(5.0).with_destination(Vec3::new(3.0, 0.0, 0.0)),
                Transform::from_translation(Vec3::ZERO),
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
}

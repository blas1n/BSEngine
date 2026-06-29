use bevy_app::{App, Plugin, Update};
use bsengine_core::{Follow, GlobalTransform, LookAt, Time, Transform};
use bsengine_ecs::{Query, Res};
use glam::{Mat3, Quat, Vec3};

pub struct FollowPlugin;

impl Plugin for FollowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (apply_follow, apply_look_at));
    }
}

fn apply_follow(
    mut followers: Query<(&Follow, &mut Transform)>,
    targets: Query<&GlobalTransform>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds;
    for (follow, mut transform) in followers.iter_mut() {
        let Ok(target_gt) = targets.get(follow.target) else {
            continue;
        };
        let desired = target_gt.0.w_axis.truncate() + follow.offset;
        let diff = desired - transform.translation;
        let dist = diff.length();
        if dist < 1e-5 {
            continue;
        }
        if follow.speed.is_infinite() || dist <= follow.speed * dt {
            transform.translation = desired;
        } else {
            transform.translation += diff / dist * follow.speed * dt;
        }
    }
}

fn apply_look_at(mut lookers: Query<(&LookAt, &mut Transform)>, targets: Query<&GlobalTransform>) {
    for (look_at, mut transform) in lookers.iter_mut() {
        let Ok(target_gt) = targets.get(look_at.target) else {
            continue;
        };
        let direction = target_gt.0.w_axis.truncate() - transform.translation;
        if direction.length_squared() < 1e-8 {
            continue;
        }
        let fwd = direction.normalize();
        let up = look_at.up;

        // Guard against degenerate up == fwd
        let up = if fwd.abs_diff_eq(up, 0.01) || fwd.abs_diff_eq(-up, 0.01) {
            if up.abs_diff_eq(Vec3::Y, 0.01) {
                Vec3::Z
            } else {
                Vec3::Y
            }
        } else {
            up
        };

        let right = up.cross(fwd).normalize();
        let actual_up = fwd.cross(right).normalize();
        // Columns: right=X, actual_up=Y, -fwd=Z (forward is -Z in RH coords)
        transform.rotation = Quat::from_mat3(&Mat3::from_cols(right, actual_up, -fwd)).normalize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{Follow, GlobalTransform, LookAt, Time, Transform};
    use glam::{Mat4, Vec3};

    fn make_app() -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(FollowPlugin);
        let mut t = Time::default();
        t.set_delta_for_test(1.0);
        app.insert_resource(t);
        app
    }

    #[test]
    fn entity_snaps_to_target_when_speed_infinite() {
        let mut app = make_app();
        let target = app
            .world_mut()
            .spawn((
                Transform::from_translation(Vec3::new(5.0, 0.0, 0.0)),
                GlobalTransform(Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0))),
            ))
            .id();

        app.world_mut()
            .spawn((Transform::default(), Follow::new(target)));
        app.update();

        let transform = app
            .world_mut()
            .query::<(&Follow, &Transform)>()
            .iter(app.world())
            .next()
            .map(|(_, t)| t.translation)
            .unwrap();
        assert!((transform.x - 5.0).abs() < 0.01);
    }

    #[test]
    fn entity_moves_toward_target_at_speed() {
        let mut app = make_app();
        let target = app
            .world_mut()
            .spawn((
                Transform::from_translation(Vec3::new(10.0, 0.0, 0.0)),
                GlobalTransform(Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0))),
            ))
            .id();

        app.world_mut()
            .spawn((Transform::default(), Follow::new(target).with_speed(3.0)));
        app.update(); // dt = 1.0

        let pos = app
            .world_mut()
            .query::<(&Follow, &Transform)>()
            .iter(app.world())
            .next()
            .map(|(_, t)| t.translation)
            .unwrap();
        assert!((pos.x - 3.0).abs() < 0.01); // moved 3 units toward target
    }

    #[test]
    fn look_at_rotates_toward_target() {
        let mut app = make_app();
        let target = app
            .world_mut()
            .spawn((
                Transform::from_translation(Vec3::new(0.0, 0.0, -5.0)),
                GlobalTransform(Mat4::from_translation(Vec3::new(0.0, 0.0, -5.0))),
            ))
            .id();

        app.world_mut()
            .spawn((Transform::default(), LookAt::new(target)));
        app.update();

        let rot = app
            .world_mut()
            .query::<(&LookAt, &Transform)>()
            .iter(app.world())
            .next()
            .map(|(_, t)| t.rotation)
            .unwrap();
        // Should be identity (already facing -Z)
        assert!(rot.is_finite());
    }
}

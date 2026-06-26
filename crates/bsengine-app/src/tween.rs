use bevy_app::{App, Plugin, Update};
use bsengine_core::{Time, Transform, Tween, TweenTarget};
use bsengine_ecs::{Query, Res};

pub struct TweenPlugin;

impl Plugin for TweenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tick_tweens);
    }
}

fn tick_tweens(mut query: Query<(&mut Tween, &mut Transform)>, time: Res<Time>) {
    for (mut tween, mut transform) in query.iter_mut() {
        if tween.finished {
            continue;
        }

        tween.elapsed += time.delta_seconds;

        let raw_t = (tween.elapsed / tween.duration).clamp(0.0, 1.0);
        let done = raw_t >= 1.0;

        let directional_t = if tween.reversed { 1.0 - raw_t } else { raw_t };
        let eased_t = tween.easing.apply(directional_t);

        match &tween.target {
            TweenTarget::Translation { from, to } => {
                transform.translation = from.lerp(*to, eased_t);
            }
            TweenTarget::Rotation { from, to } => {
                transform.rotation = from.slerp(*to, eased_t);
            }
            TweenTarget::Scale { from, to } => {
                transform.scale = from.lerp(*to, eased_t);
            }
        }

        if done {
            match tween.repeat {
                bsengine_core::RepeatMode::Once => {
                    tween.finished = true;
                }
                bsengine_core::RepeatMode::Loop => {
                    tween.elapsed -= tween.duration;
                }
                bsengine_core::RepeatMode::PingPong => {
                    tween.elapsed -= tween.duration;
                    tween.reversed = !tween.reversed;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{EasingFn, RepeatMode, Time, Transform, Tween, TweenTarget};
    use glam::Vec3;

    fn make_app_with_delta(delta: f32) -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(TweenPlugin);
        let mut t = Time::default();
        t.set_delta_for_test(delta);
        app.insert_resource(t);
        app
    }

    #[test]
    fn tween_plugin_builds() {
        let mut app = crate::new_app();
        app.add_plugins(TweenPlugin);
        app.insert_resource(Time::default());
    }

    #[test]
    fn translation_tween_at_half_duration() {
        let mut app = make_app_with_delta(0.5);
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                Tween::new(
                    TweenTarget::Translation {
                        from: Vec3::ZERO,
                        to: Vec3::new(10.0, 0.0, 0.0),
                    },
                    1.0,
                ),
            ))
            .id();

        app.update();

        let transform = app.world().get::<Transform>(entity).unwrap();
        assert!((transform.translation.x - 5.0).abs() < 0.01);
    }

    #[test]
    fn tween_once_finishes_after_full_duration() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                Tween::new(
                    TweenTarget::Translation {
                        from: Vec3::ZERO,
                        to: Vec3::X,
                    },
                    0.5,
                ),
            ))
            .id();

        app.update();

        let tween = app.world().get::<Tween>(entity).unwrap();
        assert!(tween.finished);
    }

    #[test]
    fn tween_loop_does_not_finish() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                Tween::new(
                    TweenTarget::Translation {
                        from: Vec3::ZERO,
                        to: Vec3::X,
                    },
                    0.5,
                )
                .with_repeat(RepeatMode::Loop),
            ))
            .id();

        app.update();

        let tween = app.world().get::<Tween>(entity).unwrap();
        assert!(!tween.finished);
    }

    #[test]
    fn tween_pingpong_reverses_direction() {
        let mut app = make_app_with_delta(0.5);
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                Tween::new(
                    TweenTarget::Translation {
                        from: Vec3::ZERO,
                        to: Vec3::X,
                    },
                    0.5,
                )
                .with_repeat(RepeatMode::PingPong),
            ))
            .id();

        app.update();

        let tween = app.world().get::<Tween>(entity).unwrap();
        assert!(tween.reversed);
        assert!(!tween.finished);
    }

    #[test]
    fn scale_tween_interpolates() {
        let mut app = make_app_with_delta(0.5);
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                Tween::new(
                    TweenTarget::Scale {
                        from: Vec3::ONE,
                        to: Vec3::splat(3.0),
                    },
                    1.0,
                ),
            ))
            .id();

        app.update();

        let transform = app.world().get::<Transform>(entity).unwrap();
        assert!((transform.scale.x - 2.0).abs() < 0.01);
    }

    #[test]
    fn rotation_tween_interpolates() {
        use glam::Quat;
        let mut app = make_app_with_delta(0.5);
        let from_rot = Quat::IDENTITY;
        let to_rot = Quat::from_rotation_y(std::f32::consts::PI);
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                Tween::new(
                    TweenTarget::Rotation {
                        from: from_rot,
                        to: to_rot,
                    },
                    1.0,
                ),
            ))
            .id();

        app.update();

        let transform = app.world().get::<Transform>(entity).unwrap();
        let expected = from_rot.slerp(to_rot, 0.5);
        assert!(transform.rotation.abs_diff_eq(expected, 1e-4));
    }

    #[test]
    fn easing_applied_to_tween() {
        let mut app = make_app_with_delta(0.5);
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                Tween::new(
                    TweenTarget::Translation {
                        from: Vec3::ZERO,
                        to: Vec3::X,
                    },
                    1.0,
                )
                .with_easing(EasingFn::EaseInQuad),
            ))
            .id();

        app.update();

        let transform = app.world().get::<Transform>(entity).unwrap();
        // EaseInQuad at t=0.5 is 0.25
        assert!((transform.translation.x - 0.25).abs() < 0.01);
    }
}

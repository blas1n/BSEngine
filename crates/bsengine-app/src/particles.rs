use bevy_app::{App, Plugin, Update};
use bsengine_core::{Particle, ParticleEmitter, Time, Transform};
use bsengine_ecs::{Query, Res};
use glam::Vec3;

/// Emits, integrates and ages every `ParticleEmitter`'s particles.
///
/// Component in `bsengine-core`, system here -- the split `LifetimePlugin`
/// already uses.
pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tick_emitters);
    }
}

/// A cap on live particles per emitter.
///
/// Reached only by a rate high enough to outrun the lifetime. Dropping the
/// excess beats growing without bound in a frame that is already behind, and it
/// is logged rather than silent because a truncated effect looks exactly like a
/// broken one.
const MAX_PARTICLES_PER_EMITTER: usize = 4096;

fn tick_emitters(mut emitters: Query<(&Transform, &mut ParticleEmitter)>, time: Res<Time>) {
    let dt = time.delta_seconds;
    for (transform, mut emitter) in emitters.iter_mut() {
        let origin = transform.position.0;

        // What to emit this tick: the burst debt, plus the continuous rate
        // carried as a fraction so a rate below one per frame still emits.
        let mut wanted = std::mem::take(&mut emitter.pending_burst) as usize;
        if emitter.enabled && emitter.rate > 0.0 {
            emitter.spawn_debt += emitter.rate * dt;
            let whole = emitter.spawn_debt.floor();
            emitter.spawn_debt -= whole;
            wanted += whole as usize;
        }

        let room = MAX_PARTICLES_PER_EMITTER.saturating_sub(emitter.live.len());
        if wanted > room {
            tracing::warn!(
                "[particles] emitter is at the {MAX_PARTICLES_PER_EMITTER} cap; dropping {} \
                 particles this tick",
                wanted - room
            );
            wanted = room;
        }

        let speed = emitter.initial_speed;
        for _ in 0..wanted {
            let direction = random_cone_direction(&mut emitter);
            emitter.live.push(Particle {
                position: origin,
                velocity: direction * speed,
                age: 0.0,
            });
        }

        // Integrate and age, dropping anything past its lifetime.
        let lifetime = emitter.particle_lifetime;
        let gravity = emitter.gravity;
        emitter.live.retain_mut(|p| {
            p.velocity.y -= gravity * dt;
            p.position += p.velocity * dt;
            p.age += dt;
            p.age < lifetime
        });
    }
}

/// A direction inside the emitter's cone, around +Y.
fn random_cone_direction(emitter: &mut ParticleEmitter) -> Vec3 {
    let spread = emitter
        .spread_degrees
        .to_radians()
        .clamp(0.0, std::f32::consts::PI);
    let cos_min = spread.cos();
    // Uniform over the spherical cap rather than over the angle, which would
    // bunch particles towards the tip of the cone.
    let cos_theta = 1.0 - emitter.rng.unit() * (1.0 - cos_min);
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let phi = emitter.rng.unit() * std::f32::consts::TAU;
    Vec3::new(sin_theta * phi.cos(), cos_theta, sin_theta * phi.sin())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::ParticleEmitter;

    /// An app with the plugin and a pinned timestep, so ageing is exact.
    fn app_with(emitter: ParticleEmitter) -> (bevy_app::App, bevy_ecs::entity::Entity) {
        let mut app = crate::new_app();
        // TimePlugin first, then the fixed clock over the top of the one it
        // inserts: `Time::fixed` only sets the step, and `delta_seconds` stays
        // zero until something calls `tick()`. Without the plugin every test
        // here runs at dt = 0, which looks like gravity, ageing and continuous
        // emission all being broken at once.
        app.add_plugins(crate::TimePlugin);
        app.add_plugins(ParticlePlugin);
        app.insert_resource(Time::fixed(0.1));
        let e = app.world_mut().spawn((Transform::default(), emitter)).id();
        (app, e)
    }

    fn live_count(app: &bevy_app::App, e: bevy_ecs::entity::Entity) -> usize {
        app.world().get::<ParticleEmitter>(e).unwrap().live.len()
    }

    #[test]
    fn a_burst_emits_exactly_its_count_once() {
        let (mut app, e) = app_with(ParticleEmitter {
            rate: 0.0,
            burst_count: 12,
            particle_lifetime: 100.0,
            ..Default::default()
        });
        app.world_mut()
            .get_mut::<ParticleEmitter>(e)
            .unwrap()
            .burst();

        app.update();
        assert_eq!(live_count(&app, e), 12);

        // Spent, not repeated every frame afterwards.
        app.update();
        assert_eq!(live_count(&app, e), 12);
    }

    #[test]
    fn particles_die_after_their_lifetime() {
        let (mut app, e) = app_with(ParticleEmitter {
            rate: 0.0,
            burst_count: 5,
            particle_lifetime: 0.25,
            ..Default::default()
        });
        app.world_mut()
            .get_mut::<ParticleEmitter>(e)
            .unwrap()
            .burst();
        app.update();
        assert_eq!(live_count(&app, e), 5);

        // 0.1s a tick; three more takes every particle past 0.25s.
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(live_count(&app, e), 0);
    }

    #[test]
    fn gravity_bends_a_particle_downward() {
        let (mut app, e) = app_with(ParticleEmitter {
            rate: 0.0,
            burst_count: 1,
            initial_speed: 0.0,
            gravity: 10.0,
            particle_lifetime: 100.0,
            ..Default::default()
        });
        app.world_mut()
            .get_mut::<ParticleEmitter>(e)
            .unwrap()
            .burst();
        app.update();
        let after_one = app.world().get::<ParticleEmitter>(e).unwrap().live[0]
            .position
            .y;
        app.update();
        let after_two = app.world().get::<ParticleEmitter>(e).unwrap().live[0]
            .position
            .y;
        assert!(
            after_two < after_one,
            "gravity should pull a particle down: {after_one} then {after_two}"
        );
    }

    #[test]
    fn a_disabled_emitter_stops_emitting_continuously_but_still_bursts() {
        // `enabled` governs the rate, not the emitter's existence. A hit-spark
        // emitter sits idle until something asks it for a burst.
        let (mut app, e) = app_with(ParticleEmitter {
            rate: 100.0,
            burst_count: 3,
            particle_lifetime: 100.0,
            enabled: false,
            ..Default::default()
        });
        app.update();
        assert_eq!(live_count(&app, e), 0);

        app.world_mut()
            .get_mut::<ParticleEmitter>(e)
            .unwrap()
            .burst();
        app.update();
        assert_eq!(live_count(&app, e), 3);
    }

    #[test]
    fn a_continuous_emitter_emits_at_its_rate() {
        // 20/s at 0.1s a tick is two per tick.
        let (mut app, e) = app_with(ParticleEmitter {
            rate: 20.0,
            particle_lifetime: 100.0,
            enabled: true,
            ..Default::default()
        });
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(live_count(&app, e), 10);
    }

    #[test]
    fn a_rate_below_one_per_frame_still_emits() {
        // 5/s at 0.1s a tick is half a particle per frame. Without the
        // fractional carry, truncation would emit nothing, ever.
        let (mut app, e) = app_with(ParticleEmitter {
            rate: 5.0,
            particle_lifetime: 100.0,
            enabled: true,
            ..Default::default()
        });
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(live_count(&app, e), 5);
    }

    #[test]
    fn particles_start_at_the_emitters_position() {
        // Emission reads the entity's Transform, which is the only reason the
        // simulation lives in a system rather than on the component.
        let mut app = crate::new_app();
        app.add_plugins(crate::TimePlugin);
        app.add_plugins(ParticlePlugin);
        app.insert_resource(Time::fixed(0.1));
        let e = app
            .world_mut()
            .spawn((
                Transform::from_position(Vec3::new(3.0, 5.0, -2.0)),
                ParticleEmitter {
                    rate: 0.0,
                    burst_count: 1,
                    initial_speed: 0.0,
                    gravity: 0.0,
                    particle_lifetime: 100.0,
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut()
            .get_mut::<ParticleEmitter>(e)
            .unwrap()
            .burst();
        app.update();

        let p = app.world().get::<ParticleEmitter>(e).unwrap().live[0];
        assert!(
            (p.position - Vec3::new(3.0, 5.0, -2.0)).length() < 1e-5,
            "expected the emitter's position, got {:?}",
            p.position
        );
    }
}

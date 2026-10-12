//! Particles, in pixels.
//!
//! The first feature in this engine designed on the assumption that pixel
//! readback exists. Item 28 was deferred precisely because half of it would
//! have been unobservable; these are the tests that made it not so.

mod common;

use bsengine_rhi_wgpu::particles::{ParticleBatch, ParticleInstance};
use common::{Draw, Harness, Light, Scene};
use glam::Vec3;

/// One bright particle at the origin.
fn one_at(position: Vec3, size: f32) -> Vec<ParticleBatch> {
    vec![ParticleBatch {
        texture_id: None,
        instances: vec![ParticleInstance {
            position: position.to_array(),
            size,
            color: [1.0, 0.9, 0.2, 1.0],
        }],
    }]
}

/// How many pixels differ from an empty frame -- the particle's footprint.
fn footprint(with: &common::Pixels, without: &common::Pixels) -> usize {
    (0..with.height)
        .flat_map(|y| (0..with.width).map(move |x| (x, y)))
        .filter(|&(x, y)| with.at(x, y) != without.at(x, y))
        .count()
}

/// A GPU emitter: `spawn` warm particles at the origin, barely moving.
fn gpu_emitter(tick: u64, spawn: u32) -> bsengine_rhi_wgpu::gpu_particles::GpuEmitterFrame {
    bsengine_rhi_wgpu::gpu_particles::GpuEmitterFrame {
        id: 1,
        tick,
        spawn,
        origin: Vec3::ZERO,
        dt: 0.01,
        seed: 9,
        capacity: 4096,
        lifetime: 5.0,
        speed: 0.5,
        spread_degrees: 180.0,
        gravity: 0.0,
        start_size: 0.08,
        end_size: 0.08,
        start_color: [1.0, 0.9, 0.2],
        end_color: [1.0, 0.9, 0.2],
        texture_id: None,
        collide: false,
        bounce: 0.5,
        friction: 0.0,
    }
}

/// GPU-simulated particles are drawn by the same pass as CPU ones: a cloud
/// of them reaches the framebuffer in their colour. The dead slots of the
/// ring -- drawn as zero-size quads -- draw nothing: an emitter that has
/// emitted nothing leaves the frame exactly as empty. And an emitter no
/// longer named is gone from the next frame.
#[test]
fn gpu_particles_reach_the_framebuffer_and_dead_slots_draw_nothing() {
    let mut h = Harness::new();
    let empty = h.render(&Scene::default());

    let all_dead = h.render(&Scene {
        gpu_particles: vec![gpu_emitter(1, 0)],
        ..Scene::default()
    });
    assert_eq!(
        footprint(&all_dead, &empty),
        0,
        "4096 dead slots, drawn as zero-size quads, must draw nothing"
    );

    let cloud = h.render(&Scene {
        gpu_particles: vec![gpu_emitter(2, 2000)],
        ..Scene::default()
    });
    assert!(
        footprint(&cloud, &empty) > 50,
        "2000 live particles around the origin should cover the frame's middle, saw {}",
        cloud.describe()
    );
    let [r, g, b, _] = cloud.centre();
    assert!(
        r > b + 40 && g > b + 40,
        "in their warm colour at the centre, saw {}",
        cloud.describe()
    );

    let gone = h.render(&Scene::default());
    assert_eq!(
        footprint(&gone, &empty),
        0,
        "an emitter not named in a frame is dropped, not drawn from stale buffers"
    );
}

#[test]
fn a_particle_reaches_the_framebuffer() {
    let mut h = Harness::new();
    let empty = h.render(&Scene::default());
    let with = h.render(&Scene {
        particles: one_at(Vec3::ZERO, 0.5),
        ..Scene::default()
    });

    assert!(
        with.differs_from(&empty),
        "a particle in front of the camera should be drawn, saw {}",
        with.describe()
    );
    let [r, g, b, _] = with.centre();
    assert!(
        r > b + 40 && g > b + 40,
        "expected the particle's warm colour at the centre, saw {}",
        with.describe()
    );
}

#[test]
fn a_particle_is_hidden_by_an_opaque_object_in_front_of_it() {
    let mut h = Harness::new();
    let cube = h.cube();

    // Depth testing stays on in the particle pass. Without it, sparks draw
    // through walls -- the same property the transparent pass needs, and the
    // same mistake it would be easy to make twice.
    let pixels = h.render(&Scene {
        draws: vec![Draw::new(cube, Vec3::new(0.0, 0.0, 2.0)).colour(Vec3::new(0.0, 1.0, 0.0))],
        particles: one_at(Vec3::new(0.0, 0.0, -1.0), 0.5),
        light: Light {
            color: Vec3::ZERO,
            ambient: Vec3::ONE,
            ..Light::default()
        },
        ..Scene::default()
    });

    let [r, g, _, _] = pixels.centre();
    assert!(
        g > r,
        "the cube in front should hide the particle behind it, saw {}",
        pixels.describe()
    );
}

#[test]
fn a_billboard_faces_the_camera_from_any_angle() {
    // The open question of this whole task. The quad is built from the camera's
    // right and up, pulled out of the view-projection; if that basis is wrong,
    // the quad collapses to an edge -- or vanishes -- when the camera moves off
    // one axis. Seen from three directions at the same distance, a billboard
    // covers comparable area every time; a fixed-orientation quad does not.
    let mut h = Harness::new();
    let positions = [
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::new(5.0, 0.0, 0.0),
        Vec3::new(3.0, 3.5, 3.0),
    ];

    let mut areas = Vec::new();
    for camera_pos in positions {
        let empty = h.render(&Scene {
            camera_pos,
            ..Scene::default()
        });
        let with = h.render(&Scene {
            particles: one_at(Vec3::ZERO, 0.5),
            camera_pos,
            ..Scene::default()
        });
        areas.push(footprint(&with, &empty));
    }

    assert!(
        areas.iter().all(|a| *a > 50),
        "the particle should be visible from every angle; footprints were {areas:?}"
    );
    let smallest = *areas.iter().min().unwrap() as f32;
    let largest = *areas.iter().max().unwrap() as f32;
    assert!(
        largest / smallest < 1.5,
        "a billboard should cover a comparable area from any angle; footprints \
         were {areas:?}, which differ by more than half"
    );
}

#[test]
fn a_particles_alpha_thins_it_rather_than_hiding_it() {
    // Alpha blending, not REPLACE. With REPLACE a half-transparent particle
    // paints its full colour and this reads the same as the opaque one.
    let mut h = Harness::new();
    let opaque = h.render(&Scene {
        particles: one_at(Vec3::ZERO, 0.5),
        ..Scene::default()
    });
    let faint = h.render(&Scene {
        particles: vec![ParticleBatch {
            texture_id: None,
            instances: vec![ParticleInstance {
                position: [0.0, 0.0, 0.0],
                size: 0.5,
                color: [1.0, 0.9, 0.2, 0.25],
            }],
        }],
        ..Scene::default()
    });

    assert!(
        faint.centre() != opaque.centre(),
        "alpha should change what a particle puts on screen; both read {:?}",
        faint.centre()
    );
    // And it thins towards the background rather than away from it.
    let empty = h.render(&Scene::default());
    let distance = |p: &common::Pixels| {
        let a = p.centre();
        let b = empty.centre();
        (a[0] as i32 - b[0] as i32).abs()
            + (a[1] as i32 - b[1] as i32).abs()
            + (a[2] as i32 - b[2] as i32).abs()
    };
    assert!(
        distance(&faint) < distance(&opaque),
        "a fainter particle should sit closer to the background: {} vs {}",
        distance(&faint),
        distance(&opaque)
    );
}

/// GPU particles collide with what the renderer actually drew: dropped onto
/// a floor, a colliding emitter's particles end the second on it, where the
/// same emitter without collision -- the premise -- has dropped them through
/// it. Through the real frame loop: each step reads the depth buffer the
/// previous frame left, through the matrix that frame drew with.
#[test]
fn gpu_particles_land_on_the_floor_the_frame_drew() {
    let mut h = Harness::new();
    let cube = h.cube();
    // Top face at y = -1, under the emitter at the origin and in view of the
    // camera at (0, 0, 5).
    let floor = || {
        Draw::new(cube, Vec3::ZERO).scaled(Vec3::new(20.0, 0.2, 20.0), Vec3::new(0.0, -1.1, 0.0))
    };
    let mut lowest_after = |collide: bool| {
        for tick in 1..=60 {
            let emitter = bsengine_rhi_wgpu::gpu_particles::GpuEmitterFrame {
                spawn: if tick == 1 { 50 } else { 0 },
                dt: 0.02,
                gravity: 10.0,
                collide,
                bounce: 0.2,
                ..gpu_emitter(tick, 0)
            };
            h.render(&Scene {
                draws: vec![floor()],
                gpu_particles: vec![emitter],
                ..Scene::default()
            });
        }
        let particles = h.gpu_particles(1).expect("the emitter has buffers");
        let live: Vec<f32> = particles
            .iter()
            .filter(|(i, _)| i.size > 0.0)
            .map(|(i, _)| i.position[1])
            .collect();
        assert_eq!(live.len(), 50, "premise: all fifty still live");
        // Dropped between frames so the next run starts from an empty ring.
        h.render(&Scene::default());
        live.into_iter().fold(f32::MAX, f32::min)
    };
    let without = lowest_after(false);
    assert!(
        without < -2.0,
        "premise: without collision they fall through the floor ({without})"
    );
    let with = lowest_after(true);
    assert!(
        with > -1.05,
        "with collision none sinks into it: lowest {with}"
    );
}

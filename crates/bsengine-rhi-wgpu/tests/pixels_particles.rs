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

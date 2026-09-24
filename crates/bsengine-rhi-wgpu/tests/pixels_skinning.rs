//! Compute skinning, seen in pixels.
//!
//! The registry's own tests read the skinned vertices back and check the
//! arithmetic; this checks the one thing they cannot: that the buffer the
//! compute pass writes is the buffer the render passes draw from. A cube whose
//! only joint moves it a unit and a half to the right must land on screen
//! exactly where a plain cube *transformed* by that same translation lands --
//! the same geometry at the same world position, so the same pixels, not
//! merely "different from before".

mod common;

use common::{Draw, Harness, Scene};
use glam::{Mat4, Vec3};

fn one(mesh: u64, position: Vec3) -> Scene {
    Scene {
        draws: vec![Draw::new(mesh, position)],
        ..Default::default()
    }
}

/// At rest the skinned cube is the cube; posed, it is the cube moved by its
/// joint -- pixel for pixel the same frame as the transform would give.
#[test]
fn a_skinned_cube_lands_where_its_joint_puts_it() {
    let mut h = Harness::new();
    let cube = h.cube();
    let skinned = h.skinned_cube(1);
    let shift = Vec3::new(1.5, 0.0, 0.0);

    let plain_at_origin = h.render(&one(cube, Vec3::ZERO));
    let plain_shifted = h.render(&one(cube, shift));
    assert!(
        plain_at_origin.differs_from(&plain_shifted),
        "premise: a unit and a half sideways must move the cube on screen"
    );

    // Rest: never skinned, so the buffer still holds the rest vertices.
    let rest = h.render(&one(skinned, Vec3::ZERO));
    assert!(
        !rest.differs_from(&plain_at_origin),
        "at rest the skinned cube must draw exactly as the plain cube: {}",
        rest.describe()
    );

    assert!(h.skin(skinned, &[Mat4::from_translation(shift)]));
    let posed = h.render(&one(skinned, Vec3::ZERO));
    assert!(
        posed.differs_from(&rest),
        "the joint must move the cube on screen"
    );
    assert_eq!(
        posed.max_channel_diff(&plain_shifted),
        0,
        "posed by its joint, the skinned cube must draw exactly as the plain cube \
         transformed by the same translation -- same geometry, same place, same pixels"
    );

    // And back: a second skin with the identity restores the rest frame, so
    // the pass rewrites the whole buffer each time rather than accumulating.
    assert!(h.skin(skinned, &[Mat4::IDENTITY]));
    let back = h.render(&one(skinned, Vec3::ZERO));
    assert_eq!(
        back.max_channel_diff(&plain_at_origin),
        0,
        "skinning with the identity must put every vertex back at rest"
    );
}

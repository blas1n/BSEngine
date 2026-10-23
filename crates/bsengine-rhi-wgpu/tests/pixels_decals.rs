//! Decals.
//!
//! The last test here is the one that justifies the whole DBuffer
//! construction. A decal blended on top of the finished frame would look
//! identical to this one in every other test in this file -- it is only when a
//! shadow falls across the decal that "the surface's albedo changed" and
//! "something was painted over the lighting" come apart.

mod common;

use bsengine_rhi_wgpu::decals::DecalDraw;
use common::{Draw, Harness, Light, Pixels, Scene};
use glam::{Mat4, Vec3};

/// A big flat floor at the origin. Its top face sits at y = 0.5.
fn floor(mesh: u64) -> Draw {
    Draw::new(mesh, Vec3::ZERO).scaled(Vec3::new(20.0, 1.0, 20.0), Vec3::ZERO)
}

/// A shallow camera, so the floor fills the frame at an angle.
const CAMERA: Vec3 = Vec3::new(0.0, 4.0, 7.0);

/// A decal box covering the floor's top face.
///
/// `normal_fade` is zero unless a test is about the fade: with a fade in the
/// way, a test measuring whether the decal arrived at all could fail for a
/// reason that has nothing to do with projection.
fn decal(size: Vec3, texture: u64) -> DecalDraw {
    DecalDraw {
        model: Mat4::from_translation(Vec3::new(0.0, 0.5, 0.0)) * Mat4::from_scale(size),
        opacity: 1.0,
        normal_fade: 0.0,
        texture: Some(texture),
        normal_texture: None,
    }
}

fn lit_scene(mesh: u64) -> Scene {
    Scene {
        draws: vec![floor(mesh)],
        camera_pos: CAMERA,
        look_at: Vec3::ZERO,
        ..Default::default()
    }
}

#[test]
fn a_decal_changes_the_floor_under_it() {
    let mut h = Harness::new();
    let cube = h.cube();
    let red = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);

    let plain = h.render(&lit_scene(cube));
    let with_decal = h.render(&Scene {
        decals: vec![decal(Vec3::new(8.0, 2.0, 8.0), red)],
        ..lit_scene(cube)
    });

    let before = plain.centre();
    let after = with_decal.centre();
    assert!(
        after[0] > before[0] && after[2] < before[2],
        "a red decal should make the floor redder and less blue: {before:?} -> {after:?}"
    );
}

#[test]
fn a_decal_is_bounded_by_its_box() {
    // ⚠️ The box has to be tested in *depth*, not across the screen. Only the
    // box's own silhouette produces fragments at all, so a shader that never
    // checked whether the reconstructed position is inside the box still leaves
    // the frame's corners alone -- an earlier version of this test compared
    // corner pixels and passed with the check deleted.
    //
    // This box floats well above the floor. The floor is inside its silhouette
    // and more than a unit below its underside, so it is exactly the surface a
    // missing check would paint and a working one must not.
    //
    // ⚠️ It has to be a *big* box. A thinner slab at the same height produced
    // no fragments at all from this camera -- measured, 0 differing pixels with
    // the check deleted -- so the fixture, not the shader, was what left the
    // floor alone.
    let mut h = Harness::new();
    let cube = h.cube();
    let red = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);

    let plain = h.render(&lit_scene(cube));
    let floating = h.render(&Scene {
        decals: vec![DecalDraw {
            model: Mat4::from_translation(Vec3::new(0.0, 3.0, 0.0))
                * Mat4::from_scale(Vec3::new(4.0, 2.0, 4.0)),
            opacity: 1.0,
            normal_fade: 0.0,
            texture: Some(red),
            normal_texture: None,
        }],
        ..lit_scene(cube)
    });

    assert!(
        !floating.differs_from(&plain),
        "a decal whose box does not reach the floor must leave it alone, even          where the box covers it on screen: {}",
        floating.describe()
    );

    // Paired with the above: the same box lowered onto the floor does paint it,
    // so the test cannot be passing because nothing ever draws.
    let resting = h.render(&Scene {
        decals: vec![decal(Vec3::new(2.0, 2.0, 2.0), red)],
        ..lit_scene(cube)
    });
    assert!(
        resting.differs_from(&plain),
        "and the same box over the floor has to paint it"
    );
}

#[test]
fn opacity_scales_how_much_of_the_floor_survives() {
    let mut h = Harness::new();
    let cube = h.cube();
    let red = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);
    let full = decal(Vec3::new(8.0, 2.0, 8.0), red);

    let plain = h.render(&lit_scene(cube));
    let transparent = h.render(&Scene {
        decals: vec![DecalDraw {
            opacity: 0.0,
            ..full
        }],
        ..lit_scene(cube)
    });
    let half = h.render(&Scene {
        decals: vec![DecalDraw {
            opacity: 0.5,
            ..full
        }],
        ..lit_scene(cube)
    });
    let opaque = h.render(&Scene {
        decals: vec![full],
        ..lit_scene(cube)
    });

    assert_eq!(
        transparent.centre(),
        plain.centre(),
        "opacity 0 has to be indistinguishable from no decal at all"
    );
    let redness = |p: &Pixels| p.centre()[0] as i32 - p.centre()[2] as i32;
    assert!(
        redness(&half) > redness(&plain) && redness(&half) < redness(&opaque),
        "half opacity should land between the two: plain {}, half {}, opaque {}",
        redness(&plain),
        redness(&half),
        redness(&opaque)
    );
}

#[test]
fn the_normal_fade_drops_a_decal_on_a_surface_it_barely_grazes() {
    // The floor faces straight up and the decal projects straight down, so
    // they are exactly aligned -- a fade threshold above 1 can never be met and
    // must remove the decal entirely. That is the same arithmetic that removes
    // it from a wall the box happens to clip, without needing a wall in frame.
    let mut h = Harness::new();
    let cube = h.cube();
    let red = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);
    let base = decal(Vec3::new(8.0, 2.0, 8.0), red);

    let plain = h.render(&lit_scene(cube));
    let faded = h.render(&Scene {
        decals: vec![DecalDraw {
            // Above any achievable dot product, so the smoothstep is 0
            // everywhere.
            normal_fade: 1.5,
            ..base
        }],
        ..lit_scene(cube)
    });
    let unfaded = h.render(&Scene {
        decals: vec![base],
        ..lit_scene(cube)
    });

    assert!(
        unfaded.differs_from(&plain),
        "the unfaded decal has to be visible for this to measure anything"
    );
    assert_eq!(
        faded.centre(),
        plain.centre(),
        "a fade nothing can satisfy must leave the floor alone"
    );
}

/// The reason this is a DBuffer and not a pass over the finished frame.
///
/// A decal is folded into albedo *before* lighting, so a shadow falling across
/// it dims it exactly as it dims the floor. Painted on afterwards, the decal
/// would be equally bright in light and in shadow -- and every other test in
/// this file would still pass.
///
/// How much the decal changed a pixel is compared where the caster's shadow
/// falls and where the floor is lit. Which pixels those are comes from a frame
/// without the caster, not from how dark the floor happens to be: this test
/// first sorted the decal's pixels by brightness and took the darkest tenth
/// as "shadowed" -- but the shadow covers a few hundred of its 20,000 pixels,
/// so that tenth was mostly lit floor, and the test passed only because SSAO
/// darkened the floor under the caster. With SSAO fixed it failed (lit 103,
/// "shadowed" 88). And in linear light, because what a shadow multiplies is
/// light, and sRGB's curve squeezes a step high up its range.
#[test]
fn a_decal_in_shadow_is_dimmed_by_the_shadow() {
    let mut h = Harness::new();
    let cube = h.cube();
    let red = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);

    // A caster above the floor, with the sun tilted so its shadow lands beside
    // it rather than underneath where the camera cannot see it.
    let caster = Draw::new(cube, Vec3::new(-1.5, 2.0, 0.0))
        .scaled(Vec3::new(1.5, 1.5, 1.5), Vec3::new(-1.5, 2.0, 0.0));
    let light = Light {
        direction: Vec3::new(-0.6, -1.0, -0.2).normalize(),
        // Far below the harness default of 0.15. Ambient reaches shadowed
        // floor too, so a bright ambient lifts the shadow until "dimmed by
        // the shadow" and "not dimmed at all" are a few percent apart --
        // which is a fixture making the property nearly unobservable
        // rather than a property that is nearly absent.
        ambient: Vec3::splat(0.02),
        ..Light::default()
    };
    let shadowed = Scene {
        draws: vec![floor(cube), caster],
        light: light.clone(),
        ..lit_scene(cube)
    };

    let no_caster = h.render(&Scene {
        light,
        ..lit_scene(cube)
    });
    let without = h.render(&shadowed);
    let with = h.render(&Scene {
        decals: vec![decal(Vec3::new(12.0, 2.0, 12.0), red)],
        ..shadowed
    });

    // Red, not luma: the floor is a blue-grey, and red is the channel this
    // decal writes.
    let touched = |x: u32, y: u32| with.at(x, y)[0] > without.at(x, y)[0];
    // The shadow: floor the caster took most of the light from (and not the
    // caster itself, which is brighter, not darker). Lit: floor the caster
    // left exactly as it was.
    let in_shadow = |x: u32, y: u32| {
        let (b, n) = (without.at(x, y), no_caster.at(x, y));
        luminance(b) < 0.5 * luminance(n)
    };
    let in_light = |x: u32, y: u32| without.at(x, y) == no_caster.at(x, y);
    let delta = |x: u32, y: u32| linear(with.at(x, y)[0]) - linear(without.at(x, y)[0]);

    let (mut lit, mut dark) = (Vec::new(), Vec::new());
    for y in 0..without.height {
        for x in 0..without.width {
            if !touched(x, y) {
                continue;
            }
            if in_shadow(x, y) {
                dark.push(delta(x, y));
            } else if in_light(x, y) {
                lit.push(delta(x, y));
            }
        }
    }
    // Premise: the decal covers both, in numbers worth averaging.
    assert!(
        dark.len() > 50 && lit.len() > 1000,
        "premise: decal pixels in shadow {} and in light {}",
        dark.len(),
        lit.len()
    );
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
    let (lit, dark) = (mean(&lit), mean(&dark));
    assert!(
        dark < lit * 0.5,
        "the decal should be markedly dimmer where the floor is shadowed, \
         because what changed is albedo and the shadow still multiplies it: \
         lit {lit:.3}, shadowed {dark:.3} (linear red)"
    );
}

/// An opaque decal gives the floor its texture's colour exactly: a grey decal
/// over a white floor reads as the same floor textured with that grey. Grey,
/// not red: the decal colour is kept sRGB-encoded and decoded where it is
/// read, and a channel at 0 or 255 decodes to itself -- red would pass with
/// the decode left out, this grey reads half again as bright.
#[test]
fn an_opaque_decal_is_its_texture_s_colour() {
    let mut h = Harness::new();
    let plane = h.plane();
    let grey = h.two_colour_texture([128, 128, 128, 255], [128, 128, 128, 255]);
    let white_floor = || {
        Draw::new(plane, Vec3::ZERO)
            .scaled(Vec3::new(20.0, 1.0, 20.0), Vec3::new(0.0, 0.5, 0.0))
            .colour(Vec3::ONE)
    };
    let decalled = h.render(&Scene {
        draws: vec![white_floor()],
        decals: vec![decal(Vec3::new(8.0, 2.0, 8.0), grey)],
        camera_pos: CAMERA,
        look_at: Vec3::ZERO,
        ..Default::default()
    });
    let textured = h.render(&Scene {
        draws: vec![white_floor().textured(grey)],
        camera_pos: CAMERA,
        look_at: Vec3::ZERO,
        ..Default::default()
    });
    let (a, b) = (decalled.centre(), textured.centre());
    assert!(b[0] > 30 && b[0] < 220, "premise: a mid grey: {b:?}");
    for c in 0..3 {
        assert!(
            (a[c] as i32 - b[c] as i32).abs() <= 2,
            "the decal's grey is the texture's: {a:?} against {b:?}"
        );
    }
}

/// An sRGB-encoded channel as linear light, 0..1.
fn linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// A pixel's brightness in display values, for "darker than" comparisons.
fn luminance(p: [u8; 4]) -> f32 {
    0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32
}

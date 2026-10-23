//! Screen-space GI: the trace through the depth buffer, and the composite
//! that replaces each pixel's indirect diffuse light where the trace found
//! something.
//!
//! The fixture: a white floor running away from the camera, a wall standing
//! on it to the right, and the flat ambient as the light everything had
//! before -- seen from above and behind, so the floor beside the wall and
//! the wall's face are both in view.

mod common;

use bsengine_core::{
    AmbientOcclusion, Bloom, ScreenSpaceGlobalIllumination, ToneMap, ToneMappingMode,
};
use common::{Draw, Harness, Light, Pixels, Scene, HEIGHT, WIDTH};
use glam::Vec3;

const AMBIENT: Vec3 = Vec3::splat(0.3);
/// The floor's albedo: coloured, so a reflectance applied wrongly per channel
/// shows.
const FLOOR: Vec3 = Vec3::new(0.8, 0.5, 0.3);

/// Columns of floor beside the wall's foot (the wall stands at about 13/24
/// to 19/24 of the frame, from this camera), and of floor far from it.
const NEAR: (u32, u32) = (WIDTH * 9 / 24, WIDTH * 12 / 24);
const FAR: (u32, u32) = (WIDTH / 24, WIDTH * 4 / 24);

fn vertex(p: [f32; 3], n: [f32; 3]) -> bsengine_rhi_wgpu::Vertex {
    bsengine_rhi_wgpu::Vertex {
        position: p,
        color: [1.0, 1.0, 1.0],
        normal: n,
        uv: [0.0, 0.0],
        uv2: [0.0, 0.0],
        tangent: [0.0; 4],
    }
}

/// The floor, y = 0 from x = -8 to 1 and z = -2 to -16, facing up: wide
/// enough to fill the frame's left, where the floor far from the wall is.
fn floor(h: &mut Harness) -> u64 {
    let n = [0.0, 1.0, 0.0];
    h.mesh(
        &[
            vertex([-8.0, 0.0, -2.0], n),
            vertex([1.0, 0.0, -2.0], n),
            vertex([1.0, 0.0, -16.0], n),
            vertex([-8.0, 0.0, -16.0], n),
        ],
        &[0, 1, 2, 0, 2, 3],
    )
}

/// The wall, x = 1 from y = 0 to 3 along the floor, facing the floor (-x).
fn wall(h: &mut Harness) -> u64 {
    let n = [-1.0, 0.0, 0.0];
    h.mesh(
        &[
            vertex([1.0, 0.0, -2.0], n),
            vertex([1.0, 0.0, -16.0], n),
            vertex([1.0, 3.0, -16.0], n),
            vertex([1.0, 3.0, -2.0], n),
        ],
        // Wound to face -x, toward the floor and the camera.
        &[0, 2, 1, 0, 3, 2],
    )
}

fn scene(draws: Vec<Draw>, ssgi: Option<ScreenSpaceGlobalIllumination>) -> Scene {
    Scene {
        draws,
        light: Light {
            color: Vec3::ZERO,
            ambient: AMBIENT,
            ..Light::default()
        },
        camera_pos: Vec3::new(-1.5, 3.0, 2.0),
        look_at: Vec3::new(0.0, 0.0, -8.0),
        bloom: Some(Bloom::default().disabled()),
        ssao: Some(AmbientOcclusion::default().disabled()),
        tone_map: Some(ToneMap::new(ToneMappingMode::None)),
        ssgi,
        ..Scene::default()
    }
}

fn on() -> Option<ScreenSpaceGlobalIllumination> {
    Some(ScreenSpaceGlobalIllumination {
        rays: 16,
        steps: 48,
        stride: 0.1,
        thickness: 0.002,
        ..Default::default()
    })
}

fn linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Mean linear rgb over the floor pixels in a column band `x0..x1` of the
/// middle rows -- the floor's pixels are the grey ones, the wall's red or
/// black, so those are left out by colour.
fn floor_band(p: &Pixels, x0: u32, x1: u32) -> Vec3 {
    let mut sum = Vec3::ZERO;
    let mut n = 0.0;
    for y in HEIGHT / 2..HEIGHT * 3 / 4 {
        for x in x0..x1 {
            let [r, g, b, _] = p.at(x, y);
            // The floor's colour: red over green over blue, as its albedo.
            if r > g && g > b && b > 10 {
                sum += Vec3::new(linear(r), linear(g), linear(b));
                n += 1.0;
            }
        }
    }
    assert!(n > 20.0, "premise: floor pixels in {x0}..{x1}: {n}");
    sum / n
}

/// One frame of a scene -- the first, on a surface of its own, so screen-space
/// GI's history holds nothing of another scene and the frame is this trace
/// alone (the temporal tests render sequences on purpose).
fn first_frame(
    ssgi: Option<ScreenSpaceGlobalIllumination>,
    draws: impl FnOnce(&mut Harness) -> Vec<Draw>,
) -> Pixels {
    let mut h = Harness::new();
    let d = draws(&mut h);
    h.render(&scene(d, ssgi))
}

/// The floor, and the wall standing on it glowing `glow` (reflecting nothing).
fn floor_and_wall(glow: Vec3) -> impl FnOnce(&mut Harness) -> Vec<Draw> {
    move |h| {
        let (f, w) = (floor(h), wall(h));
        vec![
            Draw::new(f, Vec3::ZERO).colour(FLOOR).roughness(1.0),
            Draw::new(w, Vec3::ZERO).colour(Vec3::ZERO).emissive(glow),
        ]
    }
}

fn floor_alone(h: &mut Harness) -> Vec<Draw> {
    let f = floor(h);
    vec![Draw::new(f, Vec3::ZERO).colour(FLOOR).roughness(1.0)]
}

const RED: Vec3 = Vec3::new(3.0, 0.0, 0.0);

/// The red wall's light lands on the floor beside it: the floor near the wall
/// reddens -- red up, green and blue barely -- and more than the floor far
/// from it does.
#[test]
fn a_red_wall_bleeds_onto_the_floor_beside_it() {
    let off = first_frame(None, floor_and_wall(RED));
    let lit = first_frame(on(), floor_and_wall(RED));
    let (near_off, near_on) = (
        floor_band(&off, NEAR.0, NEAR.1),
        floor_band(&lit, NEAR.0, NEAR.1),
    );
    let (far_off, far_on) = (
        floor_band(&off, FAR.0, FAR.1),
        floor_band(&lit, FAR.0, FAR.1),
    );
    let gain = near_on - near_off;
    assert!(
        gain.x > 0.05,
        "the floor by the wall reddens: {near_off} -> {near_on}"
    );
    assert!(
        gain.y.abs() < gain.x * 0.3 && gain.z.abs() < gain.x * 0.3,
        "red, not white: {gain}"
    );
    assert!(
        gain.x > 2.0 * (far_on - far_off).x,
        "more by the wall than away from it: {gain} vs {}",
        far_on - far_off
    );
}

/// What the trace finds replaces the light the surface had, rather than
/// adding to it: beside a black wall, the floor's rays that meet the wall
/// bring nothing back where the flat ambient had given them light, and the
/// floor there darkens. A trace that only added could never do that.
#[test]
fn a_black_wall_darkens_the_floor_it_shades() {
    let before = floor_band(
        &first_frame(None, floor_and_wall(Vec3::ZERO)),
        NEAR.0,
        NEAR.1,
    );
    let after = floor_band(
        &first_frame(on(), floor_and_wall(Vec3::ZERO)),
        NEAR.0,
        NEAR.1,
    );
    assert!(
        after.y < before.y * 0.9,
        "the black wall takes its share of the sky: {before} -> {after}"
    );
    // But only its share: beside its foot the wall fills at most half of the
    // floor's hemisphere, so the floor keeps at least about half its light.
    // A ray counted as hitting more than once would take more than all of it.
    assert!(
        after.y > before.y * 0.4,
        "no more than the wall's share: {before} -> {after}"
    );
}

/// The trace's light stands in for the surface's own exactly: a wall as
/// bright as the ambient it hides -- glowing 0.3, reflecting nothing --
/// changes nothing on the floor beside it, though the floor's rays find it
/// (the premise: the same wall black darkens the floor). Holds the units
/// (the shading keeps irradiance as `E / PI`), the reflectance per channel
/// and the share replaced to one another.
#[test]
fn a_wall_as_bright_as_the_ambient_changes_nothing() {
    let band = |p: &Pixels| floor_band(p, NEAR.0, NEAR.1);
    let before = band(&first_frame(None, floor_and_wall(AMBIENT)));
    let after = band(&first_frame(on(), floor_and_wall(AMBIENT)));
    let dark_off = band(&first_frame(None, floor_and_wall(Vec3::ZERO)));
    let darkened = dark_off - band(&first_frame(on(), floor_and_wall(Vec3::ZERO)));
    assert!(
        darkened.min_element() > 0.0 && darkened.y > 0.1 * dark_off.y,
        "premise: the floor's rays find the wall: {darkened}"
    );
    // Per channel, against what the same rays take away when the wall is
    // black -- the share of the light they replace there -- so a reflectance
    // wrong in one channel shows even where few rays hit.
    let changed = (after - before).abs();
    for c in 0..3 {
        assert!(
            changed[c] < 0.1 * darkened[c],
            "channel {c}: a wall as bright as the ambient moves it {} of the {} a black one takes ({before} -> {after})",
            changed[c],
            darkened[c]
        );
    }
}

/// The intensity scales the replacement: at a half, the black wall takes half
/// of what it takes at one.
#[test]
fn the_intensity_scales_what_is_replaced() {
    let band = |p: &Pixels| floor_band(p, NEAR.0, NEAR.1);
    let off = band(&first_frame(None, floor_and_wall(Vec3::ZERO)));
    let full = band(&first_frame(on(), floor_and_wall(Vec3::ZERO)));
    let half_on = on().map(|s| ScreenSpaceGlobalIllumination {
        intensity: 0.5,
        ..s
    });
    let half = band(&first_frame(half_on, floor_and_wall(Vec3::ZERO)));
    let (d1, d2) = ((off - full).y, (off - half).y);
    assert!(d1 > 0.01, "premise: the wall darkens the floor: {d1}");
    assert!(
        (d2 / d1 - 0.5).abs() < 0.1,
        "half the intensity, half the change: {d2} of {d1}"
    );
}

/// Where no ray can find anything -- a floor with nothing above it -- the
/// floor is lit as without screen-space GI, byte for byte: every ray misses
/// and the composite adds exactly zero. And a disabled component is no
/// component, the whole frame identical.
#[test]
fn with_nothing_to_find_the_frame_is_unchanged() {
    let off = first_frame(None, floor_alone);
    let lit = first_frame(on(), floor_alone);
    // The floor's own interior, byte for byte. At its outer edge, where the
    // depth jumps to the background, a grazing ray can pass behind the edge
    // within the thickness and count as a hit -- a handful of pixels, the
    // depth buffer's own ambiguity rather than light from anything (the
    // spatial filter spreads each a pixel or two).
    for y in HEIGHT / 4..HEIGHT * 3 / 4 {
        for x in WIDTH / 4..WIDTH * 3 / 4 {
            assert_eq!(lit.at(x, y), off.at(x, y), "({x}, {y})");
        }
    }
    assert!(
        lit.differing_pixels(&off) <= 40,
        "only the edge: {} pixels",
        lit.differing_pixels(&off)
    );
    let disabled = first_frame(
        Some(ScreenSpaceGlobalIllumination {
            enabled: false,
            ..Default::default()
        }),
        floor_alone,
    );
    assert!(!disabled.differs_from(&off));
}

/// Mean absolute difference between horizontally neighbouring pixels' red,
/// in linear light, over the floor beside the wall: how noisy the floor is
/// there (the light itself varies far more slowly than a pixel).
fn noise(p: &Pixels) -> f32 {
    let mut sum = 0.0f32;
    let mut n = 0.0f32;
    for y in HEIGHT / 2..HEIGHT * 3 / 4 {
        for x in NEAR.0..NEAR.1 - 1 {
            let (a, b) = (p.at(x, y), p.at(x + 1, y));
            if a[0] > a[1] && b[0] > b[1] {
                sum += (linear(a[0]) - linear(b[0])).abs();
                n += 1.0;
            }
        }
    }
    sum / n.max(1.0)
}

fn few_rays() -> Option<ScreenSpaceGlobalIllumination> {
    on().map(|s| ScreenSpaceGlobalIllumination { rays: 2, ..s })
}

/// Frames accumulate: with two rays a pixel the first frame is noisy, and
/// after two dozen -- each turned differently -- the floor beside the red
/// wall is smoother, and its light the same: the mean of many frames of two
/// rays is the mean of one frame of sixteen, as an average of independent
/// estimates of the same light should be.
#[test]
fn frames_accumulate_into_less_noise_and_the_same_light() {
    let mut h = Harness::new();
    let draws = floor_and_wall(RED)(&mut h);
    let s = scene(draws, few_rays());
    let first = noise(&h.render_at_frame(&s, 0));
    for frame in 1..31 {
        h.render_at_frame(&s, frame);
    }
    let last = h.render_at_frame(&s, 31);
    assert!(
        noise(&last) < 0.6 * first,
        "the history settles the noise: {first} -> {}",
        noise(&last)
    );
    let reference = floor_band(&first_frame(on(), floor_and_wall(RED)), NEAR.0, NEAR.1);
    let settled = floor_band(&last, NEAR.0, NEAR.1);
    assert!(
        ((settled - reference) / reference).abs().max_element() < 0.05,
        "the same light: {settled} vs {reference}"
    );
}

/// A surface just uncovered has no history: where the wall stood, the far
/// wall now showing is that wall's first frame, not a blend into what the
/// near wall gathered. Its view depth is not what the history recorded
/// there. The floor glows while the near wall stands, so what the near wall
/// gathered -- the floor's light, from close by -- is far from what the far
/// wall's own trace finds, and a blend into it would show.
///
/// The far wall is what makes this a measurement: with nothing behind the
/// near wall, the uncovered pixels are background, which the temporal pass
/// leaves alone whatever it does with a history -- the first version of
/// this test compared background with background and passed with the
/// rejection deleted.
#[test]
fn an_uncovered_surface_starts_its_history_over() {
    let mut h = Harness::new();
    let (f, w) = (floor(&mut h), wall(&mut h));
    let draws = move |near: bool| {
        let mut far = Draw::new(w, Vec3::ZERO).colour(FLOOR).roughness(1.0);
        // Three further along x, and reaching below the floor's plane, so
        // that it fills every ray that went past the near wall's place.
        far.transform = glam::Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 2.0, 1.0),
            glam::Quat::IDENTITY,
            Vec3::new(3.0, -3.0, 0.0),
        );
        let mut d = vec![
            Draw::new(f, Vec3::ZERO)
                .colour(FLOOR)
                .roughness(1.0)
                .emissive(Vec3::splat(0.4)),
            far,
        ];
        if near {
            d.push(Draw::new(w, Vec3::ZERO).colour(Vec3::ONE));
        }
        d
    };
    for frame in 0..20 {
        h.render_at_frame(&scene(draws(true), on()), frame);
    }
    let uncovered = h.render_at_frame(&scene(draws(false), on()), 20);
    // The same frame's rays, on a renderer with no history at all.
    let fresh = {
        let mut h = Harness::new();
        let (f2, w2) = (floor(&mut h), wall(&mut h));
        assert_eq!((f2, w2), (f, w), "premise: the same mesh ids");
        h.render_at_frame(&scene(draws(false), on()), 20)
    };
    // Where the near wall stood: the columns it covered, middle rows -- the
    // pixels of those that show the far wall now (lit brighter than the
    // background's 80); the rest still look past it.
    let mut worst = 0i32;
    let (mut n, mut brightest) = (0, 0u8);
    for y in HEIGHT * 3 / 8..HEIGHT / 2 {
        for x in WIDTH * 14 / 24..WIDTH * 17 / 24 {
            if fresh.at(x, y)[0] <= 100 {
                continue;
            }
            n += 1;
            brightest = brightest.max(fresh.at(x, y)[0]);
            for c in 0..3 {
                worst = worst
                    .max((i32::from(uncovered.at(x, y)[c]) - i32::from(fresh.at(x, y)[c])).abs());
            }
        }
    }
    // Premise: the far wall fills a good part of the near wall's place, and
    // is not clipped, which would hide a blend under 255.
    assert!(
        n > 200 && brightest < 240,
        "premise: the far wall, unclipped: {n} pixels, up to {brightest}"
    );
    assert!(
        worst <= 3,
        "the uncovered wall is its own first frame: off by {worst}"
    );
}

/// The spatial filter takes noise out of a single frame: with two rays a
/// pixel, the first frame's floor beside the red wall is far less noisy
/// filtered than not, and lit the same.
#[test]
fn the_spatial_filter_smooths_a_frame() {
    let one = |spatial: bool| {
        let mut h = Harness::new();
        h.set_ssgi_spatial(spatial);
        // Dimmer than `RED`: a noisy pixel over 1 clips in the 8-bit frame,
        // which would bias the unfiltered mean low.
        let d = floor_and_wall(Vec3::new(0.8, 0.0, 0.0))(&mut h);
        h.render(&scene(d, few_rays()))
    };
    let (raw, filtered) = (one(false), one(true));
    assert!(
        noise(&filtered) < 0.6 * noise(&raw),
        "filtered {} against raw {}",
        noise(&filtered),
        noise(&raw)
    );
    let (a, b) = (
        floor_band(&raw, NEAR.0, NEAR.1),
        floor_band(&filtered, NEAR.0, NEAR.1),
    );
    assert!(
        ((a - b) / a).abs().max_element() < 0.05,
        "the same light: {a} vs {b}"
    );
}

/// A white plate floating over the floor beside the red wall, facing up.
fn plate(h: &mut Harness) -> u64 {
    let n = [0.0, 1.0, 0.0];
    h.mesh(
        &[
            vertex([-0.6, 0.6, -5.0], n),
            vertex([0.4, 0.6, -5.0], n),
            vertex([0.4, 0.6, -8.0], n),
            vertex([-0.6, 0.6, -8.0], n),
        ],
        &[0, 1, 2, 0, 2, 3],
    )
}

/// A moving surface keeps its history: the plate jumps between two places
/// beside the wall every frame, and its own velocity finds where it was --
/// so its light settles over the frames as a still surface's does. The two
/// places do not overlap: reprojected by the camera alone, every frame would
/// look where the plate no longer is, find the floor's depth there, and
/// start over, as noisy as the first. (A plate sliding within its own plane
/// would not show this: the same depth lies under every pixel either way.)
#[test]
fn a_moving_surface_keeps_its_history() {
    let at = |frame: u32| {
        glam::Mat4::from_translation(Vec3::new(
            if frame.is_multiple_of(2) { 0.0 } else { -1.3 },
            0.0,
            0.0,
        ))
    };
    let mut h = Harness::new();
    let (f, w, pl) = (floor(&mut h), wall(&mut h), plate(&mut h));
    let frame_of = |frame: u32| {
        let mut moving = Draw::new(pl, Vec3::ZERO).colour(Vec3::ONE).roughness(1.0);
        moving.transform = at(frame);
        let mut d = vec![
            Draw::new(f, Vec3::ZERO).colour(FLOOR).roughness(1.0),
            Draw::new(w, Vec3::ZERO)
                .colour(Vec3::ZERO)
                .emissive(Vec3::new(0.8, 0.0, 0.0)),
        ];
        d.push(if frame == 0 {
            moving
        } else {
            moving.moved_from(at(frame - 1))
        });
        d
    };
    let last = 24u32;
    // The plate's pixels in the last frame, marked by drawing it magenta there.
    let mask = {
        let mut m = Harness::new();
        let pl = plate(&mut m);
        let mut marker = Draw::new(pl, Vec3::ZERO)
            .colour(Vec3::ZERO)
            .emissive(Vec3::new(1.0, 0.0, 1.0));
        marker.transform = at(last);
        m.render(&scene(vec![marker], None))
    };
    let plate_noise = |p: &Pixels| {
        let mut sum = 0.0f32;
        let mut n = 0.0f32;
        for y in 0..HEIGHT {
            for x in 0..WIDTH - 1 {
                let (a, b) = (mask.at(x, y), mask.at(x + 1, y));
                if a[0] > 200 && a[2] > 200 && b[0] > 200 && b[2] > 200 {
                    sum += (linear(p.at(x, y)[0]) - linear(p.at(x + 1, y)[0])).abs();
                    n += 1.0;
                }
            }
        }
        assert!(n > 50.0, "premise: the plate is in view: {n}");
        sum / n
    };
    let first = {
        let mut f0 = Harness::new();
        let (_, _, pl0) = (floor(&mut f0), wall(&mut f0), plate(&mut f0));
        assert_eq!(pl0, pl, "premise: the same mesh ids");
        // The last frame's scene, as a first frame: its noise unaccumulated.
        f0.render_at_frame(&scene(frame_of(last), few_rays()), last)
    };
    for frame in 0..last {
        h.render_at_frame(&scene(frame_of(frame), few_rays()), frame);
    }
    let settled = h.render_at_frame(&scene(frame_of(last), few_rays()), last);
    assert!(
        plate_noise(&settled) < 0.6 * plate_noise(&first),
        "the moving plate settles: {} -> {}",
        plate_noise(&first),
        plate_noise(&settled)
    );
}

/// How far the spatial filter moves the pixels of one surface that lie
/// within its 5 x 5 reach of another: the mean, per channel, of filtered
/// minus unfiltered light over them -- signed, so the rays' noise, which the
/// filter takes out, averages away and what is left is light carried over
/// from the other side. `draws` builds the scene, `marked` the same scene
/// with the surface measured glowing green and every other surface blue.
fn spill(
    draws: impl Fn(&mut Harness) -> Vec<Draw>,
    marked: impl Fn(&mut Harness) -> Vec<Draw>,
) -> (Vec3, u32) {
    let one = |spatial: bool| {
        let mut h = Harness::new();
        h.set_ssgi_spatial(spatial);
        let d = draws(&mut h);
        h.render(&scene(d, on()))
    };
    let (raw, filtered) = (one(false), one(true));
    let mask = first_frame(None, &marked);
    let green = |x: u32, y: u32| {
        let c = mask.at(x, y);
        c[1] > 200 && c[2] < 50
    };
    let blue = |x: i32, y: i32| {
        let (x, y) = (
            x.clamp(0, WIDTH as i32 - 1) as u32,
            y.clamp(0, HEIGHT as i32 - 1) as u32,
        );
        let c = mask.at(x, y);
        c[2] > 200 && c[1] < 50
    };
    let mut sum = Vec3::ZERO;
    let mut n = 0;
    for y in 2..HEIGHT - 2 {
        for x in 2..WIDTH - 2 {
            if !green(x, y) {
                continue;
            }
            let near = (-2..=2).any(|dy| (-2..=2).any(|dx| blue(x as i32 + dx, y as i32 + dy)));
            if !near {
                continue;
            }
            let (a, b) = (filtered.at(x, y), raw.at(x, y));
            sum += Vec3::new(
                linear(a[0]) - linear(b[0]),
                linear(a[1]) - linear(b[1]),
                linear(a[2]) - linear(b[2]),
            );
            n += 1;
        }
    }
    (sum / n.max(1) as f32, n)
}

/// The spatial filter keeps to one surface across a crease: the floor
/// beside the red wall meets it at the same depth, so only the normals tell
/// them apart. The wall's rays find the floor's light -- green in it, which
/// the floor's own rays, finding the wall, do not -- and the floor within the
/// filter's reach of the wall keeps its own: filtered, it moves by under a
/// sixth of what it does with the normal weight left out (-0.006 red here,
/// against -0.039).
#[test]
fn the_spatial_filter_keeps_to_one_surface_across_a_crease() {
    let (moved, n) = spill(
        |h: &mut Harness| floor_and_wall(Vec3::new(0.8, 0.0, 0.0))(h),
        |h| {
            let (f, w) = (floor(h), wall(h));
            vec![
                Draw::new(f, Vec3::ZERO)
                    .colour(Vec3::ZERO)
                    .emissive(Vec3::new(0.0, 1.0, 0.0)),
                Draw::new(w, Vec3::ZERO)
                    .colour(Vec3::ZERO)
                    .emissive(Vec3::new(0.0, 0.0, 1.0)),
            ]
        },
    );
    assert!(n > 100, "premise: the floor along the wall is in view: {n}");
    assert!(
        moved.abs().max_element() < 0.015,
        "the floor keeps its own light: {moved}"
    );
}

/// The spatial filter keeps to one surface across a step: the plate glows
/// green over the floor, facing up as the floor does, so only the depth
/// tells them apart. The floor's rays find the plate's green and the plate's
/// own rays (up, at the wall and the sky) do not, and the plate within the
/// filter's reach of the floor keeps its own: filtered, its green moves by
/// almost nothing (-0.0002 here; +0.05 with the depth weight left out).
#[test]
fn the_spatial_filter_keeps_to_one_surface_across_a_step() {
    let (moved, n) = spill(
        |h: &mut Harness| {
            let (f, w, pl) = (floor(h), wall(h), plate(h));
            vec![
                Draw::new(f, Vec3::ZERO).colour(FLOOR).roughness(1.0),
                Draw::new(w, Vec3::ZERO)
                    .colour(Vec3::ZERO)
                    .emissive(Vec3::new(0.8, 0.0, 0.0)),
                Draw::new(pl, Vec3::ZERO)
                    .colour(Vec3::ONE)
                    .roughness(1.0)
                    .emissive(Vec3::new(0.0, 0.5, 0.0)),
            ]
        },
        |h| {
            let (f, w, pl) = (floor(h), wall(h), plate(h));
            let blue = Vec3::new(0.0, 0.0, 1.0);
            vec![
                Draw::new(f, Vec3::ZERO).colour(Vec3::ZERO).emissive(blue),
                Draw::new(w, Vec3::ZERO).colour(Vec3::ZERO).emissive(blue),
                Draw::new(pl, Vec3::ZERO)
                    .colour(Vec3::ZERO)
                    .emissive(Vec3::new(0.0, 1.0, 0.0)),
            ]
        },
    );
    assert!(n > 50, "premise: the plate's rim is in view: {n}");
    assert!(
        moved.abs().max_element() < 0.015,
        "the plate keeps its own light: {moved}"
    );
}

/// Switched off and back on, the trace starts over: the history it left
/// when switched off is of a scene that may have changed since -- here the
/// wall stops glowing red while the trace is off -- and the depth check
/// cannot tell, because nothing moved. The first frame back is the black
/// wall's own first frame, not a blend into the red one's history.
#[test]
fn switched_back_on_it_starts_over() {
    let draws = |f: u64, w: u64, glow: Vec3| {
        vec![
            Draw::new(f, Vec3::ZERO).colour(FLOOR).roughness(1.0),
            Draw::new(w, Vec3::ZERO).colour(Vec3::ZERO).emissive(glow),
        ]
    };
    // Twenty frames of the red wall, then the black one at frame 21 --
    // `off` frames with the trace switched off in between.
    let after = |off: u32| {
        let mut h = Harness::new();
        let (f, w) = (floor(&mut h), wall(&mut h));
        for frame in 0..20 {
            h.render_at_frame(&scene(draws(f, w, RED), on()), frame);
        }
        for frame in 20..20 + off {
            h.render_at_frame(&scene(draws(f, w, Vec3::ZERO), None), frame);
        }
        h.render_at_frame(&scene(draws(f, w, Vec3::ZERO), on()), 21)
    };
    let fresh = {
        let mut h = Harness::new();
        let (f, w) = (floor(&mut h), wall(&mut h));
        h.render_at_frame(&scene(draws(f, w, Vec3::ZERO), on()), 21)
    };
    let worst = |p: &Pixels| {
        let mut worst = 0i32;
        for y in HEIGHT / 2..HEIGHT * 3 / 4 {
            for x in NEAR.0..NEAR.1 {
                for c in 0..3 {
                    worst =
                        worst.max((i32::from(p.at(x, y)[c]) - i32::from(fresh.at(x, y)[c])).abs());
                }
            }
        }
        worst
    };
    // Premise: kept on, the history does carry the red over -- it is what
    // switching off has to throw away.
    let kept = worst(&after(0));
    assert!(kept > 10, "premise: kept on, the red history shows: {kept}");
    let back = worst(&after(1));
    assert!(back <= 3, "switched back on, it starts over: off by {back}");
}

/// What screen-space GI costs at 1920 x 1080, at its default settings: the
/// floor-and-wall frame with and without it, a minute's worth of frames
/// after a warm-up. `cargo test --release -p bsengine-rhi-wgpu --test
/// pixels_ssgi -- --ignored --nocapture bench_ssgi` -- release, because a
/// debug build's validation layers dominate any GPU timing (CLAUDE.md).
/// Each frame is read back, which costs the same with and without; the
/// difference is the trace, the two filters and the composite. On an RTX
/// 3060: about 1.4-1.7 ms at half resolution, 4.9 at full.
#[test]
#[ignore]
fn bench_ssgi() {
    let frames = 60u32;
    let mut times = Vec::new();
    for (label, ssgi, spatial) in [
        ("off", None, true),
        ("on", Some(ScreenSpaceGlobalIllumination::default()), true),
        (
            "on, no spatial filter",
            Some(ScreenSpaceGlobalIllumination::default()),
            false,
        ),
        (
            "on, full resolution",
            full_res(Some(ScreenSpaceGlobalIllumination::default())),
            true,
        ),
    ] {
        let mut h = Harness::new();
        h.resize(1920, 1080);
        h.set_ssgi_spatial(spatial);
        let d = floor_and_wall(RED)(&mut h);
        let s = scene(d, ssgi);
        for frame in 0..10 {
            h.render_at_frame(&s, frame);
        }
        let t = std::time::Instant::now();
        for frame in 10..10 + frames {
            h.render_at_frame(&s, frame);
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);
        eprintln!("ssgi {label}: {ms:.2} ms/frame");
        times.push(ms);
    }
    eprintln!(
        "ssgi costs {:.2} ms/frame at 1080p ({:.2} without the spatial filter, {:.2} at full resolution)",
        times[1] - times[0],
        times[2] - times[0],
        times[3] - times[0]
    );
}

fn full_res(s: Option<ScreenSpaceGlobalIllumination>) -> Option<ScreenSpaceGlobalIllumination> {
    s.map(|s| ScreenSpaceGlobalIllumination {
        full_resolution: true,
        ..s
    })
}

/// Eight frames of the scene `draws` builds, sixteen rays a pixel: the
/// light settled, its noise mostly gone, so what is left of a difference
/// between two ways of tracing it is the tracing's.
fn settled(
    draws: impl FnOnce(&mut Harness) -> Vec<Draw>,
    ssgi: Option<ScreenSpaceGlobalIllumination>,
) -> Pixels {
    let mut h = Harness::new();
    let d = draws(&mut h);
    let s = scene(d, ssgi);
    for frame in 0..8 {
        h.render_at_frame(&s, frame);
    }
    h.render_at_frame(&s, 8)
}

/// The trace runs at half the width and height unless asked for full, and
/// finds the same light: the red the wall bleeds onto the floor beside it,
/// settled, within 1.5% of what tracing every pixel finds (0.5% here; with
/// the history read a quarter of a pixel off its samples, 3%).
#[test]
fn traces_at_half_resolution_and_finds_the_same_light() {
    let mut h = Harness::new();
    let d = floor_and_wall(RED)(&mut h);
    h.render(&scene(d, on()));
    assert_eq!(h.ssgi_trace_size(), (WIDTH / 2, HEIGHT / 2));
    let d = floor_and_wall(RED)(&mut h);
    h.render(&scene(d, full_res(on())));
    assert_eq!(h.ssgi_trace_size(), (WIDTH, HEIGHT));

    let half = floor_band(&settled(floor_and_wall(RED), on()), NEAR.0, NEAR.1);
    let full = floor_band(
        &settled(floor_and_wall(RED), full_res(on())),
        NEAR.0,
        NEAR.1,
    );
    let off = floor_band(&first_frame(None, floor_and_wall(RED)), NEAR.0, NEAR.1);
    assert!(
        full.x > 2.0 * off.x,
        "premise: the wall bleeds red: {off} -> {full}"
    );
    assert!(
        ((half - full) / full).abs().max_element() < 0.015,
        "half resolution, the same light: {half} against {full}"
    );
}

/// At half resolution the light keeps to its own surface where one stands
/// in front of another: the glowing plate over the floor (same normal,
/// another depth), and along its rim -- the plate's pixels within two of the
/// floor's, the floor's within two of the plate's -- the light is what
/// tracing every pixel gives, to 0.015 in each channel's mean (0.007 here).
/// Brought up without the depth weight, the floor's light spreads onto the
/// plate's rim (+0.037 green); read from the history a quarter of a pixel
/// off, the plate's into the floor's history (-0.047).
#[test]
fn half_resolution_light_keeps_to_its_surface_at_the_rim() {
    let draws = |h: &mut Harness| {
        let (f, w, pl) = (floor(h), wall(h), plate(h));
        vec![
            Draw::new(f, Vec3::ZERO).colour(FLOOR).roughness(1.0),
            Draw::new(w, Vec3::ZERO)
                .colour(Vec3::ZERO)
                .emissive(Vec3::new(0.8, 0.0, 0.0)),
            Draw::new(pl, Vec3::ZERO)
                .colour(Vec3::ONE)
                .roughness(1.0)
                .emissive(Vec3::new(0.0, 0.5, 0.0)),
        ]
    };
    let (half, full) = (settled(draws, on()), settled(draws, full_res(on())));
    // The floor marked red, the plate green, the wall blue.
    let mask = first_frame(None, |h: &mut Harness| {
        let (f, w, pl) = (floor(h), wall(h), plate(h));
        let glow = |c: Vec3| Draw::new(0, Vec3::ZERO).colour(Vec3::ZERO).emissive(c);
        vec![
            Draw {
                mesh: f,
                ..glow(Vec3::X)
            },
            Draw {
                mesh: w,
                ..glow(Vec3::Z)
            },
            Draw {
                mesh: pl,
                ..glow(Vec3::Y)
            },
        ]
    });
    let is = |x: i32, y: i32, c: usize| {
        let (x, y) = (
            x.clamp(0, WIDTH as i32 - 1) as u32,
            y.clamp(0, HEIGHT as i32 - 1) as u32,
        );
        mask.at(x, y)[c] > 200
    };
    for (name, me, other) in [("floor", 0usize, 1usize), ("plate", 1, 0)] {
        let mut sum = Vec3::ZERO;
        let mut n = 0;
        for y in 2..HEIGHT as i32 - 2 {
            for x in 2..WIDTH as i32 - 2 {
                let rim = (-2..=2).any(|dy| (-2..=2).any(|dx| is(x + dx, y + dy, other)));
                if !is(x, y, me) || !rim {
                    continue;
                }
                let (a, b) = (half.at(x as u32, y as u32), full.at(x as u32, y as u32));
                sum += Vec3::new(
                    linear(a[0]) - linear(b[0]),
                    linear(a[1]) - linear(b[1]),
                    linear(a[2]) - linear(b[2]),
                );
                n += 1;
            }
        }
        assert!(n > 50, "premise: the {name}'s rim is in view: {n}");
        let moved = sum / n as f32;
        assert!(
            moved.abs().max_element() < 0.015,
            "the {name}'s rim keeps its own light at half resolution: {moved}"
        );
    }
}

/// A thin pole standing on the floor in front of the red wall, facing the
/// camera: about a pixel wide.
fn pole(h: &mut Harness, width: f32) -> u64 {
    let n = [0.0, 0.0, 1.0];
    h.mesh(
        &[
            vertex([0.3, 0.0, -6.0], n),
            vertex([0.3 + width, 0.0, -6.0], n),
            vertex([0.3 + width, 2.0, -6.0], n),
            vertex([0.3, 2.0, -6.0], n),
        ],
        &[0, 1, 2, 0, 2, 3],
    )
}

/// A thing thinner than a trace pixel keeps the light it had: the pole in
/// front of the red wall is a pixel or so wide, and where all four trace
/// samples around one of its pixels went past it -- to the wall far behind
/// -- none of them is its light, and that pixel is what it is with no
/// screen-space GI at all. Borrowing the nearest of them instead painted
/// the wall's light on it (39 levels).
#[test]
fn a_surface_every_trace_sample_went_past_keeps_its_own_light() {
    let draws = |h: &mut Harness| {
        let (f, w, p) = (floor(h), wall(h), pole(h, 0.03));
        vec![
            Draw::new(f, Vec3::ZERO).colour(FLOOR).roughness(1.0),
            Draw::new(w, Vec3::ZERO)
                .colour(Vec3::ZERO)
                .emissive(Vec3::new(0.8, 0.0, 0.0)),
            Draw::new(p, Vec3::ZERO).colour(Vec3::ONE).roughness(1.0),
        ]
    };
    let half = settled(draws, on());
    let off = first_frame(None, draws);
    // Premise: the trace is at work in this frame -- the floor beside the
    // wall takes its red.
    let (lit, unlit) = (
        floor_band(&half, NEAR.0, NEAR.1),
        floor_band(&off, NEAR.0, NEAR.1),
    );
    assert!(
        lit.x > 1.15 * unlit.x,
        "premise: the wall bleeds: {unlit} -> {lit}"
    );
    // The pole marked green.
    let mask = first_frame(None, |h: &mut Harness| {
        let (f, w, p) = (floor(h), wall(h), pole(h, 0.03));
        let glow = |c: Vec3| Draw::new(0, Vec3::ZERO).colour(Vec3::ZERO).emissive(c);
        vec![
            Draw {
                mesh: f,
                ..glow(Vec3::Z)
            },
            Draw {
                mesh: w,
                ..glow(Vec3::Z)
            },
            Draw {
                mesh: p,
                ..glow(Vec3::Y)
            },
        ]
    });
    let is_pole = |x: u32, y: u32| mask.at(x.min(WIDTH - 1), y.min(HEIGHT - 1))[1] > 200;
    let mut n = 0;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            // The trace samples around (x, y): the first pixel of each of
            // the four blocks the upsample reads.
            let (bx, by) = (x / 2, y / 2);
            let sampled = (0..2).any(|dy| (0..2).any(|dx| is_pole(2 * (bx + dx), 2 * (by + dy))));
            if !is_pole(x, y) || sampled {
                continue;
            }
            n += 1;
            for c in 0..3 {
                let d = (i32::from(half.at(x, y)[c]) - i32::from(off.at(x, y)[c])).abs();
                assert!(d <= 2, "the pole at ({x}, {y}) keeps its light: off by {d}");
            }
        }
    }
    assert!(
        n >= 10,
        "premise: pole pixels no trace sample landed on: {n}"
    );
}

/// Under MSAA the trace reads the resolved normals: the opaque pass writes
/// them into a multisampled target first, and screen-space GI traces from
/// the single-sample one the resolve fills. The red wall bleeds onto the
/// floor beside it under 4x MSAA as it does without, each on a renderer of
/// its own (so the single-sample normals hold the clear until a resolve
/// fills them): a resolve gone missing would leave the trace no surface to
/// light, and the bleed gone.
#[test]
fn under_msaa_the_trace_reads_the_resolved_normals() {
    let one = |msaa: u32| {
        let mut h = Harness::new();
        let d = floor_and_wall(RED)(&mut h);
        let p = h.render(&Scene {
            msaa,
            ..scene(d, on())
        });
        if msaa > 1 {
            assert_eq!(h.msaa_samples(), msaa, "premise: drawn multisampled");
        }
        floor_band(&p, NEAR.0, NEAR.1)
    };
    let (single, multi) = (one(1), one(4));
    let off = floor_band(&first_frame(None, floor_and_wall(RED)), NEAR.0, NEAR.1);
    assert!(
        single.x > 1.5 * off.x,
        "premise: the wall bleeds red: {off} -> {single}"
    );
    assert!(
        ((multi - single) / single).abs().max_element() < 0.05,
        "under MSAA the same bleed: {single} against {multi}"
    );
}

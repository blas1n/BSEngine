//! Spot light cookies, measured at the pixel.
//!
//! The fixture: a white wall 5 units in front of the camera, lit by nothing
//! but one spot light standing between the two -- no sun, no ambient, no
//! bloom, no SSAO, tonemap `None` -- so a dark pixel inside the cone can only
//! be the cookie's doing. The light points the way the camera looks, so the
//! picture lands on the wall as the camera sees a slide projector's: the
//! texture's left on the screen's left, its top at the top.
//!
//! The cookies are 16x16 two-tone images rather than 2x1 ones: bilinear
//! filtering across a two-texel image is a gradient over the whole middle
//! half, and the samples below, a third of the way out from the centre,
//! would land inside it.

mod common;

use bsengine_core::{AmbientOcclusion, Bloom, ToneMap, ToneMappingMode};
use bsengine_rhi_wgpu::SpotLightEntry;
use common::{Draw, Harness, Light, Pixels, Scene, HEIGHT, WIDTH};
use glam::{Quat, Vec3};

const CX: u32 = WIDTH / 2;
const CY: u32 = HEIGHT / 2;
/// Pixels from the centre to sample at: inside the 30-degree cone (about 37
/// pixels in radius on the wall), clear of the cookie's middle seam.
const OFF: u32 = 15;
/// Well outside the cone.
const OUTSIDE: u32 = 60;

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

fn wall(cube: u64) -> Draw {
    Draw::new(cube, Vec3::ZERO)
        .scaled(Vec3::new(40.0, 40.0, 0.2), Vec3::new(0.0, 0.0, -5.0))
        .colour(Vec3::ONE)
        .roughness(1.0)
}

/// A spot light at `position` pointing along `rotation`'s -Z, with
/// `rotation`'s +X as its cookie's right, as `bsengine-render` builds the
/// entry from a transform.
fn spot(position: Vec3, rotation: Quat, outer_degrees: f32, cookie: Option<u64>) -> SpotLightEntry {
    SpotLightEntry {
        position,
        direction: rotation * Vec3::NEG_Z,
        right: rotation * Vec3::X,
        color: Vec3::ONE,
        intensity: 3.0,
        range: 20.0,
        inner_angle: (outer_degrees - 1.0).to_radians(),
        outer_angle: outer_degrees.to_radians(),
        cookie,
    }
}

fn scene(draws: Vec<Draw>, spots: Vec<SpotLightEntry>) -> Scene {
    Scene {
        draws,
        light: Light {
            color: Vec3::ZERO,
            ambient: Vec3::ZERO,
            spots,
            ..Light::default()
        },
        bloom: Some(Bloom::default().disabled()),
        ssao: Some(AmbientOcclusion::default().disabled()),
        tone_map: Some(ToneMap::new(ToneMappingMode::None)),
        ..Scene::default()
    }
}

/// A 16x16 cookie, `lit` where `bright(x, y)` says (x right, y down, both
/// 0..16), black elsewhere.
fn cookie(h: &mut Harness, lit: [u8; 4], bright: impl Fn(u32, u32) -> bool) -> u64 {
    let mut rgba = Vec::with_capacity(16 * 16 * 4);
    for y in 0..16 {
        for x in 0..16 {
            rgba.extend_from_slice(if bright(x, y) { &lit } else { &BLACK });
        }
    }
    h.texture_with(
        bsengine_core::TextureImportSettings::default(),
        16,
        16,
        &rgba,
    )
}

fn red(p: &Pixels, x: u32, y: u32) -> u8 {
    p.at(x, y)[0]
}

/// The wall facing the light, with the cone's centre at the frame's centre.
fn facing_wall(h: &mut Harness, rotation: Quat, cookie: Option<u64>) -> Pixels {
    let cube = h.cube();
    h.render(&scene(
        vec![wall(cube)],
        vec![spot(Vec3::ZERO, rotation, 30.0, cookie)],
    ))
}

/// The plain cone lights both sides of its centre; with a cookie whose right
/// half is black, the right side goes dark and the left stays exactly as lit
/// as it was. Outside the cone is dark either way.
#[test]
fn a_cookie_darkens_the_cone_where_its_texture_is_dark() {
    let mut h = Harness::new();
    let left_white = cookie(&mut h, WHITE, |x, _| x < 8);
    let plain = facing_wall(&mut h, Quat::IDENTITY, None);
    let cookied = facing_wall(&mut h, Quat::IDENTITY, Some(left_white));

    let (left, right) = (CX - OFF, CX + OFF);
    assert!(
        red(&plain, left, CY) > 100 && red(&plain, right, CY) > 100,
        "premise: the plain cone lights both samples ({} and {})",
        red(&plain, left, CY),
        red(&plain, right, CY)
    );
    assert_eq!(
        red(&plain, CX - OUTSIDE, CY),
        0,
        "premise: outside the cone is dark"
    );

    assert!(
        red(&cookied, right, CY) < 5,
        "the cookie's black half shades the cone: {}",
        red(&cookied, right, CY)
    );
    assert!(
        red(&cookied, left, CY).abs_diff(red(&plain, left, CY)) <= 3,
        "its white half leaves the light as it was: {} against {}",
        red(&cookied, left, CY),
        red(&plain, left, CY)
    );
}

/// The picture is the right way up, not only the right way round: a cookie
/// white on top lights the top of the cone. A shader that projected with the
/// image's v running upward would pass the left/right test above.
#[test]
fn a_cookie_is_projected_the_right_way_up() {
    let mut h = Harness::new();
    let top_white = cookie(&mut h, WHITE, |_, y| y < 8);
    let p = facing_wall(&mut h, Quat::IDENTITY, Some(top_white));
    assert!(
        red(&p, CX, CY - OFF) > 100,
        "top: {}",
        red(&p, CX, CY - OFF)
    );
    assert!(
        red(&p, CX, CY + OFF) < 5,
        "bottom: {}",
        red(&p, CX, CY + OFF)
    );
}

/// Turning the light a quarter about its own axis turns the picture with it:
/// the light's right is now the world's up, so the cookie's dark right half
/// lands on top and its left on the bottom, while left and right of the
/// centre -- now the picture's vertical middle -- are lit. The case the
/// roll in the light's spare floats exists for.
#[test]
fn the_cookie_turns_with_the_light() {
    let mut h = Harness::new();
    let left_white = cookie(&mut h, WHITE, |x, _| x < 8);
    let turned = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    let p = facing_wall(&mut h, turned, Some(left_white));
    assert!(red(&p, CX, CY - OFF) < 5, "top: {}", red(&p, CX, CY - OFF));
    assert!(
        red(&p, CX, CY + OFF) > 100,
        "bottom: {}",
        red(&p, CX, CY + OFF)
    );
    // The seam runs left to right through the centre now; a quarter of the
    // way down the frame from it, either side, the picture is white.
    assert!(
        red(&p, CX - OFF, CY + OFF) > 100,
        "lower left: {}",
        red(&p, CX - OFF, CY + OFF)
    );
    assert!(
        red(&p, CX + OFF, CY + OFF) > 100,
        "lower right: {}",
        red(&p, CX + OFF, CY + OFF)
    );
}

/// A light pointing straight down takes the basis's other reference axis
/// (`cookie::cookie_basis`), where the CPU's roll and the shader's basis
/// could disagree without any sideways light noticing. Seen from overhead,
/// the light's frame is the screen's: its right is +X, its up is -Z, which is
/// the top of the frame.
#[test]
fn a_cookie_under_a_light_pointing_straight_down() {
    let mut h = Harness::new();
    let left_white = cookie(&mut h, WHITE, |x, _| x < 8);
    let top_white = cookie(&mut h, WHITE, |_, y| y < 8);
    let cube = h.cube();
    let down = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    assert!(
        (down * Vec3::NEG_Z).distance(Vec3::NEG_Y) < 1e-6,
        "premise: pointing down"
    );
    let mut render = |id| {
        let floor = Draw::new(cube, Vec3::ZERO)
            .scaled(Vec3::new(40.0, 0.2, 40.0), Vec3::new(0.0, -0.1, 0.0))
            .roughness(1.0);
        h.render(&Scene {
            camera_pos: Vec3::new(0.0, 6.0, 0.01),
            look_at: Vec3::ZERO,
            ..scene(
                vec![floor],
                vec![spot(Vec3::new(0.0, 3.0, 0.0), down, 30.0, Some(id))],
            )
        })
    };
    let lr = render(left_white);
    assert!(
        red(&lr, CX - OFF, CY) > 100,
        "left: {}",
        red(&lr, CX - OFF, CY)
    );
    assert!(
        red(&lr, CX + OFF, CY) < 5,
        "right: {}",
        red(&lr, CX + OFF, CY)
    );
    let tb = render(top_white);
    assert!(
        red(&tb, CX, CY - OFF) > 100,
        "top: {}",
        red(&tb, CX, CY - OFF)
    );
    assert!(
        red(&tb, CX, CY + OFF) < 5,
        "bottom: {}",
        red(&tb, CX, CY + OFF)
    );
}

/// The picture spans the cone, edge to edge: a cookie white only in its
/// middle half lights the middle of the cone and leaves its rim dark, where
/// the plain cone is fully lit. Projected too small (say, through the inner
/// angle, or a tangent too large), the middle half would cover the rim.
#[test]
fn the_picture_spans_the_cone() {
    let mut h = Harness::new();
    let middle = cookie(&mut h, WHITE, |x, y| {
        (4..12).contains(&x) && (4..12).contains(&y)
    });
    let plain = facing_wall(&mut h, Quat::IDENTITY, None);
    let cookied = facing_wall(&mut h, Quat::IDENTITY, Some(middle));
    // 30 pixels out: about 0.8 of the cone's radius, inside its fully lit
    // inner angle, and outside the middle half of the picture.
    let rim = CX + 30;
    assert!(
        red(&plain, rim, CY) > 100,
        "premise: the rim is lit: {}",
        red(&plain, rim, CY)
    );
    assert!(
        red(&cookied, rim, CY) < 5,
        "rim: {}",
        red(&cookied, rim, CY)
    );
    assert!(
        red(&cookied, CX + OFF, CY) > 100,
        "middle: {}",
        red(&cookied, CX + OFF, CY)
    );
}

/// The cookie's colour, not just its brightness, multiplies the light: a red
/// gel leaves red and takes green and blue.
#[test]
fn a_coloured_cookie_tints_the_light() {
    let mut h = Harness::new();
    let red_gel = cookie(&mut h, [255, 0, 0, 255], |_, _| true);
    let p = facing_wall(&mut h, Quat::IDENTITY, Some(red_gel));
    let [r, g, b, _] = p.at(CX, CY);
    assert!(r > 100 && g < 5 && b < 5, "centre: {:?}", [r, g, b]);
}

/// Two lights with two cookies each get their own: the left light's picture
/// is white on its left, the right light's on its right. Lights sharing one
/// atlas layer -- every cookie drawn into layer 0, or every light reading it
/// -- would show the same picture twice.
#[test]
fn each_light_projects_its_own_cookie() {
    let mut h = Harness::new();
    let left_white = cookie(&mut h, WHITE, |x, _| x < 8);
    let right_white = cookie(&mut h, WHITE, |x, _| x >= 8);
    let cube = h.cube();
    // 20 degrees: about 23 pixels in radius, so the two cones, 3 units
    // (about 39 pixels) either side of the centre, do not overlap.
    let p = h.render(&scene(
        vec![wall(cube)],
        vec![
            spot(
                Vec3::new(-3.0, 0.0, 0.0),
                Quat::IDENTITY,
                20.0,
                Some(left_white),
            ),
            spot(
                Vec3::new(3.0, 0.0, 0.0),
                Quat::IDENTITY,
                20.0,
                Some(right_white),
            ),
        ],
    ));
    // 3 units on the wall is 3 * 150 / (2 * 9.9 * tan 30deg) pixels.
    let at = (3.0 * HEIGHT as f32 / (2.0 * 9.9 * 30f32.to_radians().tan())).round() as u32;
    let d = 10;
    let (a, b) = (CX - at, CX + at);
    assert!(
        red(&p, a - d, CY) > 100,
        "left light, its left: {}",
        red(&p, a - d, CY)
    );
    assert!(
        red(&p, a + d, CY) < 5,
        "left light, its right: {}",
        red(&p, a + d, CY)
    );
    assert!(
        red(&p, b - d, CY) < 5,
        "right light, its left: {}",
        red(&p, b - d, CY)
    );
    assert!(
        red(&p, b + d, CY) > 100,
        "right light, its right: {}",
        red(&p, b + d, CY)
    );
}

/// A cookie whose texture is not on the GPU -- still loading, or a path that
/// never resolves -- leaves the light a plain cone rather than a dark one,
/// as Unity's does while a cookie streams in.
#[test]
fn a_missing_cookie_texture_leaves_a_plain_cone() {
    let mut h = Harness::new();
    let plain = facing_wall(&mut h, Quat::IDENTITY, None);
    let missing = facing_wall(&mut h, Quat::IDENTITY, Some(u64::MAX));
    assert!(red(&plain, CX + OFF, CY) > 100, "premise: the cone is lit");
    assert!(
        !missing.differs_from(&plain),
        "an unknown id changed the frame: {}",
        missing.describe()
    );
}

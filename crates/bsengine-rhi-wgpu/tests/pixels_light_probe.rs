//! Baked light-probe GI, measured at the pixel.
//!
//! The trap this file is written around: **a broken probe implementation also
//! makes things brighter.** Position-varying ambient light, a mis-scaled
//! constant, a stuck coefficient -- every one of them raises the floor's luma,
//! and a test that asserted "brighter with probes on" would pass for all of
//! them. What only working bounced GI does is make a *white* floor beside a
//! *red* wall go red, and follow the wall's colour when that colour changes.
//!
//! So every measurement here is a per-channel *bias* -- red against the mean of
//! green and blue -- never a brightness. A uniform brightening leaves the bias
//! exactly where it was.
//!
//! What the floor's centre pixel actually reads, for the record:
//!
//! | wall  | probes off        | probes on         |
//! |-------|-------------------|-------------------|
//! | red   | `[159, 159, 159]` | `[220, 142, 142]` |
//! | green | `[159, 159, 159]` | `[157, 228, 157]` |
//!
//! The two off-states are the same pixel because with probes off an emissive
//! surface lights nothing but itself -- the entire bleed is the bake's doing,
//! and there is no direct-light path for it to be confused with.
//!
//! These numbers are post-tonemap: `tone_map: None` in the harness means
//! `ToneMap::default()`, which is ACES and enabled, so the table is what an eye
//! would see rather than raw radiance. The thresholds below sit well under the
//! margins it shows.
//!
//! Fixture notes:
//!
//! - The wall is **emissive**, not merely brightly lit. Probe capture shades
//!   direct light only, so a purely diffuse wall reflects at most
//!   `sun * albedo / PI` -- under a third of the sun's own radiance -- and what
//!   survives the cosine convolution and the falloff to the floor is a
//!   correspondingly small fraction of that. `Draw::emissive` at
//!   [`WALL_RADIANCE`] makes the wall a genuine radiator instead, which is what
//!   a one-bounce probe bake is built to pick up.
//! - The wall is a **cube**, the floor a **plane**: the plane is the only mesh
//!   with white vertex colours (`cube_vertices` paints its faces), so it is the
//!   one that can carry a truly white albedo for the bleed to land on.
//! - The wall sits **outside** the volume. Nothing about the effect requires
//!   that, but it keeps the emitter's own pixels out of the probe branch, so a
//!   frame difference can only come from surfaces that were lit by the bake.
//! - No skybox. With none loaded the no-probe ambient is exactly
//!   `light.ambient * albedo`, a flat grey, which makes the off-state baseline
//!   achromatic and every asserted colour shift unambiguously the probes'.

mod common;

use bsengine_core::LightProbeVolume;
use common::{Draw, Harness, Pixels, Scene};
use glam::Vec3;

/// Height of the floor plane. Comfortably inside [`volume`]'s vertical span,
/// so the trilinear lookup at the measured pixel blends real probes rather
/// than clamping at a face.
const FLOOR_Y: f32 = -0.5;

/// Where the wall stands, just beyond the volume's `x` face.
const WALL_X: f32 = 3.0;

/// Radiance of the wall, in the same pre-tonemap units the scene shader works
/// in. High enough that one bounce is worth tens of 8-bit levels on the floor,
/// low enough that the floor never clips -- a clipped channel cannot show a
/// rise.
const WALL_RADIANCE: f32 = 6.0;

/// A volume around the origin covering the middle of the floor.
///
/// `[4, 2, 4]` is `LightProbeVolume`'s own default and exactly `MAX_PROBES`.
/// Two probes on `y` put one just under the floor and one above it, which is
/// the pair the floor's lookup interpolates between.
fn volume() -> LightProbeVolume {
    LightProbeVolume {
        half_extents: Vec3::new(2.5, 1.5, 2.5).into(),
        resolution: [4, 2, 4],
    }
}

/// A white floor with a coloured emissive wall beside it, and the camera
/// looking down at the floor so the centre pixel is floor and nothing else.
///
/// `shift` displaces the floor, the wall and the camera together. The volume is
/// pinned to the world origin (see `Scene::light_probes`), so shifting the
/// scene is how a test puts a surface *outside* the volume while keeping the
/// framing -- and therefore every other term in the frame -- identical.
fn wall_and_floor(
    plane: u64,
    cube: u64,
    wall_colour: Vec3,
    shift: Vec3,
    probes: Option<LightProbeVolume>,
) -> Scene {
    Scene {
        draws: vec![
            Draw::new(plane, Vec3::ZERO)
                .scaled(
                    Vec3::new(16.0, 1.0, 16.0),
                    Vec3::new(0.0, FLOOR_Y, 0.0) + shift,
                )
                .colour(Vec3::ONE)
                .roughness(0.9),
            Draw::new(cube, Vec3::ZERO)
                .scaled(
                    Vec3::new(0.3, 4.0, 8.0),
                    Vec3::new(WALL_X, FLOOR_Y + 2.0, 0.0) + shift,
                )
                .colour(wall_colour)
                .emissive(wall_colour * WALL_RADIANCE),
        ],
        camera_pos: Vec3::new(0.0, 2.0, 6.0) + shift,
        look_at: Vec3::new(0.0, FLOOR_Y, 0.0) + shift,
        light_probes: probes,
        ..Scene::default()
    }
}

const RED: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const GREEN: Vec3 = Vec3::new(0.0, 1.0, 0.0);

/// The same scene rendered twice, probes off and then on, with the bake
/// guaranteed to be this scene's.
///
/// `render_frame` re-bakes only when the volume differs from the last one it
/// baked -- deliberately, so a bake costs one frame on load rather than every
/// frame. Every test here uses the same box, so two probe scenes rendered back
/// to back would leave the second lit by the *first* scene's bake, and a
/// green-wall frame would come out red. Rendering with no volume in between
/// resets that cache, and an off/on pair is what every measurement below needs
/// anyway.
fn probes_off_then_on(
    h: &mut Harness,
    scene: impl Fn(Option<LightProbeVolume>) -> Scene,
) -> (Pixels, Pixels) {
    let off = h.render(&scene(None));
    let on = h.render(&scene(Some(volume())));
    (off, on)
}

/// The floor pixel every measurement below is taken at: the centre of the
/// frame, which the camera is aimed at the floor for.
fn floor_pixel(p: &Pixels) -> [u8; 4] {
    p.centre()
}

/// How far channel `c` stands above the mean of the other two, in 8-bit levels.
///
/// A *bias*, not a brightness, and that is the whole point: an implementation
/// that merely made the frame brighter raises all three channels together and
/// leaves this number where it was. Only light that arrived carrying a colour
/// moves it.
fn bias(p: [u8; 4], c: usize) -> f32 {
    let v = [p[0] as f32, p[1] as f32, p[2] as f32];
    v[c] - 0.5 * (v[(c + 1) % 3] + v[(c + 2) % 3])
}

const R: usize = 0;
const G: usize = 1;

#[test]
fn a_white_floor_beside_a_red_wall_picks_up_red() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());

    let (off, on) = probes_off_then_on(&mut h, |p| wall_and_floor(plane, cube, RED, Vec3::ZERO, p));

    let before = floor_pixel(&off);
    let after = floor_pixel(&on);

    // The baseline has to be neutral for a colour shift to mean anything. With
    // no skybox and no probes the floor's ambient is `light.ambient * albedo`
    // on a white albedo, and its direct light is a white sun.
    assert!(
        bias(before, R).abs() < 4.0,
        "the probes-off floor should be achromatic before any bleed, saw {before:?}"
    );

    // The assertion that matters. Not `after` brighter than `before` -- a
    // stuck coefficient or a position-varying ambient would pass that. The red
    // channel has to pull *away from its own green and blue*.
    assert!(
        bias(after, R) > bias(before, R) + 12.0,
        "the floor beside a red wall should go red once probes are baked: \
         red bias {:.1} -> {:.1} (floor {before:?} -> {after:?}). No rise in \
         bias means whatever the probes did to this frame was colourless, \
         which is not bounced light.",
        bias(before, R),
        bias(after, R)
    );

    // And spelled out channel by channel, because a bias can also rise by the
    // green and blue *falling*. Red must actually gain, and gain more than the
    // channels the wall emits none of.
    let d = |i: usize| after[i] as f32 - before[i] as f32;
    assert!(
        d(0) > d(1) + 12.0 && d(0) > d(2) + 12.0,
        "red must gain more than green and blue: dR {:.1}, dG {:.1}, dB {:.1} \
         ({before:?} -> {after:?})",
        d(0),
        d(1),
        d(2)
    );
}

#[test]
fn a_surface_outside_the_volume_is_unchanged() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());

    // The same scene as the test above, lowered until floor, wall and camera
    // are all clear of the volume. The picture is geometrically identical --
    // only its position relative to the probe box has changed.
    const BELOW: Vec3 = Vec3::new(0.0, -6.0, 0.0);

    let (off, on) = probes_off_then_on(&mut h, |p| wall_and_floor(plane, cube, RED, BELOW, p));

    // First prove this scene is one probes would visibly change if it were
    // inside. Without that, "unchanged" could just mean "insensitive", and the
    // equality below would hold for a probe path that never ran at all.
    let (inside_off, inside_on) =
        probes_off_then_on(&mut h, |p| wall_and_floor(plane, cube, RED, Vec3::ZERO, p));
    assert!(
        bias(floor_pixel(&inside_on), R) > bias(floor_pixel(&inside_off), R) + 12.0,
        "the control scene must actually pick up the wall, else this test \
         proves nothing: {:?} against {:?}",
        floor_pixel(&inside_off),
        floor_pixel(&inside_on)
    );

    assert_eq!(
        on.data,
        off.data,
        "a volume that contains none of the scene must leave every pixel \
         exactly as it was. It did not -- so `inside_probe_volume` is letting \
         probe irradiance out of its box, and probe GI is really just a global \
         ambient with extra steps. probes-off {}, probes-on {}",
        off.describe(),
        on.describe()
    );
}

#[test]
fn a_scene_with_no_probe_volume_is_pixel_identical() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let scene = |probes| wall_and_floor(plane, cube, RED, Vec3::ZERO, probes);

    let before = h.render(&scene(None));

    // Bake a volume in between, so the probe uniform is provably non-zero and
    // provably reaching the shader by the time the last frame is drawn. A
    // "nothing changed" result from a renderer that never baked anything would
    // be worthless.
    let baked = h.render(&scene(Some(volume())));
    assert!(
        baked.differs_from(&before),
        "the volume should have changed the frame, saw {}",
        baked.describe()
    );

    let after = h.render(&scene(None));
    assert_eq!(
        after.data, before.data,
        "with no volume the frame must be byte-for-byte what it was before \
         probes existed. It is not -- so the zeroed `enabled: 0` upload is \
         leaking probe irradiance into every scene that never asked for it, \
         which is exactly the 'probes just brightened everything' failure this \
         test exists for."
    );
}

#[test]
fn a_green_wall_bleeds_green_not_red() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());

    let (off, on) = probes_off_then_on(&mut h, |p| {
        wall_and_floor(plane, cube, GREEN, Vec3::ZERO, p)
    });
    let before = floor_pixel(&off);
    let after = floor_pixel(&on);

    // The bleed follows the wall. An implementation that read the wrong SH
    // coefficient, or folded the three channels together, would tint the floor
    // the same way whatever colour the wall is -- so this is the assertion the
    // red test cannot make on its own.
    assert!(
        bias(after, G) > bias(before, G) + 12.0,
        "the floor beside a green wall should go green: green bias {:.1} -> \
         {:.1} ({before:?} -> {after:?})",
        bias(before, G),
        bias(after, G)
    );
    assert!(
        bias(after, R) < bias(before, R) + 6.0,
        "and it must not also go red: red bias {:.1} -> {:.1} \
         ({before:?} -> {after:?}). Red rising under a green wall means the \
         bleed is not reading the scene's actual colour.",
        bias(before, R),
        bias(after, R)
    );

    // Cross-checked against the red wall in the same frame geometry: swapping
    // the emitter's colour must swap which channel gains. This is what
    // separates "the probes carry colour" from "the probes carry *this*
    // scene's colour".
    let (_, red_on) =
        probes_off_then_on(&mut h, |p| wall_and_floor(plane, cube, RED, Vec3::ZERO, p));
    let red = floor_pixel(&red_on);
    assert!(
        bias(red, R) > bias(after, R) + 12.0 && bias(after, G) > bias(red, G) + 12.0,
        "red wall and green wall must bleed different channels: red wall \
         {red:?}, green wall {after:?}"
    );
}

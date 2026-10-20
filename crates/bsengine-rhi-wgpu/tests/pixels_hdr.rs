//! HDR display output (`bsengine_core::HdrOutputSettings`): the frame in
//! linear scRGB -- 1.0 = 80 nits -- with mid-tones as SDR shows them and
//! highlights kept up to the display's peak.
//!
//! An HDR display is not something a test machine has, so this reads the
//! float target the swapchain would present, and compares it against the SDR
//! renderer drawing the same scene: what must match, matches; what must
//! differ -- only the highlights -- differs, by the amounts the settings say.

mod common;

use std::collections::HashMap;

use bsengine_core::{HdrOutputSettings, UiAnchor, UiState, UiWidget};
use common::{Draw, Harness, Light, Scene, HEIGHT, WIDTH};
use glam::Vec3;

/// Paper white 200 nits (SDR white = 2.5 in scRGB), peak 1000 (5x white).
const HDR: HdrOutputSettings = HdrOutputSettings {
    enabled: true,
    paper_white_nits: 200.0,
    max_nits: 1000.0,
};
const SCALE: f32 = 200.0 / 80.0;
const PEAK: f32 = 1000.0 / 200.0;

fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The red channel at the frame's centre, in scRGB.
fn hdr_centre(h: &Harness) -> f32 {
    let f = h.last_hdr();
    f[((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize]
}

fn glowing(h: &mut Harness, e: f32) -> Scene {
    let cube = h.cube();
    Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO).emissive(Vec3::splat(e))],
        tone_map: Some(bsengine_core::ToneMap::default()),
        ..Scene::default()
    }
}

/// Below scene-linear white the HDR frame is the SDR frame, only put at paper
/// white: a dim surface reads the same light in both. The hard side of HDR
/// output is that it changes nothing it should not -- an HDR curve that also
/// squeezed the mid-tones (scaling the SDR curve to the peak, `k f(x / k)`)
/// would show here, at half the brightness.
#[test]
fn hdr_output_keeps_the_mid_tones_of_sdr() {
    let mut sdr = Harness::new();
    let mut hdr = Harness::new_hdr(HDR);
    for e in [0.05f32, 0.2, 0.5] {
        let s = glowing(&mut sdr, e);
        let want = srgb_to_linear(sdr.render(&s).centre()[0]);
        let s = glowing(&mut hdr, e);
        hdr.render(&s);
        let got = hdr_centre(&hdr) / SCALE;
        assert!(
            (got - want).abs() <= 0.01 + want * 0.03,
            "emissive {e}: SDR shows {want:.4} of white, HDR {got:.4}"
        );
    }
}

/// Above white, SDR clips and HDR does not: a surface SDR shows as plain
/// white comes out well above paper white, and no surface, however bright,
/// beyond the display's peak.
#[test]
fn hdr_output_keeps_highlights_up_to_the_peak() {
    let mut sdr = Harness::new();
    let mut hdr = Harness::new_hdr(HDR);
    let s = glowing(&mut sdr, 8.0);
    assert_eq!(
        sdr.render(&s).centre()[0],
        255,
        "premise: SDR shows this surface as plain white"
    );
    let s = glowing(&mut hdr, 8.0);
    hdr.render(&s);
    let bright = hdr_centre(&hdr) / SCALE;
    assert!(
        bright > 1.5 && bright <= PEAK,
        "a surface SDR clips to white should come out between 1.5x white and \
         the peak ({PEAK}x): {bright:.3}"
    );
    let s = glowing(&mut hdr, 1000.0);
    hdr.render(&s);
    let blinding = hdr_centre(&hdr) / SCALE;
    assert!(
        blinding <= PEAK * 1.001 && blinding > bright,
        "brighter still, and never past the peak ({PEAK}x): {blinding:.3}"
    );
}

/// The UI under HDR output shows its white at paper white -- what SDR white
/// is everywhere else -- not at scRGB 1.0, 80 nits, a third of it. egui drawn
/// straight into the float target lands there, and too bright in its
/// mid-tones besides (it writes gamma-encoded values to a non-sRGB target).
#[test]
fn hdr_output_draws_the_ui_at_paper_white() {
    let mut h = Harness::new_hdr(HDR);
    let white = h.two_colour_texture([255, 255, 255, 255], [255, 255, 255, 255]);
    let mut ui = UiState::default();
    ui.set_widget(UiWidget::Image {
        id: "white".into(),
        texture_path: "ui/white.png".into(),
        x: 40.0,
        y: 30.0,
        width: 80.0,
        height: 60.0,
        anchor: UiAnchor::TOP_LEFT,
    });
    let scene = Scene {
        ui,
        ui_textures: HashMap::from([("ui/white.png".to_string(), white)]),
        ..Scene::default()
    };
    // egui draws nothing the first frame it sees a widget, and then fades
    // the area in by the clock: drawn until it stops changing.
    let at = ((60 * WIDTH + 80) * 4) as usize;
    let mut last = -1.0f32;
    let mut px = [0.0f32; 3];
    for _ in 0..120 {
        h.render(&scene);
        let f = h.last_hdr();
        px = [f[at], f[at + 1], f[at + 2]];
        if px[0] > 0.5 && (px[0] - last).abs() < 1e-3 {
            break;
        }
        last = px[0];
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        px.iter().all(|&v| (v - SCALE).abs() < 0.05),
        "the UI's white should be paper white ({SCALE} in scRGB): {px:?}"
    );
}

/// A rotated cube's edges against a black backdrop, glowing `e` and lit by nothing else
/// (the harness's sun would light its faces whatever `e` is), shown as SDR
/// would.
fn edges(h: &mut Harness, e: f32, fxaa: bool, smaa: bool) -> common::Pixels {
    let cube = h.cube();
    h.render(&Scene {
        draws: vec![
            Draw::new(cube, Vec3::ZERO)
                .rotated_z(0.4)
                .emissive(Vec3::splat(e)),
            // A black backdrop: against the clear colour's grey a dim edge
            // has contrast to spare in any space, and decides nothing.
            Draw::new(cube, Vec3::new(0.0, 0.0, -3.0))
                .scaled(Vec3::new(30.0, 30.0, 0.1), Vec3::new(0.0, 0.0, -3.0))
                .colour(Vec3::ZERO),
        ],
        tone_map: Some(bsengine_core::ToneMap::default()),
        light: Light {
            color: Vec3::ZERO,
            ambient: Vec3::ZERO,
            ..Light::default()
        },
        fxaa: fxaa.then(bsengine_core::Fxaa::default),
        smaa: smaa.then(|| bsengine_core::Smaa {
            enabled: true,
            ..Default::default()
        }),
        ..Scene::default()
    })
}

/// FXAA and SMAA find the same edges in an HDR frame as in the SDR one. Their
/// thresholds are for perceptual luma in 0..1; the HDR target holds linear
/// light with white at paper white, so luma is divided by it and encoded
/// first. Measured as SDR would show both frames, on a bright cube and on a
/// dim one: a bright edge has contrast to spare either way, and only the dim
/// one -- under the passes' absolute thresholds in linear light, over them in
/// perceptual luma -- shows whether the luma was put into the space the
/// thresholds are for.
#[test]
fn hdr_output_antialiases_as_sdr_does() {
    let mut sdr = Harness::new();
    let mut hdr = Harness::new_hdr(HDR);
    for (name, fxaa, smaa, e) in [
        ("FXAA, bright", true, false, 0.6),
        ("FXAA, dim", true, false, 0.03),
        ("SMAA, bright", false, true, 0.6),
        ("SMAA, dim", false, true, 0.03),
    ] {
        let plain = edges(&mut sdr, e, false, false);
        let want = edges(&mut sdr, e, fxaa, smaa);
        let off_by = |a: &common::Pixels, b: &common::Pixels| {
            (0..a.data.len())
                .filter(|&i| (a.data[i] as i32 - b.data[i] as i32).abs() > 3)
                .count()
        };
        // Premise: the pass did something to this frame.
        let changed = off_by(&want, &plain);
        assert!(
            changed > 100,
            "premise: {name} changes the edges in SDR ({changed})"
        );
        let got = edges(&mut hdr, e, fxaa, smaa);
        // Counted, not the worst pixel: SDR decides on 8-bit values and HDR on
        // half floats, so a pixel right at a threshold can go either way (6 of
        // FXAA's 510 here). With the luma left in linear light, every pixel
        // the dim edge's pass changed came out different.
        let different = off_by(&want, &got);
        assert!(
            different * 20 <= changed,
            "{name} should treat the HDR frame as it does the SDR one: {different} \
             of the {changed} pixels it changes come out otherwise"
        );
    }
}

/// A colour grade is authored for SDR's 0..1, and applies there under HDR
/// output too: below white, the graded HDR frame is the graded SDR one.
#[test]
fn hdr_output_grades_what_sdr_would() {
    let mut sdr = Harness::new();
    let mut hdr = Harness::new_hdr(HDR);
    let scene = |h: &mut Harness| {
        let cube = h.cube();
        Scene {
            draws: vec![Draw::new(cube, Vec3::ZERO).emissive(Vec3::new(0.5, 0.3, 0.1))],
            tone_map: Some(bsengine_core::ToneMap::default()),
            color_grading: Some(bsengine_core::ColorGrading {
                enabled: true,
                contrast: 1.6,
                saturation: 0.3,
                ..Default::default()
            }),
            ..Scene::default()
        }
    };
    let ungraded = {
        let cube = sdr.cube();
        sdr.render(&Scene {
            draws: vec![Draw::new(cube, Vec3::ZERO).emissive(Vec3::new(0.5, 0.3, 0.1))],
            tone_map: Some(bsengine_core::ToneMap::default()),
            ..Scene::default()
        })
        .centre()
    };
    let s = scene(&mut sdr);
    let want = sdr.render(&s).centre();
    assert_ne!(
        want, ungraded,
        "premise: the grade changes the colour in SDR"
    );
    let s = scene(&mut hdr);
    let got = hdr.render(&s).centre();
    for c in 0..3 {
        assert!(
            (want[c] as i32 - got[c] as i32).abs() <= 2,
            "the HDR frame should be graded as the SDR one: {got:?} against {want:?}"
        );
    }
}

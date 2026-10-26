//! HDR display output (`bsengine_core::HdrOutputSettings`): the frame with
//! mid-tones as SDR shows them and highlights kept up to the display's peak,
//! sent as scRGB (linear BT.709 floats, 1.0 = 80 nits) or as HDR10 (ten-bit
//! PQ over BT.2020).
//!
//! An HDR display is not something a test machine has, so this reads the
//! target the swapchain would present, and compares it against the SDR
//! renderer drawing the same scene: what must match, matches; what must
//! differ -- only the highlights -- differs, by the amounts the settings say.
//! Every such check runs on both signals, HDR10's decoded back into scRGB
//! (`read_pixels_hdr`); the HDR10 tests at the end read its code values
//! themselves, against ST 2084 and BT.2087 worked out here, so an encoder and
//! decoder wrong the same way cannot pass for right.

mod common;

use std::collections::HashMap;

use bsengine_core::{HdrOutputFormat, HdrOutputSettings, UiAnchor, UiState, UiWidget};
use bsengine_rhi_wgpu::surface::HdrSignal;
use common::{Draw, Harness, Light, Scene, HEIGHT, WIDTH};
use glam::Vec3;

/// Paper white 200 nits (SDR white = 2.5 in scRGB), peak 1000 (5x white).
const fn hdr(format: HdrOutputFormat) -> HdrOutputSettings {
    HdrOutputSettings {
        enabled: true,
        format,
        paper_white_nits: Some(200.0),
        max_nits: Some(1000.0),
    }
}
const SCALE: f32 = 200.0 / 80.0;
const PEAK: f32 = 1000.0 / 200.0;

/// The two signals, as the checks below name them.
const SIGNALS: [(&str, HdrOutputFormat); 2] = [
    ("scRGB", HdrOutputFormat::Scrgb),
    ("HDR10", HdrOutputFormat::Hdr10),
];

/// An HDR harness on `format`, and the premise that it carries that signal.
fn hdr_harness(format: HdrOutputFormat) -> Harness {
    let h = Harness::new_hdr(hdr(format));
    let want = match format {
        HdrOutputFormat::Scrgb => HdrSignal::Scrgb,
        HdrOutputFormat::Auto | HdrOutputFormat::Hdr10 => HdrSignal::Hdr10,
    };
    assert_eq!(
        h.surface().hdr_signal(),
        Some(want),
        "premise: the harness outputs {format:?}"
    );
    h
}

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
    for (signal, format) in SIGNALS {
        let mut hdr = hdr_harness(format);
        for e in [0.05f32, 0.2, 0.5] {
            let s = glowing(&mut sdr, e);
            let want = srgb_to_linear(sdr.render(&s).centre()[0]);
            let s = glowing(&mut hdr, e);
            hdr.render(&s);
            let got = hdr_centre(&hdr) / SCALE;
            assert!(
                (got - want).abs() <= 0.01 + want * 0.03,
                "{signal}, emissive {e}: SDR shows {want:.4} of white, HDR {got:.4}"
            );
        }
    }
}

/// Above white, SDR clips and HDR does not: a surface SDR shows as plain
/// white comes out well above paper white, and no surface, however bright,
/// beyond the display's peak.
#[test]
fn hdr_output_keeps_highlights_up_to_the_peak() {
    let mut sdr = Harness::new();
    let s = glowing(&mut sdr, 8.0);
    assert_eq!(
        sdr.render(&s).centre()[0],
        255,
        "premise: SDR shows this surface as plain white"
    );
    for (signal, format) in SIGNALS {
        let mut hdr = hdr_harness(format);
        let s = glowing(&mut hdr, 8.0);
        hdr.render(&s);
        let bright = hdr_centre(&hdr) / SCALE;
        assert!(
            bright > 1.5 && bright <= PEAK * 1.01,
            "{signal}: a surface SDR clips to white should come out between 1.5x \
             white and the peak ({PEAK}x): {bright:.3}"
        );
        let s = glowing(&mut hdr, 1000.0);
        hdr.render(&s);
        let blinding = hdr_centre(&hdr) / SCALE;
        // HDR10's ten bits put a code step about 1% apart up here.
        assert!(
            blinding <= PEAK * 1.01 && blinding > bright,
            "{signal}: brighter still, and never past the peak ({PEAK}x): {blinding:.3}"
        );
    }
}

/// A UI whose only widget is a `width` x `height` image of `colour` at
/// (40, 30): drawn until egui stops fading it in, then the frame's pixel at
/// (80, 60), inside it.
fn draw_ui_image(h: &mut Harness, colour: [u8; 4]) -> Scene {
    let tex = h.two_colour_texture(colour, colour);
    let mut ui = UiState::default();
    ui.set_widget(UiWidget::Image {
        id: "swatch".into(),
        texture_path: "ui/swatch.png".into(),
        x: 40.0,
        y: 30.0,
        width: 80.0,
        height: 60.0,
        anchor: UiAnchor::TOP_LEFT,
    });
    let scene = Scene {
        ui,
        ui_textures: HashMap::from([("ui/swatch.png".to_string(), tex)]),
        ..Scene::default()
    };
    // egui draws nothing the first frame it sees a widget, and then fades
    // the area in by the clock: drawn until it stops changing.
    let mut last = -1.0f32;
    for _ in 0..120 {
        h.render(&scene);
        let now = h.last_hdr()[UI_AT];
        if now > 0.5 && (now - last).abs() < 1e-3 {
            break;
        }
        last = now;
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    scene
}

/// Where [`draw_ui_image`]'s image is read: (80, 60), red channel first.
const UI_AT: usize = ((60 * WIDTH + 80) * 4) as usize;

/// The UI under HDR output shows its white at paper white -- what SDR white
/// is everywhere else -- not at scRGB 1.0, 80 nits, a third of it. egui drawn
/// straight into the float target lands there, and too bright in its
/// mid-tones besides (it writes gamma-encoded values to a non-sRGB target).
#[test]
fn hdr_output_draws_the_ui_at_paper_white() {
    for (signal, format) in SIGNALS {
        let mut h = hdr_harness(format);
        draw_ui_image(&mut h, [255, 255, 255, 255]);
        let f = h.last_hdr();
        let px = [f[UI_AT], f[UI_AT + 1], f[UI_AT + 2]];
        assert!(
            px.iter().all(|&v| (v - SCALE).abs() < 0.05),
            "{signal}: the UI's white should be paper white ({SCALE} in scRGB): {px:?}"
        );
    }
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
    for (signal, format) in SIGNALS {
        let mut hdr = hdr_harness(format);
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
            // Counted, not the worst pixel: SDR decides on 8-bit values and
            // HDR on half floats, so a pixel right at a threshold can go
            // either way (6 of FXAA's 510 here). With the luma left in linear
            // light, every pixel the dim edge's pass changed came out
            // different.
            let different = off_by(&want, &got);
            assert!(
                different * 20 <= changed,
                "{signal}, {name}: the HDR frame should be treated as the SDR one is: \
                 {different} of the {changed} pixels it changes come out otherwise"
            );
        }
    }
}

/// A colour grade is authored for SDR's 0..1, and applies there under HDR
/// output too: below white, the graded HDR frame is the graded SDR one.
#[test]
fn hdr_output_grades_what_sdr_would() {
    let mut sdr = Harness::new();
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
    for (signal, format) in SIGNALS {
        let mut hdr = hdr_harness(format);
        let s = scene(&mut hdr);
        let got = hdr.render(&s).centre();
        for c in 0..3 {
            assert!(
                (want[c] as i32 - got[c] as i32).abs() <= 2,
                "{signal}: the HDR frame should be graded as the SDR one: {got:?} \
                 against {want:?}"
            );
        }
    }
}

/// SMPTE ST 2084's inverse EOTF, on the CPU and apart from the shader's:
/// absolute nits to the ten-bit PQ code value.
fn pq_code(nits: f64) -> u16 {
    let (m1, m2) = (2610.0 / 16384.0, 2523.0 / 4096.0 * 128.0);
    let (c1, c2, c3) = (
        3424.0 / 4096.0,
        2413.0 / 4096.0 * 32.0,
        2392.0 / 4096.0 * 32.0,
    );
    let y = (nits / 10_000.0).powf(m1);
    (((c1 + c2 * y) / (1.0 + c3 * y)).powf(m2) * 1023.0).round() as u16
}

/// The HDR10 signal's white is paper white in absolute nits: UI white at 200
/// nits is code 592 in every channel, the standard's value and nothing
/// relative to it -- a frame left in scRGB's units, or put at 80 nits, or
/// gamma-encoded instead of PQ, all land elsewhere.
#[test]
fn hdr10_sends_paper_white_as_its_pq_code() {
    assert_eq!(pq_code(200.0), 592, "premise: ST 2084's code for 200 nits");
    let mut h = hdr_harness(HdrOutputFormat::Hdr10);
    draw_ui_image(&mut h, [255, 255, 255, 255]);
    let px = h.last_hdr10()[UI_AT / 4];
    for (c, &code) in px[..3].iter().enumerate() {
        assert!(
            code.abs_diff(592) <= 1,
            "channel {c}: paper white should be PQ code 592: {px:?}"
        );
    }
    assert_eq!(px[3], 3, "the signal is opaque");
}

/// HDR10's primaries are BT.2020's: BT.709 red, sent at paper white, is
/// mostly red with some green and a little blue there (ITU-R BT.2087: 0.6274,
/// 0.0691, 0.0164 of it). Only the matrix moves energy into green and blue;
/// a signal that kept BT.709's primaries would send red alone.
#[test]
fn hdr10_sends_bt709_colours_in_bt2020_primaries() {
    let want = [0.627_404, 0.069_097, 0.016_392].map(|share| pq_code(share * 200.0));
    assert_eq!(
        want,
        [543, 333, 224],
        "premise: worked out from the standards"
    );
    let mut h = hdr_harness(HdrOutputFormat::Hdr10);
    draw_ui_image(&mut h, [255, 0, 0, 255]);
    let px = h.last_hdr10()[UI_AT / 4];
    for c in 0..3 {
        assert!(
            px[c].abs_diff(want[c]) <= 1,
            "BT.709 red at paper white should be BT.2020 {want:?}: {px:?}"
        );
    }
}

/// Under HDR10 the UI is laid over the frame in the output pass, from a layer
/// egui only draws into on frames that have a UI. A frame without one must not
/// show the layer's last contents: here the swatch goes away, and what was
/// white under it comes back to the backdrop.
#[test]
fn hdr10_shows_no_ui_on_a_frame_without_one() {
    let mut h = hdr_harness(HdrOutputFormat::Hdr10);
    draw_ui_image(&mut h, [255, 255, 255, 255]);
    let with_ui = h.last_hdr()[UI_AT];
    assert!(
        (with_ui - SCALE).abs() < 0.05,
        "premise: the swatch showed ({with_ui})"
    );
    h.render(&Scene::default());
    let without = h.last_hdr()[UI_AT];
    assert!(
        without < SCALE * 0.5,
        "with the UI gone the swatch should be gone too: {without} (it was {with_ui})"
    );
}

/// A resize makes the HDR10 frame anew at the new size, and the output pass
/// must read the new one: its bind group holds the frame's view, and one left
/// over from before keeps the old texture alive. Resized to a different size
/// -- at the same size a stale frame is still the one the chain writes, and
/// nothing shows -- the HDR10 frame must be the scRGB one resized alike.
#[test]
fn hdr10_follows_the_frame_through_a_resize() {
    let (w, h) = (300u32, 225u32);
    let frame = |format| {
        let mut harness = hdr_harness(format);
        let s = glowing(&mut harness, 0.5);
        harness.render(&s);
        harness.resize(w, h);
        let s = glowing(&mut harness, 0.5);
        harness.render(&s);
        harness.last_hdr()
    };
    let scrgb = frame(HdrOutputFormat::Scrgb);
    let hdr10 = frame(HdrOutputFormat::Hdr10);
    assert_eq!(
        (scrgb.len(), hdr10.len()),
        ((w * h * 4) as usize, (w * h * 4) as usize),
        "premise: both frames are the new size"
    );
    let centre = ((h / 2 * w + w / 2) * 4) as usize;
    assert!(
        scrgb[centre] > SCALE * 0.1,
        "premise: the cube is at the resized frame's centre ({})",
        scrgb[centre]
    );
    let differ = (0..scrgb.len())
        .step_by(4)
        .filter(|&i| (scrgb[i] - hdr10[i]).abs() > 0.02 + scrgb[i] * 0.05)
        .count();
    assert!(
        differ * 100 <= (w * h) as usize,
        "after a resize the HDR10 frame should be the scRGB one: {differ} of {} \
         pixels differ",
        w * h
    );
}

/// `Auto` -- the default -- sends HDR10 where the display offers it, as
/// Unreal and Unity HDRP do; the offscreen target stands in for a display
/// that offers both.
#[test]
fn hdr_output_auto_sends_hdr10() {
    let h = Harness::new_hdr(hdr(HdrOutputFormat::Auto));
    assert_eq!(h.surface().hdr_signal(), Some(HdrSignal::Hdr10));
}

/// HDR output on `format` with both levels left to the display.
const fn from_display(format: HdrOutputFormat) -> HdrOutputSettings {
    HdrOutputSettings {
        enabled: true,
        format,
        paper_white_nits: None,
        max_nits: None,
    }
}

/// A Windows display: SDR white at `sdr` nits, a `max`-nit panel.
fn windows_display(sdr: f32, max: f32) -> wgpu::DisplayHdrInfo {
    wgpu::DisplayHdrInfo {
        luminance: Some(wgpu::DisplayLuminance {
            max_nits: Some(max),
            sdr_white_nits: Some(sdr),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// With paper white left to the display, the UI's white is the display's SDR
/// white -- 280 nits here, the OS's brightness slider, so 3.5 in scRGB and PQ
/// code 629 -- not the 200-nit default.
#[test]
fn hdr_output_takes_paper_white_from_the_display() {
    let display = windows_display(280.0, 1500.0);
    for (signal, format) in SIGNALS {
        let mut h = Harness::new_hdr_on(from_display(format), &display);
        draw_ui_image(&mut h, [255, 255, 255, 255]);
        let f = h.last_hdr();
        let px = [f[UI_AT], f[UI_AT + 1], f[UI_AT + 2]];
        assert!(
            px.iter().all(|&v| (v - 3.5).abs() < 0.06),
            "{signal}: the UI's white should be the display's SDR white (3.5): {px:?}"
        );
    }
    let mut h = Harness::new_hdr_on(from_display(HdrOutputFormat::Hdr10), &display);
    draw_ui_image(&mut h, [255, 255, 255, 255]);
    let code = h.last_hdr10()[UI_AT / 4][0];
    assert!(
        code.abs_diff(pq_code(280.0)) <= 1,
        "HDR10: 280 nits is code {}: {code}",
        pq_code(280.0)
    );
}

/// With the peak left to the display, highlights run up to the display's
/// own: a 4000-nit panel at 200-nit white has 20x white of headroom, four
/// times the 1000-nit default's, and a blinding surface uses it.
#[test]
fn hdr_output_takes_the_peak_from_the_display() {
    for (signal, format) in SIGNALS {
        let mut h = Harness::new_hdr_on(from_display(format), &windows_display(200.0, 4000.0));
        let s = glowing(&mut h, 1000.0);
        h.render(&s);
        let blinding = hdr_centre(&h) / SCALE;
        assert!(
            blinding > PEAK * 2.0 && blinding <= 20.0 * 1.01,
            "{signal}: a blinding surface should reach toward the display's 20x \
             white, past the default's {PEAK}x: {blinding:.2}"
        );
    }
}

/// On a Mac's EDR, scRGB 1.0 is the system's own SDR white, so the UI's white
/// is 1.0 there -- at 200 nits / 80 it would be two and a half times the
/// desktop's white -- and highlights stop at the headroom there is now.
#[test]
fn hdr_output_on_edr_puts_sdr_white_at_edr_white() {
    let mac = wgpu::DisplayHdrInfo {
        headroom: Some(wgpu::DisplayHeadroom {
            current: Some(3.0),
            potential: Some(16.0),
            reference: None,
        }),
        ..Default::default()
    };
    let mut h = Harness::new_hdr_on(from_display(HdrOutputFormat::Scrgb), &mac);
    draw_ui_image(&mut h, [255, 255, 255, 255]);
    let f = h.last_hdr();
    assert!(
        (f[UI_AT] - 1.0).abs() < 0.02,
        "the UI's white should be EDR white, 1.0: {}",
        f[UI_AT]
    );
    let s = glowing(&mut h, 1000.0);
    h.render(&s);
    let blinding = hdr_centre(&h);
    assert!(
        blinding > 2.0 && blinding <= 3.0 * 1.001,
        "highlights should stop at the 3x headroom: {blinding:.3}"
    );
}

/// A window moved to another display is told that display's levels from
/// the next frame -- in the composite, the UI layer's pass and the HDR10
/// output pass alike -- and an answer that knows nothing (Metal off the main
/// thread) changes nothing back.
#[test]
fn hdr_output_follows_the_window_to_another_display() {
    for (signal, format) in SIGNALS {
        let mut h = Harness::new_hdr_on(from_display(format), &windows_display(200.0, 1000.0));
        draw_ui_image(&mut h, [255, 255, 255, 255]);
        let before = h.last_hdr()[UI_AT];
        assert!(
            (before - 2.5).abs() < 0.05,
            "{signal}: premise: 200 nits first ({before})"
        );
        h.move_to_display(&windows_display(320.0, 1000.0));
        let scene = draw_ui_image(&mut h, [255, 255, 255, 255]);
        let after = h.last_hdr()[UI_AT];
        assert!(
            (after - 4.0).abs() < 0.07,
            "{signal}: on the 320-nit display the UI's white should be 4.0: {after}"
        );
        h.move_to_display(&wgpu::DisplayHdrInfo::default());
        h.render(&scene);
        let kept = h.last_hdr()[UI_AT];
        assert!(
            (kept - after).abs() < 0.02,
            "{signal}: an empty answer should leave the levels as they were: {kept} \
             (was {after})"
        );
    }
}

/// The scene follows the new display too, not only the UI: a mid-tone surface
/// is the same share of SDR white on either display, so it comes out 320 / 200
/// times as bright in scRGB after the move -- the composite was told.
#[test]
fn hdr_output_scene_follows_the_window_to_another_display() {
    for (signal, format) in SIGNALS {
        let mut h = Harness::new_hdr_on(from_display(format), &windows_display(200.0, 1000.0));
        let s = glowing(&mut h, 0.3);
        h.render(&s);
        let before = hdr_centre(&h);
        h.move_to_display(&windows_display(320.0, 1000.0));
        let s = glowing(&mut h, 0.3);
        h.render(&s);
        let after = hdr_centre(&h);
        let ratio = after / before;
        assert!(
            (ratio - 1.6).abs() < 0.06,
            "{signal}: the scene's mid-tone should brighten by 320/200 with the \
             display's white: {before:.4} -> {after:.4} ({ratio:.3}x)"
        );
    }
}

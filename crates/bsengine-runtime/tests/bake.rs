//! `bsengine-runtime --bake <dir>` end to end: the real binary, a real scene,
//! real files.
//!
//! The baker's arithmetic is `bsengine-bake`'s tests' business, against closed
//! forms. What only this file can prove is the chain around it: that the
//! scene's static entities -- and only those -- reach the baker with the
//! triangles the renderer draws, that their shadows land where their geometry
//! is, and that the files a scene load reads back are written where it looks.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

/// A throwaway project, removed on drop.
struct Probe(PathBuf);

impl Drop for Probe {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create directories");
    std::fs::write(path, contents).expect("write");
}

const STATIC: &str = r#"components: [("bsengine_core::lightmap_static::LightmapStatic", "(resolution_scale: 1.0)")]"#;

impl Probe {
    /// An 8 x 8 floor with a crate standing in its middle, both static; a
    /// ball beside them that is not; an overcast sky (the sun's ambient) and
    /// a sun. `extra` is appended to the entity list.
    fn create(extra: &str) -> Self {
        Self::create_with("Indirect", extra)
    }

    /// [`Self::create`] with the sun's `bake_mode`.
    fn create_with(sun_mode: &str, extra: &str) -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "bsengine-bake-cli-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        write(
            &dir.join("project.toml"),
            "[project]\nname = \"Bake Probe\"\nentry_scene = \"assets/scenes/main.ron\"\n\n\
             [window]\nwidth = 64\nheight = 64\n",
        );
        write(
            &dir.join("assets/scenes/main.ron"),
            &format!(
                r#"SceneDescriptor(entities: [
    EntityDescriptor(name: "Camera", camera: true, transform: Some((position: (0.0, 6.0, 10.0))), look_at: Some((0.0, 0.0, 0.0))),
    EntityDescriptor(name: "Sun", directional_light: Some((direction: (-0.3, -1.0, -0.2), color: (1.0, 1.0, 1.0), ambient: (0.5, 0.5, 0.5), bake_mode: {sun_mode}))),
    EntityDescriptor(name: "Floor", primitive: Some(Plane), transform: Some((position: (0.0, 0.0, 0.0), scale: (8.0, 1.0, 8.0))), {STATIC}),
    EntityDescriptor(name: "Crate", primitive: Some(Cube), transform: Some((position: (0.0, 0.5, 0.0))), {STATIC}),
    EntityDescriptor(name: "Ball", primitive: Some(Sphere), transform: Some((position: (3.0, 0.5, 0.0)))),
    {extra}
])
"#
            ),
        );
        Self(dir)
    }

    fn bake(&self) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
            .arg("--bake")
            .arg(&self.0)
            .args([
                "--samples",
                "24",
                "--bounces",
                "1",
                "--texels-per-unit",
                "8",
            ])
            .output()
            .expect("the runtime binary runs")
    }

    /// [`Self::bake`] on a named backend, with the `--json` report parsed.
    fn bake_on(&self, backend: &str) -> (std::process::Output, serde_json::Value) {
        let out = Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
            .arg("--bake")
            .arg(&self.0)
            .args([
                "--samples",
                "24",
                "--bounces",
                "1",
                "--texels-per-unit",
                "8",
                "--json",
                "--backend",
                backend,
            ])
            .output()
            .expect("the runtime binary runs");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let last = stdout
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        let report = serde_json::from_str(last.trim()).unwrap_or_else(|e| {
            panic!(
                "no JSON report ({e}): {stdout}\n{}",
                String::from_utf8_lossy(&out.stderr)
            )
        });
        (out, report)
    }

    /// The floor's lightmap at its open corner and under the crate.
    fn floor_open_and_under(&self) -> (f32, f32) {
        let data = self.data();
        let floor = data.entries.iter().find(|e| e.entity == "Floor").unwrap();
        let img = image::open(self.0.join(&floor.texture)).unwrap().to_rgba8();
        let [sx, sy, ox, oy] = floor.scale_offset;
        let at = |u: f32, v: f32| texel(&img, ox + u * sx, oy + v * sy);
        (at(0.06, 0.06), at(0.5, 0.5))
    }

    fn data(&self) -> bsengine_scene::lightmaps::LightmapData {
        let text = std::fs::read_to_string(self.0.join("assets/scenes/main.lightmaps"))
            .expect("the bake wrote the data file beside the scene");
        ron::from_str(&text).expect("the data file parses")
    }
}

fn texel(img: &image::RgbaImage, u: f32, v: f32) -> f32 {
    let (w, h) = img.dimensions();
    let p = img.get_pixel(
        ((u * w as f32) as u32).min(w - 1),
        ((v * h as f32) as u32).min(h - 1),
    );
    bsengine_bake::decode_rgbm([p[0], p[1], p[2], p[3]], bsengine_core::LIGHTMAP_RGBM_RANGE).x
}

/// The static entities -- and only they -- are baked into an RGBM layer with a
/// linear sidecar, and the floor is dark where the crate stands on it and
/// open-sky bright at its edge: the crate's geometry reached the baker in the
/// right place.
#[test]
fn a_scene_bakes_its_static_entities_and_their_shadows() {
    let probe = Probe::create("");
    let out = probe.bake();
    assert!(
        out.status.success(),
        "bake failed: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let data = probe.data();
    let names: Vec<&str> = data.entries.iter().map(|e| e.entity.as_str()).collect();
    assert_eq!(
        names,
        ["Crate", "Floor"],
        "the static two, by name; not the ball"
    );
    assert_eq!(
        data.scene_fingerprint,
        bsengine_scene::lightmaps::fingerprint(
            &std::fs::read_to_string(probe.0.join("assets/scenes/main.ron")).unwrap()
        )
    );

    let floor = &data.entries[1];
    let layer = probe.0.join(&floor.texture);
    let meta = std::fs::read_to_string(format!("{}.meta", layer.display()))
        .expect("the layer has a sidecar");
    assert!(meta.contains("srgb: false"), "RGBM is linear: {meta}");
    let img = image::open(&layer).expect("the layer is a PNG").to_rgba8();

    // The floor's own uv2 spans its rectangle; its centre is under the crate
    // and its corner is open to the sky.
    let [sx, sy, ox, oy] = floor.scale_offset;
    let at = |u: f32, v: f32| texel(&img, ox + u * sx, oy + v * sy);
    let open = at(0.06, 0.06);
    let under = at(0.5, 0.5);
    assert!(
        open > 0.2,
        "premise: an open corner of the floor sees the sky: {open}"
    );
    assert!(
        under < open * 0.5,
        "under the crate the floor is shadowed: {under} vs {open}"
    );
}

/// `--backend gpu` and `--backend cpu` bake the same scene to the same light,
/// through the whole chain -- the scene gathered, traced, written -- and each
/// report says which ran. The sun is `Baked`, so the direct term and its
/// shadow under the crate are in both; the two agree within the noise of 24
/// samples. (Texel for texel against closed forms is `bsengine-bake`'s; what
/// this adds is that the runtime hands the GPU the same input.)
#[test]
fn the_gpu_and_cpu_backends_bake_the_same_light() {
    let probe = Probe::create_with("Baked", "");
    let mut seen = Vec::new();
    for backend in ["cpu", "gpu"] {
        let (out, report) = probe.bake_on(backend);
        assert!(
            out.status.success(),
            "--backend {backend}: {report}\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            report["backend"], backend,
            "the report names the backend that ran"
        );
        seen.push(probe.floor_open_and_under());
    }
    let ((cpu_open, cpu_under), (gpu_open, gpu_under)) = (seen[0], seen[1]);
    assert!(
        gpu_open > 0.5 && gpu_under < gpu_open * 0.5,
        "premise: the GPU bake lit the open floor and shadowed under the crate: {gpu_open}, {gpu_under}"
    );
    assert!(
        (gpu_open - cpu_open).abs() < 0.05,
        "open: gpu {gpu_open} vs cpu {cpu_open}"
    );
    assert!(
        (gpu_under - cpu_under).abs() < 0.05,
        "under: gpu {gpu_under} vs cpu {cpu_under}"
    );
}

/// `auto` takes a hardware GPU when the machine has one and the CPU when it
/// has none or only a software rasteriser -- the same answer
/// `GpuBaker::new_hardware` gives this test, whichever machine it runs on.
#[test]
fn auto_bakes_on_a_hardware_gpu_when_there_is_one() {
    let expected = if bsengine_bake::GpuBaker::new_hardware().is_ok() {
        "gpu"
    } else {
        "cpu"
    };
    let probe = Probe::create("");
    let (out, report) = probe.bake_on("auto");
    assert!(out.status.success(), "{report}");
    assert_eq!(report["backend"], expected);
}

/// A backend that does not exist is refused, by name, and nothing is written.
#[test]
fn an_unknown_backend_is_refused() {
    let probe = Probe::create("");
    let (out, report) = probe.bake_on("vulkan");
    assert!(!out.status.success());
    let error = report["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("vulkan") && error.contains("cpu, gpu or auto"),
        "{error}"
    );
    assert!(!probe.0.join("assets/scenes/main.lightmaps").exists());
}

/// A bake of an unchanged scene is the same bytes, and keeps each layer's
/// identity: re-baking should not churn a project's history.
#[test]
fn baking_twice_writes_the_same_files() {
    let probe = Probe::create("");
    assert!(probe.bake().status.success());
    let layer = probe.0.join(&probe.data().entries[0].texture);
    let first = std::fs::read(&layer).unwrap();
    let first_meta = std::fs::read_to_string(format!("{}.meta", layer.display())).unwrap();
    assert!(probe.bake().status.success());
    assert_eq!(std::fs::read(&layer).unwrap(), first, "the same pixels");
    assert_eq!(
        std::fs::read_to_string(format!("{}.meta", layer.display())).unwrap(),
        first_meta,
        "the same identity and hash"
    );
}

/// Two static entities with one name cannot both be recorded by name: the
/// bake refuses, says which name, and writes nothing.
#[test]
fn duplicate_static_names_are_refused() {
    let probe = Probe::create(&format!(
        r#"EntityDescriptor(name: "Crate", primitive: Some(Cube), transform: Some((position: (2.0, 0.5, 2.0))), {STATIC}),"#
    ));
    let out = probe.bake();
    assert!(!out.status.success(), "a duplicate name must fail the bake");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Crate"), "the message names it: {stderr}");
    assert!(!probe.0.join("assets/scenes/main.lightmaps").exists());
}

/// A `Baked` sun's direct light is in the lightmap, an `Indirect` one's is
/// not: on the open floor the first stores the sky plus the sun's E / pi, the
/// second the sky alone (0.5, the ambient). The sun's cosine on the floor is
/// 1 / |(-0.3, -1, -0.2)| ~= 0.94, so the difference is about 0.94 / pi.
#[test]
fn a_baked_suns_direct_light_is_in_the_lightmap() {
    let open_floor = |probe: &Probe| {
        let out = probe.bake();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let data = probe.data();
        let floor = data.entries.iter().find(|e| e.entity == "Floor").unwrap();
        let img = image::open(probe.0.join(&floor.texture))
            .unwrap()
            .to_rgba8();
        let [sx, sy, ox, oy] = floor.scale_offset;
        texel(&img, ox + 0.06 * sx, oy + 0.06 * sy)
    };
    let indirect = open_floor(&Probe::create_with("Indirect", ""));
    let baked = open_floor(&Probe::create_with("Baked", ""));
    assert!(
        (indirect - 0.5).abs() < 0.05,
        "premise: the sky alone: {indirect}"
    );
    let sun = 1.0 / (0.3f32 * 0.3 + 1.0 + 0.2 * 0.2).sqrt() / std::f32::consts::PI;
    assert!(
        (baked - indirect - sun).abs() < 0.05,
        "the baked sun adds its E / pi ({sun}): {baked} vs {indirect}"
    );
}

/// Each layer's directions are written beside it as `lightmap_<n>_dir.png`,
/// linear like the layer, and the data names them per entry. What they hold
/// is where the light came from: on the open floor the sky alone arrives from
/// above, and a baked sun pulls the mean toward itself -- toward
/// `(0.3, 0.2)` in x/z, the reverse of its direction. (The direction's own
/// arithmetic is `bsengine-bake`'s; this is the runtime writing what it got.)
#[test]
fn a_bake_writes_where_each_texels_light_came_from() {
    let open_floor = |probe: &Probe| {
        let out = probe.bake();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let data = probe.data();
        let floor = data.entries.iter().find(|e| e.entity == "Floor").unwrap();
        assert_eq!(
            floor.direction,
            floor.texture.replace(".png", "_dir.png"),
            "the entry names its layer's directions"
        );
        let file = probe.0.join(&floor.direction);
        let meta = std::fs::read_to_string(format!("{}.meta", file.display()))
            .expect("the directions have a sidecar");
        assert!(
            meta.contains("srgb: false"),
            "directions are linear: {meta}"
        );
        let img = image::open(&file).expect("a PNG").to_rgba8();
        let (w, h) = img.dimensions();
        let [sx, sy, ox, oy] = floor.scale_offset;
        let p = img.get_pixel(
            (((ox + 0.06 * sx) * w as f32) as u32).min(w - 1),
            (((oy + 0.06 * sy) * h as f32) as u32).min(h - 1),
        );
        bsengine_bake::decode_direction([p[0], p[1], p[2], p[3]])
    };
    let sky = open_floor(&Probe::create_with("Indirect", ""));
    let baked = open_floor(&Probe::create_with("Baked", ""));
    assert!(
        sky.y > 0.3 && sky.length() < 1.02,
        "premise: the sky's light comes from above, and a mean is no longer than 1: {sky}"
    );
    let toward_sun = (baked - sky).x * 0.3 + (baked - sky).z * 0.2;
    assert!(
        toward_sun > 0.02,
        "the baked sun pulls the mean toward itself: {baked} vs {sky}"
    );
}

/// An adaptive volume baked `Once`, over the probe scene's floor and crate.
const ADAPTIVE_VOLUME: &str = r#"EntityDescriptor(name: "Probes", transform: Some((position: (0.0, 2.0, 0.0))), components: [("bsengine_core::light_probe::LightProbeVolume", "(half_extents: (3.0, 3.0, 3.0), placement: Adaptive, update: Once, min_probe_spacing: 0.3, max_probe_spacing: 1.5)")]),"#;

/// A scene with an adaptive volume baked `Once` gets its probes baked with
/// its lightmaps: `probes.bin` beside the layers, named for the volume in the
/// scene's data, reading back as a layout cut around the crate and floor. And
/// they hold the scene's light: a probe at the volume's top, nothing above it
/// but the overcast sky (0.5), receives that sky facing up -- within L2's
/// ringing of the floor's step below.
#[test]
fn an_adaptive_volume_is_baked_with_the_lightmaps() {
    let probe = Probe::create(ADAPTIVE_VOLUME);
    let out = probe.bake();
    assert!(
        out.status.success(),
        "bake failed: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let data = probe.data();
    let entry = data.probes.expect("the data names the baked probes");
    assert_eq!(entry.volume, "Probes");
    assert_eq!(entry.file, "assets/lightmaps/main/probes.bin");
    let bytes = std::fs::read(probe.0.join(&entry.file)).expect("the probes were written");
    let baked = bsengine_bake::probe_bake::decode(&bytes).expect("and read back");
    assert_eq!(baked.probes.len(), baked.layout.probes.len());
    assert!(
        baked.layout.bricks.iter().any(|b| b.level == 1),
        "premise: the crate and floor refined the bricks near them"
    );
    assert!(
        baked.layout.bricks.iter().any(|b| b.level == 0),
        "and open air stays coarse"
    );
    let top = baked
        .layout
        .probes
        .iter()
        .position(|p| (p.position.y - 5.0).abs() < 1e-3 && p.position.x.abs() < 1e-3)
        .expect("a probe at the volume's top, over the crate");
    let sh = baked.probes[top].sh;
    // E / PI facing up, the cosine lobe's band factors as shading has them.
    use std::f32::consts::{FRAC_PI_4, PI};
    let up = (sh[0] * (0.282_095 * PI)
        + sh[1] * (0.488_603 * 2.0 * PI / 3.0)
        + sh[6] * (0.315_392 * 2.0 * FRAC_PI_4))
        / PI;
    assert!((up.x - 0.5).abs() < 0.05, "the sky above the volume: {up}");

    // Without the volume, a re-bake removes them: no stale probes to ship.
    let scene = probe.0.join("assets/scenes/main.ron");
    let text = std::fs::read_to_string(&scene).unwrap();
    std::fs::write(&scene, text.replace(ADAPTIVE_VOLUME, "")).unwrap();
    assert!(probe.bake().status.success());
    assert!(probe.data().probes.is_none());
    assert!(
        !probe.0.join(&entry.file).exists(),
        "the old probes are gone"
    );
}

/// A grid volume, or a realtime one, bakes no probes: the raster bake and the
/// update are theirs.
#[test]
fn only_an_adaptive_volume_baked_once_bakes_probes() {
    for volume in [
        ADAPTIVE_VOLUME.replace("placement: Adaptive, ", ""),
        ADAPTIVE_VOLUME.replace("update: Once", "update: Realtime"),
    ] {
        let probe = Probe::create(&volume);
        assert!(probe.bake().status.success());
        assert!(probe.data().probes.is_none(), "{volume}");
    }
}

/// A baked scene's lightmaps ship with it: a packaged build (both a loose
/// copy and a single archive) carries the data file and every layer it
/// names, each with the sidecar that says it is linear -- without which the
/// layer would load as an sRGB colour and every texel would decode wrong.
/// Nothing in the scene names any of them, which is why this can break.
#[test]
fn a_package_carries_the_scenes_lightmaps() {
    let probe = Probe::create("");
    assert!(probe.bake().status.success());
    let layer = probe.data().entries[0].texture.clone();
    let package = |mode: &str| {
        let out = probe.0.join(format!("build-{mode}"));
        let status = Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
            .arg("--package")
            .arg(&probe.0)
            .arg("--out")
            .arg(&out)
            .arg("--mode")
            .arg(mode)
            .status()
            .expect("run --package");
        assert!(status.success(), "packaging as {mode} failed");
        out
    };

    let direction = probe.data().entries[0].direction.clone();
    assert!(!direction.is_empty(), "premise: the bake is directional");

    let loose = package("loose");
    for file in [
        "assets/scenes/main.lightmaps",
        layer.as_str(),
        &format!("{layer}.meta"),
        direction.as_str(),
        &format!("{direction}.meta"),
    ] {
        assert!(loose.join(file).is_file(), "the loose build has {file}");
    }

    let pak = package("pak");
    let archive = bsengine_asset::pak::Pak::open(pak.join(bsengine_asset::cook::PAK_FILE_NAME))
        .expect("the archive opens");
    for file in [
        "assets/scenes/main.lightmaps",
        layer.as_str(),
        direction.as_str(),
    ] {
        assert!(archive.get(file).is_some(), "the archive has {file}");
    }
    let meta = archive
        .get(&format!("{layer}.meta"))
        .expect("the layer's sidecar is archived");
    assert!(
        String::from_utf8_lossy(meta).contains("srgb: false"),
        "and it still says linear"
    );
}

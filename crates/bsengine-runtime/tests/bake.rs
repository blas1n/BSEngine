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
    EntityDescriptor(name: "Sun", directional_light: Some((direction: (-0.3, -1.0, -0.2), color: (1.0, 1.0, 1.0), ambient: (0.5, 0.5, 0.5)))),
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

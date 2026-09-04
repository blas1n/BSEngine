use std::env;

use bsengine_app::{
    new_app, AnimationPlugin, AnimationStateMachinePlugin, LifetimePlugin, NavMeshPlugin,
    ParticlePlugin, TerrainBrushPlugin, TerrainPlugin, TimePlugin,
};
use bsengine_asset::{AssetIdentityPlugin, AssetPlugin, AssetStatusPlugin, AssetWatcherPlugin};
use bsengine_audio::AudioPlugin;
use bsengine_core::{EditorPlayState, InspectorState};
use bsengine_editor::EditorPlugin;
use bsengine_gltf::{GltfPlugin, SkinnedMeshPlugin};
use bsengine_input::InputPlugin;
use bsengine_network::NetworkPlugin;
use bsengine_physics::PhysicsPlugin;
use bsengine_render::RenderPlugin;
use bsengine_rhi_wgpu::WgpuRHIPlugin;
use bsengine_scene::ScenePlugin;
use bsengine_scripting::ScriptingPlugin;
use bsengine_window::{WindowDescriptor, WindowPlugin};

mod scene_systems;
mod test_mode;
mod test_protocol;
mod test_query;

use scene_systems::{register_scene_systems, ProjectManifest};

fn main() {
    let mut args = env::args().skip(1);
    let first_arg = args.next().unwrap_or_else(|| ".".to_string());

    if first_arg == "--fixup" {
        let project_dir = args.next().unwrap_or_else(|| ".".to_string());
        let as_json = match args.next().as_deref() {
            Some("--json") => true,
            Some(other) => panic!("unknown argument after project dir: {other}"),
            None => false,
        };
        std::process::exit(run_fixup(&project_dir, as_json));
    }

    if first_arg == "--package" {
        let project_dir = args.next().unwrap_or_else(|| ".".to_string());
        let mut out_dir = None;
        let mut mode = None;
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--out" => {
                    out_dir = Some(
                        args.next()
                            .unwrap_or_else(|| panic!("--out requires a directory")),
                    );
                }
                "--mode" => {
                    let value = args
                        .next()
                        .unwrap_or_else(|| panic!("--mode requires loose or pak"));
                    mode = Some(match value.as_str() {
                        "loose" => bsengine_asset::cook::PackageMode::Loose,
                        "pak" => bsengine_asset::cook::PackageMode::Pak,
                        other => panic!("unknown --mode {other}; expected loose or pak"),
                    });
                }
                other => panic!("unknown argument after project dir: {other}"),
            }
        }
        let out_dir = out_dir.unwrap_or_else(|| format!("{project_dir}/dist"));
        std::process::exit(run_package(&project_dir, &out_dir, mode));
    }

    if first_arg == "--test" {
        let project_dir = args.next().unwrap_or_else(|| ".".to_string());
        match args.next().as_deref() {
            Some("--replay") => {
                let log_path = args
                    .next()
                    .unwrap_or_else(|| panic!("--replay requires a log file path"));
                let passed = test_mode::run_replay_mode(&project_dir, &log_path);
                std::process::exit(if passed { 0 } else { 1 });
            }
            Some(other) => panic!("unknown argument after project dir: {other}"),
            None => test_mode::run_test_mode(&project_dir),
        }
        return;
    }

    run_windowed(&first_arg);
}

/// `--fixup <dir> [--json]`: settles every reference in a project that only
/// resolves because the engine remembers where an asset used to be, then forgets
/// the memories nothing needs any more.
///
/// # Why this is a mode of the runtime rather than a tool of its own
///
/// It is the counterpart of the warnings `--test` and the windowed app already
/// print. `bsengine-scene` and `bsengine_asset::load_async` both warn, every
/// time, that a reference resolved somewhere other than what it spells and that
/// the file should be re-saved; this is the command that does the re-saving.
/// Putting it beside `--test` is what makes it findable from the same place the
/// warning is read, and it costs nothing at run time — the branch is taken
/// before any `App` is built.
///
/// # It builds no engine
///
/// No window, no renderer, no scripting VM, not even a Bevy `App`. `fixup` is a
/// directory walk and a text edit, so this runs against a project that has never
/// been launched and finishes in milliseconds. That is deliberate: a repair tool
/// that needed the game to boot could not repair a project the game cannot boot.
///
/// # Output, and the exit code
///
/// The report goes to **stdout** — as text for a human, or as JSON with
/// `--json`, which is what `bsengine-mcp`'s `game_fixup` reads. Everything the
/// scan itself has to say goes to **stderr** through the ordinary logging setup,
/// so one stream is the answer and the other is the commentary and a caller can
/// parse the first without filtering the second.
///
/// Exits `1` when the project could not be scanned at all, and when the report
/// carries a problem — a scene that could not be written, one that will not
/// parse, a reference too ambiguous to touch. Each of those is work `fixup` was
/// asked to do and did not, so a script that runs this must not read it as
/// success. A stale path in JavaScript is *not* a problem in that sense: it is
/// reported for a human to act on, and exiting non-zero for it would mean a
/// project could never be clean.
fn run_fixup(project_dir: &str, as_json: bool) -> i32 {
    bsengine_core::init_logging();

    let report = match bsengine_asset::identity::fixup(project_dir) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("fixup: cannot scan {project_dir}/assets ({e})");
            return 1;
        }
    };

    if as_json {
        match serde_json::to_string_pretty(&report) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("fixup: cannot encode the report ({e})");
                return 1;
            }
        }
    } else {
        print!("{report}");
    }

    i32::from(!report.problems.is_empty())
}

/// `--package <dir> [--out <dir>]`: collects exactly the assets the project
/// reaches and writes a build that runs without the editor.
///
/// # Why this is a mode of the runtime rather than a tool of its own
///
/// Two reasons, and the second is the load-bearing one. It is the binary that
/// already knows how to read a project, so a separate tool would be a second
/// thing to keep in step with the manifest format. And the executable a build
/// needs *is this process* — `bsengine-runtime` is the shipping runtime, so
/// `current_exe()` is exactly the binary to copy, with nothing to locate and
/// nothing to get wrong.
///
/// # Output, and the exit code
///
/// Exits `1` when a reference resolves to nothing, when the output directory is
/// occupied, or when the build could not be written. Each of those is work
/// `--package` was asked to do and did not.
///
/// A path spelled only in a *script* that resolves to nothing is printed as a
/// warning and does **not** fail the run, for the reason
/// [`bsengine_asset::cook::ScriptMention`] records: it is a guess that a quoted
/// string was a path, and it can as easily be dead code.
fn run_package(
    project_dir: &str,
    out_dir: &str,
    mode_override: Option<bsengine_asset::cook::PackageMode>,
) -> i32 {
    bsengine_core::init_logging();

    let manifest_path = format!("{project_dir}/project.toml");
    let manifest_str = match std::fs::read_to_string(&manifest_path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("package: cannot read {manifest_path} ({e})");
            return 1;
        }
    };
    let manifest: ProjectManifest = match toml::from_str(&manifest_str) {
        Ok(manifest) => manifest,
        Err(e) => {
            eprintln!("package: cannot parse {manifest_path} ({e})");
            return 1;
        }
    };

    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("package: cannot locate this executable ({e})");
            return 1;
        }
    };

    // The flag wins over the manifest, the ordinary precedence for a build
    // option: the setting says what this project normally produces, the flag
    // says what this one invocation should.
    let mode = mode_override.unwrap_or(manifest.package.mode);

    let cooked = match bsengine_asset::cook::package(
        project_dir,
        &manifest.project.entry_scene,
        &manifest.package.extra_assets,
        mode,
        &exe,
        std::path::Path::new(out_dir),
    ) {
        Ok(cooked) => cooked,
        Err(e) => {
            eprintln!("package: {e}");
            return 1;
        }
    };

    for mention in &cooked.script_mentions {
        println!(
            "warning: {} names {}, which resolves to nothing",
            mention.script, mention.path
        );
    }

    if !cooked.is_ok() {
        for missing in &cooked.missing {
            eprintln!(
                "error: {} names {}, which resolves to nothing",
                missing.referrer, missing.path
            );
        }
        for problem in &cooked.problems {
            eprintln!("error: {problem}");
        }
        eprintln!(
            "package: {} unresolved reference(s) and {} unreadable file(s); \
             nothing was written",
            cooked.missing.len(),
            cooked.problems.len()
        );
        return 1;
    }

    println!("packaged {} asset(s) into {out_dir}", cooked.assets.len());
    0
}

/// Opens `<project_dir>/game.pak` when this is a packaged build, and installs
/// it for scene reads.
///
/// Returns the archive so the caller can also hand it to
/// [`bsengine_asset::PakAssetPlugin`], which must be added **before**
/// `AssetPlugin` — `bevy_asset` builds its sources during that plugin's
/// `build`, so a source registered afterwards is silently ignored.
///
/// A missing archive is the ordinary unpackaged case and means loose files. One
/// that is present but unreadable is not: it would leave the game reading
/// whatever files happen to be lying around instead of the build it shipped
/// with, so it stops here rather than degrading into a half-working game.
fn open_pak(project_dir: &str) -> Option<std::sync::Arc<bsengine_asset::pak::Pak>> {
    let path = std::path::Path::new(project_dir).join(bsengine_asset::cook::PAK_FILE_NAME);
    if !path.is_file() {
        return None;
    }
    let pak = std::sync::Arc::new(
        bsengine_asset::pak::Pak::open(&path)
            .unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display())),
    );
    bsengine_asset::pak_source::install(pak.clone(), project_dir);
    Some(pak)
}

fn run_windowed(project_dir: &str) {
    let manifest_path = format!("{project_dir}/project.toml");

    let manifest_str = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("Cannot read {manifest_path}: {e}"));
    let manifest: ProjectManifest = toml::from_str(&manifest_str)
        .unwrap_or_else(|e| panic!("Cannot parse {manifest_path}: {e}"));

    let scene_path = format!("{}/{}", project_dir, manifest.project.entry_scene);
    let title = manifest
        .window
        .title
        .clone()
        .unwrap_or_else(|| manifest.project.name.clone());

    let mut app = new_app();
    app.insert_resource(bsengine_core::OcclusionCullingEnabled(
        manifest.render.occlusion_culling,
    ));
    // Before `AssetPlugin`, and that ordering is the whole reason this is a
    // separate plugin: `bevy_asset` builds its sources during that plugin's
    // `build`, so a source registered afterwards is silently ignored -- and a
    // silently ignored pak source means a packaged build quietly reading loose
    // files instead of its own archive.
    if let Some(pak) = open_pak(project_dir) {
        app.add_plugins(bsengine_asset::PakAssetPlugin {
            pak,
            project_dir: project_dir.to_string(),
        });
    }
    app.add_plugins(TimePlugin)
        .add_plugins(AssetPlugin)
        // Windowed only, deliberately. `--test` builds its own app
        // (test_mode::build_test_app) with its own plugin list, so leaving
        // this out of that list is the whole of the decision: a replay has
        // nobody editing files, so a watcher there would buy nothing and
        // cost a background thread plus a source of frame-to-frame variation
        // in the one mode that pins its clocks precisely to stay
        // reproducible. Needs AssetPlugin (for AssetServer) and a ProjectDir,
        // which ScriptingPlugin inserts below at build time — i.e. before any
        // Startup system, including this plugin's, ever runs.
        .add_plugins(AssetWatcherPlugin)
        // Unlike the watcher above, this one is in `--test`'s plugin list
        // too (test_mode::build_test_app) — see there for why. Without it
        // registered *somewhere* the whole status API is inert: the resource
        // never exists, so `AssetStatuses::get` and `Bsengine.getAssetStatus`
        // answer `unknown` for every path forever, including the ones that
        // just failed to load.
        .add_plugins(AssetStatusPlugin)
        // Walks `<ProjectDir>/assets` once at Startup so `ScenePlugin` below
        // can resolve a scene's asset references by identity instead of by
        // path — the point of roadmap item 30. Registered in all three hosts
        // (here, `--test`'s app, and the editor) for the same reason the
        // status plugin is: a reader with nothing to read is not a smaller
        // feature, it is no feature, and this one fails without a symptom —
        // a spawn that finds no index falls back to the stored path and loads
        // exactly as it did before, silently.
        //
        // Order is not left to this list. `ScenePlugin::build` declares
        // `.after(build_asset_index)`; see `AssetIdentityPlugin`'s docs for
        // why being in the same `Startup` schedule is not enough on its own.
        // `ProjectDir` comes from `ScriptingPlugin` at the bottom of this
        // list, inserted at build time and so already present before any
        // Startup system runs — the same arrangement `AssetWatcherPlugin`
        // relies on above.
        .add_plugins(AssetIdentityPlugin)
        .add_plugins(WgpuRHIPlugin::windowed())
        .add_plugins(WindowPlugin {
            descriptor: WindowDescriptor {
                title,
                width: manifest.window.width,
                height: manifest.window.height,
                resizable: manifest.window.resizable,
            },
        })
        .add_plugins(InputPlugin)
        .add_plugins(AudioPlugin)
        .add_plugins(PhysicsPlugin)
        .add_plugins(NetworkPlugin)
        .add_plugins(EditorPlugin)
        .add_plugins(RenderPlugin)
        .add_plugins(GltfPlugin)
        .add_plugins(SkinnedMeshPlugin)
        .add_plugins(AnimationPlugin)
        .add_plugins(AnimationStateMachinePlugin)
        .add_plugins(NavMeshPlugin)
        // Both of these count something down each frame, and neither was
        // installed anywhere until now. `Bsengine.setLifetime()` has existed as
        // a scripting API the whole time with nothing to tick it, so it has
        // never despawned anything in a running game.
        .add_plugins(LifetimePlugin)
        .add_plugins(ParticlePlugin)
        // Loads each scene-authored `Terrain`'s heightmap and spawns its chunk
        // entities (render mesh + Rapier heightfield collider). Was missing
        // from this list entirely until roadmap item 44's demo project needed
        // it -- `Terrain`/`TerrainPlugin` existed and were unit-tested
        // (`bsengine-app`'s own test suite, and `bsengine-editor`'s
        // `terrain_write`/"Create Terrain" spawn paths), but nothing had ever
        // added `TerrainPlugin` to the actual windowed runtime's plugin list,
        // so a `Terrain` entity in a real game would sit in the scene file
        // and never grow chunks -- the whole system was inert in production.
        .add_plugins(TerrainPlugin)
        // Picks the terrain surface point under the cursor while the
        // editor's terrain brush tool is active. Registered here, not just
        // in bsengine-app's own test suite -- see TerrainPlugin's comment
        // above for the exact gap (a plugin present in one host and absent
        // in the other is a feature that works when you look at it and not
        // when you test it) this follows the same precedent to avoid.
        .add_plugins(TerrainBrushPlugin)
        .add_plugins(ScenePlugin::from_file(&scene_path))
        .add_plugins(ScriptingPlugin {
            project_dir: project_dir.to_string(),
        });
    register_scene_systems(&mut app);

    // bsengine-runtime's job is to run a game, not edit one — EditorPlugin
    // is still included (for now, this is the only windowed entry point,
    // and its inspector/hierarchy tooling is useful during development),
    // but it defaults to InspectorState::editor()'s Stopped play state,
    // which silently gates scripts (WASD, onUpdate, ...) off until the
    // user finds and clicks the toolbar's Play button. Force Playing here
    // so `cargo run -p bsengine-runtime -- <game>` actually plays the game
    // immediately, matching what running a game is supposed to do.
    {
        let mut inspector = app.world_mut().resource_mut::<InspectorState>();
        inspector.play_state = EditorPlayState::Playing;
        // Populated on manual Ctrl+S saves otherwise; without this, a
        // freshly-launched game (never saved) has no path for the Play
        // button's "reload the scene" behavior to reload from.
        inspector.current_scene_path = Some(scene_path.clone());
    }

    app.run();
}

use std::env;

use bsengine_app::{
    new_app, AnimationPlugin, AnimationStateMachinePlugin, ClothPlugin, LifetimePlugin,
    NavMeshPlugin, ParticlePlugin, TerrainBrushPlugin, TerrainPlugin, TimePlugin,
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

mod audio_occlusion;
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
                        .unwrap_or_else(|| panic!("--mode requires loose, pak or single"));
                    mode = Some(match value.as_str() {
                        "loose" => bsengine_asset::cook::PackageMode::Loose,
                        "pak" => bsengine_asset::cook::PackageMode::Pak,
                        "single" => bsengine_asset::cook::PackageMode::Single,
                        other => panic!("unknown --mode {other}; expected loose, pak or single"),
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

    let mut frame_limit = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--frames" => {
                let value = args
                    .next()
                    .unwrap_or_else(|| panic!("--frames requires a count"));
                frame_limit = Some(
                    value
                        .parse::<u32>()
                        .unwrap_or_else(|_| panic!("--frames expects a number, got {value}")),
                );
            }
            other => panic!("unknown argument after project dir: {other}"),
        }
    }

    run_windowed(&first_arg, frame_limit);
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
    // The runtime refuses to start on a bad `[input]` binding; a package
    // that cannot start is worse than a package that was never made.
    if let Err(e) = manifest.input.actions() {
        eprintln!("package: {manifest_path} [input]:\n{e}");
        return 1;
    }

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

    // With each block-compressed texture's mip cache made here, so no
    // player's machine encodes one (see `install_shipped_mips`).
    let cooked = match bsengine_asset::cook::package_with_precook(
        project_dir,
        &manifest.project.entry_scene,
        &manifest.package.extra_assets,
        mode,
        &exe,
        std::path::Path::new(out_dir),
        Some(&bsengine_rhi_wgpu::precook_mip_cache),
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

    println!(
        "packaged {} asset(s) into {out_dir}, with {} precooked compressed texture(s)",
        cooked.assets.len(),
        cooked.precooked_mips.len()
    );
    0
}

/// Opens the archive this run plays from, when it is a packaged build, and
/// installs it for scene reads: the one embedded in this executable by
/// `--mode single`, or else `<project_dir>/game.pak` from `--mode pak`.
///
/// Returns the archive so the caller can also hand it to
/// [`bsengine_asset::PakAssetPlugin`], which must be added **before**
/// `AssetPlugin` — `bevy_asset` builds its sources during that plugin's
/// `build`, so a source registered afterwards is silently ignored.
///
/// The embedded archive is looked for first, and a build that has one never
/// looks beside itself: a single-file build is complete by definition, and a
/// `game.pak` lying next to it belongs to something else.
///
/// A missing archive is the ordinary unpackaged case and means loose files. One
/// that is present but unreadable is not: it would leave the game reading
/// whatever files happen to be lying around instead of the build it shipped
/// with, so it stops here rather than degrading into a half-working game.
fn open_pak(project_dir: &str) -> Option<std::sync::Arc<bsengine_asset::pak::Pak>> {
    let pak = match embedded_pak() {
        Some(pak) => pak,
        None => {
            let path = std::path::Path::new(project_dir).join(bsengine_asset::cook::PAK_FILE_NAME);
            if !path.is_file() {
                return None;
            }
            bsengine_asset::pak::Pak::open(&path)
                .unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display()))
        }
    };
    let pak = std::sync::Arc::new(pak);
    bsengine_asset::pak_source::install(pak.clone(), project_dir);
    Some(pak)
}

/// The archive embedded in this executable, if this is a single-file build.
///
/// A damaged trailer is a panic, for the reason `open_pak` gives for an
/// unreadable `game.pak`: the player launched *this* build, and starting a
/// different game out of whatever files surround it is worse than stopping.
fn embedded_pak() -> Option<bsengine_asset::pak::Pak> {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            tracing::warn!("cannot locate this executable ({e}); assuming it embeds no archive");
            return None;
        }
    };
    match bsengine_asset::embed::read_embedded(&exe) {
        Ok(Some(bytes)) => Some(
            bsengine_asset::pak::Pak::from_bytes(bytes).unwrap_or_else(|e| {
                panic!("Cannot read the archive embedded in {}: {e}", exe.display())
            }),
        ),
        Ok(None) => None,
        Err(e) => panic!("Cannot read {}: {e}", exe.display()),
    }
}

/// The project manifest: out of the archive when the build carries it there
/// (`--mode single` stores it as [`bsengine_asset::cook::MANIFEST_ENTRY`]),
/// and from `<project_dir>/project.toml` otherwise.
///
/// Takes the archive [`open_pak`] returned rather than looking it up, so the
/// order -- archive first, then the manifest that may live inside it -- is
/// visible at the call site instead of hidden in a global.
/// `project.toml`'s `[input]` actions, inserted before `InputPlugin` so its
/// `init_resource` leaves them alone. A bad binding stops the game at start,
/// as a manifest that does not parse does: an action bound to a typo would
/// otherwise ship as a button that silently does nothing. `package` checks
/// the same thing, so a packaged build never reaches this panic.
pub(crate) fn insert_input_actions(app: &mut bevy_app::App, manifest: &ProjectManifest) {
    match manifest.input.actions() {
        Ok(actions) => {
            app.insert_resource(actions);
        }
        Err(e) => panic!("Cannot use project.toml's [input] table:\n{e}"),
    }
}

fn read_manifest(project_dir: &str, pak: Option<&bsengine_asset::pak::Pak>) -> ProjectManifest {
    let manifest_path = format!("{project_dir}/project.toml");
    let (text, source) = match pak.and_then(|pak| pak.get(bsengine_asset::cook::MANIFEST_ENTRY)) {
        Some(bytes) => (
            String::from_utf8(bytes.to_vec())
                .unwrap_or_else(|e| panic!("The manifest in the archive is not UTF-8: {e}")),
            "the manifest in the archive".to_string(),
        ),
        None => (
            std::fs::read_to_string(&manifest_path)
                .unwrap_or_else(|e| panic!("Cannot read {manifest_path}: {e}")),
            manifest_path,
        ),
    };
    toml::from_str(&text).unwrap_or_else(|e| panic!("Cannot parse {source}: {e}"))
}

/// Opens the game's window and runs it until it is closed -- or, with
/// `--frames`, until it has drawn that many frames.
///
/// Thin on purpose: everything the player's app needs is assembled by
/// [`build_windowed_app`], and the only thing added here is the optional frame
/// limit. `window_smoke.rs` reaches this through the real executable, so what it
/// certifies is the arrangement a player gets rather than a second one
/// assembled for the test.
fn run_windowed(project_dir: &str, frame_limit: Option<u32>) {
    let mut app = build_windowed_app(project_dir);
    if let Some(frames) = frame_limit {
        quit_after_frames(&mut app, frames);
    }
    app.run();
}

/// Makes the game quit on its own after `frames` frames, reporting what it drew
/// on the way out -- what `--frames` does, and the whole of it.
///
/// ⚠️ Exists because the windowed path cannot be tested in-process. winit
/// refuses to build an event loop off the main thread, and libtest runs every
/// test on a worker thread, so a `#[test]` that called
/// [`winit_runner`](bsengine_window::winit_runner) would panic before opening
/// anything. `window_smoke.rs` therefore runs this binary as a subprocess --
/// which is the better test anyway, since it covers `main`'s own argument
/// handling and the executable a player actually launches.
///
/// ⚠️ Reports draw calls, not just a frame count. A window that opens and
/// presents nothing -- a surface that failed to configure, a renderer that
/// found no camera -- still ticks `Update` happily, and a frame count alone
/// would call that a success.
fn quit_after_frames(app: &mut bevy_app::App, frames: u32) {
    app.add_event::<bevy_app::AppExit>();

    let mut seen = 0u32;
    let mut drawn = 0u32;
    app.add_systems(
        bevy_app::Last,
        move |surface: Option<bevy_ecs::prelude::Res<bsengine_rhi_wgpu::WgpuSurfaceResource>>,
              mut exit: bevy_ecs::prelude::EventWriter<bevy_app::AppExit>| {
            seen += 1;
            if let Some(stats) = surface.and_then(|s| s.0.latest_frame_stats()) {
                drawn = drawn.max(stats.draw_calls);
            }
            if seen >= frames {
                // The line `window_smoke.rs` parses. Printed once, on the way
                // out, so a truncated run cannot look like a complete one.
                println!("frames={seen} draw_calls={drawn}");
                exit.send(bevy_app::AppExit::Success);
            }
        },
    );
}

/// Builds the app a player runs: every plugin, the project's manifest, its
/// entry scene, and `Playing` already set.
///
/// Stops short of `run()` so [`run_windowed`] can add the `--frames` limit
/// before the event loop takes the app.
fn build_windowed_app(project_dir: &str) -> bevy_app::App {
    // The archive before the manifest: a single-file build keeps its manifest
    // inside the archive, so there is nothing to read until that is open.
    let pak = open_pak(project_dir);
    let manifest = read_manifest(project_dir, pak.as_deref());
    // The log file and the crash handler, before `new_app()` below sets up
    // stderr-only logging (the first set-up wins) and before anything else
    // can panic -- a bad `[input]` binding, a scene that will not load --
    // so those land in a report too. Named for the project, which is why it
    // cannot come any earlier than the manifest.
    bsengine_core::crash::init_for_project(&manifest.project.name);

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
    app.insert_resource(bsengine_core::ShadowSettings {
        distance: manifest.render.shadow_distance,
        cascades: manifest.render.shadow_cascades,
        blend: manifest.render.shadow_cascade_blend,
    });
    app.insert_resource(bsengine_core::TextureStreamingSettings::from_manifest(
        manifest.render.texture_streaming_budget_mb,
        manifest.render.texture_mip_bias,
    ));
    // Before `AssetPlugin`, and that ordering is the whole reason this is a
    // separate plugin: `bevy_asset` builds its sources during that plugin's
    // `build`, so a source registered afterwards is silently ignored -- and a
    // silently ignored pak source means a packaged build quietly reading loose
    // files instead of its own archive.
    if let Some(pak) = pak {
        crate::install_shipped_mips(&mut app, &pak);
        app.add_plugins(bsengine_asset::PakAssetPlugin {
            pak,
            project_dir: project_dir.to_string(),
        });
    }
    insert_input_actions(&mut app, &manifest);
    // From `project.toml`'s `[network]` table. Inserted before the plugins so
    // `NetworkPlugin`'s `init_resource` finds it already present and leaves it
    // alone -- registering it afterwards would overwrite the project's settings
    // with defaults, and silently, since every default is a working value.
    app.insert_resource(bsengine_network::NetworkConfig {
        aoi_radius: manifest.network.aoi_radius,
        interpolation_delay_ticks: manifest.network.interpolation_delay_ticks,
        simulated_latency_frames: manifest.network.simulated_latency_frames,
        simulated_loss: manifest.network.simulated_loss,
        simulator_seed: manifest.network.simulator_seed,
        rpc_resend_frames: manifest.network.rpc_resend_frames,
    });
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
        .add_plugins(bsengine_app::TimelinePlugin)
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
        // Generates each scene-authored `Cloth`'s sheet mesh and steps it every
        // frame. In both hosts from the start, for the reason `TerrainPlugin`'s
        // comment above records the hard way: a component whose plugin is
        // missing from the runtime is a feature that passes its own tests and
        // does nothing in a real game.
        .add_plugins(ClothPlugin)
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

    app
}

/// Gives the texture registry the archive's precooked mip cache files --
/// what `--package` made for each block-compressed texture
/// (`bsengine_rhi_wgpu::precook_mip_cache`) -- so a pak or single-file build
/// encodes nothing on the player's machine. A loose package needs no help:
/// the registry reads its `SHIPPED_MIP_DIR` directory by itself.
fn install_shipped_mips(app: &mut bevy_app::App, pak: &std::sync::Arc<bsengine_asset::pak::Pak>) {
    let pak = std::sync::Arc::clone(pak);
    app.insert_resource(bsengine_rhi_wgpu::ShippedMipCacheResource(
        std::sync::Arc::new(move |name: &str| {
            pak.get(&format!("{}/{name}", bsengine_rhi_wgpu::SHIPPED_MIP_DIR))
                .map(<[u8]>::to_vec)
        }),
    ));
}

#[cfg(test)]
mod tests {
    /// The packager writes precooked files where the runtime reads them.
    /// The two crates cannot share the constant (the asset crate must not
    /// depend on the GPU crate), so this pins them together: a rename on
    /// one side alone would ship every file where no runtime looks.
    #[test]
    fn the_packager_ships_mips_where_the_runtime_looks_for_them() {
        assert_eq!(
            bsengine_asset::cook::PRECOOKED_MIP_DIR,
            bsengine_rhi_wgpu::SHIPPED_MIP_DIR
        );
    }

    /// The archive lookup `install_shipped_mips` gives the registry finds a
    /// precooked file by its bare name -- the name the registry asks for --
    /// and nothing that is not there.
    #[test]
    fn the_archive_lookup_finds_a_shipped_file_by_its_name() {
        let path = std::env::temp_dir().join(format!("bse-shipped-{}.pak", std::process::id()));
        bsengine_asset::pak::write_pak(
            &path,
            &[(
                format!("{}/abc.mips", bsengine_rhi_wgpu::SHIPPED_MIP_DIR),
                b"blocks".to_vec(),
            )],
        )
        .unwrap();
        let pak = std::sync::Arc::new(bsengine_asset::pak::Pak::open(&path).unwrap());
        let mut app = bevy_app::App::new();
        super::install_shipped_mips(&mut app, &pak);
        let lookup = app
            .world()
            .resource::<bsengine_rhi_wgpu::ShippedMipCacheResource>()
            .0
            .clone();
        assert_eq!(lookup("abc.mips").as_deref(), Some(&b"blocks"[..]));
        assert_eq!(lookup("other.mips"), None);
        let _ = std::fs::remove_file(&path);
    }
}

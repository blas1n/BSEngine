//! The browser build's entry point.
//!
//! A page loads the runtime's wasm module, which fetches the game's archive --
//! a `--package <project> --mode pak` `game.pak`, which carries the manifest,
//! the scenes, the scripts and the cooked assets -- and runs the same player
//! app a desktop build runs (`build_player_app`), drawing into a canvas.
//!
//! The archive's URL is the page's `?pak=` query parameter, `game.pak`
//! (beside the page) by default.

use std::sync::Arc;

use wasm_bindgen::{JsCast, JsValue};

/// The project directory the archive is installed under. Any name serves --
/// a pak's entries are project-relative, and every path the engine builds
/// starts from this -- so it is a fixed one, not a directory that exists.
const PROJECT_DIR: &str = "game";

/// Starts the game: logging to the console, then (asynchronously, as a page
/// has to) the archive, then the app.
pub fn start() {
    console_error_panic_hook::set_once();
    // The console, which is where a browser build's log goes; `new_app()`
    // calls it again later, which is a no-op.
    bsengine_core::logging::init_logging();
    drop_removed_webgpu_limits();
    resume_audio_on_first_input();
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = run().await {
            tracing::error!("the game could not start: {e}");
            report(&format!("error: {e}"));
        }
    });
}

async fn run() -> Result<(), String> {
    let url = pak_url();
    tracing::info!("fetching {url}");
    let bytes = fetch(&url).await?;
    let pak = bsengine_asset::pak::Pak::from_bytes(bytes)
        .map_err(|e| format!("{url} is not a game archive: {e}"))?;
    let pak = Arc::new(pak);
    bsengine_asset::pak_source::install(pak.clone(), PROJECT_DIR);
    let mut app = crate::build_player_app(PROJECT_DIR, Some(pak));
    report_frames(&mut app);
    app.run();
    Ok(())
}

/// Makes the browser's `requestDevice` ignore the device limits WebGPU has
/// since removed from the specification.
///
/// wgpu 22 sends every limit it knows, `maxInterStageShaderComponents`
/// included, and current browsers reject a request naming a limit they do not
/// recognise ("The limit ... is not recognized"), so no device is ever made.
/// The limit is gone from the spec (folded into `maxInterStageShaderVariables`)
/// and newer wgpu no longer sends it; until the engine moves to one, the
/// request is passed on without it. A browser that still knows the limit
/// would only have been told the default anyway.
fn drop_removed_webgpu_limits() {
    let _ = js_sys::eval(
        r#"(() => {
            if (typeof GPUAdapter === 'undefined' || GPUAdapter.prototype.__bsePatched) return;
            const request = GPUAdapter.prototype.requestDevice;
            GPUAdapter.prototype.requestDevice = function (descriptor) {
                if (descriptor && descriptor.requiredLimits) {
                    const limits = Object.assign({}, descriptor.requiredLimits);
                    delete limits.maxInterStageShaderComponents;
                    descriptor = Object.assign({}, descriptor, { requiredLimits: limits });
                }
                return request.call(this, descriptor);
            };
            GPUAdapter.prototype.__bsePatched = true;
        })()"#,
    );
}

/// Starts the game's sound at the player's first click, touch or key press.
///
/// A browser keeps every `AudioContext` a page makes suspended until the
/// person using it has interacted with the page ("The AudioContext was not
/// allowed to start"), so a game that plays sound from its first frame would
/// stay silent for good: the audio backend makes its context once, at
/// startup, and never asks again. Unity's and Godot's web players do the same
/// as this -- remember each context the page uses and resume them all on the
/// first input. Sound from before that moment is lost, as it is there.
///
/// The contexts are found through `AudioNode.prototype.connect`, which every
/// sound graph calls, rather than by replacing the `AudioContext` constructor:
/// wasm-bindgen's glue reads that constructor once, when the module loads --
/// before this runs -- so a replacement would never see a context made.
fn resume_audio_on_first_input() {
    let _ = js_sys::eval(
        r#"(() => {
            if (typeof AudioNode === 'undefined' || globalThis.__bseAudioContexts) return;
            const made = globalThis.__bseAudioContexts = new Set();
            const connect = AudioNode.prototype.connect;
            AudioNode.prototype.connect = function (...args) {
                made.add(this.context);
                return connect.apply(this, args);
            };
            const resume = () => {
                for (const ctx of made) if (ctx.state === 'suspended') ctx.resume();
                for (const e of ['pointerdown', 'keydown', 'touchend'])
                    removeEventListener(e, resume, true);
            };
            for (const e of ['pointerdown', 'keydown', 'touchend'])
                addEventListener(e, resume, true);
        })()"#,
    );
}

/// `?pak=<url>`, or `game.pak` beside the page.
fn pak_url() -> String {
    web_sys::window()
        .and_then(|w| w.location().search().ok())
        .and_then(|q| {
            web_sys::UrlSearchParams::new_with_str(&q)
                .ok()
                .and_then(|p| p.get("pak"))
        })
        .unwrap_or_else(|| "game.pak".to_string())
}

async fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no window")?;
    let response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(describe)?;
    let response: web_sys::Response = response.dyn_into().map_err(describe)?;
    if !response.ok() {
        return Err(format!("{url}: HTTP {}", response.status()));
    }
    let buffer = wasm_bindgen_futures::JsFuture::from(response.array_buffer().map_err(describe)?)
        .await
        .map_err(describe)?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

fn describe(e: JsValue) -> String {
    e.as_string().unwrap_or_else(|| format!("{e:?}"))
}

/// Writes `text` to the page's `#bsengine-status` element, if it has one:
/// how a page -- and the browser smoke test -- sees what the game is doing.
fn report(text: &str) {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("bsengine-status"))
    {
        el.set_text_content(Some(text));
    }
}

/// Reports frames drawn and the most draw calls in one, every frame -- the
/// browser's counterpart of the desktop runtime's `--frames` line. A canvas
/// that ticks and draws nothing still counts frames, which is why the draw
/// calls ride along.
fn report_frames(app: &mut bevy_app::App) {
    let mut frames = 0u32;
    let mut drawn = 0u32;
    app.add_systems(
        bevy_app::Last,
        move |surface: Option<bevy_ecs::prelude::Res<bsengine_rhi_wgpu::WgpuSurfaceResource>>| {
            frames += 1;
            if let Some(stats) = surface.and_then(|s| s.0.latest_frame_stats()) {
                drawn = drawn.max(stats.draw_calls);
            }
            report(&format!("frames={frames} draw_calls={drawn}"));
        },
    );
}

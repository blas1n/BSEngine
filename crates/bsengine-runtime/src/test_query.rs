//! Read-only state queries and mechanical assertion evaluation for the
//! headless test runtime. Queries are evaluated directly against the ECS
//! `World` — no scripting/V8 involvement — using the same shapes as the
//! `Bsengine.*` JS getters for consistency.

use base64::Engine;
use bevy_ecs::world::World;
use bsengine_asset::{AssetStatus, AssetStatuses};
use bsengine_core::{HudTexts, ProjectDir, Transform, Visible};
use bsengine_rhi_wgpu::WgpuSurfaceResource;
use bsengine_scene::Name;
use bsengine_scripting::ops::render_asset_status;
use serde_json::{json, Value};

pub fn get_transform(world: &mut World, name: &str) -> Value {
    let mut q = world.query::<(&Name, &Transform)>();
    for (n, t) in q.iter(world) {
        if n.0 == name {
            return json!({
                "x": t.position.0.x, "y": t.position.0.y, "z": t.position.0.z,
                "rx": t.rotation.0.x, "ry": t.rotation.0.y, "rz": t.rotation.0.z, "rw": t.rotation.0.w,
                "sx": t.scale.0.x, "sy": t.scale.0.y, "sz": t.scale.0.z,
            });
        }
    }
    Value::Null
}

pub fn get_visible(world: &mut World, name: &str) -> Value {
    let mut q = world.query::<(&Name, Option<&Visible>)>();
    for (n, v) in q.iter(world) {
        if n.0 == name {
            return json!(v.map(|v| v.is_visible).unwrap_or(true));
        }
    }
    json!(true)
}

pub fn get_entity_names(world: &mut World) -> Value {
    let mut q = world.query::<&Name>();
    let names: Vec<String> = q.iter(world).map(|n| n.0.clone()).collect();
    json!(names)
}

/// Reads a HUD text slot (as set by `Bsengine.setHudText(id, text)`) by its
/// string `id`. Returns `null` if that slot is unset — lets a replay assert
/// on-screen feedback (e.g. "Level Complete!") directly, instead of only
/// inferring it indirectly from entity positions.
pub fn get_hud_text(world: &mut World, id: &str) -> Value {
    match world.get_resource::<HudTexts>() {
        Some(hud) => match hud.0.get(id) {
            Some(text) => json!(text),
            None => Value::Null,
        },
        None => Value::Null,
    }
}

/// Reads what the engine knows about one asset path, as the same string
/// `Bsengine.getAssetStatus` hands a script: `"loaded"`, `"loading"`,
/// `"failed: <reason>"` or `"unknown"`. Rendered by
/// [`bsengine_scripting::ops::render_asset_status`], so the MCP tool that
/// wraps this query and the JS op cannot drift into two vocabularies for the
/// same fact.
///
/// `path` may be spelled either way round:
///
/// * **Project-relative** — `"assets/sounds/hit.wav"`, the same string a script
///   hands `Bsengine.playSound`. Preferred, and what the MCP tool documents.
/// * **The exact key** — `"<project_dir>/assets/sounds/hit.wav"`, with
///   `<project_dir>` byte-for-byte as it was passed to `--test`.
///
/// # Why it has to resolve and not merely look up
///
/// `AssetStatuses` is keyed by whatever `bsengine_core::resolve_project_path`
/// produced at the load site, which is `format!("{project_dir}/{path}")` —
/// verbatim, with no separator normalisation. The MCP server passes `--test`
/// an *absolute* project directory (`SessionRegistry` joins its games root,
/// itself built from the server's root argument, onto the game name), so the
/// live key for mini-arena's fox is
/// `f:\Works\BSEngine\games\mini-arena/assets/models/fox.glb` — a spelling no
/// caller can reasonably be asked to reconstruct, and one that mixes
/// separators. A lookup-only query answered `"unknown"` for the documented
/// spelling, i.e. "nothing ever requested that path", which is the single
/// answer that must never be wrong: it is the whole distinction this API adds
/// over the `tracing::warn!` it replaces.
///
/// Resolution is tried only *after* an exact-key miss, so the exact key still
/// answers and a project-relative path can never shadow one.
///
/// Answers `"unknown"` — never an error — when `AssetStatuses` is absent
/// entirely, matching what the JS op does in a host that never added
/// `AssetStatusPlugin`. Unreachable in this runtime, whose `--test` app adds
/// that plugin (see `test_mode::build_test_app`); the arm exists so a caller
/// gets the same four answers from any app rather than a fifth failure mode
/// that depends on host wiring.
pub fn get_asset_status(world: &mut World, path: &str) -> Value {
    // Read before `statuses` is taken, and owned rather than borrowed: both
    // are `&World` reborrows of the same `&mut World`, and `resolve_project_path`
    // returning a `String` is what ends the first one.
    let resolved = bsengine_core::resolve_project_path(world.get_resource::<ProjectDir>(), path);

    let status = match world.get_resource::<AssetStatuses>() {
        Some(statuses) => match statuses.get(path) {
            AssetStatus::Unknown => statuses.get(&resolved),
            known => known,
        },
        None => AssetStatus::Unknown,
    };
    json!(render_asset_status(&status))
}

/// Every asset path this session's engine has something to say about, sorted.
///
/// # Why this exists next to [`get_asset_status`]
///
/// `"unknown"` is the one answer that is indistinguishable from a typo. It is
/// also the answer a caller gets for a path this app genuinely never requested.
/// `--test` mode carries `RenderPlugin`/`GltfPlugin` (see
/// `test_mode::build_test_app`), so meshes, shaders and textures are asked
/// for exactly as they are in the windowed runtime; "unknown" here means
/// what it says.
/// Handing back the keys the engine *does* hold turns "I cannot tell which of
/// those two happened" into a fact the caller can read, without a second
/// guess at the spelling.
///
/// Deliberately a plain array of paths, not a map of path → status: a caller
/// that wants a status asks [`get_asset_status`] about one path, and a shape
/// that carried statuses would be a second, wider vocabulary for the same fact.
pub fn get_known_asset_paths(world: &mut World) -> Value {
    let mut paths: Vec<&str> = match world.get_resource::<AssetStatuses>() {
        Some(statuses) => statuses.iter().map(|(path, _)| path).collect(),
        None => Vec::new(),
    };
    paths.sort_unstable();
    json!(paths)
}

/// Reads one pixel from the most recently rendered frame, as sRGB-encoded
/// RGBA plus a perceptual brightness (`luma`) computed the same way
/// `bsengine-rhi-wgpu`'s pixel test harness does. Errors when no renderer is
/// attached (no `WgpuSurfaceResource` -- a host with no `WgpuRHIPlugin`, or
/// one still waiting on a `WindowHandle`) or when `x`/`y` fall outside the
/// frame.
pub fn get_pixel(world: &mut World, x: u32, y: u32) -> Result<Value, String> {
    let surface = world
        .get_resource::<WgpuSurfaceResource>()
        .ok_or_else(|| "no renderer attached: get_pixel needs a WgpuSurfaceResource".to_string())?;
    let width = surface.0.width();
    let height = surface.0.height();
    if x >= width || y >= height {
        return Err(format!(
            "pixel ({x}, {y}) is outside the {width}x{height} frame"
        ));
    }
    let data = surface.0.read_pixels()?;
    let i = ((y * width + x) * 4) as usize;
    let (r, g, b, a) = (data[i], data[i + 1], data[i + 2], data[i + 3]);
    let luma = 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32;
    Ok(json!({ "r": r, "g": g, "b": b, "a": a, "luma": luma }))
}

/// Encodes the most recently rendered frame as a PNG, base64-encoded so it
/// travels as one JSON string field. Errors when no renderer is attached (no
/// `WgpuSurfaceResource`), when the buffer `read_pixels()` returns doesn't
/// match the surface's own width/height (shouldn't happen in practice --
/// `read_pixels` always returns exactly `width*height*4` bytes, but this
/// turns a future contract violation into a clean error instead of a panic
/// in `RgbaImage::from_raw`), or if PNG encoding itself fails.
pub fn screenshot(world: &mut World) -> Result<Value, String> {
    let surface = world.get_resource::<WgpuSurfaceResource>().ok_or_else(|| {
        "no renderer attached: screenshot needs a WgpuSurfaceResource".to_string()
    })?;
    let width = surface.0.width();
    let height = surface.0.height();
    let data = surface.0.read_pixels()?;
    let image = image::RgbaImage::from_raw(width, height, data)
        .ok_or_else(|| "read_pixels returned a buffer the wrong size for the frame".to_string())?;
    let mut png_bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    let data_base64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
    Ok(json!({
        "width": width,
        "height": height,
        "format": "png",
        "data_base64": data_base64,
    }))
}

/// Returns the most recently completed frame's profiling data (CPU/GPU
/// timing, draw calls, triangles, texture memory) as reported by
/// `WgpuSurface`'s rolling frame-stats history. Errors when no renderer is
/// attached, or when no frame has rendered yet (an empty history -- shouldn't
/// happen once the test app has rendered at least one frame, but turns a
/// would-be `unwrap` panic into a clean error).
pub fn get_frame_stats(world: &mut World) -> Result<Value, String> {
    let surface = world.get_resource::<WgpuSurfaceResource>().ok_or_else(|| {
        "no renderer attached: get_frame_stats needs a WgpuSurfaceResource".to_string()
    })?;
    let stats = surface
        .0
        .latest_frame_stats()
        .ok_or_else(|| "no frame has rendered yet".to_string())?;
    Ok(json!({
        "cpu_frame_time_ms": stats.cpu_frame_time_ms,
        "gpu_timestamps_supported": stats.gpu_timestamps_supported,
        "gpu_pass_times_ms": stats.gpu_pass_times_ms.iter().map(|p| json!({
            "name": p.name,
            "duration_ms": p.duration_ms,
        })).collect::<Vec<_>>(),
        "draw_calls": stats.draw_calls,
        "triangles": stats.triangles,
        "texture_memory_bytes": stats.texture_memory_bytes,
        "texture_count": stats.texture_count,
    }))
}

pub fn run_query(world: &mut World, tool: &str, args: &Value) -> Result<Value, String> {
    match tool {
        "get_transform" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "get_transform requires string 'name'".to_string())?;
            Ok(get_transform(world, name))
        }
        "get_visible" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "get_visible requires string 'name'".to_string())?;
            Ok(get_visible(world, name))
        }
        "get_entity_names" => Ok(get_entity_names(world)),
        "get_hud_text" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "get_hud_text requires string 'id'".to_string())?;
            Ok(get_hud_text(world, id))
        }
        "get_asset_status" => {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "get_asset_status requires string 'path'".to_string())?;
            Ok(get_asset_status(world, path))
        }
        "get_known_asset_paths" => Ok(get_known_asset_paths(world)),
        "get_pixel" => {
            let x = args
                .get("x")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| "get_pixel requires integer 'x'".to_string())?
                as u32;
            let y = args
                .get("y")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| "get_pixel requires integer 'y'".to_string())?
                as u32;
            get_pixel(world, x, y)
        }
        "screenshot" => screenshot(world),
        "get_frame_stats" => get_frame_stats(world),
        other => Err(format!("unknown query tool: {other}")),
    }
}

pub fn eval_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }
    let mut current = value;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

pub fn eval_op(actual: &Value, op: &str, expected: &Value) -> Result<bool, String> {
    match op {
        "exists" => Ok(!actual.is_null()),
        "==" => Ok(actual == expected),
        "!=" => Ok(actual != expected),
        ">" | ">=" | "<" | "<=" => {
            let a = actual
                .as_f64()
                .ok_or_else(|| format!("actual value {actual} is not numeric"))?;
            let e = expected
                .as_f64()
                .ok_or_else(|| format!("expected value {expected} is not numeric"))?;
            Ok(match op {
                ">" => a > e,
                ">=" => a >= e,
                "<" => a < e,
                "<=" => a <= e,
                _ => unreachable!(),
            })
        }
        other => Err(format!("unknown operator: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn get_transform_returns_null_when_entity_missing() {
        let mut world = World::new();
        assert_eq!(get_transform(&mut world, "Ghost"), Value::Null);
    }

    #[test]
    fn get_transform_returns_position_for_named_entity() {
        let mut world = World::new();
        world.spawn((
            Name("Player".to_string()),
            Transform::from_position(Vec3::new(1.0, 2.0, 3.0)),
        ));
        let result = get_transform(&mut world, "Player");
        assert_eq!(result["x"], json!(1.0));
        assert_eq!(result["y"], json!(2.0));
        assert_eq!(result["z"], json!(3.0));
    }

    #[test]
    fn get_visible_defaults_true_when_no_visible_component() {
        let mut world = World::new();
        world.spawn((Name("Player".to_string()), Transform::default()));
        assert_eq!(get_visible(&mut world, "Player"), json!(true));
    }

    #[test]
    fn get_visible_reflects_visible_component() {
        let mut world = World::new();
        world.spawn((
            Name("Player".to_string()),
            Transform::default(),
            Visible { is_visible: false },
        ));
        assert_eq!(get_visible(&mut world, "Player"), json!(false));
    }

    #[test]
    fn get_hud_text_returns_null_when_slot_unset() {
        let mut world = World::new();
        world.insert_resource(HudTexts::default());
        assert_eq!(get_hud_text(&mut world, "1"), Value::Null);
    }

    #[test]
    fn get_hud_text_returns_null_when_resource_missing() {
        let mut world = World::new();
        assert_eq!(get_hud_text(&mut world, "1"), Value::Null);
    }

    #[test]
    fn get_hud_text_returns_set_text() {
        let mut world = World::new();
        let mut hud = HudTexts::default();
        hud.0.insert("1".to_string(), "Level Complete!".to_string());
        world.insert_resource(hud);
        assert_eq!(get_hud_text(&mut world, "1"), json!("Level Complete!"));
    }

    #[test]
    fn run_query_dispatches_get_hud_text() {
        let mut world = World::new();
        let mut hud = HudTexts::default();
        hud.0.insert("0".to_string(), "Fell! Retry".to_string());
        world.insert_resource(hud);
        let result = run_query(&mut world, "get_hud_text", &json!({"id": "0"})).unwrap();
        assert_eq!(result, json!("Fell! Retry"));
    }

    #[test]
    fn get_entity_names_lists_all_named_entities() {
        let mut world = World::new();
        world.spawn((Name("A".to_string()), Transform::default()));
        world.spawn((Name("B".to_string()), Transform::default()));
        let result = get_entity_names(&mut world);
        let names: Vec<String> =
            serde_json::from_value(result).expect("should deserialize as Vec<String>");
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"A".to_string()));
        assert!(names.contains(&"B".to_string()));
    }

    // Only the `Unknown` direction is reachable from a hand-built `World`:
    // `AssetStatuses`' map is private and written by nothing but the real
    // collector, deliberately, so that no caller can forge a verdict. The
    // `Failed`/`Loaded` directions are therefore pinned where they can be
    // driven for real — `crates/bsengine-mcp/tests/test_tools.rs`, against a
    // live session with a load that actually fails.
    #[test]
    fn get_asset_status_is_unknown_when_the_status_resource_is_absent() {
        let mut world = World::new();
        assert_eq!(
            get_asset_status(&mut world, "games/x/assets/sounds/a.ogg"),
            json!("unknown"),
            "a host without AssetStatusPlugin must answer like the JS op does, \
             not invent a fifth answer"
        );
    }

    #[test]
    fn run_query_dispatches_get_asset_status() {
        let mut world = World::new();
        let result = run_query(
            &mut world,
            "get_asset_status",
            &json!({"path": "games/x/assets/sounds/a.ogg"}),
        )
        .unwrap();
        assert_eq!(result, json!("unknown"));
    }

    // A `ProjectDir` in the world must not turn an unrequested path into some
    // other answer: resolution is a second *lookup*, not a second verdict.
    #[test]
    fn get_asset_status_resolution_never_invents_an_answer() {
        let mut world = World::new();
        world.insert_resource(ProjectDir("games/x".to_string()));
        assert_eq!(
            get_asset_status(&mut world, "assets/sounds/a.ogg"),
            json!("unknown"),
            "resolving a path is a lookup under a second spelling, not a claim \
             that anything requested it"
        );
    }

    #[test]
    fn get_known_asset_paths_is_empty_when_the_status_resource_is_absent() {
        let mut world = World::new();
        assert_eq!(
            get_known_asset_paths(&mut world),
            json!([]),
            "a host without AssetStatusPlugin knows no paths; it must say so \
             rather than fail"
        );
    }

    #[test]
    fn run_query_dispatches_get_known_asset_paths() {
        let mut world = World::new();
        let result = run_query(&mut world, "get_known_asset_paths", &json!({})).unwrap();
        assert_eq!(result, json!([]));
    }

    #[test]
    fn run_query_get_asset_status_requires_a_path() {
        let mut world = World::new();
        let err = run_query(&mut world, "get_asset_status", &json!({})).unwrap_err();
        assert!(err.contains("path"), "unhelpful error: {err}");
    }

    #[test]
    fn run_query_dispatches_get_transform() {
        let mut world = World::new();
        world.spawn((
            Name("Player".to_string()),
            Transform::from_position(Vec3::new(0.0, 0.0, 5.0)),
        ));
        let result = run_query(&mut world, "get_transform", &json!({"name": "Player"})).unwrap();
        assert_eq!(result["z"], json!(5.0));
    }

    #[test]
    fn run_query_unknown_tool_errors() {
        let mut world = World::new();
        assert!(run_query(&mut world, "nope", &json!({})).is_err());
    }

    #[test]
    fn eval_path_extracts_nested_field() {
        let v = json!({"x": 1, "nested": {"y": 2}});
        assert_eq!(eval_path(&v, "x"), Some(&json!(1)));
        assert_eq!(eval_path(&v, "nested.y"), Some(&json!(2)));
    }

    #[test]
    fn eval_path_empty_returns_whole_value() {
        let v = json!({"x": 1});
        assert_eq!(eval_path(&v, ""), Some(&v));
    }

    #[test]
    fn eval_op_greater_than_numeric() {
        assert!(eval_op(&json!(5), ">", &json!(3)).unwrap());
        assert!(!eval_op(&json!(2), ">", &json!(3)).unwrap());
    }

    #[test]
    fn eval_op_exists_checks_non_null() {
        assert!(!eval_op(&json!(null), "exists", &Value::Null).unwrap());
        assert!(eval_op(&json!(1), "exists", &Value::Null).unwrap());
    }

    #[test]
    fn eval_op_unknown_operator_errors() {
        assert!(eval_op(&json!(1), "~=", &json!(1)).is_err());
    }

    #[test]
    fn run_query_get_pixel_errors_when_no_renderer_is_attached() {
        let mut world = World::new();
        let err = run_query(&mut world, "get_pixel", &json!({"x": 0, "y": 0})).unwrap_err();
        assert!(err.contains("renderer"), "unhelpful error: {err}");
    }

    #[test]
    fn run_query_screenshot_errors_when_no_renderer_is_attached() {
        let mut world = World::new();
        let err = run_query(&mut world, "screenshot", &json!({})).unwrap_err();
        assert!(err.contains("renderer"), "unhelpful error: {err}");
    }

    // Unlike `get_transform`/`get_hud_text`/etc., a `WgpuSurfaceResource` with
    // an actually-rendered frame can't be hand-built from a bare `World` --
    // it needs the real `WgpuRHIPlugin`/`RenderPlugin` stack. Mirrors
    // `test_mode`'s own `get_pixel_reads_the_last_rendered_frame`/
    // `screenshot_returns_a_decodable_png` tests: build the real headless app
    // via `build_test_app` and step it once so `WgpuSurface::render_frame`
    // has actually run before `latest_frame_stats()` is asked for anything.
    #[test]
    fn run_query_dispatches_get_frame_stats() {
        let project_dir = format!("{}/../../games/cube-evader", env!("CARGO_MANIFEST_DIR"));
        let mut app = crate::test_mode::build_test_app(&project_dir, None, false);
        app.update();

        let result = run_query(app.world_mut(), "get_frame_stats", &json!({}))
            .expect("get_frame_stats should succeed once RenderPlugin has drawn a frame");
        assert!(result.get("cpu_frame_time_ms").is_some());
        assert!(result.get("draw_calls").is_some());
        assert!(result.get("texture_memory_bytes").is_some());
    }

    #[test]
    fn run_query_get_frame_stats_errors_when_no_renderer_is_attached() {
        let mut world = World::new();
        let err = run_query(&mut world, "get_frame_stats", &json!({})).unwrap_err();
        assert!(err.contains("no renderer attached"), "unhelpful error: {err}");
    }
}

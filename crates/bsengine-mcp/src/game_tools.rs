use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::tool::{McpTool, McpToolOutput};

const SCENE_FORMAT_DOCS: &str = r#"Scene file format (RON — Rusty Object Notation):

SceneDescriptor(entities: [
  EntityDescriptor(
    name: "Camera",            // required — identifies entity for JS getTransform/setTransform
    camera: true,              // marks as main camera
    transform: Some((
      position: (0.0, 8.0, 12.0),    // x y z world position
      rotation:    (0.0, 0.0, 0.0, 1.0), // quaternion xyzw (optional, default identity)
      scale:       (1.0, 1.0, 1.0),      // optional, default 1 1 1
    )),
    look_at: Some((0.0, 0.0, 0.0)),      // optional: auto-aim camera at this world point
                                          // overrides rotation when set; useful for top-down/orbital cameras
  ),
  EntityDescriptor(
    name: "Sun",
    directional_light: Some((
      direction: (-0.4, -0.8, -0.4),  // normalized direction
      color:     (1.0, 1.0, 1.0),     // optional, default white
      ambient:   (0.1, 0.1, 0.1),     // optional, default 0.1
    )),
  ),
  EntityDescriptor(
    name: "Player",
    primitive: Some(Cube),     // available primitives: Cube only
    transform: Some((position: (0.0, 0.5, 0.0))),
    color: Some((1.0, 0.2, 0.2)),    // optional: albedo/base color [r, g, b] linear 0–1
                                      // multiplies vertex color and texture; default white
    emissive: Some((0.0, 0.0, 0.0)), // optional: self-illumination color; default black (none)
    script: Some("assets/scripts/player.js"),  // relative to game root
  ),
])

Rules:
- Always include a Camera entity (camera: true) for rendering
- Always include a Sun entity (directional_light) or scene will be unlit
- primitive: Some(Cube) renders a white cube; use color to tint it
- look_at on a camera entity auto-computes rotation to face the target point
- color sets the albedo/surface color; emissive makes the entity glow
- name is the key used by JS Bsengine.getTransform/setTransform"#;

const SCRIPT_API_DOCS: &str = r#"BSEngine JavaScript API (runs in V8 via Deno Core):

Transform:
  Bsengine.getTransform(name: string) → { x, y, z } | null
    Get world position of an entity by name. Returns null if not found.

  Bsengine.setTransform(name: string, x: number, y: number, z: number)
    Set world position of an entity by name.

Input:
  Bsengine.isKeyPressed(key: string) → boolean
    Check if a key is held. Available keys:
    "W" "A" "S" "D" "Space" "Enter" "Escape" "Up" "Down" "Left" "Right"

Material:
  Bsengine.setEmissive(name: string, r: number, g: number, b: number)
    Set the emissive (glow) color of an entity at runtime. Values 0–1 linear.

  Bsengine.setColor(name: string, r: number, g: number, b: number)
    Set the albedo/base color of an entity at runtime. Values 0–1 linear.

Scene:
  Bsengine.getEntityNames() → string[]
    Returns names of all entities currently in the scene.

Logging:
  Bsengine.log(message: string)
    Print a message to the engine log (tracing INFO).

Entry point — called every frame with the name of the entity this script is attached to:
  function onUpdate(self) { ... }

Each entity's script runs independently. Use `self` to reference the owning entity.

Example (WASD movement on the entity this script is attached to):
  const SPEED = 0.05;
  function onUpdate(self) {
    const t = Bsengine.getTransform(self);
    if (!t) return;
    let { x, y, z } = t;
    if (Bsengine.isKeyPressed("W")) z -= SPEED;
    if (Bsengine.isKeyPressed("S")) z += SPEED;
    if (Bsengine.isKeyPressed("A")) x -= SPEED;
    if (Bsengine.isKeyPressed("D")) x += SPEED;
    Bsengine.setTransform(self, x, y, z);
  }

Example (flash red when near origin):
  function onUpdate(self) {
    const t = Bsengine.getTransform(self);
    if (!t) return;
    const dist = Math.sqrt(t.x * t.x + t.z * t.z);
    Bsengine.setEmissive(self, dist < 2.0 ? 1.0 : 0.0, 0.0, 0.0);
  }

Example (controlling another entity by name from this script):
  function onUpdate(self) {
    const enemy = Bsengine.getTransform("Enemy");
    if (enemy) Bsengine.setTransform("Enemy", enemy.x + 0.01, enemy.y, enemy.z);
  }

Notes:
- Scripts load once at startup; onUpdate(self) runs every frame (~60fps)
- Each entity's script is isolated — multiple entities can each have their own script
- path is relative to game root (e.g. "assets/scripts/player.js")"#;

/// Builds the `game_create`/`scene_write`/`script_write`/`game_validate` tools,
/// each scoped to game projects under `root/games/`.
pub fn game_tools(root: PathBuf) -> Vec<McpTool> {
    let r1 = root.clone();
    let r2 = root.clone();
    let r3 = root.clone();
    let r4 = root.clone();

    vec![
        McpTool {
            name: "game_create".to_string(),
            description: format!(
                "Create a new BSEngine game project at games/<name>/.\n\n\
                Creates:\n\
                  games/<name>/project.toml         — project manifest\n\
                  games/<name>/assets/scenes/       — scene files directory\n\
                  games/<name>/assets/scripts/      — JS script files directory\n\n\
                After creating, use scene_write to define entities and script_write to add behavior.\n\n\
                {SCENE_FORMAT_DOCS}\n\n\
                {SCRIPT_API_DOCS}"
            ),
            input_schema: Some(json!({
                "type": "object",
                "properties": {
                    "name":   { "type": "string", "description": "Game folder name (no spaces, e.g. 'my-game')" },
                    "title":  { "type": "string", "description": "Window title shown to the player" },
                    "width":  { "type": "integer", "description": "Window width in pixels", "default": 1280 },
                    "height": { "type": "integer", "description": "Window height in pixels", "default": 720 },
                },
                "required": ["name", "title"],
            })),
            handler: Box::new(move |args| game_create(&r1, args)),
        },
        McpTool {
            name: "scene_write".to_string(),
            description: format!(
                "Write the main scene file (assets/scenes/main.ron) for a BSEngine game.\n\n\
                {SCENE_FORMAT_DOCS}"
            ),
            input_schema: Some(json!({
                "type": "object",
                "properties": {
                    "game":    { "type": "string", "description": "Game folder name under games/" },
                    "content": { "type": "string", "description": "Full RON scene content (SceneDescriptor(...))" },
                },
                "required": ["game", "content"],
            })),
            handler: Box::new(move |args| scene_write(&r2, args)),
        },
        McpTool {
            name: "script_write".to_string(),
            description: format!(
                "Write a JavaScript script file for a BSEngine game entity.\n\n\
                {SCRIPT_API_DOCS}"
            ),
            input_schema: Some(json!({
                "type": "object",
                "properties": {
                    "game":    { "type": "string", "description": "Game folder name under games/" },
                    "path":    { "type": "string", "description": "Script path relative to game root (e.g. 'assets/scripts/player.js')" },
                    "content": { "type": "string", "description": "JavaScript source code" },
                },
                "required": ["game", "path", "content"],
            })),
            handler: Box::new(move |args| script_write(&r3, args)),
        },
        McpTool {
            name: "game_validate".to_string(),
            description: "Validate a BSEngine game project — checks that project.toml, scene file, \
                and all referenced scripts exist and are valid. Returns the command to run the game.\n\n\
                Run command: cargo run -p bsengine-runtime -- ./games/<name>".to_string(),
            input_schema: Some(json!({
                "type": "object",
                "properties": {
                    "game": { "type": "string", "description": "Game folder name under games/" },
                },
                "required": ["game"],
            })),
            handler: Box::new(move |args| game_validate(&r4, args)),
        },
    ]
}

fn get_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, McpToolOutput> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpToolOutput::error(&format!("missing required field: {key}")))
}

fn game_create(root: &Path, args: Value) -> McpToolOutput {
    let name = match get_str(&args, "name") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };
    let title = match get_str(&args, "title") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };
    let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(1280);
    let height = args.get("height").and_then(|v| v.as_u64()).unwrap_or(720);

    let game_dir = root.join("games").join(&name);

    for sub in &["assets/scenes", "assets/scripts"] {
        if let Err(e) = std::fs::create_dir_all(game_dir.join(sub)) {
            return McpToolOutput::error(&format!("failed to create {sub}: {e}"));
        }
    }

    let manifest = format!(
        "[project]\nname = \"{title}\"\nentry_scene = \"assets/scenes/main.ron\"\n\n\
         [window]\ntitle = \"{title}\"\nwidth = {width}\nheight = {height}\n"
    );

    if let Err(e) = std::fs::write(game_dir.join("project.toml"), &manifest) {
        return McpToolOutput::error(&format!("failed to write project.toml: {e}"));
    }

    McpToolOutput::success(json!({
        "created": format!("games/{name}/"),
        "next_steps": [
            format!("Use scene_write to create games/{name}/assets/scenes/main.ron"),
            "Use script_write to create JS scripts for entities",
            format!("Run: cargo run -p bsengine-runtime -- ./games/{name}"),
        ],
    }))
}

fn scene_write(root: &Path, args: Value) -> McpToolOutput {
    let game = match get_str(&args, "game") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };
    let content = match get_str(&args, "content") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };

    if let Err(e) = ron::from_str::<ron::Value>(&content) {
        return McpToolOutput::error(&format!("invalid RON: {e}"));
    }

    let path = root
        .join("games")
        .join(&game)
        .join("assets/scenes/main.ron");
    if let Err(e) = std::fs::write(&path, &content) {
        return McpToolOutput::error(&format!("failed to write scene: {e}"));
    }

    McpToolOutput::success(json!({ "written": format!("games/{game}/assets/scenes/main.ron") }))
}

fn script_write(root: &Path, args: Value) -> McpToolOutput {
    let game = match get_str(&args, "game") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };
    let rel_path = match get_str(&args, "path") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };
    let content = match get_str(&args, "content") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };

    let full_path = root.join("games").join(&game).join(&rel_path);
    if let Some(parent) = full_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return McpToolOutput::error(&format!("failed to create dirs: {e}"));
        }
    }

    if let Err(e) = std::fs::write(&full_path, &content) {
        return McpToolOutput::error(&format!("failed to write script: {e}"));
    }

    McpToolOutput::success(json!({ "written": format!("games/{game}/{rel_path}") }))
}

fn game_validate(root: &Path, args: Value) -> McpToolOutput {
    let game = match get_str(&args, "game") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };

    let game_dir = root.join("games").join(&game);

    let manifest_path = game_dir.join("project.toml");
    let manifest_str = match std::fs::read_to_string(&manifest_path) {
        Ok(s) => s,
        Err(_) => {
            return McpToolOutput::error(&format!(
                "games/{game}/project.toml not found — run game_create first"
            ))
        }
    };

    let manifest: toml::Value = match toml::from_str(&manifest_str) {
        Ok(v) => v,
        Err(e) => return McpToolOutput::error(&format!("project.toml parse error: {e}")),
    };

    let entry_scene = manifest
        .get("project")
        .and_then(|p| p.get("entry_scene"))
        .and_then(|v| v.as_str())
        .unwrap_or("assets/scenes/main.ron");

    let scene_path = game_dir.join(entry_scene);
    let scene_str = match std::fs::read_to_string(&scene_path) {
        Ok(s) => s,
        Err(_) => {
            return McpToolOutput::error(&format!(
                "{entry_scene} not found — use scene_write to create it"
            ))
        }
    };

    let scene = match ron::from_str::<ron::Value>(&scene_str) {
        Ok(v) => v,
        Err(e) => return McpToolOutput::error(&format!("scene parse error: {e}")),
    };

    // Check all script paths referenced in the scene exist.
    let mut missing_scripts: Vec<String> = Vec::new();
    let mut refs = Vec::new();
    collect_script_refs(&scene, &mut refs);
    for (entity, path) in refs {
        match path {
            Some(script_rel) if !game_dir.join(&script_rel).exists() => {
                missing_scripts.push(script_rel)
            }
            Some(_) => {}
            None => {
                return McpToolOutput::error(&format!(
                    "entity '{entity}' has a `script:` value that is neither a path string nor a \
                     (guid: \"…\", path: \"…\") pair"
                ))
            }
        }
    }

    if !missing_scripts.is_empty() {
        return McpToolOutput::error(&format!(
            "missing script files: {} — use script_write to create them",
            missing_scripts.join(", ")
        ));
    }

    McpToolOutput::success(json!({
        "valid": true,
        "run_command": format!("cargo run -p bsengine-runtime -- ./games/{game}"),
    }))
}

/// Collects every `script:` reference a parsed scene holds, as
/// `(owning entity's name, the path it names)`.
///
/// A `None` path means the value was a script reference this cannot read a
/// path out of; the caller reports that rather than skipping it, because
/// "found nothing to check" is the one answer a validator must never give
/// quietly.
///
/// # Why the scene is walked parsed rather than matched as text
///
/// This replaced `line.strip_prefix("script: Some(\"")`, which stopped
/// matching the moment roadmap item 30 gave a scene reference its second
/// spelling — `script: Some((guid: "…", path: "…"))`. Nothing about that
/// failure was visible: every migrated game would have gone on passing
/// `game_validate` with zero of its references checked, which is worse than a
/// validator that errors. Any replacement pattern would carry the same risk
/// forward, so the match is on structure instead: the scene is already parsed
/// above to confirm it is valid RON, and both spellings land in that value as
/// a string or as a map with a `path` key.
///
/// # Why not `bsengine_scene::AssetRef`
///
/// It is the type that actually defines these two spellings, and using it
/// would be the robust answer if the dependency were free. It is not:
/// `bsengine-scene` pulls in `bsengine-gltf`, and with it `bsengine-render`,
/// `bsengine-rhi-wgpu` and `wgpu` — the whole GPU stack — into
/// `bsengine-mcp-server`, a JSON-RPC binary that today needs none of it. The
/// cost of the coupling below is that a *third* spelling would need adding
/// here too; that is a smaller and much more visible cost than the build it
/// would otherwise take on.
///
/// Recurses the whole value rather than assuming `entities: [..]` at the top
/// level, so nesting a scene's entities later cannot silently empty this out.
fn collect_script_refs(value: &ron::Value, out: &mut Vec<(String, Option<String>)>) {
    match value {
        ron::Value::Map(map) => {
            let owner = map
                .iter()
                .find_map(|(k, v)| match (k, v) {
                    (ron::Value::String(k), ron::Value::String(name)) if k == "name" => {
                        Some(name.clone())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| "(unnamed)".to_string());
            for (key, val) in map.iter() {
                match key {
                    // An explicit `script: None` is "this entity has no
                    // script", not a reference that could not be read.
                    ron::Value::String(k) if k == "script" && *val == ron::Value::Option(None) => {}
                    // Not recursed into, so one reference is reported once.
                    ron::Value::String(k) if k == "script" => {
                        out.push((owner.clone(), asset_ref_path(val)))
                    }
                    _ => collect_script_refs(val, out),
                }
            }
        }
        ron::Value::Seq(items) => {
            for item in items {
                collect_script_refs(item, out);
            }
        }
        ron::Value::Option(Some(inner)) => collect_script_refs(inner, out),
        _ => {}
    }
}

/// The path out of an asset reference in either spelling: the bare
/// `"assets/scripts/player.js"` every pre-item-30 scene stores, or the
/// `(guid: "…", path: "…")` pair a migrated one does.
fn asset_ref_path(value: &ron::Value) -> Option<String> {
    match value {
        ron::Value::Option(Some(inner)) => asset_ref_path(inner),
        ron::Value::String(path) => Some(path.clone()),
        ron::Value::Map(map) => map.iter().find_map(|(k, v)| match (k, v) {
            (ron::Value::String(k), ron::Value::String(path)) if k == "path" => Some(path.clone()),
            _ => None,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn temp_root() -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        (dir, root)
    }

    #[test]
    fn game_create_makes_dirs_and_manifest() {
        let (_tmp, root) = temp_root();
        let tools = game_tools(root.clone());
        let create = tools.iter().find(|t| t.name == "game_create").unwrap();
        let out = (create.handler)(json!({"name": "test-game", "title": "Test Game"}));
        assert!(out.is_ok(), "error: {:?}", out.error);
        assert!(root.join("games/test-game/project.toml").exists());
        assert!(root.join("games/test-game/assets/scenes").exists());
        assert!(root.join("games/test-game/assets/scripts").exists());
    }

    #[test]
    fn scene_write_validates_ron_and_saves() {
        let (_tmp, root) = temp_root();
        std::fs::create_dir_all(root.join("games/test/assets/scenes")).unwrap();
        let tools = game_tools(root.clone());
        let sw = tools.iter().find(|t| t.name == "scene_write").unwrap();
        let out = (sw.handler)(json!({
            "game": "test",
            "content": "SceneDescriptor(entities: [])"
        }));
        assert!(out.is_ok(), "{:?}", out.error);
        assert!(root.join("games/test/assets/scenes/main.ron").exists());
    }

    #[test]
    fn scene_write_rejects_invalid_ron() {
        let (_tmp, root) = temp_root();
        std::fs::create_dir_all(root.join("games/test/assets/scenes")).unwrap();
        let tools = game_tools(root.clone());
        let sw = tools.iter().find(|t| t.name == "scene_write").unwrap();
        let out = (sw.handler)(json!({"game": "test", "content": "not ron {{{ "}));
        assert!(!out.is_ok());
    }

    #[test]
    fn script_write_creates_file() {
        let (_tmp, root) = temp_root();
        let tools = game_tools(root.clone());
        let sw = tools.iter().find(|t| t.name == "script_write").unwrap();
        let out = (sw.handler)(json!({
            "game": "g",
            "path": "assets/scripts/player.js",
            "content": "function onUpdate() {}"
        }));
        assert!(out.is_ok(), "{:?}", out.error);
        assert!(root.join("games/g/assets/scripts/player.js").exists());
    }

    #[test]
    fn game_validate_detects_missing_manifest() {
        let (_tmp, root) = temp_root();
        let tools = game_tools(root.clone());
        let gv = tools.iter().find(|t| t.name == "game_validate").unwrap();
        let out = (gv.handler)(json!({"game": "nonexistent"}));
        assert!(!out.is_ok());
        assert!(out.error.unwrap().contains("project.toml not found"));
    }

    #[test]
    fn game_validate_passes_valid_game() {
        let (_tmp, root) = temp_root();
        let tools = game_tools(root.clone());

        // Create game
        let create = tools.iter().find(|t| t.name == "game_create").unwrap();
        (create.handler)(json!({"name": "valid", "title": "Valid"}));

        // Write scene (no scripts)
        let sw = tools.iter().find(|t| t.name == "scene_write").unwrap();
        (sw.handler)(json!({"game": "valid", "content": "SceneDescriptor(entities: [])"}));

        let gv = tools.iter().find(|t| t.name == "game_validate").unwrap();
        let out = (gv.handler)(json!({"game": "valid"}));
        assert!(out.is_ok(), "{:?}", out.error);
        assert_eq!(out.content["valid"], true);
        assert!(out.content["run_command"]
            .as_str()
            .unwrap()
            .contains("valid"));
    }

    /// Writes a game whose entry scene holds exactly `scene`, then validates
    /// it.
    fn validate_game_with_scene(root: &Path, scene: &str) -> McpToolOutput {
        let tools = game_tools(root.to_path_buf());
        let create = tools.iter().find(|t| t.name == "game_create").unwrap();
        (create.handler)(json!({"name": "g", "title": "G"}));
        let sw = tools.iter().find(|t| t.name == "scene_write").unwrap();
        let written = (sw.handler)(json!({"game": "g", "content": scene}));
        assert!(written.is_ok(), "{:?}", written.error);
        let gv = tools.iter().find(|t| t.name == "game_validate").unwrap();
        (gv.handler)(json!({"game": "g"}))
    }

    // The regression roadmap item 30 sub-item B would otherwise have shipped
    // silently. `game_validate` used to find script references by matching the
    // raw text `script: Some("`, which a migrated reference —
    // `script: Some((guid: "…", path: "…"))` — never matches: it would find
    // zero references in this scene, check nothing, and report the game valid
    // while the script it names does not exist. A validator that quietly stops
    // validating is worse than one that fails, so this asserts the failure.
    #[test]
    fn game_validate_reports_a_missing_script_named_by_a_guid_pair() {
        let (_tmp, root) = temp_root();
        let out = validate_game_with_scene(
            &root,
            r#"SceneDescriptor(entities: [
                EntityDescriptor(
                    name: "Player",
                    script: Some((guid: "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19", path: "assets/scripts/nope.js")),
                ),
            ])"#,
        );
        assert!(
            !out.is_ok(),
            "a (guid, path) reference to a script that does not exist must fail validation"
        );
        let err = out.error.unwrap();
        assert!(err.contains("assets/scripts/nope.js"), "error was: {err}");
    }

    // The other half of the same guarantee: the bare spelling every
    // unmigrated scene still uses must keep being checked. Without this, a
    // fix aimed only at the new spelling could pass the test above while
    // silently dropping the old one.
    #[test]
    fn game_validate_reports_a_missing_script_named_by_a_bare_path() {
        let (_tmp, root) = temp_root();
        let out = validate_game_with_scene(
            &root,
            r#"SceneDescriptor(entities: [
                EntityDescriptor(name: "Player", script: Some("assets/scripts/nope.js")),
            ])"#,
        );
        assert!(!out.is_ok(), "bare-path references must still be checked");
        assert!(out.error.unwrap().contains("assets/scripts/nope.js"));
    }

    // Both spellings resolving to a script that exists must pass, or the two
    // tests above would also be satisfied by a validator that simply always
    // failed. Also pins that a `script: None` is not mistaken for a reference
    // whose path could not be read.
    #[test]
    fn game_validate_passes_when_both_spellings_name_scripts_that_exist() {
        let (_tmp, root) = temp_root();
        let tools = game_tools(root.clone());
        let create = tools.iter().find(|t| t.name == "game_create").unwrap();
        (create.handler)(json!({"name": "g", "title": "G"}));
        let script_write = tools.iter().find(|t| t.name == "script_write").unwrap();
        for name in ["bare.js", "paired.js"] {
            let out = (script_write.handler)(json!({
                "game": "g",
                "path": format!("assets/scripts/{name}"),
                "content": "function onUpdate() {}"
            }));
            assert!(out.is_ok(), "{:?}", out.error);
        }
        let out = validate_game_with_scene(
            &root,
            r#"SceneDescriptor(entities: [
                EntityDescriptor(name: "A", script: Some("assets/scripts/bare.js")),
                EntityDescriptor(
                    name: "B",
                    script: Some((guid: "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19", path: "assets/scripts/paired.js")),
                ),
                EntityDescriptor(name: "C", script: None),
                EntityDescriptor(name: "D"),
            ])"#,
        );
        assert!(out.is_ok(), "{:?}", out.error);
        assert_eq!(out.content["valid"], true);
    }

    // A `script:` value in neither spelling is reported rather than skipped —
    // silently ignoring it is the same class of failure as the text matcher
    // that stopped matching.
    #[test]
    fn game_validate_reports_a_script_reference_it_cannot_read_a_path_from() {
        let (_tmp, root) = temp_root();
        let out = validate_game_with_scene(
            &root,
            r#"SceneDescriptor(entities: [
                EntityDescriptor(name: "Player", script: Some((guid: "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19"))),
            ])"#,
        );
        assert!(!out.is_ok(), "a guid with no path must not be skipped");
        let err = out.error.unwrap();
        assert!(err.contains("Player"), "error was: {err}");
    }
}

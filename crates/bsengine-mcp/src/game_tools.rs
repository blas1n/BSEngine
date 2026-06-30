use std::path::PathBuf;

use serde_json::{json, Value};

use crate::tool::{McpTool, McpToolOutput};

const SCENE_FORMAT_DOCS: &str = r#"Scene file format (RON — Rusty Object Notation):

SceneDescriptor(entities: [
  EntityDescriptor(
    name: "Camera",            // required — identifies entity for JS getTransform/setTransform
    camera: true,              // marks as main camera
    transform: Some((
      translation: (0.0, 8.0, 12.0),    // x y z world position
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
    transform: Some((translation: (0.0, 0.5, 0.0))),
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

fn game_create(root: &PathBuf, args: Value) -> McpToolOutput {
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

fn scene_write(root: &PathBuf, args: Value) -> McpToolOutput {
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

fn script_write(root: &PathBuf, args: Value) -> McpToolOutput {
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

fn game_validate(root: &PathBuf, args: Value) -> McpToolOutput {
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

    if let Err(e) = ron::from_str::<ron::Value>(&scene_str) {
        return McpToolOutput::error(&format!("scene parse error: {e}"));
    }

    // Check all script paths referenced in the scene exist
    let mut missing_scripts: Vec<String> = Vec::new();
    for line in scene_str.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("script: Some(\"") {
            if let Some(script_rel) = rest.strip_suffix("\"),") {
                let script_path = game_dir.join(script_rel);
                if !script_path.exists() {
                    missing_scripts.push(script_rel.to_string());
                }
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
}

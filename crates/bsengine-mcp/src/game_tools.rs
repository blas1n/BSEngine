use std::path::{Path, PathBuf};

use bsengine_asset::identity::{read_import_settings, write_import_settings, ImportSettings};
use bsengine_core::{ModelImportSettings, TextureImportSettings};
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
  EntityDescriptor(
    name: "Wheel",
    parent: Some("Player"),    // optional: name of another entity in this same scene file;
                                // this entity's transform becomes relative to the parent's.
                                // Absent means a root entity. Unknown/self-referential names
                                // are dropped with a warning at load time, not a hard error.
    primitive: Some(Cube),
    transform: Some((position: (1.0, 0.0, 0.0))),  // relative to Player, not world space
  ),
])

Rules:
- Always include a Camera entity (camera: true) for rendering
- Always include a Sun entity (directional_light) or scene will be unlit
- primitive: Some(Cube) renders a white cube; use color to tint it
- look_at on a camera entity auto-computes rotation to face the target point
- color sets the albedo/surface color; emissive makes the entity glow
- parent makes this entity's transform relative to another entity by name (same scene file only)
- name is the key used by JS Bsengine.getPosition/setPosition"#;

const SCRIPT_API_DOCS: &str = r#"BSEngine JavaScript API (runs in V8 via Deno Core):

Transform:
  Bsengine.getPosition(name: string) → Vec3 | null
    Get an entity's position by name. Returns null if not found.
    A Vec3 has .x/.y/.z plus Unity-style helpers: .magnitude, .normalized,
    .add(v), .sub(v), .mul(s), and statics on Bsengine.Vec3 such as
    Bsengine.Vec3.distance(a, b), .lerp(a, b, t), .dot(a, b), .cross(a, b).

  Bsengine.setPosition(name: string, x, y, z)   // or setPosition(name, vec3)
    Set an entity's position by name. Accepts loose scalars or a Vec3, so
    Bsengine.setPosition("B", Bsengine.getPosition("A")) works.

  Bsengine.getTransform(name: string) → { position: Vec3, rotation: Quat, scale: Vec3 } | null
  Bsengine.setTransform(name: string, t)
    The whole transform. setTransform takes exactly what getTransform returns.

  Bsengine.vec3(x, y, z), Bsengine.Quat.euler(pitch, yaw, roll),
  Bsengine.Quat.lookRotation(forward, up)  // face a direction

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
    const p = Bsengine.getPosition(self);
    if (!p) return;
    let { x, y, z } = p;
    if (Bsengine.isKeyPressed("W")) z -= SPEED;
    if (Bsengine.isKeyPressed("S")) z += SPEED;
    if (Bsengine.isKeyPressed("A")) x -= SPEED;
    if (Bsengine.isKeyPressed("D")) x += SPEED;
    Bsengine.setPosition(self, x, y, z);
  }

Example (flash red when near origin):
  function onUpdate(self) {
    const p = Bsengine.getPosition(self);
    if (!p) return;
    const dist = Bsengine.Vec3.distance(p, Bsengine.Vec3.zero);
    Bsengine.setEmissive(self, dist < 2.0 ? 1.0 : 0.0, 0.0, 0.0);
  }

Example (controlling another entity by name from this script):
  function onUpdate(self) {
    const enemy = Bsengine.getPosition("Enemy");
    if (enemy) Bsengine.setPosition("Enemy", enemy.add(Bsengine.vec3(0.01, 0, 0)));
  }

Notes:
- Scripts load once at startup; onUpdate(self) runs every frame (~60fps)
- Each entity's script is isolated — multiple entities can each have their own script
- path is relative to game root (e.g. "assets/scripts/player.js")"#;

const SCRIPT_GRAPH_DOCS: &str = r#"Script graph file format (RON). A graph is nodes with numbered ids, edges
between named ports, and variables. Flow ports ("exec", "then", "true", "false", "body",
"completed") decide what runs next; data ports carry values. An edge is
(from: (node id, output port), to: (node id, input port)).

(
    nodes: [
        (id: 0, kind: OnUpdate, position: (0.0, 0.0)),
        (id: 1, kind: Branch, position: (220.0, 0.0)),
        (id: 2, kind: Call("isKeyDown"), position: (0.0, 80.0)),
        (id: 3, kind: Literal(Text("Space")), position: (-200.0, 80.0)),
        (id: 4, kind: Call("addPosition"), position: (460.0, 0.0)),
        (id: 5, kind: SelfEntity, position: (220.0, 100.0)),
        (id: 6, kind: Vec3Make, position: (220.0, 180.0)),
        (id: 7, kind: Literal(Number(0.0)), position: (0.0, 180.0)),
        (id: 8, kind: Multiply, position: (0.0, 260.0)),
        (id: 9, kind: GetVar("speed"), position: (-200.0, 240.0)),
        (id: 10, kind: Call("getDeltaTime"), position: (-200.0, 300.0)),
    ],
    edges: [
        (from: (0, "then"), to: (1, "exec")),
        (from: (2, "out"), to: (1, "condition")),
        (from: (3, "out"), to: (2, "key")),
        (from: (1, "true"), to: (4, "exec")),
        (from: (5, "out"), to: (4, "entity")),
        (from: (6, "out"), to: (4, "delta")),
        (from: (7, "out"), to: (6, "x")), (from: (8, "out"), to: (6, "y")), (from: (7, "out"), to: (6, "z")),
        (from: (9, "out"), to: (8, "a")), (from: (10, "out"), to: (8, "b")),
    ],
    variables: [(name: "speed", initial: Number(2.0))],
)

Node kinds and their ports (input -> output):
  Events (one flow output "then"): OnStart, OnUpdate, OnKeyPressed("Space"), OnInterval(0.5) [seconds],
    OnCollision (also outputs "other": Entity, readable only downstream of its "then")
  Flow: Branch (exec, condition: Bool -> true, false); Sequence (exec -> then0, then1, then2);
    ForLoop (exec, first, last: Number -> body, completed; "index": Number readable only under body);
    WhileLoop (exec, condition: Bool -> body, completed; stopped after 100000 iterations in one frame);
    Delay (exec, frames: Number -> completed, run that many frames later)
  Values: Literal(Number(1.0)) | Literal(Text("a")) | Literal(Bool(true)) (-> out); SelfEntity (-> out: Entity);
    GetVar("name") (-> out); SetVar("name") (exec, value -> then); ToText (x: Number -> out: Text);
    Concat (a, b: Text -> out: Text)
  Math: Add, Subtract, Multiply, Divide (a, b: Number -> out); Compare(Less | LessOrEqual | Greater |
    GreaterOrEqual | Equal | NotEqual) (a, b -> out: Bool); Not (x -> out); And, Or (a, b -> out);
    Vec3Make (x, y, z -> out: Vec3); Vec3Split (v -> x, y, z)
  Call("name"): one of the Bsengine functions below; an impure call has "exec" -> "then" plus a data
    port per parameter and "out" when it returns something; a pure call (a getter) has only data ports.
    Types: Number, Bool, Text, Entity (an entity's name; Text and Entity connect to each other), Vec3.

Compiles to one `onUpdate(self)`: OnStart runs on the first frame, OnKeyPressed is an isKeyPressed
check each frame, OnCollision registers Bsengine.onCollision on the first frame, OnInterval accumulates
getDeltaTime. Pure nodes are inlined where read; reading a flow-scoped value (OnCollision "other",
ForLoop "index") outside its flow is a compile error, as is a missing input, a type mismatch, an
unknown call, a flow output connected twice, and a cycle."#;

/// Builds the `game_create`/`scene_write`/`script_write`/`game_validate`/
/// `asset_import_settings`/`asset_references`/`script_graph_compile` tools,
/// each scoped to game projects under `root/games/`.
pub fn game_tools(root: PathBuf) -> Vec<McpTool> {
    let r1 = root.clone();
    let r2 = root.clone();
    let r3 = root.clone();
    let r4 = root.clone();
    let r5 = root.clone();
    let r6 = root.clone();
    let r7 = root.clone();

    vec![
        McpTool {
            name: "script_graph_compile".to_string(),
            description: format!(
                "Compile a visual script graph (`<name>.scriptgraph.ron`, written with \
                script_write or by hand) to the JavaScript the runtime loads, written beside it \
                as `<name>.js` -- what the editor's Script Graph panel's Compile button does. \
                Point a scene entity's `script:` at the `.js`. Returns {{path, js_path, js}} on \
                success; on a graph error nothing is written and the error names the node and \
                port to fix. The `.js` starts with a header saying it is generated; the next \
                compile overwrites it.\n\n\
                Available Call names: {ops}.\n\n\
                {SCRIPT_GRAPH_DOCS}",
                ops = bsengine_visualscript::OPS
                    .iter()
                    .map(|o| {
                        let params: Vec<String> = o
                            .params
                            .iter()
                            .map(|(n, t)| format!("{n}: {}", t.name()))
                            .collect();
                        match o.returns {
                            Some(r) => format!("{}({}) -> {}", o.name, params.join(", "), r.name()),
                            None => format!("{}({})", o.name, params.join(", ")),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            input_schema: Some(json!({
                "type": "object",
                "properties": {
                    "game": { "type": "string", "description": "Game folder name under games/" },
                    "path": { "type": "string", "description": "Graph path relative to the game root, ending in .scriptgraph.ron (e.g. 'assets/scripts/bob.scriptgraph.ron')" },
                },
                "required": ["game", "path"],
            })),
            handler: Box::new(move |args| script_graph_compile(&r7, args)),
        },
        McpTool {
            name: "asset_references".to_string(),
            description: "What an asset references and what references it -- Unreal's \
                Reference Viewer and Godot's View Owners, as a query. Built from the same \
                static walk the packager uses: every `assets/...` path in every scene \
                reachable from project.toml's `entry_scene` (and `[package] extra_assets`), \
                following prefabs, reflected components (a Terrain's textures) and quoted \
                paths in scripts (`loadScene(\"assets/scenes/level2.ron\")`).\n\n\
                With `path`: {path, reached, referencers, dependencies}. `referencers` are \
                the files that name it (`project.toml` when the manifest does), \
                `dependencies` the assets it names; `reached: false` means nothing names \
                it and a packaged build leaves it out.\n\
                Without `path`: the whole graph -- {assets, edges: [[referrer, asset], ...], \
                unreferenced: [...], missing: [{referrer, path}, ...]}. `unreferenced` are \
                the assets nothing reaches; `missing` are references to files that do not \
                exist, which fail packaging."
                .to_string(),
            input_schema: Some(json!({
                "type": "object",
                "properties": {
                    "game": { "type": "string", "description": "Game folder name under games/" },
                    "path": { "type": "string", "description": "Asset path relative to the game root (e.g. 'assets/models/fox.glb'); omit for the whole graph" },
                },
                "required": ["game"],
            })),
            handler: Box::new(move |args| asset_references(&r6, args)),
        },
        McpTool {
            name: "asset_import_settings".to_string(),
            description: "Read or change how one asset is imported -- the per-asset settings \
                Unity keeps in a .meta and Godot in a .import, stored here in the asset's \
                `<file>.meta` sidecar. Without `settings`, returns what is in force for the \
                asset (`recorded: false` means the kind's defaults, nothing tuned yet). With \
                `settings`, merges the given fields onto what is in force, writes the sidecar \
                (minting an identity for an asset no scan has seen yet, keeping the identity \
                of one that has), and returns the result. Fields not given keep their value; \
                an unknown field is an error, not ignored.\n\n\
                Textures (png, jpg, jpeg, hdr): `srgb` (bool, default true -- off for data \
                textures such as normal maps), `mipmaps` (bool, default true), `filter` \
                (\"Linear\" | \"Nearest\", default Linear), `wrap` (\"Repeat\" | \"Clamp\" | \
                \"Mirror\", default Repeat), `streaming` (bool, default false -- upload only \
                the mip levels up to 64 px at load and bring the larger ones in one per \
                frame afterwards; needs `mipmaps`).\n\
                Models (glb, gltf): `scale` (number > 0, default 1.0 -- baked into vertices, \
                skeleton, bind matrices and animation keys; 0.01 brings a centimetre file to \
                metres), `import_animations` (bool, default true).\n\n\
                A running game or editor that watches the project reloads the asset with the \
                new settings; a test session started before the edit sees them on its next \
                load of that asset."
                .to_string(),
            input_schema: Some(json!({
                "type": "object",
                "properties": {
                    "game":     { "type": "string", "description": "Game folder name under games/" },
                    "path":     { "type": "string", "description": "Asset path relative to the game root (e.g. 'assets/textures/wall.png' or 'assets/models/fox.glb')" },
                    "settings": { "type": "object", "description": "Fields to change, by name; omit to read. See the description for each kind's fields." },
                },
                "required": ["game", "path"],
            })),
            handler: Box::new(move |args| asset_import_settings(&r5, args)),
        },
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

/// The fields of one kind's settings as a JSON object, which is both what
/// the tool returns and what a partial edit is merged onto.
fn import_settings_json(settings: &ImportSettings) -> Value {
    match settings {
        ImportSettings::Texture(t) => serde_json::to_value(t),
        ImportSettings::Model(m) => serde_json::to_value(m),
    }
    .unwrap_or(Value::Null)
}

/// Whether `rel` stays inside the game it is joined onto: relative, and
/// without a `..` that could climb out. Shared by the tools that take an
/// asset path, so none of them can be handed `../../Cargo.toml`.
fn leaves_the_game(rel: &Path) -> bool {
    rel.is_absolute()
        || rel.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::Prefix(_)
            )
        })
}

fn asset_references(root: &Path, args: Value) -> McpToolOutput {
    let game = match get_str(&args, "game") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };
    let game_dir = root.join("games").join(&game);
    if !game_dir.join("project.toml").is_file() {
        return McpToolOutput::error(&format!(
            "games/{game}/project.toml not found — run game_create first"
        ));
    }
    let cooked = match bsengine_asset::cook::cook_project(&game_dir) {
        Ok(c) => c,
        Err(e) => return McpToolOutput::error(&format!("games/{game}: {e}")),
    };

    let Some(rel) = args.get("path").and_then(Value::as_str) else {
        return McpToolOutput::success(json!({
            "game": game,
            "assets": cooked.assets,
            "edges": cooked.edges,
            "unreferenced": cooked.unreferenced,
            "missing": cooked.missing,
        }));
    };
    if leaves_the_game(Path::new(rel)) {
        return McpToolOutput::error(
            "path must be relative to the game root and may not leave it (no `..`)",
        );
    }
    // The walk keys everything by the project-relative spelling with forward
    // slashes; a path an agent spelled with backslashes would look up
    // nothing and report "unreferenced" for a file the entry scene names.
    let rel = rel.replace('\\', "/");
    if !game_dir.join(&rel).is_file() {
        return McpToolOutput::error(&format!("games/{game}/{rel}: no such file"));
    }
    McpToolOutput::success(json!({
        "path": rel,
        "reached": cooked.assets.contains(&rel),
        "referencers": cooked.referencers_of(&rel),
        "dependencies": cooked.dependencies_of(&rel),
    }))
}

/// The suffix a graph file carries; the `.js` is the same name without it.
const SCRIPT_GRAPH_SUFFIX: &str = ".scriptgraph.ron";

fn script_graph_compile(root: &Path, args: Value) -> McpToolOutput {
    let game = match get_str(&args, "game") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };
    let rel = match get_str(&args, "path") {
        Ok(v) => v.replace('\\', "/"),
        Err(e) => return e,
    };
    let game_dir = root.join("games").join(&game);
    if !game_dir.join("project.toml").is_file() {
        return McpToolOutput::error(&format!(
            "games/{game}/project.toml not found — run game_create first"
        ));
    }
    // Scoped to the game like the other file-writing tools: this one writes
    // a `.js` beside whatever `path` names.
    if leaves_the_game(Path::new(&rel)) {
        return McpToolOutput::error(
            "path must be relative to the game root and may not leave it (no `..`)",
        );
    }
    // The suffix is the contract: the `.js` is named by stripping it, and a
    // file without it would have its `.js` land at a name nothing predicts.
    let Some(stem) = rel.strip_suffix(SCRIPT_GRAPH_SUFFIX) else {
        return McpToolOutput::error(&format!(
            "path must end in {SCRIPT_GRAPH_SUFFIX}; got {rel}"
        ));
    };
    let shown = format!("games/{game}/{rel}");
    let text = match std::fs::read_to_string(game_dir.join(&rel)) {
        Ok(t) => t,
        Err(e) => return McpToolOutput::error(&format!("{shown}: {e}")),
    };
    let graph: bsengine_visualscript::ScriptGraph = match ron::from_str(&text) {
        Ok(g) => g,
        Err(e) => return McpToolOutput::error(&format!("{shown}: not a script graph: {e}")),
    };
    let js = match bsengine_visualscript::compile(&graph) {
        Ok(js) => js,
        Err(e) => return McpToolOutput::error(&format!("{shown}: {e}")),
    };
    let js_rel = format!("{stem}.js");
    let js_path = game_dir.join(&js_rel);
    if let Err(e) = std::fs::write(&js_path, &js) {
        return McpToolOutput::error(&format!("games/{game}/{js_rel}: {e}"));
    }
    McpToolOutput::success(json!({
        "path": rel,
        "js_path": js_rel,
        "js": js,
    }))
}

fn asset_import_settings(root: &Path, args: Value) -> McpToolOutput {
    let game = match get_str(&args, "game") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };
    let rel = match get_str(&args, "path") {
        Ok(v) => v.to_string(),
        Err(e) => return e,
    };
    // Scoped to the game: this tool writes a file beside whatever `path`
    // names, so `../../.cargo/config.toml.meta` must not be reachable from
    // it. `script_write` has no such guard, and this one is not a licence
    // for that -- an agent that can write arbitrary scripts is already
    // trusted with the tree, but a *sidecar* landing beside a non-asset is a
    // file the scan would then refuse to explain, so the cheap check goes in.
    let rel_path = Path::new(&rel);
    if leaves_the_game(rel_path) {
        return McpToolOutput::error(
            "path must be relative to the game root and may not leave it (no `..`)",
        );
    }
    let asset = root.join("games").join(&game).join(rel_path);
    let shown = format!("games/{game}/{rel}");

    let report = match read_import_settings(&asset) {
        Ok(r) => r,
        Err(e) => return McpToolOutput::error(&format!("{shown}: {e}")),
    };

    let Some(patch) = args.get("settings") else {
        return McpToolOutput::success(json!({
            "path": shown,
            "kind": report.kind,
            "recorded": report.recorded,
            "settings": import_settings_json(&report.settings),
        }));
    };
    let Some(patch) = patch.as_object() else {
        return McpToolOutput::error("`settings` must be an object of the kind's fields");
    };

    // Merge onto what is in force, field by field, refusing a field the kind
    // does not have: `mipmap` for `mipmaps` silently ignored would leave an
    // agent certain it had turned mipmaps off.
    let mut merged = import_settings_json(&report.settings);
    let fields = merged.as_object().cloned().unwrap_or_default();
    for (key, value) in patch {
        if !fields.contains_key(key) {
            let known: Vec<&String> = fields.keys().collect();
            return McpToolOutput::error(&format!(
                "unknown {} import setting `{key}`; the settings are {known:?}",
                report.kind
            ));
        }
        merged[key] = value.clone();
    }
    let settings = match report.kind {
        "Texture" => serde_json::from_value::<TextureImportSettings>(merged.clone())
            .map(ImportSettings::Texture),
        "Model" => {
            serde_json::from_value::<ModelImportSettings>(merged.clone()).map(ImportSettings::Model)
        }
        other => return McpToolOutput::error(&format!("unhandled import kind {other}")),
    };
    let settings = match settings {
        Ok(s) => s,
        Err(e) => {
            return McpToolOutput::error(&format!("invalid {} import settings: {e}", report.kind))
        }
    };
    if let ImportSettings::Model(m) = &settings {
        // The loader would read a bad scale as 1.0 and warn; here somebody is
        // asking for it, so refuse instead of writing a value that will be
        // silently corrected on every load.
        if !m.usable_scale().1 {
            return McpToolOutput::error(&format!(
                "scale must be a finite number greater than zero, got {}",
                m.scale
            ));
        }
    }

    match write_import_settings(&asset, settings) {
        Ok(meta) => McpToolOutput::success(json!({
            "path": shown,
            "kind": report.kind,
            "recorded": true,
            "settings": import_settings_json(&settings),
            "written": format!("games/{game}/{}", meta.strip_prefix(root.join("games").join(&game)).unwrap_or(&meta).display()).replace('\\', "/"),
            "note": "an engine watching this project reloads the asset with these settings; a test session sees them on its next load of the asset",
        })),
        Err(e) => McpToolOutput::error(&format!("{shown}: {e}")),
    }
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

    fn import_tool(root: &Path) -> McpTool {
        game_tools(root.to_path_buf())
            .into_iter()
            .find(|t| t.name == "asset_import_settings")
            .expect("registered")
    }

    fn references_tool(root: &Path) -> McpTool {
        game_tools(root.to_path_buf())
            .into_iter()
            .find(|t| t.name == "asset_references")
            .expect("registered")
    }

    fn graph_tool(root: &Path) -> McpTool {
        game_tools(root.to_path_buf())
            .into_iter()
            .find(|t| t.name == "script_graph_compile")
            .expect("registered")
    }

    /// A game with a graph that compiles: `OnUpdate -> log("tick")`.
    fn game_with_graph(root: &Path) {
        let write = |rel: &str, text: &str| {
            let path = root.join("games/g").join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write(
            "project.toml",
            "[project]\nname = \"G\"\nentry_scene = \"assets/scenes/main.ron\"\n",
        );
        write(
            "assets/scripts/tick.scriptgraph.ron",
            r#"(
    nodes: [
        (id: 0, kind: OnUpdate, position: (0.0, 0.0)),
        (id: 1, kind: Call("log"), position: (200.0, 0.0)),
        (id: 2, kind: Literal(Text("tick")), position: (0.0, 60.0)),
    ],
    edges: [
        (from: (0, "then"), to: (1, "exec")),
        (from: (2, "out"), to: (1, "message")),
    ],
)"#,
        );
    }

    /// The tool writes exactly what `compile` produces, beside the graph,
    /// and returns it -- the same file the panel's Compile writes, so a
    /// scene's `script:` can name it.
    #[test]
    fn script_graph_compile_writes_the_js_beside_the_graph() {
        let (_tmp, root) = temp_root();
        game_with_graph(&root);
        let tool = graph_tool(&root);
        let out =
            (tool.handler)(json!({"game": "g", "path": "assets/scripts/tick.scriptgraph.ron"}));
        assert!(out.is_ok(), "{:?}", out.error);
        assert_eq!(out.content["js_path"], "assets/scripts/tick.js");
        let js_path = root.join("games/g/assets/scripts/tick.js");
        let written =
            std::fs::read_to_string(&js_path).expect("the .js is written beside the graph");
        assert_eq!(out.content["js"], written);
        assert!(
            written.starts_with(bsengine_visualscript::HEADER),
            "the file says it is generated"
        );
        assert!(
            written.contains("Bsengine.log(\"tick\");"),
            "the flow compiled: {written}"
        );
        assert!(
            written.contains("function onUpdate(self)"),
            "the runtime's entry point: {written}"
        );
        // Backslashes are accepted and the reply spells the path forward.
        let out =
            (tool.handler)(json!({"game": "g", "path": "assets\\scripts\\tick.scriptgraph.ron"}));
        assert!(out.is_ok(), "{:?}", out.error);
        assert_eq!(out.content["path"], "assets/scripts/tick.scriptgraph.ron");
    }

    /// A graph with an error compiles to nothing: no `.js` appears, and the
    /// error names the node to fix. Then the refusals: a path outside the
    /// game, a file without the suffix, a file that is not a graph, a game
    /// without a manifest.
    #[test]
    fn script_graph_compile_writes_nothing_for_a_bad_graph_and_refuses_bad_paths() {
        let (_tmp, root) = temp_root();
        game_with_graph(&root);
        // Drop the message edge: `log` is missing its input.
        let graph = root.join("games/g/assets/scripts/tick.scriptgraph.ron");
        let text = std::fs::read_to_string(&graph).unwrap();
        std::fs::write(
            &graph,
            text.replace(r#"(from: (2, "out"), to: (1, "message")),"#, ""),
        )
        .unwrap();
        let tool = graph_tool(&root);
        let out =
            (tool.handler)(json!({"game": "g", "path": "assets/scripts/tick.scriptgraph.ron"}));
        assert!(!out.is_ok());
        let error = out.error.as_deref().unwrap_or("");
        assert!(
            error.contains("node 1") && error.contains("message"),
            "the error names the node and port: {error}"
        );
        assert!(
            !root.join("games/g/assets/scripts/tick.js").exists(),
            "a failed compile must not leave a .js behind"
        );

        let out = (tool.handler)(
            json!({"game": "g", "path": "../g/assets/scripts/tick.scriptgraph.ron"}),
        );
        assert!(
            out.error.as_deref().unwrap_or("").contains("may not leave"),
            "{:?}",
            out.error
        );

        std::fs::write(root.join("games/g/assets/scripts/plain.ron"), "()").unwrap();
        let out = (tool.handler)(json!({"game": "g", "path": "assets/scripts/plain.ron"}));
        assert!(
            out.error
                .as_deref()
                .unwrap_or("")
                .contains(".scriptgraph.ron"),
            "{:?}",
            out.error
        );

        std::fs::write(
            root.join("games/g/assets/scripts/junk.scriptgraph.ron"),
            "not ron {{",
        )
        .unwrap();
        let out =
            (tool.handler)(json!({"game": "g", "path": "assets/scripts/junk.scriptgraph.ron"}));
        assert!(
            out.error
                .as_deref()
                .unwrap_or("")
                .contains("not a script graph"),
            "{:?}",
            out.error
        );

        let out =
            (tool.handler)(json!({"game": "g", "path": "assets/scripts/none.scriptgraph.ron"}));
        assert!(!out.is_ok(), "a missing file is an error");

        let out =
            (tool.handler)(json!({"game": "other", "path": "assets/scripts/tick.scriptgraph.ron"}));
        assert!(
            out.error.as_deref().unwrap_or("").contains("project.toml"),
            "{:?}",
            out.error
        );
    }

    /// The tool's description is what an agent authors graphs from: it must
    /// list every call the compiler accepts, with its signature, so that a
    /// name added to the op table is documented without anyone remembering.
    #[test]
    fn script_graph_compile_describes_every_op_and_the_format() {
        let (_tmp, root) = temp_root();
        let tool = graph_tool(&root);
        for op in bsengine_visualscript::OPS {
            assert!(
                tool.description.contains(&format!("{}(", op.name)),
                "{} is missing from the tool description",
                op.name
            );
        }
        assert!(tool.description.contains("getDeltaTime() -> Number"));
        assert!(tool
            .description
            .contains("addPosition(entity: Entity, delta: Vec3)"));
        for kind in [
            "ForLoop",
            "WhileLoop",
            "Delay",
            "OnInterval",
            "OnCollision",
            "Vec3Split",
        ] {
            assert!(tool.description.contains(kind), "{kind} is not documented");
        }
    }

    /// A game whose entry scene names a model and a script, whose script
    /// names a second scene that names the model again, plus a texture
    /// nothing names.
    fn game_with_references(root: &Path) {
        let write = |rel: &str, text: &str| {
            let path = root.join("games/g").join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write(
            "project.toml",
            "[project]\nname = \"G\"\nentry_scene = \"assets/scenes/main.ron\"\n",
        );
        write(
            "assets/scenes/main.ron",
            r#"(entities: [(name: "Hero", gltf: Some(Path("assets/models/hero.glb")), script: Some(Path("assets/scripts/hero.js")))])"#,
        );
        write(
            "assets/scripts/hero.js",
            "const NEXT = \"assets/scenes/level2.ron\";",
        );
        write(
            "assets/scenes/level2.ron",
            r#"(entities: [(name: "Hero", gltf: Some(Path("assets/models/hero.glb")))])"#,
        );
        write("assets/models/hero.glb", "glb");
        write("assets/textures/unused.png", "png");
    }

    /// Per asset: who names it and what it names, with the second scene
    /// found through the script -- the walk is the packager's, not a grep
    /// of the entry scene. The texture nothing names is `reached: false`.
    #[test]
    fn asset_references_answers_who_uses_an_asset_and_what_it_uses() {
        let (_tmp, root) = temp_root();
        game_with_references(&root);
        let tool = references_tool(&root);

        let out = (tool.handler)(json!({"game": "g", "path": "assets/models/hero.glb"}));
        assert!(out.is_ok(), "{:?}", out.error);
        assert_eq!(out.content["reached"], true);
        assert_eq!(
            out.content["referencers"],
            json!(["assets/scenes/level2.ron", "assets/scenes/main.ron"])
        );
        assert_eq!(out.content["dependencies"], json!([]));

        let out = (tool.handler)(json!({"game": "g", "path": "assets/scripts/hero.js"}));
        assert!(out.is_ok(), "{:?}", out.error);
        assert_eq!(
            out.content["referencers"],
            json!(["assets/scenes/main.ron"])
        );
        assert_eq!(
            out.content["dependencies"],
            json!(["assets/scenes/level2.ron"]),
            "a quoted path in a script is a dependency"
        );

        let out = (tool.handler)(json!({"game": "g", "path": "assets/textures/unused.png"}));
        assert!(out.is_ok(), "{:?}", out.error);
        assert_eq!(out.content["reached"], false);
        assert_eq!(out.content["referencers"], json!([]));

        let out = (tool.handler)(json!({"game": "g", "path": "assets/scenes/main.ron"}));
        assert_eq!(
            out.content["referencers"],
            json!(["project.toml"]),
            "the entry scene is named by the manifest"
        );
    }

    /// Without a path, the whole graph: every edge, the unreached assets
    /// and the dangling references, so an agent can audit a project in one
    /// call the way Godot's Orphan Resource Explorer does.
    #[test]
    fn asset_references_without_a_path_returns_the_whole_graph() {
        let (_tmp, root) = temp_root();
        game_with_references(&root);
        std::fs::write(
            root.join("games/g/assets/scenes/level2.ron"),
            r#"(entities: [(name: "Hero", gltf: Some(Path("assets/models/hero.glb")), texture: Some("assets/textures/gone.png"))])"#,
        )
        .unwrap();
        let tool = references_tool(&root);

        let out = (tool.handler)(json!({"game": "g"}));
        assert!(out.is_ok(), "{:?}", out.error);
        let edges = out.content["edges"].as_array().expect("edges");
        assert!(
            edges.contains(&json!([
                "assets/scripts/hero.js",
                "assets/scenes/level2.ron"
            ])),
            "the script -> scene edge is in the graph; got {edges:?}"
        );
        assert!(edges.contains(&json!(["project.toml", "assets/scenes/main.ron"])));
        assert_eq!(
            out.content["unreferenced"],
            json!(["assets/textures/unused.png"])
        );
        assert_eq!(
            out.content["missing"],
            json!([{"referrer": "assets/scenes/level2.ron", "path": "assets/textures/gone.png"}]),
            "a dangling reference is reported with who made it"
        );
    }

    /// The refusals, each without a walk having any effect: a path that
    /// climbs out of the game, a file that is not there, a game without a
    /// manifest.
    #[test]
    fn asset_references_refuses_paths_outside_the_game_and_missing_files() {
        let (_tmp, root) = temp_root();
        game_with_references(&root);
        let tool = references_tool(&root);

        let out = (tool.handler)(json!({"game": "g", "path": "../g/assets/models/hero.glb"}));
        assert!(!out.is_ok());
        assert!(out.error.as_deref().unwrap_or("").contains("may not leave"));

        let out = (tool.handler)(json!({"game": "g", "path": "assets/models/nope.glb"}));
        assert!(!out.is_ok());
        assert!(out.error.as_deref().unwrap_or("").contains("no such file"));

        let out = (tool.handler)(json!({"game": "other", "path": "assets/models/hero.glb"}));
        assert!(!out.is_ok());
        assert!(out.error.as_deref().unwrap_or("").contains("project.toml"));
    }

    /// A game with one texture and one model, neither scanned, so the tool's
    /// minting path is what every test below starts from.
    fn game_with_assets(root: &Path) {
        std::fs::create_dir_all(root.join("games/g/assets/textures")).unwrap();
        std::fs::create_dir_all(root.join("games/g/assets/models")).unwrap();
        std::fs::create_dir_all(root.join("games/g/assets/scenes")).unwrap();
        std::fs::write(root.join("games/g/assets/textures/wall.png"), b"png bytes").unwrap();
        std::fs::write(root.join("games/g/assets/models/fox.glb"), b"glb bytes").unwrap();
        std::fs::write(root.join("games/g/assets/scenes/main.ron"), b"()").unwrap();
    }

    #[test]
    fn asset_import_settings_reads_the_defaults_for_an_untuned_asset() {
        let (_tmp, root) = temp_root();
        game_with_assets(&root);
        let tool = import_tool(&root);
        let out = (tool.handler)(json!({"game": "g", "path": "assets/textures/wall.png"}));
        assert!(out.is_ok(), "{:?}", out.error);
        assert_eq!(out.content["kind"], "Texture");
        assert_eq!(out.content["recorded"], false);
        assert_eq!(out.content["settings"]["srgb"], true);
        assert_eq!(out.content["settings"]["wrap"], "Repeat");
        assert!(
            !root.join("games/g/assets/textures/wall.png.meta").exists(),
            "a read must not mint a sidecar"
        );

        let out = (tool.handler)(json!({"game": "g", "path": "assets/models/fox.glb"}));
        assert!(out.is_ok(), "{:?}", out.error);
        assert_eq!(out.content["kind"], "Model");
        assert_eq!(out.content["settings"]["scale"], 1.0);
        assert_eq!(out.content["settings"]["import_animations"], true);
    }

    /// The merge semantics, which is what makes a one-field edit safe for an
    /// agent: the second edit must not reset the first. A tool that replaced
    /// the whole record with `{filter: Nearest}` plus defaults would pass the
    /// first half of this test and fail the second.
    #[test]
    fn asset_import_settings_merges_each_edit_onto_what_is_recorded() {
        use bsengine_asset::identity::{sidecar_path, Sidecar};
        use bsengine_core::{TextureFilter, TextureWrap};

        let (_tmp, root) = temp_root();
        game_with_assets(&root);
        let tool = import_tool(&root);
        let png = root.join("games/g/assets/textures/wall.png");

        let out = (tool.handler)(json!({
            "game": "g", "path": "assets/textures/wall.png", "settings": {"srgb": false}
        }));
        assert!(out.is_ok(), "{:?}", out.error);
        assert_eq!(out.content["recorded"], true);
        assert_eq!(
            out.content["written"],
            "games/g/assets/textures/wall.png.meta"
        );
        let first = Sidecar::read(sidecar_path(&png)).unwrap().expect("minted");
        assert!(!first.texture_import().srgb);
        assert!(
            first.texture_import().mipmaps,
            "untouched fields keep their defaults"
        );

        let out = (tool.handler)(json!({
            "game": "g", "path": "assets/textures/wall.png",
            "settings": {"filter": "Nearest", "wrap": "Clamp"}
        }));
        assert!(out.is_ok(), "{:?}", out.error);
        let second = Sidecar::read(sidecar_path(&png))
            .unwrap()
            .expect("still there");
        assert_eq!(
            second.guid, first.guid,
            "the identity minted by the first edit survives"
        );
        assert_eq!(
            second.texture_import(),
            TextureImportSettings {
                srgb: false,
                mipmaps: true,
                filter: TextureFilter::Nearest,
                wrap: TextureWrap::Clamp,
                streaming: false,
            },
            "the second edit keeps the first's srgb: false"
        );

        let out = (tool.handler)(json!({"game": "g", "path": "assets/textures/wall.png"}));
        assert_eq!(out.content["recorded"], true);
        assert_eq!(out.content["settings"]["srgb"], false);
        assert_eq!(out.content["settings"]["filter"], "Nearest");
    }

    #[test]
    fn asset_import_settings_writes_a_models_scale_and_animation_toggle() {
        use bsengine_asset::identity::{sidecar_path, Sidecar};

        let (_tmp, root) = temp_root();
        game_with_assets(&root);
        let tool = import_tool(&root);
        let out = (tool.handler)(json!({
            "game": "g", "path": "assets/models/fox.glb",
            "settings": {"scale": 0.01, "import_animations": false}
        }));
        assert!(out.is_ok(), "{:?}", out.error);
        let sidecar = Sidecar::read(sidecar_path(root.join("games/g/assets/models/fox.glb")))
            .unwrap()
            .expect("minted");
        assert_eq!(
            sidecar.model_import(),
            ModelImportSettings {
                scale: 0.01,
                import_animations: false,
            }
        );
    }

    /// Every refusal, and that none of them wrote anything: an agent's typo
    /// must come back as an error naming the field, not as a sidecar that
    /// quietly says something else.
    #[test]
    fn asset_import_settings_refuses_bad_input_without_writing() {
        let (_tmp, root) = temp_root();
        game_with_assets(&root);
        let tool = import_tool(&root);
        let png = "assets/textures/wall.png";
        let cases: Vec<(Value, &str)> = vec![
            (
                json!({"game": "g", "path": png, "settings": {"mipmap": false}}),
                "unknown Texture import setting `mipmap`",
            ),
            (
                json!({"game": "g", "path": png, "settings": {"srgb": "yes"}}),
                "invalid Texture import settings",
            ),
            (
                json!({"game": "g", "path": png, "settings": {"wrap": "Tile"}}),
                "invalid Texture import settings",
            ),
            (
                json!({"game": "g", "path": png, "settings": 3}),
                "must be an object",
            ),
            (
                json!({"game": "g", "path": "assets/models/fox.glb", "settings": {"scale": 0}}),
                "scale must be a finite number greater than zero",
            ),
            (
                json!({"game": "g", "path": "assets/scenes/main.ron"}),
                "has no import settings",
            ),
            (
                json!({"game": "g", "path": "assets/textures/missing.png"}),
                "does not exist",
            ),
            (
                json!({"game": "g", "path": "../../Cargo.toml"}),
                "may not leave it",
            ),
            // A real texture in *another* game: without the guard this one
            // would not fail for any other reason -- the file exists and is
            // a texture -- so it is the case that tells a guard from the
            // read failing on its own.
            (
                json!({"game": "g", "path": "../other/assets/leak.png", "settings": {"srgb": false}}),
                "may not leave it",
            ),
        ];
        std::fs::create_dir_all(root.join("games/other/assets")).unwrap();
        std::fs::write(root.join("games/other/assets/leak.png"), b"png bytes").unwrap();
        for (args, expected) in cases {
            let out = (tool.handler)(args.clone());
            assert!(!out.is_ok(), "{args} must be refused");
            let error = out.error.unwrap();
            assert!(
                error.contains(expected),
                "{args}: expected the error to mention {expected:?}, got {error:?}"
            );
        }
        for meta in [
            "games/g/assets/textures/wall.png.meta",
            "games/g/assets/models/fox.glb.meta",
            "games/g/assets/scenes/main.ron.meta",
            "Cargo.toml.meta",
            "games/other/assets/leak.png.meta",
        ] {
            assert!(
                !root.join(meta).exists(),
                "{meta} must not have been written"
            );
        }
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

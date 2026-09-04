# BSEngine

A personal game engine written in Rust. Started as a C++ project in 2021, rewritten in Rust for a solid infrastructure-first foundation.

![CI](https://github.com/blas1n/BSEngine/actions/workflows/ci.yml/badge.svg)

---

## Architecture

BSEngine is organized as a Cargo workspace of focused crates:

```
bsengine-core         — shared primitives (math, error types)
bsengine-ecs          — ECS wrappers around bevy_ecs
bsengine-app          — application loop and plugin system (bevy_app)
bsengine-window       — platform window management (winit)
bsengine-input        — keyboard/mouse input abstraction
bsengine-rhi          — render hardware interface (abstract GPU trait)
bsengine-rhi-wgpu     — wgpu implementation of bsengine-rhi
bsengine-render       — scene rendering pipeline
bsengine-scene        — scene graph, entity transforms, hierarchy
bsengine-asset        — asset loading (textures, meshes)
bsengine-gltf         — GLTF/GLB import
bsengine-plugin       — runtime plugin loader
bsengine-mcp          — MCP (Model Context Protocol) server runtime
bsengine-editor       — editor backend with 700+ MCP tools
bsengine-scripting    — JavaScript scripting via Deno/V8
```

**Dependency flow:**
```
core ← ecs ← app ← window / input
                  ← rhi ← rhi-wgpu ← render ← scene ← asset / gltf
                  ← mcp ← editor
                  ← scripting
                  ← plugin
```

---

## What's Implemented

### Rendering (bsengine-render / bsengine-rhi-wgpu)
- wgpu-based GPU surface and swap chain
- Camera and mesh components with Transform hierarchy
- UV coordinates and texture sampling
- Directional lighting with PCF shadow mapping
- Point lights and spot lights with attenuation
- Cook-Torrance PBR materials
- Frustum culling via bounding sphere test

### Scene (bsengine-scene)
- Entity spawn/despawn with named entities
- Transform hierarchy (parent/child relationships)
- Scene save/load (RON format)
- Visibility component

### Editor MCP (bsengine-editor)
AI-native editor backend exposed via the Model Context Protocol. An AI agent can drive the editor entirely through MCP tool calls.

**Tool categories (~700 tools):**
- Entity spawn, despawn, duplicate, batch spawn
- Transform: position, rotation, scale (set/move/snap/align)
- Hierarchy: parent/child management
- Lights: point, directional, spot (spawn/update/remove)
- Camera: spawn, update FOV
- Mesh: attach/detach renderer
- Tags: add/remove/query tags per entity
- Selection: select/deselect by any property
- Query: get/count entities filtered by any property
- Scene: save, load, clear

Full select/deselect/count symmetry: every `get_entities_with_X` filter has a matching `select_`, `deselect_`, and `count_` variant.

### Scripting (bsengine-scripting)
- JavaScript runtime via Deno Core (V8)
- ECS ops exposed to scripts as async Deno ops
- Plugin system for loading `.js` scripts at runtime

---

## Building

Requires: Rust stable, Vulkan/Metal/DX12 GPU driver (for rendering tests on Linux: `mesa-vulkan-drivers`)

```bash
cargo build --all
cargo test --all
```

CI runs on Ubuntu and Windows via GitHub Actions.

### Packaging a build

```bash
cargo run -p bsengine-runtime -- --package games/mini-arena
```

Collects exactly the assets the project reaches from its entry scene and writes
them, `project.toml`, and the runtime into `games/mini-arena/dist/`. `--out <dir>`
writes somewhere else; the directory must be empty or absent, since an asset left
over from a previous build would ship with this one.

**Run the executable from inside that directory** — no arguments needed, since
the runtime defaults its project directory to `.`. This is not just convenience:
textures and other `AssetServer`-loaded assets resolve against the *working
directory*, so launching the executable from elsewhere leaves the game running
with its textures missing rather than failing outright.

Any reference in a scene or prefab that resolves to nothing fails the build, and
is reported with the file that names it.

A path a script *builds* — by concatenation, or chosen from data — cannot be seen
by a static walk and will not be collected. Plain quoted literals are
(`Bsengine.loadScene("assets/scenes/level2.ron")` pulls in that scene and
everything it references); one that resolves to nothing is a warning rather than
a failure, since a quoted string that looks like a path may be dead code or a
deliberate probe of something absent.

For those, and for a file a build needs that nothing references — a model's
attribution notice, say — name it in `project.toml`:

```toml
[package]
extra_assets = ["assets/models/CREDITS.md"]
```

Each entry is followed like any other reference, so listing a scene brings in
what that scene references. Unlike a path spelled only in a script, an entry
here that resolves to nothing **fails** the build: this list is written on
purpose, so a typo in it is a mistake rather than a guess.

Output is a directory, not a single file. A packed single-file mode, and the
project setting that chooses between them, is the next step.

---

## Project Status

Active development. Infrastructure is stable; rendering and editor layers are the current focus.

| Layer | Status |
|-------|--------|
| Core / ECS / App | Stable |
| Rendering (wgpu) | Functional — PBR, shadows, lights |
| Scene / Asset / GLTF | Functional |
| Editor MCP | Extensive — 700+ tools, 774 tests |
| Scripting (Deno/V8) | Early — runtime + ECS ops wired |
| Physics | Planned |
| Audio | Planned |

---

## License

MIT

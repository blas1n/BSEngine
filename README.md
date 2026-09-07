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

### Packaging modes

```toml
[package]
mode = "pak"          # or "loose" (the default)
```

`--mode <loose|pak>` on the command line overrides the setting for one build.

**`loose`** writes `assets/` as ordinary files beside the executable. Any tool
can open them, which is what makes it the default and the mode to reach for when
a build misbehaves.

**`pak`** writes a single `game.pak` instead — an index plus the asset bytes,
the shape Unreal, Unity and Godot all use. Fewer files to ship, and only this
engine can read them.

The build is then `exe + project.toml + game.pak`: **fewer files, but still not
one file.** Embedding the archive into the executable, the way Godot embeds a
`.pck`, is the route to a literal single file and is not implemented.

One asset kind cannot be packed: a `.gltf` that references sibling `.bin` or
image files resolves them through the filesystem, which an archive has none of.
`--mode pak` fails the build and names them rather than shipping a game that
loses its meshes at run time. A `.glb` is self-contained and packs fine.

---

## Networking

```toml
[network]
aoi_radius = 50.0               # omit for no limit
interpolation_delay_ticks = 3   # 0 renders the newest snapshot immediately
simulated_latency_frames = 0    # testing only
simulated_loss = 0.0            # testing only, 0.0..=1.0
simulator_seed = 0              # fixed, so a run reproduces
```

Every default reproduces the engine's earlier behaviour, so a project that says
nothing about networking behaves as it did.

**`interpolation_delay_ticks` is a cost, not a free win.** Remote entities are
rendered that many server ticks *in the past*, which is what buys smooth motion
when packets arrive unevenly. Set it to `0` and a remote entity snaps to each
snapshot as it lands — perfect on a perfect link, visibly stuttery on a real one.

Past the newest snapshot it holds, and does not extrapolate. A remote entity
whose updates stop **freezes** rather than gliding on along its last heading:
that reads as the network problem it is, where a guess that overshoots would
snap backwards when the truth arrived.

**`aoi_radius` stops updates, it does not hide entities.** An entity outside a
peer's radius stays wherever that peer last saw it. A peer with no entity of its
own receives everything, since there is no position to measure interest from.

The two `simulated_*` settings exist to test the two above, and are off by
default. They are also the only way to observe either: on a perfect link an
interpolated position and the newest snapshot agree on every frame.

**Not implemented:** client-side prediction and server reconciliation. A client
today is authoritative over its own entities and sends their transforms, so the
server never disagrees with it about them. Making the server authoritative — per
entity, opt-in — is the next step.

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

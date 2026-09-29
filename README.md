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
- Reflection probes (`ReflectionProbe`): a cubemap captured once where the probe
  stands, prefiltered like the sky, optionally box-projected; where boxes nest,
  the smallest wins (no blending)

### Scene (bsengine-scene)
- Entity spawn/despawn with named entities
- Transform hierarchy (parent/child relationships)
- Scene save/load (RON format)
- Visibility component

### Editor MCP (bsengine-editor)
AI-native editor backend exposed via the Model Context Protocol. An AI agent can drive the editor entirely through MCP tool calls.

**Tool categories (145 tools):**
- Entity spawn, despawn, duplicate, batch spawn
- Transform: position, rotation, scale (set/move/snap/align)
- Hierarchy: parent/child management
- Lights: point, directional, spot (spawn/update/remove)
- Camera: spawn, update FOV
- Mesh: attach/detach renderer
- Tags: add/remove/query tags per entity
- Selection: select/deselect an entity, all, or whatever a query matches
- Query: `query_entities` -- conditions on any field (`where`/`any`), sort, limit, and a verb (get, count, select, deselect, select_only, tags, bounds)
- Scene: save, load, clear

One query tool rather than a tool per filter: the editor once had about a thousand `select_/deselect_/count_/get_entities_with_X` variants, which an agent paid for in context on every request and which still could not combine two conditions.

### Scripting (bsengine-scripting)
- JavaScript runtime via Deno Core (V8)
- ECS ops exposed to scripts as async Deno ops
- Plugin system for loading `.js` scripts at runtime

---

## Building

Requires: Rust stable, Vulkan/Metal/DX12 GPU driver (for rendering tests on Linux: `mesa-vulkan-drivers`)

```bash
cargo build --all

# Two invocations, not `--all`: bsengine-editor overflows the stack when its
# suite runs concatenated with the rest of the workspace.
cargo test --workspace --exclude bsengine-editor
cargo test -p bsengine-editor
```

CI runs on Ubuntu, Windows and macOS via GitHub Actions, and covers more than
the tests: formatting, clippy, the component catalogue, every checked-in E2E
replay, packaging every project, and a smoke test that opens a real window.
See `CLAUDE.md` for how to run and judge those gates locally.

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

`--mode <loose|pak|single>` on the command line overrides the setting for one
build.

**`loose`** writes `assets/` as ordinary files beside the executable. Any tool
can open them, which is what makes it the default and the mode to reach for when
a build misbehaves.

**`pak`** writes a single `game.pak` instead — an index plus the asset bytes,
the shape Unreal, Unity and Godot all use. Fewer files to ship, and only this
engine can read them. The build is `exe + project.toml + game.pak`.

**`single`** goes one step further, the way Godot's "Embed PCK" does: the
archive, with `project.toml` inside it, is appended to the executable behind a
small trailer, and the runtime reads its own binary to find it. The build is
**one file**. Run it from anywhere — nothing is read from beside it; only the
`.bsengine_cache/` a run writes lands in the working directory. The exception
is macOS, where the archive is written beside the binary instead: a Mach-O
with bytes past its load commands fails code-signature validation, and on
Apple silicon an unsignable binary does not run (Godot declines to embed there
for the same reason). That build is `exe + game.pak`, still with no loose
manifest.

One asset kind cannot be packed: a `.gltf` that references sibling `.bin` or
image files resolves them through the filesystem, which an archive has none of.
`--mode pak` and `--mode single` fail the build and name them rather than
shipping a game that loses its meshes at run time. A `.glb` is self-contained
and packs fine.

---

## Input actions

Name what the player does, not which key does it:

```toml
[input]
deadzone = 0.2          # stick travel below this reads 0; the default

[input.actions]
jump       = ["Space", "Gamepad:South"]
fire       = ["Mouse:Left", "Gamepad:RightTrigger"]
move_left  = ["A", "Left", "Gamepad:LeftStickX-"]
move_right = ["D", "Right", "Gamepad:LeftStickX+"]
```

```js
if (Bsengine.isActionDown("jump")) jump();          // this frame's press
const x = Bsengine.getAxis("move_left", "move_right"); // -1..1
const v = Bsengine.getVector("move_left", "move_right", "move_back", "move_forward");
```

A binding is a key name (`"Space"`, `"E"`, `"1"`, `"ShiftLeft"`), `Mouse:Left|Right|Middle`,
a gamepad button (`Gamepad:South`, `Gamepad:DPadUp`, …), a stick direction
(`Gamepad:LeftStickX+`, `Gamepad:RightStickY-`, Y positive up) or a trigger
(`Gamepad:LeftTrigger`). An action's strength is its strongest binding: 1 for a
held button, and for a stick its travel past the deadzone stretched back over
0..1, so full travel reaches 1. `getVector` is clamped to length 1, so a
diagonal is not faster than a straight line. This is Godot's `InputMap`, and
the part Unity's Input System and Unreal's Enhanced Input share with it; there
are no mapping contexts or hold/tap interactions — build those from
`isActionDown` and a timer.

`isActionPressed` / `isActionDown` / `isActionUp` / `getActionStrength` follow
the key functions' naming: *Pressed* is held, *Down*/*Up* are this frame's edges.
A rebinding screen uses `getActionBindings(action)` and
`setActionBindings(action, ["J", "Gamepad:North"])`, which applies from the next
frame and does not persist by itself — store it with `setSaveField` if the game
should remember it (Unity likewise leaves saving binding overrides to the game).

**Mistakes stop things rather than doing nothing.** A bad binding in
`project.toml` stops the game at start, and `--package` refuses to build it,
naming every bad binding with its action. An unknown action name, or a bad
binding passed to `setActionBindings`, throws in the script. A key name the
engine does not know (`isKeyDown("space")`) throws the same way.

For an entity a remote peer drives, actions — like keys — are read from that
peer's input, and only its **key** bindings count: peers send key state, not
sticks or buttons.

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

### Prediction

An entity can opt in to being **server-simulated with client-side prediction**,
per entity, through its `NetworkId`:

```ron
components: [
    ("bsengine_core::network_id::NetworkId", "(id: 3, authority: Predicted(peer_id: 1))"),
],
```

The difference from `Client(peer_id: 1)` is who decides. `Client` means that peer
simulates the entity and reports where it is — the server takes its word, so the
two can never disagree. `Predicted` means the peer applies its own input
immediately so the entity feels responsive, while the **server** decides what
actually happened; when they disagree the server wins.

**Both peers run the entity's movement script.** That is the point of the design:
one movement rule rather than one per side that could drift apart.
`Bsengine.isKeyPressed` reads the input of whichever entity the script is running
for, so on the server it sees that client's keys and not the keyboard of the
machine hosting the game.

**The cost:** a correction re-runs that movement script once per input the server
had not yet acknowledged — bounded by latency, not by scene size. A replay
re-derives *position only* and deliberately discards everything else the script
did: the sound that played during the original input already played, and firing
it again on every correction would make a player hear their own latency.

Server authority is also the precondition for any anti-cheat. With `Client`
authority a peer can simply state its position.

**Not implemented:** lag compensation — rewinding the server to a client's view
for hit detection.

---

## Cutscenes

A `Timeline` is a RON asset of tracks over a duration:

```ron
Timeline(
    duration: 6.0,
    tracks: [
        Camera(keys: [
            (time: 0.0, position: (0.0, 3.0, 10.0), look_at: (0.0, 0.5, 0.0)),
            (time: 3.0, position: (9.0, 4.0, 4.0), look_at: (0.0, 0.5, 0.0)),
        ]),
        CameraShot(cuts: [(time: 3.5, entity: "CloseUpShot")]),
        Animation(entity: "Subject", keys: [(time: 0.5, clip: "Survey")]),
        Event(keys: [(time: 5.5, name: "intro_over")]),
    ],
)
```

An entity plays one by carrying a `TimelinePlayer`:

```ron
components: [
    ("bsengine_core::timeline::TimelinePlayer", "(timeline: \"assets/timelines/intro.ron\", time: 0.0, playing: false, speed: 1.0)"),
],
```

and a script drives it:

```js
Bsengine.timeline.play(name);      // always from the beginning
Bsengine.timeline.stop(name);
Bsengine.timeline.isPlaying(name);
Bsengine.timeline.eventFired("intro_over");   // true only on the frame it fires
```

`games/cutscene-demo` is a working example. See it before writing one.

### The Timeline panel

The editor's **Timeline** panel draws a timeline's tracks and scrubs it.
Selecting an entity with a `TimelinePlayer` opens the timeline it names; the
path box opens any `.ron` file, and keeps winning until a different timeline
entity is selected.

**Preview** (off by default) turns the viewport into the cutscene camera at the
playhead and poses the entities the animation tracks name. It is off by default
because silently taking over the viewport would be surprising. It restores every
animation it touched when you turn it off — the opposite of the runtime's rule,
deliberately, because looking at a cutscene must not be a way to edit the scene.
The camera is never restored because it is never changed: the preview overrides
the editor's own view rather than moving the scene's camera entity.

### Editing

Click a key to select it, drag it along the time axis to retime it, and use the
strip below the tracks to edit its values. **Add Key** inserts at the playhead —
a camera key captures the editor camera's current position and target, so
framing a shot and pressing Add is the whole gesture. **Add Track** picks a
kind; **Delete Key** and **Delete Track** act on the selection. **Undo** and
**Redo** are panel buttons, **Save** writes the file, and **Revert** re-reads
it. A timeline with unsaved edits will not be swapped out from under you by
changing the selection — Save, Revert or Discard first.

Four things worth knowing before you rely on it:

- **Ctrl+Z does not undo timeline edits.** It is the editor's scene undo and
  keeps that meaning everywhere; the timeline's history is its own, on the
  panel's Undo and Redo buttons. Sharing one shortcut between two histories
  would mean two consumers of one flag, and that failure is silent.
- **Saving loses comments that sit between tracks.** RON has no comments in its
  data model, so re-serialising drops them. The block *above* the data is
  preserved verbatim, which keeps the part that says why a file exists — but a
  comment beside a particular track does not survive the first Save.
- **Saving reformats.** The writer expands each key across several lines, so a
  compactly hand-written file grows: `games/cutscene-demo`'s `intro.ron` goes
  from about 25 lines to about 48. Type names are kept (`Timeline(`, `Camera(`,
  `CameraKey(`), so the result still says what it is.
- **Saving leaves the asset's `.meta` sidecar stale** until the project is next
  run, because the sidecar records a hash of the file's bytes. That is expected;
  a scan regenerates it.

Event tracks are drawn and editable but have no effect in the editor, which runs
no gameplay scripts to receive events.

### The four track kinds

**`Camera`** moves the camera smoothly between keys. Keys carry `look_at` rather
than a rotation, because that is what composing a shot actually involves; the
direction is interpolated and the orientation rebuilt from it, so a camera
tracking a subject keeps pointing at it between keys instead of drifting off and
snapping back.

**`CameraShot`** cuts. A cut wins over a dolly at the instant it lands — a cut is
a statement about that frame, and blending into one is not a cut.

**`Animation`** starts a clip on a named entity. **`Event`** fires a name for
scripts.

### What to know before relying on it

**A shot names an ordinary entity, not a camera.** The engine renders exactly one
camera — whichever `Camera` component the renderer reaches first — so a cut
*copies* the named entity's transform and FOV onto that camera rather than
switching to it. A shot entity needs no `Camera` of its own.

**A timeline writes only the entities it names.** Everything else keeps
simulating, so playing one is not a global mode. The corollary is that a timeline
driving an entity a script also drives every frame is a race: give a cutscene its
own entities, or stop the script.

**Stopping does not restore anything.** Whatever the timeline moved stays where
it was left. Restoring would need a snapshot of arbitrary components and would
surprise the common case — a cutscene that exists to move the player somewhere.

**Events are per-frame, not queued.** `eventFired` answers about the current
frame only, so a script has to be looking on the frame a beat lands. This is
deliberate: a queue would let a script see the ending long after it happened.

**Not implemented:** an editor track view and scrubbing, blending between
overlapping timelines, sub-timelines, and audio tracks.

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

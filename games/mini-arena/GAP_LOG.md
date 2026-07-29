# Mini Action Arena — Engineering Gap Log

Friction and bugs found while building this demo and while authoring its E2E
test (`assets/tests/basic-playthrough.testlog.json`). Every entry below was
either verified directly (reproduced, root-caused, and in most cases fixed)
or is carried forward from earlier tasks in this plan with the same standard
of verification.

## Blocking

*(empty — every issue below that actually blocked this task's deliverable
was fixed immediately, as it was found, rather than left blocking.)*

## Non-blocking

### Found and fixed during this task's E2E test authoring

Writing the headless playthrough turned out to be far from a formality: the
player genuinely could not move, then could not attack, then could not kill
the Enemy, in a chain of ten distinct, independently-verified bugs. Every one
of them was reproduced with a minimal repro (usually a throwaway
`Bsengine.setHudText("debug", ...)` probe of raw engine state) before being
fixed, so this list is a real account of what was actually wrong, not a
guess.

**Engine-level (affects every game, not just this one):**

- **Reflect type registration lived only in `EditorPlugin`, unreachable by
  headless test mode.** `bsengine-runtime --test`'s `build_test_app` never
  adds `EditorPlugin` (it can't — that plugin needs a render/window stack a
  headless app doesn't have), but *every* `register_type::<T>()` call for
  gameplay components (`Shield`, `SaveData`, `AnimationStateMachine`,
  `NavMeshAgent`, `Bloom`, `ToneMap`, ...) lived inline inside
  `EditorPlugin::build`. A scene's `components: [...]` RON entries
  deserialize via the ECS type registry (`spawn_scene_entities` in
  `bsengine-scene`), so every one of those components was silently dropped
  in headless mode (logged only as a `tracing::warn!` "unknown reflected
  type path", easy to miss). Concretely: `Player`'s `Shield` never attached,
  so `Bsengine.getShield("Player")` always returned its `unwrap_or(0.0)`
  fallback, and player.js's `dead = getShield(self) <= 0.0` check was
  permanently true — the player could not move at all in headless mode. This
  also means the earlier `SaveData` reflect-registration fix (this plan's
  Task 0) was *also* silently unreachable in headless mode the whole time;
  nothing had actually exercised it headlessly before now. **Fixed** by
  extracting a shared `bsengine_scene::register_gameplay_reflect_types(app)`
  function (in `crates/bsengine-scene/src/plugin.rs`, exported from
  `lib.rs`), called from both `EditorPlugin::build`
  (`crates/bsengine-editor/src/plugin.rs`) and the headless
  `build_test_app` (`crates/bsengine-runtime/src/test_mode.rs`), so the two
  runtimes stay in parity going forward.

- **`bsengine-runtime --test`'s `PressKey`/`ReleaseKey`/`PressMouse`/
  `ReleaseMouse` commands mutated the `Input<T>` resource directly, outside
  any frame's schedule — silently breaking every edge-triggered input check
  (`isKeyDown`/`isKeyUp`, and any future `onKeyDown`/`onKeyUp` JS
  callback).** `clear_input_state` (which wipes `just_pressed`/
  `just_released` each frame) runs first in `PreUpdate`, ahead of the
  event-draining systems, specifically so real input (which arrives as
  `Events<KeyInput>` from the window layer) sees the correct
  clear-then-set order within one frame. A direct `.press()`/`.release()`
  call bypasses that event queue entirely and lands strictly *before* the
  next `Step`'s first `app.update()` — so that same `clear_input_state`
  always wiped the edge flag before any script ever saw it. In practice this
  meant `Bsengine.isKeyDown(...)` could never be observed as true through
  the test protocol, at all, no matter the command sequence — mini-arena's
  attack (Space), pause toggle (Escape), and checkpoint reload (Enter) were
  all completely untestable headlessly before this fix. Held/level state
  (`isKeyPressed`) was unaffected, which is exactly why this went unnoticed.
  **Fixed** by routing all four commands through
  `Events<KeyInput>`/`Events<MouseInput>` instead
  (`crates/bsengine-runtime/src/test_mode.rs`), matching how real input
  actually flows.

- **`NavMeshPlugin` (the plugin that actually moves `NavMeshAgent`
  entities — its one system, `navigate_agents`) was never added to
  `bsengine-runtime` at all, in *either* the windowed runtime (`main.rs`) or
  the headless test runtime (`test_mode.rs`).** The Enemy's NavMesh pursuit
  AI — one of this demo's headline features, and an item on
  `ENGINE_ROADMAP.md`'s completion checklist — has never actually functioned
  in a real running build of this game; it only worked in `bsengine-app`'s
  own unit tests, which add the plugin explicitly. **Fixed** by adding
  `.add_plugins(NavMeshPlugin)` to both entry points. This also required
  adding `.add_plugins(TimePlugin)` to the headless test app (`main.rs`
  already had it) — `navigate_agents` reads the ECS `Time` resource for its
  `dt`, which nothing previously inserted headlessly, since JS scripting
  tracks its own separate wall-clock timing (`ScriptTimingState`) instead of
  going through `Time`.

- **`Bsengine.raycast()`'s hit result serialized as `{ entity_name: ... }`,
  not `{ entityName: ... }`** — every other `Bsengine.*` JS API in this
  engine uses camelCase (`isKeyDown`, `getForwardVector`, `setHudText`, ...),
  but `RaycastHitJson` (`bsengine-scripting/src/ops.rs`) was left as plain
  Rust snake_case with no `#[serde(rename_all)]`. A caller checking
  `hit.entityName` (the only sensible name to guess, matching the engine's
  own convention) silently got `undefined` — no error anywhere, just an
  always-false comparison. This alone was enough to make melee combat
  completely non-functional regardless of aim, range, or timing. **Fixed**
  with `#[serde(rename_all = "camelCase")]` on `RaycastHitJson`.

**Content-level (specific to `games/mini-arena/assets/scripts/player.js`):**

- **Continuous WASD movement used `Bsengine.isKeyDown` (edge-triggered:
  true for exactly one frame per physical key transition) instead of
  `Bsengine.isKeyPressed` (level-triggered: true for as long as the key is
  held).** Every other game in this repo (`cube-breakout`, `cube-evader`,
  `cube-roller`, `tilt-run`) correctly uses `isKeyPressed` for held-movement
  checks; mini-arena's player.js was the one outlier. Holding W would move
  the player for at most one frame, then stop, until released and
  re-pressed. **Fixed**: `W`/`S`/`A`/`D`/`Left`/`Right` now use
  `isKeyPressed`; `Enter` (checkpoint reload) and `Space` (attack) correctly
  keep `isKeyDown`, since those really are one-shot, edge-triggered actions.

- **The yaw computed for facing (`Math.atan2(dx, dz)`) produced a forward
  vector exactly 180° backwards from the direction of travel.**
  `Bsengine.getForwardVector()` returns `rotation * (0, 0, -1)`; setting yaw
  directly from `atan2(dx, dz)` makes that come out as `(-dx, 0, -dz)`.
  Confirmed empirically: holding S+D (dx=dz=+0.71) reported
  `fwd=[-0.71,0,-0.71]`. Every consumer of "forward" — the attack raycast's
  direction, and the knockback direction sent to the Enemy on hit — was
  therefore aiming/pushing the exact opposite way the player was actually
  walking. **Fixed**: `Math.atan2(-dx, -dz)`.

- **The attack raycast's origin was placed at the player's exact position**
  — i.e., inside the player's own capsule collider (radius 0.35) — **and**
  **at `pos.y + 0.9`, above the collider's actual vertical extent** (a
  `Capsule(half_height: 0.5, radius: 0.35)` totals a 0.85 half-extent, and
  both entities sit at `y = 0`). Either bug alone made every attack miss:
  the solid-ray-cast semantics reported an immediate self-hit
  (`{entityName: "Player", distance: 0}`) at the wrong height would have
  skimmed clean over the Enemy's collider even if the self-hit weren't
  happening. **Fixed**: the origin is now offset forward (in the aim
  direction) by `SELF_RADIUS_CLEARANCE = 0.5` to clear the player's own
  collider, at `pos.y + 0.5` (inside the capsule's cylindrical midsection).

- **The kill-check read `Bsengine.getShield("Enemy")` immediately after
  calling `Bsengine.damageShield("Enemy", ...)`, in the same script call —
  but `damageShield` only *queues* a command, applied to the world after
  every script finishes that frame, while `getShield` reads a snapshot
  captured at the *start* of the frame.** The check therefore always saw the
  pre-this-hit value, one hit behind reality: with a 30-max-shield Enemy and
  15 damage per hit, it took **three** landed hits to trigger `destroy()`,
  not the documented two. **Fixed** by reading `getShield` *before* calling
  `damageShield` and computing the post-hit value locally
  (`preHitShield - ATTACK_DAMAGE <= 0.0`), which is correct because any
  *earlier* hit's damage was already flushed to the world by a prior frame.

### Genuinely new finding: driving the headless E2E protocol directly (Step 1 of this task)

The task's original design assumed an interactive MCP session
(`test_session_start`, `test_press_key`, ...); those tools weren't available
here, so I drove `bsengine-runtime --test <dir>`'s stdin/stdout JSON-line
protocol directly with a small Python script (piped subprocess, one command
per line, one JSON response per line). This worked well as a standalone
mechanism — it's genuinely just a thin, directly-scriptable protocol with no
hidden dependency on an MCP client, exactly as advertised. A few honest
observations from actually doing it:

- **The protocol itself was pleasant to drive**: newline-delimited JSON
  in/out, one response per command, no framing surprises, no MCP-specific
  assumptions anywhere in `test_protocol.rs`/`test_mode.rs`. Building an
  interactive session (persistent subprocess, read-eval-print against real
  engine state) took only a few dozen lines of Python.
- **The real friction wasn't the protocol shape, it was silent failure
  modes on the engine side of it** — see the "Engine-level" bugs above. A
  `press_key` that's silently a no-op for edge-triggered checks, a
  `components:` entry that's silently dropped, a plugin that's silently
  never registered: none of these produced any error response over the
  wire. Every response was `{"ok":true,...}`. The protocol faithfully
  reported "I did what you asked", which was true — the bug was always one
  layer below, in what the engine did (or didn't do) as a result. This
  matters for anyone else authoring a recording the same way: a clean
  `ok:true` on every command is not evidence the game is actually behaving
  correctly, only that the command was well-formed.
- **`get_transform` returning `null` for a missing entity turned out to be
  a convenient way to assert "this entity was destroyed"** (`path: "x", op:
  "==", value: null`) — there's no dedicated `not_exists`/`contains` op, but
  this reads naturally once you know `get_transform`'s null-on-missing
  contract, and this recording uses it for both the Enemy-destroyed and
  Pickup-collected checks.
- **Wall-clock-driven JS movement (`Bsengine.getDeltaTime()`, real
  `Instant::now()` deltas) makes stepped-frame counts inherently
  machine-speed-dependent**, unlike physics-driven movement (Rapier's
  `IntegrationParameters` step at a fixed *virtual* timestep regardless of
  real elapsed time — this is why `tilt-run`'s existing recordings, which
  drive a physics-based ball, don't have this issue). A `{"cmd":"step",
  "frames":900}` covers a fixed amount of *game* time only if the real CPU
  cost per frame is roughly constant; on a much faster or slower machine the
  same frame count covers a different distance. This recording's frame
  counts were empirically calibrated with real margin on this environment
  and verified to pass four consecutive replay runs, but it's worth flagging
  as a standing source of potential flakiness for any future JS-movement-
  driven (as opposed to physics-driven) headless recording.

### Pre-existing, not touched by this task

- No dedicated MCP tool for attaching `rigidbody`/`collider` to a live
  entity — only hand-authored scene RON.
- No progress-bar/health-bar UI widget — `Bsengine.ui.*` only offers
  label/button/panel/text-input, so a "health bar" has to be faked with
  plain text (as this project did) or two overlapping panels.
- `gltf:` scene-RON paths resolve relative to process CWD, not project dir,
  unlike `script:` paths. `CustomShader.path` has the same CWD-relative
  behavior despite its own doc comment claiming otherwise (a
  documentation/implementation mismatch) — see `pickup.js`'s comment on
  `Bsengine.setShader`.
- Point lights cast no shadows (only the single directional light does).
- No time/clock uniform available to custom WGSL shaders — animation of
  shader-visible values has to be driven from JS (`setEmissive`) every frame
  instead.
- No relative-move scripting op (`Bsengine.moveEntity` does not exist) —
  every scripted movement has to read current position, add a delta, and
  write it back with `setTransform`.
- No scripting op to close the game process — a pause menu's "Quit" button
  can only ask the player to close the window themselves.
- The pause menu (`pause.js`) only shows/hides a UI overlay — it does not
  actually pause the simulation. `paused` is a variable local to `pause.js`'s
  own closure; no other script, and no engine system (`run_scripts`,
  `PhysicsPlugin`, `NavMeshPlugin`), is aware of it. The Enemy keeps chasing
  and dealing contact damage, and the Player keeps responding to WASD/attack
  input, while the "Paused" panel is on screen — a real gap versus Unity/
  Unreal, where pausing typically also stops `Time.timeScale`-driven
  simulation. There's no `Bsengine.pause()`/`setTimeScale()`-style op to
  build a real pause on top of. `ENGINE_ROADMAP.md`'s completion checklist
  for this item only asks for "일시정지 메뉴 (Bsengine.ui)" (a pause *menu*,
  literally), which this satisfies — but it's worth flagging here explicitly
  since "pause menu" reasonably implies "pauses" to most readers.
- Enemy knockback is a scripted position offset, not a real Rapier impulse,
  because the enemy's `Kinematic` rigidbody (required for `NavMeshAgent` to
  own its `Transform`) ignores physics impulses by definition.

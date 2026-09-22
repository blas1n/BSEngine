# Working in BSEngine

A Rust game engine (wgpu + Bevy ECS + Rapier + deno_core/V8), with an editor, a
scripting API, and 11 playable projects under `games/`. `README.md` covers what
the engine *does*; this file covers what it costs to work on it safely.

Everything below was learned by getting it wrong first. The commands are the
ones CI actually runs — if a command here disagrees with your instinct, the
command is right.

---

## Build and test

`.cargo/config.toml` already sets `RUST_MIN_STACK` and a Windows-local
`target-dir`. **The target dir must be on the same drive letter as the cargo
registry** (`$CARGO_HOME/registry`): `v8`'s build script creates a `gn_root`
symlink only when the two differ, and creating it needs a privilege a normal
Windows account lacks, so every crate behind `deno_core` fails with

```
symlink_dir failed: Os { code: 1314, ... }   →   failed to run custom build command for `v8`
```

The checked-in `target-dir` assumes the registry is on `C:`. If `CARGO_HOME`
points elsewhere (on the machine this was written on it is `F:\.cargo`, set
as a user environment variable), override per command:

```bash
CARGO_TARGET_DIR='F:\BSEngine-target' cargo ...
```

This is a *drive* condition, not a permissions one — Developer Mode or an
elevated shell would also work, but the override is what CI's own
`CARGO_TARGET_DIR` env var already does. Check `echo $CARGO_HOME` before the
first build of a session; the failure costs a full dependency compile to
reach.

```bash
cargo build --all

# Two invocations, not one. bsengine-editor shares a process with V8 badly:
# run concatenated with the rest of the workspace, its reflection-heavy suite
# overflows the stack. Split, both pass.
cargo test --workspace --exclude bsengine-editor   # ~2,000 tests
cargo test -p bsengine-editor                      # ~940 tests

cargo +nightly fmt --all                           # nightly, see below
cargo clippy --all-targets                         # --all-targets matters
cargo run -p bsengine-catalog --bin catalog -- --check
```

**`cargo +nightly fmt`, not `cargo fmt`.** `.rustfmt.toml` uses `ignore = [...]`,
which is a nightly-only option. On stable it is silently not applied, and your
"formatted" tree then fails CI.

**`clippy --all-targets`, not `cargo build`.** `cargo build` does not compile
`#[cfg(test)]` at all, so a test module that does not compile builds green
locally and fails in CI.

**V8**: the `v8` crate downloads a large prebuilt archive. CI caches it and
points `RUSTY_V8_ARCHIVE` at the file; locally, set the same variable if you
have one already downloaded, or expect a slow first build.

### Running a game

```bash
cargo run -p bsengine-runtime -- games/mini-arena           # windowed
cargo run -p bsengine-runtime -- games/mini-arena --frames 5 # quit after N frames
cargo run -p bsengine-runtime -- --test games/mini-arena    # headless test mode
cargo run -p bsengine-runtime -- --package games/mini-arena --mode loose|pak
```

---

## Gates — what CI runs, and how to judge a run

CI runs the same job on **ubuntu-latest, windows-latest and macos-latest**:
fmt, clippy, catalog check, build, the two test invocations, every checked-in
E2E replay, and packaging of every project in both modes. Windows takes about
an hour; the other two are much faster.

### Judge by exit code, never by summary text

Two gates print the same thing whether they pass or fail:

- **clippy**: a `#[deny]` lint surfaces as `error:`, not `warning:`. Counting
  `^warning:` lines says "unchanged" while clippy is failing.
- **catalog**: `--check` prints `checked N components and M ops.` **whether or
  not there are violations.**

So print every gate's status explicitly, and never chain them with `&&` — if
the first fails you want to see the rest anyway:

```bash
cargo +nightly fmt --all;                                    echo "FMT=$?"
cargo test --workspace --exclude bsengine-editor;            echo "WS=$?"
cargo test -p bsengine-editor;                               echo "EDITOR=$?"
cargo run -p bsengine-catalog --bin catalog -- --check;      echo "CATALOG=$?"
cargo clippy --all-targets;                                  echo "CLIPPY=$?"
```

Current baselines: catalog **79 components / 287 ops**; clippy has a noisy
non-zero warning baseline, so compare *counts*, not presence.

### A green `cargo test` is not a green CI

- **E2E replays are not `cargo test`.** They are recorded gameplay logs run by
  the runtime binary. Nothing in `cargo test` touches them, so a script or
  scene change can pass the whole suite and break every recording. Run the
  loop locally before pushing changes to scripts, scenes or physics tuning.
- **`cargo test` is fail-fast.** One failing crate means later crates never
  run — their results are absent, not passing.

---

## Traps that pass silently

These all produced a green run while being wrong. They are worth memorising.

- **A filter that matches nothing still prints `ok` and exits 0.** Verified:

  ```
  $ cargo test -p bsengine-asset --lib "watcher\|identity"; echo $?
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 150 filtered out
  0
  ```

  Two ways to land here. The filter is a **substring match, not a regex**, so
  `"a\|b"` matches no test at all; and a renamed test silently stops running
  under an old filter. **Read the `passed` count, not the word `ok`** — and if
  it is smaller than you expected, that is the failure.
  (`cargo test -p bsengine-runtime --lib` is *not* an instance of this: it
  exits 101 with "no library targets", because the crate is a binary. Loud, so
  it costs you nothing.)
- **A falling test count is the only sign you measured the wrong tree.** Running
  in the wrong worktree still prints `ok`; it just runs a different branch's
  tests. Print `pwd` with every measurement, and treat "same as baseline" as
  suspicious when you expected a change.
- **GPU tests must share one device.** Each `request_device` costs real VRAM;
  a dozen device-creating tests make Windows CI fail with
  `RequestDeviceError(OutOfMemory)` — *in whichever test asked last*, so the
  failure looks unrelated to your change. Share one via `OnceLock`.
- **Performance numbers must come from `--release`.** `cargo test` builds with
  wgpu's validation layers, which cost roughly 7x. A whole optimisation effort
  was once aimed at what turned out to be validation overhead. State the
  profile whenever you state a number.
- **Compare a one-off cost against the state *after* it, not before.** A load
  spike measured against pre-load frames read as a 16ms hitch; measured against
  post-load frames it was −1.59ms. Two designs were built on the wrong reading.

---

## Testing discipline

The suite is the main thing keeping this engine honest, and the recurring
failure is not a test that fails — it is a test that passes without ever
exercising what it claims.

**Mutation-verify anything you assert.** Break the feature on purpose and watch
the test fail. Until you have seen that, a green test is evidence of nothing.
Real examples from this repo: a damping assertion passed with damping deleted;
a mutation collapsing every instance to slot 0 passed 19 suites; disabling
cascade selection passed all 7 pixel tests.

**A mutation that also breaks the observer proves nothing.** If you cannot see
the property without the code that implements it, the test is not observing the
property. Before doubting an assertion, check (1) that the mutation was fair and
(2) that the code under test *runs at all* — count it.

**Assert the fixture's premise in the same test.** The single most common defect
here is a fixture that never creates the conditions the feature needs, leaving a
green test that measured nothing: every entity at the origin, a cloth too stiff
to sag, a camera positioned so cascade 0 is always correct. Assert the premise
("the soft sheet actually sagged") next to the result.

**Ask which side is the hard one.** For most features, one of "it changed" and
"it didn't change" is trivially true. Find the assertion that a do-nothing
implementation would fail.

**Enumerate the public surface.** "Mutation-verified" is a statement about the
assertions that exist. Before calling a feature covered, list its public
surface and ask which *consumer-side* assertion covers each item.

---

## Conventions

- **Branch + PR for everything**; CI green auto-merges. Work in a git worktree
  under `.worktrees/` (gitignored).
- **Comments explain why, and name the failure they prevent.** The existing
  comments are unusually long on purpose — they are the record of what went
  wrong. Match that density; do not trim them to look tidy.
- **Component/op catalogue**: every public `#[derive(Component)]` must be
  registered for reflection (rule R1) or it is invisible to the Inspector, to
  MCP and to reflected scenes. Check ownership with
  `cargo run -p bsengine-catalog --bin catalog -- --concept <word>` *before*
  adding a component or op — the catalogue does not detect duplicates for you.
- **Reflected components**: adding a field is safe when the type derives
  `#[reflect(Default)]`; without it, old scenes break silently.
- **Follow the reference engines.** When designing a feature, look at how Unity,
  Unreal and Godot actually do it rather than inventing options. Where all three
  converge, that is the evidence; where they diverge, pick and say why.
- `docs/BSENGINE_VS_UNITY_UNREAL.md` tracks feature parity and remaining gaps.
  **Verify its claims with `grep` before acting on them** — it is a stack of
  dated snapshots and its older prose is reliably out of date.

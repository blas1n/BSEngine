This crate is a modern alternative to `web-bindgen-futures` built around `async-task`.

# Pros & Cons

**Pros:**
+ Provides both `spawn` (for `Send` futures) and `spawn_local` (for `!Send` futures).
+ `spawn` and `spawn_local` return `Task<T>` futures, which resolve to return values.
+ It's possible to cancel futures using task handles.
+ Non-send futures (which are common on web) can have send task-handles.

**Cons:**
+ Some older browser versions are not supported.

# Platform Support

This crate only supports the `wasm32-unknown-unknown` target, and makes use of various browser-specific APIs.

Enabling the `+atomics` nightly target feature automatically enables a thread-safe runtime, which works with web-workers but which may have different performance characteristics.

# Testing

This crate supports headless-browser testing using `wasm-bindgen-test`. To run the test-suite, install `wasm-pack` and run the following, substitution `<BROWSER>` for `--chrome`, `--firefox`, or `--safari` as desired.

```ignore
wasm-pack test --headless <BROWSER>
```

To test the thread-safe backend, use the nightly toolchain, and uncomment the build instruction in `.cargo/config.toml`.

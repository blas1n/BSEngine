# Bevy Mikktspace

[![License](https://img.shields.io/badge/license-MIT%2FApache%2FZlib-blue.svg)](https://github.com/bevyengine/bevy#license)
[![Crates.io](https://img.shields.io/crates/v/bevy_mikktspace.svg)](https://crates.io/crates/bevy_mikktspace)
[![Downloads](https://img.shields.io/crates/d/bevy_mikktspace.svg)](https://crates.io/crates/bevy_mikktspace)
[![Docs](https://docs.rs/bevy_mikktspace/badge.svg)](https://docs.rs/bevy_mikktspace/latest/bevy_mikktspace/)
[![Discord](https://img.shields.io/discord/691052431525675048.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/bevy)

This is a rewrite of the [Mikkelsen Tangent Space Algorithm reference implementation](https://archive.blender.org/wiki/2015/index.php/Dev:Shading/Tangent_Space_Normal_Maps/) in Rust. It is loosely based on [`mikktspace`](https://github.com/gltf-rs/mikktspace), an existing port, except `bevy_mikktspace` has:
- exact **byte-for-byte output equivalence** with the original C source under all conditions[^1].
- fully idiomatic rust with **no unsafe** code, to support building in no-unsafe contexts.
- **no dependencies**, to avoid needing perpetual dependency-version-bump releases in the future.

Requires at least Rust 1.85.1.

## Examples

### cube_tangents

Demonstrates generating tangents for a cube with 4 triangular faces per side.

```sh
cargo run --example cube_tangents
```

## Features

The original reference implementation has a couple bugs,
which are largely inconsequential in most practical applications.
However, fixing them would mean diverging from exact output equivalence,
so `bevy_mikktspace` offers features to control this behavior:

- `corrected-edge-sorting`:
  Correct a comparison in the reference's edge quicksort implementation.
  This can only differ on the last triangle in a model.
- `corrected-vertex-welding`:
  Guarantees the smallest-index vertex is chosen when welding.
  This differs from the reference on NaN vertices.

## License agreement

Licensed under either of

* Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
* MIT license
  ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))

at your option. AND parts of the code are licensed under:

* Zlib license
  [https://opensource.org/licenses/Zlib](https://opensource.org/licenses/Zlib)

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[^1]: Extensive fuzz-testing against the reference implementation revealed divergence in NaN handling in [https://github.com/gltf-rs/mikktspace](https://github.com/gltf-rs/mikktspace), which is *probably* inconsequential for practical uses.
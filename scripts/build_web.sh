#!/usr/bin/env bash
# Builds a project for the browser: a folder to serve over HTTP, holding
#
#   index.html   the page (written by `--package --mode web`)
#   game.pak     the game's data, with its manifest inside
#   pkg/         the engine: the runtime compiled to wasm32, and its JS glue
#
# the same three parts Godot's and Unity's web exports have.
#
# usage: scripts/build_web.sh <project-dir> [out-dir] [--debug]
#   out-dir defaults to <project-dir>/dist-web.
#
# Needs the wasm32 target (`rustup target add wasm32-unknown-unknown`) and
# `wasm-bindgen` at exactly the version Cargo.lock resolved -- the glue it
# writes and the wasm module must agree, and a different CLI version refuses
# the module ("rust Wasm file schema version ... this binary ...") -- which
# this script checks before the long compile, not after it.
#
# A page cannot be opened from file:// (the module and the archive are
# fetched), so serve the folder: `python -m http.server -d <out-dir>`.
set -euo pipefail

profile=release
args=()
for a in "$@"; do
  if [[ $a == --debug ]]; then profile=debug; else args+=("$a"); fi
done
project=${args[0]:?usage: scripts/build_web.sh <project-dir> [out-dir] [--debug]}
out=${args[1]:-$project/dist-web}

want=$(cargo metadata --format-version 1 --locked |
  grep -o '"name":"wasm-bindgen","version":"[^"]*"' | head -1 | sed 's/.*"version":"//; s/"$//')
have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)
if [[ "$have" != "$want" ]]; then
  echo "wasm-bindgen ${have:-is not installed}; this tree needs $want:" >&2
  echo "  cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 1
fi

flag=--release
[[ $profile == debug ]] && flag=
# The data first: it is quick, and a project that cannot be packaged should
# fail before the engine's compile, not after it.
cargo run $flag -p bsengine-runtime -- --package "$project" --mode web --out "$out"
cargo build $flag -p bsengine-runtime --target wasm32-unknown-unknown

target_dir=$(cargo metadata --format-version 1 --no-deps |
  grep -o '"target_directory":"[^"]*"' | sed 's/.*:"//; s/"$//; s/\\\\/\//g')
wasm-bindgen --target web --no-typescript --out-dir "$out/pkg" \
  "$target_dir/wasm32-unknown-unknown/$profile/bsengine-runtime.wasm"
echo "web build in $out -- serve it, e.g. python -m http.server -d \"$out\""

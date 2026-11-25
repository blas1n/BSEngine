# Patched dependencies

Everything else builds from `vendor/`, exactly as crates.io serves it. A crate
here is the crates.io source of the version `Cargo.lock` names, with a change
marked `BSEngine patch`, used through `[patch.crates-io]` in the workspace
`Cargo.toml`. A patched crate is not in `vendor/` (`cargo vendor` skips path
dependencies), and `exclude = ["patches"]` keeps it out of the workspace, so
`cargo test --workspace` and clippy do not run on it.

Each one goes as soon as upstream has the fix: bump to that release, delete the
copy here and its `[patch]` line, and re-vendor.

## wgpu-hal 30.0.1 — GLES pixel buffer left bound after a barrier

**The change.** `src/gles/queue.rs`, `C::BufferBarrier`: after the memory
barrier, unbind `PIXEL_UNPACK_BUFFER` (when the barrier bound it, for
`COPY_SRC`) and `PIXEL_PACK_BUFFER` (for `COPY_DST`).

**Why.** The barrier binds the buffer to those points and leaves it there. On a
GLES device without `GL_EXT_buffer_storage` wgpu keeps every mappable buffer --
every `write_texture` staging buffer among them -- in CPU memory, and uploads
from it with `glTexSubImage2D` given a client pointer. With a pixel unpack
buffer still bound, GL reads that pointer as an offset into the bound buffer
instead. So after the first storage-to-copy barrier, which the renderer issues
every frame, every uncompressed texture upload was lost.

**Where it showed.** The Android emulator's GLES 3.1 translator (android-35,
`-feature GLESDynamicVersion`): mini-arena's HUD drew the glyphs in egui's first
atlas upload and none added later -- "You died. Press Enter ..." came out as five
scattered letters, while the emulator's Vulkan drew it whole. Unbinding before
the frame's submit brought them back; this patch does the same where the
binding is made. Desktop GL has buffer storage, so `WGPU_BACKEND=gl` tests
cannot see it.

**Guard.** `gles_gate::tests::wgpu_hal_is_the_patched_copy` fails when
`Cargo.lock` takes wgpu-hal from the registry again: a `[patch]` whose version
stops matching is ignored with only a warning.

**Upstream.** Reported as https://github.com/gfx-rs/wgpu/issues/11115; present on
wgpu's `trunk` as of 2026-11-25.

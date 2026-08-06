//! The sidecars checked into this repository must describe the assets checked
//! in beside them.
//!
//! `measure_file` hashes an asset's raw bytes, and those hashes are committed.
//! That only works if the bytes are the same wherever the repo is checked out.
//! They were not: `.gitattributes` pinned `eol=lf` for source but not for
//! `.js`/`.ron`/`.wgsl`, so a Windows checkout got CRLF and a Linux one got LF,
//! and every committed hash had been recorded from the CRLF form. On Linux --
//! which is to say in CI -- all 28 text assets had always looked changed. The
//! only visible symptom was that running a project rewrote sidecars and dirtied
//! the working tree, which is easy to read as a quirk rather than as the
//! integrity check failing every single time.
//!
//! This test is what makes that loud. It fails on a checkout whose line
//! endings do not match what the sidecars record, which is exactly the state
//! the `.gitattributes` rules exist to prevent.

use std::path::{Path, PathBuf};

use bsengine_asset::identity::{measure_file, Sidecar, SIDECAR_EXTENSION};

/// The workspace root, found by walking up from this crate's directory.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root is two levels above crates/bsengine-asset")
        .to_path_buf()
}

/// Every `*.meta` under `games/`, recursively.
fn committed_sidecars(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            committed_sidecars(&path, found);
        } else if path.extension().and_then(|e| e.to_str()) == Some(SIDECAR_EXTENSION) {
            found.push(path);
        }
    }
}

#[test]
fn every_committed_sidecar_matches_the_asset_beside_it() {
    let games = workspace_root().join("games");
    let mut sidecars = Vec::new();
    committed_sidecars(&games, &mut sidecars);
    assert!(
        !sidecars.is_empty(),
        "found no sidecars under {}; if the games moved, this test is pointing at nothing \
         and has been passing for that reason",
        games.display()
    );

    let mut wrong = Vec::new();
    for meta in &sidecars {
        // `foo.js.meta` describes `foo.js`.
        let asset = meta.with_extension("");
        if !asset.exists() {
            wrong.push(format!("{}: no asset beside it", meta.display()));
            continue;
        }
        let sidecar = match Sidecar::read(meta) {
            Ok(Some(s)) => s,
            Ok(None) => continue,
            Err(e) => {
                wrong.push(format!("{}: unreadable ({e})", meta.display()));
                continue;
            }
        };
        let (hash, size) = measure_file(&asset).expect("read an asset that exists");
        if sidecar.hash != hash {
            wrong.push(format!(
                "{}: records {} but the asset hashes to {} (recorded size {:?}, actual {size})",
                meta.display(),
                sidecar.hash,
                hash,
                sidecar.size,
            ));
        } else if sidecar.size != Some(size) {
            wrong.push(format!(
                "{}: hash agrees but size does not -- records {:?}, asset is {size}",
                meta.display(),
                sidecar.size,
            ));
        }
    }

    assert!(
        wrong.is_empty(),
        "{} of {} committed sidecars do not describe their asset.\n\n{}\n\n\
         A whole-repo mismatch where every difference equals the file's line count is \
         a line-ending problem, not drift: check that .gitattributes still pins eol=lf \
         for .js/.ron/.wgsl/.meta and re-checkout. A single file's mismatch means the \
         asset was edited without a scan -- load its project once to refresh it.",
        wrong.len(),
        sidecars.len(),
        wrong.join("\n"),
    );
}

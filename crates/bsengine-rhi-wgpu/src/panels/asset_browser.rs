//! Asset Browser panel: scans `./assets/`, categorizes files, and lets the
//! user spawn meshes / attach scripts / load scenes by dragging or
//! double-clicking tiles. See
//! `docs/superpowers/specs/2026-07-23-editor-ui-redesign-design.md` section
//! B/C for the approved design.

use std::path::{Path, PathBuf};

/// Coarse category used for the tile icon and drag/drop behavior. No
/// `Material` variant — `bsengine-asset` has no material-asset type yet
/// (confirmed during design), so texture files are their own terminal
/// category with no attach target in this plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetKind {
    Directory,
    Scene,
    Script,
    Mesh,
    Texture,
    Other,
}

/// Categorizes a single path by extension. Directories are identified by
/// the caller (via `Path::is_dir` at scan time, see `scan_dir`) rather than
/// here, since a bare path string can't be checked against the filesystem
/// in a pure function — this function only looks at the extension string.
fn categorize_by_extension(path: &Path) -> AssetKind {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("ron") => AssetKind::Scene,
        Some("js") => AssetKind::Script,
        Some("glb") | Some("gltf") => AssetKind::Mesh,
        Some("png") | Some("jpg") | Some("jpeg") => AssetKind::Texture,
        _ => AssetKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorize_by_extension_maps_known_extensions() {
        assert_eq!(
            categorize_by_extension(Path::new("scenes/main.ron")),
            AssetKind::Scene
        );
        assert_eq!(
            categorize_by_extension(Path::new("scripts/ball.js")),
            AssetKind::Script
        );
        assert_eq!(
            categorize_by_extension(Path::new("models/rig.glb")),
            AssetKind::Mesh
        );
        assert_eq!(
            categorize_by_extension(Path::new("models/rig.gltf")),
            AssetKind::Mesh
        );
        assert_eq!(
            categorize_by_extension(Path::new("tex/grass.png")),
            AssetKind::Texture
        );
        assert_eq!(
            categorize_by_extension(Path::new("tex/grass.JPG")),
            AssetKind::Texture
        );
        assert_eq!(
            categorize_by_extension(Path::new("readme.txt")),
            AssetKind::Other
        );
        assert_eq!(
            categorize_by_extension(Path::new("no_extension")),
            AssetKind::Other
        );
    }
}

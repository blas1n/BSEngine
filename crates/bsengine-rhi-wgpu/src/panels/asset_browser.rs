//! Asset Browser panel: scans `./assets/`, categorizes files, and lets the
//! user spawn meshes / attach scripts / load scenes by dragging or
//! double-clicking tiles. See
//! `docs/superpowers/specs/2026-07-23-editor-ui-redesign-design.md` section
//! B/C for the approved design.

use bsengine_core::{EditorPanel, EditorPanelContext};
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

/// One row in the Asset Browser's tile grid or folder tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetEntry {
    pub name: String,
    pub path: PathBuf,
    pub kind: AssetKind,
    pub is_dir: bool,
}

/// Scans one directory (non-recursive — the panel navigates into
/// subdirectories rather than showing the whole tree flattened) and returns
/// its immediate children, directories first, then alphabetically within
/// each group. Unreadable entries (permission errors, races with concurrent
/// deletes) are silently skipped rather than failing the whole scan — this
/// is a best-effort UI listing, not a source of truth.
pub fn scan_dir(dir: &Path) -> Vec<AssetEntry> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<AssetEntry> = read_dir
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            let is_dir = e.file_type().ok()?.is_dir();
            let name = e.file_name().to_string_lossy().to_string();
            let kind = if is_dir {
                AssetKind::Directory
            } else {
                categorize_by_extension(&path)
            };
            Some(AssetEntry {
                name,
                path,
                kind,
                is_dir,
            })
        })
        .collect();
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    entries
}

/// Root directory the panel scans, relative to the editor process's cwd —
/// same convention `dock.rs::layout_path()` uses for `editor_layout.json`.
fn assets_root() -> PathBuf {
    PathBuf::from("assets")
}

/// Unity Project panel / Unreal Content Browser equivalent: scans
/// `assets_root()`, shows a folder tree + tile grid of the current
/// directory, and lets the user spawn meshes / attach scripts / load
/// scenes via drag-and-drop or double-click.
pub struct AssetBrowserPanel {
    root: PathBuf,
    current_dir: PathBuf,
    entries: Vec<AssetEntry>,
    scanned: bool,
}

impl Default for AssetBrowserPanel {
    fn default() -> Self {
        let root = assets_root();
        Self {
            current_dir: root.clone(),
            root,
            entries: Vec::new(),
            scanned: false,
        }
    }
}

impl EditorPanel for AssetBrowserPanel {
    fn id(&self) -> &str {
        "assets"
    }

    fn title(&self) -> String {
        "Assets".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut EditorPanelContext) {
        if !self.scanned {
            self.entries = scan_dir(&self.current_dir);
            self.scanned = true;
        }
        ui.label(format!(
            "{} entries in {:?}",
            self.entries.len(),
            self.current_dir
        ));
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

    #[test]
    fn scan_dir_lists_files_and_dirs_sorted_dirs_first() {
        let tmp = std::env::temp_dir().join("bse_asset_browser_scan_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("Models")).unwrap();
        std::fs::write(tmp.join("main.ron"), "").unwrap();
        std::fs::write(tmp.join("ball.js"), "").unwrap();

        let entries = scan_dir(&tmp);

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].name, "Models");
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].kind, AssetKind::Directory);
        assert_eq!(entries[1].name, "ball.js");
        assert_eq!(entries[1].kind, AssetKind::Script);
        assert_eq!(entries[2].name, "main.ron");
        assert_eq!(entries[2].kind, AssetKind::Scene);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn scan_dir_on_missing_directory_returns_empty() {
        let tmp = std::env::temp_dir().join("bse_asset_browser_scan_test_missing");
        std::fs::remove_dir_all(&tmp).ok();
        assert!(scan_dir(&tmp).is_empty());
    }
}

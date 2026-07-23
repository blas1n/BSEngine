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

/// Drag payload for Mesh/Script tiles dragged out of the Asset Browser onto
/// the Hierarchy or Viewport. Distinct from the `u64` entity-id payload
/// Hierarchy rows already use for reparenting — egui's drag-and-drop keys
/// drop targets by payload type, so both can coexist on the same rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetDragPayload {
    pub path: PathBuf,
    pub kind: AssetKind,
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
    /// Set by a double-clicked Scene tile; drained at the end of `ui` into
    /// `InspectorCmd::LoadScene` via `ctx.insp.cmd_queue`.
    pending_load_scene: Option<String>,
    /// In-memory cache of decoded+downsampled Texture-tile thumbnails,
    /// keyed by asset path. Cleared implicitly whenever this panel is
    /// dropped — there is no disk cache in this task.
    thumbnail_cache: std::collections::HashMap<PathBuf, egui::TextureHandle>,
    /// Cached subdirectory paths under `root`, keyed by parent directory.
    /// Rebuilt on Refresh and on navigation — NOT rescanned every frame,
    /// unlike a naive implementation of `draw_folder_tree` would do.
    tree_cache: std::collections::HashMap<PathBuf, Vec<PathBuf>>,
}

impl Default for AssetBrowserPanel {
    fn default() -> Self {
        let root = assets_root();
        Self {
            current_dir: root.clone(),
            root,
            entries: Vec::new(),
            scanned: false,
            pending_load_scene: None,
            thumbnail_cache: std::collections::HashMap::new(),
            tree_cache: std::collections::HashMap::new(),
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

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorPanelContext) {
        if !self.scanned {
            self.entries = scan_dir(&self.current_dir);
            self.scanned = true;
        }

        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                self.entries = scan_dir(&self.current_dir);
                self.tree_cache.clear();
            }
            ui.separator();
            self.draw_breadcrumb(ui);
        });
        ui.separator();

        let mut navigate_to: Option<PathBuf> = None;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(160.0);
                self.draw_folder_tree(ui, self.root.clone(), &mut navigate_to);
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for entry in self.entries.clone() {
                        self.draw_tile(ui, &entry, &mut navigate_to, ctx);
                    }
                });
            });
        });

        if let Some(dir) = navigate_to {
            self.current_dir = dir;
            self.entries = scan_dir(&self.current_dir);
            self.tree_cache.clear();
        }

        if let Some(path) = self.pending_load_scene.take() {
            ctx.insp
                .cmd_queue
                .push(bsengine_core::InspectorCmd::LoadScene { path });
        }
    }
}

impl AssetBrowserPanel {
    /// Renders `Assets / Sub / Folder` as clickable segments; clicking a
    /// segment sets `current_dir` to that ancestor and rescans.
    fn draw_breadcrumb(&mut self, ui: &mut egui::Ui) {
        let Ok(relative) = self.current_dir.strip_prefix(&self.root) else {
            ui.label(self.current_dir.to_string_lossy());
            return;
        };
        let mut path_so_far = self.root.clone();
        let mut clicked_dir: Option<PathBuf> = None;
        ui.horizontal(|ui| {
            if ui.link("Assets").clicked() {
                clicked_dir = Some(self.root.clone());
            }
            for component in relative.components() {
                path_so_far.push(component);
                ui.label("/");
                if ui
                    .link(component.as_os_str().to_string_lossy().to_string())
                    .clicked()
                {
                    clicked_dir = Some(path_so_far.clone());
                }
            }
        });
        if let Some(dir) = clicked_dir {
            self.current_dir = dir;
            self.entries = scan_dir(&self.current_dir);
        }
    }

    /// Recursively renders `dir`'s subdirectories as a `CollapsingHeader`
    /// tree, mirroring `HierarchyPanel::draw_row`'s recursion shape.
    /// Clicking a folder's label sets `*navigate_to`.
    ///
    /// Subdirectory lists are cached in `self.tree_cache` keyed by `dir`
    /// rather than rescanned every frame — the cache is only populated here
    /// (lazily, on first visit to a given `dir`) and is invalidated wholesale
    /// by `ui()` on Refresh and on navigation, matching this panel's
    /// existing "manual refresh only" design for `self.entries`.
    fn draw_folder_tree(
        &mut self,
        ui: &mut egui::Ui,
        dir: PathBuf,
        navigate_to: &mut Option<PathBuf>,
    ) {
        let subdirs: Vec<PathBuf> = if let Some(cached) = self.tree_cache.get(&dir) {
            cached.clone()
        } else {
            let subdirs: Vec<PathBuf> = scan_dir(&dir)
                .into_iter()
                .filter(|e| e.is_dir)
                .map(|e| e.path)
                .collect();
            self.tree_cache.insert(dir.clone(), subdirs.clone());
            subdirs
        };
        let label = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Assets".to_string());

        if subdirs.is_empty() {
            if ui
                .selectable_label(dir == self.current_dir, label)
                .clicked()
            {
                *navigate_to = Some(dir);
            }
            return;
        }

        egui::CollapsingHeader::new(label)
            .id_salt(dir.to_string_lossy().to_string())
            .default_open(true)
            .show(ui, |ui| {
                for sub in subdirs {
                    self.draw_folder_tree(ui, sub, navigate_to);
                }
            });
    }

    /// Loads and downsamples `path` to a 64×64 `egui::TextureHandle`,
    /// caching the result in-memory for the lifetime of this panel (cleared
    /// implicitly whenever the whole `AssetBrowserPanel` is dropped — there
    /// is no disk cache in this task). Returns `None` if the file can't be
    /// decoded (corrupt/unsupported image); the tile falls back to the
    /// fixed Texture icon in that case.
    fn thumbnail_for(&mut self, ctx: &egui::Context, path: &Path) -> Option<egui::TextureHandle> {
        if let Some(handle) = self.thumbnail_cache.get(path) {
            return Some(handle.clone());
        }
        let img = image::open(path).ok()?;
        let thumb = img.thumbnail(64, 64).to_rgba8();
        let size = [thumb.width() as usize, thumb.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, thumb.as_raw());
        let handle = ctx.load_texture(
            path.to_string_lossy().to_string(),
            color_image,
            egui::TextureOptions::default(),
        );
        self.thumbnail_cache
            .insert(path.to_path_buf(), handle.clone());
        Some(handle)
    }

    /// Fixed per-kind icon shown on a tile. Texture tiles get a real
    /// decoded-image thumbnail via `thumbnail_for`; other kinds always show
    /// this fixed icon, and Texture falls back to it when decoding fails.
    fn icon_for_kind(kind: AssetKind) -> &'static str {
        match kind {
            AssetKind::Directory => egui_phosphor::regular::FOLDER,
            AssetKind::Scene => egui_phosphor::regular::FILM_STRIP,
            AssetKind::Script => egui_phosphor::regular::FILE_JS,
            AssetKind::Mesh => egui_phosphor::regular::CUBE,
            AssetKind::Texture => egui_phosphor::regular::FILE_IMAGE,
            AssetKind::Other => egui_phosphor::regular::FILE,
        }
    }

    fn draw_tile(
        &mut self,
        ui: &mut egui::Ui,
        entry: &AssetEntry,
        navigate_to: &mut Option<PathBuf>,
        _ctx: &mut EditorPanelContext,
    ) {
        ui.vertical(|ui| {
            ui.set_width(64.0);
            let response = if entry.kind == AssetKind::Texture {
                match self.thumbnail_for(ui.ctx(), &entry.path) {
                    Some(handle) => ui.add(egui::ImageButton::new(
                        egui::Image::new(&handle).fit_to_exact_size(egui::vec2(48.0, 48.0)),
                    )),
                    None => ui.button(Self::icon_for_kind(entry.kind)),
                }
            } else {
                ui.button(Self::icon_for_kind(entry.kind))
            };
            ui.label(&entry.name);

            if entry.is_dir {
                if response.clicked() {
                    *navigate_to = Some(entry.path.clone());
                }
                return;
            }

            match entry.kind {
                AssetKind::Scene => {
                    if response.double_clicked() {
                        self.pending_load_scene = Some(entry.path.to_string_lossy().to_string());
                    }
                }
                AssetKind::Mesh | AssetKind::Script => {
                    let drag_id = ui.id().with(("asset_tile_drag", &entry.path));
                    let drag_response = ui.interact(response.rect, drag_id, egui::Sense::drag());
                    let combined = drag_response | response;
                    combined.dnd_set_drag_payload(AssetDragPayload {
                        path: entry.path.clone(),
                        kind: entry.kind,
                    });
                }
                _ => {}
            }
        });
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

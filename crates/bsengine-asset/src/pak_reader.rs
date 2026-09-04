//! Serving a packaged build's assets to `bevy_asset` out of a [`Pak`].
//!
//! # Why this backs the *default* source
//!
//! Registered as a *named* source, every `AssetServer::load` call site in the
//! engine would need a `pak://` prefix — dozens of edits, in crates that should
//! not have to know a build was packaged. Registered as the **default**, none of
//! them change at all.
//!
//! That works because `AssetSourceBuilders::init_default_source`
//! (`bevy_asset-0.14/src/io/source.rs:351`) is `get_or_insert_with`: a default
//! registered *before* `AssetPlugin` is added survives it untouched. The
//! ordering is not optional — sources are built during `AssetPlugin::build`, so
//! anything registered afterwards is ignored.

use std::path::Path;
use std::sync::Arc;

use bevy_asset::io::{
    AssetReader, AssetReaderError, ErasedAssetReader, PathStream, Reader, VecReader,
};

use crate::pak::Pak;

/// Turns a path as `AssetServer::load` received it into the key the archive
/// stores it under.
///
/// Delegates to [`crate::pak_source::archive_key`] rather than repeating the
/// rule. The two readers look into the same archive with paths built by the
/// same function, so a second copy that drifted would mean one of them silently
/// reading from disk -- which is a bug this feature actually shipped once, when
/// the scene side stripped only `./` and so missed every absolute project
/// directory.
fn archive_key(path: &Path, project_dir: &str) -> String {
    crate::pak_source::archive_key(&path.to_string_lossy(), project_dir)
}

/// Serves `bevy_asset` reads out of an opened archive.
pub struct PakAssetReader {
    pak: Arc<Pak>,
    project_dir: String,
}

impl PakAssetReader {
    /// Serves `pak`, reading load paths as relative to `project_dir` — the same
    /// string `resolve_project_path` prepends.
    pub fn new(pak: Arc<Pak>, project_dir: impl Into<String>) -> Self {
        Self {
            pak,
            project_dir: project_dir.into(),
        }
    }
}

impl AssetReader for PakAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<Box<Reader<'a>>, AssetReaderError> {
        let key = archive_key(path, &self.project_dir);
        match self.pak.get(&key) {
            Some(bytes) => Ok(Box::new(VecReader::new(bytes.to_vec()))),
            None => Err(AssetReaderError::NotFound(path.to_path_buf())),
        }
    }

    /// Always `NotFound`, which is already this engine's live behaviour:
    /// [`crate::AssetPlugin`] builds `bevy_asset` with `AssetMetaCheck::Never`
    /// because this crate's identity sidecars use the very filename `bevy_asset`
    /// reserves for its own meta. Packing them would be packing that collision.
    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<Box<Reader<'a>>, AssetReaderError> {
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }

    /// Refused rather than answered with an empty listing: nothing in this
    /// engine loads a directory, and an empty stream would read as "the
    /// directory is there and holds nothing" — the wrong answer to give about
    /// an archive that may well hold it.
    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(self.pak.has_prefix(&archive_key(path, &self.project_dir)))
    }
}

/// Builds the boxed reader `AssetSourceBuilder::with_reader` asks for.
pub fn erased(pak: Arc<Pak>, project_dir: String) -> Box<dyn ErasedAssetReader> {
    Box::new(PakAssetReader::new(pak, project_dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact shapes `resolve_project_path` produces.
    ///
    /// This is the single most likely thing in the feature to be silently
    /// wrong, and the failure is quiet in the worst way: a key that never
    /// matches sends every asset to the disk fallback, which is *present* in a
    /// source tree, so everything looks fine right up until a shipped build
    /// loads nothing.
    #[test]
    fn a_load_path_is_normalised_to_an_archive_key() {
        // project_dir "." — a build run from inside itself, the shipped case
        assert_eq!(
            archive_key(Path::new("./assets/models/fox.glb"), "."),
            "assets/models/fox.glb"
        );
        // project_dir naming a directory — the source-tree case
        assert_eq!(
            archive_key(
                Path::new("games/mini-arena/assets/x.png"),
                "games/mini-arena"
            ),
            "assets/x.png"
        );
        // Windows separators, which `Path` hands through unchanged
        assert_eq!(
            archive_key(Path::new(r".\assets\models\fox.glb"), "."),
            "assets/models/fox.glb"
        );
        // Already project-relative
        assert_eq!(archive_key(Path::new("assets/a.txt"), "."), "assets/a.txt");
        // An empty project dir is what `resolve_project_path` treats as "no
        // prefix at all", so it must behave like "."
        assert_eq!(archive_key(Path::new("assets/a.txt"), ""), "assets/a.txt");
    }

    /// A path outside the project keeps its shape rather than being mangled
    /// into a false match — the archive must not answer for what it lacks.
    #[test]
    fn an_unrelated_path_is_not_forced_to_match() {
        assert_eq!(
            archive_key(Path::new("elsewhere/x.png"), "games/mini-arena"),
            "elsewhere/x.png"
        );
    }

    /// Paired with the above: the keys really do reach entries in a real
    /// archive. Testing the string function alone would leave "the normalised
    /// key is what `Pak` stores" unproven.
    #[test]
    fn a_normalised_key_finds_the_entry_it_names() {
        let bytes =
            crate::pak::write_pak_bytes(&[("assets/models/fox.glb".to_string(), b"mesh".to_vec())])
                .expect("write");
        let pak = Pak::from_bytes(bytes).expect("open");

        assert_eq!(
            pak.get(&archive_key(Path::new("./assets/models/fox.glb"), ".")),
            Some(b"mesh".as_slice()),
            "the key a load path normalises to must be the key the cook stored"
        );
    }
}

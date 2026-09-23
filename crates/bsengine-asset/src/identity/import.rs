//! Reading and writing an asset's import settings through its sidecar, for
//! the things that edit them: the MCP `asset_import_settings` tool and the
//! editor's Inspector.
//!
//! The loaders read settings on their own (`Sidecar::read_beside_loading_asset`);
//! this module is the *writing* side, and it exists as one function because a
//! write has two things to get right that a caller would otherwise each get
//! right separately, or not: an asset that has never been scanned has no
//! sidecar yet and needs an identity minted for it -- the same way
//! [`scan`](super::scan::scan) would, so the next scan finds a sidecar it
//! agrees with -- and an asset that *has* one must keep its guid, hash and
//! former paths through the write, since an identity rewritten on a settings
//! edit is a reference broken in every scene that held it.

use super::sidecar::{sidecar_path, ImportSettings, Sidecar, SidecarError};
use super::AssetGuid;
use bsengine_core::{ModelImportSettings, TextureImportSettings};
use std::fmt;
use std::path::{Path, PathBuf};

/// Which kind of import settings a file takes, by its extension: `Texture`
/// for the image formats the texture loader serves, `Model` for glTF, and
/// `None` for everything else -- a scene, a script, a sound -- which has no
/// import settings at all.
///
/// Case-insensitive, as `scan::identify` and `watcher::reconstruct` are, so
/// `FOX.GLB` on a case-insensitive filesystem is a model here too.
pub fn import_kind_for(asset: &Path) -> Option<&'static str> {
    let extension = asset.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "hdr" => Some("Texture"),
        "glb" | "gltf" => Some("Model"),
        _ => None,
    }
}

/// The defaults for a kind named by [`import_kind_for`].
pub fn default_import_settings(kind: &str) -> Option<ImportSettings> {
    match kind {
        "Texture" => Some(ImportSettings::Texture(TextureImportSettings::default())),
        "Model" => Some(ImportSettings::Model(ModelImportSettings::default())),
        _ => None,
    }
}

/// What a read found: the settings in force for the asset, and whether the
/// sidecar actually records them or they are the kind's defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportReport {
    /// `"Texture"` or `"Model"`.
    pub kind: &'static str,
    /// The settings in force: recorded ones, or the kind's defaults.
    pub settings: ImportSettings,
    /// Whether `settings` came from the sidecar (`true`) or are the defaults
    /// because there is no sidecar, or it records none, or it records another
    /// kind's (`false`).
    pub recorded: bool,
}

/// Why a read or write could not be done.
#[derive(Debug)]
pub enum ImportError {
    /// The file's extension has no import settings.
    NoSettingsForKind(PathBuf),
    /// The asset itself is not on disk, so there is nothing to record settings
    /// for -- a sidecar beside a missing file is the orphan case the scan's
    /// recovery exists to repair, and this should not manufacture one.
    AssetMissing(PathBuf),
    /// The settings given are for another kind than the file is.
    KindMismatch {
        /// What the file is.
        file: &'static str,
        /// What the settings were for.
        given: &'static str,
    },
    /// The sidecar exists and could not be read or understood. Never
    /// overwritten -- see `Sidecar::read` for why.
    Sidecar(SidecarError),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSettingsForKind(path) => write!(
                f,
                "{} has no import settings: textures (png, jpg, jpeg, hdr) and models (glb, \
                 gltf) do",
                path.display()
            ),
            Self::AssetMissing(path) => write!(f, "{} does not exist", path.display()),
            Self::KindMismatch { file, given } => write!(
                f,
                "the file is a {file} but the settings given are a {given}'s"
            ),
            Self::Sidecar(e) => write!(
                f,
                "the sidecar exists but could not be read ({e}); fix or delete it by hand \
                 rather than have it overwritten"
            ),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<SidecarError> for ImportError {
    fn from(e: SidecarError) -> Self {
        Self::Sidecar(e)
    }
}

/// The import settings in force for `asset`.
pub fn read_import_settings(asset: &Path) -> Result<ImportReport, ImportError> {
    let kind =
        import_kind_for(asset).ok_or_else(|| ImportError::NoSettingsForKind(asset.into()))?;
    if !asset.is_file() {
        return Err(ImportError::AssetMissing(asset.into()));
    }
    let recorded = Sidecar::read(sidecar_path(asset))?
        .and_then(|sidecar| sidecar.import)
        .filter(|settings| settings.kind() == kind);
    Ok(match recorded {
        Some(settings) => ImportReport {
            kind,
            settings,
            recorded: true,
        },
        None => ImportReport {
            kind,
            settings: default_import_settings(kind).expect("a kind import_kind_for returned"),
            recorded: false,
        },
    })
}

/// Records `settings` in the sidecar beside `asset`, and returns the sidecar's
/// path.
///
/// An asset with a sidecar keeps everything else in it -- guid, hash, size,
/// former paths -- and gets the `import` field replaced. An asset without one
/// gets a sidecar minted the way a scan would mint it, so the next scan finds
/// its own work already done rather than a file it must treat as foreign.
///
/// Refuses, rather than guesses, when the settings are for another kind than
/// the file is: `Model(..)` beside a `.png` would parse fine and then be
/// ignored by the loader with a warning, which is a worse outcome than an
/// error here at the moment somebody could still fix it.
pub fn write_import_settings(
    asset: &Path,
    settings: ImportSettings,
) -> Result<PathBuf, ImportError> {
    let kind =
        import_kind_for(asset).ok_or_else(|| ImportError::NoSettingsForKind(asset.into()))?;
    if settings.kind() != kind {
        return Err(ImportError::KindMismatch {
            file: kind,
            given: settings.kind(),
        });
    }
    if !asset.is_file() {
        return Err(ImportError::AssetMissing(asset.into()));
    }
    let meta = sidecar_path(asset);
    let mut sidecar = match Sidecar::read(&meta)? {
        Some(existing) => existing,
        None => {
            let (hash, size) = super::sidecar::measure_file(asset).map_err(SidecarError::Io)?;
            Sidecar {
                guid: AssetGuid::new(),
                hash,
                size: Some(size),
                former_paths: Vec::new(),
                import: None,
            }
        }
    };
    sidecar.import = Some(settings);
    sidecar.write(&meta)?;
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::TextureFilter;

    struct ProbeDir(PathBuf);
    impl Drop for ProbeDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn probe(tag: &str) -> ProbeDir {
        let dir = std::env::temp_dir().join(format!(
            "bsengine-import-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        ProbeDir(dir)
    }

    fn model(settings: ModelImportSettings) -> ImportSettings {
        ImportSettings::Model(settings)
    }

    fn texture(settings: TextureImportSettings) -> ImportSettings {
        ImportSettings::Texture(settings)
    }

    #[test]
    fn kinds_follow_the_loaders_extensions_case_insensitively() {
        for (name, kind) in [
            ("a.png", Some("Texture")),
            ("a.JPG", Some("Texture")),
            ("a.jpeg", Some("Texture")),
            ("a.hdr", Some("Texture")),
            ("a.glb", Some("Model")),
            ("a.GLTF", Some("Model")),
            ("a.ron", None),
            ("a.js", None),
            ("a.wav", None),
            ("a", None),
        ] {
            assert_eq!(import_kind_for(Path::new(name)), kind, "{name}");
        }
    }

    /// The identity is the reason the sidecar exists; a settings edit must
    /// not touch it. Every field but `import` is asserted unchanged, and the
    /// premise -- that the sidecar had non-trivial values to lose -- is in
    /// the fixture: a former path, a size, a hash a re-mint would not repeat.
    #[test]
    fn writing_settings_keeps_the_existing_identity_intact() {
        let probe = probe("keep-identity");
        let asset = probe.0.join("fox.glb");
        std::fs::write(&asset, b"not really a glb").unwrap();
        let before = Sidecar {
            guid: AssetGuid::new(),
            hash: "blake3:deadbeef".to_string(),
            size: Some(16),
            former_paths: vec!["assets/models/old_fox.glb".to_string()],
            import: None,
        };
        before.write(sidecar_path(&asset)).unwrap();

        let meta = write_import_settings(
            &asset,
            model(ModelImportSettings {
                scale: 0.01,
                import_animations: true,
            }),
        )
        .unwrap();
        assert_eq!(meta, sidecar_path(&asset));

        let after = Sidecar::read(&meta).unwrap().expect("still there");
        assert_eq!(
            after.guid, before.guid,
            "the guid must survive a settings edit"
        );
        assert_eq!(after.hash, before.hash);
        assert_eq!(after.size, before.size);
        assert_eq!(after.former_paths, before.former_paths);
        assert_eq!(after.model_import().scale, 0.01);
    }

    /// The other half: an asset nobody has scanned gets a sidecar that a
    /// scan would agree with -- its hash and size are the file's, so the
    /// next scan does not re-mint or re-hash it.
    #[test]
    fn writing_settings_for_an_unscanned_asset_mints_a_sidecar_the_scan_agrees_with() {
        let probe = probe("mint");
        let asset = probe.0.join("tex.png");
        std::fs::write(&asset, b"pretend png bytes").unwrap();
        assert!(!sidecar_path(&asset).exists(), "premise: no sidecar yet");

        write_import_settings(
            &asset,
            texture(TextureImportSettings {
                srgb: false,
                ..Default::default()
            }),
        )
        .unwrap();

        let minted = Sidecar::read(sidecar_path(&asset))
            .unwrap()
            .expect("minted");
        let (hash, size) = super::super::sidecar::measure_file(&asset).unwrap();
        assert_eq!(
            minted.hash, hash,
            "the hash must be the file's, as a scan records it"
        );
        assert_eq!(minted.size, Some(size));
        assert!(!minted.texture_import().srgb);
    }

    #[test]
    fn reading_reports_defaults_until_something_is_recorded() {
        let probe = probe("read");
        let asset = probe.0.join("tex.png");
        std::fs::write(&asset, b"pretend png bytes").unwrap();

        let fresh = read_import_settings(&asset).unwrap();
        assert_eq!(fresh.kind, "Texture");
        assert!(!fresh.recorded);
        assert_eq!(fresh.settings, texture(TextureImportSettings::default()));

        write_import_settings(
            &asset,
            texture(TextureImportSettings {
                filter: TextureFilter::Nearest,
                ..Default::default()
            }),
        )
        .unwrap();
        let tuned = read_import_settings(&asset).unwrap();
        assert!(tuned.recorded);
        assert_eq!(
            tuned.settings,
            texture(TextureImportSettings {
                filter: TextureFilter::Nearest,
                ..Default::default()
            })
        );
    }

    #[test]
    fn a_sidecar_of_another_kind_reads_as_unrecorded_defaults() {
        let probe = probe("other-kind");
        let asset = probe.0.join("fox.glb");
        std::fs::write(&asset, b"glb").unwrap();
        Sidecar {
            guid: AssetGuid::new(),
            hash: "blake3:x".to_string(),
            size: Some(3),
            former_paths: Vec::new(),
            import: Some(texture(TextureImportSettings::default())),
        }
        .write(sidecar_path(&asset))
        .unwrap();
        let report = read_import_settings(&asset).unwrap();
        assert_eq!(report.kind, "Model");
        assert!(!report.recorded);
        assert_eq!(report.settings, model(ModelImportSettings::default()));
    }

    #[test]
    fn the_wrong_kind_of_settings_is_refused_before_anything_is_written() {
        let probe = probe("mismatch");
        let asset = probe.0.join("fox.glb");
        std::fs::write(&asset, b"glb").unwrap();
        let err = write_import_settings(&asset, texture(TextureImportSettings::default()))
            .expect_err("a texture's settings on a model");
        assert!(
            matches!(
                err,
                ImportError::KindMismatch {
                    file: "Model",
                    given: "Texture"
                }
            ),
            "{err}"
        );
        assert!(
            !sidecar_path(&asset).exists(),
            "nothing must have been written"
        );
    }

    #[test]
    fn files_without_import_settings_and_missing_files_are_errors() {
        let probe = probe("errors");
        let scene = probe.0.join("main.ron");
        std::fs::write(&scene, b"()").unwrap();
        assert!(matches!(
            read_import_settings(&scene),
            Err(ImportError::NoSettingsForKind(_))
        ));
        assert!(matches!(
            write_import_settings(&scene, model(ModelImportSettings::default())),
            Err(ImportError::NoSettingsForKind(_))
        ));
        let missing = probe.0.join("nope.png");
        assert!(matches!(
            read_import_settings(&missing),
            Err(ImportError::AssetMissing(_))
        ));
        assert!(matches!(
            write_import_settings(&missing, texture(TextureImportSettings::default())),
            Err(ImportError::AssetMissing(_))
        ));
        assert!(
            !sidecar_path(&missing).exists(),
            "no sidecar may be minted beside a file that is not there"
        );
    }

    /// The scan's rule, kept here: a sidecar that will not parse is left
    /// alone, because overwriting it could discard an identity references
    /// still point at.
    #[test]
    fn a_broken_sidecar_is_an_error_and_is_left_exactly_as_it_was() {
        let probe = probe("broken");
        let asset = probe.0.join("tex.png");
        std::fs::write(&asset, b"png").unwrap();
        let meta = sidecar_path(&asset);
        std::fs::write(&meta, b"(guid: \"not a guid\", hash: 3)").unwrap();
        let err = write_import_settings(&asset, texture(TextureImportSettings::default()))
            .expect_err("a broken sidecar");
        assert!(matches!(err, ImportError::Sidecar(_)), "{err}");
        assert_eq!(
            std::fs::read(&meta).unwrap(),
            b"(guid: \"not a guid\", hash: 3)",
            "the broken sidecar must be untouched"
        );
        assert!(matches!(
            read_import_settings(&asset),
            Err(ImportError::Sidecar(_))
        ));
    }
}

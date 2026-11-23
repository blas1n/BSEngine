//! `project.toml`'s `[android]` table: what an Android build is called, which
//! version it is, and the key it is signed with.
//!
//! ```toml
//! [android]
//! package = "com.example.mygame"   # default com.bsengine.<project name>
//! version_code = 3                 # default 1; must grow with every upload
//! version_name = "1.2"             # default "1.0"; what the store shows
//! keystore = "keys/release.jks"    # project-relative; absent = debug key
//! key_alias = "release"
//! graphics_apis = ["vulkan", "gles"]  # tried in order; the default
//! ```
//!
//! The passwords are never in the project: `scripts/build_android.sh` reads
//! them from `BSENGINE_ANDROID_KEYSTORE_PASSWORD` (and
//! `BSENGINE_ANDROID_KEY_PASSWORD`, which defaults to it). Unity keeps the
//! keystore path and alias in Player Settings and asks for the passwords
//! each session; Godot keeps path and user in the export preset and takes the
//! password from `GODOT_ANDROID_KEYSTORE_RELEASE_PASSWORD`. A file checked in
//! with the game is the wrong place for a secret that can publish it.
//!
//! Validated by `--package` (every mode, so a mistake is found long before
//! anyone builds for Android) and written out by `--mode android` as
//! `android.properties` for the build script, which used to `sed` the name
//! out of `project.toml` and fix the version at 1.

use serde::Deserialize;
use std::path::Path;

/// The highest `versionCode` Google Play accepts.
const MAX_VERSION_CODE: u32 = 2_100_000_000;

/// `project.toml`'s `[android]` table; every field optional.
///
/// `deny_unknown_fields`, unlike the other tables: a misspelt `keystore`
/// would otherwise fall back to the debug key without a word, and a release
/// signed with it cannot be updated by one signed properly -- the store ties
/// an app to its first key for good.
#[derive(Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AndroidSection {
    /// The application id. Default `com.bsengine.<project name>`.
    #[serde(default)]
    pub package: Option<String>,
    /// The internal version, which every upload must raise. Default 1.
    #[serde(default)]
    pub version_code: Option<u32>,
    /// The version players see. Default "1.0".
    #[serde(default)]
    pub version_name: Option<String>,
    /// The release keystore, relative to the project. Absent: debug key.
    #[serde(default)]
    pub keystore: Option<String>,
    /// The key in [`Self::keystore`] to sign with.
    #[serde(default)]
    pub key_alias: Option<String>,
    /// The graphics APIs to try, first to last (`"vulkan"`, `"gles"`).
    /// Absent: `["vulkan", "gles"]`. See [`bsengine_core::GraphicsApis`].
    #[serde(default)]
    pub graphics_apis: Option<Vec<bsengine_core::GraphicsApi>>,
}

/// What `--mode android` hands the build script, resolved and validated.
#[derive(Debug, Clone, PartialEq)]
pub struct AndroidBuild {
    /// The application id, valid as one.
    pub package: String,
    /// `versionCode`, 1 to [`MAX_VERSION_CODE`].
    pub version_code: u32,
    /// `versionName`.
    pub version_name: String,
    /// The release key: the keystore's absolute path and the alias.
    pub signing: Option<(String, String)>,
}

impl AndroidSection {
    /// The graphics APIs to try, in order: [`Self::graphics_apis`] or the
    /// default. Read by every build, not only Android's -- the renderer
    /// ignores it elsewhere.
    pub fn graphics_apis(&self) -> bsengine_core::GraphicsApis {
        self.graphics_apis
            .clone()
            .map(bsengine_core::GraphicsApis)
            .unwrap_or_default()
    }

    /// Resolves the defaults against the project and checks every value,
    /// listing all that are wrong rather than the first.
    ///
    /// `for_android`: whether this is an Android build, the only one that
    /// needs the keystore *file*. Every other build checks the values but not
    /// that the key is on this machine -- a release keystore is usually kept
    /// out of the repository, and a desktop build must not fail for want of
    /// it.
    pub fn resolve(
        &self,
        project_name: &str,
        project_dir: &Path,
        for_android: bool,
    ) -> Result<AndroidBuild, String> {
        let mut problems = Vec::new();

        let package = match &self.package {
            Some(p) => {
                if let Err(e) = check_package(p) {
                    problems.push(format!("package = {p:?}: {e}"));
                }
                p.clone()
            }
            None => default_package(project_name),
        };

        let version_code = self.version_code.unwrap_or(1);
        if !(1..=MAX_VERSION_CODE).contains(&version_code) {
            problems.push(format!(
                "version_code = {version_code}: must be 1 to {MAX_VERSION_CODE} (Google Play's limit)"
            ));
        }

        let version_name = self.version_name.clone().unwrap_or_else(|| "1.0".into());
        if version_name.trim().is_empty() || version_name.contains(['\n', '\r']) {
            problems.push(format!(
                "version_name = {version_name:?}: must be one non-empty line"
            ));
        }

        let signing = match (&self.keystore, &self.key_alias) {
            (None, None) => None,
            (Some(_), None) | (None, Some(_)) => {
                problems.push(
                    "keystore and key_alias go together: name both to sign with a release \
                     key, or neither for the debug key"
                        .into(),
                );
                None
            }
            (Some(keystore), Some(alias)) => {
                let path = project_dir.join(keystore);
                if for_android && !path.is_file() {
                    problems.push(format!(
                        "keystore = {keystore:?}: no such file ({})",
                        path.display()
                    ));
                }
                if alias.trim().is_empty() {
                    problems.push("key_alias is empty".into());
                }
                let absolute = std::path::absolute(&path).unwrap_or(path);
                Some((absolute.to_string_lossy().replace('\\', "/"), alias.clone()))
            }
        };

        if let Some(apis) = &self.graphics_apis {
            if apis.is_empty() {
                problems.push(
                    "graphics_apis = []: name at least one of \"vulkan\" and \"gles\", or leave \
                     it out for both"
                        .into(),
                );
            }
            for (i, api) in apis.iter().enumerate() {
                if apis[..i].contains(api) {
                    problems.push(format!("graphics_apis names {api:?} twice"));
                }
            }
        }

        if problems.is_empty() {
            Ok(AndroidBuild {
                package,
                version_code,
                version_name,
                signing,
            })
        } else {
            Err(problems.join("\n"))
        }
    }
}

impl AndroidBuild {
    /// `android.properties`: one `key=value` per line, for
    /// `scripts/build_android.sh`. `label` (the app's name under its icon)
    /// and `version_name` are XML-escaped here, for the manifest the script
    /// writes; nothing in a value can contain a newline (checked above, and
    /// a package or path cannot).
    pub fn properties(&self, label: &str) -> String {
        let mut out = format!(
            "package={}\nversion_code={}\nversion_name_xml={}\nlabel_xml={}\n",
            self.package,
            self.version_code,
            xml_escape(&self.version_name),
            xml_escape(&label.replace(['\n', '\r'], " ")),
        );
        if let Some((keystore, alias)) = &self.signing {
            out.push_str(&format!("keystore={keystore}\nkey_alias={alias}\n"));
        }
        out
    }
}

/// `com.bsengine.<id>`, `<id>` the project name lower-cased with everything
/// but letters and digits made `_` -- what the build script always made, so
/// an installed build is still the same app.
pub fn default_package(project_name: &str) -> String {
    let mut id: String = project_name
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '_'
            }
        })
        .collect();
    if id.is_empty() || id.starts_with(|c: char| c.is_ascii_digit()) {
        id.insert(0, '_');
    }
    format!("com.bsengine.{id}")
}

/// Android's rule for an application id: two or more dot-separated parts,
/// each a letter followed by letters, digits or underscores.
fn check_package(p: &str) -> Result<(), &'static str> {
    let parts: Vec<&str> = p.split('.').collect();
    if parts.len() < 2 {
        return Err("needs at least two parts, like com.example.game");
    }
    for part in parts {
        let mut chars = part.chars();
        match chars.next() {
            Some(c) if c.is_ascii_alphabetic() => {}
            _ => return Err("each part must start with a letter"),
        }
        if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err("only letters, digits and _ are allowed in a part");
        }
    }
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(toml_text: &str) -> Result<AndroidSection, String> {
        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(default)]
            android: AndroidSection,
        }
        toml::from_str::<Wrapper>(toml_text)
            .map(|w| w.android)
            .map_err(|e| e.to_string())
    }

    /// No table is the build the script always made: the old package name
    /// (so an installed copy updates in place), version 1, debug key.
    #[test]
    fn no_table_is_the_build_there_always_was() {
        let built = AndroidSection::default()
            .resolve("Mini Action Arena", Path::new("."), true)
            .unwrap();
        assert_eq!(built.package, "com.bsengine.mini_action_arena");
        assert_eq!(built.version_code, 1);
        assert_eq!(built.version_name, "1.0");
        assert_eq!(built.signing, None);
        assert_eq!(default_package("3D Racer"), "com.bsengine._3d_racer");
    }

    #[test]
    fn every_setting_reaches_the_build() {
        let dir = std::env::temp_dir().join(format!("bse_android_ks_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("keys")).unwrap();
        std::fs::write(dir.join("keys/release.jks"), b"not checked here").unwrap();
        let built = section(
            "[android]\npackage = \"com.studio.game\"\nversion_code = 7\n\
             version_name = \"1.2 & more\"\nkeystore = \"keys/release.jks\"\n\
             key_alias = \"upload\"\n",
        )
        .unwrap()
        .resolve("Game", &dir, true)
        .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(built.package, "com.studio.game");
        assert_eq!(built.version_code, 7);
        let (keystore, alias) = built.signing.clone().expect("a release key");
        assert!(keystore.ends_with("keys/release.jks") && Path::new(&keystore).is_absolute());
        assert_eq!(alias, "upload");

        let props = built.properties("Tom & Jerry's \"Game\"");
        assert!(props.contains("package=com.studio.game\n"));
        assert!(props.contains("version_code=7\n"));
        assert!(props.contains("version_name_xml=1.2 &amp; more\n"));
        assert!(props.contains("label_xml=Tom &amp; Jerry&apos;s &quot;Game&quot;\n"));
        assert!(props.contains(&format!("keystore={keystore}\nkey_alias=upload\n")));
    }

    /// The debug-key build writes no signing lines, which is how the script
    /// knows to use the debug key.
    #[test]
    fn a_debug_key_build_names_no_keystore() {
        let props = AndroidSection::default()
            .resolve("Game", Path::new("."), true)
            .unwrap()
            .properties("Game");
        assert!(!props.contains("keystore="), "{props}");
        assert!(!props.contains("key_alias="), "{props}");
    }

    /// A misspelt key is an error, not a silent debug-key build.
    #[test]
    fn a_misspelt_key_is_refused() {
        let err = section("[android]\nkeystore_path = \"keys/release.jks\"\n").unwrap_err();
        assert!(err.contains("keystore_path"), "{err}");
    }

    /// Every bad value is reported, all at once.
    #[test]
    fn bad_values_are_all_reported() {
        let bad = AndroidSection {
            package: Some("game".into()),
            version_code: Some(0),
            version_name: Some(" ".into()),
            keystore: Some("missing.jks".into()),
            key_alias: None,
            graphics_apis: Some(vec![
                bsengine_core::GraphicsApi::Gles,
                bsengine_core::GraphicsApi::Gles,
            ]),
        };
        let err = bad.resolve("Game", Path::new("."), true).unwrap_err();
        for expected in [
            "package",
            "version_code",
            "version_name",
            "go together",
            "twice",
        ] {
            assert!(err.contains(expected), "no {expected} problem in:\n{err}");
        }

        let missing = AndroidSection {
            keystore: Some("no-such.jks".into()),
            key_alias: Some("k".into()),
            ..Default::default()
        };
        let err = missing.resolve("Game", Path::new("."), true).unwrap_err();
        assert!(err.contains("no such file"), "{err}");
        // Not an Android build: the key need not be on this machine.
        assert!(missing.resolve("Game", Path::new("."), false).is_ok());

        for (p, ok) in [
            ("com.example.game", true),
            ("com.example", true),
            ("com.Example.g4me_x", true),
            ("com", false),
            ("com.1game", false),
            ("com..game", false),
            ("com.ga-me", false),
            ("com.example.", false),
        ] {
            assert_eq!(check_package(p).is_ok(), ok, "{p}");
        }
        assert!(AndroidSection {
            version_code: Some(MAX_VERSION_CODE + 1),
            ..Default::default()
        }
        .resolve("Game", Path::new("."), true)
        .is_err());
    }

    /// `graphics_apis`: the default tries Vulkan, then GLES; a list is taken
    /// in its order; an empty list and an API that does not exist are refused
    /// when the project is read, not when a phone fails to draw.
    #[test]
    fn graphics_apis_are_read_in_order_and_checked() {
        use bsengine_core::GraphicsApi::{Gles, Vulkan};
        assert_eq!(
            AndroidSection::default().graphics_apis().0,
            vec![Vulkan, Gles]
        );

        let gles_only = section("[android]\ngraphics_apis = [\"gles\"]\n").unwrap();
        assert_eq!(gles_only.graphics_apis().0, vec![Gles]);
        let reversed = section("[android]\ngraphics_apis = [\"gles\", \"vulkan\"]\n").unwrap();
        assert_eq!(reversed.graphics_apis().0, vec![Gles, Vulkan]);
        assert!(reversed.resolve("Game", Path::new("."), true).is_ok());

        let empty = section("[android]\ngraphics_apis = []\n").unwrap();
        let err = empty.resolve("Game", Path::new("."), true).unwrap_err();
        assert!(err.contains("at least one"), "{err}");

        let err = section("[android]\ngraphics_apis = [\"metal\"]\n").unwrap_err();
        assert!(err.contains("metal"), "{err}");
    }
}

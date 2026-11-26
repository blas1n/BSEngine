//! `project.toml`'s `[ios]` table: what an iOS build is called, which version
//! it is, and the Apple team that signs it.
//!
//! ```toml
//! [ios]
//! bundle_id = "com.example.mygame"  # default com.bsengine.<project name>
//! version = "1.2"                   # default "1.0"; what the App Store shows
//! build = "7"                       # default "1"; must grow with every upload
//! team_id = "ABCDE12345"            # optional; the Xcode project signs with it
//! ```
//!
//! The same four every reference engine asks for: Unity's Bundle Identifier,
//! Version, Build and Signing Team ID; Godot's `bundle_identifier`,
//! `short_version`, `version` and `app_store_team_id`; Unreal's Bundle
//! Identifier, Version Info and IOS Team ID. No certificate or profile is
//! named here: signing for a device is done by Xcode with the team's own
//! keychain, as in all three.
//!
//! Validated by `--package` in every mode, as `[android]` is
//! (`android_settings.rs`), so a bundle id the store would refuse is found
//! at the first desktop build; written out by `--mode ios` as
//! `ios.properties` for `scripts/build_ios.sh`. The rules are here rather
//! than in the script because they are easy to get wrong in shell and easy to
//! test here.

use serde::Deserialize;

/// Apple's limit on a `CFBundleIdentifier`'s length.
const MAX_BUNDLE_ID_LEN: usize = 155;

/// `project.toml`'s `[ios]` table; every field optional.
///
/// `deny_unknown_fields`, as `[android]`: a misspelt `bundle_id` would
/// otherwise build under the default id without a word, and an app uploaded
/// under one id cannot be moved to another.
#[derive(Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IosSection {
    /// `CFBundleIdentifier`. Default `com.bsengine.<project name>`.
    #[serde(default)]
    pub bundle_id: Option<String>,
    /// `CFBundleShortVersionString`, the version players see. Default "1.0".
    #[serde(default)]
    pub version: Option<String>,
    /// `CFBundleVersion`, which every upload of a version must raise.
    /// Default "1".
    #[serde(default)]
    pub build: Option<String>,
    /// The Apple Developer team id the Xcode project signs with
    /// (`DEVELOPMENT_TEAM`). Absent: the team is picked in Xcode.
    #[serde(default)]
    pub team_id: Option<String>,
}

/// What `--mode ios` hands the build script, resolved and validated.
#[derive(Debug, Clone, PartialEq)]
pub struct IosBuild {
    /// `CFBundleIdentifier`, valid as one.
    pub bundle_id: String,
    /// `CFBundleShortVersionString`.
    pub version: String,
    /// `CFBundleVersion`.
    pub build: String,
    /// `DEVELOPMENT_TEAM`, if the project names one.
    pub team_id: Option<String>,
}

impl IosSection {
    /// Resolves the defaults and checks every value, listing all that are
    /// wrong rather than the first.
    pub fn resolve(&self, project_name: &str) -> Result<IosBuild, String> {
        let mut problems = Vec::new();

        let bundle_id = match &self.bundle_id {
            Some(id) => {
                if let Err(e) = check_bundle_id(id) {
                    problems.push(format!("bundle_id = {id:?}: {e}"));
                }
                id.clone()
            }
            None => default_bundle_id(project_name),
        };

        let version = self.version.clone().unwrap_or_else(|| "1.0".into());
        if !is_version_number(&version) {
            problems.push(format!(
                "version = {version:?}: must be one to three numbers joined by dots, \
                 such as \"1.2\" (App Store Connect refuses anything else)"
            ));
        }

        let build = self.build.clone().unwrap_or_else(|| "1".into());
        if !is_version_number(&build) {
            problems.push(format!(
                "build = {build:?}: must be one to three numbers joined by dots, such as \"7\""
            ));
        }

        if let Some(team) = &self.team_id {
            let ok = team.len() == 10
                && team
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
            if !ok {
                problems.push(format!(
                    "team_id = {team:?}: an Apple team id is ten capital letters and digits \
                     (Membership details on developer.apple.com)"
                ));
            }
        }

        if problems.is_empty() {
            Ok(IosBuild {
                bundle_id,
                version,
                build,
                team_id: self.team_id.clone(),
            })
        } else {
            Err(problems.join("\n"))
        }
    }
}

impl IosBuild {
    /// The `ios.properties` text: `key=value` lines the script reads. The
    /// name goes into `Info.plist` as XML text, so it is escaped here, and a
    /// line break, which would end the property, is a space. Nothing else can
    /// hold either (checked by [`IosSection::resolve`]).
    pub fn properties(&self, project_name: &str) -> String {
        let name = project_name
            .replace(['\n', '\r'], " ")
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let mut out = format!(
            "bundle_id={}\nversion={}\nbuild={}\nname_xml={name}\n",
            self.bundle_id, self.version, self.build
        );
        if let Some(team) = &self.team_id {
            out.push_str(&format!("team_id={team}\n"));
        }
        out
    }
}

/// `com.bsengine.<id>`, `<id>` the project name lower-cased with every run of
/// anything but letters and digits made one `-`, and none at either end.
///
/// Not Android's `default_package`: an iOS bundle id may hold only letters,
/// digits, `-` and `.` (Apple's `CFBundleIdentifier`), so Android's `_` would
/// be refused at install ("bundle identifier ... is invalid"). A name with
/// nothing usable in it becomes `game`.
pub fn default_bundle_id(project_name: &str) -> String {
    let mut id = String::new();
    for c in project_name.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            id.push(c);
        } else if !id.is_empty() && !id.ends_with('-') {
            id.push('-');
        }
    }
    while id.ends_with('-') {
        id.pop();
    }
    if id.is_empty() {
        id.push_str("game");
    }
    format!("com.bsengine.{id}")
}

/// Apple's rule for a `CFBundleIdentifier`: letters, digits, `-` and `.`,
/// at most 155 of them. Plus what every store id is: reverse-DNS, two or more
/// parts and none empty.
fn check_bundle_id(id: &str) -> Result<(), &'static str> {
    if id.len() > MAX_BUNDLE_ID_LEN {
        return Err("longer than Apple's 155 characters");
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
    {
        return Err("only letters, digits, '-' and '.' are allowed ('_' is not)");
    }
    let parts: Vec<&str> = id.split('.').collect();
    if parts.len() < 2 || parts.iter().any(|p| p.is_empty()) {
        return Err("must be reverse-DNS: two or more parts joined by single dots");
    }
    Ok(())
}

/// One to three non-negative integers joined by dots: what App Store Connect
/// takes for both `CFBundleShortVersionString` and `CFBundleVersion`.
fn is_version_number(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    (1..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(toml_text: &str) -> IosSection {
        toml::from_str(toml_text).expect("parses")
    }

    #[test]
    fn a_bundle_id_holds_only_what_ios_accepts() {
        assert_eq!(
            default_bundle_id("Mini Action Arena"),
            "com.bsengine.mini-action-arena"
        );
        assert_eq!(default_bundle_id("3D  Racer!"), "com.bsengine.3d-racer");
        assert_eq!(
            default_bundle_id("__cube_evader__"),
            "com.bsengine.cube-evader"
        );
        assert_eq!(default_bundle_id("게임"), "com.bsengine.game");
        for name in ["Mini Action Arena", "a_b", "x.y", " lead", "trail ", "日本"] {
            let id = default_bundle_id(name);
            assert!(check_bundle_id(&id).is_ok(), "{name:?} -> {id:?}");
            assert!(!id.contains("--") && !id.ends_with('-'), "{id:?}");
        }
    }

    /// No table: the build every iOS build was before the table existed --
    /// the default id, "1.0", build "1", no team.
    #[test]
    fn no_table_is_the_build_there_always_was() {
        let build = IosSection::default().resolve("Mini & Arena").unwrap();
        assert_eq!(
            build,
            IosBuild {
                bundle_id: "com.bsengine.mini-arena".into(),
                version: "1.0".into(),
                build: "1".into(),
                team_id: None,
            }
        );
        assert_eq!(
            build.properties("Mini & Arena"),
            "bundle_id=com.bsengine.mini-arena\nversion=1.0\nbuild=1\n\
             name_xml=Mini &amp; Arena\n"
        );
    }

    /// Each field reaches the properties, each checked on its own line, so a
    /// field read into the wrong key fails here.
    #[test]
    fn every_setting_reaches_the_build() {
        let build = section(
            "bundle_id = \"com.studio.Game-2\"\nversion = \"2.10.3\"\nbuild = \"41\"\n\
             team_id = \"AB12CD34EF\"\n",
        )
        .resolve("Tom & <Jerry>\nII")
        .unwrap();
        let props = build.properties("Tom & <Jerry>\nII");
        assert_eq!(
            props.lines().collect::<Vec<_>>(),
            [
                "bundle_id=com.studio.Game-2",
                "version=2.10.3",
                "build=41",
                "name_xml=Tom &amp; &lt;Jerry&gt; II",
                "team_id=AB12CD34EF",
            ]
        );
    }

    #[test]
    fn a_misspelt_key_is_refused() {
        let err = toml::from_str::<IosSection>("bundle_identifier = \"com.a.b\"\n")
            .expect_err("an unknown key");
        assert!(err.to_string().contains("bundle_identifier"), "{err}");
    }

    /// Every bad value is named, all at once, and each kind of mistake on its
    /// own: an id with `_` (Android's habit), one part, an empty part; a
    /// version with a letter or four numbers; a team id of the wrong length.
    #[test]
    fn bad_values_are_all_reported() {
        let err = section(
            "bundle_id = \"com.studio.my_game\"\nversion = \"1.0b\"\nbuild = \"1.2.3.4\"\n\
             team_id = \"abc\"\n",
        )
        .resolve("x")
        .expect_err("four bad values");
        for name in ["bundle_id", "version", "build", "team_id"] {
            assert!(
                err.contains(&format!("{name} = ")),
                "{name} not named: {err}"
            );
        }
        assert_eq!(err.lines().count(), 4, "{err}");

        for id in ["game", "com..game", ".com.game", "com.game.", "com.ga me"] {
            assert!(check_bundle_id(id).is_err(), "{id:?} accepted");
        }
        assert!(check_bundle_id(&format!("com.{}", "a".repeat(152))).is_err());
        assert!(check_bundle_id(&format!("com.{}", "a".repeat(151))).is_ok());

        for v in ["1", "1.0", "1.2.3", "10.20.30"] {
            assert!(is_version_number(v), "{v:?} refused");
        }
        for v in ["", "1.", ".1", "1..2", "v1", "1.2.3.4", "1.-2", "１"] {
            assert!(!is_version_number(v), "{v:?} accepted");
        }
    }
}

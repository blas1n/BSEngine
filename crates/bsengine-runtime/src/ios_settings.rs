//! What `scripts/build_ios.sh` names and labels an app by: `ios.properties`,
//! written beside the archive by `--package <project> --mode ios`, as
//! `android.properties` is for an APK (`android_settings.rs`).
//!
//! The values are worked out here rather than in the script for the reason
//! Android's are: the rules are easy to get wrong in shell and easy to test
//! here. A project table for them (`[ios]`: bundle id, version, team) comes
//! with the Xcode export; until then every build gets the defaults below.

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

/// The `ios.properties` text: `key=value` lines the script reads. The name
/// goes into `Info.plist` as XML text, so it is escaped here, and a line
/// break, which would end the property, is a space.
pub fn properties(project_name: &str) -> String {
    let name = project_name
        .replace(['\n', '\r'], " ")
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "bundle_id={}\nname_xml={name}\n",
        default_bundle_id(project_name)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.'),
                "{name:?} -> {id:?}"
            );
            assert!(!id.contains("--") && !id.ends_with('-'), "{id:?}");
        }
    }

    #[test]
    fn the_properties_carry_the_id_and_an_escaped_name() {
        assert_eq!(
            properties("Tom & <Jerry>\nII"),
            "bundle_id=com.bsengine.tom-jerry-ii\nname_xml=Tom &amp; &lt;Jerry&gt; II\n"
        );
    }
}

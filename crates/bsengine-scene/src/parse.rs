//! Reading scene and prefab files, and saying which keys in them were ignored.
//!
//! The descriptor types do not `deny_unknown_fields`, deliberately: a file
//! written by a newer engine, or one carrying a field since removed, should
//! still load. But serde's alternative is silence, and silence is how
//! `scale-level` shipped six spot lights spelled `inner_angle:` and
//! `outer_angle:` -- the fields are `inner_angle_degrees` and
//! `outer_angle_degrees` -- that loaded, ran and drew with the default cone
//! instead of the one written, with nothing anywhere saying so.
//!
//! Godot does the same thing with a property its `.tscn` loader does not
//! recognise: the scene loads and the editor logs which property it dropped.
//! This follows it -- every loader goes through [`parse_descriptor`], which
//! warns once per ignored key with its path in the file -- and the workspace
//! test `every_checked_in_scene_and_prefab_uses_only_known_keys` holds the
//! shipped games to zero, so the next misspelling fails CI rather than a
//! playtest.

use serde::de::DeserializeOwned;

/// Parses a RON descriptor and returns it with the path of every key the type
/// did not have a field for, in file order (`entities.3.spot_light.?.inner_angle`: a `?` steps into an `Option`).
///
/// Nothing is dropped from the result that `ron::from_str` would have kept,
/// and it fails exactly where `ron::from_str` fails: this only *reports*.
pub fn parse_with_unknown_keys<T: DeserializeOwned>(
    content: &str,
) -> Result<(T, Vec<String>), ron::error::SpannedError> {
    let mut deserializer = ron::Deserializer::from_str(content)?;
    let mut unknown = Vec::new();
    let value =
        serde_ignored::deserialize(&mut deserializer, |path| unknown.push(path.to_string()))
            .map_err(|e| deserializer.span_error(e))?;
    // What `ron::from_str` does after the value: trailing text is an error,
    // not something to ignore.
    deserializer.end().map_err(|e| deserializer.span_error(e))?;
    Ok((value, unknown))
}

/// [`parse_with_unknown_keys`], warning once per ignored key with `path` (the
/// file) so it reaches the log a developer reads. What every scene and prefab
/// loader calls.
pub fn parse_descriptor<T: DeserializeOwned>(
    path: &str,
    content: &str,
) -> Result<T, ron::error::SpannedError> {
    let (value, unknown) = parse_with_unknown_keys(content)?;
    for key in unknown {
        tracing::warn!(
            "{path}: `{key}` is not a field this engine reads, so it was ignored -- \
             a misspelled key leaves the default in its place"
        );
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PrefabDescriptor, SceneDescriptor};

    /// The case that motivated this: a misspelled spot light key is named,
    /// with where it is, and the value it meant to set is not applied -- the
    /// default stands, which is exactly why it has to be said.
    #[test]
    fn a_misspelled_key_is_reported_with_its_path() {
        let (scene, unknown) = parse_with_unknown_keys::<SceneDescriptor>(
            r#"SceneDescriptor(entities: [
                EntityDescriptor(name: "A"),
                EntityDescriptor(name: "Spot", spot_light: Some((intensity: 2.0, inner_angle: 18.0))),
            ])"#,
        )
        .unwrap();
        assert_eq!(
            unknown,
            vec!["entities.1.spot_light.?.inner_angle".to_string()]
        );
        let spot = scene.entities[1].spot_light.as_ref().unwrap();
        assert_eq!(spot.intensity, 2.0, "the known key beside it still applies");
        assert_eq!(spot.inner_angle_degrees, 22.5, "the misspelled one did not");
    }

    /// The hard side: a file using only real keys -- including optional ones,
    /// an `AssetRef` written as a bare path, and the reflected `components`
    /// list -- reports nothing. A reporter that flagged everything would pass
    /// the test above.
    #[test]
    fn known_keys_are_not_reported() {
        let (_, unknown) = parse_with_unknown_keys::<SceneDescriptor>(
            r#"SceneDescriptor(entities: [EntityDescriptor(
                name: "E",
                transform: Some((position: (1.0, 2.0, 3.0))),
                script: Some("assets/scripts/e.js"),
                spot_light: Some((inner_angle_degrees: 10.0, outer_angle_degrees: 20.0)),
                components: [("bsengine_core::light::PointLight", "(intensity: 2.0)")],
            )])"#,
        )
        .unwrap();
        assert_eq!(unknown, Vec::<String>::new());
    }

    /// Prefabs go through the same reader.
    #[test]
    fn prefabs_report_too() {
        let (_, unknown) = parse_with_unknown_keys::<PrefabDescriptor>(
            r#"PrefabDescriptor(entities: [EntityDescriptor(name: "B", colour: Some((1.0, 0.0, 0.0)))])"#,
        )
        .unwrap();
        assert_eq!(unknown, vec!["entities.0.colour".to_string()]);
    }

    /// Every scene (and prefab) the games ship uses only keys the engine
    /// reads. The guard that turns the next misspelling into a CI failure
    /// rather than a light quietly at its default: `scale-level`'s six spot
    /// lights were exactly that until this test existed.
    ///
    /// Scene files are found by content (an `entities:` list) rather than by
    /// folder, because `net-2p-demo` keeps its two outside `assets/scenes`;
    /// the other `.ron` files the games ship -- script graphs, shader graphs,
    /// timelines -- have no entity list.
    #[test]
    fn every_checked_in_scene_and_prefab_uses_only_known_keys() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().unwrap().to_string_lossy();
                    // Build output and packaged copies, not sources.
                    if !matches!(
                        name.as_ref(),
                        "dist" | "dist-web" | "target" | ".bsengine_cache"
                    ) {
                        walk(&path, out);
                    }
                } else if path.extension().is_some_and(|e| e == "ron") {
                    out.push(path);
                }
            }
        }
        let mut files = Vec::new();
        walk(std::path::Path::new("../../games"), &mut files);

        let mut checked = 0;
        let mut problems = Vec::new();
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            if !text.contains("entities:") {
                continue;
            }
            let shown = file.display().to_string();
            let unknown = if shown.contains("prefab") {
                parse_with_unknown_keys::<PrefabDescriptor>(&text).map(|(_, u)| u)
            } else {
                parse_with_unknown_keys::<SceneDescriptor>(&text).map(|(_, u)| u)
            }
            .unwrap_or_else(|e| panic!("{shown} does not parse: {e}"));
            checked += 1;
            problems.extend(unknown.into_iter().map(|k| format!("{shown}: {k}")));
        }
        // 16 under `assets/scenes` and net-2p-demo's two. Fewer means the walk
        // missed games and the empty problem list below means nothing.
        assert!(checked >= 18, "premise: found only {checked} scene files");
        assert!(
            problems.is_empty(),
            "keys the engine ignores:\n{}",
            problems.join("\n")
        );
    }

    /// Only reporting: what `ron::from_str` rejects, this rejects --
    /// including trailing text after the value.
    #[test]
    fn it_fails_where_ron_fails() {
        let bad = r#"SceneDescriptor(entities: []) trailing"#;
        assert!(ron::from_str::<SceneDescriptor>(bad).is_err(), "premise");
        assert!(parse_with_unknown_keys::<SceneDescriptor>(bad).is_err());
        assert!(parse_with_unknown_keys::<SceneDescriptor>("SceneDescriptor(entities: [").is_err());
    }
}

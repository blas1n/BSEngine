use bevy_ecs::prelude::Resource;

/// Root directory of the current project, used to resolve every
/// project-relative asset path (`script:`, `gltf:`, custom shader paths, ...).
#[derive(Resource, Default, Clone)]
pub struct ProjectDir(pub String);

/// Resolves `path` against `project_dir`, unless `project_dir` is absent or empty.
///
/// ⚠️ Unchecked: this is a string join, and `path` may be `../../x`, `/etc/x`
/// or `C:/x`. Right for a path the developer wrote into a scene or the editor
/// built itself; **wrong for one a script supplied** -- use
/// [`resolve_project_path_checked`] there.
pub fn resolve_project_path(project_dir: Option<&ProjectDir>, path: &str) -> String {
    match project_dir {
        Some(pd) if !pd.0.is_empty() => format!("{}/{}", pd.0, path),
        _ => path.to_string(),
    }
}

/// Why a string is not a path inside the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectPathError {
    /// Nothing is left of it (`""`, `"."`, `"a/.."`).
    Empty,
    /// It starts at a root (`/x`, and `\x`, which is normalised to it).
    Absolute,
    /// A `..` climbs above the project directory.
    Escapes,
    /// A segment holds `:` -- a drive (`C:`), a URL, a `bevy_asset` source
    /// (`other://x` picks *another asset source*, which has its own root) or an
    /// NTFS alternate data stream (`x.png:stream`). None is a file in the project.
    Colon,
    /// A NUL byte, which truncates the path in the C APIs underneath.
    Nul,
}

impl std::fmt::Display for ProjectPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Empty => "the path is empty",
            Self::Absolute => "the path is absolute; it must be relative to the project",
            Self::Escapes => "the path climbs out of the project with `..`",
            Self::Colon => "the path contains `:` (a drive, a URL or an asset-source prefix)",
            Self::Nul => "the path contains a NUL byte",
        })
    }
}

impl std::error::Error for ProjectPathError {}

/// `path` as a project-relative path: forward slashes, no `.` or empty
/// segments, every `..` resolved against an earlier segment of the same path --
/// or an error if it is not inside the project.
///
/// Normalised *before* it is judged, so `assets\..\..\x`, `assets//../../x`
/// and `./assets/../../x` are all `Escapes` rather than slipping past a check
/// that only looked for the literal `../`.
pub fn project_path(path: &str) -> Result<String, ProjectPathError> {
    if path.contains('\0') {
        return Err(ProjectPathError::Nul);
    }
    let path = path.replace('\\', "/");
    if path.starts_with('/') {
        return Err(ProjectPathError::Absolute);
    }
    let mut kept: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        if segment.contains(':') {
            return Err(ProjectPathError::Colon);
        }
        match segment {
            "" | "." => {}
            ".." => {
                kept.pop().ok_or(ProjectPathError::Escapes)?;
            }
            s => kept.push(s),
        }
    }
    if kept.is_empty() {
        return Err(ProjectPathError::Empty);
    }
    Ok(kept.join("/"))
}

/// [`resolve_project_path`] for a path that came from somewhere that is not
/// trusted to stay inside the project -- a script's argument. The path is
/// [`project_path`]-checked first, so what is joined onto the project
/// directory cannot leave it.
pub fn resolve_project_path_checked(
    project_dir: Option<&ProjectDir>,
    path: &str,
) -> Result<String, ProjectPathError> {
    Ok(resolve_project_path(project_dir, &project_path(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_with_project_dir_joins_with_slash() {
        let pd = ProjectDir("games/demo".to_string());
        assert_eq!(
            resolve_project_path(Some(&pd), "assets/x.glb"),
            "games/demo/assets/x.glb"
        );
    }

    #[test]
    fn resolve_with_empty_project_dir_returns_path_unchanged() {
        let pd = ProjectDir(String::new());
        assert_eq!(
            resolve_project_path(Some(&pd), "assets/x.glb"),
            "assets/x.glb"
        );
    }

    #[test]
    fn resolve_with_no_project_dir_returns_path_unchanged() {
        assert_eq!(resolve_project_path(None, "assets/x.glb"), "assets/x.glb");
    }

    #[test]
    fn a_path_inside_the_project_is_normalised() {
        for (given, want) in [
            ("assets/x.glb", "assets/x.glb"),
            ("assets\\models\\x.glb", "assets/models/x.glb"),
            ("./assets//x.glb", "assets/x.glb"),
            ("assets/a/../x.glb", "assets/x.glb"),
            // Climbs and comes back: still inside.
            ("assets/../assets/x.glb", "assets/x.glb"),
            ("assets/x.glb/", "assets/x.glb"),
        ] {
            assert_eq!(project_path(given).as_deref(), Ok(want), "{given:?}");
        }
    }

    /// The hard side: each of these reaches a file outside the project (or
    /// another asset source) if it is joined onto the project directory
    /// unchecked. The spellings are the ones that slip past a check for the
    /// literal `../` -- backslashes, doubled slashes, a `.` in front, a climb
    /// that only escapes after enough segments.
    #[test]
    fn a_path_that_leaves_the_project_is_refused() {
        for (given, want) in [
            ("../x.glb", ProjectPathError::Escapes),
            ("assets/../../x.glb", ProjectPathError::Escapes),
            ("assets\\..\\..\\x.glb", ProjectPathError::Escapes),
            ("assets//../../x.glb", ProjectPathError::Escapes),
            ("./assets/../../x.glb", ProjectPathError::Escapes),
            ("a/b/../../../x.glb", ProjectPathError::Escapes),
            ("/etc/passwd", ProjectPathError::Absolute),
            ("\\\\server\\share\\x.glb", ProjectPathError::Absolute),
            ("C:/Windows/x.glb", ProjectPathError::Colon),
            ("C:\\Windows\\x.glb", ProjectPathError::Colon),
            ("other://assets/x.glb", ProjectPathError::Colon),
            ("assets/x.png:stream", ProjectPathError::Colon),
            ("assets/x\0.png", ProjectPathError::Nul),
            ("", ProjectPathError::Empty),
            (".", ProjectPathError::Empty),
            ("assets/..", ProjectPathError::Empty),
        ] {
            assert_eq!(project_path(given), Err(want), "{given:?}");
        }
    }

    #[test]
    fn the_checked_join_cannot_leave_the_project_directory() {
        let pd = ProjectDir("games/demo".to_string());
        assert_eq!(
            resolve_project_path_checked(Some(&pd), "assets\\x.glb").as_deref(),
            Ok("games/demo/assets/x.glb")
        );
        // The unchecked join is the hole this closes: same input, a path that
        // is no longer under `games/demo`.
        assert_eq!(
            resolve_project_path(Some(&pd), "../../secret"),
            "games/demo/../../secret"
        );
        assert_eq!(
            resolve_project_path_checked(Some(&pd), "../../secret"),
            Err(ProjectPathError::Escapes)
        );
        assert_eq!(
            resolve_project_path_checked(None, "../secret"),
            Err(ProjectPathError::Escapes)
        );
    }
}

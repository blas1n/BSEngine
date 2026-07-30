use bevy_ecs::prelude::Resource;

/// Root directory of the current project, used to resolve every
/// project-relative asset path (`script:`, `gltf:`, custom shader paths, ...).
#[derive(Resource, Default, Clone)]
pub struct ProjectDir(pub String);

/// Resolves `path` against `project_dir`, unless `project_dir` is absent or empty.
pub fn resolve_project_path(project_dir: Option<&ProjectDir>, path: &str) -> String {
    match project_dir {
        Some(pd) if !pd.0.is_empty() => format!("{}/{}", pd.0, path),
        _ => path.to_string(),
    }
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
}

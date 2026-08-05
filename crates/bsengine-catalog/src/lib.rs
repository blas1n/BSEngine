//! A static catalogue of the engine's components and scripting ops.
//!
//! Answers "does this concept already exist, and who owns it" before a new
//! component or op gets written. Parsed from source rather than read from a
//! running app's type registry, because the registry only sees registered
//! types and the question is asked before the code exists.

#![deny(missing_docs)]

use std::path::Path;

pub mod parse;
pub mod rules;

pub use parse::{Component, Field, Op};
pub use rules::Violation;

/// The whole catalogue: every component and op in the workspace.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Catalog {
    /// Every `#[derive(Component)]` type found.
    pub components: Vec<Component>,
    /// Every `#[op2]` scripting op found.
    pub ops: Vec<Op>,
}

impl Catalog {
    /// Scans `crates/` and `apps/` under `root` and builds the catalogue.
    pub fn scan(root: &Path) -> std::io::Result<Self> {
        let mut components = Vec::new();
        let mut ops = Vec::new();
        let mut registered: std::collections::BTreeSet<String> = Default::default();

        for top in ["crates", "apps"] {
            let dir = root.join(top);
            if !dir.is_dir() {
                continue;
            }
            for entry in walkdir::WalkDir::new(&dir)
                .into_iter()
                .filter_map(Result::ok)
            {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                    continue;
                }
                // Generated and vendored sources are not this engine's design.
                if path.components().any(|c| c.as_os_str() == "target") {
                    continue;
                }
                let src = match std::fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let krate = crate_name_of(path, &dir);
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");

                components.extend(parse::components_in_source(&src, &krate, &rel));
                ops.extend(parse::ops_in_source(&src, &krate, &rel));
                registered.extend(parse::registered_names_in_source(&src));
            }
        }

        for c in &mut components {
            c.registered = registered.contains(&c.name);
        }
        components.sort_by(|a, b| a.name.cmp(&b.name));
        ops.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self { components, ops })
    }
}

/// The crate a file belongs to — the first path segment under `crates/`/`apps/`.
fn crate_name_of(path: &Path, top: &Path) -> String {
    path.strip_prefix(top)
        .ok()
        .and_then(|p| p.components().next())
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The workspace root, found by walking up from this crate's directory.
    fn workspace_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root is two levels above crates/bsengine-catalog")
            .to_path_buf()
    }

    #[test]
    fn scanning_the_workspace_finds_known_components() {
        let cat = Catalog::scan(&workspace_root()).expect("scan the workspace");

        // Assert on known types and their properties rather than on a total
        // count -- the count changes with every component added, and a test
        // that must be edited on every unrelated change gets edited carelessly.
        let rb = cat
            .components
            .iter()
            .find(|c| c.name == "RigidBody")
            .expect("RigidBody is a component");
        assert_eq!(rb.krate, "bsengine-physics");
        assert_eq!(
            rb.fields.len(),
            3,
            "body_type, linear_damping, angular_damping"
        );

        let cam = cat
            .components
            .iter()
            .find(|c| c.name == "Camera")
            .expect("Camera is a component");
        assert!(
            cam.registered,
            "Camera is registered in register_gameplay_reflect_types"
        );
    }

    #[test]
    fn scanning_the_workspace_finds_known_ops() {
        let cat = Catalog::scan(&workspace_root()).expect("scan the workspace");
        let op = cat
            .ops
            .iter()
            .find(|o| o.name == "bsengine_get_velocity")
            .expect("bsengine_get_velocity is an op");
        assert_eq!(op.krate, "bsengine-scripting");
        assert!(!op.doc.is_empty(), "missing_docs is denied workspace-wide");
    }

    #[test]
    fn a_components_target_directory_is_not_scanned() {
        // target/ holds generated sources from build scripts and vendored
        // crates. Scanning it would pull in types this engine does not own.
        let cat = Catalog::scan(&workspace_root()).expect("scan the workspace");
        assert!(
            cat.components
                .iter()
                .all(|c| !c.location.contains("target")),
            "target/ leaked into the catalogue"
        );
    }
}

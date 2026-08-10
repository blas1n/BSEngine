//! A static catalogue of the engine's components and scripting ops.
//!
//! Answers "does this concept already exist, and who owns it" before a new
//! component or op gets written. Parsed from source rather than read from a
//! running app's type registry, because the registry only sees registered
//! types and the question is asked before the code exists.

#![deny(missing_docs)]

use std::path::Path;

pub mod concept;
pub mod parse;
pub mod rules;

pub use parse::{Component, Field, Op};
pub use rules::Violation;

/// Everything in the catalogue that mentions one concept.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConceptHits {
    /// The concept as queried, lowercased.
    pub concept: String,
    /// Components whose name or a field name contains the concept.
    pub components: Vec<Component>,
    /// Ops whose name contains the concept.
    pub ops: Vec<Op>,
}

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

    /// Finds everything that mentions `concept`.
    ///
    /// Matches a component by its own name or any of its field names, and an op
    /// by its name, after splitting each identifier into words — so `velocity`
    /// finds `Velocity`, `linear_velocity`, and `bsengine_get_velocity_x`, but
    /// not `velocities_buffer`.
    ///
    /// Read the two lists together. A concept with ops but no component lives
    /// outside the component set — in a backend, a resource, or the scripting
    /// layer. A concept with both, in different crates, is the more interesting
    /// case: it means two subsystems own the same word, which is what
    /// `velocity` turned out to be here. The catalogue does not judge which is
    /// right; it shows both and their rustdoc.
    pub fn concept(&self, concept: &str) -> ConceptHits {
        let needle = concept.to_ascii_lowercase();
        let matches = |ident: &str| concept::words(ident).contains(&needle);

        ConceptHits {
            concept: needle.clone(),
            components: self
                .components
                .iter()
                .filter(|c| matches(&c.name) || c.fields.iter().any(|f| matches(&f.name)))
                .cloned()
                .collect(),
            ops: self
                .ops
                .iter()
                .filter(|o| matches(&o.name))
                .cloned()
                .collect(),
        }
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
    fn velocity_has_exactly_one_owner_and_it_is_not_a_component() {
        // The history here is the reason this test exists at all.
        //
        // A `Velocity` component was proposed for roadmap item 27 on the
        // grounds that none existed. `bsengine_core::Velocity` already did --
        // and separately, physics velocity lived in the physics backend behind
        // a family of ops. Two subsystems owning one word. Item 33 settled it
        // by deleting the kinematic stack, so velocity now lives in exactly one
        // place: the physics backend, reachable only through ops.
        //
        // This test guards that outcome. If a `Velocity` component ever comes
        // back, it fails and whoever added it has to say why the engine needs
        // two owners of the word again.
        let cat = Catalog::scan(&workspace_root()).expect("scan the workspace");
        let hits = cat.concept("velocity");

        assert!(
            hits.components.is_empty(),
            "velocity is the physics backend's; a component owning it too is the \
             duplication item 33 removed. Found: {:?}",
            hits.components.iter().map(|c| &c.name).collect::<Vec<_>>()
        );
        // Named rather than counted. This was `len() >= 10` while eighteen
        // per-axis velocity ops existed; item 42 deleted twelve of them and the
        // count assertion failed for a change that was the point of the work.
        // What the concept needs is that it is reachable and that no component
        // owns it -- not how many spellings it has.
        for expected in [
            "bsengine_get_velocity",
            "bsengine_set_velocity",
            "bsengine_add_velocity",
            "bsengine_get_angular_velocity",
            "bsengine_set_angular_velocity",
            "bsengine_add_angular_velocity",
        ] {
            assert!(
                hits.ops.iter().any(|o| o.name == expected),
                "velocity is exposed through a family of ops and {expected} is                  missing from it; found {:?}",
                hits.ops.iter().map(|o| &o.name).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn a_concept_can_be_owned_by_a_component_and_ops_at_once() {
        // The two-owner case velocity used to demonstrate. `rotation` still
        // does: `Transform` stores it and a family of ops reads and writes it.
        // Keeping a live example matters -- the catalogue's whole point is
        // surfacing this shape, so it should be exercised by something real.
        let cat = Catalog::scan(&workspace_root()).expect("scan the workspace");
        let hits = cat.concept("rotation");
        assert!(
            hits.components.iter().any(|c| c.name == "Transform"),
            "Transform stores rotation; found: {:?}",
            hits.components.iter().map(|c| &c.name).collect::<Vec<_>>()
        );
        assert!(
            !hits.ops.is_empty(),
            "and the scripting API exposes it separately"
        );
    }

    #[test]
    fn a_concept_that_lives_in_a_field_reports_its_component() {
        // `damping` is not in any type name -- it is `RigidBody`'s two fields.
        // Matching only type names would miss it, and most concepts live in
        // fields.
        let cat = Catalog::scan(&workspace_root()).expect("scan the workspace");
        let hits = cat.concept("damping");
        assert!(
            hits.components.iter().any(|c| c.name == "RigidBody"),
            "linear_damping/angular_damping are RigidBody's fields; found: {:?}",
            hits.components.iter().map(|c| &c.name).collect::<Vec<_>>()
        );
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

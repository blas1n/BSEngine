//! The mechanical rules the CI gate enforces.
//!
//! These are hygiene rules, not a duplication detector. Whether two concepts
//! are the same is a judgement -- `linear_speed` being the magnitude of
//! `velocity`, or two types named `Name` meaning the same thing -- and no rule
//! here decides it. A green gate means the rules passed, not that the design
//! is free of duplication. The MCP tool exists for that question.

use crate::parse::Component;

/// A rule violation, ready to print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Which rule was broken, e.g. `"R1"`.
    pub rule: &'static str,
    /// What to fix, naming the offender and where it lives.
    pub message: String,
}

/// R1 — every **public** `#[derive(Component)]` type must be registered for
/// reflection.
///
/// An unregistered component is invisible to the Inspector, to
/// `set_reflected_component`, and to reflected scene entries, so it cannot be
/// inspected or attached by a tool or an agent.
///
/// There is no opt-out list. Non-`pub` components are skipped because they are
/// internal by construction — no other crate can name one, so registering it
/// from the shared registration function is impossible, and nothing outside
/// its crate can reach it. Visibility is the exception mechanism: declared at
/// the definition, enforced by the compiler, and it costs a real change to the
/// type rather than a line in a list.
pub fn check_r1(components: &[Component]) -> Vec<Violation> {
    components
        .iter()
        .filter(|c| c.public && !c.registered)
        .map(|c| Violation {
            rule: "R1",
            message: format!(
                "{} ({}) is not registered — add `app.register_type::<{}>();` to \
                 `register_gameplay_reflect_types` in crates/bsengine-scene/src/plugin.rs",
                c.name, c.location, c.name
            ),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::Component;

    fn component(name: &str, registered: bool) -> Component {
        Component {
            name: name.to_string(),
            krate: "bsengine-physics".to_string(),
            location: "crates/bsengine-physics/src/components.rs:29".to_string(),
            fields: Vec::new(),
            doc: "A component.".to_string(),
            registered,
            public: true,
        }
    }

    #[test]
    fn r1_flags_an_unregistered_component() {
        let v = check_r1(&[component("RigidBody", false)]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "R1");
        assert!(v[0].message.contains("RigidBody"), "names the offender");
        assert!(
            v[0].message
                .contains("crates/bsengine-physics/src/components.rs:29"),
            "says where it is, so fixing it does not require a search"
        );
    }

    #[test]
    fn r1_passes_when_every_component_is_registered() {
        assert!(check_r1(&[component("RigidBody", true)]).is_empty());
    }

    #[test]
    fn r1_ignores_a_non_public_component() {
        // `pub(crate)` and private components cannot be named from the shared
        // registration function and cannot be reached by a scene, the
        // Inspector, or MCP. Visibility is the exception mechanism.
        let mut internal = component("PhysicsHandles", false);
        internal.public = false;
        assert!(check_r1(&[internal]).is_empty());
    }
}

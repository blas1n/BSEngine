//! The mechanical rules the CI gate enforces.
//!
//! These are hygiene rules, not a duplication detector. Whether two concepts
//! are the same is a judgement -- `linear_speed` being the magnitude of
//! `velocity`, or two types named `Name` meaning the same thing -- and no rule
//! here decides it. A green gate means the rules passed, not that the design
//! is free of duplication. The MCP tool exists for that question.

use crate::parse::Component;
use crate::parse::Op;
use std::collections::BTreeSet;

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

/// R2 — no *new* per-axis op variants.
///
/// The workspace already exposes 45 ops whose names end in `_x`, `_y` or `_z`,
/// alongside whole-vector forms of the same concept. That combinatorial shape
/// is a large part of why 49 components need 298 ops. This rule does not remove
/// the existing ones — it is a ratchet: the baseline is allowed, anything new
/// is not. Shrinking the baseline is separate work.
///
/// Adding a name to the baseline is a deliberate act that shows up in the diff.
pub fn check_r2(ops: &[Op], baseline: &BTreeSet<String>) -> Vec<Violation> {
    ops.iter()
        .filter(|o| {
            let n = &o.name;
            n.ends_with("_x") || n.ends_with("_y") || n.ends_with("_z")
        })
        .filter(|o| !baseline.contains(&o.name))
        .map(|o| Violation {
            rule: "R2",
            message: format!(
                "{} ({}) is a new per-axis op — prefer a whole-vector op. If this really \
                 is needed, add it to crates/bsengine-catalog/axis_ops_baseline.txt with \
                 a comment saying why",
                o.name, o.location
            ),
        })
        .collect()
}

/// Reads the R2 baseline: one op name per line, `#` comments and blanks ignored.
pub fn read_baseline(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::Component;
    use crate::parse::Op;

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

    fn op(name: &str) -> Op {
        Op {
            name: name.to_string(),
            krate: "bsengine-scripting".to_string(),
            location: "crates/bsengine-scripting/src/ops.rs:1".to_string(),
            doc: "An op.".to_string(),
        }
    }

    #[test]
    fn r2_allows_axis_ops_that_are_in_the_baseline() {
        let baseline = ["bsengine_get_velocity_x".to_string()]
            .into_iter()
            .collect();
        assert!(check_r2(&[op("bsengine_get_velocity_x")], &baseline).is_empty());
    }

    #[test]
    fn r2_flags_a_new_axis_op() {
        let baseline = ["bsengine_get_velocity_x".to_string()]
            .into_iter()
            .collect();
        let v = check_r2(&[op("bsengine_get_acceleration_y")], &baseline);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "R2");
        assert!(v[0].message.contains("bsengine_get_acceleration_y"));
        assert!(
            v[0].message
                .contains("crates/bsengine-scripting/src/ops.rs:1"),
            "says where it is"
        );
    }

    #[test]
    fn r2_ignores_ops_that_are_not_per_axis() {
        let baseline = Default::default();
        assert!(check_r2(&[op("bsengine_get_velocity")], &baseline).is_empty());
    }

    #[test]
    fn an_op_ending_in_x_that_is_not_an_axis_is_still_flagged() {
        // The rule is a name-shape heuristic, not semantics. If a legitimate op
        // ever ends in _x, _y or _z, adding it to the baseline with a comment is
        // the ratchet working, not failing.
        let baseline = Default::default();
        assert_eq!(check_r2(&[op("bsengine_set_matrix_x")], &baseline).len(), 1);
    }

    #[test]
    fn the_baseline_parser_ignores_comments_and_blank_lines() {
        let text = "# a comment\n\nbsengine_get_velocity_x\n  bsengine_get_velocity_y  \n";
        let parsed = read_baseline(text);
        assert_eq!(parsed.len(), 2);
        assert!(parsed.contains("bsengine_get_velocity_x"));
        assert!(
            parsed.contains("bsengine_get_velocity_y"),
            "lines are trimmed"
        );
    }
}

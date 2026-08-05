//! Splits identifiers into concept words and looks concepts up.
//!
//! Derived from the names themselves rather than from declared metadata. A
//! hand-maintained concept list would drift from the code the first time
//! someone renamed a field; a derived one cannot.

/// Splits an identifier into lowercase concept words.
///
/// Handles both `snake_case` and `CamelCase`, and drops the `bsengine` prefix
/// every op carries — a word that matches everything distinguishes nothing.
pub fn words(ident: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in ident.split('_') {
        if part.is_empty() {
            continue;
        }
        // Split CamelCase runs into their words.
        let mut current = String::new();
        for ch in part.chars() {
            if ch.is_uppercase() && !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            current.push(ch.to_ascii_lowercase());
        }
        if !current.is_empty() {
            out.push(current);
        }
    }
    out.retain(|w| w != "bsengine");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identifier_splits_into_concept_words() {
        assert_eq!(words("linear_damping"), vec!["linear", "damping"]);
        assert_eq!(words("RigidBody"), vec!["rigid", "body"]);
        assert_eq!(
            words("bsengine_get_velocity_x"),
            vec!["get", "velocity", "x"]
        );
        assert_eq!(words("mesh_id"), vec!["mesh", "id"]);
    }

    #[test]
    fn the_bsengine_prefix_is_not_a_concept() {
        // Every op carries it, so it would match everything and mean nothing.
        assert!(!words("bsengine_set_velocity").contains(&"bsengine".to_string()));
    }
}

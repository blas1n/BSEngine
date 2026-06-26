use bevy_ecs::prelude::Component;

/// Arbitrary string labels on an entity — for filtering, grouping, and queries.
/// Use `tags.has("enemy")` in systems to check membership without defining an enum.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Tag(Vec<String>);

impl Tag {
    pub fn new(labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(labels.into_iter().map(|s| s.into()).collect())
    }

    pub fn has(&self, label: &str) -> bool {
        self.0.iter().any(|s| s == label)
    }

    pub fn add(&mut self, label: impl Into<String>) {
        let s = label.into();
        if !self.0.contains(&s) {
            self.0.push(s);
        }
    }

    pub fn remove(&mut self, label: &str) {
        self.0.retain(|s| s != label);
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        assert!(Tag::default().is_empty());
    }

    #[test]
    fn new_from_strings() {
        let t = Tag::new(["enemy", "heavy"]);
        assert!(t.has("enemy"));
        assert!(t.has("heavy"));
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn has_returns_false_for_missing() {
        let t = Tag::new(["ally"]);
        assert!(!t.has("enemy"));
    }

    #[test]
    fn add_inserts_new_label() {
        let mut t = Tag::default();
        t.add("player");
        assert!(t.has("player"));
    }

    #[test]
    fn add_is_idempotent() {
        let mut t = Tag::default();
        t.add("x");
        t.add("x");
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn remove_deletes_existing() {
        let mut t = Tag::new(["a", "b"]);
        t.remove("a");
        assert!(!t.has("a"));
        assert!(t.has("b"));
    }

    #[test]
    fn remove_missing_is_noop() {
        let mut t = Tag::new(["a"]);
        t.remove("z");
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn iter_yields_all_labels() {
        let t = Tag::new(["x", "y", "z"]);
        let labels: Vec<&str> = t.iter().collect();
        assert_eq!(labels, vec!["x", "y", "z"]);
    }
}

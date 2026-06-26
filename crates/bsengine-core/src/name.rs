use bevy_ecs::prelude::Component;

/// A human-readable label for an entity — used for debugging and scene-file lookups.
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name(pub String);

impl Name {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_new_stores_string() {
        let n = Name::new("player");
        assert_eq!(n.as_str(), "player");
    }

    #[test]
    fn name_from_string_owned() {
        let n = Name::new(String::from("enemy"));
        assert_eq!(n.as_str(), "enemy");
    }

    #[test]
    fn name_display() {
        let n = Name::new("boss");
        assert_eq!(format!("{n}"), "boss");
    }

    #[test]
    fn name_equality() {
        assert_eq!(Name::new("a"), Name::new("a"));
        assert_ne!(Name::new("a"), Name::new("b"));
    }

    #[test]
    fn name_clone() {
        let n1 = Name::new("hero");
        let n2 = n1.clone();
        assert_eq!(n1, n2);
    }
}

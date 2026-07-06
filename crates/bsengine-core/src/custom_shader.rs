use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone)]
pub struct CustomShader {
    pub path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_path() {
        let cs = CustomShader {
            path: "shaders/wave.wgsl".to_string(),
        };
        assert_eq!(cs.path, "shaders/wave.wgsl");
    }

    #[test]
    fn clone_preserves_path() {
        let cs = CustomShader {
            path: "fx.wgsl".to_string(),
        };
        let c2 = cs.clone();
        assert_eq!(c2.path, cs.path);
    }

    #[test]
    fn debug_contains_path() {
        let cs = CustomShader {
            path: "outline.wgsl".to_string(),
        };
        let s = format!("{cs:?}");
        assert!(s.contains("outline.wgsl"));
    }
}

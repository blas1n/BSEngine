use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Overrides the default material shader with a custom WGSL shader file.
///
/// The file is one of two things. A whole shader, `vs_main` and `fs_main`,
/// writes its own final colour and takes no part in the lighting, reflections
/// or motion vectors. Or a surface for the engine to light, when its first line
/// is `// bsengine: lit`: a `fn surface(in: VertOut) -> Surface`, usually
/// starting from `material_surface(in)`, which the standard material's shading
/// then lights like any other surface. See `bsengine_shadergraph::LIT_DIRECTIVE`.
#[derive(Component, Debug, Clone, Default, Reflect)]
#[reflect(Component, Default)]
pub struct CustomShader {
    /// Path to the WGSL shader source, relative to the project's shader directory.
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

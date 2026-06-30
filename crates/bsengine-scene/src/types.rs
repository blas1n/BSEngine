use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDescriptor {
    pub entities: Vec<EntityDescriptor>,
}

/// Built-in primitive mesh shapes that the runtime can spawn without an asset file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Primitive {
    Cube,
}

/// Marker component inserted by `ScenePlugin` for entities with `primitive: Some(...)`.
/// The runtime converts this into a `MeshRenderer` with registered GPU geometry.
#[derive(Component, Debug, Clone)]
pub struct PrimitiveMesh(pub Primitive);

/// Relative path to a JS script file, resolved against the project root by the scripting plugin.
#[derive(Component, Debug, Clone)]
pub struct ScriptPath(pub String);

/// Describes a single entity in the scene file.
///
/// All component fields are optional and default to absent; only `name` is
/// required.  The legacy `components` field is kept for compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDescriptor {
    pub name: String,
    #[serde(default)]
    pub transform: Option<TransformDescriptor>,
    #[serde(default)]
    pub gltf: Option<String>,
    #[serde(default)]
    pub camera: bool,
    #[serde(default)]
    pub directional_light: Option<DirectionalLightDescriptor>,
    #[serde(default)]
    pub primitive: Option<Primitive>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub emissive: Option<[f32; 3]>,
    /// Albedo/base color as [r, g, b] in linear 0–1. Multiplies the mesh vertex color and texture.
    #[serde(default)]
    pub color: Option<[f32; 3]>,
    /// Camera-only: point in world space the camera should face. Overrides the transform rotation.
    #[serde(default)]
    pub look_at: Option<[f32; 3]>,
    #[serde(default)]
    pub components: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformDescriptor {
    #[serde(default)]
    pub translation: [f32; 3],
    /// Quaternion as [x, y, z, w].  Defaults to identity.
    #[serde(default = "default_rotation")]
    pub rotation: [f32; 4],
    #[serde(default = "default_scale")]
    pub scale: [f32; 3],
}

impl Default for TransformDescriptor {
    fn default() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation: default_rotation(),
            scale: default_scale(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionalLightDescriptor {
    pub direction: [f32; 3],
    #[serde(default = "default_white")]
    pub color: [f32; 3],
    #[serde(default = "default_ambient")]
    pub ambient: [f32; 3],
}

fn default_rotation() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn default_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

fn default_white() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

fn default_ambient() -> [f32; 3] {
    [0.1, 0.1, 0.1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_descriptor_deserializes_from_ron() {
        let ron_str = r#"
            SceneDescriptor(
                entities: [
                    EntityDescriptor(
                        name: "Player",
                        components: [],
                    ),
                    EntityDescriptor(
                        name: "Camera",
                        components: [],
                    ),
                ],
            )
        "#;
        let scene: SceneDescriptor = ron::from_str(ron_str).expect("Failed to parse RON");
        assert_eq!(scene.entities.len(), 2);
        assert_eq!(scene.entities[0].name, "Player");
        assert_eq!(scene.entities[1].name, "Camera");
    }

    #[test]
    fn entity_descriptor_has_components() {
        let ron_str = r#"
            EntityDescriptor(
                name: "Enemy",
                components: [
                    ("Transform", "{\"x\": 0.0}"),
                    ("Health", "{\"max_hp\": 50}"),
                ],
            )
        "#;
        let entity: EntityDescriptor = ron::from_str(ron_str).expect("Failed to parse RON");
        assert_eq!(entity.name, "Enemy");
        assert_eq!(entity.components.len(), 2);
        assert_eq!(entity.components[0].0, "Transform");
    }

    #[test]
    fn entity_descriptor_with_transform_deserializes() {
        // RON uses tuple syntax () for fixed-size arrays [f32; N]
        let ron_str = r#"
            EntityDescriptor(
                name: "Cube",
                transform: Some((
                    translation: (1.0, 2.0, 3.0),
                    rotation: (0.0, 0.0, 0.0, 1.0),
                    scale: (1.0, 1.0, 1.0),
                )),
                gltf: Some("models/cube.gltf"),
            )
        "#;
        let entity: EntityDescriptor = ron::from_str(ron_str).unwrap();
        let t = entity.transform.unwrap();
        assert_eq!(t.translation, [1.0, 2.0, 3.0]);
        assert_eq!(entity.gltf.as_deref(), Some("models/cube.gltf"));
    }

    #[test]
    fn entity_descriptor_with_camera_deserializes() {
        let ron_str = r#"EntityDescriptor(name: "Cam", camera: true)"#;
        let entity: EntityDescriptor = ron::from_str(ron_str).unwrap();
        assert!(entity.camera);
    }

    #[test]
    fn entity_descriptor_with_directional_light_deserializes() {
        let ron_str = r#"
            EntityDescriptor(
                name: "Sun",
                directional_light: Some((
                    direction: (-0.4, -0.8, -0.4),
                )),
            )
        "#;
        let entity: EntityDescriptor = ron::from_str(ron_str).unwrap();
        let dl = entity.directional_light.unwrap();
        assert_eq!(dl.direction, [-0.4, -0.8, -0.4]);
        assert_eq!(dl.color, [1.0, 1.0, 1.0]);
        assert_eq!(dl.ambient, [0.1, 0.1, 0.1]);
    }

    #[test]
    fn transform_descriptor_default() {
        let t = TransformDescriptor::default();
        assert_eq!(t.translation, [0.0, 0.0, 0.0]);
        assert_eq!(t.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(t.scale, [1.0, 1.0, 1.0]);
    }
}

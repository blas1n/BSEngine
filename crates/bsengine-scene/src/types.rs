use bevy_ecs::prelude::{Component, Resource};
use serde::{Deserialize, Serialize};

/// Root of a scene file: the list of entities to spawn plus scene-wide settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDescriptor {
    /// Entities to spawn into the world, in file order.
    pub entities: Vec<EntityDescriptor>,
    /// Optional equirectangular skybox image path (relative to the scene file).
    #[serde(default)]
    pub skybox: Option<String>,
}

/// Built-in primitive mesh shapes that the runtime can spawn without an asset file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Primitive {
    /// Unit cube.
    Cube,
    /// Unit sphere.
    Sphere,
    /// Flat ground plane.
    Plane,
    /// Cylinder with hemispherical caps.
    Capsule,
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
    /// Name assigned to the spawned entity's `Name` component.
    pub name: String,
    /// Initial position/rotation/scale. Absent means no `Transform` component is added.
    #[serde(default)]
    pub transform: Option<TransformDescriptor>,
    /// Path to a glTF asset to load as this entity's mesh.
    #[serde(default)]
    pub gltf: Option<String>,
    /// Whether this entity should get a `Camera` component.
    #[serde(default)]
    pub camera: bool,
    /// Camera-only: vertical field of view in degrees. Defaults to 60 if absent.
    #[serde(default)]
    pub camera_fov: Option<f32>,
    /// Directional (sun-like) light to attach to this entity.
    #[serde(default)]
    pub directional_light: Option<DirectionalLightDescriptor>,
    /// Point light to attach to this entity.
    #[serde(default)]
    pub point_light: Option<PointLightDescriptor>,
    /// Spot light to attach to this entity.
    #[serde(default)]
    pub spot_light: Option<SpotLightDescriptor>,
    /// Built-in primitive mesh shape to spawn, if not using a glTF asset.
    #[serde(default)]
    pub primitive: Option<Primitive>,
    /// Path to a JS script to attach via `ScriptPath`.
    #[serde(default)]
    pub script: Option<String>,
    /// Emissive color as [r, g, b], added on top of the base color.
    #[serde(default)]
    pub emissive: Option<[f32; 3]>,
    /// Albedo/base color as [r, g, b] in linear 0–1. Multiplies the mesh vertex color and texture.
    #[serde(default)]
    pub color: Option<[f32; 3]>,
    /// Camera-only: point in world space the camera should face. Overrides the transform rotation.
    #[serde(default)]
    pub look_at: Option<[f32; 3]>,
    /// Physics body type; requires `collider` to also be set to take effect.
    #[serde(default)]
    pub rigidbody: Option<RigidBodyDesc>,
    /// Collision shape and material; requires `rigidbody` to also be set to take effect.
    #[serde(default)]
    pub collider: Option<ColliderDesc>,
    /// Legacy (name, json) component pairs, kept for backwards compatibility.
    #[serde(default)]
    pub components: Vec<(String, String)>,
}

/// Position, rotation, and scale for a scene entity, as written in a scene file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformDescriptor {
    /// World-space position as [x, y, z]. Defaults to the origin.
    #[serde(default)]
    pub translation: [f32; 3],
    /// Quaternion as [x, y, z, w].  Defaults to identity.
    #[serde(default = "default_rotation")]
    pub rotation: [f32; 4],
    /// Per-axis scale as [x, y, z]. Defaults to uniform scale of 1.
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

/// Sun-like light that shines uniformly along `direction`, with no falloff over distance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionalLightDescriptor {
    /// Direction the light travels in, as [x, y, z]. Does not need to be normalized.
    pub direction: [f32; 3],
    /// Light color as [r, g, b] in linear 0-1. Defaults to white.
    #[serde(default = "default_white")]
    pub color: [f32; 3],
    /// Ambient light color added uniformly to unlit surfaces, as [r, g, b].
    #[serde(default = "default_ambient")]
    pub ambient: [f32; 3],
}

/// Omnidirectional light that falls off with distance from the entity's position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointLightDescriptor {
    /// Light color as [r, g, b] in linear 0-1. Defaults to white.
    #[serde(default = "default_white")]
    pub color: [f32; 3],
    /// Brightness multiplier. Defaults to 1.
    #[serde(default = "default_intensity")]
    pub intensity: f32,
    /// Maximum distance the light reaches, in world units.
    #[serde(default = "default_range")]
    pub range: f32,
}

/// Cone-shaped light that falls off with distance and angle from the entity's forward direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotLightDescriptor {
    /// Light color as [r, g, b] in linear 0-1. Defaults to white.
    #[serde(default = "default_white")]
    pub color: [f32; 3],
    /// Brightness multiplier. Defaults to 1.
    #[serde(default = "default_intensity")]
    pub intensity: f32,
    /// Maximum distance the light reaches, in world units.
    #[serde(default = "default_range")]
    pub range: f32,
    /// Inner cone half-angle in degrees — full brightness inside.
    #[serde(default = "default_spot_inner_angle_degrees")]
    pub inner_angle_degrees: f32,
    /// Outer cone half-angle in degrees — zero brightness outside.
    #[serde(default = "default_spot_outer_angle_degrees")]
    pub outer_angle_degrees: f32,
}

/// Rigid body type for physics descriptors in scene files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RigidBodyDesc {
    /// Fully simulated body, affected by forces, gravity, and collisions.
    Dynamic,
    /// Immovable body that other bodies can collide with but that never moves itself.
    Static,
    /// Body moved directly by code/animation rather than by the physics simulation.
    Kinematic,
}

/// Collider shape descriptor for scene files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ColliderShapeDesc {
    /// Axis-aligned box collider defined by its half-extents.
    Box {
        /// Half-extent along the x axis.
        hx: f32,
        /// Half-extent along the y axis.
        hy: f32,
        /// Half-extent along the z axis.
        hz: f32,
    },
    /// Spherical collider.
    Sphere {
        /// Sphere radius.
        radius: f32,
    },
    /// Capsule collider: a cylinder with hemispherical caps, aligned to the local y axis.
    Capsule {
        /// Half the height of the cylindrical section, excluding the end caps.
        half_height: f32,
        /// Radius of the cylindrical section and end caps.
        radius: f32,
    },
}

/// Full collider descriptor for scene files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColliderDesc {
    /// Collision geometry.
    pub shape: ColliderShapeDesc,
    /// Bounciness, from 0 (no bounce) to 1 (fully elastic).
    #[serde(default)]
    pub restitution: f32,
    /// Surface friction coefficient. Defaults to 0.5.
    #[serde(default = "default_friction")]
    pub friction: f32,
    /// If true, this collider detects overlaps but does not physically collide.
    #[serde(default)]
    pub sensor: bool,
}

/// Component spawned by ScenePlugin for entities with rigidbody+collider data.
/// The runtime resolves this into actual physics components.
#[derive(Component, Debug, Clone)]
pub struct PhysicsBodyDesc {
    /// Physics simulation type for this body.
    pub rigidbody: RigidBodyDesc,
    /// Collision shape and material for this body.
    pub collider: ColliderDesc,
}

/// Signals a runtime scene transition was requested via script.
#[derive(Resource)]
pub struct PendingSceneLoad {
    /// Path to the scene file to load next.
    pub path: String,
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

fn default_intensity() -> f32 {
    1.0
}

fn default_range() -> f32 {
    10.0
}

fn default_spot_inner_angle_degrees() -> f32 {
    22.5
}

fn default_spot_outer_angle_degrees() -> f32 {
    30.0
}

fn default_friction() -> f32 {
    0.5
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

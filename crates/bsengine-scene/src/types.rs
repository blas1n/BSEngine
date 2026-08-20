use bevy_ecs::prelude::{Component, ReflectComponent, Resource};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use serde::{Deserialize, Serialize};

/// A reference from a scene to an asset.
///
/// Stores both an identity and a path, GUID first, following Godot 4: the path
/// is what a human reads and edits, and the fallback when the GUID is unknown
/// — a project with no sidecars behaves exactly as it did before item 30.
///
/// Accepts a bare path string too, which is what every scene in `games/` still
/// contains and what the MCP tools and scripting API hand in. That is not a
/// transitional wart: paths remain the reference format at every API boundary
/// (item 23's design), and only *stored* scene references gain an identity.
///
/// The GUID is held as a `String` rather than `bsengine_asset::AssetGuid`
/// because `bsengine-scene` does not depend on `bsengine-asset` — a scene file
/// is data, and parsing one must not require the asset database to be present.
/// Resolution (Task 2) is where the two meet.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum AssetRef {
    /// A path with no recorded identity — the pre-item-30 form.
    Path(String),
    /// A path with the identity of the asset it named when the scene was saved.
    Identified {
        /// The asset's stable identity.
        guid: String,
        /// Where it was when this scene was written; also what a human reads.
        path: String,
    },
}

impl AssetRef {
    /// The asset's path as written in the scene file.
    ///
    /// Present in both spellings, so this never fails. Called once per entity
    /// on the spawn path, hence `&str` rather than an allocation.
    pub fn path(&self) -> &str {
        match self {
            Self::Path(path) => path,
            Self::Identified { path, .. } => path,
        }
    }

    /// The recorded identity, or `None` for a bare path.
    pub fn guid(&self) -> Option<&str> {
        match self {
            Self::Path(_) => None,
            Self::Identified { guid, .. } => Some(guid),
        }
    }
}

impl From<String> for AssetRef {
    fn from(path: String) -> Self {
        Self::Path(path)
    }
}

impl From<&str> for AssetRef {
    fn from(path: &str) -> Self {
        Self::Path(path.to_string())
    }
}

/// What a reference may be spelled as, quoted in error messages so a typo in a
/// scene file says what was expected rather than only where it stopped.
const ASSET_REF_EXPECTING: &str = "an asset path string such as \"assets/models/fox.glb\", or a \
     guid/path pair such as (guid: \"0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19\", path: \
     \"assets/models/fox.glb\")";

/// Hand-written so a malformed reference says which spellings are accepted.
///
/// `#[serde(untagged)]` parses and round-trips both forms correctly under
/// `ron` 0.8, but every failure — an unknown field, a missing `path`, a
/// non-string `guid`, a bare number — collapses to the same
/// `data did not match any variant of untagged enum AssetRef`, and an unknown
/// field alongside a valid pair is silently dropped rather than reported. For a
/// typo inside a scene file that is an unguessable failure, so the two forms
/// are deserialized explicitly here and named when neither matches.
impl<'de> Deserialize<'de> for AssetRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(AssetRefVisitor)
    }
}

struct AssetRefVisitor;

impl<'de> serde::de::Visitor<'de> for AssetRefVisitor {
    type Value = AssetRef;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(ASSET_REF_EXPECTING)
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<AssetRef, E> {
        Ok(AssetRef::Path(value.to_string()))
    }

    fn visit_string<E: serde::de::Error>(self, value: String) -> Result<AssetRef, E> {
        Ok(AssetRef::Path(value))
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<AssetRef, A::Error> {
        use serde::de::Error as _;
        let mut guid: Option<String> = None;
        let mut path: Option<String> = None;
        while let Some(key) = map.next_key::<AssetRefField>()? {
            match key {
                AssetRefField::Guid => {
                    if guid.is_some() {
                        return Err(A::Error::duplicate_field("guid"));
                    }
                    guid = Some(map.next_value()?);
                }
                AssetRefField::Path => {
                    if path.is_some() {
                        return Err(A::Error::duplicate_field("path"));
                    }
                    path = Some(map.next_value()?);
                }
                AssetRefField::Unknown(other) => {
                    return Err(A::Error::custom(format!(
                        "unknown field `{other}` in asset reference; expected {ASSET_REF_EXPECTING}"
                    )));
                }
            }
        }
        match (guid, path) {
            (Some(guid), Some(path)) => Ok(AssetRef::Identified { guid, path }),
            // A lone `path:` is the same thing a bare string says.
            (None, Some(path)) => Ok(AssetRef::Path(path)),
            (Some(_), None) => Err(A::Error::custom(format!(
                "asset reference has a `guid` but no `path`; expected {ASSET_REF_EXPECTING}"
            ))),
            (None, None) => Err(A::Error::custom(format!(
                "empty asset reference; expected {ASSET_REF_EXPECTING}"
            ))),
        }
    }
}

/// A key inside the `(guid: ..., path: ...)` spelling.
///
/// Deserialized through `deserialize_identifier` rather than as a `String`:
/// `ron`'s struct-key deserializer rejects `deserialize_string` outright with
/// `ExpectedIdentifier`, which would make every pair fail to parse.
enum AssetRefField {
    Guid,
    Path,
    Unknown(String),
}

impl<'de> Deserialize<'de> for AssetRefField {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FieldVisitor;

        impl serde::de::Visitor<'_> for FieldVisitor {
            type Value = AssetRefField;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("`guid` or `path`")
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<AssetRefField, E> {
                Ok(match value {
                    "guid" => AssetRefField::Guid,
                    "path" => AssetRefField::Path,
                    other => AssetRefField::Unknown(other.to_string()),
                })
            }
        }

        deserializer.deserialize_identifier(FieldVisitor)
    }
}

/// Root of a scene file: the list of entities to spawn plus scene-wide settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDescriptor {
    /// Entities to spawn into the world, in file order.
    pub entities: Vec<EntityDescriptor>,
    /// Optional equirectangular skybox image path (relative to the scene file).
    #[serde(default)]
    pub skybox: Option<String>,
}

/// Root of a prefab file: a reusable entity subtree, instantiated by name
/// reference from a scene file, a runtime script/MCP call, or the editor.
///
/// Deliberately not `SceneDescriptor` reused verbatim -- `SceneDescriptor`
/// carries a scene-wide `skybox` field that has no meaning on a reusable
/// entity template, and a prefab author setting it would silently do
/// nothing once the entities are instantiated into a scene.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabDescriptor {
    /// Entities in this prefab, in file order. Exactly one must have no
    /// `parent:` (the root); this is validated at instantiation time, not
    /// at parse time, since RON itself has no way to express the
    /// constraint.
    pub entities: Vec<EntityDescriptor>,
}

/// Built-in primitive mesh shapes that the runtime can spawn without an asset file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, Reflect)]
#[reflect(Default)]
pub enum Primitive {
    /// Unit cube.
    #[default]
    Cube,
    /// Unit sphere.
    Sphere,
    /// Flat ground plane.
    Plane,
    /// Cylinder with hemispherical caps.
    Capsule,
}

/// Marker component inserted by `ScenePlugin` for entities with `primitive: Some(...)`.
/// The runtime converts this into a `MeshRenderer` with registered GPU geometry. Reflected
/// so it appears in the Inspector's generic Reflected Fields list -- its shape is editable
/// there via the enum-variant-switching UI added earlier in this plan.
#[derive(Component, Debug, Clone, Default, Reflect)]
#[reflect(Component, Default)]
pub struct PrimitiveMesh(pub Primitive);

/// Relative path to a JS script file, resolved against the project root by the scripting
/// plugin. Reflected so it appears in the Inspector's generic Reflected Fields list.
#[derive(Component, Debug, Clone, Default, Reflect)]
#[reflect(Component, Default)]
pub struct ScriptPath(pub String);

/// Describes a single entity in the scene file.
///
/// All component fields are optional and default to absent; only `name` is
/// required.  The legacy `components` field is kept for compatibility.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityDescriptor {
    /// Name assigned to the spawned entity's `Name` component.
    pub name: String,
    /// Initial position/rotation/scale. Absent means no `Transform` component is added.
    #[serde(default)]
    pub transform: Option<TransformDescriptor>,
    /// Reference to a glTF asset to load as this entity's mesh.
    #[serde(default)]
    pub gltf: Option<AssetRef>,
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
    /// Reference to a JS script to attach via `ScriptPath`.
    #[serde(default)]
    pub script: Option<AssetRef>,
    /// Reference to an image to use as this entity's base color texture.
    ///
    /// Same `AssetRef` shape as [`gltf`](EntityDescriptor::gltf) and
    /// [`script`](EntityDescriptor::script): a bare path still parses, and an
    /// identified reference survives the file being renamed. A raw string here
    /// would put a hole in exactly the guarantee item 30 built.
    #[serde(default)]
    pub texture: Option<AssetRef>,
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
    /// Per-second linear damping for a dynamic body — how fast it sheds speed
    /// with nothing pushing it.
    ///
    /// `RigidBody` has carried this field all along; until now the scene format
    /// had no way to say it, so every scene-built dynamic body had zero damping
    /// and slid until something stopped it. Absent means the engine default.
    #[serde(default)]
    pub linear_damping: Option<f32>,
    /// Angular counterpart to [`linear_damping`](EntityDescriptor::linear_damping).
    #[serde(default)]
    pub angular_damping: Option<f32>,
    /// Surface opacity: 1.0 (the default when absent) is solid, and anything
    /// below it puts the entity in the sorted transparent pass.
    #[serde(default)]
    pub opacity: Option<f32>,
    /// Name of this entity's parent, if any. Resolved against the other
    /// entities in the same scene file after all of them have spawned — see
    /// `spawn_scene_entities` in `plugin.rs`. Absent (the default) means a
    /// root entity, exactly as every scene written before this field
    /// existed. Must name an entity in the *same* scene file; cross-scene
    /// parent references are not supported.
    #[serde(default)]
    pub parent: Option<String>,
    /// Reference to a prefab asset. If present, this descriptor acts as an
    /// *instantiation point* rather than a normal entity: none of this
    /// descriptor's own component fields (transform aside) are used --
    /// instead the referenced prefab's own entity subtree is spawned in
    /// its place. See `instantiate_prefab` in `prefab.rs`.
    #[serde(default)]
    pub prefab: Option<AssetRef>,
    /// Reflected components not covered by this struct's own typed fields
    /// (e.g. `AnimationStateMachine`, `NavMeshAgent`, `Shield`, `Bloom`,
    /// `ToneMap`), as (fully-qualified type path, RON-encoded value) pairs.
    /// Applied via `bevy_reflect`'s `TypedReflectDeserializer` against the
    /// app's registered types — the same mechanism `set_reflected_component`
    /// (the MCP tool) and the Inspector's generic reflected-field editor use.
    /// An unknown type path or a value that doesn't match the type's shape
    /// logs a warning and is skipped, not a hard load failure.
    #[serde(default)]
    pub components: Vec<(String, String)>,
}

/// Position, rotation, and scale for a scene entity, as written in a scene file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformDescriptor {
    /// World-space position as [x, y, z]. Defaults to the origin.
    #[serde(default)]
    pub position: [f32; 3],
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
            position: [0.0, 0.0, 0.0],
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Reflect)]
pub enum RigidBodyDesc {
    /// Fully simulated body, affected by forces, gravity, and collisions.
    Dynamic,
    /// Immovable body that other bodies can collide with but that never moves itself.
    Static,
    /// Body moved directly by code/animation rather than by the physics simulation.
    Kinematic,
}

/// Collider shape descriptor for scene files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Reflect)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect)]
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
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct PhysicsBodyDesc {
    /// Physics simulation type for this body.
    pub rigidbody: RigidBodyDesc,
    /// Collision shape and material for this body.
    pub collider: ColliderDesc,
    /// Per-second linear damping, or `None` for the engine default.
    pub linear_damping: Option<f32>,
    /// Per-second angular damping, or `None` for the engine default.
    pub angular_damping: Option<f32>,
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
                    position: (1.0, 2.0, 3.0),
                    rotation: (0.0, 0.0, 0.0, 1.0),
                    scale: (1.0, 1.0, 1.0),
                )),
                gltf: Some("models/cube.gltf"),
            )
        "#;
        let entity: EntityDescriptor = ron::from_str(ron_str).unwrap();
        let t = entity.transform.unwrap();
        assert_eq!(t.position, [1.0, 2.0, 3.0]);
        assert_eq!(
            entity.gltf.as_ref().map(AssetRef::path),
            Some("models/cube.gltf")
        );
    }

    #[test]
    fn opacity_defaults_to_absent_and_parses_when_authored() {
        // A scene written before this field existed must still parse, and must
        // come back as "no opinion" rather than as an accidental zero. An
        // entity that silently turned invisible would be the worst possible
        // failure mode for a defaulted opacity.
        let older: EntityDescriptor = ron::from_str(r#"EntityDescriptor(name: "A")"#)
            .expect("a scene written before opacity existed should still parse");
        assert_eq!(older.opacity, None);

        let authored: EntityDescriptor =
            ron::from_str(r#"EntityDescriptor(name: "B", opacity: Some(0.25))"#)
                .expect("opacity should parse when authored");
        assert_eq!(authored.opacity, Some(0.25));
    }

    #[test]
    fn texture_reference_parses_in_both_spellings_and_stays_optional() {
        // Absent: every scene written before this field existed.
        let none: EntityDescriptor = ron::from_str(r#"EntityDescriptor(name: "A")"#)
            .expect("scenes written before this field must still parse");
        assert_eq!(none.texture, None);

        // The pre-item-30 spelling.
        let bare: EntityDescriptor = ron::from_str(
            r#"EntityDescriptor(name: "B", texture: Some("assets/textures/checker.png"))"#,
        )
        .expect("a bare path should parse");
        assert_eq!(
            bare.texture.as_ref().map(AssetRef::path),
            Some("assets/textures/checker.png")
        );

        // The identified spelling, which is what makes a rename survivable.
        let identified: EntityDescriptor = ron::from_str(
            r#"EntityDescriptor(name: "C", texture: Some((guid: "abc", path: "assets/textures/checker.png")))"#,
        )
        .expect("an identified reference should parse");
        assert_eq!(
            identified.texture.as_ref().map(AssetRef::path),
            Some("assets/textures/checker.png")
        );
    }

    #[test]
    fn entity_descriptor_with_camera_deserializes() {
        let ron_str = r#"EntityDescriptor(name: "Cam", camera: true)"#;
        let entity: EntityDescriptor = ron::from_str(ron_str).unwrap();
        assert!(entity.camera);
    }

    #[test]
    fn entity_descriptor_with_parent_deserializes_and_defaults_to_absent() {
        let with_parent: EntityDescriptor =
            ron::from_str(r#"EntityDescriptor(name: "Wheel", parent: Some("Body"))"#)
                .expect("a scene entity should be able to name its parent");
        assert_eq!(with_parent.parent.as_deref(), Some("Body"));

        // Every scene written before this field existed must still parse.
        let older: EntityDescriptor = ron::from_str(r#"EntityDescriptor(name: "Root")"#)
            .expect("a scene written before `parent` existed should still parse");
        assert_eq!(older.parent, None);
    }

    #[test]
    fn entity_descriptor_with_prefab_deserializes_and_defaults_to_absent() {
        let with_prefab: EntityDescriptor = ron::from_str(
            r#"EntityDescriptor(name: "Spawn1", prefab: Some("assets/prefabs/enemy.ron"))"#,
        )
        .expect("a scene entity should be able to reference a prefab");
        assert_eq!(
            with_prefab.prefab.as_ref().map(|r| r.path().to_string()),
            Some("assets/prefabs/enemy.ron".to_string())
        );

        let older: EntityDescriptor = ron::from_str(r#"EntityDescriptor(name: "Root")"#)
            .expect("a scene written before `prefab` existed should still parse");
        assert_eq!(older.prefab, None);
    }

    #[test]
    fn prefab_descriptor_parses_a_root_and_a_child() {
        let prefab: PrefabDescriptor = ron::from_str(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(name: "Wheel", parent: Some("Body"), primitive: Some(Cube)),
            ])"#,
        )
        .expect("a prefab file with a root and a child should parse");
        assert_eq!(prefab.entities.len(), 2);
        assert_eq!(prefab.entities[1].parent.as_deref(), Some("Body"));
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
    fn an_asset_ref_accepts_a_bare_path_as_well_as_a_guid_pair() {
        let bare: AssetRef = ron::from_str(r#""assets/models/fox.glb""#).expect("bare path");
        assert_eq!(bare.path(), "assets/models/fox.glb");
        assert_eq!(bare.guid(), None, "a bare path carries no identity");

        let paired: AssetRef = ron::from_str(
            r#"(guid: "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19", path: "assets/models/fox.glb")"#,
        )
        .expect("guid pair");
        assert_eq!(paired.path(), "assets/models/fox.glb");
        assert_eq!(paired.guid(), Some("0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19"));
    }

    #[test]
    fn an_existing_scene_still_parses_unchanged() {
        // The scenes in games/ are all bare-path today and migrate one at a
        // time. If this breaks, every one of them breaks at once.
        let ron_str = r#"(entities: [(name: "Fox", gltf: Some("assets/models/fox.glb"))])"#;
        let scene: SceneDescriptor = ron::from_str(ron_str).expect("legacy scene");
        assert_eq!(
            scene.entities[0].gltf.as_ref().unwrap().path(),
            "assets/models/fox.glb"
        );
        assert_eq!(scene.entities[0].gltf.as_ref().unwrap().guid(), None);
    }

    #[test]
    fn an_asset_ref_round_trips_in_both_forms() {
        // `EditorCommand::SaveScene` rewrites whole scenes; a form that parses
        // but re-serializes differently would corrupt one on save.
        for original in [
            AssetRef::Path("assets/models/fox.glb".to_string()),
            AssetRef::Identified {
                guid: "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19".to_string(),
                path: "assets/models/fox.glb".to_string(),
            },
        ] {
            let encoded = ron::to_string(&original).expect("serialize");
            let decoded: AssetRef = ron::from_str(&encoded).expect(&encoded);
            assert_eq!(decoded, original, "round trip changed {encoded}");
            let re_encoded = ron::to_string(&decoded).expect("re-serialize");
            assert_eq!(re_encoded, encoded, "re-serialization is not stable");
        }
    }

    #[test]
    fn a_whole_scene_round_trips_with_both_reference_forms() {
        let ron_str = r#"(entities: [
            (name: "Bare", gltf: Some("assets/models/fox.glb"), script: Some("assets/scripts/a.js")),
            (name: "Identified", gltf: Some((guid: "0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19", path: "assets/models/fox.glb"))),
        ])"#;
        let scene: SceneDescriptor = ron::from_str(ron_str).expect("mixed scene");
        let encoded = ron::to_string(&scene).expect("serialize scene");
        let reloaded: SceneDescriptor = ron::from_str(&encoded).expect(&encoded);
        assert_eq!(reloaded.entities[0].gltf, scene.entities[0].gltf);
        assert_eq!(reloaded.entities[0].script, scene.entities[0].script);
        assert_eq!(reloaded.entities[1].gltf, scene.entities[1].gltf);
        assert_eq!(
            reloaded.entities[1].gltf.as_ref().unwrap().guid(),
            Some("0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19"),
            "the identity survived a save/load cycle"
        );
    }

    #[test]
    fn a_malformed_asset_ref_names_the_accepted_spellings() {
        // `#[serde(untagged)]` reports only "data did not match any variant of
        // untagged enum AssetRef", which makes a typo in a scene file an
        // unguessable failure. The hand-written impl must do better.
        let err = ron::from_str::<AssetRef>(r#"(uid: "abc", path: "assets/models/fox.glb")"#)
            .expect_err("an unknown field is not a valid reference");
        let message = err.to_string();
        assert!(
            message.contains("uid"),
            "the message must name the offending field: {message}"
        );
        assert!(
            message.contains("guid") && message.contains("path"),
            "the message must name what is accepted: {message}"
        );

        let err = ron::from_str::<AssetRef>(r#"(guid: "abc")"#)
            .expect_err("a guid with no path is not a valid reference");
        assert!(
            err.to_string().contains("path"),
            "the message must name the missing field: {err}"
        );

        let err = ron::from_str::<AssetRef>("42").expect_err("a number is not a reference");
        let message = err.to_string();
        assert!(
            message.contains("asset path") && message.contains("guid"),
            "the message must describe both spellings: {message}"
        );
    }

    #[test]
    fn transform_descriptor_default() {
        let t = TransformDescriptor::default();
        assert_eq!(t.position, [0.0, 0.0, 0.0]);
        assert_eq!(t.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(t.scale, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn transform_descriptor_supports_equality_comparison() {
        let a = TransformDescriptor {
            position: [1.0, 2.0, 3.0],
            ..Default::default()
        };
        let b = a.clone();
        let c = TransformDescriptor {
            position: [9.0, 9.0, 9.0],
            ..Default::default()
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn collider_desc_supports_equality_comparison() {
        let a = ColliderDesc {
            shape: ColliderShapeDesc::Sphere { radius: 1.0 },
            restitution: 0.2,
            friction: 0.5,
            sensor: false,
        };
        let b = a.clone();
        let c = ColliderDesc {
            sensor: true,
            ..a.clone()
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn entity_descriptor_has_a_default_with_only_name_and_components_populated() {
        let d = EntityDescriptor {
            name: "X".to_string(),
            ..Default::default()
        };
        assert_eq!(d.name, "X");
        assert_eq!(d.transform, None);
        assert!(d.components.is_empty());
    }
}

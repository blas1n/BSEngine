pub mod aabb;
pub mod ambient_occlusion;
pub mod anchor;
pub mod angular_velocity;
pub mod animation_player;
pub mod attach_point;
pub mod audio_emitter;
pub mod audio_filter;
pub mod billboard;
pub mod bloom;
pub mod bone;
pub mod buoyancy;
pub mod camera;
pub mod capsule;
pub mod character_controller;
pub mod chromatic_aberration;
pub mod cloth_body;
pub mod collider;
pub mod color;
pub mod color_grading;
pub mod cooldown;
pub mod damage;
pub mod damping;
pub mod decal;
pub mod depth_of_field;
pub mod dissolve;
pub mod emissive;
pub mod environment_map;
pub mod external_impulse;
pub mod flip_book;
pub mod fog;
pub mod follow;
pub mod friction;
pub mod global_transform;
pub mod gravity;
pub mod grid_snap;
pub mod health;
pub mod interactable;
pub mod inventory;
pub mod joint;
pub mod layer;
pub mod lens_flare;
pub mod lifetime;
pub mod light;
pub mod lod;
pub mod logging;
pub mod mass;
pub mod material;
pub mod mesh;
pub mod minimap;
pub mod motion_blur;
pub mod name;
pub mod nav_mesh_agent;
pub mod network_id;
pub mod outline;
pub mod parent;
pub mod particle_system;
pub mod portal;
pub mod projectile;
pub mod ray;
pub mod reflection_probe;
pub mod restitution;
pub mod reverb_zone;
pub mod rigid_body;
pub mod screen_shake;
pub mod shield;
pub mod skeleton;
pub mod skinned_mesh;
pub mod skybox;
pub mod spawn_point;
pub mod sphere;
pub mod spring;
pub mod sprite;
pub mod stat;
pub mod tag;
pub mod text;
pub mod time;
pub mod timer;
pub mod tone_map;
pub mod transform;
pub mod trigger;
pub mod tween;
pub mod velocity;
pub mod viewport;
pub mod vignette;
pub mod visible;
pub mod volumetric_light;
pub mod water_body;
pub mod wind;
pub mod z_index;

pub use aabb::Aabb;
pub use ambient_occlusion::AmbientOcclusion;
pub use anchor::{Anchor, AnchorPreset};
pub use angular_velocity::AngularVelocity;
pub use animation_player::AnimationPlayer;
pub use attach_point::AttachPoint;
pub use audio_emitter::{AudioEmitter, AudioListener};
pub use audio_filter::{AudioFilter, FilterType};
pub use billboard::{Billboard, BillboardMode};
pub use bloom::Bloom;
pub use bone::Bone;
pub use buoyancy::Buoyancy;
pub use camera::Camera;
pub use capsule::Capsule;
pub use character_controller::CharacterController;
pub use chromatic_aberration::ChromaticAberration;
pub use cloth_body::ClothBody;
pub use collider::Collider;
pub use color::Color;
pub use color_grading::ColorGrading;
pub use cooldown::Cooldown;
pub use damage::{Damage, DamageType};
pub use damping::Damping;
pub use decal::Decal;
pub use depth_of_field::DepthOfField;
pub use dissolve::Dissolve;
pub use emissive::Emissive;
pub use environment_map::EnvironmentMap;
pub use external_impulse::ExternalImpulse;
pub use flip_book::FlipBook;
pub use fog::{Fog, FogMode};
pub use follow::{Follow, LookAt};
pub use friction::Friction;
pub use global_transform::GlobalTransform;
pub use gravity::{Gravity, GravityScale};
pub use grid_snap::GridSnap;
pub use health::Health;
pub use interactable::{InteractTrigger, Interactable};
pub use inventory::Inventory;
pub use joint::{Joint, JointType};
pub use layer::Layer;
pub use lens_flare::LensFlare;
pub use lifetime::Lifetime;
pub use light::{DirectionalLight, PointLight, SpotLight};
pub use lod::{Lod, LodLevel};
pub use logging::init_logging;
pub use mass::Mass;
pub use material::Material;
pub use mesh::Mesh;
pub use minimap::Minimap;
pub use motion_blur::MotionBlur;
pub use name::Name;
pub use nav_mesh_agent::{NavAgentState, NavMeshAgent};
pub use network_id::{NetworkAuthority, NetworkId};
pub use outline::{Outline, OutlineMode};
pub use parent::Parent;
pub use particle_system::{EmissionShape, ParticleSystem};
pub use portal::Portal;
pub use projectile::Projectile;
pub use ray::Ray;
pub use reflection_probe::{ProbeUpdateMode, ReflectionProbe};
pub use restitution::Restitution;
pub use reverb_zone::ReverbZone;
pub use rigid_body::RigidBody;
pub use screen_shake::ScreenShake;
pub use shield::Shield;
pub use skeleton::Skeleton;
pub use skinned_mesh::SkinnedMesh;
pub use skybox::{Skybox, SkyboxProjection};
pub use spawn_point::SpawnPoint;
pub use sphere::Sphere;
pub use spring::Spring;
pub use sprite::{Sprite, TextureAtlas};
pub use stat::Stat;
pub use tag::Tag;
pub use text::{Text, TextAlignment};
pub use time::Time;
pub use timer::Timer;
pub use tone_map::{ToneMap, ToneMappingMode};
pub use transform::Transform;
pub use trigger::{Trigger, TriggerEvent};
pub use tween::{EasingFn, RepeatMode, Tween, TweenTarget};
pub use velocity::Velocity;
pub use viewport::Viewport;
pub use vignette::Vignette;
pub use visible::Visible;
pub use volumetric_light::VolumetricLight;
pub use water_body::WaterBody;
pub use wind::Wind;
pub use z_index::ZIndex;

pub fn propagate_global_transforms(world: &mut bevy_ecs::world::World) {
    use bevy_ecs::prelude::Entity;
    use glam::Mat4;
    use std::collections::HashMap;

    let mut query = world.query::<(Entity, &Transform, Option<&Parent>)>();
    let entries: Vec<(Entity, Mat4, Option<Entity>)> = query
        .iter(world)
        .map(|(e, t, p)| (e, t.to_matrix(), p.map(|p| p.0)))
        .collect();

    let mut globals: HashMap<Entity, Mat4> =
        entries.iter().map(|(e, local, _)| (*e, *local)).collect();

    for _ in 0..8 {
        for (e, local, parent) in &entries {
            if let Some(parent_e) = parent {
                if let Some(&parent_global) = globals.get(parent_e) {
                    globals.insert(*e, parent_global * *local);
                }
            }
        }
    }

    let mut gt_query = world.query::<(Entity, &mut GlobalTransform)>();
    for (e, mut gt) in gt_query.iter_mut(world) {
        if let Some(&mat) = globals.get(&e) {
            gt.0 = mat;
        }
    }
}

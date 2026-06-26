pub mod aabb;
pub mod angular_velocity;
pub mod animation_player;
pub mod audio_emitter;
pub mod billboard;
pub mod camera;
pub mod capsule;
pub mod collider;
pub mod color;
pub mod damping;
pub mod external_impulse;
pub mod follow;
pub mod friction;
pub mod global_transform;
pub mod gravity;
pub mod health;
pub mod layer;
pub mod lifetime;
pub mod light;
pub mod lod;
pub mod logging;
pub mod mass;
pub mod material;
pub mod mesh;
pub mod name;
pub mod outline;
pub mod parent;
pub mod ray;
pub mod restitution;
pub mod rigid_body;
pub mod shield;
pub mod sphere;
pub mod sprite;
pub mod tag;
pub mod text;
pub mod time;
pub mod timer;
pub mod transform;
pub mod tween;
pub mod velocity;
pub mod viewport;
pub mod visible;
pub mod z_index;

pub use aabb::Aabb;
pub use angular_velocity::AngularVelocity;
pub use animation_player::AnimationPlayer;
pub use audio_emitter::{AudioEmitter, AudioListener};
pub use billboard::{Billboard, BillboardMode};
pub use camera::Camera;
pub use capsule::Capsule;
pub use collider::Collider;
pub use color::Color;
pub use damping::Damping;
pub use external_impulse::ExternalImpulse;
pub use follow::{Follow, LookAt};
pub use friction::Friction;
pub use global_transform::GlobalTransform;
pub use gravity::{Gravity, GravityScale};
pub use health::Health;
pub use layer::Layer;
pub use lifetime::Lifetime;
pub use light::{DirectionalLight, PointLight, SpotLight};
pub use lod::{Lod, LodLevel};
pub use logging::init_logging;
pub use mass::Mass;
pub use material::Material;
pub use mesh::Mesh;
pub use name::Name;
pub use outline::{Outline, OutlineMode};
pub use parent::Parent;
pub use ray::Ray;
pub use restitution::Restitution;
pub use rigid_body::RigidBody;
pub use shield::Shield;
pub use sphere::Sphere;
pub use sprite::{Sprite, TextureAtlas};
pub use tag::Tag;
pub use text::{Text, TextAlignment};
pub use time::Time;
pub use timer::Timer;
pub use transform::Transform;
pub use tween::{EasingFn, RepeatMode, Tween, TweenTarget};
pub use velocity::Velocity;
pub use viewport::Viewport;
pub use visible::Visible;
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

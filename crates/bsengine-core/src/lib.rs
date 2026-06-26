pub mod aabb;
pub mod angular_velocity;
pub mod camera;
pub mod color;
pub mod global_transform;
pub mod layer;
pub mod lifetime;
pub mod light;
pub mod logging;
pub mod material;
pub mod name;
pub mod parent;
pub mod ray;
pub mod sphere;
pub mod tag;
pub mod time;
pub mod timer;
pub mod transform;
pub mod tween;
pub mod velocity;
pub mod visible;

pub use aabb::Aabb;
pub use angular_velocity::AngularVelocity;
pub use camera::Camera;
pub use color::Color;
pub use global_transform::GlobalTransform;
pub use layer::Layer;
pub use lifetime::Lifetime;
pub use light::{DirectionalLight, PointLight, SpotLight};
pub use logging::init_logging;
pub use material::Material;
pub use name::Name;
pub use parent::Parent;
pub use ray::Ray;
pub use sphere::Sphere;
pub use tag::Tag;
pub use time::Time;
pub use timer::Timer;
pub use transform::Transform;
pub use tween::{EasingFn, RepeatMode, Tween, TweenTarget};
pub use velocity::Velocity;
pub use visible::Visible;

pub fn propagate_global_transforms(world: &mut bevy_ecs::world::World) {
    use bevy_ecs::prelude::Entity;
    use glam::Mat4;
    use std::collections::HashMap;

    // Collect: entity → (local matrix, parent entity)
    let mut query = world.query::<(Entity, &Transform, Option<&Parent>)>();
    let entries: Vec<(Entity, Mat4, Option<Entity>)> = query
        .iter(world)
        .map(|(e, t, p)| (e, t.to_matrix(), p.map(|p| p.0)))
        .collect();

    // Initialize global matrices with local transforms
    let mut globals: HashMap<Entity, Mat4> =
        entries.iter().map(|(e, local, _)| (*e, *local)).collect();

    // Propagate up to 8 levels deep
    for _ in 0..8 {
        for (e, local, parent) in &entries {
            if let Some(parent_e) = parent {
                if let Some(&parent_global) = globals.get(parent_e) {
                    globals.insert(*e, parent_global * *local);
                }
            }
        }
    }

    // Write results back
    let mut gt_query = world.query::<(Entity, &mut GlobalTransform)>();
    for (e, mut gt) in gt_query.iter_mut(world) {
        if let Some(&mat) = globals.get(&e) {
            gt.0 = mat;
        }
    }
}

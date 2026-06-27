use bevy_ecs::prelude::Component;

/// Shape of the nav-mesh obstacle footprint.
#[derive(Debug, Clone, PartialEq)]
pub enum ObstacleShape {
    /// Circular footprint. `radius` in metres.
    Circle { radius: f32 },
    /// Axis-aligned rectangular footprint. `half_x` / `half_z` in metres.
    Box { half_x: f32, half_z: f32 },
    /// Vertical capsule footprint. `radius` and `height` in metres.
    Capsule { radius: f32, height: f32 },
}

impl ObstacleShape {
    /// Approximate horizontal radius for broad-phase culling queries.
    pub fn bounding_radius(&self) -> f32 {
        match self {
            ObstacleShape::Circle { radius } => *radius,
            ObstacleShape::Box { half_x, half_z } => half_x.hypot(*half_z),
            ObstacleShape::Capsule { radius, .. } => *radius,
        }
    }
}

/// Navigation-mesh obstacle component.
///
/// The nav-mesh system reads this component to carve holes in the walkable
/// surface. Static obstacles (`dynamic = false`) are baked at build time and
/// are cheaper at runtime; dynamic obstacles are re-evaluated each frame the
/// entity moves.
///
/// The pathfinding system does NOT use `Collider` directly — attach this
/// component alongside `Collider` for both physics and navigation blocking.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Obstacle {
    pub shape: ObstacleShape,
    /// When true the obstacle is re-evaluated every frame (for moving entities).
    /// When false the obstacle is baked once into the nav mesh at build time.
    pub dynamic: bool,
    /// How deeply this obstacle carves below the nav mesh surface (metres).
    /// Larger values prevent agents from walking under thin overhangs.
    pub carve_depth: f32,
    pub enabled: bool,
}

impl Obstacle {
    pub fn circle(radius: f32) -> Self {
        Self {
            shape: ObstacleShape::Circle {
                radius: radius.max(0.0),
            },
            dynamic: false,
            carve_depth: 0.3,
            enabled: true,
        }
    }

    pub fn box_shape(half_x: f32, half_z: f32) -> Self {
        Self {
            shape: ObstacleShape::Box {
                half_x: half_x.max(0.0),
                half_z: half_z.max(0.0),
            },
            dynamic: false,
            carve_depth: 0.3,
            enabled: true,
        }
    }

    pub fn capsule(radius: f32, height: f32) -> Self {
        Self {
            shape: ObstacleShape::Capsule {
                radius: radius.max(0.0),
                height: height.max(0.0),
            },
            dynamic: false,
            carve_depth: 0.3,
            enabled: true,
        }
    }

    pub fn dynamic(mut self) -> Self {
        self.dynamic = true;
        self
    }

    pub fn with_carve_depth(mut self, depth: f32) -> Self {
        self.carve_depth = depth.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn bounding_radius(&self) -> f32 {
        self.shape.bounding_radius()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_bounding_radius() {
        let o = Obstacle::circle(3.0);
        assert_eq!(o.bounding_radius(), 3.0);
    }

    #[test]
    fn box_bounding_radius() {
        let o = Obstacle::box_shape(3.0, 4.0);
        assert!((o.bounding_radius() - 5.0).abs() < 1e-4); // 3-4-5 triangle
    }

    #[test]
    fn capsule_bounding_radius_equals_radius() {
        let o = Obstacle::capsule(2.0, 5.0);
        assert_eq!(o.bounding_radius(), 2.0);
    }

    #[test]
    fn dynamic_flag_set() {
        let o = Obstacle::circle(1.0).dynamic();
        assert!(o.dynamic);
    }

    #[test]
    fn carve_depth_builder() {
        let o = Obstacle::box_shape(1.0, 1.0).with_carve_depth(0.5);
        assert_eq!(o.carve_depth, 0.5);
    }

    #[test]
    fn disabled_builder() {
        let o = Obstacle::capsule(1.0, 2.0).disabled();
        assert!(!o.enabled);
    }
}

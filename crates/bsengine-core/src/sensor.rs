use bevy_ecs::prelude::{Component, Entity};

/// Shape used to define the sensor's detection volume.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SensorShape {
    /// Sphere with the given radius.
    Sphere { radius: f32 },
    /// Axis-aligned box with half-extents (hx, hy, hz).
    Box { hx: f32, hy: f32, hz: f32 },
    /// Vertical cylinder with given radius and half-height.
    Cylinder { radius: f32, half_height: f32 },
}

/// A trigger zone that detects when entities enter or leave its volume.
///
/// The physics system maintains `inside` by calling `enter`/`exit` each frame.
/// Game logic can then poll `is_inside`, `just_entered`, or `just_exited`.
/// Clear `just_entered` / `just_exited` at the end of each frame.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Sensor {
    pub shape: SensorShape,
    /// Layer mask: only entities whose layer overlaps this mask are tracked. 0 = all layers.
    pub layer_mask: u32,
    /// Entities currently fully or partially inside the volume.
    pub inside: Vec<Entity>,
    /// Entities that entered this frame.
    pub just_entered: Vec<Entity>,
    /// Entities that exited this frame.
    pub just_exited: Vec<Entity>,
    /// Whether the sensor fires events at all.
    pub enabled: bool,
}

impl Sensor {
    pub fn sphere(radius: f32) -> Self {
        Self::new(SensorShape::Sphere {
            radius: radius.max(0.0),
        })
    }

    pub fn aabb(hx: f32, hy: f32, hz: f32) -> Self {
        Self::new(SensorShape::Box {
            hx: hx.max(0.0),
            hy: hy.max(0.0),
            hz: hz.max(0.0),
        })
    }

    pub fn cylinder(radius: f32, half_height: f32) -> Self {
        Self::new(SensorShape::Cylinder {
            radius: radius.max(0.0),
            half_height: half_height.max(0.0),
        })
    }

    fn new(shape: SensorShape) -> Self {
        Self {
            shape,
            layer_mask: 0,
            inside: Vec::new(),
            just_entered: Vec::new(),
            just_exited: Vec::new(),
            enabled: true,
        }
    }

    pub fn with_layer_mask(mut self, mask: u32) -> Self {
        self.layer_mask = mask;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Register that `entity` has entered the volume this frame.
    pub fn enter(&mut self, entity: Entity) {
        if !self.enabled {
            return;
        }
        if !self.inside.contains(&entity) {
            self.inside.push(entity);
            self.just_entered.push(entity);
        }
    }

    /// Register that `entity` has exited the volume this frame.
    pub fn exit(&mut self, entity: Entity) {
        if !self.enabled {
            return;
        }
        if let Some(pos) = self.inside.iter().position(|&e| e == entity) {
            self.inside.swap_remove(pos);
            self.just_exited.push(entity);
        }
    }

    /// Clear per-frame enter/exit lists. Call at the end of each frame.
    pub fn flush(&mut self) {
        self.just_entered.clear();
        self.just_exited.clear();
    }

    pub fn is_inside(&self, entity: Entity) -> bool {
        self.inside.contains(&entity)
    }

    pub fn count(&self) -> usize {
        self.inside.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inside.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::World;

    fn entities(n: usize) -> Vec<Entity> {
        let mut w = World::new();
        (0..n).map(|_| w.spawn_empty().id()).collect()
    }

    #[test]
    fn entity_enters_and_is_inside() {
        let es = entities(1);
        let mut s = Sensor::sphere(5.0);
        s.enter(es[0]);
        assert!(s.is_inside(es[0]));
        assert_eq!(s.just_entered, vec![es[0]]);
    }

    #[test]
    fn entity_exits_and_is_removed() {
        let es = entities(1);
        let mut s = Sensor::sphere(5.0);
        s.enter(es[0]);
        s.exit(es[0]);
        assert!(!s.is_inside(es[0]));
        assert_eq!(s.just_exited, vec![es[0]]);
    }

    #[test]
    fn flush_clears_frame_lists() {
        let es = entities(1);
        let mut s = Sensor::sphere(5.0);
        s.enter(es[0]);
        s.flush();
        assert!(s.just_entered.is_empty());
        assert!(s.is_inside(es[0])); // entity still inside, only frame list cleared
    }

    #[test]
    fn double_enter_is_idempotent() {
        let es = entities(1);
        let mut s = Sensor::sphere(5.0);
        s.enter(es[0]);
        s.enter(es[0]);
        assert_eq!(s.count(), 1);
        assert_eq!(s.just_entered.len(), 1);
    }

    #[test]
    fn disabled_sensor_ignores_events() {
        let es = entities(1);
        let mut s = Sensor::sphere(5.0).disabled();
        s.enter(es[0]);
        assert!(!s.is_inside(es[0]));
        assert!(s.just_entered.is_empty());
    }
}

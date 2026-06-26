use bevy_ecs::prelude::{Component, Entity};
use glam::Vec3;

/// A single mass-point in a Verlet-integration rope.
#[derive(Debug, Clone, PartialEq)]
pub struct RopeNode {
    pub position: Vec3,
    pub prev_position: Vec3,
    /// If true the physics system will not move this node (e.g. anchored endpoint).
    pub pinned: bool,
}

impl RopeNode {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            prev_position: position,
            pinned: false,
        }
    }

    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }
}

/// Rope / chain / cable simulation using Verlet integration with distance constraints.
///
/// The physics system:
///   1. Calls `integrate(gravity, dt)` to advance Verlet positions.
///   2. Calls `constrain(iterations)` to satisfy segment-length constraints.
///   3. Optionally calls `attach_head(entity_pos)` / `attach_tail(entity_pos)` each frame.
///
/// Render systems read `nodes` to build a ribbon or chain mesh.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Rope {
    pub nodes: Vec<RopeNode>,
    /// Rest length of each segment (derived from initial node positions).
    pub segment_length: f32,
    /// Damping coefficient [0 = no damping, 1 = instant stop].
    pub damping: f32,
    /// Entity whose transform pins the head node (None = free).
    pub head_anchor: Option<Entity>,
    /// Entity whose transform pins the tail node (None = free).
    pub tail_anchor: Option<Entity>,
    /// Constraint solver iteration count per frame.
    pub iterations: u32,
    pub enabled: bool,
}

impl Rope {
    /// Build a rope with `node_count` equally-spaced nodes starting at `origin`
    /// hanging downward along `-Y`. The head node is pinned.
    pub fn new(origin: Vec3, node_count: usize, segment_length: f32) -> Self {
        let count = node_count.max(2);
        let nodes = (0..count)
            .map(|i| {
                let pos = origin - Vec3::Y * (i as f32 * segment_length);
                let mut n = RopeNode::new(pos);
                if i == 0 {
                    n.pinned = true;
                }
                n
            })
            .collect();
        Self {
            nodes,
            segment_length: segment_length.max(1e-4),
            damping: 0.01,
            head_anchor: None,
            tail_anchor: None,
            iterations: 8,
            enabled: true,
        }
    }

    pub fn with_damping(mut self, d: f32) -> Self {
        self.damping = d.clamp(0.0, 1.0);
        self
    }

    pub fn with_iterations(mut self, n: u32) -> Self {
        self.iterations = n.max(1);
        self
    }

    pub fn with_head_anchor(mut self, entity: Entity) -> Self {
        self.head_anchor = Some(entity);
        self
    }

    pub fn with_tail_anchor(mut self, entity: Entity) -> Self {
        self.tail_anchor = Some(entity);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Move the head (index 0) to `pos` and pin it.
    pub fn attach_head(&mut self, pos: Vec3) {
        if let Some(n) = self.nodes.first_mut() {
            n.position = pos;
            n.prev_position = pos;
            n.pinned = true;
        }
    }

    /// Move the tail (last index) to `pos` and pin it.
    pub fn attach_tail(&mut self, pos: Vec3) {
        if let Some(n) = self.nodes.last_mut() {
            n.position = pos;
            n.prev_position = pos;
            n.pinned = true;
        }
    }

    /// Verlet integration step. `gravity` is world-space acceleration (e.g. `Vec3::NEG_Y * 9.8`).
    pub fn integrate(&mut self, gravity: Vec3, dt: f32) {
        if !self.enabled {
            return;
        }
        let damping = 1.0 - self.damping;
        for node in &mut self.nodes {
            if node.pinned {
                continue;
            }
            let velocity = (node.position - node.prev_position) * damping;
            node.prev_position = node.position;
            node.position += velocity + gravity * dt * dt;
        }
    }

    /// Distance constraint relaxation (Jakobsen).
    pub fn constrain(&mut self) {
        if !self.enabled {
            return;
        }
        for _ in 0..self.iterations {
            for i in 0..self.nodes.len().saturating_sub(1) {
                let (left, right) = self.nodes.split_at_mut(i + 1);
                let a = &mut left[i];
                let b = &mut right[0];

                let delta = b.position - a.position;
                let dist = delta.length();
                if dist < 1e-8 {
                    continue;
                }
                let correction = delta * ((dist - self.segment_length) / dist * 0.5);

                if !a.pinned {
                    a.position += correction;
                }
                if !b.pinned {
                    b.position -= correction;
                }
            }
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// World-space length of the fully-extended rope.
    pub fn total_length(&self) -> f32 {
        self.segment_length * (self.nodes.len().saturating_sub(1)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rope_has_correct_node_count() {
        let r = Rope::new(Vec3::ZERO, 5, 1.0);
        assert_eq!(r.node_count(), 5);
    }

    #[test]
    fn head_node_is_pinned() {
        let r = Rope::new(Vec3::ZERO, 4, 1.0);
        assert!(r.nodes[0].pinned);
        assert!(!r.nodes[1].pinned);
    }

    #[test]
    fn integrate_moves_unpinned_nodes_with_gravity() {
        let mut r = Rope::new(Vec3::ZERO, 2, 1.0);
        let init_y = r.nodes[1].position.y;
        r.integrate(Vec3::NEG_Y * 9.8, 0.016);
        assert!(r.nodes[1].position.y < init_y);
        // Head (pinned) stays put.
        assert_eq!(r.nodes[0].position, Vec3::ZERO);
    }

    #[test]
    fn constrain_restores_segment_length() {
        let mut r = Rope::new(Vec3::ZERO, 2, 1.0);
        // Force nodes apart.
        r.nodes[1].position = Vec3::new(0.0, -5.0, 0.0);
        for _ in 0..32 {
            r.constrain();
        }
        let dist = r.nodes[0].position.distance(r.nodes[1].position);
        assert!((dist - 1.0).abs() < 0.01);
    }

    #[test]
    fn attach_head_pins_node() {
        let mut r = Rope::new(Vec3::ZERO, 3, 1.0);
        r.attach_head(Vec3::new(0.0, 5.0, 0.0));
        assert_eq!(r.nodes[0].position, Vec3::new(0.0, 5.0, 0.0));
        assert!(r.nodes[0].pinned);
    }

    #[test]
    fn total_length_matches_segments() {
        let r = Rope::new(Vec3::ZERO, 6, 0.5);
        assert!((r.total_length() - 2.5).abs() < 1e-6);
    }
}

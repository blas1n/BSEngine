use bsengine_gltf::NodeTransform;
use glam::{Mat4, Quat, Vec3};

/// One bone's rigid body, derived from the rest-pose skeleton.
///
/// A bone *is* the segment between a node and its parent, and there is one of
/// these per node — **roots included**. A root spans nothing, so its capsule
/// collapses to a sphere of `bone_radius`, and that is on purpose rather than
/// a degenerate case tolerated: a skeleton's root is usually the hips, with
/// the spine and both legs hanging off it. Skip it and the character comes
/// apart into three unconnected pieces at the pelvis the instant it is
/// switched on, each of which falls perfectly convincingly on its own.
///
/// What a zero-length bone must not get is zero *mass* — Rapier reads that as
/// infinite, so the bone would hold the whole skeleton up in mid-air instead
/// of dropping it. See [`plan_bones`] on how the mass split avoids that.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BonePlan {
    /// Index into `SkinnedMesh.nodes` — the bone's own (child) end.
    pub node: usize,
    /// Parent node index, or `None` for the root bone. The bone is jointed to
    /// that node's body.
    pub parent: Option<usize>,
    /// Capsule midpoint, in model space — halfway between the parent node's
    /// rest position and this node's.
    pub center: Vec3,
    /// Model-space rotation taking `+Y` onto the bone's direction.
    ///
    /// Rapier's capsules are Y-aligned, so without this every bone's collider
    /// would lie along the model's up axis no matter which way the bone
    /// actually points — and the joint anchors below, which are expressed as
    /// `±half_height` along local Y, would land nowhere near the joint.
    pub rotation: Quat,
    /// Capsule half-height: the bone's rest length / 2.
    ///
    /// Rapier measures a capsule's half-height over its *cylindrical* part, so
    /// the collider overhangs each end of the bone by `bone_radius` — which is
    /// what gives a joint its rounded shoulder.
    pub half_height: f32,
    /// Share of `total_mass`, proportional to bone length.
    pub mass: f32,
}

impl BonePlan {
    /// The bone's head — the end it shares with its parent — in the bone's own
    /// local space. This is the point a joint holds.
    pub fn local_head(&self) -> Vec3 {
        Vec3::new(0.0, -self.half_height, 0.0)
    }

    /// The bone's tail, in its own local space: where its children attach.
    pub fn local_tail(&self) -> Vec3 {
        Vec3::new(0.0, self.half_height, 0.0)
    }

    /// The capsule's volume, used to turn [`mass`] into the collider density
    /// Rapier actually takes.
    ///
    /// [`mass`]: BonePlan::mass
    pub fn volume(&self, radius: f32) -> f32 {
        let r2 = radius * radius;
        std::f32::consts::PI * r2 * 2.0 * self.half_height
            + (4.0 / 3.0) * std::f32::consts::PI * r2 * radius
    }
}

/// Accumulates each node's rest-pose **global** transform from its local one.
///
/// A fixed number of passes rather than a topological sort, because glTF does
/// not promise a parent's index is lower than its child's — the same shape
/// `compute_joint_matrices_blended` and `propagate_global_transforms` already
/// use in this codebase.
fn rest_globals(nodes: &[NodeTransform]) -> Vec<Mat4> {
    let locals: Vec<Mat4> = nodes
        .iter()
        .map(|n| {
            Mat4::from_scale_rotation_translation(
                Vec3::from(n.scale),
                Quat::from_array(n.rotation),
                Vec3::from(n.position),
            )
        })
        .collect();
    let mut globals = locals.clone();
    for _ in 0..8 {
        for (i, node) in nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                globals[i] = globals[parent] * locals[i];
            }
        }
    }
    globals
}

/// Derives one [`BonePlan`] per bone from the rest-pose node hierarchy.
///
/// Pure on purpose: every geometry and mass decision the ragdoll makes is
/// decided here, where it can be checked without a physics world at all — the
/// same reasoning that isolated `select_lod_level` and the spherical-harmonics
/// maths elsewhere in this engine.
///
/// # Mass
///
/// `total_mass` is split in proportion to each bone's **capsule extent**,
/// `length + 2 * bone_radius`, rather than its bare length. Long bones are
/// still heavier, which is the property that matters, and the caps are what
/// keep a degenerate bone — two joints authored at the same position, which
/// real rigs do have — from being handed a mass of exactly zero. Rapier reads
/// zero mass as infinite, so such a bone would not fall; it would hold the
/// rest of the skeleton up.
pub fn plan_bones(nodes: &[NodeTransform], bone_radius: f32, total_mass: f32) -> Vec<BonePlan> {
    let globals = rest_globals(nodes);

    struct Segment {
        node: usize,
        parent: Option<usize>,
        head: Vec3,
        tail: Vec3,
    }

    let position_of = |i: usize| globals[i].to_scale_rotation_translation().2;
    let segments: Vec<Segment> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| Segment {
            node: i,
            parent: node.parent,
            // A root is its own head: a point, not a segment.
            head: position_of(node.parent.unwrap_or(i)),
            tail: position_of(i),
        })
        .collect();

    let weights: Vec<f32> = segments
        .iter()
        .map(|s| (s.tail - s.head).length() + 2.0 * bone_radius.max(0.0))
        .collect();
    let total_weight: f32 = weights.iter().sum();

    segments
        .iter()
        .zip(&weights)
        .map(|(s, &weight)| {
            let delta = s.tail - s.head;
            let length = delta.length();
            BonePlan {
                node: s.node,
                parent: s.parent,
                center: (s.head + s.tail) * 0.5,
                // `from_rotation_arc` is undefined for a zero vector, and a
                // zero-length bone has no direction to point at anyway.
                rotation: if length > f32::EPSILON {
                    Quat::from_rotation_arc(Vec3::Y, delta / length)
                } else {
                    Quat::IDENTITY
                },
                half_height: length * 0.5,
                // An equal split when every weight is zero (a radius of zero
                // over a skeleton whose joints all coincide) -- anything else
                // divides by zero and hands back NaN masses.
                mass: if total_weight > 0.0 {
                    total_mass * weight / total_weight
                } else {
                    total_mass / segments.len().max(1) as f32
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node at `position` in its parent's space, unrotated and unscaled.
    fn node(name: &str, position: [f32; 3], parent: Option<usize>) -> NodeTransform {
        NodeTransform {
            name: name.to_string(),
            position,
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            parent,
        }
    }

    /// A root plus two bones of different lengths hanging straight down:
    /// node 1 is 2 units below the root, node 2 is 1 unit below node 1.
    fn uneven_chain() -> Vec<NodeTransform> {
        vec![
            node("Root", [0.0, 10.0, 0.0], None),
            node("Long", [0.0, -2.0, 0.0], Some(0)),
            node("Short", [0.0, -1.0, 0.0], Some(1)),
        ]
    }

    #[test]
    fn bone_masses_sum_to_the_requested_total() {
        // A mass distribution that does not sum to `total_mass` gives a ragdoll
        // that falls at the wrong rate -- plausible-looking, and hard to
        // attribute to this function months later.
        let plans = plan_bones(&uneven_chain(), 0.08, 70.0);
        let sum: f32 = plans.iter().map(|p| p.mass).sum();
        assert!(
            (sum - 70.0).abs() < 0.001,
            "the {} bones' masses sum to {sum}, not the requested 70.0: {:?}",
            plans.len(),
            plans.iter().map(|p| p.mass).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_longer_bone_gets_more_mass_than_a_shorter_one() {
        // Guards the "proportional to length" claim. Without it, an equal split
        // would pass the sum test above without anyone noticing.
        let nodes = uneven_chain();
        let plans = plan_bones(&nodes, 0.08, 70.0);
        let long = plans.iter().find(|p| p.node == 1).expect("bone for node 1");
        let short = plans.iter().find(|p| p.node == 2).expect("bone for node 2");
        assert!(
            long.mass > short.mass,
            "the 2-unit bone got {} and the 1-unit bone {}; mass must follow length",
            long.mass,
            short.mass
        );
    }

    #[test]
    fn each_bone_capsule_spans_from_its_parent_to_itself() {
        // Not straight down this time: a bone that is not axis-aligned is the
        // case where a capsule left at its default Y orientation stops covering
        // the bone at all, and where the joint anchors stop meeting.
        //
        // The root is deliberately *not* at the origin either: with it there,
        // a node's local position and its global one are the same number, and
        // this test passes just as well on an implementation that never
        // accumulates the hierarchy at all.
        let nodes = vec![
            node("Root", [10.0, 20.0, 30.0], None),
            node("Arm", [3.0, 4.0, 0.0], Some(0)),
        ];
        let plans = plan_bones(&nodes, 0.1, 10.0);
        let arm = *plans.iter().find(|p| p.node == 1).expect("bone for node 1");
        let head_pos = Vec3::new(10.0, 20.0, 30.0);
        let tail_pos = Vec3::new(13.0, 24.0, 30.0);

        assert!(
            arm.center.abs_diff_eq((head_pos + tail_pos) * 0.5, 0.001),
            "the capsule centre must sit midway between {head_pos:?} and {tail_pos:?}, got {:?}",
            arm.center
        );
        assert!(
            (arm.half_height - 2.5).abs() < 0.001,
            "half-height must be half the 5-unit separation, got {}",
            arm.half_height
        );
        // The two ends, put back into model space, must be the two nodes.
        let head = arm.center + arm.rotation * arm.local_head();
        let tail = arm.center + arm.rotation * arm.local_tail();
        assert!(
            head.abs_diff_eq(head_pos, 0.001),
            "the bone's head must land on its parent, got {head:?}"
        );
        assert!(
            tail.abs_diff_eq(tail_pos, 0.001),
            "the bone's tail must land on the bone's own node, got {tail:?}"
        );
    }

    #[test]
    fn a_skeleton_with_no_bones_plans_nothing_instead_of_panicking() {
        assert!(
            plan_bones(&[], 0.08, 70.0).is_empty(),
            "no nodes at all is no bones"
        );
        // A lone root is a degenerate skeleton, not a crash: zero length, and
        // still a real mass, so nothing downstream divides by zero.
        let lone = plan_bones(&[node("Lonely", [0.0, 0.0, 0.0], None)], 0.08, 70.0);
        assert_eq!(lone.len(), 1);
        assert_eq!(lone[0].half_height, 0.0);
        assert!(
            (lone[0].mass - 70.0).abs() < 0.001,
            "the only bone carries the whole mass, got {}",
            lone[0].mass
        );
    }

    #[test]
    fn the_root_gets_a_body_so_its_children_are_not_left_as_separate_pieces() {
        // A skeleton root is the hips, and the spine and both legs hang off it.
        // If the root spans nothing and is therefore skipped, each of those
        // three has no parent body to be jointed to -- and the character comes
        // apart at the pelvis into three pieces that each fall perfectly
        // convincingly on their own. The collapse test would not notice: every
        // parent/child pair that *exists* would still be within its joint
        // distance.
        let nodes = vec![
            node("Hips", [0.0, 1.0, 0.0], None),
            node("Spine", [0.0, 0.3, 0.0], Some(0)),
            node("LeftUpLeg", [0.1, -0.1, 0.0], Some(0)),
            node("RightUpLeg", [-0.1, -0.1, 0.0], Some(0)),
        ];
        let plans = plan_bones(&nodes, 0.08, 70.0);
        assert_eq!(plans.len(), 4, "every node is a bone, the root included");

        let hips = plans
            .iter()
            .find(|p| p.node == 0)
            .expect("bone for the root");
        assert_eq!(hips.parent, None, "the root has nothing above it");
        assert_eq!(
            hips.half_height, 0.0,
            "a root spans nothing, so its capsule is a sphere"
        );
        assert!(
            hips.mass > 0.0,
            "a zero-mass body reads as infinite mass in Rapier and would hold \
             the whole ragdoll up in the air; got {}",
            hips.mass
        );
        for child in [1, 2, 3] {
            assert_eq!(
                plans.iter().find(|p| p.node == child).expect("bone").parent,
                Some(0),
                "node {child} must be jointed to the root's body, not left free"
            );
        }
    }

    #[test]
    fn a_bone_inherits_its_parents_rotation_when_computing_its_rest_position() {
        // Bone positions are local, so a rotated parent moves its children.
        // Reading `nodes[i].position` directly instead of accumulating globals
        // gives a skeleton that is correct only for an unrotated bind pose --
        // which most test rigs are, and most real ones are not.
        let nodes = vec![
            NodeTransform {
                name: "Root".to_string(),
                position: [0.0, 0.0, 0.0],
                // A quarter turn about Z: the child's local +Y becomes -X.
                rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2).to_array(),
                scale: [1.0, 1.0, 1.0],
                parent: None,
            },
            node("Child", [0.0, 2.0, 0.0], Some(0)),
        ];
        let plans = plan_bones(&nodes, 0.05, 1.0);
        let child = *plans.iter().find(|p| p.node == 1).expect("bone for node 1");
        let tail = child.center + child.rotation * child.local_tail();
        assert!(
            tail.abs_diff_eq(Vec3::new(-2.0, 0.0, 0.0), 0.001),
            "the child sits at its parent's rotated +Y, i.e. (-2, 0, 0), got {tail:?}"
        );
    }
}

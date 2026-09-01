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

/// The per-node **global** transforms a ragdoll's bone bodies currently imply,
/// ready to be handed to `SkinnedMesh::pose_override`.
///
/// `bone_poses` gives each plan's body's world `(position, rotation)`, in
/// `plans` order; `None` for a bone whose body does not exist yet leaves that
/// node in its rest pose rather than snapping it to the origin.
///
/// # Which bone drives which node
///
/// A [`BonePlan`] spans *from a node's parent to that node*, but the geometry
/// skinned to a joint is the limb hanging *below* it: the thigh is weighted to
/// the hip joint, not to the knee. So a node is driven by its **first child
/// bone** — the capsule whose head sits on that node — and only a leaf, which
/// has no child, falls back to its own. Driving each node from its own bone
/// instead is the subtle version of this that looks almost right: every node
/// would still be in exactly the right *place*, because the two bones meet
/// there, and every limb would rotate with the segment above it, so the mesh
/// would slide off its own capsules as the ragdoll bent.
///
/// A node with several children (a pelvis, with a spine and two legs) takes the
/// first. That is not arbitrary in the case that matters: a rig's pelvis
/// geometry runs from the hips up to the spine, and the spine is the child that
/// bone belongs to.
///
/// # How a body becomes a transform
///
/// Each bone body carries a rigid delta from where its plan put it,
/// `now * rest⁻¹`, and that delta is applied to the node's rest global. Nothing
/// is decomposed and recomposed on the way through, so a rig with scale in its
/// bind pose keeps it.
pub fn pose_from_bones(
    nodes: &[NodeTransform],
    plans: &[BonePlan],
    bone_poses: &[Option<(Vec3, Quat)>],
) -> Vec<Mat4> {
    let rest = rest_globals(nodes);

    let mut own: Vec<Option<usize>> = vec![None; nodes.len()];
    let mut first_child: Vec<Option<usize>> = vec![None; nodes.len()];
    for (i, plan) in plans.iter().enumerate() {
        if plan.node < nodes.len() {
            own[plan.node] = Some(i);
        }
        if let Some(parent) = plan.parent {
            if parent < nodes.len() && first_child[parent].is_none() {
                first_child[parent] = Some(i);
            }
        }
    }

    (0..nodes.len())
        .map(|node| {
            let Some(driver) = first_child[node].or(own[node]) else {
                return rest[node];
            };
            let Some(Some((position, rotation))) = bone_poses.get(driver).copied() else {
                return rest[node];
            };
            let plan = &plans[driver];
            let now = Mat4::from_rotation_translation(rotation, position);
            let bone_rest = Mat4::from_rotation_translation(plan.rotation, plan.center);
            now * bone_rest.inverse() * rest[node]
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

    // ---- the pose the bodies imply -------------------------------------

    /// Every bone still exactly where its plan put it.
    fn undisturbed(plans: &[BonePlan]) -> Vec<Option<(Vec3, Quat)>> {
        plans.iter().map(|p| Some((p.center, p.rotation))).collect()
    }

    #[test]
    fn bones_left_where_they_were_built_reproduce_the_rest_pose_exactly() {
        // The frame a ragdoll switches on, before a single step, the bodies are
        // on their plans' centres and the mesh must not move at all. Any error
        // in the `now * rest⁻¹` round trip shows up here as a character that
        // twitches into a different shape the instant it is activated.
        let nodes = uneven_chain();
        let plans = plan_bones(&nodes, 0.08, 70.0);
        let pose = pose_from_bones(&nodes, &plans, &undisturbed(&plans));
        let rest = rest_globals(&nodes);

        assert_eq!(pose.len(), nodes.len(), "one transform per node");
        for (i, (got, want)) in pose.iter().zip(&rest).enumerate() {
            assert!(
                got.abs_diff_eq(*want, 0.001),
                "node {i} moved on activation: {got:?} vs the rest pose {want:?}"
            );
        }
    }

    #[test]
    fn moving_every_bone_carries_every_node_with_it() {
        let nodes = uneven_chain();
        let plans = plan_bones(&nodes, 0.08, 70.0);
        let drop = Vec3::new(1.0, -4.0, 2.0);
        let moved: Vec<Option<(Vec3, Quat)>> = plans
            .iter()
            .map(|p| Some((p.center + drop, p.rotation)))
            .collect();

        let pose = pose_from_bones(&nodes, &plans, &moved);
        let rest = rest_globals(&nodes);
        for (i, (got, want)) in pose.iter().zip(&rest).enumerate() {
            let got = got.to_scale_rotation_translation().2;
            let want = want.to_scale_rotation_translation().2 + drop;
            assert!(
                got.abs_diff_eq(want, 0.001),
                "node {i} should have travelled with its bone to {want:?}, got {got:?}"
            );
        }
    }

    #[test]
    fn a_node_follows_the_bone_below_it_rather_than_the_one_above_it() {
        // The decision this function exists to make, and the one whose wrong
        // answer looks almost right. A bone spans parent -> child, but the
        // geometry weighted to a joint is the limb *below* it: the thigh is
        // skinned to the hip, not to the knee. Drive each node from its own
        // bone instead and every node is still in exactly the right place --
        // the two bones meet there -- while every limb rotates with the segment
        // above it, and the mesh slides off its own capsules as the ragdoll
        // bends.
        //
        // Only bone 2 (node 1 -> node 2) moves here: a quarter turn about Z,
        // pivoting on the node it shares with its parent, so the whole of the
        // change is a rotation of the segment below node 1.
        let nodes = uneven_chain();
        let plans = plan_bones(&nodes, 0.08, 70.0);
        let shin = plans.iter().position(|p| p.node == 2).expect("bone 2");
        let pivot = Vec3::new(0.0, 8.0, 0.0); // node 1's rest position
        let turn = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);

        let mut poses = undisturbed(&plans);
        poses[shin] = Some((
            pivot + turn * (plans[shin].center - pivot),
            turn * plans[shin].rotation,
        ));
        let pose = pose_from_bones(&nodes, &plans, &poses);

        let (_, rotation, translation) = pose[1].to_scale_rotation_translation();
        assert!(
            translation.abs_diff_eq(pivot, 0.001),
            "node 1 is the pivot, so it must not have moved: {translation:?}"
        );
        assert!(
            rotation.abs_diff_eq(turn, 0.001) || (-rotation).abs_diff_eq(turn, 0.001),
            "node 1 must take its rotation from the bone hanging off it, which \
             turned by 90 degrees about Z; got {rotation:?}"
        );

        // And the node below really did swing round to where that puts it.
        let tip = pose[2].to_scale_rotation_translation().2;
        assert!(
            tip.abs_diff_eq(Vec3::new(1.0, 8.0, 0.0), 0.001),
            "node 2 hangs one unit below node 1 at rest, so a quarter turn \
             about Z puts it one unit to +X of the pivot; got {tip:?}"
        );

        // The root is above the bone that moved and must be untouched.
        let root = pose[0].to_scale_rotation_translation().2;
        assert!(
            root.abs_diff_eq(Vec3::new(0.0, 10.0, 0.0), 0.001),
            "nothing moved the root's own bone, so the root stays put: {root:?}"
        );
    }

    #[test]
    fn a_bone_with_no_body_yet_leaves_its_node_in_the_rest_pose() {
        // There is one frame between a ragdoll being switched on and its bone
        // bodies existing in Rapier. Snapping the node to the origin for that
        // frame would be a visible pop; leaving it in the rest pose is not.
        let nodes = uneven_chain();
        let plans = plan_bones(&nodes, 0.08, 70.0);
        let pose = pose_from_bones(&nodes, &plans, &vec![None; plans.len()]);
        for (got, want) in pose.iter().zip(&rest_globals(&nodes)) {
            assert!(got.abs_diff_eq(*want, 0.001));
        }
        assert!(
            pose_from_bones(&[], &[], &[]).is_empty(),
            "no nodes at all is no pose, not a panic"
        );
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

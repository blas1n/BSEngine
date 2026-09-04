//! Animation retargeting between skeletons with different bone names and
//! different rest poses.
//!
//! Kept free of ECS and skinning types, the same way [`crate::ik`] is: the
//! maths is testable against two node lists and a mapping, with no rig to load.

use crate::loader::NodeTransform;
use glam::{Mat4, Quat};

/// Copies a source skeleton's motion onto a target skeleton whose bones have
/// different names and a different rest pose.
///
/// Transfers each mapped bone's rotation RELATIVE TO ITS OWN REST, and applies
/// that delta to the target's rest:
///
/// ```text
/// delta        = source_rest.inverse() * source_animated
/// target_local = target_rest * delta
/// ```
///
/// Copying the absolute local rotation instead is the obvious implementation
/// and is wrong whenever the two rigs bind their skeletons differently — one
/// arm out, one arm down, or simply different joint orientations — because the
/// limb then points somewhere unrelated to what the source is doing. It also
/// passes a test that retargets a rig onto an identical copy of itself, which
/// is why `a_target_with_a_different_rest_pose_still_receives_the_motion`
/// exists.
///
/// **Rotation only.** Translation and scale stay with the target: bone lengths
/// belong to the target rig, and overwriting them deforms the character into
/// the source's proportions rather than retargeting the motion onto it. The
/// cost is that a differently-proportioned target can slide or hover, and
/// [`crate::IkChains`] is the tool for that — the two compose deliberately.
///
/// A pair naming a bone either skeleton lacks is skipped with a warning. A typo
/// in a mapping is the likeliest authoring error here, and it must say so
/// rather than silently posing nothing or discarding the rest of the mapping.
pub fn retarget_locals(
    target_nodes: &[NodeTransform],
    target_locals: &mut [Mat4],
    source_nodes: &[NodeTransform],
    source_locals: &[Mat4],
    pairs: &[(String, String)],
) {
    for (source_bone, target_bone) in pairs {
        let (Some(si), Some(ti)) = (
            source_nodes.iter().position(|n| &n.name == source_bone),
            target_nodes.iter().position(|n| &n.name == target_bone),
        ) else {
            tracing::warn!(
                "[retarget] mapping names a bone one of the skeletons lacks: \
                 {source_bone:?} -> {target_bone:?}"
            );
            continue;
        };
        let (Some(source_local), Some(target_local)) =
            (source_locals.get(si), target_locals.get(ti))
        else {
            continue;
        };

        let source_rest = rest_rotation(&source_nodes[si]);
        let target_rest = rest_rotation(&target_nodes[ti]);
        let (_, source_animated, _) = source_local.to_scale_rotation_translation();
        let delta = source_rest.inverse() * source_animated;

        // The target keeps its own scale and translation; only the rotation is
        // replaced. Recomposing from the target's existing local rather than
        // building a fresh matrix is what preserves them.
        let (scale, _, translation) = target_local.to_scale_rotation_translation();
        target_locals[ti] =
            Mat4::from_scale_rotation_translation(scale, target_rest * delta, translation);
    }
}

/// A node's rest-pose rotation, as the glTF stored it.
fn rest_rotation(node: &NodeTransform) -> Quat {
    Quat::from_xyzw(
        node.rotation[0],
        node.rotation[1],
        node.rotation[2],
        node.rotation[3],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    /// Two bones: a root and a child that the tests drive.
    fn rig(name_prefix: &str, child_rest: Quat) -> Vec<NodeTransform> {
        vec![
            NodeTransform {
                name: format!("{name_prefix}_root"),
                ..Default::default()
            },
            NodeTransform {
                name: format!("{name_prefix}_arm"),
                rotation: child_rest.to_array(),
                parent: Some(0),
                ..Default::default()
            },
        ]
    }

    fn locals_of(nodes: &[NodeTransform]) -> Vec<Mat4> {
        nodes
            .iter()
            .map(|n| {
                Mat4::from_scale_rotation_translation(
                    Vec3::from(n.scale),
                    rest_rotation(n),
                    Vec3::from(n.position),
                )
            })
            .collect()
    }

    /// Where a local transform's rotation sends +Y. Compared as a DIRECTION
    /// rather than an Euler angle or `Quat::angle_between`: item 53 produced
    /// three separate confidently-wrong angular measurements with those,
    /// including `angle_between` reporting two genuinely different cases as
    /// identical. A rotated vector is unbounded and has an exact expected
    /// value.
    fn points(local: Mat4) -> Vec3 {
        let (_, rot, _) = local.to_scale_rotation_translation();
        rot * Vec3::Y
    }

    #[test]
    fn retargeting_a_rig_onto_an_identical_copy_changes_nothing() {
        // The cheapest guard against a delta composed in the wrong order: with
        // matching rest poses the deltas cancel and the target must be left
        // exactly as it was.
        let nodes = rig("a", Quat::IDENTITY);
        let source_locals = {
            let mut l = locals_of(&nodes);
            l[1] = Mat4::from_quat(Quat::from_rotation_z(0.7));
            l
        };
        let mut target_locals = locals_of(&nodes);
        let before = target_locals.clone();

        retarget_locals(
            &nodes,
            &mut target_locals,
            &nodes,
            &source_locals,
            &[("a_arm".to_string(), "a_arm".to_string())],
        );

        // The mapped bone takes the source's pose; everything else is untouched.
        assert_eq!(target_locals[0], before[0], "the root must not move");
        let got = points(target_locals[1]);
        let want = points(source_locals[1]);
        assert!(
            (got - want).length() < 1.0e-5,
            "with identical rest poses the target must land exactly on the \
             source's pose: {got:?} vs {want:?}"
        );
    }

    #[test]
    fn a_target_with_a_different_rest_pose_still_receives_the_motion() {
        // THE test, and the fixture is what makes it one.
        //
        // BOTH rigs bind their arm away from identity, and differently. That is
        // load-bearing: the first version gave the source an IDENTITY rest, so
        // `source_rest.inverse()` was the identity too and an absolute copy
        // produced numerically the same answer -- the mutation survived and the
        // test proved nothing. A retargeting test can only tell the two
        // implementations apart when the SOURCE's rest is non-trivial.
        let source_rest = Quat::from_rotation_y(0.9);
        let target_rest = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let source_nodes = rig("s", source_rest);
        let target_nodes = rig("t", target_rest);

        // The source lifts its arm 0.6 rad about Z away from its own rest.
        let motion = Quat::from_rotation_z(0.6);
        let mut source_locals = locals_of(&source_nodes);
        source_locals[1] = Mat4::from_quat(source_rest * motion);

        let mut target_locals = locals_of(&target_nodes);
        retarget_locals(
            &target_nodes,
            &mut target_locals,
            &source_nodes,
            &source_locals,
            &[("s_arm".to_string(), "t_arm".to_string())],
        );

        // The target's arm must be its OWN rest turned by the same delta the
        // source turned by -- not the source's raw rotation.
        let want = (target_rest * motion) * Vec3::Y;
        let got = points(target_locals[1]);
        // What an absolute copy would produce: the source's animated rotation
        // dropped onto the target's rest, carrying the source's bind with it.
        let naive = (target_rest * (source_rest * motion)) * Vec3::Y;
        println!("retargeted {got:?}, want {want:?}, a naive copy would give {naive:?}");
        assert!(
            (got - want).length() < 1.0e-5,
            "the target must receive the DELTA applied to its own rest: got \
             {got:?}, want {want:?}"
        );
        assert!(
            (got - naive).length() > 0.1,
            "and it must differ from a naive absolute copy ({naive:?}), or this \
             test cannot tell the two implementations apart"
        );
    }

    #[test]
    fn an_unmapped_bone_keeps_its_own_pose() {
        // Retargeting must not zero the bones it was not told about.
        let nodes = rig("a", Quat::IDENTITY);
        let source_locals = locals_of(&nodes);
        let mut target_locals = locals_of(&nodes);
        target_locals[1] = Mat4::from_quat(Quat::from_rotation_y(1.1));
        let kept = target_locals[1];

        retarget_locals(&nodes, &mut target_locals, &nodes, &source_locals, &[]);
        assert_eq!(
            target_locals[1], kept,
            "a bone named by no pair must be left exactly as it was"
        );
    }

    #[test]
    fn a_pair_naming_a_missing_bone_is_skipped_and_the_rest_still_applies() {
        // A typo must not panic, and must not discard the pairs around it.
        let nodes = rig("a", Quat::IDENTITY);
        let mut source_locals = locals_of(&nodes);
        source_locals[1] = Mat4::from_quat(Quat::from_rotation_z(0.5));
        let mut target_locals = locals_of(&nodes);

        retarget_locals(
            &nodes,
            &mut target_locals,
            &nodes,
            &source_locals,
            &[
                ("no_such_bone".to_string(), "a_arm".to_string()),
                ("a_arm".to_string(), "a_arm".to_string()),
            ],
        );

        let got = points(target_locals[1]);
        let want = points(source_locals[1]);
        assert!(
            (got - want).length() < 1.0e-5,
            "the valid pair must still apply after a bad one is skipped: \
             {got:?} vs {want:?}"
        );
    }
}

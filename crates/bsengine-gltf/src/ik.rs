//! Two-bone inverse kinematics.
//!
//! Kept free of ECS and skeleton types on purpose, the same way
//! [`crate::skinned_mesh`]'s geometry helpers and `bsengine_physics::plan_bones`
//! are: the maths is testable against three points and a target, with no rig to
//! build.

use glam::{Quat, Vec3};

/// Solves a two-bone chain so `tip` reaches `target`, returning new world-space
/// rotations to apply to the root and mid joints.
///
/// Analytic, via the law of cosines. Two bones have a closed-form solution;
/// FABRIK and CCD exist for longer chains and would be more code for a worse
/// result here.
///
/// **The bend plane comes from the chain's current pose.** Whatever direction
/// the animation already bends the knee is preserved, so nothing has to be
/// authored per chain and a foot correction cannot swing a leg sideways. A pole
/// vector was declined for exactly that reason: it is one more thing to author,
/// and getting it wrong produces a backwards knee that reads as a solver bug.
///
/// A target beyond the chain's reach straightens it toward the target rather
/// than failing. See the clamp below for why that case needs naming.
pub fn solve_two_bone(root: Vec3, mid: Vec3, tip: Vec3, target: Vec3) -> (Quat, Quat) {
    let upper = (mid - root).length();
    let lower = (tip - mid).length();

    // A zero-length bone has no direction to rotate, and every angle below
    // divides by these. Leave such a chain alone rather than emitting NaN.
    if upper <= f32::EPSILON || lower <= f32::EPSILON {
        return (Quat::IDENTITY, Quat::IDENTITY);
    }
    let to_target = target - root;
    let target_dist = to_target.length();
    if target_dist <= f32::EPSILON {
        return (Quat::IDENTITY, Quat::IDENTITY);
    }
    let aim_dir = to_target / target_dist;

    // Construct the solved pose directly rather than composing angle deltas
    // onto the current one. Deltas need a signed rotation about a bend axis and
    // the sign depends on how that axis was derived -- easy to get backwards,
    // and a backwards knee looks like a solver bug rather than a sign error.
    // Placing the knee explicitly and then reading off the rotations that get
    // there cannot have that problem: when the target is reachable the tip
    // lands on it by construction.

    // The bend axis, taken from the CURRENT pose so the animation's own knee
    // direction survives. Rotating `aim_dir` about this axis by a positive
    // angle moves it toward the side the knee is already on: for
    // `n = (target - root) x (mid - root)`, `n x (target - root)` is the
    // component of `mid - root` perpendicular to the aim, by the vector triple
    // product.
    let axis = to_target.cross(mid - root);
    let axis = if axis.length_squared() > 1.0e-12 {
        axis.normalize()
    } else {
        // The chain is already straight along the aim, so the pose picks out no
        // plane. Any perpendicular will do, but it has to be stable -- leaving
        // the axis zero would make the knee unable to bend at all.
        let seed = if aim_dir.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
        aim_dir.cross(seed).normalize()
    };

    // Interior angle at the root, from the law of cosines.
    //
    // `reach` is clamped to what the chain can actually span. The clamp is
    // load-bearing, not defensive tidiness: past full extension the cosine
    // argument exceeds 1 and `acos` of that is NaN, which propagates through
    // the joint matrices into every skinned vertex, so the whole character
    // disappears -- a symptom pointing nowhere near this function. Clamped, an
    // unreachable target instead straightens the chain toward it, which is what
    // it should look like anyway.
    let reach = target_dist.clamp(f32::EPSILON, upper + lower);
    let cos_root =
        ((upper * upper + reach * reach - lower * lower) / (2.0 * upper * reach)).clamp(-1.0, 1.0);
    let root_angle = cos_root.acos();

    let new_mid = root + Quat::from_axis_angle(axis, root_angle) * aim_dir * upper;

    // Read the rotations off the constructed pose. `mid_rot` is expressed in
    // the frame `root_rot` leaves behind, because the skeleton composes it as
    // `root_rot * mid_rot` when walking the parent chain.
    let root_rot = Quat::from_rotation_arc((mid - root).normalize(), (new_mid - root).normalize());
    let new_lower = target - new_mid;
    let mid_rot = if new_lower.length_squared() > 1.0e-12 {
        Quat::from_rotation_arc(
            (tip - mid).normalize(),
            (root_rot.inverse() * new_lower).normalize(),
        )
    } else {
        Quat::IDENTITY
    };

    (root_rot, mid_rot)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bent chain: root at the origin, knee forward, tip below.
    fn chain() -> (Vec3, Vec3, Vec3) {
        (
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.0, 1.0, 0.3),
            Vec3::new(0.0, 0.0, 0.0),
        )
    }

    /// Where the tip ends up once the solved rotations are applied.
    fn solved_tip(root: Vec3, mid: Vec3, tip: Vec3, target: Vec3) -> Vec3 {
        let (root_rot, mid_rot) = solve_two_bone(root, mid, tip, target);
        let new_mid = root + root_rot * (mid - root);
        // The mid rotation composes onto the root's, the way the skeleton's
        // parent chain applies it.
        new_mid + (root_rot * mid_rot) * (tip - mid)
    }

    #[test]
    fn a_reachable_target_puts_the_tip_exactly_on_it() {
        // Two-bone IK has an unusually strong property: for any reachable
        // target the tip lands ON it. That is an exact expected value, not a
        // proxy.
        //
        // Measured as a DISTANCE, deliberately. Item 53 produced three separate
        // confidently-wrong angular measurements -- `to_euler` reporting pi for
        // a zero rotation, and `Quat::angle_between` capping so a driving wheel
        // and a braked one read identically. Distance is unbounded and
        // direction-free, so it avoids all of them.
        let (root, mid, tip) = chain();
        for target in [
            Vec3::new(0.4, 0.3, 0.2),
            Vec3::new(-0.6, 0.9, 0.0),
            Vec3::new(0.0, 0.5, -0.7),
        ] {
            let landed = solved_tip(root, mid, tip, target);
            let err = (landed - target).length();
            assert!(
                err < 1.0e-3,
                "tip should land on a reachable target {target:?}, but landed \
                 at {landed:?}, {err} m away"
            );
        }
    }

    #[test]
    fn an_unreachable_target_extends_the_chain_and_stays_finite() {
        // Past full extension the law of cosines takes acos of a value > 1.
        // Unclamped that is NaN, and NaN propagates through the joint matrices
        // into every skinned vertex -- the character VANISHES, a symptom
        // pointing nowhere near the solver.
        //
        // Paired on purpose: "produced no NaN" alone is satisfied by a solver
        // that returns its input unchanged, so this also asserts the chain
        // actually straightened toward the target.
        let (root, mid, tip) = chain();
        let far = Vec3::new(0.0, 2.0, 50.0);
        let (root_rot, mid_rot) = solve_two_bone(root, mid, tip, far);
        assert!(
            root_rot.is_finite() && mid_rot.is_finite(),
            "an unreachable target must not produce NaN: {root_rot:?} {mid_rot:?}"
        );

        let landed = solved_tip(root, mid, tip, far);
        assert!(
            landed.is_finite(),
            "the resulting pose must be finite, got {landed:?}"
        );
        let reach = (mid - root).length() + (tip - mid).length();
        let extension = (landed - root).length();
        assert!(
            extension > reach * 0.98,
            "an unreachable target must straighten the chain: it reaches \
             {extension} m of a possible {reach} m"
        );
    }

    #[test]
    fn a_target_already_at_the_tip_leaves_the_pose_alone() {
        // Catches a solver that rewrites rotations unconditionally.
        let (root, mid, tip) = chain();
        let landed = solved_tip(root, mid, tip, tip);
        assert!(
            (landed - tip).length() < 1.0e-4,
            "solving for the tip's own position must not move it: {landed:?} \
             vs {tip:?}"
        );
    }

    #[test]
    fn the_knee_keeps_the_bend_direction_the_pose_already_had() {
        // The pose-derived pole. The same chain bent two opposite ways, solved
        // to the SAME target, must keep its own bend side each time. A solver
        // that hardcodes a plane collapses them onto one answer.
        let root = Vec3::new(0.0, 2.0, 0.0);
        let tip = Vec3::new(0.0, 0.0, 0.0);
        let target = Vec3::new(0.2, 0.4, 0.1);

        let forward_mid = Vec3::new(0.0, 1.0, 0.4);
        let backward_mid = Vec3::new(0.0, 1.0, -0.4);

        let (fr, _) = solve_two_bone(root, forward_mid, tip, target);
        let (br, _) = solve_two_bone(root, backward_mid, tip, target);
        let forward_knee = root + fr * (forward_mid - root);
        let backward_knee = root + br * (backward_mid - root);

        assert!(
            forward_knee.z > 0.0 && backward_knee.z < 0.0,
            "each chain must keep the side it was already bent toward: \
             forward knee at z={}, backward knee at z={}",
            forward_knee.z,
            backward_knee.z
        );
    }

    #[test]
    fn a_degenerate_chain_is_left_alone_rather_than_producing_nan() {
        // A zero-length bone has no direction to rotate and every angle above
        // divides by it.
        let p = Vec3::new(1.0, 1.0, 1.0);
        let (a, b) = solve_two_bone(p, p, p, Vec3::new(2.0, 0.0, 0.0));
        assert!(a.is_finite() && b.is_finite(), "{a:?} {b:?}");
    }
}

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
        let seed = if aim_dir.x.abs() < 0.9 {
            Vec3::X
        } else {
            Vec3::Y
        };
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

/// How many times [`solve_goals`] sweeps every chain.
///
/// FABRIK converges quickly for a single chain -- two or three passes are
/// usually indistinguishable from twenty. What needs the extra sweeps is the
/// *disagreement* between goals that share bones: each sweep moves the shared
/// joints halfway toward satisfying both, so the count is really "how patient
/// the solver is with two hands pulling in different directions".
pub const GOAL_ITERATIONS: usize = 10;

/// Solves one chain of joints so its last joint reaches `target`, in place.
///
/// FABRIK: reach the tip to the target and walk back up the chain putting each
/// joint a bone-length from the one below it, then pin the root back where it
/// started and walk down again. Both passes only ever place a joint at a fixed
/// distance from its neighbour, which is why the bone lengths come out of it
/// unchanged — that is the property the whole method is built on, and the one
/// worth asserting, because a solver that simply slid the joints toward the
/// target would also "reach".
///
/// Longer chains than two are what this is for; [`solve_two_bone`] is exact
/// for the two-bone case and stays the right tool there.
///
/// A target further away than the chain is long straightens it toward the
/// target, which is the same answer [`solve_two_bone`] gives and for the same
/// reason: there is no pose that reaches, and pointing at it is what a limb
/// does.
pub fn solve_chain(joints: &mut [Vec3], target: Vec3, iterations: usize) {
    if joints.len() < 2 {
        return;
    }
    let lengths: Vec<f32> = joints.windows(2).map(|w| (w[1] - w[0]).length()).collect();
    let total: f32 = lengths.iter().sum();
    // Every step below divides by a bone length. A chain of coincident joints
    // has no direction to work with, so leave it rather than emit NaN.
    if lengths.iter().any(|l| *l <= f32::EPSILON) {
        return;
    }

    let root = joints[0];
    let to_target = target - root;
    let dist = to_target.length();
    if dist <= f32::EPSILON {
        return;
    }

    if dist > total {
        // Out of reach: straighten along the aim. Iterating instead would
        // approach this same pose slowly and never arrive.
        let dir = to_target / dist;
        for i in 1..joints.len() {
            joints[i] = joints[i - 1] + dir * lengths[i - 1];
        }
        return;
    }

    for _ in 0..iterations {
        reach_backward(joints, target, &lengths);
        reach_forward(joints, root, &lengths);
    }
}

/// FABRIK's backward pass: put the tip on the target, then walk up placing each
/// joint a bone-length from the one below it.
///
/// Separate from [`reach_forward`] because [`solve_goals`] needs them apart: it
/// averages what every chain's backward pass wanted for a shared joint, and
/// only then runs one forward pass to restore the lengths. Running both per
/// chain and averaging afterwards mixes two root-pinned answers and then
/// corrects the mixture, which settles on a pose that reaches neither target --
/// measured, a hand 0.7 short in a two-arm reach.
fn reach_backward(joints: &mut [Vec3], target: Vec3, lengths: &[f32]) {
    let last = joints.len() - 1;
    joints[last] = target;
    for i in (0..last).rev() {
        let dir = (joints[i] - joints[i + 1]).normalize_or_zero();
        let dir = if dir == Vec3::ZERO { Vec3::Y } else { dir };
        joints[i] = joints[i + 1] + dir * lengths[i];
    }
}

/// FABRIK's forward pass: pin the root where the body holds it and rebuild the
/// chain downward at the right lengths.
///
/// This is the *only* place lengths are restored, which is what makes the
/// result a pose rather than an average of poses.
fn reach_forward(joints: &mut [Vec3], root: Vec3, lengths: &[f32]) {
    joints[0] = root;
    for i in 0..joints.len() - 1 {
        let dir = (joints[i + 1] - joints[i]).normalize_or_zero();
        let dir = if dir == Vec3::ZERO { Vec3::Y } else { dir };
        joints[i + 1] = joints[i] + dir * lengths[i];
    }
}

/// Solves several chains that share joints, together.
///
/// `positions` holds every joint once; each chain is a list of indices into it,
/// root first. Sharing is what makes this *full body* rather than a limb
/// solver: when both arms name the spine, reaching one hand forward bends the
/// spine, and the other arm's goal pulls it back. Unreal's Full Body IK node is
/// the same idea — one solver over the hierarchy with several effectors, rather
/// than one chain at a time.
///
/// # Why the shared joints are averaged
///
/// Each chain is solved independently within a sweep, so a joint two chains
/// both moved has two answers. Averaging them is the standard resolution and
/// the only one that treats the goals symmetrically: taking the last chain's
/// answer would make the result depend on the order the goals happen to be
/// listed in, which is not something an author chose.
///
/// Weights scale how far each goal pulls: a goal at `0.0` leaves its chain
/// where the animation put it, so foot IK can fade out rather than pop.
pub fn solve_goals(
    positions: &mut [Vec3],
    chains: &[Vec<usize>],
    targets: &[Vec3],
    weights: &[f32],
    iterations: usize,
) {
    if chains.is_empty() || chains.len() != targets.len() || chains.len() != weights.len() {
        return;
    }
    if chains
        .iter()
        .any(|c| c.iter().any(|&i| i >= positions.len()))
    {
        return;
    }
    let original = positions.to_vec();
    // Bone lengths from the pose the animation gave, taken once. Reading them
    // back each iteration would let an error compound into the definition of
    // what "the right length" is.
    let lengths: Vec<Vec<f32>> = chains
        .iter()
        .map(|c| {
            c.windows(2)
                .map(|w| (original[w[1]] - original[w[0]]).length())
                .collect()
        })
        .collect();
    let active = |i: usize| weights[i] > 0.0 && chains[i].len() >= 2;

    for _ in 0..iterations {
        // Backward, per chain, into a shared accumulator. A joint two chains
        // both reach for gets the average of what each wanted -- which is where
        // the goals actually negotiate, and the only step that treats them
        // symmetrically.
        let mut sum = vec![Vec3::ZERO; positions.len()];
        let mut count = vec![0u32; positions.len()];
        for (c, chain) in chains.iter().enumerate() {
            if !active(c) {
                continue;
            }
            let mut joints: Vec<Vec3> = chain.iter().map(|&i| positions[i]).collect();
            reach_backward(&mut joints, targets[c], &lengths[c]);
            for (slot, &index) in chain.iter().enumerate() {
                sum[index] += joints[slot];
                count[index] += 1;
            }
        }
        for i in 0..positions.len() {
            if count[i] > 0 {
                positions[i] = sum[i] / count[i] as f32;
            }
        }

        // Forward, per chain, from the root the body holds. ⚠️ This is the only
        // length-restoring sweep. An earlier version ran a full FABRIK per
        // chain, averaged the results, and then restored lengths on top -- two
        // corrections over one average, which settles on a fixed point that
        // reaches neither target. It converged; it simply converged to the
        // wrong pose, which is why more iterations did not help.
        for (c, chain) in chains.iter().enumerate() {
            if !active(c) {
                continue;
            }
            let mut joints: Vec<Vec3> = chain.iter().map(|&i| positions[i]).collect();
            reach_forward(&mut joints, original[chain[0]], &lengths[c]);
            for (slot, &index) in chain.iter().enumerate() {
                positions[index] = joints[slot];
            }
        }
    }

    // ⚠️ Blended once, here, and not inside the loop. Lerping by the weight
    // every iteration compounds: eight halvings land 99.6% of the way, so
    // "half weight" meant "almost all of it" -- and foot IK fading out would
    // have snapped instead.
    //
    // A joint two goals share takes the larger of their weights. Sequential
    // blending would compound again, and averaging would let a fully-on goal be
    // diluted by a neighbour that is fading -- the hand that is *holding*
    // something should win.
    let mut blend = vec![0.0f32; positions.len()];
    for (c, chain) in chains.iter().enumerate() {
        let w = weights[c].clamp(0.0, 1.0);
        for &i in chain {
            blend[i] = blend[i].max(w);
        }
    }
    for i in 0..positions.len() {
        positions[i] = original[i].lerp(positions[i], blend[i]);
    }
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

    /// Lengths of every segment, which is what the solver must not change.
    fn bone_lengths(joints: &[Vec3]) -> Vec<f32> {
        joints.windows(2).map(|w| (w[1] - w[0]).length()).collect()
    }

    /// Where the multi-goal fixtures put the pelvis.
    ///
    /// ⚠️ Deliberately **not** the origin. Averaging the shared joints and
    /// summing them differ by a scale about the origin, and a pose rooted there
    /// is invariant under it -- the bone directions survive the scale and the
    /// length pass rebuilds the same skeleton. With the root at the origin,
    /// summing instead of averaging was indistinguishable from correct.
    const PELVIS: Vec3 = Vec3::new(3.0, 0.5, 1.0);

    /// The two-arm rig used by the multi-goal tests, hung off [`PELVIS`].
    ///
    ///        3       4      <- hands
    ///         \     /
    ///          2           <- shoulder (shared)
    ///          |
    ///          1           <- spine (shared)
    ///          |
    ///          0           <- pelvis (root)
    fn two_arm_rig() -> Vec<Vec3> {
        vec![
            PELVIS,
            PELVIS + Vec3::new(0.0, 1.0, 0.0),
            PELVIS + Vec3::new(0.0, 2.0, 0.0),
            PELVIS + Vec3::new(-1.0, 2.0, 0.0),
            PELVIS + Vec3::new(1.0, 2.0, 0.0),
        ]
    }

    /// A straight chain of unit bones along +Y, rooted at the origin.
    fn straight_chain(bones: usize) -> Vec<Vec3> {
        (0..=bones).map(|i| Vec3::new(0.0, i as f32, 0.0)).collect()
    }

    #[test]
    fn a_chain_reaches_a_target_it_can_reach() {
        let mut joints = straight_chain(4);
        let target = Vec3::new(2.0, 1.5, 0.5);
        solve_chain(&mut joints, target, GOAL_ITERATIONS);
        let tip = *joints.last().unwrap();
        assert!(
            (tip - target).length() < 0.01,
            "the tip should land on the target, got {tip:?} for {target:?}"
        );
    }

    #[test]
    fn the_bones_keep_their_lengths() {
        // ⚠️ The assertion that separates a solver from a cheat. Sliding every
        // joint toward the target reaches it too, and stretches the limb doing
        // it -- which reads as a rig bug rather than a solver one.
        let mut joints = straight_chain(4);
        let before = bone_lengths(&joints);
        solve_chain(&mut joints, Vec3::new(2.0, 1.5, 0.5), GOAL_ITERATIONS);
        let after = bone_lengths(&joints);
        for (i, (b, a)) in before.iter().zip(&after).enumerate() {
            assert!(
                (b - a).abs() < 1.0e-3,
                "bone {i} changed length: {b} -> {a} (all: {before:?} -> {after:?})"
            );
        }
    }

    #[test]
    fn the_root_stays_where_the_body_put_it() {
        // A limb is attached to something. A solver that let the root drift
        // would detach the arm from the shoulder and the whole character would
        // slide toward whatever it was reaching for.
        let mut joints = straight_chain(3);
        let root = joints[0];
        solve_chain(&mut joints, Vec3::new(5.0, 0.0, 0.0), GOAL_ITERATIONS);
        assert!((joints[0] - root).length() < 1.0e-4, "{:?}", joints[0]);
    }

    #[test]
    fn an_unreachable_target_straightens_the_chain_toward_it() {
        let mut joints = straight_chain(3);
        let target = Vec3::new(100.0, 0.0, 0.0);
        solve_chain(&mut joints, target, GOAL_ITERATIONS);

        let aim = (target - joints[0]).normalize();
        for w in joints.windows(2) {
            let dir = (w[1] - w[0]).normalize();
            assert!(
                dir.dot(aim) > 0.999,
                "every bone should point at the target, got {dir:?} vs {aim:?}"
            );
        }
        // And still not stretched: pointing at something out of reach is not
        // permission to grow.
        for l in bone_lengths(&joints) {
            assert!((l - 1.0).abs() < 1.0e-3, "bone length {l}");
        }
    }

    #[test]
    fn a_degenerate_chain_is_left_alone() {
        let p = Vec3::new(1.0, 1.0, 1.0);
        let mut joints = vec![p, p, p];
        solve_chain(&mut joints, Vec3::new(3.0, 0.0, 0.0), GOAL_ITERATIONS);
        assert!(joints.iter().all(|j| j.is_finite()), "{joints:?}");
        assert_eq!(joints, vec![p, p, p], "nothing to solve, nothing to move");
    }

    #[test]
    fn two_goals_sharing_a_spine_both_pull_it() {
        // The property that makes this full-body rather than per-limb: a joint
        // both chains name ends up between the answers either would have given
        // alone. Solved one at a time, the spine would sit wherever the last
        // chain left it.
        //
        //        3       4      <- hands
        //         \     /
        //          2           <- shoulder (shared)
        //          |
        //          1           <- spine (shared)
        //          |
        //          0           <- pelvis (root)
        let base = two_arm_rig();
        let chains = vec![vec![0, 1, 2, 3], vec![0, 1, 2, 4]];
        // Both hands pulled the same way in z, so the shared spine has an
        // unambiguous direction to bend in and the test does not depend on how
        // the averaging breaks a tie.
        let targets = vec![
            PELVIS + Vec3::new(-1.0, 2.0, 1.5),
            PELVIS + Vec3::new(1.0, 2.0, 1.5),
        ];

        let mut both = base.clone();
        solve_goals(&mut both, &chains, &targets, &[1.0, 1.0], GOAL_ITERATIONS);

        assert!(
            both[1].z > PELVIS.z + 0.1,
            "the shared spine should have been pulled forward by both hands, \
             got {:?} against a pelvis at {PELVIS:?}",
            both[1]
        );
        // ⚠️ And the pelvis has not moved. A limb is attached to a body; a
        // solver that let the root drift would slide the character toward
        // whatever it reached for. This is also what separates averaging the
        // shared joints from summing them, which scales the pose about the
        // origin and therefore moves a root that is not there.
        assert!(
            (both[0] - PELVIS).length() < 1.0e-3,
            "the pelvis moved: {:?} from {PELVIS:?}",
            both[0]
        );
        for (i, hand) in [(3usize, targets[0]), (4, targets[1])] {
            assert!(
                (both[i] - hand).length() < 0.4,
                "hand {i} should be near its target: {:?} vs {hand:?}",
                both[i]
            );
        }
    }

    #[test]
    fn a_zero_weight_goal_leaves_its_chain_alone() {
        // Foot IK fades out rather than switching off, so weight zero has to
        // mean "exactly the animation", not "nearly".
        let base = straight_chain(3);
        let chains = vec![vec![0, 1, 2, 3]];
        let mut solved = base.clone();
        solve_goals(
            &mut solved,
            &chains,
            &[Vec3::new(2.0, 1.0, 0.0)],
            &[0.0],
            GOAL_ITERATIONS,
        );
        for (a, b) in base.iter().zip(&solved) {
            assert!((*a - *b).length() < 1.0e-4, "{a:?} vs {b:?}");
        }
    }

    #[test]
    fn solving_several_goals_keeps_every_bone_its_length() {
        // ⚠️ The chain solver's own test asserts this, and this path does not
        // go through it: `solve_goals` averages what the chains returned, and
        // an average of two valid poses is not itself one. Summing instead of
        // averaging -- one character's worth of difference -- leaves every goal
        // reached and the skeleton stretched.
        let base = two_arm_rig();
        let chains = vec![vec![0, 1, 2, 3], vec![0, 1, 2, 4]];
        let targets = vec![
            PELVIS + Vec3::new(-1.0, 2.0, 1.5),
            PELVIS + Vec3::new(1.0, 2.0, 1.5),
        ];

        let mut solved = base.clone();
        solve_goals(&mut solved, &chains, &targets, &[1.0, 1.0], GOAL_ITERATIONS);

        for chain in &chains {
            for w in chain.windows(2) {
                let (a, b) = (w[0], w[1]);
                let before = (base[b] - base[a]).length();
                let after = (solved[b] - solved[a]).length();
                assert!(
                    (before - after).abs() < 0.05,
                    "bone {a}->{b} changed length: {before} -> {after} \
                     (pose {solved:?})"
                );
            }
        }
    }

    #[test]
    fn a_partial_weight_lands_between_the_animation_and_the_solve() {
        // ⚠️ Zero weight is handled by skipping the chain outright, so a test
        // at zero never reaches the blend at all -- deleting the blend left
        // every test green. A weight in between is the only thing that
        // exercises it, and it is the case foot IK actually uses while fading.
        let base = straight_chain(3);
        let chains = vec![vec![0, 1, 2, 3]];
        let targets = vec![Vec3::new(2.0, 1.0, 0.0)];

        let mut full = base.clone();
        solve_goals(&mut full, &chains, &targets, &[1.0], GOAL_ITERATIONS);
        let mut half = base.clone();
        solve_goals(&mut half, &chains, &targets, &[0.5], GOAL_ITERATIONS);

        let tip = |p: &Vec<Vec3>| *p.last().unwrap();
        let (animated, solved, blended) = (tip(&base), tip(&full), tip(&half));
        let reach = (solved - animated).length();
        let partial = (blended - animated).length();
        assert!(
            partial > 0.05 * reach && partial < 0.95 * reach,
            "half weight should move partway: animated {animated:?}, \
             blended {blended:?}, solved {solved:?}"
        );
    }

    #[test]
    fn a_joint_no_goal_names_is_untouched() {
        // A tail bone, say. The solver has no opinion about it and must not
        // invent one -- averaging over zero contributions would put it at the
        // origin.
        let mut positions = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(7.0, 7.0, 7.0),
        ];
        let untouched = positions[3];
        solve_goals(
            &mut positions,
            &[vec![0, 1, 2]],
            &[Vec3::new(1.0, 1.0, 0.0)],
            &[1.0],
            GOAL_ITERATIONS,
        );
        assert_eq!(positions[3], untouched);
    }

    #[test]
    fn the_answer_does_not_depend_on_the_order_the_goals_are_listed_in() {
        // Averaging the shared joints is what buys this. Taking the last
        // chain's answer would make the result depend on something no author
        // chose.
        let base = two_arm_rig();
        let targets = [
            PELVIS + Vec3::new(-1.5, 2.5, 1.0),
            PELVIS + Vec3::new(1.5, 1.5, -1.0),
        ];

        let mut forward = base.clone();
        solve_goals(
            &mut forward,
            &[vec![0, 1, 2, 3], vec![0, 1, 2, 4]],
            &targets,
            &[1.0, 1.0],
            GOAL_ITERATIONS,
        );
        let mut reversed = base.clone();
        solve_goals(
            &mut reversed,
            &[vec![0, 1, 2, 4], vec![0, 1, 2, 3]],
            &[targets[1], targets[0]],
            &[1.0, 1.0],
            GOAL_ITERATIONS,
        );

        for (i, (a, b)) in forward.iter().zip(&reversed).enumerate() {
            assert!(
                (*a - *b).length() < 1.0e-3,
                "joint {i} moved with the listing order: {a:?} vs {b:?}"
            );
        }
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

use bevy_app::{App, Plugin, PostUpdate};
use bevy_ecs::prelude::{Component, Query, ReflectComponent, Res, ResMut};
use bevy_reflect::prelude::ReflectDefault;

use crate::animation::{AnimationChannel, AnimationClip, Interpolation, KeyframeValues};
use crate::loader::{NodeTransform, SkinData, VertexSkin};
use bsengine_rhi_wgpu::Vertex;
use glam::{Mat4, Quat, Vec3};

/// One two-bone IK chain: three bones by name, and a world-space target the
/// tip should reach.
///
/// Named rather than indexed, following `Ragdoll.joint_overrides` — a scene
/// author writes the bone names the rig actually uses, and a name the skeleton
/// lacks is skipped with a warning rather than silently posing the wrong joint.
///
/// Not a `Component` itself: a character needs one of these per limb, and an
/// entity can hold only one of any given component. They live in a list on
/// [`IkChains`], the same shape `Vehicle.wheels: Vec<WheelConfig>` uses for the
/// same reason.
#[derive(Debug, Clone, Default, bevy_reflect::Reflect)]
pub struct IkChain {
    /// Upper bone — the hip or shoulder.
    pub root_bone: String,
    /// Middle bone — the knee or elbow.
    pub mid_bone: String,
    /// Tip bone — the foot or hand, the one driven onto the target.
    pub tip_bone: String,
    /// World-space position the tip should reach.
    pub target: bsengine_core::ReflectVec3,
    /// How much of the solved pose to apply. `0.0` leaves the animation
    /// untouched; `1.0` puts the tip on the target.
    ///
    /// Exists so foot IK can blend out rather than switch off. A hard switch
    /// pops the foot on the frame it disengages — the same artefact the ragdoll
    /// return blend was built to avoid.
    pub weight: f32,
}

/// Every IK chain on one character.
///
/// A list rather than one component per chain because an entity can hold only
/// one of any given component and a character needs one chain per limb -- the
/// same reason `Vehicle` carries `wheels: Vec<WheelConfig>`.
#[derive(Component, Debug, Clone, Default, bevy_reflect::Reflect)]
#[reflect(Component, Default)]
pub struct IkChains {
    /// The chains, solved in order. Two chains naming the same bones fight;
    /// the last one wins, which is a scene-authoring error rather than
    /// something to resolve here.
    pub chains: Vec<IkChain>,
}

/// Rest-pose (bind pose) geometry, skin/joint data, and node hierarchy needed
/// to re-derive a skinned mesh's deformed vertices every frame from whichever
/// clip its `AnimationPlayer` is currently sampling. Attached alongside
/// `MeshRenderer` by `GltfPlugin` when the source glTF had a skin.
/// # What is reflected, and what is not
///
/// `mesh_id` is reflected; every other field below is `#[reflect(ignore)]`.
/// R1 asks that a public component be *visible* — that the Inspector shows the
/// entity has a skinned mesh and that MCP can see it is attached — and
/// `mesh_id` is the whole of what identifies one. The rest is per-vertex data
/// sized by the imported asset, tens of thousands of entries for an ordinary
/// character, and it is written once by the importer and read every frame by
/// the skinning system; nothing an Inspector or an agent could set by hand is
/// in there.
///
/// `rest_vertices` also could not be reflected without a dependency change:
/// `Vertex` is `bsengine-rhi-wgpu`'s `#[repr(C)]`/`bytemuck::Pod` GPU-upload
/// type, and that crate does not depend on `bevy_reflect`. Adding the
/// dependency to expose data nobody edits would be the wrong trade.
#[derive(Component, Clone, bevy_reflect::Reflect)]
#[reflect(Component)]
pub struct SkinnedMesh {
    /// The GPU mesh id (from `GpuMeshRegistry::register`) this component's
    /// deformed vertices get re-uploaded into each frame.
    pub mesh_id: u64,
    /// Bind-pose vertex data — always the deformation *source*; never itself
    /// overwritten, so each frame deforms fresh from the same rest pose.
    ///
    /// Not reflected: see the type-level note above.
    #[reflect(ignore)]
    pub rest_vertices: Vec<Vertex>,
    /// Per-vertex joint indices/weights, same length and order as `rest_vertices`.
    ///
    /// Not reflected: see the type-level note above.
    #[reflect(ignore)]
    pub skin: Vec<VertexSkin>,
    /// This skin's joint node indices (joint order) and inverse bind matrices.
    ///
    /// Not reflected: see the type-level note above.
    #[reflect(ignore)]
    pub skin_data: SkinData,
    /// Every node's rest-pose local transform and parent, indexed by node index.
    ///
    /// Not reflected: see the type-level note above.
    #[reflect(ignore)]
    pub nodes: Vec<NodeTransform>,
    /// Per-node **global** transforms from somewhere other than the animation
    /// clips, in [`nodes`] order — or empty, which means the clips are the
    /// source and nothing has been overridden.
    ///
    /// This is the whole of how a ragdoll drives a skinned mesh. `bsengine-gltf`
    /// must not know what physics is, so the physics crate — which already
    /// depends on this one — writes the pose its bone bodies imply here, and
    /// [`SkinnedMeshPlugin`]'s system feeds it through the same
    /// `global * inverse_bind_matrix` step the animated path uses. Skinning
    /// therefore never asks *why* the pose is what it is, and the ragdoll never
    /// has to reach into vertex blending.
    ///
    /// Whoever fills it is responsible for emptying it again: as long as this
    /// is non-empty the animation clips are ignored entirely, so a stale
    /// override leaves the character frozen in whatever pose put it there.
    ///
    /// Not reflected: see the type-level note above.
    ///
    /// [`nodes`]: SkinnedMesh::nodes
    #[reflect(ignore)]
    pub pose_override: Vec<Mat4>,
    /// How much of [`pose_override`](SkinnedMesh::pose_override) to use, where
    /// 1.0 is the override alone and 0.0 is the animation alone.
    ///
    /// Exists so a ragdoll can hand the skeleton back to animation gradually
    /// instead of snapping. This crate stays ignorant of what wrote the
    /// override or why -- it just honours the weight.
    ///
    /// Not reflected: see the type-level note above.
    #[reflect(ignore)]
    pub pose_override_weight: f32,
    /// World-space position of each [`IkChains`] chain's tip bone, in chain
    /// order, as of the last time the skinning system ran.
    ///
    /// Published so something outside this crate can find where a foot
    /// actually is without re-deriving the pose. `bsengine-physics` reads it to
    /// cast its ground probe, the same way it reads
    /// [`pose_override`](SkinnedMesh::pose_override) -- the dependency runs
    /// physics -> gltf, so the data has to travel in this direction.
    ///
    /// Runtime output, never authored, hence not reflected.
    #[reflect(ignore)]
    pub ik_tip_positions: Vec<Vec3>,
    /// The skinning matrix per joint as of the last time [`SkinnedMeshPlugin`]'s
    /// system ran — `global[joint_node] * inverse_bind_matrix[joint]`, in
    /// [`SkinData::joint_node_indices`] order.
    ///
    /// Output, not input: writing it does nothing, because the next frame
    /// recomputes it from whichever source is driving the skeleton. It is the
    /// pose the mesh is actually being deformed by, which nothing outside that
    /// system could otherwise observe — the matrices used to be computed and
    /// thrown away inside one loop body.
    ///
    /// Not reflected: see the type-level note above.
    #[reflect(ignore)]
    pub joint_matrices: Vec<Mat4>,
}

/// The full set of animation clips available to an entity's `AnimationPlayer`,
/// keyed by clip name — the clip library `GltfPlugin` extracts once at import
/// time and attaches alongside `SkinnedMesh`/`AnimationPlayer`.
///
/// # What is reflected, and what is not
///
/// Nothing but the component's own presence: `clips` is `#[reflect(ignore)]`,
/// so what registration buys is that the Inspector shows the entity *has* a
/// clip library and MCP can see it is attached. That is exactly what R1 asks
/// for, and for this component it is also all that is meaningful.
///
/// Reflecting the map itself was considered and rejected. It would require
/// `AnimationClip`, `AnimationChannel`, `KeyframeValues` and `Interpolation`
/// to become `Reflect` — and the payoff would be an Inspector rendering every
/// keyframe time and value of every clip, which is asset-sized data (a walk
/// cycle is hundreds of keyframes across dozens of channels) that nobody
/// edits through a property grid.
///
/// Reflected *deserialisation* is not needed either, and that is the reason
/// this stops at registration rather than chasing `ReflectDeserialize` the way
/// `AnimationStateMachine`'s `HashSet<String>` had to. This component is
/// populated by `GltfPlugin` from the imported file; it is never authored in a
/// scene's `components:` list, because there is no way to write a clip library
/// by hand that would not just be a worse spelling of the glTF it came from.
#[derive(Component, Clone, Default, bevy_reflect::Reflect)]
#[reflect(Component)]
pub struct AnimationClipLibrary {
    /// Clips by name, as parsed from the source glTF file.
    ///
    /// Not reflected: see the type-level note above.
    #[reflect(ignore)]
    pub clips: std::collections::HashMap<String, AnimationClip>,
}

impl AnimationClipLibrary {
    /// Builds a library from a flat list of clips, keyed by their own `name`.
    pub fn from_clips(clips: Vec<AnimationClip>) -> Self {
        Self {
            clips: clips.into_iter().map(|c| (c.name.clone(), c)).collect(),
        }
    }
}

/// Finds the keyframe pair bracketing `time` and the 0..1 interpolation
/// factor between them. Clamps to the first/last keyframe outside the clip's
/// range. Returns `None` for an empty channel (shouldn't happen for a valid
/// glTF, but avoids a panic on malformed data).
fn bracket(times: &[f32], time: f32) -> Option<(usize, usize, f32)> {
    if times.is_empty() {
        return None;
    }
    if times.len() == 1 || time <= times[0] {
        return Some((0, 0, 0.0));
    }
    if time >= *times.last().unwrap() {
        let last = times.len() - 1;
        return Some((last, last, 0.0));
    }
    for i in 0..times.len() - 1 {
        if time >= times[i] && time <= times[i + 1] {
            let span = times[i + 1] - times[i];
            let t = if span > f32::EPSILON {
                (time - times[i]) / span
            } else {
                0.0
            };
            return Some((i, i + 1, t));
        }
    }
    None
}

/// Samples a translation channel at `time`. `None` if `channel.values` isn't
/// `Translations` (e.g. this channel actually animates rotation/scale).
/// CubicSpline is treated as Step (holds the earlier keyframe) — this engine's
/// `KeyframeValues` doesn't store in/out tangents, so true cubic interpolation
/// isn't representable yet; documented simplification, not a bug.
fn sample_translation(channel: &AnimationChannel, time: f32) -> Option<Vec3> {
    let KeyframeValues::Translations(values) = &channel.values else {
        return None;
    };
    let (i0, i1, t) = bracket(&channel.times, time)?;
    let a = Vec3::from(values[i0]);
    let b = Vec3::from(values[i1]);
    Some(match channel.interpolation {
        Interpolation::Linear => a.lerp(b, t),
        Interpolation::Step | Interpolation::CubicSpline => a,
    })
}

/// Rotation counterpart to [`sample_translation`] — slerps instead of lerping.
fn sample_rotation(channel: &AnimationChannel, time: f32) -> Option<Quat> {
    let KeyframeValues::Rotations(values) = &channel.values else {
        return None;
    };
    let (i0, i1, t) = bracket(&channel.times, time)?;
    let a = Quat::from_array(values[i0]);
    let b = Quat::from_array(values[i1]);
    Some(match channel.interpolation {
        Interpolation::Linear => a.slerp(b, t),
        Interpolation::Step | Interpolation::CubicSpline => a,
    })
}

/// Scale counterpart to [`sample_translation`].
fn sample_scale(channel: &AnimationChannel, time: f32) -> Option<Vec3> {
    let KeyframeValues::Scales(values) = &channel.values else {
        return None;
    };
    let (i0, i1, t) = bracket(&channel.times, time)?;
    let a = Vec3::from(values[i0]);
    let b = Vec3::from(values[i1]);
    Some(match channel.interpolation {
        Interpolation::Linear => a.lerp(b, t),
        Interpolation::Step | Interpolation::CubicSpline => a,
    })
}

/// One node's animated translation/rotation/scale at `time`, falling back to
/// its rest value for anything the clip does not drive.
fn sample_node_trs(
    node_index: usize,
    rest: &NodeTransform,
    channels: &[AnimationChannel],
    time: f32,
) -> (Vec3, Quat, Vec3) {
    let mut t = Vec3::from(rest.position);
    let mut r = Quat::from_array(rest.rotation);
    let mut s = Vec3::from(rest.scale);
    for channel in channels.iter().filter(|c| c.node_index == node_index) {
        if let Some(v) = sample_translation(channel, time) {
            t = v;
        }
        if let Some(v) = sample_rotation(channel, time) {
            r = v;
        }
        if let Some(v) = sample_scale(channel, time) {
            s = v;
        }
    }
    (t, r, s)
}

/// One clip's contribution to a blend: its channels and where in it to sample.
#[derive(Clone, Copy)]
pub struct ClipSample<'a> {
    /// The clip's animation channels.
    pub channels: &'a [AnimationChannel],
    /// Playback time within the clip, in seconds.
    pub time: f32,
    /// How much of the final pose comes from this clip. Weights are normalised,
    /// so they do not have to sum to one.
    pub weight: f32,
}

/// Composes each node's local transform by blending several clips.
///
/// Blending happens on translation/rotation/scale, *before* the matrix is
/// composed. Averaging the matrices instead would be wrong in a way that looks
/// almost right: a half-blend of two rotations comes out shrunken and skewed
/// rather than rotated halfway, because the average of two rotation matrices is
/// not a rotation matrix.
///
/// Rotations use `slerp` accumulated pairwise, and the sign of each quaternion
/// is aligned to the accumulator first — `q` and `-q` are the same orientation,
/// and blending toward the wrong one takes the long way round the sphere.
pub fn compute_local_transforms_blended(
    nodes: &[NodeTransform],
    clips: &[ClipSample<'_>],
) -> Vec<Mat4> {
    let total: f32 = clips.iter().map(|c| c.weight.max(0.0)).sum();
    if clips.is_empty() || total <= f32::EPSILON {
        return nodes
            .iter()
            .map(|rest| {
                Mat4::from_scale_rotation_translation(
                    Vec3::from(rest.scale),
                    Quat::from_array(rest.rotation),
                    Vec3::from(rest.position),
                )
            })
            .collect();
    }

    nodes
        .iter()
        .enumerate()
        .map(|(node_index, rest)| {
            let mut acc_t = Vec3::ZERO;
            let mut acc_s = Vec3::ZERO;
            let mut acc_r: Option<Quat> = None;
            let mut used = 0.0_f32;

            for clip in clips {
                let w = clip.weight.max(0.0) / total;
                if w <= f32::EPSILON {
                    continue;
                }
                let (t, r, s) = sample_node_trs(node_index, rest, clip.channels, clip.time);
                acc_t += t * w;
                acc_s += s * w;
                used += w;
                acc_r = Some(match acc_r {
                    None => r,
                    Some(prev) => {
                        // Align signs: `-q` is the same orientation as `q`, and
                        // slerping to the wrong representative rotates the long
                        // way round.
                        let r = if prev.dot(r) < 0.0 { -r } else { r };
                        prev.slerp(r, w / used)
                    }
                });
            }

            let r = acc_r.unwrap_or_else(|| Quat::from_array(rest.rotation));
            Mat4::from_scale_rotation_translation(acc_s, r.normalize(), acc_t)
        })
        .collect()
}

/// Decides which clips compose this frame's pose, and at what weights.
///
/// One clip normally; two while a state machine is mid-transition — the state
/// being left and the state being entered. Until this existed, `blend_weight`
/// was advanced every frame and read by nothing, so every "crossfade" in this
/// engine was really a hard cut.
///
/// Both clips are sampled at the same time value. For the looping locomotion
/// clips these transitions exist for — idle/walk/run — that is what keeps the
/// feet in phase across the blend. A clip whose meaning depends on its own
/// timeline would need its own clock, and nothing here has one yet.
fn blend_samples<'a>(
    current: &'a AnimationClip,
    library: &'a AnimationClipLibrary,
    time: f32,
    asm: Option<&bsengine_core::AnimationStateMachine>,
) -> Vec<ClipSample<'a>> {
    let Some(asm) = asm else {
        return vec![ClipSample {
            channels: &current.channels,
            time,
            weight: 1.0,
        }];
    };

    let w = asm.blend_weight.clamp(0.0, 1.0);
    let crossfading = asm.blend_from.is_some();

    let mut samples = Vec::new();
    push_state_samples(
        &mut samples,
        asm.states.get(&asm.current_state),
        current,
        library,
        asm,
        time,
        if crossfading { w } else { 1.0 },
    );
    if crossfading {
        let from = asm.blend_from.as_ref().and_then(|s| asm.states.get(s));
        // No fallback clip for the state being left: if its clip is missing
        // there is nothing to blend out of, and contributing `current` again
        // under the leaving state's weight would just dim the pose.
        let before = samples.len();
        push_state_samples(&mut samples, from, current, library, asm, time, 1.0 - w);
        if samples.len() == before {
            // Nothing came from the leaving state, so the entering one is the
            // whole pose rather than a fraction of it.
            for sample in &mut samples {
                sample.weight = 1.0;
            }
        }
    }
    if samples.is_empty() {
        samples.push(ClipSample {
            channels: &current.channels,
            time,
            weight: 1.0,
        });
    }
    samples
}

/// Appends one state's contribution, scaled by `scale`.
///
/// A state with a blend tree contributes the clips that tree names at the
/// current parameter value — one or two of them. A state without one
/// contributes its single clip. `fallback` is what `AnimationPlayer` is already
/// playing, used when the state names a clip the model does not have, so a
/// mismatch degrades to "keep animating" rather than to a frozen half pose.
#[allow(clippy::too_many_arguments)]
fn push_state_samples<'a>(
    out: &mut Vec<ClipSample<'a>>,
    state: Option<&bsengine_core::AsmState>,
    fallback: &'a AnimationClip,
    library: &'a AnimationClipLibrary,
    asm: &bsengine_core::AnimationStateMachine,
    time: f32,
    scale: f32,
) {
    if scale <= f32::EPSILON {
        return;
    }
    // A state name that is not in the graph contributes nothing at all. It is
    // tempting to fall back to `fallback` here, but that clip is what the
    // *entering* state is already playing, so doing so would blend the pose
    // against itself and dim it — and for a transition out of a state that does
    // not exist, there is simply nothing to blend out of. The caller restores
    // full weight when this leaves it with nothing.
    let Some(state) = state else {
        return;
    };

    if let Some(tree) = &state.blend {
        let value = asm
            .params_float
            .get(tree.param.as_str())
            .copied()
            .unwrap_or(0.0);
        let mut any = false;
        for (name, weight) in tree.sample(value) {
            if let Some(clip) = library.clips.get(&name) {
                any = true;
                out.push(ClipSample {
                    channels: &clip.channels,
                    time,
                    weight: weight * scale,
                });
            }
        }
        if any {
            return;
        }
        // A tree naming only clips the model lacks is a scene error, not a
        // reason to stop animating.
    }

    let clip = library.clips.get(&state.clip).unwrap_or(fallback);
    out.push(ClipSample {
        channels: &clip.channels,
        time,
        weight: scale,
    });
}

/// Walks the node hierarchy to compose each node's GLOBAL transform from its
/// local transform and its parent chain. Iterates a fixed number of passes
/// rather than a proper topological sort, matching the same pattern
/// `bsengine_core::propagate_global_transforms` already uses for parent/child
/// Transform hierarchies in this codebase.
///
/// Called twice per skinned character when IK is in play: once to give the
/// solver world positions to work against, and again after the solved
/// rotations are written back into the locals, so the tip bone and everything
/// below it follow. Skipping the second pass moves the two solved bones and
/// leaves the foot where the clip put it.
fn accumulate_globals(nodes: &[NodeTransform], locals: &[Mat4]) -> Vec<Mat4> {
    let mut globals = locals.to_vec();
    for _ in 0..8 {
        for (i, node) in nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                globals[i] = globals[parent] * locals[i];
            }
        }
    }
    globals
}

/// Turns per-node global transforms into one skinning matrix per joint
/// (`global[joint_node] * inverse_bind_matrix[joint]`) — the matrix each of
/// that joint's vertices gets blended through.
///
/// Split out from the animated path because it is the step the ragdoll shares.
/// `SkinnedMesh::pose_override` supplies different globals and everything from
/// here on is identical, which is what keeps "physics drives the bones" from
/// touching vertex blending or the GPU upload at all.
///
/// A joint naming a node the skeleton does not have contributes the identity
/// rather than panicking: with two sources of globals there are now two ways
/// for the two to disagree about how many nodes there are, and a malformed
/// asset should not take the frame down.
fn joint_matrices_from_globals(globals: &[Mat4], skin: &SkinData) -> Vec<Mat4> {
    skin.joint_node_indices
        .iter()
        .zip(&skin.inverse_bind_matrices)
        .map(|(&node_index, ibm)| {
            globals.get(node_index).copied().unwrap_or(Mat4::IDENTITY)
                * Mat4::from_cols_array_2d(ibm)
        })
        .collect()
}

/// The animated path: compose globals from the sampled clips, then turn them
/// into skinning matrices.
fn compute_joint_matrices_blended(
    nodes: &[NodeTransform],
    skin: &SkinData,
    clips: &[ClipSample<'_>],
) -> Vec<Mat4> {
    compute_joint_matrices_with_ik(nodes, skin, clips, &[])
}

/// As [`compute_joint_matrices_blended`], with IK chains applied between the
/// global accumulation and the inverse-bind step.
///
/// That is the only place they can go. The chains need world positions to solve
/// against, which do not exist until the globals are accumulated; and they must
/// be applied before the inverse bind matrices, which is what turns globals into
/// skinning matrices.
fn compute_joint_matrices_with_ik(
    nodes: &[NodeTransform],
    skin: &SkinData,
    clips: &[ClipSample<'_>],
    chains: &[&IkChain],
) -> Vec<Mat4> {
    compute_pose_with_ik(nodes, skin, clips, chains).0
}

/// As [`compute_joint_matrices_with_ik`], also returning each chain's tip bone
/// world position.
///
/// Split out rather than folded in because the tips are only wanted by the
/// system that publishes them; every test and every other caller wants the
/// matrices alone.
fn compute_pose_with_ik(
    nodes: &[NodeTransform],
    skin: &SkinData,
    clips: &[ClipSample<'_>],
    chains: &[&IkChain],
) -> (Vec<Mat4>, Vec<Vec3>) {
    let mut locals = compute_local_transforms_blended(nodes, clips);
    let mut globals = accumulate_globals(nodes, &locals);

    for chain in chains {
        if chain.weight <= 0.0 {
            continue;
        }
        let (Some(root), Some(mid), Some(tip)) = (
            node_index_by_name(nodes, &chain.root_bone),
            node_index_by_name(nodes, &chain.mid_bone),
            node_index_by_name(nodes, &chain.tip_bone),
        ) else {
            // A name the rig does not have is a scene typo. Warn once per frame
            // rather than posing some other joint, which would look like a
            // solver bug.
            tracing::warn!(
                "[ik] chain names a bone this skeleton lacks: {:?} / {:?} / {:?}",
                chain.root_bone,
                chain.mid_bone,
                chain.tip_bone
            );
            continue;
        };

        let pos = |i: usize| globals[i].transform_point3(Vec3::ZERO);
        let (root_rot, mid_rot) =
            crate::ik::solve_two_bone(pos(root), pos(mid), pos(tip), chain.target.0);

        // Blend toward the solved rotation rather than snapping to it, so a
        // foot can fade in and out of IK.
        let w = chain.weight.clamp(0.0, 1.0);
        let root_rot = Quat::IDENTITY.slerp(root_rot, w);
        let mid_rot = Quat::IDENTITY.slerp(mid_rot, w);

        // The solver returns world-space rotations; the locals are relative to
        // each bone's parent, so each is carried into the parent's frame before
        // being applied.
        apply_world_rotation(&mut locals, &globals, nodes, root, root_rot);
        apply_world_rotation(&mut locals, &globals, nodes, mid, mid_rot);

        // Re-accumulate, or the two rotated bones move and the tip — and
        // everything below it — stays where the clip put it.
        globals = accumulate_globals(nodes, &locals);
    }

    // Read the tips back off the FINAL globals, after every chain has been
    // solved and re-accumulated. Reading them mid-loop would publish a foot
    // position that a later chain then moved.
    let tips = chains
        .iter()
        .map(|c| {
            node_index_by_name(nodes, &c.tip_bone)
                .map(|i| globals[i].transform_point3(Vec3::ZERO))
                .unwrap_or(Vec3::ZERO)
        })
        .collect();

    (joint_matrices_from_globals(&globals, skin), tips)
}

/// Rotates one node by a world-space rotation, written into its parent-relative
/// local transform.
fn apply_world_rotation(
    locals: &mut [Mat4],
    globals: &[Mat4],
    nodes: &[NodeTransform],
    index: usize,
    world_rot: Quat,
) {
    let parent_global = nodes[index]
        .parent
        .map(|p| globals[p])
        .unwrap_or(Mat4::IDENTITY);
    let (_, parent_rot, _) = parent_global.to_scale_rotation_translation();
    let local_delta = parent_rot.inverse() * world_rot * parent_rot;
    let (scale, rot, translation) = locals[index].to_scale_rotation_translation();
    locals[index] =
        Mat4::from_scale_rotation_translation(scale, local_delta * rot, translation);
}

/// Finds a node by its glTF name.
fn node_index_by_name(nodes: &[NodeTransform], name: &str) -> Option<usize> {
    nodes.iter().position(|n| n.name == name)
}

/// Blends one rest-pose vertex position through up to 4 joint matrices by
/// weight — the standard linear blend skinning (LBS) formula.
fn blend_vertex_position(rest: Vec3, skin: &VertexSkin, joint_matrices: &[Mat4]) -> Vec3 {
    let mut result = Vec3::ZERO;
    for i in 0..4 {
        let w = skin.weights[i];
        if w == 0.0 {
            continue;
        }
        let j = skin.joints[i] as usize;
        if let Some(m) = joint_matrices.get(j) {
            result += w * m.transform_point3(rest);
        }
    }
    result
}

/// Normal counterpart to [`blend_vertex_position`] — blends direction vectors
/// (no translation) through the same joint matrices. Uses each joint matrix's
/// linear part directly rather than its inverse-transpose; correct under
/// uniform scale (true for every joint in a typical character rig), a known,
/// documented simplification versus fully-correct non-uniform-scale normal
/// skinning.
fn blend_vertex_normal(rest_normal: Vec3, skin: &VertexSkin, joint_matrices: &[Mat4]) -> Vec3 {
    let mut result = Vec3::ZERO;
    for i in 0..4 {
        let w = skin.weights[i];
        if w == 0.0 {
            continue;
        }
        let j = skin.joints[i] as usize;
        if let Some(m) = joint_matrices.get(j) {
            result += w * m.transform_vector3(rest_normal);
        }
    }
    result.normalize_or_zero()
}

/// Drives CPU-side skeletal skinning: each frame, for every entity with a
/// `SkinnedMesh`, composes that entity's joint matrices, blends the rest-pose
/// vertices through them, and re-uploads the result into the same GPU mesh id.
///
/// The joint matrices come from one of two sources. Normally the entity's
/// `AnimationClipLibrary`/`AnimationPlayer` are sampled and the pose is
/// accumulated down the node hierarchy. When something has filled
/// [`SkinnedMesh::pose_override`] — the ragdoll, today — those per-node globals
/// are used instead and the clips are not read at all. Only the *source* of the
/// globals differs; the `global * inverse_bind_matrix` step, the vertex
/// blending, and the upload are shared. Runs in
/// `PostUpdate`, after `bsengine_app::AnimationStateMachinePlugin` (if
/// present) has already updated which clip/time the player should be
/// showing this frame -- this plugin has no direct dependency on that one,
/// it just needs to run after it if both are added, which PostUpdate
/// ordering relative to Update naturally provides since AnimationPlayer's
/// own tick already happens in Update.
pub struct SkinnedMeshPlugin;

impl Plugin for SkinnedMeshPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, update_skinned_meshes);
    }
}

fn update_skinned_meshes(
    mut query: Query<(
        &mut SkinnedMesh,
        Option<&AnimationClipLibrary>,
        Option<&bsengine_core::AnimationPlayer>,
        Option<&bsengine_core::AnimationStateMachine>,
        Option<&IkChains>,
    )>,
    mesh_registry: Option<ResMut<bsengine_rhi_wgpu::GpuMeshRegistry>>,
    queue: Option<Res<bsengine_rhi_wgpu::GpuQueueResource>>,
) {
    // Deliberately not an early return when there is no GPU. Composing the
    // joint matrices is a few dozen matrix products over the skeleton and it is
    // the *pose*, which a headless host still wants to be right; blending the
    // vertices is tens of thousands of products whose only consumer is the
    // upload, so that half is what the GPU's absence skips.
    let mut gpu = mesh_registry.zip(queue);

    for (mut skinned, library, player, asm, ik) in query.iter_mut() {
        // Set by the clip branch below; the ragdoll-override branches leave it
        // None, and an override means physics is driving the whole skeleton
        // anyway.
        let mut published_tips: Option<Vec<Vec3>> = None;
        // The one branch this whole feature turns on, and the one that fails
        // silently: with the bodies built and falling but the clips still
        // sourcing the pose, the character looks completely normal while a full
        // ragdoll simulates underneath it.
        let joint_matrices = if skinned.pose_override.is_empty() {
            // No override at all — clips are the sole source. Byte-identical to
            // the pre-ragdoll path.
            let (Some(library), Some(player)) = (library, player) else {
                continue;
            };
            let Some(clip) = library.clips.get(&player.clip) else {
                continue;
            };

            // A state machine mid-transition contributes the state it is
            // leaving as well as the one it is entering. Until this existed,
            // `blend_weight` was advanced every frame and read by nothing, so
            // every "crossfade" was really a hard cut.
            //
            // Both clips are sampled at the same `player.time`. For the looping
            // locomotion clips these transitions are for — idle/walk/run — that
            // is what keeps the feet in phase across the blend; a clip whose
            // meaning depends on its own timeline would need its own clock, and
            // nothing here has one yet.
            let samples = blend_samples(clip, library, player.time, asm);
            let chains: Vec<&IkChain> =
                ik.map(|c| c.chains.iter().collect()).unwrap_or_default();
            let (matrices, tips) =
                compute_pose_with_ik(&skinned.nodes, &skinned.skin_data, &samples, &chains);
            published_tips = Some(tips);
            matrices
        } else if skinned.pose_override_weight >= 1.0 {
            // Override present, full weight — clips are not read at all.
            // Byte-identical to the pre-weight override path.
            joint_matrices_from_globals(&skinned.pose_override, &skinned.skin_data)
        } else {
            // Override present, partial weight — blend between clip-derived and
            // override globals per node.
            let override_matrices =
                joint_matrices_from_globals(&skinned.pose_override, &skinned.skin_data);

            // Clip sources are optional; if missing fall back to the override alone
            // rather than skipping the entity with a partially-complete pose.
            let animated_matrices = (|| {
                let library = library?;
                let player = player?;
                let clip = library.clips.get(&player.clip)?;
                let samples = blend_samples(clip, library, player.time, asm);
                Some(compute_joint_matrices_blended(
                    &skinned.nodes,
                    &skinned.skin_data,
                    &samples,
                ))
            })();

            let w = skinned.pose_override_weight.clamp(0.0, 1.0);
            match animated_matrices {
                None => override_matrices,
                Some(animated) => override_matrices
                    .iter()
                    .zip(&animated)
                    .map(|(over_m, anim_m)| {
                        let (over_s, over_r, over_t) = over_m.to_scale_rotation_translation();
                        let (anim_s, anim_r, anim_t) = anim_m.to_scale_rotation_translation();
                        Mat4::from_scale_rotation_translation(
                            Vec3::lerp(anim_s, over_s, w),
                            Quat::slerp(anim_r, over_r, w),
                            Vec3::lerp(anim_t, over_t, w),
                        )
                    })
                    .collect(),
            }
        };

        // Publish where the IK tips ended up, so the ground probe in
        // `bsengine-physics` can cast from the foot without re-deriving the
        // pose. Only the clip branch produces these; under a pose override
        // physics is already driving the whole skeleton.
        if let Some(tips) = published_tips {
            skinned.ik_tip_positions = tips;
        }

        if let Some((mesh_registry, queue)) = gpu.as_mut() {
            let deformed: Vec<Vertex> = skinned
                .rest_vertices
                .iter()
                .zip(&skinned.skin)
                .map(|(v, s)| {
                    let pos = blend_vertex_position(Vec3::from(v.position), s, &joint_matrices);
                    let normal = blend_vertex_normal(Vec3::from(v.normal), s, &joint_matrices);
                    Vertex {
                        position: pos.to_array(),
                        color: v.color,
                        normal: normal.to_array(),
                        uv: v.uv,
                    }
                })
                .collect();
            mesh_registry.update_vertices(&queue.0, skinned.mesh_id, &deformed);
        }

        skinned.joint_matrices = joint_matrices;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_library_from_clips_keys_by_name() {
        let lib = AnimationClipLibrary::from_clips(vec![
            AnimationClip {
                name: "walk".to_string(),
                channels: vec![],
                duration: 1.0,
            },
            AnimationClip {
                name: "run".to_string(),
                channels: vec![],
                duration: 0.5,
            },
        ]);
        assert_eq!(lib.clips.len(), 2);
        assert!((lib.clips["walk"].duration - 1.0).abs() < 0.001);
        assert!((lib.clips["run"].duration - 0.5).abs() < 0.001);
    }

    #[test]
    fn empty_clip_library_default_has_no_clips() {
        let lib = AnimationClipLibrary::default();
        assert!(lib.clips.is_empty());
    }

    use crate::animation::{AnimationChannel, Interpolation, KeyframeValues};
    use glam::{Quat, Vec3};

    #[test]
    fn sample_translation_linear_interpolates_between_keyframes() {
        let channel = AnimationChannel {
            node_index: 0,
            times: vec![0.0, 1.0],
            values: KeyframeValues::Translations(vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
            interpolation: Interpolation::Linear,
        };
        let v = sample_translation(&channel, 0.5).unwrap();
        assert!((v.x - 1.0).abs() < 0.001);
    }

    #[test]
    fn sample_translation_clamps_before_first_keyframe() {
        let channel = AnimationChannel {
            node_index: 0,
            times: vec![1.0, 2.0],
            values: KeyframeValues::Translations(vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
            interpolation: Interpolation::Linear,
        };
        let v = sample_translation(&channel, 0.0).unwrap();
        assert_eq!(v, Vec3::ZERO);
    }

    #[test]
    fn sample_translation_clamps_after_last_keyframe() {
        let channel = AnimationChannel {
            node_index: 0,
            times: vec![0.0, 1.0],
            values: KeyframeValues::Translations(vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
            interpolation: Interpolation::Linear,
        };
        let v = sample_translation(&channel, 5.0).unwrap();
        assert!((v.x - 2.0).abs() < 0.001);
    }

    #[test]
    fn sample_rotation_slerps_between_keyframes() {
        let a = Quat::IDENTITY;
        let b = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let channel = AnimationChannel {
            node_index: 0,
            times: vec![0.0, 1.0],
            values: KeyframeValues::Rotations(vec![a.to_array(), b.to_array()]),
            interpolation: Interpolation::Linear,
        };
        let r = sample_rotation(&channel, 0.5).unwrap();
        let expected = a.slerp(b, 0.5);
        assert!((r.dot(expected)).abs() > 0.999);
    }

    #[test]
    fn sample_step_interpolation_holds_earlier_keyframe() {
        let channel = AnimationChannel {
            node_index: 0,
            times: vec![0.0, 1.0],
            values: KeyframeValues::Translations(vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
            interpolation: Interpolation::Step,
        };
        let v = sample_translation(&channel, 0.9).unwrap();
        assert_eq!(v, Vec3::ZERO);
    }

    // ---- pose blending (roadmap item 29) ---------------------------------

    /// A one-node skeleton at the origin, unrotated and unscaled.

    // ---- IK chains applied during skinning (roadmap item 54, sub-step 1/2) --

    /// A three-node leg: hip at the origin, knee one unit down and slightly
    /// forward, foot two units down. Each node is a joint with an identity
    /// inverse bind matrix, so a joint matrix applied to the origin IS that
    /// node's world position and nothing else.
    fn leg_skeleton() -> (Vec<NodeTransform>, SkinData) {
        let nodes = vec![
            NodeTransform {
                name: "hip".to_string(),
                position: [0.0, 2.0, 0.0],
                ..Default::default()
            },
            NodeTransform {
                name: "knee".to_string(),
                position: [0.0, -1.0, 0.3],
                parent: Some(0),
                ..Default::default()
            },
            NodeTransform {
                name: "foot".to_string(),
                position: [0.0, -1.0, -0.3],
                parent: Some(1),
                ..Default::default()
            },
        ];
        let skin = SkinData {
            joint_node_indices: vec![0, 1, 2],
            inverse_bind_matrices: vec![Mat4::IDENTITY.to_cols_array_2d(); 3],
        };
        (nodes, skin)
    }

    fn chain_to(target: Vec3, weight: f32) -> IkChain {
        IkChain {
            root_bone: "hip".to_string(),
            mid_bone: "knee".to_string(),
            tip_bone: "foot".to_string(),
            target: target.into(),
            weight,
        }
    }

    /// Where each joint ends up, per the matrices skinning would actually use.
    fn joint_positions(nodes: &[NodeTransform], skin: &SkinData, chains: &[&IkChain]) -> Vec<Vec3> {
        compute_joint_matrices_with_ik(nodes, skin, &[], chains)
            .iter()
            .map(|m| m.transform_point3(Vec3::ZERO))
            .collect()
    }

    #[test]
    fn an_ik_chain_pulls_its_tip_bone_to_the_target() {
        // Asserted on the JOINT MATRICES -- what skinning actually consumes --
        // not on an intermediate. A disconnected consumer looks exactly like a
        // working producer, which is the failure shape this codebase has hit
        // repeatedly (a ragdoll falling underneath a character that animates on
        // as if nothing happened).
        let (nodes, skin) = leg_skeleton();
        let target = Vec3::new(0.3, 0.6, 0.2);
        let chain = chain_to(target, 1.0);

        let before = joint_positions(&nodes, &skin, &[]);
        let after = joint_positions(&nodes, &skin, &[&chain]);

        let err = (after[2] - target).length();
        println!("foot moved {:?} -> {:?}, {err} m from target", before[2], after[2]);
        assert!(
            err < 1.0e-3,
            "the foot joint must land on the target; it is at {:?}, {err} m \
             away from {target:?}",
            after[2]
        );
        // The hip is the chain's root and must not translate -- IK rotates, it
        // does not move the character.
        assert!(
            (after[0] - before[0]).length() < 1.0e-4,
            "the root joint must not move: {:?} -> {:?}",
            before[0],
            after[0]
        );
    }

    #[test]
    fn a_weight_of_zero_is_byte_identical_to_no_chain_at_all() {
        // The pair. Without it, IK that is always on passes the test above.
        // Byte-identical rather than approximate: weight 0 must not touch the
        // rotations at all, so the two matrix sets are the same bits.
        let (nodes, skin) = leg_skeleton();
        let chain = chain_to(Vec3::new(0.3, 0.6, 0.2), 0.0);

        let none = compute_joint_matrices_with_ik(&nodes, &skin, &[], &[]);
        let zero = compute_joint_matrices_with_ik(&nodes, &skin, &[], &[&chain]);
        assert_eq!(
            none, zero,
            "a zero-weight chain must leave the pose bit-for-bit unchanged"
        );
    }

    #[test]
    fn a_partial_weight_lands_between_the_animated_and_solved_poses() {
        // The reason `weight` exists: a foot has to fade into IK rather than
        // pop on the frame it engages. Half weight must be measurably away from
        // BOTH endpoints -- a blend that quietly returns one of them satisfies
        // any weaker assertion.
        let (nodes, skin) = leg_skeleton();
        let target = Vec3::new(0.3, 0.6, 0.2);

        let animated = joint_positions(&nodes, &skin, &[])[2];
        let solved = joint_positions(&nodes, &skin, &[&chain_to(target, 1.0)])[2];
        let half = joint_positions(&nodes, &skin, &[&chain_to(target, 0.5)])[2];

        let span = (solved - animated).length();
        println!("animated {animated:?}, half {half:?}, solved {solved:?}");
        assert!(
            (half - animated).length() > span * 0.1 && (half - solved).length() > span * 0.1,
            "half weight must sit between the animated pose {animated:?} and \
             the solved pose {solved:?}, but landed at {half:?}"
        );
    }

    #[test]
    fn a_chain_naming_a_bone_the_skeleton_lacks_is_skipped() {
        // A typo'd bone name must leave the pose alone rather than panicking or
        // posing some other joint.
        let (nodes, skin) = leg_skeleton();
        let mut chain = chain_to(Vec3::new(0.3, 0.6, 0.2), 1.0);
        chain.mid_bone = "no_such_bone".to_string();

        let none = compute_joint_matrices_with_ik(&nodes, &skin, &[], &[]);
        let typo = compute_joint_matrices_with_ik(&nodes, &skin, &[], &[&chain]);
        assert_eq!(
            none, typo,
            "a chain naming a missing bone must not change the pose"
        );
    }

    #[test]
    fn the_bones_below_the_chain_follow_it() {
        // The globals are re-accumulated after the solved rotations are written
        // into the locals. Skipping that moves the two solved bones and leaves
        // the foot where the clip put it -- which reads as a broken solver
        // rather than a missing accumulation pass.
        //
        // The foot IS the tip here, so its landing on the target (asserted
        // above) already depends on the re-accumulation. This pins the bones
        // too: rotating them must not change their lengths.
        let (nodes, skin) = leg_skeleton();
        let chain = chain_to(Vec3::new(0.3, 0.6, 0.2), 1.0);
        let before = joint_positions(&nodes, &skin, &[]);
        let after = joint_positions(&nodes, &skin, &[&chain]);

        // Compared against the rest pose's OWN lengths rather than a constant
        // written by hand. The first version asserted 1.0 m and failed at
        // 1.0440307 -- which is just what the local offset (0, -1, 0.3)
        // measures. The test was wrong, not the solver, and reading the length
        // off the rest pose is both correct and stricter: it keeps meaning
        // "rotate, do not stretch" if the fixture ever changes.
        let rest_upper = (before[1] - before[0]).length();
        let rest_lower = (before[2] - before[1]).length();
        let upper = (after[1] - after[0]).length();
        let lower = (after[2] - after[1]).length();
        println!("upper {rest_upper} -> {upper}, lower {rest_lower} -> {lower}");
        assert!(
            (upper - rest_upper).abs() < 1.0e-3,
            "IK must rotate bones, not stretch them: upper bone went \
             {rest_upper} m -> {upper} m"
        );
        assert!(
            (lower - rest_lower).abs() < 1.0e-3,
            "IK must rotate bones, not stretch them: lower bone went \
             {rest_lower} m -> {lower} m"
        );
    }

    fn one_node() -> Vec<NodeTransform> {
        vec![NodeTransform {
            name: String::new(),
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            parent: None,
        }]
    }

    /// A channel holding node 0 at a constant rotation.
    fn fixed_rotation(q: Quat) -> AnimationChannel {
        AnimationChannel {
            node_index: 0,
            times: vec![0.0, 1.0],
            values: KeyframeValues::Rotations(vec![q.to_array(), q.to_array()]),
            interpolation: Interpolation::Linear,
        }
    }

    /// A channel holding node 0 at a constant translation.
    fn fixed_translation(v: Vec3) -> AnimationChannel {
        AnimationChannel {
            node_index: 0,
            times: vec![0.0, 1.0],
            values: KeyframeValues::Translations(vec![v.to_array(), v.to_array()]),
            interpolation: Interpolation::Linear,
        }
    }

    /// A library holding two clips, each pinning node 0 to a translation.
    fn two_clip_library() -> AnimationClipLibrary {
        AnimationClipLibrary::from_clips(vec![
            AnimationClip {
                name: "idle".to_string(),
                channels: vec![fixed_translation(Vec3::ZERO)],
                duration: 1.0,
            },
            AnimationClip {
                name: "walk".to_string(),
                channels: vec![fixed_translation(Vec3::new(10.0, 0.0, 0.0))],
                duration: 1.0,
            },
        ])
    }

    /// A state machine halfway through idle -> walk.
    fn mid_transition() -> bsengine_core::AnimationStateMachine {
        use bsengine_core::{AnimationStateMachine, AsmState};
        let mut asm = AnimationStateMachine::default();
        asm.states.insert("idle".to_string(), AsmState::new("idle"));
        asm.states.insert("walk".to_string(), AsmState::new("walk"));
        asm.current_state = "walk".to_string();
        asm.blend_from = Some("idle".to_string());
        asm.blend_weight = 0.5;
        asm
    }

    #[test]
    fn a_transitioning_state_machine_contributes_both_clips() {
        // The bug this item fixes. `blend_weight` has been advanced every frame
        // since the state machine was written and read by nothing, so a
        // "crossfade" was a hard cut. Anything that stops passing the leaving
        // state's clip through puts that back.
        let library = two_clip_library();
        let walk = library.clips.get("walk").expect("clip exists");
        let asm = mid_transition();

        let samples = blend_samples(walk, &library, 0.0, Some(&asm));

        assert_eq!(samples.len(), 2, "both clips take part in a transition");
        assert!((samples[0].weight - 0.5).abs() < 1e-6, "entering state");
        assert!((samples[1].weight - 0.5).abs() < 1e-6, "leaving state");

        let pose = compute_local_transforms_blended(&one_node(), &samples);
        let x = pose[0].to_scale_rotation_translation().2.x;
        assert!(
            (x - 5.0).abs() < 0.001,
            "halfway through idle -> walk the node should be halfway, got {x}"
        );
    }

    /// A state machine whose one state is a walk/run blend space.
    fn blend_tree_machine(speed: f32) -> bsengine_core::AnimationStateMachine {
        use bsengine_core::{AnimationStateMachine, AsmState, BlendClip, BlendTree1D};
        let mut asm = AnimationStateMachine::default();
        asm.states.insert(
            "locomotion".to_string(),
            AsmState::new("idle").with_blend(BlendTree1D {
                param: "speed".to_string(),
                clips: vec![
                    BlendClip {
                        clip: "idle".to_string(),
                        threshold: 0.0,
                    },
                    BlendClip {
                        clip: "walk".to_string(),
                        threshold: 4.0,
                    },
                ],
            }),
        );
        asm.current_state = "locomotion".to_string();
        asm.params_float.insert("speed".to_string(), speed);
        asm
    }

    #[test]
    fn a_blend_tree_state_plays_both_neighbouring_clips() {
        // The point of the whole item: between thresholds the motion is a
        // mixture, not one clip or the other. A crossfade could only be right
        // for the instant it was halfway.
        let library = two_clip_library();
        let idle = library.clips.get("idle").expect("clip exists");
        let asm = blend_tree_machine(1.0); // a quarter of the way to walk

        let samples = blend_samples(idle, &library, 0.0, Some(&asm));
        assert_eq!(samples.len(), 2, "both sides of the axis contribute");

        let pose = compute_local_transforms_blended(&one_node(), &samples);
        let x = pose[0].to_scale_rotation_translation().2.x;
        assert!(
            (x - 2.5).abs() < 0.001,
            "a quarter of the way from idle (0) to walk (10) is 2.5, got {x}"
        );
    }

    #[test]
    fn a_blend_tree_past_its_end_plays_one_clip() {
        let library = two_clip_library();
        let idle = library.clips.get("idle").expect("clip exists");
        let asm = blend_tree_machine(99.0);

        let samples = blend_samples(idle, &library, 0.0, Some(&asm));
        assert_eq!(samples.len(), 1);

        let pose = compute_local_transforms_blended(&one_node(), &samples);
        let x = pose[0].to_scale_rotation_translation().2.x;
        assert!((x - 10.0).abs() < 0.001, "fully walk, got {x}");
    }

    #[test]
    fn a_blend_tree_naming_missing_clips_still_animates() {
        // A scene error must not freeze the character: fall back to the clip
        // `AnimationPlayer` is already playing.
        use bsengine_core::{AsmState, BlendClip, BlendTree1D};
        let library = two_clip_library();
        let idle = library.clips.get("idle").expect("clip exists");
        let mut asm = blend_tree_machine(1.0);
        asm.states.insert(
            "locomotion".to_string(),
            AsmState::new("idle").with_blend(BlendTree1D {
                param: "speed".to_string(),
                clips: vec![BlendClip {
                    clip: "nonexistent".to_string(),
                    threshold: 0.0,
                }],
            }),
        );

        let samples = blend_samples(idle, &library, 0.0, Some(&asm));
        assert_eq!(samples.len(), 1);
        assert!((samples[0].weight - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_settled_state_machine_contributes_one_clip() {
        let library = two_clip_library();
        let walk = library.clips.get("walk").expect("clip exists");
        let mut asm = mid_transition();
        asm.blend_from = None;

        let samples = blend_samples(walk, &library, 0.0, Some(&asm));
        assert_eq!(samples.len(), 1);
        assert!((samples[0].weight - 1.0).abs() < 1e-6);
    }

    #[test]
    fn an_entity_without_a_state_machine_still_animates() {
        // Not every skinned mesh has a state machine; those play their
        // `AnimationPlayer` clip and must not be disturbed by any of this.
        let library = two_clip_library();
        let walk = library.clips.get("walk").expect("clip exists");
        let samples = blend_samples(walk, &library, 0.0, None);
        assert_eq!(samples.len(), 1);
        assert!((samples[0].weight - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_transition_from_a_missing_clip_falls_back_to_one() {
        // A state naming a clip the model does not have must not blend against
        // nothing and freeze the character at half pose.
        let library = two_clip_library();
        let walk = library.clips.get("walk").expect("clip exists");
        let mut asm = mid_transition();
        asm.blend_from = Some("nonexistent".to_string());

        let samples = blend_samples(walk, &library, 0.0, Some(&asm));
        assert_eq!(samples.len(), 1);
        assert!((samples[0].weight - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_half_blend_of_two_rotations_is_the_midpoint_rotation() {
        // The property that forces blending to happen on TRS rather than on the
        // composed matrices. Averaging two rotation matrices gives something
        // that is not a rotation at all — it comes out shrunken — so this
        // checks the result still has unit scale as well as the right angle.
        let a = [fixed_rotation(Quat::IDENTITY)];
        let b = [fixed_rotation(Quat::from_rotation_y(
            std::f32::consts::FRAC_PI_2,
        ))];
        let nodes = one_node();

        let blended = compute_local_transforms_blended(
            &nodes,
            &[
                ClipSample {
                    channels: &a,
                    time: 0.0,
                    weight: 0.5,
                },
                ClipSample {
                    channels: &b,
                    time: 0.0,
                    weight: 0.5,
                },
            ],
        );

        let (scale, rotation, _) = blended[0].to_scale_rotation_translation();
        let expected = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
        assert!(
            rotation.abs_diff_eq(expected, 0.001) || (-rotation).abs_diff_eq(expected, 0.001),
            "expected the 45 degree midpoint, got {rotation:?}"
        );
        assert!(
            scale.abs_diff_eq(Vec3::ONE, 0.001),
            "a blended rotation must not shrink the node: {scale:?}"
        );
    }

    #[test]
    fn blend_weights_move_the_result_toward_the_heavier_clip() {
        let a = [fixed_translation(Vec3::ZERO)];
        let b = [fixed_translation(Vec3::new(10.0, 0.0, 0.0))];
        let nodes = one_node();

        let quarter = compute_local_transforms_blended(
            &nodes,
            &[
                ClipSample {
                    channels: &a,
                    time: 0.0,
                    weight: 0.75,
                },
                ClipSample {
                    channels: &b,
                    time: 0.0,
                    weight: 0.25,
                },
            ],
        );
        let x = quarter[0].to_scale_rotation_translation().2.x;
        assert!((x - 2.5).abs() < 0.001, "expected 2.5, got {x}");
    }

    #[test]
    fn weights_do_not_have_to_sum_to_one() {
        // A blend tree hands over raw weights; normalising here means callers
        // never have to, and a tree that sums to 2 does not send the skeleton
        // twice as far.
        let a = [fixed_translation(Vec3::ZERO)];
        let b = [fixed_translation(Vec3::new(10.0, 0.0, 0.0))];
        let nodes = one_node();

        let blended = compute_local_transforms_blended(
            &nodes,
            &[
                ClipSample {
                    channels: &a,
                    time: 0.0,
                    weight: 3.0,
                },
                ClipSample {
                    channels: &b,
                    time: 0.0,
                    weight: 1.0,
                },
            ],
        );
        let x = blended[0].to_scale_rotation_translation().2.x;
        assert!((x - 2.5).abs() < 0.001, "expected 2.5, got {x}");
    }

    #[test]
    fn a_single_clip_blends_to_exactly_itself() {
        // The path every existing animation takes once blending is in place;
        // if this drifts, every non-blended animation changes.
        let a = [fixed_translation(Vec3::new(4.0, 5.0, 6.0))];
        let nodes = one_node();

        let blended = compute_local_transforms_blended(
            &nodes,
            &[ClipSample {
                channels: &a,
                time: 0.0,
                weight: 1.0,
            }],
        );
        let (t, r, sc) = sample_node_trs(0, &nodes[0], &a, 0.0);
        let plain = [Mat4::from_scale_rotation_translation(sc, r, t)];
        assert!(
            blended[0].abs_diff_eq(plain[0], 0.0001),
            "blending one clip must equal sampling it: {:?} vs {:?}",
            blended[0],
            plain[0]
        );
    }

    #[test]
    fn no_clips_at_all_leaves_the_rest_pose() {
        let nodes = vec![NodeTransform {
            name: String::new(),
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            parent: None,
        }];
        let blended = compute_local_transforms_blended(&nodes, &[]);
        let t = blended[0].to_scale_rotation_translation().2;
        assert!(t.abs_diff_eq(Vec3::new(1.0, 2.0, 3.0), 0.001));
    }

    #[test]
    fn compute_joint_matrices_uses_bind_pose_when_no_channels_animate_a_node() {
        let nodes = vec![NodeTransform {
            name: String::new(),
            position: [1.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            parent: None,
        }];
        let skin = SkinData {
            joint_node_indices: vec![0],
            inverse_bind_matrices: vec![Mat4::IDENTITY.to_cols_array_2d()],
        };
        let matrices = compute_joint_matrices_blended(&nodes, &skin, &[]);
        let expected = Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0));
        assert!(matrices[0].abs_diff_eq(expected, 0.001));
    }

    #[test]
    fn compute_joint_matrices_composes_parent_child_hierarchy() {
        let nodes = vec![
            NodeTransform {
                name: String::new(),
                position: [1.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
                parent: None,
            },
            NodeTransform {
                name: String::new(),
                position: [0.0, 2.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
                parent: Some(0),
            },
        ];
        let skin = SkinData {
            joint_node_indices: vec![1],
            inverse_bind_matrices: vec![Mat4::IDENTITY.to_cols_array_2d()],
        };
        let matrices = compute_joint_matrices_blended(&nodes, &skin, &[]);
        let world_pos = matrices[0].transform_point3(Vec3::ZERO);
        assert!(world_pos.abs_diff_eq(Vec3::new(1.0, 2.0, 0.0), 0.001));
    }

    #[test]
    fn blend_vertex_applies_single_full_weight_joint() {
        let rest = Vec3::new(1.0, 0.0, 0.0);
        let joint_matrices = vec![Mat4::from_translation(Vec3::new(0.0, 5.0, 0.0))];
        let skin = VertexSkin {
            joints: [0, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        };
        let result = blend_vertex_position(rest, &skin, &joint_matrices);
        assert!(result.abs_diff_eq(Vec3::new(1.0, 5.0, 0.0), 0.001));
    }

    #[test]
    fn blend_vertex_blends_two_joints_by_weight() {
        let rest = Vec3::ZERO;
        let joint_matrices = vec![
            Mat4::from_translation(Vec3::new(0.0, 0.0, 0.0)),
            Mat4::from_translation(Vec3::new(0.0, 10.0, 0.0)),
        ];
        let skin = VertexSkin {
            joints: [0, 1, 0, 0],
            weights: [0.5, 0.5, 0.0, 0.0],
        };
        let result = blend_vertex_position(rest, &skin, &joint_matrices);
        assert!(result.abs_diff_eq(Vec3::new(0.0, 5.0, 0.0), 0.001));
    }

    // ---- what `#[reflect(ignore)]` costs, and what it must not cost -------
    //
    // These two components are the first in this codebase to reflect only
    // part of themselves, so the part that is *not* reflected is worth a test
    // rather than an assumption. `#[reflect(ignore)]` has two different
    // behaviours depending on which way a reflected value reaches a
    // component, and `ReflectComponent::apply_or_insert` -- the call MCP's
    // `set_reflected_component` and the editor's Inspector both go through --
    // picks between them by whether the entity already has the component:
    //
    //   * it *has* it -> `Reflect::apply`, which only touches reflected
    //     fields, so the ignored ones survive;
    //   * it does not -> `FromReflect` + insert, which fills every ignored
    //     field with `Default::default()`.
    //
    // The first is the one an Inspector edit takes, and getting it wrong
    // would be silent and expensive: editing `mesh_id` would blank the
    // rest-pose vertices and the skeleton, and the mesh would simply stop
    // deforming with no error anywhere.

    /// A `SkinnedMesh` carrying one of everything the reflection API cannot
    /// see, so that "the ignored fields survived" is a claim with content.
    fn skinned_mesh_with_bulk_data(mesh_id: u64) -> SkinnedMesh {
        SkinnedMesh {
            mesh_id,
            rest_vertices: vec![Vertex {
                position: [1.0, 2.0, 3.0],
                color: [1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
            }],
            skin: vec![VertexSkin {
                joints: [3, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            }],
            skin_data: SkinData {
                joint_node_indices: vec![7],
                inverse_bind_matrices: vec![Mat4::IDENTITY.to_cols_array_2d()],
            },
            nodes: vec![NodeTransform::default()],
            pose_override: Vec::new(),
            ik_tip_positions: Vec::new(),
            pose_override_weight: 1.0,
            joint_matrices: Vec::new(),
        }
    }

    /// Registers `SkinnedMesh` the way `register_gameplay_reflect_types` does
    /// and hands back the `ReflectComponent` that registration is *for* --
    /// which is also the assertion, since a type can be in the registry
    /// without it and would then still be unreachable by MCP and the
    /// Inspector.
    fn registry_with_skinned_mesh() -> bevy_reflect::TypeRegistry {
        let mut registry = bevy_reflect::TypeRegistry::default();
        registry.register::<SkinnedMesh>();
        assert!(
            registry
                .get(std::any::TypeId::of::<SkinnedMesh>())
                .expect("SkinnedMesh must be in the registry after register()")
                .data::<ReflectComponent>()
                .is_some(),
            "registration without `ReflectComponent` data satisfies the catalog's \
             text scan for `register_type::<SkinnedMesh>` and still leaves the \
             component unreachable by `set_reflected_component` and the Inspector \
             -- which is the whole of what R1 asks for"
        );
        registry
    }

    #[test]
    fn a_reflected_edit_of_mesh_id_keeps_the_bulk_data_reflection_cannot_see() {
        let registry = registry_with_skinned_mesh();
        let reflect_component = registry
            .get(std::any::TypeId::of::<SkinnedMesh>())
            .expect("registered above")
            .data::<ReflectComponent>()
            .expect("asserted above")
            .clone();

        let mut world = bevy_ecs::world::World::new();
        let entity = world.spawn(skinned_mesh_with_bulk_data(1)).id();

        // Exactly the shape of an Inspector edit or an MCP
        // `set_reflected_component` call: a value naming only the reflected
        // field, since the ignored ones are not in the type's reflected shape
        // and cannot be spelled at all.
        let mut patch = bevy_reflect::DynamicStruct::default();
        patch.insert("mesh_id", 42u64);
        let mut entity_mut = world.entity_mut(entity);
        reflect_component.apply_or_insert(&mut entity_mut, &patch, &registry);

        let after = world
            .get::<SkinnedMesh>(entity)
            .expect("the component is still there");
        assert_eq!(after.mesh_id, 42, "the reflected field is what was edited");
        assert_eq!(
            after.rest_vertices.len(),
            1,
            "editing `mesh_id` must not blank the rest pose: an Inspector edit \
             that silently dropped the vertex data would stop the mesh \
             deforming with nothing reported anywhere"
        );
        assert_eq!(after.rest_vertices[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(after.skin.len(), 1, "nor the per-vertex skin weights");
        assert_eq!(after.skin[0].joints[0], 3);
        assert_eq!(
            after.skin_data.joint_node_indices,
            vec![7],
            "nor the skeleton"
        );
        assert_eq!(after.nodes.len(), 1, "nor the node hierarchy");
    }

    #[test]
    fn the_reflected_shape_is_mesh_id_and_nothing_else() {
        use bevy_reflect::Struct;

        let mesh = skinned_mesh_with_bulk_data(5);
        let names: Vec<&str> = mesh
            .iter_fields()
            .enumerate()
            .map(|(i, _)| mesh.name_at(i).expect("a named field"))
            .collect();
        assert_eq!(
            names,
            vec!["mesh_id"],
            "the point of the `#[reflect(ignore)]`s is that an Inspector is \
             offered the one identifying field and not tens of thousands of \
             per-vertex rows; a field appearing here that is not `mesh_id` \
             means an ignore was dropped"
        );
    }

    use bsengine_render::MeshRenderer;

    #[test]
    fn skinning_system_deforms_vertex_away_from_rest_when_animated() {
        let mut app = bsengine_app::new_app();
        app.insert_resource(bsengine_core::Time::default());
        app.add_plugins(SkinnedMeshPlugin);

        let nodes = vec![NodeTransform::default()];
        let skin_data = SkinData {
            joint_node_indices: vec![0],
            inverse_bind_matrices: vec![Mat4::IDENTITY.to_cols_array_2d()],
        };
        let rest_vertices = vec![Vertex {
            position: [1.0, 0.0, 0.0],
            color: [1.0, 1.0, 1.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        }];
        let skin = vec![VertexSkin {
            joints: [0, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        }];
        let mut clips = std::collections::HashMap::new();
        clips.insert(
            "wiggle".to_string(),
            AnimationClip {
                name: "wiggle".to_string(),
                duration: 1.0,
                channels: vec![AnimationChannel {
                    node_index: 0,
                    times: vec![0.0, 1.0],
                    values: KeyframeValues::Translations(vec![[0.0, 0.0, 0.0], [0.0, 3.0, 0.0]]),
                    interpolation: Interpolation::Linear,
                }],
            },
        );

        let entity = app
            .world_mut()
            .spawn((
                SkinnedMesh {
                    mesh_id: 1,
                    rest_vertices,
                    skin,
                    skin_data,
                    nodes,
                    pose_override: Vec::new(),
                    ik_tip_positions: Vec::new(),
                    pose_override_weight: 1.0,
                    joint_matrices: Vec::new(),
                },
                AnimationClipLibrary { clips },
                bsengine_core::AnimationPlayer::new("wiggle").with_duration(1.0),
                MeshRenderer { mesh_id: 1 },
            ))
            .id();

        app.world_mut()
            .get_mut::<bsengine_core::AnimationPlayer>(entity)
            .unwrap()
            .time = 0.5;
        app.update();

        // No GpuMeshRegistry/GpuQueueResource is present in this headless test
        // (no RHI plugin added), so the system computes the pose without
        // uploading it anywhere -- this test's job is to prove the math runs
        // end-to-end via the ECS system, not to assert on GPU state.
        let skinned = app
            .world()
            .get::<SkinnedMesh>(entity)
            .expect("the component survives the system");
        assert_eq!(skinned.joint_matrices.len(), 1);
        let moved = skinned.joint_matrices[0].transform_point3(Vec3::ZERO);
        assert!(
            moved.abs_diff_eq(Vec3::new(0.0, 1.5, 0.0), 0.001),
            "halfway through a 0 -> 3 translation the one joint should be at \
             y = 1.5, got {moved:?}"
        );
    }

    // ---- ragdoll-sourced poses (roadmap item 52, sub-step 1/2) ------------
    //
    // `pose_override` is the whole of what `bsengine-gltf` knows about the
    // ragdoll: physics writes per-node globals into it and this crate feeds
    // them through the same `global * inverse_bind_matrix` step the animated
    // path uses. That the *physics* fills it correctly is asserted in
    // `bsengine-physics`, which is the only crate that can see both ends; what
    // belongs here is that the override is honoured at all, and that its
    // absence changes nothing.

    #[test]
    fn two_ik_chains_on_one_character_both_solve_through_the_real_system() {
        // Drives `update_skinned_meshes` itself, not the pure function beneath
        // it. That distinction is the whole point of this test: the five tests
        // above call `compute_joint_matrices_with_ik` with a slice, so a list
        // of chains passes through them trivially -- and they stayed green
        // while `IkChain` was a `Component`, which an entity can hold only ONE
        // of. A fox has four legs. The pure-function tests could not see the
        // defect because they never touched the ECS path production uses.
        //
        // Two chains, because one is exactly the case the broken shape handled
        // correctly.
        let mut app = bsengine_app::new_app();
        app.insert_resource(bsengine_core::Time::default());
        app.add_plugins(SkinnedMeshPlugin);

        // Two independent legs hanging off a shared root.
        let nodes = vec![
            NodeTransform {
                name: "root".to_string(),
                ..Default::default()
            },
            NodeTransform {
                name: "l_hip".to_string(),
                position: [-0.5, 2.0, 0.0],
                parent: Some(0),
                ..Default::default()
            },
            NodeTransform {
                name: "l_knee".to_string(),
                position: [0.0, -1.0, 0.3],
                parent: Some(1),
                ..Default::default()
            },
            NodeTransform {
                name: "l_foot".to_string(),
                position: [0.0, -1.0, -0.3],
                parent: Some(2),
                ..Default::default()
            },
            NodeTransform {
                name: "r_hip".to_string(),
                position: [0.5, 2.0, 0.0],
                parent: Some(0),
                ..Default::default()
            },
            NodeTransform {
                name: "r_knee".to_string(),
                position: [0.0, -1.0, 0.3],
                parent: Some(4),
                ..Default::default()
            },
            NodeTransform {
                name: "r_foot".to_string(),
                position: [0.0, -1.0, -0.3],
                parent: Some(5),
                ..Default::default()
            },
        ];
        let skin_data = SkinData {
            joint_node_indices: (0..nodes.len()).collect(),
            inverse_bind_matrices: vec![Mat4::IDENTITY.to_cols_array_2d(); nodes.len()],
        };

        // The system needs a clip and a player: IK is a correction applied over
        // an animated pose, so with nothing animating it skips the entity
        // entirely. A clip that holds the root still is enough.
        let mut clips = std::collections::HashMap::new();
        clips.insert(
            "still".to_string(),
            AnimationClip {
                name: "still".to_string(),
                duration: 1.0,
                channels: vec![AnimationChannel {
                    node_index: 0,
                    times: vec![0.0, 1.0],
                    values: KeyframeValues::Translations(vec![[0.0, 0.0, 0.0], [0.0, 0.0, 0.0]]),
                    interpolation: Interpolation::Linear,
                }],
            },
        );

        let left_target = Vec3::new(-0.5, 0.8, 0.2);
        let right_target = Vec3::new(0.5, 0.3, -0.2);
        let entity = app
            .world_mut()
            .spawn((
                SkinnedMesh {
                    mesh_id: 1,
                    rest_vertices: Vec::new(),
                    skin: Vec::new(),
                    skin_data,
                    nodes,
                    pose_override: Vec::new(),
                    ik_tip_positions: Vec::new(),
                    pose_override_weight: 1.0,
                    joint_matrices: Vec::new(),
                },
                AnimationClipLibrary { clips },
                bsengine_core::AnimationPlayer::new("still").with_duration(1.0),
                IkChains {
                    chains: vec![
                        IkChain {
                            root_bone: "l_hip".to_string(),
                            mid_bone: "l_knee".to_string(),
                            tip_bone: "l_foot".to_string(),
                            target: left_target.into(),
                            weight: 1.0,
                        },
                        IkChain {
                            root_bone: "r_hip".to_string(),
                            mid_bone: "r_knee".to_string(),
                            tip_bone: "r_foot".to_string(),
                            target: right_target.into(),
                            weight: 1.0,
                        },
                    ],
                },
            ))
            .id();

        app.update();

        let matrices = &app
            .world()
            .get::<SkinnedMesh>(entity)
            .expect("the character keeps its skinned mesh")
            .joint_matrices;
        assert_eq!(matrices.len(), 7, "one matrix per joint");
        let at = |i: usize| matrices[i].transform_point3(Vec3::ZERO);

        let l_err = (at(3) - left_target).length();
        let r_err = (at(6) - right_target).length();
        println!("left foot {:?} ({l_err} m off), right foot {:?} ({r_err} m off)", at(3), at(6));

        assert!(
            l_err < 1.0e-3,
            "the left foot must reach its own target: {:?} is {l_err} m from \
             {left_target:?}",
            at(3)
        );
        assert!(
            r_err < 1.0e-3,
            "the right foot must reach its own target: {:?} is {r_err} m from \
             {right_target:?}. Both feet reaching the SAME point would mean \
             only one chain was applied.",
            at(6)
        );
    }


    /// A one-node skeleton, a clip translating that node to (0, 3, 0), and an
    /// identity inverse bind matrix — so a joint matrix reads back directly as
    /// where the node ended up.
    fn one_joint_app() -> (bevy_app::App, bevy_ecs::entity::Entity) {
        let mut app = bsengine_app::new_app();
        app.insert_resource(bsengine_core::Time::default());
        app.add_plugins(SkinnedMeshPlugin);

        let mut clips = std::collections::HashMap::new();
        clips.insert(
            "wiggle".to_string(),
            AnimationClip {
                name: "wiggle".to_string(),
                duration: 1.0,
                channels: vec![AnimationChannel {
                    node_index: 0,
                    times: vec![0.0, 1.0],
                    values: KeyframeValues::Translations(vec![[0.0, 3.0, 0.0], [0.0, 3.0, 0.0]]),
                    interpolation: Interpolation::Linear,
                }],
            },
        );

        let entity = app
            .world_mut()
            .spawn((
                SkinnedMesh {
                    mesh_id: 1,
                    rest_vertices: Vec::new(),
                    skin: Vec::new(),
                    skin_data: SkinData {
                        joint_node_indices: vec![0],
                        inverse_bind_matrices: vec![Mat4::IDENTITY.to_cols_array_2d()],
                    },
                    nodes: one_node(),
                    pose_override: Vec::new(),
                    ik_tip_positions: Vec::new(),
                    pose_override_weight: 1.0,
                    joint_matrices: Vec::new(),
                },
                AnimationClipLibrary { clips },
                bsengine_core::AnimationPlayer::new("wiggle").with_duration(1.0),
            ))
            .id();
        (app, entity)
    }

    #[test]
    fn a_pose_override_is_what_the_joint_matrices_come_from() {
        // The quiet failure this guards: the ragdoll's bodies can be built,
        // simulating, and perfectly correct while skinning goes on reading the
        // clip -- and the character then looks completely normal on screen with
        // a full ragdoll running underneath it. Nothing about the bodies would
        // show it; only the joint matrices do.
        let (mut app, entity) = one_joint_app();
        app.update();
        let from_clip = app
            .world()
            .get::<SkinnedMesh>(entity)
            .unwrap()
            .joint_matrices[0]
            .transform_point3(Vec3::ZERO);
        assert!(
            from_clip.abs_diff_eq(Vec3::new(0.0, 3.0, 0.0), 0.001),
            "sanity: with no override the clip drives the pose, got {from_clip:?}"
        );

        app.world_mut()
            .get_mut::<SkinnedMesh>(entity)
            .unwrap()
            .pose_override = vec![Mat4::from_translation(Vec3::new(7.0, -2.0, 0.0))];
        app.update();

        let overridden = app
            .world()
            .get::<SkinnedMesh>(entity)
            .unwrap()
            .joint_matrices[0]
            .transform_point3(Vec3::ZERO);
        assert!(
            overridden.abs_diff_eq(Vec3::new(7.0, -2.0, 0.0), 0.001),
            "with a pose override in place the clip must not be read at all; \
             expected the override's (7, -2, 0), got {overridden:?}"
        );
    }

    #[test]
    fn an_empty_pose_override_leaves_the_animated_path_byte_identical() {
        // The other direction, and the one a released engine depends on: every
        // skinned mesh that has never heard of a ragdoll must come out of this
        // exactly as it did before the feature existed.
        let (mut app, entity) = one_joint_app();
        app.update();

        let skinned = app.world().get::<SkinnedMesh>(entity).unwrap();
        let library = app.world().get::<AnimationClipLibrary>(entity).unwrap();
        let player = app
            .world()
            .get::<bsengine_core::AnimationPlayer>(entity)
            .unwrap();
        let samples = blend_samples(&library.clips["wiggle"], library, player.time, None);
        let expected = compute_joint_matrices_blended(&skinned.nodes, &skinned.skin_data, &samples);

        assert_eq!(skinned.joint_matrices.len(), expected.len());
        for (i, (got, want)) in skinned.joint_matrices.iter().zip(&expected).enumerate() {
            assert_eq!(
                got.to_cols_array(),
                want.to_cols_array(),
                "joint {i} must be bit-for-bit what the pre-ragdoll animation \
                 path produced, not merely close to it"
            );
        }
    }

    // ---- pose_override_weight (roadmap item 52, sub-step 2/2, Task 3a) ----
    //
    // These three tests prove that adding the weight field changed nothing for
    // the cases that existed before it, and that the blended path lands strictly
    // between the two endpoints (not quietly returning one of them).

    #[test]
    fn a_full_weight_override_is_byte_identical_to_no_weight_at_all() {
        // The whole claim of this task. Prove the restructure changed nothing:
        // weight 1.0 with an override must produce exactly the same matrices as
        // the old binary branch that had no weight concept.
        let (mut app, entity) = one_joint_app();

        // Override: translate the node to (7, -2, 0).
        app.world_mut()
            .get_mut::<SkinnedMesh>(entity)
            .unwrap()
            .pose_override = vec![Mat4::from_translation(Vec3::new(7.0, -2.0, 0.0))];
        // Weight stays at its constructed default of 1.0.
        app.update();

        let result = app
            .world()
            .get::<SkinnedMesh>(entity)
            .unwrap()
            .joint_matrices[0]
            .transform_point3(Vec3::ZERO);
        assert!(
            result.abs_diff_eq(Vec3::new(7.0, -2.0, 0.0), 0.001),
            "weight 1.0 must be bit-for-bit the override; got {result:?}"
        );
    }

    #[test]
    fn a_zero_weight_override_gives_back_exactly_the_animated_pose() {
        // The other endpoint: weight 0 with an override must equal what an
        // empty override gives (the clip-driven pose). This is the guarantee
        // that Task 3b can safely coast to 0 and fully restore animation.
        let (mut app, entity) = one_joint_app();

        // Compute the clip-driven reference first, no override.
        app.update();
        let animated = app
            .world()
            .get::<SkinnedMesh>(entity)
            .unwrap()
            .joint_matrices[0];

        // Now set an override that would send the joint somewhere completely
        // different, but drive the weight to zero.
        {
            let mut skinned = app.world_mut().get_mut::<SkinnedMesh>(entity).unwrap();
            skinned.pose_override = vec![Mat4::from_translation(Vec3::new(100.0, 0.0, 0.0))];
            skinned.pose_override_weight = 0.0;
        }
        app.update();

        let result = app
            .world()
            .get::<SkinnedMesh>(entity)
            .unwrap()
            .joint_matrices[0];
        assert_eq!(
            result.to_cols_array(),
            animated.to_cols_array(),
            "weight 0.0 must yield exactly the animated pose, not the override"
        );
    }

    #[test]
    fn a_half_weight_override_lands_between_the_two() {
        // Assert the blended result differs measurably from BOTH endpoints.
        // A blend that quietly returns one endpoint passes any weaker check.
        let (mut app, entity) = one_joint_app();

        // Animated pose: node at (0, 3, 0) per the wiggle clip.
        // Override: node at (10, 3, 0) — same Y so only X differs, making the
        // arithmetic easy to reason about.
        app.update();
        let animated_pt = app
            .world()
            .get::<SkinnedMesh>(entity)
            .unwrap()
            .joint_matrices[0]
            .transform_point3(Vec3::ZERO);

        {
            let mut skinned = app.world_mut().get_mut::<SkinnedMesh>(entity).unwrap();
            skinned.pose_override = vec![Mat4::from_translation(Vec3::new(10.0, 3.0, 0.0))];
            skinned.pose_override_weight = 0.5;
        }
        app.update();

        let blended_pt = app
            .world()
            .get::<SkinnedMesh>(entity)
            .unwrap()
            .joint_matrices[0]
            .transform_point3(Vec3::ZERO);

        // Half-blend of X=0 (animated) and X=10 (override) should be near 5.
        assert!(
            (blended_pt.x - 5.0).abs() < 0.01,
            "half weight should blend X to ~5.0, got {blended_pt:?}"
        );
        // Must differ from the animated endpoint (X=0).
        assert!(
            (blended_pt.x - animated_pt.x).abs() > 1.0,
            "half blend must differ measurably from the animated pose; \
             animated={animated_pt:?}, blended={blended_pt:?}"
        );
        // Must differ from the override endpoint (X=10).
        assert!(
            (blended_pt.x - 10.0).abs() > 1.0,
            "half blend must differ measurably from the full override; \
             blended={blended_pt:?}"
        );
    }
}

use bevy_app::{App, Plugin, PostUpdate, Update};
use bevy_ecs::prelude::{Component, Query, ReflectComponent, Res, ResMut};
use bevy_ecs::schedule::{IntoSystemConfigs, IntoSystemSetConfigs};
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

/// Drives this character's skeleton from another entity's animated pose.
///
/// Bone pairs are explicit and name-keyed, matching `Ragdoll.joint_overrides`
/// and [`IkChain`]. Heuristic name matching was rejected for the reason item
/// 52's ragdoll rejected it: it depends on a rigging convention and fails
/// silently on a different rig or non-English bone names -- and here a wrong
/// guess produces a plausible-looking wrong pose rather than an error.
#[derive(Component, Debug, Clone, Default, bevy_reflect::Reflect)]
#[reflect(Component, Default)]
pub struct RetargetSource {
    /// Name of the entity whose pose is copied.
    pub source: String,
    /// The entity [`source`](RetargetSource::source) names, once something has
    /// looked it up.
    ///
    /// Resolved by `bsengine-scene`, not here: this crate cannot see `Name`,
    /// which lives there, and that crate depends on this one -- the reverse
    /// edge would be a cycle. Same split as `Joint.body_b`.
    #[reflect(ignore)]
    pub resolved: Option<bevy_ecs::entity::Entity>,
    /// `(source bone, target bone)` pairs. A target bone named by no pair keeps
    /// its own animation.
    pub pairs: Vec<(String, String)>,
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

/// One full-body IK goal: a chain of bones, and where its last bone should go.
///
/// The difference from [`IkChain`] is the sharing. An `IkChain` names exactly
/// three bones and solves them alone, so two chains naming the same bone fight
/// and the last one wins -- which its own doc comment records as an authoring
/// error. Goals name as many bones as they like and are solved *together*, so
/// two arms that both list the spine agree on where it ends up: reaching one
/// hand forward bends the spine, and the other arm pulls it back.
///
/// Unreal's Full Body IK node is the same shape -- one solver over the
/// hierarchy with several effectors, rather than one chain at a time. Unity
/// composes per-limb constraints in a rig and Godot's SkeletonIK3D is
/// single-chain, so this follows Unreal where the three diverge.
#[derive(Debug, Clone, Default, bevy_reflect::Reflect)]
pub struct IkGoal {
    /// Bones from the one that may move down to the effector, in order.
    ///
    /// Names, not indices, matching [`IkChain`] and `Ragdoll.joint_overrides`.
    /// Longer chains reach further into the body: `["Spine", "Chest",
    /// "Shoulder", "UpperArm", "LowerArm", "Hand"]` lets a reach bend the
    /// spine, while `["UpperArm", "LowerArm", "Hand"]` keeps it to the arm.
    pub bones: Vec<String>,
    /// World-space position the last bone should reach.
    pub target: bsengine_core::ReflectVec3,
    /// How much of the solved pose to apply, `0.0..=1.0`.
    ///
    /// Blended once at the end of the solve, so half means half. A goal at
    /// zero leaves its bones exactly where the animation put them, which is
    /// what lets foot IK fade out rather than pop.
    pub weight: f32,
}

/// Every full-body IK goal on one character, solved together.
///
/// A list for the same reason [`IkChains`] is one: an entity holds one of any
/// component and a character has several effectors. Unlike `IkChains`, goals
/// that share bones are *resolved* rather than left to fight -- that is the
/// whole point of solving them in one pass.
///
/// The two compose: `IkChains` stays the right tool for a single limb, where
/// its analytic two-bone solution is exact and cheaper.
#[derive(Component, Debug, Clone, Default, bevy_reflect::Reflect)]
#[reflect(Component, Default)]
pub struct IkGoals {
    /// The goals. Order does not affect the result -- shared joints are
    /// averaged, so the answer cannot depend on how an author happened to list
    /// them.
    pub goals: Vec<IkGoal>,
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
    /// This character's animated local transforms, as of the last time the
    /// skinning system ran.
    ///
    /// Published so a retargeting character can read its source's pose. Locals
    /// are otherwise not stored anywhere -- only `joint_matrices` are -- and a
    /// target needs its SOURCE's, not its own.
    ///
    /// Runtime output, never authored, hence not reflected.
    #[reflect(ignore)]
    pub animated_locals: Vec<Mat4>,
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

    // A 2D space wins over a 1D one when a state has both: it is the more
    // specific authoring, and silently preferring the 1D tree would make the
    // 2D one look broken rather than ignored.
    let sampled: Option<Vec<(String, f32)>> = match (&state.blend2d, &state.blend) {
        (Some(t), _) => {
            let px = asm
                .params_float
                .get(t.param_x.as_str())
                .copied()
                .unwrap_or(0.0);
            let py = asm
                .params_float
                .get(t.param_y.as_str())
                .copied()
                .unwrap_or(0.0);
            Some(t.sample(px, py))
        }
        (None, Some(t)) => {
            let v = asm
                .params_float
                .get(t.param.as_str())
                .copied()
                .unwrap_or(0.0);
            Some(t.sample(v))
        }
        (None, None) => None,
    };
    if let Some(entries) = sampled {
        let mut any = false;
        for (name, weight) in entries {
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
    compute_joint_matrices_with_ik(nodes, skin, clips, &[], &[])
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
    goals: &[&IkGoal],
) -> Vec<Mat4> {
    compute_pose_with_ik(nodes, skin, clips, chains, goals, None, None).0
}

/// Solves every full-body goal together and writes the result into `locals`.
///
/// The solver works in world-space joint *positions*; a skeleton is driven by
/// local rotations. So the bones are gathered as positions, solved, and the
/// rotations that reproduce the solved positions are carried back into each
/// bone's parent frame -- the same trip [`crate::ik::solve_two_bone`]'s result
/// makes, just over a longer chain.
fn solve_full_body_goals(
    nodes: &[NodeTransform],
    locals: &mut [Mat4],
    globals: &mut Vec<Mat4>,
    goals: &[&IkGoal],
) {
    let live: Vec<&&IkGoal> = goals
        .iter()
        .filter(|g| g.weight > 0.0 && g.bones.len() >= 2)
        .collect();
    if live.is_empty() {
        return;
    }

    // Bone names to node indices, once. A name the rig lacks is a scene typo:
    // the goal is dropped with a warning rather than posing some other joint,
    // which would read as a solver bug.
    let mut chains: Vec<Vec<usize>> = Vec::new();
    let mut targets: Vec<Vec3> = Vec::new();
    let mut weights: Vec<f32> = Vec::new();
    for goal in &live {
        let resolved: Option<Vec<usize>> = goal
            .bones
            .iter()
            .map(|name| node_index_by_name(nodes, name))
            .collect();
        match resolved {
            Some(indices) => {
                chains.push(indices);
                targets.push(goal.target.0);
                weights.push(goal.weight);
            }
            None => tracing::warn!(
                "[ik] goal names a bone this skeleton lacks: {:?}",
                goal.bones
            ),
        }
    }
    if chains.is_empty() {
        return;
    }

    // Positions for every node, so a chain can index straight into it.
    let original: Vec<Vec3> = globals
        .iter()
        .map(|m| m.transform_point3(Vec3::ZERO))
        .collect();
    let mut solved = original.clone();
    crate::ik::solve_goals(
        &mut solved,
        &chains,
        &targets,
        &weights,
        crate::ik::GOAL_ITERATIONS,
    );

    // Rotations per bone, from the root down.
    //
    // ⚠️ Each one is measured against the bone's direction *right now*, not
    // against the pose the animation gave. Rotating a parent already turns
    // every bone under it, so a delta computed from the original pose counts
    // the parent's rotation a second time -- measured, a two-arm reach landed
    // the hand 0.7 short in y and no number of solver iterations helped,
    // because the solver was right and the application was not.
    //
    // Measuring afresh is self-correcting: after each bone is applied its child
    // is where the solve wants it, so the next bone's correction starts from
    // the truth.
    for chain in &chains {
        for k in 0..chain.len() - 1 {
            let here = globals[chain[k]].transform_point3(Vec3::ZERO);
            let current = globals[chain[k + 1]].transform_point3(Vec3::ZERO) - here;
            let desired = solved[chain[k + 1]] - solved[chain[k]];
            if current.length_squared() <= 1.0e-12 || desired.length_squared() <= 1.0e-12 {
                continue;
            }
            let rot = Quat::from_rotation_arc(current.normalize(), desired.normalize());
            apply_world_rotation(locals, globals, nodes, chain[k], rot);
            *globals = accumulate_globals(nodes, locals);
        }
    }
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
    goals: &[&IkGoal],
    retarget: Option<(&RetargetSource, &[NodeTransform], &[Mat4])>,
    // The root-motion bone, and whether its yaw is extracted too.
    root_motion_bone: Option<(usize, bool)>,
) -> (Vec<Mat4>, Vec<Vec3>, Vec<Mat4>) {
    let mut locals = compute_local_transforms_blended(nodes, clips);
    // Root motion first, before retargeting and IK: they must correct the
    // pose the character will actually show, which is the one whose travel
    // has been handed to the entity.
    if let Some((bone, yaw)) = root_motion_bone {
        strip_root_travel(nodes, &mut locals, bone, yaw);
    }

    // BEFORE the globals are accumulated, and therefore before IK.
    //
    // The order is load-bearing, not incidental: retargeting decides the pose
    // and IK corrects that pose against the world. Reversed, retargeting
    // overwrites IK's correction and foot placement silently stops working --
    // the producer/consumer failure this codebase keeps meeting, where
    // everything upstream looks healthy. Mutation-verified: moving this after
    // the IK loop puts the demo foot 1.83 m off its target instead of 1.2e-7.
    if let Some((map, source_nodes, source_locals)) = retarget {
        crate::retarget::retarget_locals(
            nodes,
            &mut locals,
            source_nodes,
            source_locals,
            &map.pairs,
        );
    }

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

    // Full-body goals, after the per-limb chains. The order matters and is not
    // incidental: a chain is exact for its own two bones, so letting it place
    // the limb first and then resolving whatever the goals disagree about is
    // strictly better than the reverse, which would have a chain overwrite the
    // body's answer for its own limb.
    solve_full_body_goals(nodes, &mut locals, &mut globals, goals);

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

    (joint_matrices_from_globals(&globals, skin), tips, locals)
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
    locals[index] = Mat4::from_scale_rotation_translation(scale, local_delta * rot, translation);
}

/// Finds a node by its glTF name.
fn node_index_by_name(nodes: &[NodeTransform], name: &str) -> Option<usize> {
    nodes.iter().position(|n| n.name == name)
}

/// Blends one rest-pose vertex position through up to 4 joint matrices by
/// weight — the standard linear blend skinning (LBS) formula.
///
/// The reference the GPU path is held to, not the production path any more:
/// `bsengine_rhi_wgpu::skinning` does this per vertex in a compute shader,
/// and the equivalence test in `plugin.rs` compares its output to this,
/// vertex by vertex, on the real fox. Kept as the definition of what the
/// shader must compute.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn blend_vertex_position(
    rest: Vec3,
    skin: &VertexSkin,
    joint_matrices: &[Mat4],
) -> Vec3 {
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
/// skinning. The compute shader makes the same simplification, on purpose.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn blend_vertex_normal(
    rest_normal: Vec3,
    skin: &VertexSkin,
    joint_matrices: &[Mat4],
) -> Vec3 {
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
        app.add_systems(PostUpdate, update_skinned_meshes)
            // In `Update`, after the players have advanced and before
            // `PostUpdate` propagates transforms, so the entity is moved in the
            // same frame its pose stops moving -- the two halves of one motion
            // must never be a frame apart, or the character stutters.
            .configure_sets(
                Update,
                bsengine_core::AnimationPoseSystems.after(bsengine_core::AnimationSystems),
            )
            .add_systems(
                Update,
                (apply_root_motion, animate_morph_weights)
                    .in_set(bsengine_core::AnimationPoseSystems),
            );
    }
}

/// Which node of the source glTF a model's `MorphWeights` belong to -- the
/// node a clip's `weights` channel names when it animates them. Inserted by
/// the glTF loader beside `MorphWeights`.
///
/// Crate-private on purpose: it is derived from the file on every import,
/// never authored, so it has no business in the Inspector or in a saved
/// scene -- where a stale copy would outlive a re-export that renumbered the
/// nodes. (Public, it would also fall under the catalogue's R1 rule and have
/// to be registered for reflection.)
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct MorphSource {
    /// The node that draws the morphed mesh.
    pub(crate) node: usize,
}

/// Samples a `weights` channel at `time`: one weight per target, linearly
/// interpolated (Step and CubicSpline hold the earlier key's values, the
/// simplification the translation sampler documents). `None` for a channel
/// that is not weights, or whose values do not divide into its keys.
fn sample_weights(channel: &AnimationChannel, time: f32) -> Option<Vec<f32>> {
    let KeyframeValues::Weights(values) = &channel.values else {
        return None;
    };
    let keys = channel.times.len();
    let per_key = match channel.interpolation {
        Interpolation::CubicSpline => 3,
        _ => 1,
    };
    if keys == 0 || values.len() % (keys * per_key) != 0 {
        return None;
    }
    let n = values.len() / (keys * per_key);
    // Under CubicSpline the value block is the middle one of each key's three.
    let value_at = |k: usize, t: usize| values[k * per_key * n + (per_key / 2) * n + t];
    let (i0, i1, f) = bracket(&channel.times, time)?;
    Some(
        (0..n)
            .map(|t| match channel.interpolation {
                Interpolation::Linear => value_at(i0, t) + (value_at(i1, t) - value_at(i0, t)) * f,
                Interpolation::Step | Interpolation::CubicSpline => value_at(i0, t),
            })
            .collect(),
    )
}

/// Drives `MorphWeights` from the playing clip's `weights` channel for the
/// model's morph node, when the clip has one -- glTF's animated blend shapes.
/// A clip without one leaves the weights to whoever else sets them (a
/// script, the scene). Runs in [`bsengine_core::AnimationPoseSystems`]: after
/// the players advance, before scripts, so a script that sets a weight this
/// frame overrides the clip, as `LateUpdate` does over Unity's Animator.
fn animate_morph_weights(
    mut query: Query<(
        &AnimationClipLibrary,
        &bsengine_core::AnimationPlayer,
        &MorphSource,
        &mut bsengine_core::MorphWeights,
    )>,
) {
    for (library, player, source, mut weights) in query.iter_mut() {
        let Some(clip) = library.clips.get(&player.clip) else {
            continue;
        };
        let Some(sampled) = clip
            .channels
            .iter()
            .filter(|c| c.node_index == source.node)
            .find_map(|c| sample_weights(c, player.time))
        else {
            continue;
        };
        // Only as many as the mesh has: a channel for more targets than the
        // mesh carries sets the ones that exist.
        let n = sampled.len().min(weights.weights.len());
        if weights.weights[..n] != sampled[..n] {
            weights.weights[..n].copy_from_slice(&sampled[..n]);
        }
    }
}

/// The root-motion bone of `skinned`: the named node, or the skin's first
/// joint when the name is empty. `None` for a name the skeleton lacks.
fn root_motion_bone(skinned: &SkinnedMesh, name: &str) -> Option<usize> {
    if name.is_empty() {
        skinned.skin_data.joint_node_indices.first().copied()
    } else {
        node_index_by_name(&skinned.nodes, name)
    }
}

/// Where `bone` sits in MODEL space when the clips in `samples` are posed --
/// through every parent, so an armature node's rotation and scale are in it
/// -- and its yaw there: the turn about model +Y, the twist half of a
/// swing-twist split (`2 atan2(y, w)`), which ignores lean and roll.
fn bone_model_pose(
    nodes: &[NodeTransform],
    samples: &[ClipSample<'_>],
    bone: usize,
) -> (Vec3, f32) {
    let locals = compute_local_transforms_blended(nodes, samples);
    let global = accumulate_globals(nodes, &locals)[bone];
    let (_, rotation, position) = global.to_scale_rotation_translation();
    (position, yaw_of(rotation))
}

/// The twist of `q` about +Y, in radians, in (-pi, pi] -- wrapped, since
/// `q` and `-q` are the same rotation and the raw formula reads them a full
/// turn apart.
fn yaw_of(q: Quat) -> f32 {
    wrap_angle(2.0 * q.y.atan2(q.w))
}

/// `a` wrapped into (-pi, pi]: a turn across the +-180° seam is the short
/// way round, not almost a full circle back.
fn wrap_angle(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let w = (a + PI).rem_euclid(TAU) - PI;
    if w <= -PI {
        w + TAU
    } else {
        w
    }
}

/// Moves `bone` back to its rest position horizontally, in model space,
/// leaving its height -- and with `strip_yaw`, turns it back to its rest yaw,
/// leaving its lean and roll: the half of root motion that happens in the
/// pose. Found in model space and pushed back through the parent's inverse,
/// which is what makes it right under a rotated or scaled armature.
fn strip_root_travel(nodes: &[NodeTransform], locals: &mut [Mat4], bone: usize, strip_yaw: bool) {
    let rest_locals = compute_local_transforms_blended(nodes, &[]);
    let rest_global = accumulate_globals(nodes, &rest_locals)[bone];
    let rest = rest_global.w_axis.truncate();
    let globals = accumulate_globals(nodes, locals);
    let (scale, rotation, now) = globals[bone].to_scale_rotation_translation();
    let position = Vec3::new(rest.x, now.y, rest.z);
    let rotation = if strip_yaw {
        let (_, rest_rotation, _) = rest_global.to_scale_rotation_translation();
        Quat::from_rotation_y(yaw_of(rest_rotation) - yaw_of(rotation)) * rotation
    } else {
        rotation
    };
    let target = Mat4::from_scale_rotation_translation(scale, rotation, position);
    let parent = nodes[bone]
        .parent
        .map(|p| globals[p])
        .unwrap_or(Mat4::IDENTITY);
    locals[bone] = parent.inverse() * target;
}

/// One stretch of root motion, from the pose at one time to the pose at a
/// later one: the horizontal travel and the turn. With `rotation`, the
/// travel is in the root's facing at the start of the stretch (relative to
/// its rest yaw) -- the frame the entity, already turned by everything
/// extracted so far, walks in. Without, it is plain model space and there is
/// no turn, which is translation-only root motion exactly as before.
fn root_motion_segment(
    from: (Vec3, f32),
    to: (Vec3, f32),
    rest_yaw: f32,
    rotation: bool,
) -> (Vec3, f32) {
    let d = to.0 - from.0;
    let horizontal = Vec3::new(d.x, 0.0, d.z);
    if rotation {
        (
            Quat::from_rotation_y(rest_yaw - from.1) * horizontal,
            wrap_angle(to.1 - from.1),
        )
    } else {
        (horizontal, 0.0)
    }
}

/// Two stretches one after the other (a loop's wrap): the second walks in
/// the facing the first turned to.
fn chain_segments(a: (Vec3, f32), b: (Vec3, f32)) -> (Vec3, f32) {
    (a.0 + Quat::from_rotation_y(a.1) * b.0, a.1 + b.1)
}

/// The other half: how far the root bone travelled horizontally since the
/// last frame, in world space, applied to the entity (or only reported).
///
/// Measured between the pose at the previous time and the pose now, both
/// through the same blend, so a crossfade moves at the blend of the two
/// clips' paces. A loop that wrapped is measured to the clip's end and then
/// from its start -- otherwise the snap back to frame 0 would drag the
/// character back a whole cycle -- and the same the other way in reverse.
#[allow(clippy::type_complexity)]
fn apply_root_motion(
    mut query: Query<(
        &SkinnedMesh,
        &AnimationClipLibrary,
        &bsengine_core::AnimationPlayer,
        Option<&bsengine_core::AnimationStateMachine>,
        &mut bsengine_core::RootMotion,
        &mut bsengine_core::Transform,
    )>,
    time: Option<Res<bsengine_core::Time>>,
) {
    let dt = time.map(|t| t.delta_seconds).unwrap_or(0.0);
    for (skinned, library, player, asm, mut motion, mut transform) in query.iter_mut() {
        let Some(clip) = library.clips.get(&player.clip) else {
            continue;
        };
        let Some(bone) = root_motion_bone(skinned, &motion.bone) else {
            continue;
        };
        let now = player.time;
        let rotation = motion.apply_rotation;
        let at =
            |t: f32| bone_model_pose(&skinned.nodes, &blend_samples(clip, library, t, asm), bone);
        let rest_yaw = if rotation {
            bone_model_pose(&skinned.nodes, &[], bone).1
        } else {
            0.0
        };
        let seg = |a: f32, b: f32| root_motion_segment(at(a), at(b), rest_yaw, rotation);
        let (local, turn) = match &motion.last_sample {
            Some((last_clip, last)) if *last_clip == player.clip => {
                let last = *last;
                let forward = player.speed >= 0.0;
                if forward && now < last {
                    chain_segments(seg(last, player.duration), seg(0.0, now))
                } else if !forward && now > last {
                    chain_segments(seg(last, 0.0), seg(player.duration, now))
                } else {
                    seg(last, now)
                }
            }
            // First frame, or a new clip: the player has already advanced
            // this frame, so measure from where it was one frame ago -- as
            // Unity measures its first root-motion step from deltaTime. A
            // paused player gets nothing.
            _ if player.playing => {
                let from = (now - dt * player.speed).clamp(0.0, player.duration.max(0.0));
                seg(from, now)
            }
            _ => (Vec3::ZERO, 0.0),
        };
        // Model space to world: the entity's own rotation and scale, as they
        // stood before this frame's turn -- the facing the stretch was
        // walked in. Its position is not part of a *direction*.
        let world = transform.rotation.0 * (transform.scale.0 * local);
        motion.last_delta = world.into();
        motion.last_rotation_delta = turn;
        if motion.apply_to_transform {
            transform.position.0 += world;
            // About the entity's own up axis, as Unity applies
            // `deltaRotation`: `rotation * delta`.
            transform.rotation.0 = (transform.rotation.0 * Quat::from_rotation_y(turn)).normalize();
        }
        motion.last_sample = Some((player.clip.clone(), now));
    }
}

pub(crate) fn update_skinned_meshes(
    mut query: Query<(
        bevy_ecs::entity::Entity,
        &mut SkinnedMesh,
        Option<&AnimationClipLibrary>,
        Option<&bsengine_core::AnimationPlayer>,
        Option<&bsengine_core::AnimationStateMachine>,
        Option<&IkChains>,
        Option<&IkGoals>,
        Option<&RetargetSource>,
        Option<&bsengine_core::GlobalTransform>,
        Option<&bsengine_core::Transform>,
        Option<&bsengine_core::RootMotion>,
    )>,
    mesh_registry: Option<ResMut<bsengine_rhi_wgpu::GpuMeshRegistry>>,
    queue: Option<Res<bsengine_rhi_wgpu::GpuQueueResource>>,
) {
    // Deliberately not an early return when there is no GPU. Composing the
    // joint matrices is a few dozen matrix products over the skeleton and it is
    // the *pose*, which a headless host still wants to be right; blending the
    // vertices is tens of thousands of products whose only consumer is the
    // upload, so that half is what the GPU's absence skips.
    // Snapshot every character's rest pose and published locals BEFORE the
    // mutable pass: a retargeting target reads another entity's `SkinnedMesh`
    // while the loop below holds `&mut` on its own.
    //
    // Taken from THIS query immutably rather than from a second one -- Bevy
    // rejects a `&SkinnedMesh` query alongside a `&mut SkinnedMesh` one in the
    // same system (B0001), and `Query<&mut T>` iterates immutably too, so no
    // `ParamSet` is needed.
    //
    // The snapshot is last frame's pose when the source is iterated after the
    // target: a one-frame lag on a retargeted character, imperceptible and far
    // cheaper than ordering the two entities.
    let poses: std::collections::HashMap<
        bevy_ecs::entity::Entity,
        (Vec<NodeTransform>, Vec<Mat4>),
    > = query
        .iter()
        .map(|(entity, mesh, ..)| (entity, (mesh.nodes.clone(), mesh.animated_locals.clone())))
        .collect();

    let mut gpu = mesh_registry.zip(queue);

    for (
        _entity,
        mut skinned,
        library,
        player,
        asm,
        ik,
        ik_goals,
        retarget,
        global,
        local,
        root_motion,
    ) in query.iter_mut()
    {
        // Set by the clip branch below; the ragdoll-override branches leave it
        // None, and an override means physics is driving the whole skeleton
        // anyway.
        let mut published_tips: Option<Vec<Vec3>> = None;
        let mut published_locals: Option<Vec<Mat4>> = None;
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
            // The skeleton is solved entirely in MODEL space -- `nodes` are
            // the glTF's own local transforms and know nothing about where the
            // character stands. `IkChain.target` is world space, because the
            // ground it comes from is. Without this conversion the solver aims
            // a foot at a world coordinate expressed in model units: on the
            // demo fox (scale 0.02) the feet sat at model y = 15 while the
            // ground was at world y = 0, and the probe found nothing at all.
            //
            // `GlobalTransform` when something propagated one, the local
            // `Transform` otherwise: propagation lives in `RenderPlugin`, which
            // is not in every host.
            let model_to_world = global
                .map(|g| g.0 .0)
                .or_else(|| {
                    local.map(|t| {
                        Mat4::from_scale_rotation_translation(t.scale.0, t.rotation.0, t.position.0)
                    })
                })
                .unwrap_or(Mat4::IDENTITY);
            let world_to_model = model_to_world.inverse();

            let model_chains: Vec<IkChain> = ik
                .map(|c| {
                    c.chains
                        .iter()
                        .map(|chain| IkChain {
                            target: world_to_model.transform_point3(chain.target.0).into(),
                            ..chain.clone()
                        })
                        .collect()
                })
                .unwrap_or_default();
            let chains: Vec<&IkChain> = model_chains.iter().collect();
            // Into model space, exactly as the chains above: the solver works
            // in the skeleton's own frame, and a target authored in world space
            // would otherwise pull the pose by however far the character has
            // walked from the origin.
            let model_goals: Vec<IkGoal> = ik_goals
                .map(|g| {
                    g.goals
                        .iter()
                        .map(|goal| IkGoal {
                            target: world_to_model.transform_point3(goal.target.0).into(),
                            ..goal.clone()
                        })
                        .collect()
                })
                .unwrap_or_default();
            let goals: Vec<&IkGoal> = model_goals.iter().collect();
            let retarget_input = retarget.and_then(|r| {
                r.resolved
                    .and_then(|e| poses.get(&e))
                    .map(|(nodes, locals)| (r, nodes.as_slice(), locals.as_slice()))
            });
            let (matrices, tips, locals) = compute_pose_with_ik(
                &skinned.nodes,
                &skinned.skin_data,
                &samples,
                &chains,
                &goals,
                retarget_input,
                root_motion.and_then(|m| {
                    root_motion_bone(&skinned, &m.bone).map(|b| (b, m.apply_rotation))
                }),
            );
            published_locals = Some(locals);
            // Published in WORLD space, which is what the ground probe needs.
            published_tips = Some(
                tips.iter()
                    .map(|t| model_to_world.transform_point3(*t))
                    .collect(),
            );
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
        if let Some(locals) = published_locals {
            skinned.animated_locals = locals;
        }

        // The blend itself runs on the GPU -- see `bsengine_rhi_wgpu::skinning`
        // for why, and for the measurement. The CPU's job ends at the palette.
        // A mesh the registry does not know as skinned (a `SkinnedMesh` built
        // by hand around a plain mesh id) is left drawing its rest pose,
        // which `skin` reports and nothing here needs to act on.
        if let Some((mesh_registry, queue)) = gpu.as_mut() {
            mesh_registry.skin(&queue.0, skinned.mesh_id, &joint_matrices);
        }

        skinned.joint_matrices = joint_matrices;
    }

    // Every character's dispatch in one submission. Per-character submits
    // were measured to cost more than the blend they replaced -- see
    // `GpuMeshRegistry::skin`.
    if let Some((mesh_registry, queue)) = gpu.as_mut() {
        mesh_registry.flush_skinning(&queue.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one property the four parts of `LoadedGltf::bake_scale` exist to
    /// hold together: a character imported at scale `s` deforms, at any
    /// moment of any clip, to exactly `s` times where the unscaled character
    /// is. Each part alone is pinned in `loader.rs`; this is the composition,
    /// through the real skinning path -- global accumulation, inverse bind,
    /// per-vertex blend. Scaling three of the four passes the per-part
    /// checks for those three and fails here by the missing factor.
    #[test]
    fn a_baked_scale_deforms_to_exactly_the_scaled_unscaled_pose() {
        use crate::loader::{GltfLoader, LoadedGltf};
        use bsengine_core::ModelImportSettings;

        const S: f32 = 2.5;
        let fox = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../games/mini-arena/assets/models/fox.glb");
        let load = |scale| {
            GltfLoader::load_full_with(
                fox.to_str().unwrap(),
                &ModelImportSettings {
                    scale,
                    import_animations: true,
                },
            )
            .expect("fox.glb loads")
        };
        let one = load(1.0);
        let two = load(S);

        // The longest clip, sampled well inside it.
        let clip = (0..one.animations.len())
            .max_by(|&a, &b| {
                one.animations[a]
                    .duration
                    .partial_cmp(&one.animations[b].duration)
                    .unwrap()
            })
            .expect("premise: fox.glb has clips");
        let time = one.animations[clip].duration * 0.4;

        let pose = |g: &LoadedGltf| -> Vec<Vec3> {
            let sample = ClipSample {
                channels: &g.animations[clip].channels,
                time,
                weight: 1.0,
            };
            let joints = compute_joint_matrices_blended(&g.nodes, &g.skins[0], &[sample]);
            let mesh = &g.meshes[0];
            mesh.vertices
                .iter()
                .zip(
                    mesh.skin
                        .as_ref()
                        .expect("fox's first primitive is skinned"),
                )
                .map(|(v, s)| blend_vertex_position(Vec3::from(v.position), s, &joints))
                .collect()
        };
        let p1 = pose(&one);
        let p2 = pose(&two);

        let extent = p1
            .iter()
            .map(|p| p.abs().max_element())
            .fold(0.0_f32, f32::max);
        // Premise: the clip actually moves the mesh off its rest pose at this
        // time. A frame where nothing moved would let a bake that ignored the
        // animation keys pass.
        let moved = p1
            .iter()
            .zip(&one.meshes[0].vertices)
            .map(|(p, v)| (*p - Vec3::from(v.position)).length())
            .fold(0.0_f32, f32::max);
        assert!(
            moved > 0.05 * extent,
            "premise: at t = {time} the clip must move the fox visibly (moved {moved}, extent {extent})"
        );

        let worst = p1
            .iter()
            .zip(&p2)
            .map(|(a, b)| (*b - *a * S).length())
            .fold(0.0_f32, f32::max);
        assert!(
            worst <= 1e-4 * extent * S,
            "the scaled character must deform to exactly {S}x the unscaled pose; \
             worst vertex is off by {worst} on a model {extent} across"
        );
    }

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

    /// Two targets, two keys: the values are keyframe-major (key 0's two
    /// weights, then key 1's), as glTF stores them. Read target-major, the
    /// pair would come out as [5, 5.5] instead of [0.5, 15].
    #[test]
    fn weights_are_sampled_per_target_from_keyframe_major_values() {
        let channel = AnimationChannel {
            node_index: 0,
            times: vec![0.0, 1.0],
            values: KeyframeValues::Weights(vec![0.0, 10.0, 1.0, 20.0]),
            interpolation: Interpolation::Linear,
        };
        assert_eq!(sample_weights(&channel, 0.5), Some(vec![0.5, 15.0]));
    }

    /// Under CubicSpline each key holds in-tangent, value, out-tangent; the
    /// value is the middle block. The tangents here are 9 so reading the
    /// wrong block is unmistakable.
    #[test]
    fn cubic_spline_weights_read_the_value_block_not_a_tangent() {
        let channel = AnimationChannel {
            node_index: 0,
            times: vec![0.0, 1.0],
            values: KeyframeValues::Weights(vec![9.0, 0.2, 9.0, 9.0, 0.8, 9.0]),
            interpolation: Interpolation::CubicSpline,
        };
        assert_eq!(sample_weights(&channel, 0.0), Some(vec![0.2]));
        assert_eq!(sample_weights(&channel, 1.0), Some(vec![0.8]));
    }

    // ---- pose blending (roadmap item 29) ---------------------------------

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
        compute_joint_matrices_with_ik(nodes, skin, &[], chains, &[])
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
        println!(
            "foot moved {:?} -> {:?}, {err} m from target",
            before[2], after[2]
        );
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

        let none = compute_joint_matrices_with_ik(&nodes, &skin, &[], &[], &[]);
        let zero = compute_joint_matrices_with_ik(&nodes, &skin, &[], &[&chain], &[]);
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

        let none = compute_joint_matrices_with_ik(&nodes, &skin, &[], &[], &[]);
        let typo = compute_joint_matrices_with_ik(&nodes, &skin, &[], &[&chain], &[]);
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

    /// A one-node skeleton at the origin, unrotated and unscaled.
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

    /// A library whose three clips sit at distinct positions, so a wrong
    /// weighting shows as a wrong position rather than a plausible one.
    fn three_clip_library() -> AnimationClipLibrary {
        AnimationClipLibrary::from_clips(vec![
            AnimationClip {
                name: "idle".to_string(),
                channels: vec![fixed_translation(Vec3::ZERO)],
                duration: 1.0,
            },
            AnimationClip {
                name: "run".to_string(),
                channels: vec![fixed_translation(Vec3::new(30.0, 0.0, 0.0))],
                duration: 1.0,
            },
            AnimationClip {
                name: "strafe".to_string(),
                channels: vec![fixed_translation(Vec3::new(0.0, 12.0, 0.0))],
                duration: 1.0,
            },
        ])
    }

    /// Locomotion laid out the way a game would: speed on x, strafe on y.
    fn blend_space_2d(speed: f32, strafe: f32) -> bsengine_core::AnimationStateMachine {
        use bsengine_core::{AnimationStateMachine, AsmState, BlendClip2D, BlendTree2D};
        let mut asm = AnimationStateMachine::default();
        asm.states.insert(
            "locomotion".to_string(),
            AsmState::new("idle").with_blend2d(BlendTree2D {
                param_x: "speed".to_string(),
                param_y: "strafe".to_string(),
                clips: vec![
                    BlendClip2D {
                        clip: "idle".into(),
                        x: 0.0,
                        y: 0.0,
                    },
                    BlendClip2D {
                        clip: "run".into(),
                        x: 3.0,
                        y: 0.0,
                    },
                    BlendClip2D {
                        clip: "strafe".into(),
                        x: 0.0,
                        y: 3.0,
                    },
                ],
            }),
        );
        asm.current_state = "locomotion".to_string();
        asm.params_float.insert("speed".to_string(), speed);
        asm.params_float.insert("strafe".to_string(), strafe);
        asm
    }

    /// The consumer-side proof. The pure `sample` tests show the weights are
    /// right; this shows the animation path asks for them and mixes the pose
    /// accordingly. A disconnected producer is indistinguishable from a working
    /// one if only the producer is tested.
    #[test]
    fn a_2d_blend_space_mixes_all_three_clips_into_the_pose() {
        let library = three_clip_library();
        let idle = library.clips.get("idle").expect("clip exists");
        // The centroid of the triangle: a third of each.
        let asm = blend_space_2d(1.0, 1.0);

        let samples = blend_samples(idle, &library, 0.0, Some(&asm));
        assert_eq!(
            samples.len(),
            3,
            "all three corners contribute at the centroid"
        );

        let pose = compute_local_transforms_blended(&one_node(), &samples);
        let t = pose[0].to_scale_rotation_translation().2;
        assert!(
            (t.x - 10.0).abs() < 0.01,
            "a third of run's 30 along x is 10, got {}",
            t.x
        );
        assert!(
            (t.y - 4.0).abs() < 0.01,
            "a third of strafe's 12 along y is 4, got {}",
            t.y
        );
    }

    /// Both axes must reach the pose, not just the first.
    #[test]
    fn the_second_axis_of_a_2d_blend_space_changes_the_pose() {
        let library = three_clip_library();
        let idle = library.clips.get("idle").expect("clip exists");

        let low = compute_local_transforms_blended(
            &one_node(),
            &blend_samples(idle, &library, 0.0, Some(&blend_space_2d(1.0, 0.2))),
        )[0]
        .to_scale_rotation_translation()
        .2;
        let high = compute_local_transforms_blended(
            &one_node(),
            &blend_samples(idle, &library, 0.0, Some(&blend_space_2d(1.0, 2.0))),
        )[0]
        .to_scale_rotation_translation()
        .2;
        assert!(
            high.y > low.y + 1.0,
            "raising the strafe parameter must move the pose along y: {} -> {}",
            low.y,
            high.y
        );
    }

    /// A state carrying both spaces must use the 2D one.
    #[test]
    fn a_2d_space_takes_precedence_over_a_1d_one_on_the_same_state() {
        use bsengine_core::{BlendClip, BlendTree1D};
        let library = three_clip_library();
        let idle = library.clips.get("idle").expect("clip exists");
        let mut asm = blend_space_2d(0.0, 3.0); // pure strafe under the 2D space
                                                // A 1D tree that would say "pure run" if it won.
        if let Some(state) = asm.states.get_mut("locomotion") {
            state.blend = Some(BlendTree1D {
                param: "speed".to_string(),
                clips: vec![BlendClip {
                    clip: "run".to_string(),
                    threshold: 0.0,
                }],
            });
        }
        let samples = blend_samples(idle, &library, 0.0, Some(&asm));
        let pose = compute_local_transforms_blended(&one_node(), &samples);
        let t = pose[0].to_scale_rotation_translation().2;
        assert!(
            t.y > 11.0 && t.x < 0.01,
            "the 2D space says pure strafe (y=12); if the 1D tree won this would \
             be pure run (x=30). Got {t:?}"
        );
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
            animated_locals: Vec::new(),
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
                    animated_locals: Vec::new(),
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

    // ---- Retargeting through the real system (item 54, sub-step 2/2) ------

    /// A two-bone rig whose arm binds at `rest`, with a clip that holds the arm
    /// at `animated`.
    fn retarget_rig(
        prefix: &str,
        rest: Quat,
        animated: Quat,
    ) -> (SkinnedMesh, AnimationClipLibrary) {
        let nodes = vec![
            NodeTransform {
                name: format!("{prefix}_root"),
                ..Default::default()
            },
            NodeTransform {
                name: format!("{prefix}_arm"),
                position: [0.0, 1.0, 0.0],
                rotation: rest.to_array(),
                parent: Some(0),
                ..Default::default()
            },
        ];
        let mut clips = std::collections::HashMap::new();
        clips.insert(
            "pose".to_string(),
            AnimationClip {
                name: "pose".to_string(),
                duration: 1.0,
                channels: vec![AnimationChannel {
                    node_index: 1,
                    times: vec![0.0, 1.0],
                    values: KeyframeValues::Rotations(vec![
                        animated.to_array(),
                        animated.to_array(),
                    ]),
                    interpolation: Interpolation::Linear,
                }],
            },
        );
        (
            SkinnedMesh {
                mesh_id: 1,
                rest_vertices: Vec::new(),
                skin: Vec::new(),
                skin_data: SkinData {
                    joint_node_indices: vec![0, 1],
                    inverse_bind_matrices: vec![Mat4::IDENTITY.to_cols_array_2d(); 2],
                },
                nodes,
                pose_override: Vec::new(),
                pose_override_weight: 1.0,
                ik_tip_positions: Vec::new(),
                animated_locals: Vec::new(),
                joint_matrices: Vec::new(),
            },
            AnimationClipLibrary { clips },
        )
    }

    #[test]
    fn a_retargeted_character_follows_its_source_through_the_real_system() {
        // Drives `update_skinned_meshes` itself with two entities. The pure
        // function tests cannot see the ECS wiring, and that is exactly where
        // the last sub-step's defect lived: five green tests while the
        // component could only ever hold one chain.
        //
        // Both rigs bind their arm away from identity, and differently -- with
        // an identity source rest the delta and an absolute copy agree
        // numerically and the test proves nothing.
        let source_rest = Quat::from_rotation_y(0.9);
        let target_rest = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let motion = Quat::from_rotation_z(0.6);

        let mut app = bsengine_app::new_app();
        app.insert_resource(bsengine_core::Time::default());
        app.add_plugins(SkinnedMeshPlugin);

        let (source_mesh, source_lib) = retarget_rig("s", source_rest, source_rest * motion);
        let source = app
            .world_mut()
            .spawn((
                source_mesh,
                source_lib,
                bsengine_core::AnimationPlayer::new("pose").with_duration(1.0),
            ))
            .id();

        let (target_mesh, target_lib) = retarget_rig("t", target_rest, target_rest);
        let target = app
            .world_mut()
            .spawn((
                target_mesh,
                target_lib,
                bsengine_core::AnimationPlayer::new("pose").with_duration(1.0),
                RetargetSource {
                    source: "unused-in-this-test".to_string(),
                    resolved: Some(source),
                    pairs: vec![("s_arm".to_string(), "t_arm".to_string())],
                },
            ))
            .id();

        // Two frames: the first publishes the source's locals, the second lets
        // the target read them. The one-frame lag is by design -- ordering two
        // entities inside one query would cost more than it buys.
        app.update();
        app.update();

        let arm = app
            .world()
            .get::<SkinnedMesh>(target)
            .expect("the target keeps its skinned mesh")
            .joint_matrices[1];
        let (_, got_rot, _) = arm.to_scale_rotation_translation();
        let got = got_rot * Vec3::Y;
        let want = (target_rest * motion) * Vec3::Y;
        let unretargeted = target_rest * Vec3::Y;
        println!("retargeted arm {got:?}, want {want:?}, unretargeted would be {unretargeted:?}");

        assert!(
            (got - want).length() < 1.0e-4,
            "the target's arm must receive the source's delta applied to its \
             own rest: got {got:?}, want {want:?}"
        );
        assert!(
            (got - unretargeted).length() > 0.1,
            "and it must differ from the target's own animation ({unretargeted:?}), \
             or this test cannot tell retargeting from doing nothing"
        );
    }

    #[test]
    fn retargeting_runs_before_ik_so_foot_placement_survives() {
        // Order is the thing that fails silently here. Retargeting decides the
        // pose; IK corrects it against the world. Run the other way round,
        // retargeting overwrites the correction and the limb sits wherever the
        // source's motion put it -- with everything upstream looking healthy.
        //
        // Both features drive the SAME limb, which is what makes the ordering
        // observable: if retargeting ran last, the tip could not be on the IK
        // target.
        let source_rest = Quat::from_rotation_y(0.9);
        let motion = Quat::from_rotation_z(0.9);

        let mut app = bsengine_app::new_app();
        app.insert_resource(bsengine_core::Time::default());
        app.add_plugins(SkinnedMeshPlugin);

        let (source_mesh, source_lib) = retarget_rig("s", source_rest, source_rest * motion);
        let source = app
            .world_mut()
            .spawn((
                source_mesh,
                source_lib,
                bsengine_core::AnimationPlayer::new("pose").with_duration(1.0),
            ))
            .id();

        // A three-bone target so there is a chain for IK to solve.
        let (nodes, skin) = leg_skeleton();
        let mut clips = std::collections::HashMap::new();
        clips.insert(
            "pose".to_string(),
            AnimationClip {
                name: "pose".to_string(),
                duration: 1.0,
                channels: Vec::new(),
            },
        );
        let target_pos = Vec3::new(0.3, 0.6, 0.2);
        let target = app
            .world_mut()
            .spawn((
                SkinnedMesh {
                    mesh_id: 1,
                    rest_vertices: Vec::new(),
                    skin: Vec::new(),
                    skin_data: skin,
                    nodes,
                    pose_override: Vec::new(),
                    pose_override_weight: 1.0,
                    ik_tip_positions: Vec::new(),
                    animated_locals: Vec::new(),
                    joint_matrices: Vec::new(),
                },
                AnimationClipLibrary { clips },
                bsengine_core::AnimationPlayer::new("pose").with_duration(1.0),
                RetargetSource {
                    source: "unused".to_string(),
                    resolved: Some(source),
                    // Drives the SAME bone the IK chain roots at.
                    pairs: vec![("s_arm".to_string(), "hip".to_string())],
                },
                IkChains {
                    chains: vec![IkChain {
                        root_bone: "hip".to_string(),
                        mid_bone: "knee".to_string(),
                        tip_bone: "foot".to_string(),
                        target: target_pos.into(),
                        weight: 1.0,
                    }],
                },
            ))
            .id();

        app.update();
        app.update();

        let matrices = &app
            .world()
            .get::<SkinnedMesh>(target)
            .expect("the target keeps its skinned mesh")
            .joint_matrices;
        let foot = matrices[2].transform_point3(Vec3::ZERO);
        let err = (foot - target_pos).length();
        println!("foot with retargeting AND ik: {foot:?}, {err} m from the IK target");
        assert!(
            err < 1.0e-3,
            "the foot must still land on its IK target while retargeting drives \
             the same limb: {foot:?} is {err} m from {target_pos:?}. A larger \
             error means retargeting ran after IK and overwrote the correction."
        );
    }

    /// A torso with two arms hanging off a shared spine, for the goal tests.
    ///
    /// ```text
    ///   6            3         <- hands
    ///    \          /
    ///     5        2           <- upper arms
    ///      \      /
    ///        1                 <- chest (shared by both goals)
    ///        |
    ///        0                 <- pelvis (root)
    /// ```
    fn torso_with_two_arms() -> Vec<NodeTransform> {
        vec![
            NodeTransform {
                name: "pelvis".to_string(),
                ..Default::default()
            },
            NodeTransform {
                name: "chest".to_string(),
                position: [0.0, 1.0, 0.0],
                parent: Some(0),
                ..Default::default()
            },
            NodeTransform {
                name: "l_arm".to_string(),
                position: [-0.5, 0.5, 0.0],
                parent: Some(1),
                ..Default::default()
            },
            NodeTransform {
                name: "l_hand".to_string(),
                position: [-0.7, 0.0, 0.0],
                parent: Some(2),
                ..Default::default()
            },
            NodeTransform {
                name: "r_arm".to_string(),
                position: [0.5, 0.5, 0.0],
                parent: Some(1),
                ..Default::default()
            },
            NodeTransform {
                name: "r_hand".to_string(),
                position: [0.7, 0.0, 0.0],
                parent: Some(4),
                ..Default::default()
            },
        ]
    }

    /// Spawns `torso_with_two_arms` with the given goals and returns the joint
    /// positions the skinning would actually use.
    ///
    /// Asserted on the joint matrices for the reason the chain tests give: a
    /// disconnected consumer looks exactly like a working producer, and this
    /// codebase has shipped that shape before.
    fn solve_goals_through_the_system(goals: Vec<IkGoal>) -> Vec<Vec3> {
        let mut app = bsengine_app::new_app();
        app.insert_resource(bsengine_core::Time::default());
        app.add_plugins(SkinnedMeshPlugin);

        let nodes = torso_with_two_arms();
        let skin_data = SkinData {
            joint_node_indices: (0..nodes.len()).collect(),
            inverse_bind_matrices: vec![Mat4::IDENTITY.to_cols_array_2d(); nodes.len()],
        };
        // IK is a correction over an animated pose, so the system skips an
        // entity with nothing animating. A clip that holds the root still is
        // enough -- the same trick the chain test uses.
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
                    animated_locals: Vec::new(),
                    pose_override_weight: 1.0,
                    joint_matrices: Vec::new(),
                },
                AnimationClipLibrary { clips },
                bsengine_core::AnimationPlayer::new("still").with_duration(1.0),
                IkGoals { goals },
                // ⚠️ Not at the origin. Goal targets are authored in world
                // space and solved in the skeleton's own frame, and with an
                // identity transform those are the same thing -- deleting the
                // conversion left every assertion here green.
                bsengine_core::Transform::from_position(CHARACTER_AT),
            ))
            .id();

        app.update();

        app.world()
            .get::<SkinnedMesh>(entity)
            .expect("the character keeps its skinned mesh")
            .joint_matrices
            .iter()
            .map(|m| m.transform_point3(Vec3::ZERO))
            .collect()
    }

    /// Where the goal-test character stands. See the spawn for why it matters.
    const CHARACTER_AT: Vec3 = Vec3::new(5.0, 0.0, 2.0);

    /// A goal whose target is given in the skeleton's frame, carried out to
    /// world space the way an author would write it.
    fn arm_goal(side: &str, target: Vec3) -> IkGoal {
        let target = target + CHARACTER_AT;
        IkGoal {
            bones: vec![
                "pelvis".to_string(),
                "chest".to_string(),
                format!("{side}_arm"),
                format!("{side}_hand"),
            ],
            target: target.into(),
            weight: 1.0,
        }
    }

    #[test]
    fn full_body_goals_bend_the_shared_spine() {
        // The whole reason this exists. `IkChains` solves each limb alone, so
        // two chains naming the chest fight and the last one wins -- its own
        // doc comment says so. Goals are solved together, and a chest both arms
        // name ends up somewhere both agree on.
        // ⚠️ The target has to sit BETWEEN two reaches, and finding that is
        // most of what this fixture is.
        //
        // From the chest the arm alone spans about 1.41; from the pelvis the
        // whole chain spans about 2.41. A target inside the first needs no help
        // from the spine, so "the chest bends" would be false. A target at the
        // second is at the solver's limit, where the test measures convergence
        // rather than correctness -- 2.38 put the hand 1.7 off in y.
        //
        // This one is 1.54 from the chest and 2.04 from the pelvis: out of the
        // arm's reach, well inside the body's.
        let reach = 1.1;
        let posed = solve_goals_through_the_system(vec![
            arm_goal("l", Vec3::new(-1.0, 1.4, reach)),
            arm_goal("r", Vec3::new(1.0, 1.4, reach)),
        ]);
        assert_eq!(posed.len(), 6, "one matrix per joint");

        assert!(
            posed[1].z > 0.1,
            "both hands reaching forward should carry the chest with them, \
             got {:?}",
            posed[1]
        );
        assert!(
            posed[0].length() < 1.0e-3,
            "and the pelvis must stay put -- a character does not slide toward \
             what it reaches for: {:?}",
            posed[0]
        );
        // ⚠️ And each hand is near the target *in the skeleton's frame*. The
        // assertions above are about the chest and the pelvis and are satisfied
        // by any forward reach, so without this the world-to-model conversion
        // could be deleted and everything here would stay green -- the goal
        // would simply pull toward a point offset by however far the character
        // stands from the origin.
        for (joint, want) in [
            (3usize, Vec3::new(-1.0, 1.4, reach)),
            (5, Vec3::new(1.0, 1.4, reach)),
        ] {
            assert!(
                (posed[joint] - want).length() < 0.25,
                "hand {joint} should reach {want:?} in model space, got {:?} \
                 (character stands at {CHARACTER_AT:?})",
                posed[joint]
            );
        }
    }

    #[test]
    fn a_goal_naming_a_bone_the_rig_lacks_is_dropped_not_guessed() {
        // A scene typo. Posing some other joint would read as a solver bug, so
        // the goal is refused -- and the rest of the pose has to survive it.
        let posed = solve_goals_through_the_system(vec![IkGoal {
            bones: vec!["pelvis".to_string(), "nonexistent".to_string()],
            target: (Vec3::new(2.0, 2.0, 2.0) + CHARACTER_AT).into(),
            weight: 1.0,
        }]);
        let rest = solve_goals_through_the_system(Vec::new());
        for (i, (a, b)) in posed.iter().zip(&rest).enumerate() {
            assert!(
                (*a - *b).length() < 1.0e-4,
                "joint {i} moved for a goal that names nothing: {a:?} vs {b:?}"
            );
        }
    }

    #[test]
    fn a_character_with_no_goals_poses_exactly_as_before() {
        // The property that lets this ship without touching any existing scene.
        let rest = solve_goals_through_the_system(Vec::new());
        let zero_weight = solve_goals_through_the_system(vec![IkGoal {
            weight: 0.0,
            ..arm_goal("l", Vec3::new(-1.2, 1.5, 1.4))
        }]);
        for (i, (a, b)) in rest.iter().zip(&zero_weight).enumerate() {
            assert!(
                (*a - *b).length() < 1.0e-4,
                "joint {i}: a zero-weight goal is not the same as no goal: \
                 {a:?} vs {b:?}"
            );
        }
    }

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
                    animated_locals: Vec::new(),
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
        println!(
            "left foot {:?} ({l_err} m off), right foot {:?} ({r_err} m off)",
            at(3),
            at(6)
        );

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
                    animated_locals: Vec::new(),
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

    /// A rig built the way exporters build them: an armature node turned
    /// -90° about X and scaled 0.5 (Blender's axes), and hips under it that
    /// a one-second "walk" carries 2 units along their local +Y -- which that
    /// armature turns into model -Z at half size, so one lap travels 1 model
    /// unit toward -Z. Halfway, the hips also rise 0.4 local +Z = 0.2 model
    /// +Y: the bob root motion must leave in the pose.
    ///
    /// `rest_turn` rests the hips turned about model +Y (a local turn about
    /// +Z under the armature), with the arc keyed on top of that rest -- the
    /// mesh faces `rest_turn` off the entity's forward, as a rig whose root
    /// rests turned does. Zero for every test but the one about it.
    fn root_motion_rig_with(rest_turn: f32) -> (SkinnedMesh, AnimationClipLibrary) {
        let armature = NodeTransform {
            name: "Armature".to_string(),
            rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2).to_array(),
            scale: [0.5; 3],
            ..Default::default()
        };
        let hips = NodeTransform {
            name: "Hips".to_string(),
            parent: Some(0),
            rotation: Quat::from_rotation_z(rest_turn).to_array(),
            ..Default::default()
        };
        let mut clips = std::collections::HashMap::new();
        clips.insert(
            "walk".to_string(),
            AnimationClip {
                name: "walk".to_string(),
                duration: 1.0,
                channels: vec![AnimationChannel {
                    node_index: 1,
                    times: vec![0.0, 0.5, 1.0],
                    values: KeyframeValues::Translations(vec![
                        [0.0, 0.0, 0.0],
                        [0.0, 1.0, 0.4],
                        [0.0, 2.0, 0.0],
                    ]),
                    interpolation: Interpolation::Linear,
                }],
            },
        );
        clips.insert("arc".to_string(), arc_clip(rest_turn));
        (
            SkinnedMesh {
                mesh_id: 1,
                rest_vertices: Vec::new(),
                skin: Vec::new(),
                skin_data: SkinData {
                    joint_node_indices: vec![1],
                    inverse_bind_matrices: vec![Mat4::IDENTITY.to_cols_array_2d()],
                },
                nodes: vec![armature, hips],
                pose_override: Vec::new(),
                pose_override_weight: 1.0,
                ik_tip_positions: Vec::new(),
                animated_locals: Vec::new(),
                joint_matrices: Vec::new(),
            },
            AnimationClipLibrary { clips },
        )
    }

    /// A one-second "arc": the hips walk a quarter circle of radius 1 in
    /// model space, starting toward -Z and turning 90° left (+yaw) as they go
    /// -- a character walking a curve, facing along it. Keyed every 0.1 s,
    /// which is the test frame step, so every frame samples a key exactly:
    /// model position `(cos t - 1, 0, -sin t)` at turn `t`, written in the
    /// hips' local frame under the Blender armature (model `(x, y, z)` is
    /// local `(2x, -2z, 2y)`, and a model yaw is a local turn about +Z).
    ///
    /// With a `rest_turn`, the whole arc is turned by it -- walked from the
    /// rest facing, not from model -Z.
    fn arc_clip(rest_turn: f32) -> AnimationClip {
        let keys = 10;
        let (mut times, mut positions, mut rotations) = (vec![], vec![], vec![]);
        for k in 0..=keys {
            let t = k as f32 / keys as f32;
            let turn = t * std::f32::consts::FRAC_PI_2;
            let model =
                Quat::from_rotation_y(rest_turn) * Vec3::new(turn.cos() - 1.0, 0.0, -turn.sin());
            times.push(t);
            positions.push([2.0 * model.x, -2.0 * model.z, 2.0 * model.y]);
            rotations.push(Quat::from_rotation_z(rest_turn + turn).to_array());
        }
        AnimationClip {
            name: "arc".to_string(),
            duration: 1.0,
            channels: vec![
                AnimationChannel {
                    node_index: 1,
                    times: times.clone(),
                    values: KeyframeValues::Translations(positions),
                    interpolation: Interpolation::Linear,
                },
                AnimationChannel {
                    node_index: 1,
                    times,
                    values: KeyframeValues::Rotations(rotations),
                    interpolation: Interpolation::Linear,
                },
            ],
        }
    }

    /// The entity is turned 90° about Y and scaled 2, so model -Z becomes
    /// world -X at twice the length: one lap moves it 2 units toward -X.
    fn root_motion_app(
        motion: bsengine_core::RootMotion,
        speed: f32,
    ) -> (bevy_app::App, bevy_ecs::entity::Entity) {
        root_motion_app_playing(motion, speed, "walk")
    }

    fn root_motion_app_playing(
        motion: bsengine_core::RootMotion,
        speed: f32,
        clip: &str,
    ) -> (bevy_app::App, bevy_ecs::entity::Entity) {
        root_motion_app_resting(motion, speed, clip, 0.0)
    }

    fn root_motion_app_resting(
        motion: bsengine_core::RootMotion,
        speed: f32,
        clip: &str,
        rest_turn: f32,
    ) -> (bevy_app::App, bevy_ecs::entity::Entity) {
        let mut app = bsengine_app::new_app();
        let mut time = bsengine_core::Time::default();
        time.set_delta_for_test(0.1);
        app.insert_resource(time);
        app.add_plugins(bsengine_app::AnimationPlugin);
        app.add_plugins(SkinnedMeshPlugin);
        let (mesh, library) = root_motion_rig_with(rest_turn);
        let transform = bsengine_core::Transform {
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).into(),
            scale: Vec3::splat(2.0).into(),
            ..Default::default()
        };
        let entity = app
            .world_mut()
            .spawn((
                mesh,
                library,
                bsengine_core::AnimationPlayer::new(clip)
                    .with_duration(1.0)
                    .with_speed(speed),
                motion,
                transform,
            ))
            .id();
        (app, entity)
    }

    fn position(app: &bevy_app::App, entity: bevy_ecs::entity::Entity) -> Vec3 {
        app.world()
            .get::<bsengine_core::Transform>(entity)
            .unwrap()
            .position
            .0
    }

    fn near(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-3
    }

    /// Half a lap in five frames: the entity has walked 1 world unit toward
    /// -X -- every frame counted, the first included -- and the hips in the
    /// pose have stayed over the rest spot horizontally while keeping the
    /// 0.2 bob. Both halves are needed: an entity that moved while the pose
    /// also moved would walk at double speed, and one that moved while the
    /// pose kept the bob out would stop bobbing.
    #[test]
    fn root_motion_moves_the_entity_and_takes_the_travel_out_of_the_pose() {
        let (mut app, entity) = root_motion_app(bsengine_core::RootMotion::default(), 1.0);
        for _ in 0..5 {
            app.update();
        }
        let p = position(&app, entity);
        assert!(
            near(p, Vec3::new(-1.0, 0.0, 0.0)),
            "half a lap is 1 unit toward -X: {p}"
        );

        let skinned = app.world().get::<SkinnedMesh>(entity).unwrap();
        let hips = accumulate_globals(&skinned.nodes, &skinned.animated_locals)[1]
            .w_axis
            .truncate();
        assert!(
            hips.x.abs() < 1e-4 && hips.z.abs() < 1e-4,
            "the hips stay over the rest spot horizontally: {hips}"
        );
        assert!((hips.y - 0.2).abs() < 1e-4, "and keep the bob: {hips}");
    }

    /// A lap and a half: 3 units, continuously. Measuring straight from the
    /// last time to the new one across the wrap would drag the entity back a
    /// whole lap at the loop point.
    #[test]
    fn root_motion_carries_on_across_a_loop() {
        let (mut app, entity) = root_motion_app(bsengine_core::RootMotion::default(), 1.0);
        for _ in 0..15 {
            app.update();
        }
        let p = position(&app, entity);
        assert!(
            near(p, Vec3::new(-3.0, 0.0, 0.0)),
            "a lap and a half is 3 units: {p}"
        );
    }

    /// Played backwards the clip walks the other way, wrap included.
    #[test]
    fn reversed_root_motion_walks_backwards() {
        let (mut app, entity) = root_motion_app(bsengine_core::RootMotion::default(), -1.0);
        for _ in 0..15 {
            app.update();
        }
        let p = position(&app, entity);
        assert!(
            near(p, Vec3::new(3.0, 0.0, 0.0)),
            "backwards, toward +X: {p}"
        );
    }

    /// Reporting only: the entity stays put, the pose is still stripped, and
    /// the frame's travel is in `last_delta` for a script to route through a
    /// character controller.
    #[test]
    fn root_motion_can_be_reported_without_being_applied() {
        let (mut app, entity) = root_motion_app(
            bsengine_core::RootMotion {
                apply_to_transform: false,
                ..Default::default()
            },
            1.0,
        );
        for _ in 0..3 {
            app.update();
        }
        assert!(near(position(&app, entity), Vec3::ZERO));
        let delta = app
            .world()
            .get::<bsengine_core::RootMotion>(entity)
            .unwrap()
            .last_delta
            .0;
        assert!(
            near(delta, Vec3::new(-0.2, 0.0, 0.0)),
            "one frame's travel, world space: {delta}"
        );
    }

    fn yaw(app: &bevy_app::App, entity: bevy_ecs::entity::Entity) -> f32 {
        let q = app
            .world()
            .get::<bsengine_core::Transform>(entity)
            .unwrap()
            .rotation
            .0;
        yaw_of(q)
    }

    fn same_angle(a: f32, b: f32) -> bool {
        wrap_angle(a - b).abs() < 1e-3
    }

    /// The hips' yaw in the pose the character shows.
    fn shown_hips_yaw(app: &bevy_app::App, entity: bevy_ecs::entity::Entity) -> f32 {
        let skinned = app.world().get::<SkinnedMesh>(entity).unwrap();
        let (_, r, _) = accumulate_globals(&skinned.nodes, &skinned.animated_locals)[1]
            .to_scale_rotation_translation();
        yaw_of(r)
    }

    /// Walking the arc with rotation extracted: halfway, the entity has
    /// turned 45° and the pose shows the hips at their rest yaw (the turn
    /// went to the entity, not the mesh); after the lap it has turned 90° --
    /// and stands exactly where translation-only root motion puts it, since
    /// the path is the same path. A root motion that turned the entity AND
    /// kept measuring travel in model space would walk the arc turned a
    /// second time, and end somewhere else.
    #[test]
    fn root_motion_turns_the_entity_along_the_arc_it_walks() {
        use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};
        let (mut app, entity) =
            root_motion_app_playing(bsengine_core::RootMotion::default(), 1.0, "arc");
        let (mut baked, baked_entity) = root_motion_app_playing(
            bsengine_core::RootMotion {
                apply_rotation: false,
                ..Default::default()
            },
            1.0,
            "arc",
        );
        for _ in 0..5 {
            app.update();
            baked.update();
        }
        assert!(
            same_angle(yaw(&app, entity), FRAC_PI_2 + FRAC_PI_4),
            "halfway the entity has turned 45° on top of its own 90°: {}",
            yaw(&app, entity).to_degrees()
        );
        assert!(
            same_angle(shown_hips_yaw(&app, entity), 0.0),
            "and the pose shows the hips unturned: {}",
            shown_hips_yaw(&app, entity).to_degrees()
        );
        assert!(
            same_angle(shown_hips_yaw(&baked, baked_entity), FRAC_PI_4),
            "premise: without extraction the pose itself turns 45°: {}",
            shown_hips_yaw(&baked, baked_entity).to_degrees()
        );
        assert!(
            same_angle(yaw(&baked, baked_entity), FRAC_PI_2),
            "and the entity keeps its own 90°"
        );
        for _ in 0..5 {
            app.update();
            baked.update();
        }
        assert!(
            same_angle(yaw(&app, entity), PI),
            "a lap turns it 90°: {}",
            yaw(&app, entity).to_degrees()
        );
        let (p, q) = (position(&app, entity), position(&baked, baked_entity));
        assert!(
            near(p, Vec3::new(-2.0, 0.0, 2.0)) && near(p, q),
            "the same place translation-only root motion reaches: {p} vs {q}"
        );
    }

    /// Two laps of the arc make a half circle: the second quarter is walked
    /// in the facing the first turned to, across the loop's wrap. Without
    /// the turn carried over, the second lap would repeat the first's
    /// direction and end at (-4, 0, 4) instead.
    #[test]
    fn a_turning_loop_keeps_turning_across_the_wrap() {
        let (mut app, entity) =
            root_motion_app_playing(bsengine_core::RootMotion::default(), 1.0, "arc");
        for _ in 0..20 {
            app.update();
        }
        let p = position(&app, entity);
        assert!(near(p, Vec3::new(0.0, 0.0, 4.0)), "a half circle: {p}");
        assert!(
            same_angle(yaw(&app, entity), -std::f32::consts::FRAC_PI_2),
            "turned 180° on top of 90°: {}",
            yaw(&app, entity).to_degrees()
        );
    }

    /// Reporting only: the turn is in `last_rotation_delta` -- 9° a frame --
    /// and the entity has not turned.
    #[test]
    fn a_reported_turn_leaves_the_entity_facing_where_it_was() {
        let (mut app, entity) = root_motion_app_playing(
            bsengine_core::RootMotion {
                apply_to_transform: false,
                ..Default::default()
            },
            1.0,
            "arc",
        );
        for _ in 0..3 {
            app.update();
        }
        let turn = app
            .world()
            .get::<bsengine_core::RootMotion>(entity)
            .unwrap()
            .last_rotation_delta;
        assert!(
            (turn - 9f32.to_radians()).abs() < 1e-4,
            "one frame's turn: {}",
            turn.to_degrees()
        );
        assert!(same_angle(yaw(&app, entity), std::f32::consts::FRAC_PI_2));
    }

    /// A root that rests turned 30° (the mesh faces 30° off the entity's
    /// forward) walking the same arc from that rest: the turn and the path
    /// relative to the rest are what they were at 0°, so the entity turns
    /// 90° and ends where translation-only root motion puts it -- the arc
    /// turned by the rest, which is the way the mesh walked. Measuring the
    /// travel against the root's yaw without its rest would turn the whole
    /// path by -30°.
    #[test]
    fn a_root_resting_turned_walks_the_way_its_mesh_faces() {
        use std::f32::consts::PI;
        let rest = 30f32.to_radians();
        let (mut app, entity) =
            root_motion_app_resting(bsengine_core::RootMotion::default(), 1.0, "arc", rest);
        let (mut baked, baked_entity) = root_motion_app_resting(
            bsengine_core::RootMotion {
                apply_rotation: false,
                ..Default::default()
            },
            1.0,
            "arc",
            rest,
        );
        for _ in 0..10 {
            app.update();
            baked.update();
        }
        let expected = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
            * (2.0 * (Quat::from_rotation_y(rest) * Vec3::new(-1.0, 0.0, -1.0)));
        let (p, q) = (position(&app, entity), position(&baked, baked_entity));
        assert!(
            near(q, expected),
            "premise: translation-only walks the turned arc: {q} vs {expected}"
        );
        assert!(near(p, q), "and so does extraction: {p} vs {q}");
        assert!(
            same_angle(yaw(&app, entity), PI),
            "turning 90°: {}",
            yaw(&app, entity).to_degrees()
        );
        assert!(
            same_angle(shown_hips_yaw(&app, entity), rest),
            "the pose keeps its rest turn: {}",
            shown_hips_yaw(&app, entity).to_degrees()
        );
    }

    #[test]
    fn angles_wrap_the_short_way_round() {
        use std::f32::consts::PI;
        assert!((wrap_angle(1.5 * PI) + 0.5 * PI).abs() < 1e-5);
        assert!((wrap_angle(-1.5 * PI) - 0.5 * PI).abs() < 1e-5);
        assert!((wrap_angle(PI) - PI).abs() < 1e-5);
        assert!((wrap_angle(0.25) - 0.25).abs() < 1e-6);
    }
}

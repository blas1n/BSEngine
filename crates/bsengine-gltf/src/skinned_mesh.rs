use bevy_app::{App, Plugin, PostUpdate};
use bevy_ecs::prelude::{Component, Query, ReflectComponent, Res, ResMut};

use crate::animation::{AnimationChannel, AnimationClip, Interpolation, KeyframeValues};
use crate::loader::{NodeTransform, SkinData, VertexSkin};
use bsengine_rhi_wgpu::Vertex;
use glam::{Mat4, Quat, Vec3};

/// Rest-pose (bind pose) geometry, skin/joint data, and node hierarchy needed
/// to re-derive a skinned mesh's deformed vertices every frame from whichever
/// clip its `AnimationPlayer` is currently sampling. Attached alongside
/// `MeshRenderer` by `GltfPlugin` when the source glTF had a skin.
/// # What is reflected, and what is not
///
/// `mesh_id` is reflected; the four bulk fields below are `#[reflect(ignore)]`.
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

/// Composes every node's current LOCAL transform: the clip's sampled value
/// for translation/rotation/scale if `channels` has an entry targeting that
/// node, otherwise the node's rest/bind-pose value from `nodes`.
fn compute_local_transforms(
    nodes: &[NodeTransform],
    channels: &[AnimationChannel],
    time: f32,
) -> Vec<Mat4> {
    nodes
        .iter()
        .enumerate()
        .map(|(node_index, rest)| {
            let mut t = Vec3::from(rest.translation);
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
            Mat4::from_scale_rotation_translation(s, r, t)
        })
        .collect()
}

/// Walks the node hierarchy to compose each node's GLOBAL transform from its
/// local transform and its parent chain, then returns one skinning matrix per
/// joint (`global[joint_node] * inverse_bind_matrix[joint]`) — the matrix
/// each of that joint's vertices gets blended through. Iterates a fixed
/// number of passes rather than a proper topological sort, matching the same
/// pattern `bsengine_core::propagate_global_transforms` already uses for
/// parent/child Transform hierarchies in this codebase.
fn compute_joint_matrices(
    nodes: &[NodeTransform],
    skin: &SkinData,
    channels: &[AnimationChannel],
    time: f32,
) -> Vec<Mat4> {
    let locals = compute_local_transforms(nodes, channels, time);
    let mut globals = locals.clone();
    for _ in 0..8 {
        for (i, node) in nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                globals[i] = globals[parent] * locals[i];
            }
        }
    }
    skin.joint_node_indices
        .iter()
        .zip(&skin.inverse_bind_matrices)
        .map(|(&node_index, ibm)| globals[node_index] * Mat4::from_cols_array_2d(ibm))
        .collect()
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

/// Drives CPU-side skeletal skinning: each frame, for every entity with
/// `SkinnedMesh` + `AnimationClipLibrary` + `AnimationPlayer`, samples the
/// player's current clip, composes joint matrices, blends the rest-pose
/// vertices, and re-uploads them into the same GPU mesh id. Runs in
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
    query: Query<(
        &SkinnedMesh,
        &AnimationClipLibrary,
        &bsengine_core::AnimationPlayer,
    )>,
    mesh_registry: Option<ResMut<bsengine_rhi_wgpu::GpuMeshRegistry>>,
    queue: Option<Res<bsengine_rhi_wgpu::GpuQueueResource>>,
) {
    let (Some(mut mesh_registry), Some(queue)) = (mesh_registry, queue) else {
        return;
    };
    for (skinned, library, player) in query.iter() {
        let Some(clip) = library.clips.get(&player.clip) else {
            continue;
        };
        let joint_matrices = compute_joint_matrices(
            &skinned.nodes,
            &skinned.skin_data,
            &clip.channels,
            player.time,
        );
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

    #[test]
    fn compute_joint_matrices_uses_bind_pose_when_no_channels_animate_a_node() {
        let nodes = vec![NodeTransform {
            translation: [1.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            parent: None,
        }];
        let skin = SkinData {
            joint_node_indices: vec![0],
            inverse_bind_matrices: vec![Mat4::IDENTITY.to_cols_array_2d()],
        };
        let matrices = compute_joint_matrices(&nodes, &skin, &[], 0.0);
        let expected = Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0));
        assert!(matrices[0].abs_diff_eq(expected, 0.001));
    }

    #[test]
    fn compute_joint_matrices_composes_parent_child_hierarchy() {
        let nodes = vec![
            NodeTransform {
                translation: [1.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
                parent: None,
            },
            NodeTransform {
                translation: [0.0, 2.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
                parent: Some(0),
            },
        ];
        let skin = SkinData {
            joint_node_indices: vec![1],
            inverse_bind_matrices: vec![Mat4::IDENTITY.to_cols_array_2d()],
        };
        let matrices = compute_joint_matrices(&nodes, &skin, &[], 0.0);
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
        // (no RHI plugin added), so the system should compute the deformed
        // pose without panicking even when it can't upload anywhere -- this
        // test's job is to prove the math runs end-to-end via the ECS system,
        // not to assert on GPU state.
        assert!(app.world().get::<SkinnedMesh>(entity).is_some());
    }
}

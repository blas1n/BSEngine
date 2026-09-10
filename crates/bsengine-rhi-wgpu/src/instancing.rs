//! Grouping draw-call indices into instanced batches.
//!
//! Kept separate from `surface.rs` (~7,000 lines) and free of any GPU,
//! window or adapter dependency, so grouping and slot-array alignment are
//! testable as plain data.

use crate::surface::MaterialParams;
use glam::Mat4;
use std::collections::HashMap;

/// One draw call's worth of data, as `render_frame` receives it.
type DrawCall = (u64, Mat4, Option<u64>, MaterialParams, Option<String>);

/// Slot-array alignment, in `u32`s.
///
/// `wgpu::Limits::downlevel_defaults()` sets
/// `min_storage_buffer_offset_alignment` to 256 bytes, and a slot is a
/// 4-byte `u32`, so every batch must start on a 64-slot boundary to be
/// reachable by a dynamic offset.
pub const SLOT_ALIGNMENT: u32 = 64;

/// One instanced draw: every object in it shares [`Batch::mesh_id`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Batch {
    /// The mesh every instance in this batch draws.
    pub mesh_id: u64,
    /// Where this batch's slot list starts in the frame's slot array.
    /// Always a multiple of [`SLOT_ALIGNMENT`].
    pub slot_base: u32,
    /// How many instances this batch draws.
    pub count: u32,
}

/// Groups `indices` (positions in `draw_calls`) by mesh, appending each
/// batch's model-slot list to `slots`.
///
/// The values written into `slots` are the **original `draw_calls`
/// indices**, unchanged. That index is the model uniform's slot, so a
/// filtered or renumbered position would hand every object someone else's
/// transform — the same invariant the per-object point-shadow loop spells
/// out where it computes its dynamic offset.
///
/// `slots` is appended to rather than replaced because one array carries
/// every pass's lists for a single upload; each batch reaches its own list
/// through a dynamic offset.
///
/// Batches follow **first appearance** of each mesh, not hash order, so a
/// frame's draw order is reproducible.
///
/// This function holds no registry and cannot know whether a mesh is
/// loaded; the caller looks that up once per batch. It also does not clamp
/// to `MAX_OBJECTS` — callers pass an already-clamped index list.
pub fn build_batches(
    indices: &[usize],
    draw_calls: &[DrawCall],
    slots: &mut Vec<u32>,
) -> Vec<Batch> {
    let mut order: Vec<u64> = Vec::new();
    let mut groups: HashMap<u64, Vec<u32>> = HashMap::new();
    for &i in indices {
        let mesh_id = draw_calls[i].0;
        if let Some(list) = groups.get_mut(&mesh_id) {
            list.push(i as u32);
        } else {
            order.push(mesh_id);
            groups.insert(mesh_id, vec![i as u32]);
        }
    }

    let mut batches = Vec::with_capacity(order.len());
    for mesh_id in order {
        let list = &groups[&mesh_id];
        // Pad to the next dynamic-offset boundary. The padding slots are
        // never read: `count` bounds the instance range of the draw.
        while slots.len() % SLOT_ALIGNMENT as usize != 0 {
            slots.push(0);
        }
        let slot_base = slots.len() as u32;
        slots.extend_from_slice(list);
        batches.push(Batch {
            mesh_id,
            slot_base,
            count: list.len() as u32,
        });
    }
    batches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::MaterialParams;
    use glam::Mat4;

    /// Builds a `draw_calls`-shaped slice from mesh ids alone; nothing in
    /// `build_batches` reads any other field.
    fn calls(mesh_ids: &[u64]) -> Vec<DrawCall> {
        mesh_ids
            .iter()
            .map(|&id| (id, Mat4::IDENTITY, None, MaterialParams::default(), None))
            .collect()
    }

    #[test]
    fn interleaved_meshes_collapse_into_one_batch_each() {
        // A B A B -- the case a run-length grouping gets wrong, producing
        // four batches of one instead of two batches of two.
        let dc = calls(&[7, 9, 7, 9]);
        let mut slots = Vec::new();
        let batches = build_batches(&[0, 1, 2, 3], &dc, &mut slots);

        assert_eq!(
            batches.len(),
            2,
            "A B A B must group into two batches, got {batches:?}"
        );
        assert_eq!(batches[0].mesh_id, 7);
        assert_eq!(batches[0].count, 2);
        assert_eq!(batches[1].mesh_id, 9);
        assert_eq!(batches[1].count, 2);

        let first: Vec<u32> = slots
            [batches[0].slot_base as usize..(batches[0].slot_base + batches[0].count) as usize]
            .to_vec();
        let second: Vec<u32> = slots
            [batches[1].slot_base as usize..(batches[1].slot_base + batches[1].count) as usize]
            .to_vec();
        assert_eq!(
            first,
            vec![0, 2],
            "mesh 7 sits at draw_calls indices 0 and 2"
        );
        assert_eq!(
            second,
            vec![1, 3],
            "mesh 9 sits at draw_calls indices 1 and 3"
        );
    }

    #[test]
    fn every_slot_base_is_dynamic_offset_aligned() {
        // Three distinct meshes -> three batches, each of which must start
        // on a 256-byte boundary because `min_storage_buffer_offset_alignment`
        // is 256 and a slot is 4 bytes.
        let dc = calls(&[1, 2, 3]);
        let mut slots = Vec::new();
        let batches = build_batches(&[0, 1, 2], &dc, &mut slots);

        assert_eq!(batches.len(), 3);
        for b in &batches {
            assert_eq!(
                b.slot_base % SLOT_ALIGNMENT,
                0,
                "slot_base {} is not a multiple of {SLOT_ALIGNMENT}, so it cannot be used \
                 as a dynamic offset",
                b.slot_base
            );
        }
    }

    #[test]
    fn batches_append_after_slots_already_written_by_an_earlier_pass() {
        // Every pass in a frame appends to one shared array. An
        // implementation that indexed from zero would overwrite the
        // directional pass's lists with the first point light's.
        let dc = calls(&[4, 4]);
        let mut slots = vec![u32::MAX; 3]; // an earlier pass's data
        let batches = build_batches(&[0, 1], &dc, &mut slots);

        assert_eq!(batches.len(), 1);
        assert!(
            batches[0].slot_base >= 3,
            "slot_base {} overlaps the 3 slots an earlier pass already wrote",
            batches[0].slot_base
        );
        assert_eq!(
            &slots[..3],
            &[u32::MAX; 3],
            "the earlier pass's slots were overwritten"
        );
    }

    #[test]
    fn an_empty_index_list_produces_no_batches() {
        let dc = calls(&[1, 2]);
        let mut slots = Vec::new();
        let batches = build_batches(&[], &dc, &mut slots);
        assert!(batches.is_empty(), "got {batches:?}");
        assert!(slots.is_empty(), "no batches must write no slots");
    }

    #[test]
    fn one_mesh_many_objects_is_a_single_batch() {
        let dc = calls(&[5; 40]);
        let mut slots = Vec::new();
        let indices: Vec<usize> = (0..40).collect();
        let batches = build_batches(&indices, &dc, &mut slots);

        assert_eq!(batches.len(), 1, "40 objects of one mesh is one batch");
        assert_eq!(batches[0].count, 40);
    }

    #[test]
    fn batch_order_follows_first_appearance_not_hash_order() {
        // Frame-to-frame draw order must be deterministic. Iterating a
        // HashMap would reorder batches between runs.
        let dc = calls(&[30, 10, 20, 10]);
        let mut slots = Vec::new();
        let batches = build_batches(&[0, 1, 2, 3], &dc, &mut slots);

        let order: Vec<u64> = batches.iter().map(|b| b.mesh_id).collect();
        assert_eq!(
            order,
            vec![30, 10, 20],
            "batches must follow first appearance"
        );
    }
}

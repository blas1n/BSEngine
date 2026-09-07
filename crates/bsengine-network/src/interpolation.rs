//! Rendering remote entities from a buffer of timestamped snapshots.
//!
//! # Why remote entities render in the past
//!
//! To interpolate between two snapshots you need both of them, which means
//! waiting until the later one has arrived. Rendering one delay behind the
//! newest is what turns unevenly-arriving packets into smooth motion — and it is
//! a real cost, not a free win: remote entities are displayed slightly behind
//! where the server says they are. `NetworkConfig::interpolation_delay_ticks` is
//! that cost, written as a number somebody can choose.
//!
//! # Why it holds instead of extrapolating
//!
//! Past the newest snapshot this returns the newest one rather than continuing
//! along the last known velocity. Extrapolation is a guess, and a guess that
//! overshoots snaps visibly backwards when the truth arrives. Holding is also
//! wrong, but it is wrong in a way that stays still — and a frozen remote entity
//! reads as a network problem, which is what it is, rather than as the engine
//! inventing motion that never happened.

use std::collections::HashMap;

use bevy_ecs::prelude::Resource;
use bsengine_core::Transform;

use crate::packet::TransformData;

/// How many snapshots to keep per entity.
///
/// Enough to bracket a render time at any sane delay, and bounded so a session
/// that runs for hours does not grow without limit.
const BUFFER_LEN: usize = 32;

/// Timestamped snapshots per network id, oldest first.
#[derive(Resource, Default, Debug)]
pub struct SnapshotBuffers {
    buffers: HashMap<u64, Vec<(u32, TransformData)>>,
}

impl SnapshotBuffers {
    /// Records a snapshot, dropping the oldest once the buffer is full.
    pub fn push(&mut self, net_id: u64, tick: u32, data: TransformData) {
        let buffer = self.buffers.entry(net_id).or_default();
        buffer.push((tick, data));
        // UDP can deliver out of order, so ordering is restored here rather than
        // left for `sample` to re-derive on every entity on every frame.
        buffer.sort_by_key(|(tick, _)| *tick);
        if buffer.len() > BUFFER_LEN {
            buffer.remove(0);
        }
    }

    /// This entity's snapshots, oldest first.
    pub fn get(&self, net_id: u64) -> Option<&[(u32, TransformData)]> {
        self.buffers.get(&net_id).map(Vec::as_slice)
    }

    /// The newest tick recorded for any entity — what a client's render clock
    /// follows.
    pub fn newest_tick(&self) -> Option<u32> {
        self.buffers
            .values()
            .filter_map(|buffer| buffer.last().map(|(tick, _)| *tick))
            .max()
    }

    /// How many entities have snapshots.
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// Whether nothing has been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }
}

/// The transform to display at `render_tick`.
///
/// Returns `None` only for an empty buffer. Before the oldest snapshot it
/// returns the oldest; past the newest it **holds** the newest — see the module
/// docs for why that is deliberate rather than a missing feature.
pub fn sample(buffer: &[(u32, TransformData)], render_tick: f32) -> Option<Transform> {
    let first = buffer.first()?;
    let last = buffer.last()?;

    if render_tick <= first.0 as f32 {
        return Some(first.1.to_transform());
    }
    if render_tick >= last.0 as f32 {
        return Some(last.1.to_transform());
    }

    let pair = buffer
        .windows(2)
        .find(|w| render_tick >= w[0].0 as f32 && render_tick <= w[1].0 as f32)?;
    let (a_tick, a) = (pair[0].0 as f32, pair[0].1);
    let (b_tick, b) = (pair[1].0 as f32, pair[1].1);

    // Guarded because two snapshots can share a tick if the server sent twice in
    // one frame. Dividing by that span would put a NaN into every skinned vertex
    // of the entity, which presents nowhere near this function.
    let span = b_tick - a_tick;
    let t = if span > 0.0 {
        ((render_tick - a_tick) / span).clamp(0.0, 1.0)
    } else {
        1.0
    };

    let (a, b) = (a.to_transform(), b.to_transform());
    Some(Transform {
        position: a.position.0.lerp(b.position.0, t).into(),
        rotation: a.rotation.0.slerp(b.rotation.0, t).into(),
        scale: a.scale.0.lerp(b.scale.0, t).into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn at(x: f32) -> TransformData {
        let mut t = Transform::default();
        t.position = Vec3::new(x, 0.0, 0.0).into();
        TransformData::from_transform(&t)
    }

    fn buffer() -> Vec<(u32, TransformData)> {
        vec![(10u32, at(0.0)), (20u32, at(100.0))]
    }

    /// The headline behaviour, asserted as an exact fraction of a real distance.
    ///
    /// Deliberately not "strictly between the two", which item 52 learned the
    /// hard way is satisfied by ~2e-12 of numerical noise — an implementation
    /// that did no blending at all passed that shape of assertion.
    #[test]
    fn a_render_time_between_two_snapshots_is_that_fraction_of_the_way() {
        let sampled = sample(&buffer(), 12.5).expect("in range");

        assert!(
            (sampled.position.0.x - 25.0).abs() < 0.01,
            "a quarter of the way from tick 10 to tick 20 is x=25, got {}",
            sampled.position.0.x
        );
    }

    /// The endpoints, which pin that the fraction is measured from the right
    /// end: an implementation blending backwards passes the midpoint but not
    /// these.
    #[test]
    fn a_render_time_on_a_snapshot_is_that_snapshot() {
        assert!((sample(&buffer(), 10.0).expect("in range").position.0.x - 0.0).abs() < 0.01);
        assert!((sample(&buffer(), 20.0).expect("in range").position.0.x - 100.0).abs() < 0.01);
    }

    /// Asserted as equality with the last known value rather than "close to it":
    /// a short extrapolation would also be close, and the difference between
    /// holding and extrapolating is the whole decision.
    #[test]
    fn past_the_newest_snapshot_it_holds_rather_than_extrapolating() {
        let held = sample(&buffer(), 40.0).expect("holds");

        assert!(
            (held.position.0.x - 100.0).abs() < 1e-6,
            "must equal the newest snapshot, not continue past it; got {}",
            held.position.0.x
        );
    }

    /// Before the oldest, the oldest — a client that has just connected has a
    /// render clock behind everything it holds, and must show something rather
    /// than nothing.
    #[test]
    fn before_the_oldest_snapshot_it_shows_the_oldest() {
        assert!((sample(&buffer(), 1.0).expect("holds").position.0.x - 0.0).abs() < 1e-6);
    }

    #[test]
    fn an_empty_buffer_samples_to_nothing() {
        assert!(sample(&[], 5.0).is_none());
    }

    /// Two snapshots on the same tick must not divide by zero. A NaN here
    /// reaches every vertex of the entity and presents nowhere near this code.
    #[test]
    fn two_snapshots_on_one_tick_do_not_produce_a_nan() {
        let same = vec![(10u32, at(0.0)), (10u32, at(50.0))];
        let sampled = sample(&same, 10.0).expect("in range");
        assert!(sampled.position.0.x.is_finite());
    }

    /// Out-of-order arrival is normal over UDP; the buffer sorts so `sample`
    /// can assume order. Without this an older snapshot arriving late would sit
    /// at the end and be treated as the newest.
    #[test]
    fn a_late_snapshot_is_ordered_by_tick_not_by_arrival() {
        let mut buffers = SnapshotBuffers::default();
        buffers.push(1, 20, at(100.0));
        buffers.push(1, 10, at(0.0));

        let stored = buffers.get(1).expect("buffered");
        assert_eq!(
            stored.iter().map(|(tick, _)| *tick).collect::<Vec<_>>(),
            vec![10, 20],
            "ordered by tick, not by when it turned up"
        );
        assert!((sample(stored, 12.5).expect("in range").position.0.x - 25.0).abs() < 0.01);
    }

    /// The buffer is bounded, and keeps the *newest* — dropping the newest
    /// would make a long session render further and further behind.
    #[test]
    fn a_full_buffer_drops_the_oldest_and_keeps_the_newest() {
        let mut buffers = SnapshotBuffers::default();
        for tick in 0..(BUFFER_LEN as u32 + 10) {
            buffers.push(1, tick, at(tick as f32));
        }

        let stored = buffers.get(1).expect("buffered");
        assert_eq!(stored.len(), BUFFER_LEN);
        assert_eq!(
            stored.last().expect("newest").0,
            BUFFER_LEN as u32 + 9,
            "the newest snapshot must survive"
        );
    }

    #[test]
    fn the_newest_tick_is_the_max_across_entities() {
        let mut buffers = SnapshotBuffers::default();
        buffers.push(1, 5, at(0.0));
        buffers.push(2, 9, at(0.0));
        assert_eq!(buffers.newest_tick(), Some(9));
    }
}

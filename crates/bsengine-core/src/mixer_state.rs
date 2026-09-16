//! What the editor's mixer panel shows, and what it asks the mixer to change.
//!
//! The panel lives in `bsengine-rhi-wgpu` and the buses live in
//! `bsengine-audio`, and neither crate depends on the other. They meet here,
//! in the crate both already depend on — the same shape the profiler panel uses
//! to reach frame stats it cannot query directly.
//!
//! The traffic runs both ways and is deliberately asymmetric. The audio side
//! publishes a *snapshot* each frame, because a panel that held a borrow of the
//! live mixer would be reading it mid-update. The panel pushes *requests*,
//! because a panel that wrote volumes directly would be changing the mixer from
//! whatever thread egui happens to run on.

use std::sync::{Arc, Mutex};

use bevy_ecs::prelude::Resource;

/// One bus, as the panel sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct MixerBus {
    /// The bus's name, as authored in `assets/audio/buses.ron`.
    pub name: String,
    /// The bus it feeds into, or `None` for the master bus.
    pub parent: Option<String>,
    /// Its current volume, in decibels.
    pub volume_db: f32,
    /// How many DSP effects sit on it.
    ///
    /// A count rather than the effects themselves: the panel shows that a bus
    /// *has* a chain without pretending to edit one, which is a larger feature
    /// than a volume slider and would need its own design.
    pub effects: usize,
}

/// Everything the panel and the mixer pass between them.
///
/// Empty `buses` is the ordinary state for a project with no `buses.ron`, not
/// an error — the panel says so rather than showing an empty grid that looks
/// like a failure.
#[derive(Debug, Clone, Default)]
pub struct MixerState {
    /// The live bus tree, republished by the audio side every frame.
    pub buses: Vec<MixerBus>,
    /// Volume changes the panel wants applied, in the order they were made.
    ///
    /// Drained by the audio side. A queue rather than a single value because a
    /// drag produces many changes per frame and the last one is the one that
    /// matters — but dropping the earlier ones here would mean the panel and
    /// the mixer disagree about what was asked for.
    pub pending_volumes: Vec<(String, f32)>,
}

/// Shared handle to [`MixerState`].
#[derive(Resource, Clone, Default)]
pub struct MixerShared(pub Arc<Mutex<MixerState>>);

impl MixerShared {
    /// Replaces the published bus list.
    pub fn publish(&self, buses: Vec<MixerBus>) {
        if let Ok(mut state) = self.0.lock() {
            state.buses = buses;
        }
    }

    /// The most recently published bus list.
    pub fn buses(&self) -> Vec<MixerBus> {
        self.0.lock().map(|s| s.buses.clone()).unwrap_or_default()
    }

    /// Queues a volume change for the audio side to apply.
    pub fn request_volume(&self, bus: &str, db: f32) {
        if !db.is_finite() {
            return;
        }
        if let Ok(mut state) = self.0.lock() {
            state.pending_volumes.push((bus.to_string(), db));
        }
    }

    /// Takes every queued volume change, leaving the queue empty.
    pub fn take_pending(&self) -> Vec<(String, f32)> {
        self.0
            .lock()
            .map(|mut s| std::mem::take(&mut s.pending_volumes))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_buses_are_what_a_reader_sees() {
        let shared = MixerShared::default();
        assert!(shared.buses().is_empty(), "nothing published yet");
        shared.publish(vec![MixerBus {
            name: "sfx".into(),
            parent: Some("master".into()),
            volume_db: -6.0,
            effects: 2,
        }]);
        let got = shared.buses();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].volume_db, -6.0);
        assert_eq!(got[0].parent.as_deref(), Some("master"));
    }

    #[test]
    fn taking_pending_changes_empties_the_queue() {
        // The audio side applies each request exactly once; a request left
        // behind would be re-applied every frame and fight a later edit.
        let shared = MixerShared::default();
        shared.request_volume("sfx", -3.0);
        shared.request_volume("music", -12.0);
        let first = shared.take_pending();
        assert_eq!(
            first.len(),
            2,
            "both requests come back in order: {first:?}"
        );
        assert_eq!(first[0], ("sfx".to_string(), -3.0));
        assert!(
            shared.take_pending().is_empty(),
            "a second take must find nothing left"
        );
    }

    #[test]
    fn every_change_in_a_drag_is_kept_in_order() {
        // A drag produces many changes in one frame. The last is what the user
        // means, but discarding the earlier ones here would leave the panel and
        // the mixer disagreeing about what was asked.
        let shared = MixerShared::default();
        for db in [-1.0, -2.0, -3.0] {
            shared.request_volume("sfx", db);
        }
        let got = shared.take_pending();
        assert_eq!(got.len(), 3);
        assert_eq!(got.last().unwrap().1, -3.0, "the last request wins");
    }

    #[test]
    fn a_nonsense_volume_is_refused_rather_than_queued() {
        // NaN reaching `set_bus_volume` would silently mute a bus with no error
        // anywhere.
        let shared = MixerShared::default();
        shared.request_volume("sfx", f32::NAN);
        shared.request_volume("sfx", f32::INFINITY);
        assert!(shared.take_pending().is_empty());
    }
}

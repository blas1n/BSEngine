//! What a client has told the server it did, and the server has not yet
//! confirmed.
//!
//! # Why a buffer is needed at all
//!
//! A predicted entity applies its own input immediately, so it is always some
//! way ahead of the last state the server confirmed. When a correction arrives
//! it is the answer to an *older* question — the server's view as of the last
//! input it had received. Snapping to it and stopping there would throw away
//! every input the player has made since, and the entity would visibly jump
//! backwards on every packet.
//!
//! So the client keeps those inputs, and on a correction re-applies them on top
//! of the authoritative state. When the prediction was right the replay lands
//! where the entity already was and nothing is visible, which is the normal case
//! and the reason this is worth doing.

use bevy_ecs::prelude::Resource;

/// Inputs sent but not yet acknowledged, oldest first.
#[derive(Resource, Default, Debug)]
pub struct PendingInputs {
    /// `(sequence, held key names)`.
    pending: Vec<(u32, Vec<String>)>,
    /// The sequence last handed out.
    latest: u32,
}

impl PendingInputs {
    /// Allocates the next input sequence.
    pub fn next_sequence(&mut self) -> u32 {
        self.latest = self.latest.wrapping_add(1);
        self.latest
    }

    /// Records what was sent under `sequence`.
    pub fn record(&mut self, sequence: u32, keys: Vec<String>) {
        self.pending.push((sequence, keys));
    }

    /// Forgets everything the server has confirmed.
    ///
    /// Idempotent, and that is not incidental: UDP delivers duplicates, so the
    /// same acknowledgement arrives more than once as a matter of course. A
    /// non-idempotent drop would discard live input the second time.
    pub fn retain_after(&mut self, acked: u32) {
        self.pending.retain(|(sequence, _)| *sequence > acked);
    }

    /// The inputs still awaiting confirmation, oldest first — exactly what a
    /// correction has to replay.
    pub fn unacknowledged(&self) -> &[(u32, Vec<String>)] {
        &self.pending
    }

    /// How many are outstanding. This is a latency measure, not a scene-size
    /// one, which is what bounds the cost of a replay.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Whether the server has confirmed everything sent.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(name: &str) -> Vec<String> {
        vec![name.to_string()]
    }

    #[test]
    fn sequences_are_handed_out_in_order() {
        let mut pending = PendingInputs::default();
        assert_eq!(pending.next_sequence(), 1);
        assert_eq!(pending.next_sequence(), 2);
    }

    /// Acking in the middle keeps exactly the later ones — the inputs a
    /// correction still has to replay.
    #[test]
    fn acking_the_middle_of_a_run_keeps_only_the_later_inputs() {
        let mut pending = PendingInputs::default();
        for (sequence, key) in [(1, "A"), (2, "B"), (3, "C"), (4, "D")] {
            pending.record(sequence, keys(key));
        }

        pending.retain_after(2);

        assert_eq!(
            pending
                .unacknowledged()
                .iter()
                .map(|(sequence, _)| *sequence)
                .collect::<Vec<_>>(),
            vec![3, 4],
            "everything the server has seen is forgotten, and nothing else is"
        );
    }

    /// UDP duplicates packets as a matter of course, so the same ack arrives
    /// twice. A non-idempotent drop would discard live input on the second one.
    #[test]
    fn acking_the_same_sequence_twice_discards_nothing_extra() {
        let mut pending = PendingInputs::default();
        pending.record(1, keys("A"));
        pending.record(2, keys("B"));

        pending.retain_after(1);
        let after_first = pending.len();
        pending.retain_after(1);

        assert_eq!(pending.len(), after_first, "the repeat must change nothing");
        assert_eq!(pending.unacknowledged()[0].0, 2);
    }

    /// An ack for something never sent (or long past) empties the buffer rather
    /// than leaving stale input to be replayed forever.
    #[test]
    fn acking_beyond_everything_empties_the_buffer() {
        let mut pending = PendingInputs::default();
        pending.record(1, keys("A"));
        pending.record(2, keys("B"));

        pending.retain_after(99);

        assert!(pending.is_empty());
    }

    /// An out-of-order ack for an *older* sequence must not resurrect anything
    /// or drop anything newer. UDP reorders, so this arrives in practice.
    #[test]
    fn a_late_ack_for_an_older_sequence_drops_nothing_newer() {
        let mut pending = PendingInputs::default();
        pending.record(1, keys("A"));
        pending.record(2, keys("B"));
        pending.record(3, keys("C"));

        pending.retain_after(2);
        pending.retain_after(1);

        assert_eq!(
            pending
                .unacknowledged()
                .iter()
                .map(|(sequence, _)| *sequence)
                .collect::<Vec<_>>(),
            vec![3],
            "the stale ack must not bring 2 back"
        );
    }
}

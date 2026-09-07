//! A deterministic stand-in for a bad network link.
//!
//! # Why this is an instrument, not a feature
//!
//! On a perfect link the interpolated position and the newest received snapshot
//! agree on every frame, so a completely broken interpolator passes any test run
//! without this. The same goes for loss handling: with nothing dropped there is
//! nothing to handle. This module is what makes the rest of item 56 observable,
//! which is why it is built before the things it measures.
//!
//! # Why the randomness is seeded
//!
//! Because the tests depend on it. An unseeded simulator would make every one of
//! them flaky, and a flaky instrument is worse than no instrument: it turns a
//! real regression and an unlucky roll into the same red, and the usual response
//! to that is to stop believing the test.

/// Decides which packets a simulated link delivers.
///
/// Reproducible from its seed: the same seed drops the same packets in the same
/// order, every run, on every machine.
pub struct LinkSimulator {
    loss: f32,
    state: u64,
}

impl LinkSimulator {
    /// Drops `loss` of packets (clamped to `0.0..=1.0`), reproducibly from
    /// `seed`.
    pub fn new(loss: f32, seed: u64) -> Self {
        Self {
            loss: loss.clamp(0.0, 1.0),
            // Mixed with a non-zero constant because a xorshift seeded with zero
            // stays zero forever — the default seed would otherwise produce a
            // constant sequence and quietly drop everything or nothing.
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    /// Whether the next packet gets through.
    pub fn should_deliver(&mut self) -> bool {
        // xorshift64*, written out rather than pulled in as a dependency: this
        // needs to be reproducible, not statistically excellent.
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        let roll =
            (self.state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f32 / (1u64 << 53) as f32;
        roll >= self.loss
    }
}

/// Packets held back so they arrive on a later frame.
#[derive(Default)]
pub struct DelayQueue {
    /// `(frames remaining, bytes)`.
    pending: Vec<(u32, Vec<u8>)>,
}

impl DelayQueue {
    /// Holds `packet` for `frames` before it becomes due.
    pub fn push(&mut self, packet: Vec<u8>, frames: u32) {
        self.pending.push((frames, packet));
    }

    /// Counts one frame down for everything held.
    pub fn advance(&mut self) {
        for (remaining, _) in &mut self.pending {
            *remaining = remaining.saturating_sub(1);
        }
    }

    /// Takes everything whose time has come, leaving the rest waiting.
    pub fn due(&mut self) -> Vec<Vec<u8>> {
        let (ready, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|(remaining, _)| *remaining == 0);
        self.pending = waiting;
        ready.into_iter().map(|(_, packet)| packet).collect()
    }

    /// How many packets are still held.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Whether nothing is held.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property every test below depends on, so it is asserted first.
    #[test]
    fn the_same_seed_drops_the_same_packets() {
        let run = || {
            let mut sim = LinkSimulator::new(0.5, 7);
            (0..20).map(|_| sim.should_deliver()).collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    /// Paired with the above, and not redundant: a *constant* sequence is also
    /// perfectly reproducible, and would drop everything or nothing while
    /// passing the determinism test.
    #[test]
    fn half_loss_drops_roughly_half() {
        let mut sim = LinkSimulator::new(0.5, 7);
        let delivered = (0..1000).filter(|_| sim.should_deliver()).count();
        assert!(
            (400..=600).contains(&delivered),
            "expected roughly half of 1000 delivered, got {delivered}"
        );
    }

    /// The two ends, because a simulator that ignored `loss` entirely would pass
    /// both tests above.
    #[test]
    fn zero_loss_delivers_everything_and_full_loss_delivers_nothing() {
        let mut none = LinkSimulator::new(0.0, 7);
        assert!(
            (0..50).all(|_| none.should_deliver()),
            "0.0 loss must deliver every packet"
        );
        let mut all = LinkSimulator::new(1.0, 7);
        assert!(
            (0..50).all(|_| !all.should_deliver()),
            "1.0 loss must deliver none"
        );
    }

    /// A different seed is a different sequence — otherwise the seed is decoration
    /// and every "run it under a different link" test measures the same link.
    #[test]
    fn a_different_seed_is_a_different_sequence() {
        let run = |seed| {
            let mut sim = LinkSimulator::new(0.5, seed);
            (0..40).map(|_| sim.should_deliver()).collect::<Vec<_>>()
        };
        assert_ne!(run(7), run(8));
    }

    /// Exactly N frames: not before, and not never.
    #[test]
    fn a_delayed_packet_arrives_after_exactly_that_many_frames() {
        let mut queue = DelayQueue::default();
        queue.push(b"hello".to_vec(), 2);

        assert!(queue.due().is_empty(), "not on the frame it was sent");
        queue.advance();
        assert!(queue.due().is_empty(), "not one frame later");
        queue.advance();
        assert_eq!(queue.due(), vec![b"hello".to_vec()], "on the second frame");
        assert!(queue.is_empty(), "and it is not delivered twice");
    }

    /// Zero delay is immediate, which is what an unconfigured session gets.
    #[test]
    fn a_packet_held_for_no_frames_is_due_at_once() {
        let mut queue = DelayQueue::default();
        queue.push(b"now".to_vec(), 0);
        assert_eq!(queue.due(), vec![b"now".to_vec()]);
    }

    /// Ordering is preserved among packets due on the same frame, so a test can
    /// assert on a sequence rather than a set.
    #[test]
    fn packets_due_together_keep_their_order() {
        let mut queue = DelayQueue::default();
        queue.push(b"first".to_vec(), 1);
        queue.push(b"second".to_vec(), 1);
        queue.advance();
        assert_eq!(queue.due(), vec![b"first".to_vec(), b"second".to_vec()]);
    }
}

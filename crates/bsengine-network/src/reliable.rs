//! Reliable, ordered delivery over the unreliable datagram link.
//!
//! # Why this exists at all
//!
//! Everything the engine sent before this was a snapshot or an input: losing one
//! is an ordinary event that the next one repairs, which is why transforms and
//! held keys are deliberately *not* reliable and must stay that way. An RPC is
//! the opposite kind of message. "The door opened" is not repaired by the next
//! packet, because there is no next packet — it is said once and then it is
//! either known or it is not.
//!
//! Unity's Netcode (over Unity Transport's reliable pipeline), Unreal's
//! NetDriver bunches and Godot's ENet channels all default their RPCs to
//! reliable and ordered. This is the same construction they use underneath:
//! sequence numbers, acknowledgements with a history bitfield, resend until
//! acknowledged, and a small reorder buffer so the receiver hands them up in the
//! order they were sent.
//!
//! # Why ordered and not merely reliable
//!
//! All three engines order their reliable traffic, and the reason shows up the
//! first time two RPCs are causally related: "spawn the crate" then "open the
//! crate" is not the same pair of messages in the other order. Delivering
//! out-of-order costs one buffer here and saves every caller from inventing its
//! own sequencing.

/// Wire tag for a payload carried reliably.
///
/// The payload is a whole inner packet, so anything the engine sends can be put
/// on this channel without the channel knowing what it is.
pub const MSG_RELIABLE: u8 = 0x07;
/// Wire tag for an acknowledgement.
pub const MSG_ACK: u8 = 0x08;

/// Bytes a reliable envelope adds in front of its payload: the tag and the
/// sequence number.
const ENVELOPE_LEN: usize = 1 + 4;

/// How many older sequence numbers an ack reports alongside the newest one.
///
/// A single ack per packet would make a *lost ack* indistinguishable from a lost
/// packet, and the sender would resend something the receiver already has. With
/// 32 of history, any later ack that gets through repairs the gap.
const ACK_HISTORY: u32 = 32;

/// How many unacknowledged packets a channel will hold before it starts
/// dropping the oldest.
///
/// A peer that has gone away acknowledges nothing, and without a cap its queue
/// grows for as long as the process runs. Dropping the oldest with a warning is
/// the honest failure: the alternative shipped today is an unbounded `Vec`, and
/// the alternative that looks tidiest -- dropping silently -- turns a dead peer
/// into a mystery.
const MAX_UNACKED: usize = 256;

/// One packet the sender is still waiting to hear about.
struct Unacked {
    seq: u32,
    bytes: Vec<u8>,
    frames_since_send: u32,
}

/// One end of a reliable, ordered stream to a single peer.
///
/// Both directions live in one struct because both are per-peer: a server keeps
/// one of these per client, a client keeps one for the server.
#[derive(Default)]
pub struct ReliableChannel {
    /// Sequence number the next queued payload will get.
    ///
    /// Starts at 1 so that 0 can mean "nothing has been acknowledged yet"
    /// without a separate flag.
    next_seq: u32,
    unacked: Vec<Unacked>,
    /// Sequence the receiver will deliver next; everything below it is done.
    next_expected: u32,
    /// Arrivals from the future, held until the gap before them fills.
    held: Vec<(u32, Vec<u8>)>,
    /// Highest sequence seen, and a bitfield of the [`ACK_HISTORY`] before it.
    highest_seen: u32,
    seen_bits: u32,
    /// Set when something arrived since the last ack was built.
    ack_due: bool,
}

impl ReliableChannel {
    /// Wraps `payload` in an envelope and records it for resending.
    ///
    /// Returns the bytes to put on the wire. The caller sends them; this type
    /// never touches a socket, which is what lets every rule below be tested
    /// without one.
    pub fn queue(&mut self, payload: &[u8]) -> Vec<u8> {
        self.next_seq = self.next_seq.wrapping_add(1);
        let seq = self.next_seq;
        let mut bytes = Vec::with_capacity(ENVELOPE_LEN + payload.len());
        bytes.push(MSG_RELIABLE);
        bytes.extend_from_slice(&seq.to_le_bytes());
        bytes.extend_from_slice(payload);

        if self.unacked.len() >= MAX_UNACKED {
            let dropped = self.unacked.remove(0);
            tracing::warn!(
                "[network] reliable queue full ({MAX_UNACKED}); dropping unacknowledged \
                 sequence {} -- the peer has not acknowledged anything in a long time",
                dropped.seq
            );
        }
        self.unacked.push(Unacked {
            seq,
            bytes: bytes.clone(),
            frames_since_send: 0,
        });
        bytes
    }

    /// Ages every unacknowledged packet by a frame and returns those due to be
    /// sent again.
    ///
    /// Age rather than wall-clock time, for the same reason the server stamps a
    /// tick rather than a timestamp: a test can state an exact frame, and two
    /// machines' clocks have nothing to do with each other.
    pub fn due_resends(&mut self, resend_after_frames: u32) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for entry in &mut self.unacked {
            entry.frames_since_send += 1;
            if entry.frames_since_send >= resend_after_frames {
                entry.frames_since_send = 0;
                out.push(entry.bytes.clone());
            }
        }
        out
    }

    /// Forgets everything the peer has confirmed.
    pub fn on_ack(&mut self, ack: u32, bits: u32) {
        self.unacked.retain(|entry| !acked(entry.seq, ack, bits));
    }

    /// Records an arrival and returns the payloads that are now deliverable, in
    /// the order they were sent.
    ///
    /// Returns empty for a duplicate or for anything that is not a reliable
    /// envelope, and the caller is expected to ack regardless -- a duplicate
    /// means an ack was lost, so answering it is exactly the repair.
    pub fn on_receive(&mut self, packet: &[u8]) -> Vec<Vec<u8>> {
        let Some(seq) = envelope_sequence(packet) else {
            return Vec::new();
        };
        let payload = packet[ENVELOPE_LEN..].to_vec();
        self.note_seen(seq);

        if self.next_expected == 0 {
            self.next_expected = 1;
        }
        if seq < self.next_expected {
            return Vec::new();
        }
        if seq > self.next_expected {
            if !self.held.iter().any(|(held, _)| *held == seq) {
                self.held.push((seq, payload));
            }
            return Vec::new();
        }

        let mut delivered = vec![payload];
        self.next_expected += 1;
        // A held arrival may complete a run of several.
        while let Some(index) = self
            .held
            .iter()
            .position(|(seq, _)| *seq == self.next_expected)
        {
            delivered.push(self.held.remove(index).1);
            self.next_expected += 1;
        }
        delivered
    }

    /// The acknowledgement to send back, or `None` if nothing has arrived since
    /// the last one.
    pub fn pending_ack(&mut self) -> Option<Vec<u8>> {
        if !self.ack_due {
            return None;
        }
        self.ack_due = false;
        let mut bytes = Vec::with_capacity(9);
        bytes.push(MSG_ACK);
        bytes.extend_from_slice(&self.highest_seen.to_le_bytes());
        bytes.extend_from_slice(&self.seen_bits.to_le_bytes());
        Some(bytes)
    }

    /// How many packets are still waiting to be acknowledged.
    ///
    /// Exposed because it is the only way to observe that acknowledgement does
    /// anything at all: a channel that never clears its queue and one that
    /// clears it correctly send exactly the same bytes on a perfect link.
    pub fn unacked_count(&self) -> usize {
        self.unacked.len()
    }

    fn note_seen(&mut self, seq: u32) {
        self.ack_due = true;
        if seq > self.highest_seen {
            let shift = seq - self.highest_seen;
            // The old newest becomes part of the history.
            self.seen_bits = if shift >= ACK_HISTORY {
                0
            } else if self.highest_seen == 0 {
                // Nothing has been seen yet, so there is no old newest to push
                // into the history. Setting the bit anyway would claim sequence
                // 0 arrived -- harmless only because sequences start at 1, which
                // is not a reason to record something untrue.
                0
            } else {
                (self.seen_bits << shift) | (1 << (shift - 1))
            };
            self.highest_seen = seq;
        } else {
            let back = self.highest_seen - seq;
            if (1..=ACK_HISTORY).contains(&back) {
                self.seen_bits |= 1 << (back - 1);
            }
        }
    }
}

/// The sequence number in a reliable envelope, or `None` if `packet` is not one.
pub fn envelope_sequence(packet: &[u8]) -> Option<u32> {
    if packet.first() != Some(&MSG_RELIABLE) || packet.len() < ENVELOPE_LEN {
        return None;
    }
    Some(u32::from_le_bytes(
        packet[1..5].try_into().expect("4 bytes"),
    ))
}

/// The `(ack, bits)` pair in an ack packet, or `None` if `packet` is not one.
pub fn decode_ack(packet: &[u8]) -> Option<(u32, u32)> {
    if packet.first() != Some(&MSG_ACK) || packet.len() < 9 {
        return None;
    }
    Some((
        u32::from_le_bytes(packet[1..5].try_into().expect("4 bytes")),
        u32::from_le_bytes(packet[5..9].try_into().expect("4 bytes")),
    ))
}

/// Whether `seq` is covered by an ack of `ack` with history `bits`.
fn acked(seq: u32, ack: u32, bits: u32) -> bool {
    if seq == ack {
        return true;
    }
    if seq > ack {
        return false;
    }
    let back = ack - seq;
    back <= ACK_HISTORY && bits & (1 << (back - 1)) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `sender`'s queued bytes into `receiver`, dropping the ones `lost`
    /// names, and returns everything delivered.
    fn deliver(
        receiver: &mut ReliableChannel,
        packets: &[Vec<u8>],
        lost: &[usize],
    ) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for (index, packet) in packets.iter().enumerate() {
            if lost.contains(&index) {
                continue;
            }
            out.extend(receiver.on_receive(packet));
        }
        out
    }

    #[test]
    fn a_payload_arrives_intact_and_in_order() {
        let mut send = ReliableChannel::default();
        let mut recv = ReliableChannel::default();
        let packets: Vec<Vec<u8>> = [b"one".as_slice(), b"two", b"three"]
            .iter()
            .map(|p| send.queue(p))
            .collect();

        let got = deliver(&mut recv, &packets, &[]);
        assert_eq!(
            got,
            vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
        );
    }

    #[test]
    fn an_arrival_from_the_future_waits_for_the_gap_to_fill() {
        // Distinct payloads deliberately: with equal ones a receiver that
        // delivered the wrong held packet would look correct.
        let mut send = ReliableChannel::default();
        let mut recv = ReliableChannel::default();
        let packets: Vec<Vec<u8>> = [b"one".as_slice(), b"two", b"three"]
            .iter()
            .map(|p| send.queue(p))
            .collect();

        // 2 and 3 arrive first; nothing may be delivered yet.
        assert!(recv.on_receive(&packets[1]).is_empty());
        assert!(
            recv.on_receive(&packets[2]).is_empty(),
            "delivering out of order would make \"spawn then open\" arrive as \
             \"open then spawn\""
        );
        // 1 arrives and completes the run.
        assert_eq!(
            recv.on_receive(&packets[0]),
            vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
        );
    }

    #[test]
    fn a_duplicate_is_delivered_once() {
        // A resend of something already delivered is the ordinary case, not an
        // error: it is what a lost ack looks like from the sender's side.
        let mut send = ReliableChannel::default();
        let mut recv = ReliableChannel::default();
        let packet = send.queue(b"once");

        assert_eq!(recv.on_receive(&packet), vec![b"once".to_vec()]);
        assert!(
            recv.on_receive(&packet).is_empty(),
            "a resend must not run the handler a second time"
        );
    }

    #[test]
    fn a_lost_packet_is_resent_and_then_arrives() {
        let mut send = ReliableChannel::default();
        let mut recv = ReliableChannel::default();
        let packets: Vec<Vec<u8>> = [b"one".as_slice(), b"two"]
            .iter()
            .map(|p| send.queue(p))
            .collect();

        // The first is lost outright.
        let got = deliver(&mut recv, &packets, &[0]);
        assert!(got.is_empty(), "nothing can be delivered past the gap");

        // Nothing acknowledged yet, so both are still pending. Four frames
        // later they are due again.
        let mut resent = Vec::new();
        for _ in 0..4 {
            resent = send.due_resends(4);
        }
        assert_eq!(resent.len(), 2, "both unacknowledged packets come back");

        let got = deliver(&mut recv, &resent, &[]);
        assert_eq!(
            got,
            vec![b"one".to_vec(), b"two".to_vec()],
            "the repair delivers both, in the order they were sent"
        );
    }

    #[test]
    fn an_acknowledged_packet_stops_being_resent() {
        let mut send = ReliableChannel::default();
        let mut recv = ReliableChannel::default();
        let first = send.queue(b"one");
        let second = send.queue(b"two");
        recv.on_receive(&first);
        recv.on_receive(&second);

        let ack = recv.pending_ack().expect("something arrived");
        let (seq, bits) = decode_ack(&ack).expect("an ack packet");
        send.on_ack(seq, bits);

        assert_eq!(send.unacked_count(), 0, "both were acknowledged");
        assert!(
            send.due_resends(1).is_empty(),
            "resending an acknowledged packet is pure waste on the wire"
        );
    }

    #[test]
    fn an_ack_covers_the_gaps_behind_it() {
        // The receiver got 1 and 3 but not 2. Its ack has to say so, or the
        // sender resends 1 and 3 as well and a lossy link gets worse the more
        // it loses.
        let mut send = ReliableChannel::default();
        let mut recv = ReliableChannel::default();
        let packets: Vec<Vec<u8>> = [b"one".as_slice(), b"two", b"three"]
            .iter()
            .map(|p| send.queue(p))
            .collect();

        recv.on_receive(&packets[0]);
        recv.on_receive(&packets[2]);
        let (ack, bits) = decode_ack(&recv.pending_ack().expect("arrivals")).expect("an ack");
        send.on_ack(ack, bits);

        assert_eq!(
            send.unacked_count(),
            1,
            "only the packet that really is missing should remain"
        );
        let resent = send.due_resends(1);
        assert_eq!(resent.len(), 1);
        assert_eq!(
            &resent[0][ENVELOPE_LEN..],
            b"two",
            "and it must be the missing one, not whichever happened to be first"
        );
    }

    #[test]
    fn an_ack_covers_a_late_arrival_that_filled_a_gap() {
        // 2 arrives, then 1 catches up. Both are in hand, so both have to be
        // acknowledged -- an ack that reports only the newest leaves the sender
        // resending a packet that was delivered, for as long as the connection
        // lasts.
        let mut send = ReliableChannel::default();
        let mut recv = ReliableChannel::default();
        let packets: Vec<Vec<u8>> = [b"one".as_slice(), b"two"]
            .iter()
            .map(|p| send.queue(p))
            .collect();

        recv.on_receive(&packets[1]);
        recv.on_receive(&packets[0]);
        let (ack, bits) = decode_ack(&recv.pending_ack().expect("arrivals")).expect("an ack");
        send.on_ack(ack, bits);

        assert_eq!(
            send.unacked_count(),
            0,
            "a packet that arrived late is still a packet that arrived"
        );
    }

    #[test]
    fn nothing_arriving_means_nothing_to_acknowledge() {
        let mut recv = ReliableChannel::default();
        assert!(
            recv.pending_ack().is_none(),
            "an ack per frame regardless would be a packet per frame per peer \
             saying nothing"
        );
        recv.on_receive(&ReliableChannel::default().queue(b"x"));
        assert!(recv.pending_ack().is_some());
        assert!(
            recv.pending_ack().is_none(),
            "and the same arrival must not be acknowledged forever"
        );
    }

    #[test]
    fn a_packet_that_is_not_an_envelope_is_ignored() {
        let mut recv = ReliableChannel::default();
        assert!(recv.on_receive(&[crate::packet::MSG_HELLO]).is_empty());
        assert!(recv.on_receive(&[]).is_empty());
        assert_eq!(envelope_sequence(&[MSG_RELIABLE, 1]), None, "too short");
    }

    #[test]
    fn a_peer_that_never_acknowledges_does_not_grow_the_queue_forever() {
        let mut send = ReliableChannel::default();
        for _ in 0..MAX_UNACKED + 10 {
            send.queue(b"x");
        }
        assert_eq!(
            send.unacked_count(),
            MAX_UNACKED,
            "a dead peer must not be an unbounded allocation"
        );
    }
}

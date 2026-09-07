use bsengine_core::Transform;
use glam::{Quat, Vec3};

pub const MSG_HELLO: u8 = 0x01;
pub const MSG_HELLO_ACK: u8 = 0x02;
pub const MSG_TRANSFORM_BATCH: u8 = 0x03;
pub const MSG_CLIENT_TRANSFORM: u8 = 0x04;
pub const MSG_DISCONNECT: u8 = 0x05;

/// 40-byte transform snapshot sent over the wire.
#[derive(Clone, Copy, Debug)]
pub struct TransformData {
    pub px: f32,
    pub py: f32,
    pub pz: f32,
    pub rx: f32,
    pub ry: f32,
    pub rz: f32,
    pub rw: f32,
    pub sx: f32,
    pub sy: f32,
    pub sz: f32,
}

impl TransformData {
    pub fn from_transform(t: &Transform) -> Self {
        Self {
            px: t.position.x,
            py: t.position.y,
            pz: t.position.z,
            rx: t.rotation.x,
            ry: t.rotation.y,
            rz: t.rotation.z,
            rw: t.rotation.w,
            sx: t.scale.x,
            sy: t.scale.y,
            sz: t.scale.z,
        }
    }

    pub fn to_transform(self) -> Transform {
        Transform {
            position: Vec3::new(self.px, self.py, self.pz).into(),
            rotation: Quat::from_xyzw(self.rx, self.ry, self.rz, self.rw)
                .normalize()
                .into(),
            scale: Vec3::new(self.sx, self.sy, self.sz).into(),
        }
    }

    pub fn to_bytes(self) -> [u8; 40] {
        let mut b = [0u8; 40];
        let fields = [
            self.px, self.py, self.pz, self.rx, self.ry, self.rz, self.rw, self.sx, self.sy,
            self.sz,
        ];
        for (i, f) in fields.iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&f.to_le_bytes());
        }
        b
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        let r = |i: usize| f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap_or([0; 4]));
        Self {
            px: r(0),
            py: r(1),
            pz: r(2),
            rx: r(3),
            ry: r(4),
            rz: r(5),
            rw: r(6),
            sx: r(7),
            sy: r(8),
            sz: r(9),
        }
    }
}

pub fn encode_hello() -> [u8; 1] {
    [MSG_HELLO]
}

pub fn encode_hello_ack(peer_id: u64) -> [u8; 9] {
    let mut b = [0u8; 9];
    b[0] = MSG_HELLO_ACK;
    b[1..9].copy_from_slice(&peer_id.to_le_bytes());
    b
}

#[allow(dead_code)]
pub fn encode_disconnect() -> [u8; 1] {
    [MSG_DISCONNECT]
}

/// Bytes before the first entry in a [`MSG_TRANSFORM_BATCH`]: the tag, the entry
/// count, and the server tick the snapshot was taken on.
pub const BATCH_HEADER_LEN: usize = 1 + 1 + 4;

/// The server simulation frame a batch was taken on, or `None` if the packet is
/// too short to hold one.
///
/// A tick rather than a wall-clock timestamp: two peers' clocks are unrelated,
/// and this engine already advances on a fixed timestep, so a tick means the
/// same thing in both processes and a test can state it exactly. Synchronising
/// clocks instead would be its own project.
pub fn decode_batch_tick(packet: &[u8]) -> Option<u32> {
    packet
        .get(2..6)
        .map(|b| u32::from_le_bytes(b.try_into().expect("4 bytes")))
}

/// Encode a batch of (net_id, transform) pairs taken on server tick `tick`.
/// Returns None if empty.
pub fn encode_transform_batch(tick: u32, entries: &[(u64, TransformData)]) -> Option<Vec<u8>> {
    if entries.is_empty() {
        return None;
    }
    let count = entries.len().min(255) as u8;
    let mut pkt = Vec::with_capacity(BATCH_HEADER_LEN + count as usize * 48);
    pkt.push(MSG_TRANSFORM_BATCH);
    pkt.push(count);
    pkt.extend_from_slice(&tick.to_le_bytes());
    for (net_id, td) in &entries[..count as usize] {
        pkt.extend_from_slice(&net_id.to_le_bytes());
        pkt.extend_from_slice(&td.to_bytes());
    }
    Some(pkt)
}

/// Encode a single client-authoritative transform update.
pub fn encode_client_transform(net_id: u64, td: TransformData) -> [u8; 49] {
    let mut b = [0u8; 49];
    b[0] = MSG_CLIENT_TRANSFORM;
    b[1..9].copy_from_slice(&net_id.to_le_bytes());
    b[9..49].copy_from_slice(&td.to_bytes());
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_data_roundtrip() {
        let t = Transform {
            position: Vec3::new(1.0, 2.0, 3.0).into(),
            rotation: Quat::from_xyzw(0.0, 0.0, 0.0, 1.0).into(),
            scale: Vec3::ONE.into(),
        };
        let td = TransformData::from_transform(&t);
        let bytes = td.to_bytes();
        let td2 = TransformData::from_bytes(&bytes);
        let t2 = td2.to_transform();
        assert!((t.position.0 - t2.position.0).length() < 1e-5);
        assert!((t.scale.0 - t2.scale.0).length() < 1e-5);
    }

    #[test]
    fn hello_ack_encodes_peer_id() {
        let b = encode_hello_ack(42u64);
        assert_eq!(b[0], MSG_HELLO_ACK);
        assert_eq!(u64::from_le_bytes(b[1..9].try_into().unwrap()), 42);
    }

    #[test]
    fn transform_batch_empty_returns_none() {
        assert!(encode_transform_batch(0, &[]).is_none());
    }

    /// Without a tick a snapshot cannot be placed on a timeline at all, so
    /// interpolation is impossible before this exists.
    #[test]
    fn transform_batch_carries_the_server_tick() {
        let td = TransformData::from_transform(&Transform::default());
        let pkt = encode_transform_batch(7, &[(1u64, td)]).expect("batch");

        assert_eq!(pkt[0], MSG_TRANSFORM_BATCH);
        assert_eq!(decode_batch_tick(&pkt), Some(7));
        assert_eq!(pkt.len(), BATCH_HEADER_LEN + 48);
    }

    #[test]
    fn a_truncated_batch_yields_no_tick_rather_than_a_wrong_one() {
        assert_eq!(decode_batch_tick(&[MSG_TRANSFORM_BATCH, 1]), None);
    }

    #[test]
    fn transform_batch_roundtrip_count() {
        let td = TransformData::from_transform(&Transform::default());
        let entries = vec![(1u64, td), (2u64, td)];
        let pkt = encode_transform_batch(0, &entries).unwrap();
        assert_eq!(pkt[0], MSG_TRANSFORM_BATCH);
        assert_eq!(pkt[1], 2);
        assert_eq!(pkt.len(), BATCH_HEADER_LEN + 2 * 48);
    }

    #[test]
    fn client_transform_encodes_net_id() {
        let td = TransformData::from_transform(&Transform::default());
        let b = encode_client_transform(99, td);
        assert_eq!(b[0], MSG_CLIENT_TRANSFORM);
        assert_eq!(u64::from_le_bytes(b[1..9].try_into().unwrap()), 99);
    }
}

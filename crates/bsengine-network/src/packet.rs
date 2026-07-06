use bsengine_core::Transform;
use glam::{Quat, Vec3};

pub const MSG_HELLO: u8 = 0x01;
pub const MSG_HELLO_ACK: u8 = 0x02;
pub const MSG_TRANSFORM_BATCH: u8 = 0x03;
pub const MSG_CLIENT_TRANSFORM: u8 = 0x04;
pub const MSG_DISCONNECT: u8 = 0x05;

/// 40-byte transform snapshot sent over the wire.
#[derive(Clone, Copy)]
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
            px: t.translation.x,
            py: t.translation.y,
            pz: t.translation.z,
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
            translation: Vec3::new(self.px, self.py, self.pz),
            rotation: Quat::from_xyzw(self.rx, self.ry, self.rz, self.rw).normalize(),
            scale: Vec3::new(self.sx, self.sy, self.sz),
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

/// Encode a batch of (net_id, transform) pairs. Returns None if empty.
pub fn encode_transform_batch(entries: &[(u64, TransformData)]) -> Option<Vec<u8>> {
    if entries.is_empty() {
        return None;
    }
    let count = entries.len().min(255) as u8;
    let mut pkt = Vec::with_capacity(2 + count as usize * 48);
    pkt.push(MSG_TRANSFORM_BATCH);
    pkt.push(count);
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
            translation: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::from_xyzw(0.0, 0.0, 0.0, 1.0),
            scale: Vec3::ONE,
        };
        let td = TransformData::from_transform(&t);
        let bytes = td.to_bytes();
        let td2 = TransformData::from_bytes(&bytes);
        let t2 = td2.to_transform();
        assert!((t.translation - t2.translation).length() < 1e-5);
        assert!((t.scale - t2.scale).length() < 1e-5);
    }

    #[test]
    fn hello_ack_encodes_peer_id() {
        let b = encode_hello_ack(42u64);
        assert_eq!(b[0], MSG_HELLO_ACK);
        assert_eq!(u64::from_le_bytes(b[1..9].try_into().unwrap()), 42);
    }

    #[test]
    fn transform_batch_empty_returns_none() {
        assert!(encode_transform_batch(&[]).is_none());
    }

    #[test]
    fn transform_batch_roundtrip_count() {
        let td = TransformData::from_transform(&Transform::default());
        let entries = vec![(1u64, td), (2u64, td)];
        let pkt = encode_transform_batch(&entries).unwrap();
        assert_eq!(pkt[0], MSG_TRANSFORM_BATCH);
        assert_eq!(pkt[1], 2);
        assert_eq!(pkt.len(), 2 + 2 * 48);
    }

    #[test]
    fn client_transform_encodes_net_id() {
        let td = TransformData::from_transform(&Transform::default());
        let b = encode_client_transform(99, td);
        assert_eq!(b[0], MSG_CLIENT_TRANSFORM);
        assert_eq!(u64::from_le_bytes(b[1..9].try_into().unwrap()), 99);
    }
}

//! The wire format for a remote procedure call.
//!
//! Names travel as strings, the way key names already do in
//! [`encode_client_input`](crate::packet::encode_client_input) and for the same
//! reason: an id table would be smaller but needs both peers to agree on it, and
//! that is a second thing to keep in step for a saving that does not matter at
//! these sizes. Godot sends a method name too; Unreal and Unity can use an index
//! because both ends are the same compiled binary, which is not true here.

use bsengine_core::RpcCall;

/// Wire tag for a call.
pub const MSG_RPC: u8 = 0x09;

/// The largest argument payload that will be sent.
///
/// A UDP datagram that exceeds the path MTU is fragmented by IP, and a single
/// lost fragment loses the whole datagram — so a large "reliable" call would be
/// markedly *less* likely to arrive than a small one, which is the opposite of
/// what the word promises. Refusing loudly beats promising delivery that the
/// transport cannot give.
pub const MAX_ARGS_LEN: usize = 1024;

/// Encodes a call, or `None` if it will not fit.
pub fn encode_rpc(call: &RpcCall) -> Option<Vec<u8>> {
    let name = call.name.as_bytes();
    let args = call.args.as_bytes();
    if name.is_empty() || name.len() > 255 {
        tracing::warn!(
            "[network] rpc name must be 1..=255 bytes, got {}; dropping",
            name.len()
        );
        return None;
    }
    if args.len() > MAX_ARGS_LEN {
        tracing::warn!(
            "[network] rpc '{}' has {} bytes of arguments, over the {MAX_ARGS_LEN} limit; \
             dropping",
            call.name,
            args.len()
        );
        return None;
    }
    let mut pkt = Vec::with_capacity(1 + 8 + 1 + name.len() + 2 + args.len());
    pkt.push(MSG_RPC);
    pkt.extend_from_slice(&call.net_id.to_le_bytes());
    pkt.push(name.len() as u8);
    pkt.extend_from_slice(name);
    pkt.extend_from_slice(&(args.len() as u16).to_le_bytes());
    pkt.extend_from_slice(args);
    Some(pkt)
}

/// Decodes a call, or `None` for anything malformed.
///
/// Partial reads are refused rather than patched up: a call with half its
/// arguments would run a handler with values nobody sent, which is worse than
/// not running it.
pub fn decode_rpc(packet: &[u8]) -> Option<RpcCall> {
    if packet.first() != Some(&MSG_RPC) {
        return None;
    }
    let net_id = u64::from_le_bytes(packet.get(1..9)?.try_into().ok()?);
    let name_len = *packet.get(9)? as usize;
    let name = std::str::from_utf8(packet.get(10..10 + name_len)?)
        .ok()?
        .to_string();
    if name.is_empty() {
        return None;
    }
    let at = 10 + name_len;
    let args_len = u16::from_le_bytes(packet.get(at..at + 2)?.try_into().ok()?) as usize;
    let args = std::str::from_utf8(packet.get(at + 2..at + 2 + args_len)?)
        .ok()?
        .to_string();
    Some(RpcCall { net_id, name, args })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_survives_the_round_trip() {
        let call = RpcCall {
            net_id: 0x0123_4567_89AB_CDEF,
            name: "onDoorOpened".into(),
            args: r#"{"door":3,"loud":true}"#.into(),
        };
        let packet = encode_rpc(&call).expect("encodes");
        assert_eq!(decode_rpc(&packet).as_ref(), Some(&call));
    }

    #[test]
    fn empty_arguments_are_fine() {
        // The common case: a call that only means "this happened".
        let call = RpcCall {
            net_id: 1,
            name: "ping".into(),
            args: String::new(),
        };
        let packet = encode_rpc(&call).expect("encodes");
        assert_eq!(decode_rpc(&packet), Some(call));
    }

    #[test]
    fn a_truncated_packet_decodes_to_nothing() {
        let call = RpcCall {
            net_id: 1,
            name: "onDoorOpened".into(),
            args: r#"{"door":3}"#.into(),
        };
        let packet = encode_rpc(&call).expect("encodes");
        for cut in 1..packet.len() {
            assert_eq!(
                decode_rpc(&packet[..cut]),
                None,
                "a {cut}-byte prefix must not decode into a call somebody never made"
            );
        }
    }

    #[test]
    fn arguments_over_the_limit_are_refused_rather_than_fragmented() {
        let call = RpcCall {
            net_id: 1,
            name: "big".into(),
            args: "x".repeat(MAX_ARGS_LEN + 1),
        };
        assert_eq!(
            encode_rpc(&call),
            None,
            "a call too big for a datagram cannot be delivered reliably, and \
             saying so beats pretending"
        );
        let at_limit = RpcCall {
            args: "x".repeat(MAX_ARGS_LEN),
            ..call
        };
        assert!(
            encode_rpc(&at_limit).is_some(),
            "and the limit itself has to be usable"
        );
    }

    #[test]
    fn a_nameless_call_is_refused() {
        let call = RpcCall {
            net_id: 1,
            name: String::new(),
            args: String::new(),
        };
        assert_eq!(encode_rpc(&call), None);
    }

    #[test]
    fn another_message_is_not_mistaken_for_a_call() {
        assert_eq!(decode_rpc(&[crate::packet::MSG_HELLO]), None);
        assert_eq!(decode_rpc(&[]), None);
    }
}

#![no_std]

const FNV_OFFSET_BASIS_32:  u32  = 0x811c9dc5;
const FNV_OFFSET_BASIS_64:  u64  = 0xcbf29ce484222325;
const FNV_OFFSET_BASIS_128: u128 = 0x6c62272e07bb014262b821756295c58d;

const FNV_PRIME_32:  u32  = 0x01000193;
const FNV_PRIME_64:  u64  = 0x00000100000001B3;
const FNV_PRIME_128: u128 = 0x0000000001000000000000000000013B;

macro_rules! fnv_hash_impl {
    ($typ:ty, $size:literal, $fn_name:ident, $str_fn_name:ident, $offset:expr, $prime:expr) => {
        #[doc = concat![
            "Computes ",
            stringify!($size),
            "-bits fnv1a hash of the given slice, or up-to limit if provided. ",
            "If limit is zero or exceeds slice length, slice length is used instead.",
        ]]
        pub const fn $fn_name(bytes: &[u8], limit: Option<usize>) -> $typ {
            let prime = $prime;

            let mut hash = $offset;
            let mut i = 0;
            let len = match limit {
                Some(v) if 0 < v && v <= bytes.len() => {
                    v
                },
                _ => {
                    bytes.len()
                }
            };

            while i < len {
                hash ^= bytes[i] as $typ;
                hash = hash.wrapping_mul(prime);
                i += 1;
            }
            hash
        }

        #[doc = concat![
            "Computes ",
            stringify!($size),
            "-bit fnv1a hash from a str."
        ]]
        #[inline(always)]
        pub const fn $str_fn_name(input: &str) -> $typ {
            $fn_name(input.as_bytes(), None)
        }
    }
}

fnv_hash_impl!{u32,  32,  fnv1a_hash_32,  fnv1a_hash_str_32,  FNV_OFFSET_BASIS_32,  FNV_PRIME_32}
fnv_hash_impl!{u64,  64,  fnv1a_hash_64,  fnv1a_hash_str_64,  FNV_OFFSET_BASIS_64,  FNV_PRIME_64}
fnv_hash_impl!{u128, 128, fnv1a_hash_128, fnv1a_hash_str_128, FNV_OFFSET_BASIS_128, FNV_PRIME_128}

/// Computes 32-bits fnv1a hash and XORs higher and lower 16-bits.
/// This results in a 16-bits hash value.
/// Up to limit if provided, otherwise slice length.
/// If limit is zero or exceeds slice length, slice length is used instead.
#[inline(always)]
pub const fn fnv1a_hash_16_xor(bytes: &[u8], limit: Option<usize>) -> u16 {
    let bytes = fnv1a_hash_32(bytes, limit).to_ne_bytes();
    let upper: u16 = u16::from_ne_bytes([bytes[0], bytes[1]]);
    let lower: u16 = u16::from_ne_bytes([bytes[2], bytes[3]]);
    upper ^ lower
}

/// Computes 16-bit fnv1a hash from a str using XOR folding.
#[inline(always)]
pub const fn fnv1a_hash_str_16_xor(input: &str) -> u16 {
    fnv1a_hash_16_xor(input.as_bytes(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hash<T: Eq + core::fmt::Debug>(hash_func: impl FnOnce(&str) -> T, source_str: &str, expected: T) {
        let hashed = hash_func(source_str);
        let bit_size = core::mem::size_of::<T>() * 8;
        assert_eq!(hashed, expected, "fnv1a-{bit_size} hash for {source_str}")
    }

    const FOOBAR: &str = "foobar";
    const FOOBAR_HASH_32:  u32  = 0xbf9cf968;
    const FOOBAR_HASH_64:  u64  = 0x85944171f73967e8;
    const FOOBAR_HASH_128: u128 = 0x343e1662793c64bf6f0d3597ba446f18;

    #[test]
    fn test_32() {
        test_hash(fnv1a_hash_str_32, FOOBAR, FOOBAR_HASH_32)
    }
    #[test]
    fn test_64() {
        test_hash(fnv1a_hash_str_64, FOOBAR, FOOBAR_HASH_64)
     }
    #[test]
    fn test_128() { 
        test_hash(fnv1a_hash_str_128, FOOBAR, FOOBAR_HASH_128)
    }
}
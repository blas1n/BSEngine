use lhash::{DigestFmt, Md5, hmac};

fn digest_to_hex(input: impl AsRef<[u8]>) -> String {
    DigestFmt(input).to_string()
}

#[test]
fn test_simple() {
    let tests = [
        ("", "d41d8cd98f00b204e9800998ecf8427e"),
        ("a", "0cc175b9c0f1b6a831c399e269772661"),
        ("abc", "900150983cd24fb0d6963f7d28e17f72"),
        ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
        ("abcdefghijklmnopqrstuvwxyz", "c3fcd3d76192e4007dfb496cca67e13b"),
        ("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789", "d174ab98d277d9f5a5611c2c9f419d9f"),
        ("12345678901234567890123456789012345678901234567890123456789012345678901234567890", "57edf4a22be3c955ac49da2e2107b67a"),
    ];

    let mut md5 = Md5::new();
    let mut chunked_md5 = Md5::new();
    for (data, ref expected) in tests.iter() {
        let data = data.as_bytes();

        let mut chunked_const = Md5::new();
        md5.update(data);
        for chunk in data.chunks(10) {
            chunked_md5.update(chunk);
            chunked_const = chunked_const.const_update(chunk);
        }

        let hash = digest_to_hex(md5.result());
        let const_hash = digest_to_hex(lhash::md5(data));
        let chunked_hash = digest_to_hex(chunked_md5.result());
        let const_chunked_hash = digest_to_hex(chunked_const.const_result());
        let const_hash_stateful = digest_to_hex(Md5::new().const_update(data).const_result());

        assert_eq!(const_hash.len(), hash.len());
        assert_eq!(hash, *expected);
        assert_eq!(hash, const_hash);
        assert_eq!(hash, chunked_hash);
        assert_eq!(hash, const_chunked_hash);
        assert_eq!(hash, const_hash_stateful);

        md5.reset();
        chunked_md5.reset();
    }
}

#[test]
fn test_hmac() {
    let tests: [(&'static [u8], &'static [u8], &'static str); 8] = [
        (b"", b"", "74e6f7298a9c2d168935f58c001bad88"),
        (b"key", b"The quick brown fox jumps over the lazy dog", "80070713463e7749b90c2dc24911e275"),
        (b"Jefe", b"what do ya want for nothing?", "750c783e6ab0b503eaa86e310a5db738"),

        (&[0xAA; 16], &[0xDD; 50], "56be34521d144c88dbb8c733f0e8b3f6"),

        (&[0x0B; 16], b"Hi There", "9294727a3638bb1c13f48ef8158bfc9d"),
        (&[0x0C; 16], b"Test With Truncation", "56461ef2342edc00f9bab995690efd4c"),
        (&[0xAA; 80], b"Test Using Larger Than Block-Size Key - Hash Key First", "6b1ab7fe4bd7bf8f0b62e6ce61b9d0cd"),
        (&[0xAA; 80], b"Test Using Larger Than Block-Size Key and Larger Than One Block-Size Data", "6f630fad67cda0ee1fb1f562db3aa53e"),
    ];

    for (key, data, ref expected) in tests.iter() {
        let hash = hmac::<Md5>(data, key);
        let hash = digest_to_hex(hash);

        assert_eq!(hash.len(), hash.len());
        assert_eq!(hash, *expected);
    }
}

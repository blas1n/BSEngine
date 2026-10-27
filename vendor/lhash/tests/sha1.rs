use lhash::{DigestFmt, Sha1, hmac};

fn digest_to_hex(input: impl AsRef<[u8]>) -> String {
    DigestFmt(input).to_string()
}

#[test]
fn test_simple() {
    let tests = [
        ("", "da39a3ee5e6b4b0d3255bfef95601890afd80709"),
        ("The quick brown fox jumps over the lazy dog", "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"),
        ("The quick brown fox jumps over the lazy cog", "de9f2c7fd25e1b3afad3e85a0bd17d9b100db4b3"),
        ("abc", "a9993e364706816aba3e25717850c26c9cd0d89d"),
        ("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq", "84983e441c3bd26ebaae4aa1f95129e5e54670f1"),
        ("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopqabcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq", "afc53a4ea20856f98e08dc6f3a5c9833137768ed"),
    ];

    let mut hasher = Sha1::new();
    let mut chunked = Sha1::new();
    for (data, ref expected) in tests.iter() {
        let data = data.as_bytes();

        let mut chunked_const = Sha1::new();
        hasher.update(data);
        for chunk in data.chunks(25) {
            chunked.update(chunk);
            chunked_const = chunked_const.const_update(chunk);
        }

        let hash = digest_to_hex(hasher.result());
        let chunked_hash = digest_to_hex(chunked.result());
        let const_hash = digest_to_hex(lhash::sha1(data));
        let const_chunked_hash = digest_to_hex(chunked_const.const_result());
        let const_hash_stateful = digest_to_hex(Sha1::new().const_update(data).const_result());

        assert_eq!(const_hash.len(), hash.len());
        assert_eq!(hash, *expected);
        assert_eq!(const_hash, hash);
        assert_eq!(hash, chunked_hash);
        assert_eq!(hash, const_chunked_hash);
        assert_eq!(hash, const_hash_stateful);

        hasher.reset();
        chunked.reset();
    }
}

#[test]
fn test_hmac() {
    let tests: [(&'static [u8], &'static [u8], &'static str); 8] = [
        (b"", b"", "fbdb1d1b18aa6c08324b7d64b71fb76370690e1d"),
        (b"key", b"The quick brown fox jumps over the lazy dog", "de7c9b85b8b78aa6bc8a7a36f70a90701c9db4d9"),

        (&[0x0B; 20], b"Hi There", "b617318655057264e28bc0b6fb378c8ef146be00"),
        (b"Jefe", b"what do ya want for nothing?", "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"),
        (&[0xAA; 20], &[0xDD; 50], "125d7342b9ac11cd91a39af48aa17b4f63f175d3"),
        (&[0x0C; 20], b"Test With Truncation", "4c1a03424b55e07fe7f27be1d58bb9324a9a5a04"),
        (&[0xAA; 80], b"Test Using Larger Than Block-Size Key - Hash Key First", "aa4ae5e15272d00e95705637ce8a3b55ed402112"),
        (&[0xAA; 80], b"Test Using Larger Than Block-Size Key and Larger Than One Block-Size Data", "e8e99d0f45237d786d6bbaa7965c7808bbff1a91"),
    ];

    for (key, data, ref expected) in tests.iter() {
        let hash = crate::hmac::<Sha1>(data, key);
        let hash = digest_to_hex(hash);

        assert_eq!(hash, *expected);
    }
}

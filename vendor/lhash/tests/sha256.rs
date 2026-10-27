use lhash::{DigestFmt, Sha256, hmac};

fn digest_to_hex(input: impl AsRef<[u8]>) -> String {
    DigestFmt(input).to_string()
}

#[test]
fn test_simple() {
    let tests = [
        ("", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
        ("abc", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
        ("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq", "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"),
        ("abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu", "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1"),
    ];

    let mut hasher = Sha256::new();
    let mut chunked = Sha256::new();
    for (data, ref expected) in tests.iter() {
        let data = data.as_bytes();

        let mut chunked_const = Sha256::new();
        hasher.update(data);
        for chunk in data.chunks(25) {
            chunked.update(chunk);
            chunked_const = chunked_const.const_update(chunk);
        }

        let hash = digest_to_hex(hasher.result());
        let chunked_hash = digest_to_hex(chunked.result());
        let const_hash = digest_to_hex(lhash::sha256(data));
        let const_chunked_hash = digest_to_hex(chunked_const.const_result());
        let const_hash_stateful = digest_to_hex(Sha256::new().const_update(data).const_result());

        assert_eq!(const_hash.len(), hash.len());
        assert_eq!(hash, *expected);
        assert_eq!(const_hash, *expected);
        assert_eq!(hash, chunked_hash);
        assert_eq!(hash, const_chunked_hash);
        assert_eq!(hash, const_hash_stateful);

        hasher.reset();
        chunked.reset();
    }
}

#[test]
fn test_hmac() {
    let tests: [(&'static [u8], &'static [u8], &'static str); 5] = [
        (&[0x0B; 20], b"Hi There", "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"),
        (b"Jefe", b"what do ya want for nothing?", "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"),
        (&[0xAA; 20], &[0xDD; 50], "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"),
        (&[0xAA; 131], b"Test Using Larger Than Block-Size Key - Hash Key First", "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"),
        (&[0xAA; 131], b"This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm.", "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2"),
    ];

    for (key, data, ref expected) in tests.iter() {
        let hash = crate::hmac::<Sha256>(data, key);
        let hash = digest_to_hex(hash);

        assert_eq!(hash, *expected);
    }
}

use lhash::{DigestFmt, Sha512, hmac};

fn digest_to_hex(input: impl AsRef<[u8]>) -> String {
    crate::DigestFmt(input).to_string()
}

#[test]
fn test_simple() {
    let tests = [
        ("", "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"),
        ("abc", "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"),
        ("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq", "204a8fc6dda82f0a0ced7beb8e08a41657c16ef468b228a8279be331a703c33596fd15c13b1b07f9aa1d3bea57789ca031ad85c7a71dd70354ec631238ca3445"),
        ("abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu", "8e959b75dae313da8cf4f72814fc143f8f7779c6eb9f7fa17299aeadb6889018501d289e4900f7e4331b99dec4b5433ac7d329eeb6dd26545e96e55b874be909"),
        ("abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstuabcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu", "b1179d83245119c98bd9b5f813a1df5594850c7afeebb4574ad6b3e0e6fcf700b3373ee3084170c1d33a4193d8bcf1dc3005decb5d75a6c2785056a3e7fed643"),
    ];

    let mut hasher = Sha512::new();
    let mut chunked = Sha512::new();
    for (data, ref expected) in tests.iter() {
        let data = data.as_bytes();

        let mut chunked_const = Sha512::new();
        hasher.update(data);
        for chunk in data.chunks(25) {
            chunked.update(chunk);
            chunked_const = chunked_const.const_update(chunk);
        }

        let hash = digest_to_hex(hasher.result());
        let chunked_hash = digest_to_hex(chunked.result());
        let const_hash = digest_to_hex(lhash::sha512(data));
        let const_chunked_hash = digest_to_hex(chunked_const.const_result());
        let const_hash_stateful = digest_to_hex(Sha512::new().const_update(data).const_result());

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
        (&[0x0B; 20], b"Hi There", "87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cdedaa833b7d6b8a702038b274eaea3f4e4be9d914eeb61f1702e696c203a126854"),
        (b"Jefe", b"what do ya want for nothing?", "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737"),
        (&[0xAA; 20], &[0xDD; 50], "fa73b0089d56a284efb0f0756c890be9b1b5dbdd8ee81a3655f83e33b2279d39bf3e848279a722c806b485a47e67c807b946a337bee8942674278859e13292fb"),
        (&[0xAA; 131], b"Test Using Larger Than Block-Size Key - Hash Key First", "80b24263c7c1a3ebb71493c1dd7be8b49b46d1f41b4aeec1121b013783f8f3526b56d037e05f2598bd0fd2215d6a1e5295e64f73f63f0aec8b915a985d786598"),
        (&[0xAA; 131], b"This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm.", "e37b6a775dc87dbaa4dfa9f96e5e3ffddebd71f8867289865df5a32d20cdc944b6022cac3c4982b10d5eeb55c3e4de15134676fb6de0446065c97440fa8c6a58"),
    ];

    for (key, data, ref expected) in tests.iter() {
        let hash = crate::hmac::<Sha512>(data, key);
        let hash = digest_to_hex(hash);

        assert_eq!(hash, *expected);
    }
}

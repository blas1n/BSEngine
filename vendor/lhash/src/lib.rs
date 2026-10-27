//!Minimal `no_std` hashing library
//!
//!## Provided Algorithms
//!
//!These are also names of features to be used to include algorithm code.
//!
//!- `sha1`
//!- `md5`
//!- `sha256`
//!- `sha512`
//!
//!It also includes generic hmac implementation

#![no_std]
#![warn(missing_docs)]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::style))]

mod fmt;
pub use fmt::DigestFmt;

///Hashing algorithm interface
pub trait Digest {
    ///Output type
    type OutputType: AsRef<[u8]> + AsMut<[u8]> + Copy;
    ///Block type
    type BlockType: AsRef<[u8]> + AsMut<[u8]> + Copy;

    ///Creates new instance.
    fn new() -> Self;
    ///Resets algorithm's state.
    fn reset(&mut self);
    ///Hashes input
    fn update(&mut self, input: &[u8]);
    ///Finalizes algorithm and returns output.
    fn result(&mut self) -> Self::OutputType;
}

///Represents key used to sign content in `hmac` algorithm.
///
///Comparing to `hmac` function it allows to pre-compute key and just sign input directly.
///
///`Digest` is only used in methods, making user to be responsible for using correct algorithm.
pub struct HmacKey<D: Digest> {
    key: D::BlockType,
}

impl<D: Digest> HmacKey<D> {
    ///Creates new hmac key, using provided secret.
    ///
    ///If `secret` size is above that of `Digest::BlockType` then it is hashed,
    ///reducing potential quality of hmac properties.
    pub fn new(secret: &[u8]) -> Self {
        let mut inner: D::BlockType = unsafe {
            core::mem::MaybeUninit::zeroed().assume_init()
        };
        let key = inner.as_mut();

        if secret.len() <= key.len() {
            key[..secret.len()].copy_from_slice(secret);
        } else {
        let mut algo = D::new();
            algo.update(secret);
            let hash = algo.result();
            let hash = hash.as_ref();
            key[..hash.len()].copy_from_slice(hash);
            algo.reset();
        }

        for byte in key.iter_mut() {
            *byte ^= 0x36;
        }

        Self {
            key: inner,
        }
    }

    ///Signs provided `input` with the key.
    pub fn sign(&self, input: &[u8]) -> D::OutputType {
        let mut key = self.key;
        let key = key.as_mut();

        //inner
        let mut algo = D::new();
        algo.update(key);
        algo.update(input);
        let inner_result = algo.result();
        algo.reset();

        //outer
        for byte in key.iter_mut() {
            *byte ^= 0x36 ^ 0x5C;
        }
        algo.update(key);
        algo.update(inner_result.as_ref());
        algo.result()
    }
}

///Creates HMAC using provided `Digest` algorithm.
///
///- `input` - Data to hash.
///- `secret` - Data to derive HMAC's key.
pub fn hmac<D: Digest>(input: &[u8], secret: &[u8]) -> D::OutputType {
    let key = HmacKey::<D>::new(secret);
    key.sign(input)
}

#[cfg(feature = "sha1")]
mod sha1;
#[cfg(feature = "sha1")]
pub use sha1::{sha1, Sha1};
#[cfg(feature = "md5")]
mod md5;
#[cfg(feature = "md5")]
pub use md5::{md5, Md5};

#[cfg(feature = "sha256")]
mod sha256;
#[cfg(feature = "sha256")]
pub use sha256::{sha256, Sha256};

#[cfg(feature = "sha512")]
mod sha512;
#[cfg(feature = "sha512")]
pub use sha512::{sha512, Sha512};

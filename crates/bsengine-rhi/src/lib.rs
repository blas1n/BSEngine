//! Abstract render hardware interface (RHI) for BSEngine.
//!
//! Defines the `RHI`/`RHIMesh`/`RHIShader`/`RHITexture` traits that a
//! concrete GPU backend implements — see `bsengine-rhi-wgpu` for the only
//! current implementation, built on `wgpu`.
#![warn(missing_docs)]

/// Core RHI trait definitions (`RHI`, `RHIMesh`, `RHIShader`, `RHITexture`).
pub mod traits;
pub use traits::{RHIMesh, RHIShader, RHITexture, RHI};

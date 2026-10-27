#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(
    html_logo_url = "https://bevy.org/assets/icon.png",
    html_favicon_url = "https://bevy.org/assets/icon.png"
)]

//! Provides a byte-identical implementation of [`mikktspace`] in entirely safe
//! and idiomatic Rust.
//! This is used to generate [`TangentSpace`] values for 3D geometry.
//! Like the original implementation, this crate has no dependencies.
//!
//! With _only_ the default features enabled, this should produce _identical_
//! results to [`mikktspace`] on x86 architectures.
//! Other architectures may produce differing results due to the original
//! implementation's reliance on undefined behavior.
//!
//! # Usage
//!
//! As a preliminary step for `no_std` users, you must provide an implementation
//! for [`Ops`].
//! When the `std` feature is enabled, one is provided and automatically selected
//! as the default.
//!
//! First, implement [`Geometry`] for your geometry.
//!
//! ```ignore
//! impl Geometry for MyGeometry { /* ... */ }
//! ```
//!
//! The interface is how this crate reads geometric information _and_ writes back
//! the generated tangent space information.
//!
//! Finally, use [`generate_tangents`] to calculate and write back all
//! [`TangentSpace`] values.
//!
//! # Description
//!
//! The code is designed to consistently generate the same tangent spaces, for a
//! given mesh, in any tool in which it is used.
//! This is done by performing an internal welding step and subsequently an
//! order-independent evaluation of tangent space for meshes consisting of
//! triangles and quads.
//! This means faces can be received in any order and the same is true for the
//! order of vertices of each face.
//! The generated result will not be affected by such reordering.
//! Additionally, whether degenerate (vertices or texture coordinates) primitives
//! are present or not will not affect the generated results either.
//!
//! Once tangent space calculation is done the vertices of degenerate primitives
//! will simply inherit tangent space from neighboring non degenerate primitives.
//! The analysis behind this implementation can be found in Morten S. Mikkelsen's
//! master's [thesis].
//!
//! Note that though the tangent spaces at the vertices are generated in an
//! order-independent way, by this implementation, the interpolated tangent space
//! is still affected by which diagonal is chosen to split each quad.
//! A sensible solution is to have your tools pipeline always split quads by the
//! shortest diagonal.
//! This choice is order-independent and works with mirroring.
//! If these have the same length then compare the diagonals defined by the
//! texture coordinates.
//! [XNormal], which is a tool for baking normal maps, allows you to write your
//! own tangent space plugin and also quad triangulator plugin.
//!
//! # Features
//!
//! ## `std` (default)
//!
//! Provides access to the standard library, allowing a default implementation
//! of [`Ops`] to be provided.
//! If you disable this feature, you will need to provide a type implementing
//! [`Ops`] as the `O` parameter in the [`Geometry`] trait.
//!
//! ```
//! # use bevy_mikktspace::{Geometry, Ops};
//! # struct MyOps;
//! # struct MyGeometry;
//! impl Ops for MyOps {
//!     fn sqrt(x: f32) -> f32 {
//!         unimplemented!()
//!     }
//!
//!     fn acos(x: f32) -> f32 {
//!         unimplemented!()
//!     }
//! }
//!
//! # #[cfg(any())]
//! impl Geometry<MyOps> for MyGeometry { /* ... */ }
//! ```
//!
//! A common backend for implementing [`Ops`] is [`libm`]:
//!
//! ```
//! # use bevy_mikktspace::Ops;
//! # struct LibmOps;
//! impl Ops for LibmOps {
//!     fn sqrt(x: f32) -> f32 {
//!         libm::sqrtf(x)
//!     }
//!
//!     fn acos(x: f32) -> f32 {
//!         libm::acos(x as f64) as f32
//!     }
//! }
//! ```
//!
//! Note that alternate backends _may_ break byte-compatibility with the original
//! C implementation.
//! It also should go without saying that improper implementations could give
//! entirely incorrect results.
//!
//! ## `corrected-edge-sorting`
//!
//! Fixes a known bug in the original C implementation which can affect the
//! generated values.
//! The bug can cause edges to be improperly sorted, leading neighboring faces
//! to be ungrouped.
//! If you do not need byte compatibility with the C implementation, it is
//! recommended to enable this feature for improved performance, compile times,
//! and correctness.
//!
//! ## `corrected-vertex-welding`
//!
//! Fixes a known bug in the original C implementation which can affect the
//! generated values.
//! This bug causes vertices to be combined in such a way that the lowest vertex
//! index isn't reliably selected.
//! If you do not need byte compatibility with the C implementation, it is
//! recommended to enable this feature for improved performance, compile times,
//! and correctness.
//!
//! # Copyright
//!
//! This code is a Rust reimplementation of <https://github.com/mmikk/MikkTSpace>.
//! The copyright notice below reflects that history, and should not be removed.
//!
//! > Copyright (C) 2011 by Morten S. Mikkelsen
//! >
//! > This software is provided 'as-is', without any express or implied
//! > warranty.  In no event will the authors be held liable for any damages
//! > arising from the use of this software.
//! >
//! > Permission is granted to anyone to use this software for any purpose,
//! > including commercial applications, and to alter it and redistribute it
//! > freely, subject to the following restrictions:
//! >
//! > 1. The origin of this software must not be misrepresented; you must not
//! >    claim that you wrote the original software. If you use this software
//! >    in a product, an acknowledgment in the product documentation would be
//! >    appreciated but is not required.
//! > 2. Altered source versions must be plainly marked as such, and must not be
//! >    misrepresented as being the original software.
//! > 3. This notice may not be removed or altered from any source distribution.
//!
//! The above notice will also be included in any derivative source files.
//!
//! # Advice
//!
//! To avoid visual errors (distortions/unwanted hard edges in lighting), when
//! using sampled normal maps, the normal map sampler must use the exact inverse
//! of the pixel shader transformation.
//! The most efficient transformation we can possibly do in the pixel shader is
//! achieved by using, directly, the "unnormalized" interpolated tangent, bitangent
//! and vertex normal: `vT`, `vB` and `vN`.
//!
//! ```c, ignore
//! // pixel shader (fast transform out)
//! vNout = normalize(vNt.x * vT + vNt.y * vB + vNt.z * vN);
//! ```
//!
//! where `vNt` is the tangent space normal.
//!
//! The normal map sampler must likewise use the interpolated and "unnormalized"
//! tangent, bitangent and vertex normal to be compliant with the pixel shader.
//!
//! ```c, ignore
//! // sampler does (exact inverse of pixel shader):
//! float3 row0 = cross(vB, vN);
//! float3 row1 = cross(vN, vT);
//! float3 row2 = cross(vT, vB);
//! float fSign = dot(vT, row0)<0 ? -1 : 1;
//! vNt = normalize(fSign * float3(dot(vNout,row0), dot(vNout,row1), dot(vNout,row2)));
//! ```
//!
//! where `vNout` is the sampled normal in some chosen 3D space.
//!
//! Should you choose to reconstruct the bitangent in the pixel shader instead of
//! the vertex shader, as explained earlier, then be sure to do this in the normal
//! map sampler also.
//!
//! Finally, beware of quad triangulations.
//! If the normal map sampler doesn't use the same triangulation of quads as your
//! renderer then problems will occur since the interpolated tangent spaces will
//! differ even though the vertex level tangent spaces match.
//! This can be solved either by triangulating before sampling/exporting or by
//! using the order-independent choice of diagonal for splitting quads suggested earlier.
//! However, this must be used both by the sampler and your tools/rendering pipeline.
//!
//! [`mikktspace`]: http://www.mikktspace.com/
//! [thesis]: https://web.archive.org/web/20250321012901/https://image.diku.dk/projects/media/morten.mikkelsen.08.pdf
//! [XNormal]: https://xnormal.net/
//! [`libm`]: https://docs.rs/libm

#![forbid(unsafe_code)]
#![no_std]

extern crate alloc;

mod math;
mod mikktspace;

#[cfg(feature = "std")]
mod std {
    extern crate std;

    /// Implements [`Ops`](crate::Ops) using the standard library.
    /// This is the recommended default when the `std` feature is enabled, as it
    /// it designed to identically match the results provided by the original
    /// mikktspace C library.
    pub struct StdOps;

    impl crate::Ops for StdOps {
        #[inline]
        fn sqrt(x: f32) -> f32 {
            x.sqrt()
        }

        #[inline]
        fn acos(x: f32) -> f32 {
            // Using f64::acos for added precision.
            // This is required to match the C implementation.
            (x as f64).acos() as f32
        }
    }
}

pub use math::Ops;

#[cfg(feature = "std")]
pub use std::StdOps;

/// Generates [`TangentSpace`]s for the provided geometry with the grouping
/// threshold disabled.
pub fn generate_tangents<I, O>(interface: &mut I) -> Result<(), GenerateTangentSpaceError>
where
    I: Geometry<O>,
    O: Ops,
{
    generate_tangents_with_threshold(interface, -1_f32)
}

/// Generates [`TangentSpace`]s for the provided geometry with a provided `threshold`
/// for vertex grouping.
///
/// Note that unlike the original C implementation, which accepted an _angular_ threshold,
/// this function accepts a _linear_ threshold.
/// The angular threshold can be converted into a linear one trivially using cosine.
///
/// ```ignore
/// let angular_threshold = 180_f32;
/// let linear_threshold = angular_threshold.to_radians().cos();
/// ```
///
/// Appropriate threshold values should be in the range `[-1..=1]`.
pub fn generate_tangents_with_threshold<I, O>(
    interface: &mut I,
    threshold: f32,
) -> Result<(), GenerateTangentSpaceError>
where
    I: Geometry<O>,
    O: Ops,
{
    mikktspace::generate_tangent_space_and_write(interface, threshold)
}

/// Provides an interface for reading vertex information from geometry, and writing
/// back out the calculated tangent space information.
///
/// Without the `std` feature, there is no default implementation for [`Ops`]
/// provided.
/// Instead, you must also provide a type implementing [`Ops`] using an alternative
/// math backend, such as [`libm`].
///
/// [`libm`]: https://docs.rs/libm
pub trait Geometry<
    #[cfg(not(feature = "std"))] O: Ops,
    #[cfg(feature = "std")] O: Ops = std::StdOps,
>
{
    /// Returns the number of faces on the mesh to be processed.
    /// This can include unsupported face types (e.g., not triangles or quads),
    /// but they will be ignored.
    fn num_faces(&self) -> usize;

    /// Returns the number of vertices on face number `face`.
    /// `face` is a number in the range `0..get_num_faces()`.
    fn num_vertices_of_face(&self, face: usize) -> usize;

    /// Returns the position of the referenced `face` of vertex number `vert`.
    /// `face` is a number in the range `0..get_num_faces()`.
    /// `vert` is in the range `0..=2` for triangles and `0..=3` for quads.
    fn position(&self, face: usize, vert: usize) -> [f32; 3];

    /// Returns the normal of the referenced `face` of vertex number `vert`.
    /// `face` is a number in the range `0..get_num_faces()`.
    /// `vert` is in the range `0..=2` for triangles and `0..=3` for quads.
    fn normal(&self, face: usize, vert: usize) -> [f32; 3];

    /// Returns the texture coordinate of the referenced `face` of vertex number `vert`.
    /// `face` is a number in the range `0..get_num_faces()`.
    /// `vert` is in the range `0..=2` for triangles and `0..=3` for quads.
    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2];

    /// This function is used to return tangent space results to the application.
    ///
    /// Note that unlike the original C implementation, tangent spaces are turned
    /// as an [`Option<TangentSpace>`].
    /// This serves two purposes:
    ///
    /// 1. [`TangentSpace`] encodes all calculated results for a particular vertex,
    ///    and provides convenient getters for simplified results.
    /// 2. An [`Option`] is provided as the internal algorithm _may not produce a
    ///    value for this vertex_.
    ///    This typically occurs in cases where a degenerate face has no neighbors
    ///    to borrow a value from.
    ///    Instead of silently returning a default value, this implementation
    ///    explicitly provides [`None`] and leaves it up to the user to instead
    ///    use the default value.
    fn set_tangent(&mut self, tangent_space: Option<TangentSpace>, face: usize, vert: usize);
}

/// Wraps the relevant results generated when calculating the tangent space for
/// a particular vertex on a particular face.
///
/// Typically, you will call [`tangent`](TangentSpace::tangent) to retrieve the
/// tangent value.
#[derive(Clone, Copy, PartialEq)]
pub struct TangentSpace {
    tangent: [f32; 3],
    bi_tangent: [f32; 3],
    mag_s: f32,
    mag_t: f32,
    is_orientation_preserving: bool,
}

impl Default for TangentSpace {
    fn default() -> Self {
        Self {
            tangent: [1., 0., 0.],
            bi_tangent: [0., 1., 0.],
            mag_s: 1.0,
            mag_t: 1.0,
            is_orientation_preserving: false,
        }
    }
}

impl TangentSpace {
    /// Returns the normalized tangent as an `[x, y, z]` array.
    #[inline]
    pub const fn tangent(&self) -> [f32; 3] {
        self.tangent
    }

    /// Returns the normalized bi-tangent as an `[x, y, z]` array.
    #[inline]
    pub const fn bi_tangent(&self) -> [f32; 3] {
        self.bi_tangent
    }

    /// Returns the magnitude of the tangent.
    #[inline]
    pub const fn tangent_magnitude(&self) -> f32 {
        self.mag_s
    }

    /// Returns the magnitude of the bi-tangent.
    #[inline]
    pub const fn bi_tangent_magnitude(&self) -> f32 {
        self.mag_t
    }

    /// Indicates if this generated tangent preserves the original orientation of
    /// the face.
    #[inline]
    pub const fn is_orientation_preserving(&self) -> bool {
        self.is_orientation_preserving
    }

    /// Returns an encoded summary of the tangent and bi-tangent as an `[x, y, z, w]`
    /// array.
    #[inline]
    pub const fn tangent_encoded(&self) -> [f32; 4] {
        let sign = if self.is_orientation_preserving {
            1.0
        } else {
            -1.0
        };
        [self.tangent[0], self.tangent[1], self.tangent[2], sign]
    }
}

/// Error returned when failing to generate tangent spaces for a geometry.
#[derive(Clone, PartialEq, Debug)]
// Reserving the right to introduce new error variants in the future.
#[non_exhaustive]
pub enum GenerateTangentSpaceError {}

impl core::fmt::Display for GenerateTangentSpaceError {
    fn fmt(&self, _f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unreachable!()
    }
}

impl core::error::Error for GenerateTangentSpaceError {}

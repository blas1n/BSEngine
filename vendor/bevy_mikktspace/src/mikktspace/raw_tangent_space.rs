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

use crate::{Ops, math::Vec3};

pub(super) struct RawTangentSpace<O: Ops> {
    /// Normalized first order face derivative.
    pub(super) s: Vec3<O>,
    /// Normalized first order face derivative.
    pub(super) t: Vec3<O>,
    /// Original magnitude of [`s`](RawTangentSpace::s).
    pub(super) s_magnitude: f32,
    /// Original magnitude of [`t`](RawTangentSpace::t).
    pub(super) t_magnitude: f32,
}

impl<O: Ops> PartialEq for RawTangentSpace<O> {
    fn eq(&self, other: &Self) -> bool {
        self.s == other.s
            && self.t == other.t
            && self.s_magnitude == other.s_magnitude
            && self.t_magnitude == other.t_magnitude
    }
}

impl<O: Ops> Copy for RawTangentSpace<O> {}

impl<O: Ops> Clone for RawTangentSpace<O> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<O: Ops> RawTangentSpace<O> {
    pub(super) const ZERO: Self = Self {
        s: Vec3::ZERO,
        t: Vec3::ZERO,
        s_magnitude: 0f32,
        t_magnitude: 0f32,
    };
}

impl<O: Ops> RawTangentSpace<O> {
    /// Combines two [`RawTangentSpace`] values using an algorithm _similar_ to
    /// averaging.
    /// This is not strictly a mean, as it combines vector magnitudes and directions
    /// separately.
    /// For example, if `lhs.s` and `rhs.t` are `a` and `-a` (for some arbitrary vector `a`),
    /// the expected mean would be `0`.
    /// However, this method will instead return `a` (as `(a + a) / 2 == a`).
    pub(super) fn combine(self, rhs: Self) -> Self {
        // this if is important. Due to floating point precision
        // averaging when ts0==ts1 will cause a slight difference
        // which results in tangent space splits later on
        if self == rhs {
            self
        } else {
            // TODO: Determine if `s_magnitude` and `t_magnitude` are being calculated incorrectly.
            //       This matches the C implementation, but may be incorrect.
            RawTangentSpace {
                s_magnitude: 0.5f32 * (self.s_magnitude + rhs.s_magnitude),
                t_magnitude: 0.5f32 * (self.t_magnitude + rhs.t_magnitude),
                s: (self.s + rhs.s).normalized_or_zero(),
                t: (self.t + rhs.t).normalized_or_zero(),
            }
        }
    }
}

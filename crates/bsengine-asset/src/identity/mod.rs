//! Stable asset identity: the [`AssetGuid`] type and the `.meta` sidecar that
//! stores it beside the asset.
//!
//! An asset's path is how a human finds it and a terrible way for a project to
//! refer to it: renaming `fox.glb` silently breaks every scene, script and
//! material that named it. The identity here is the fix — a value that is
//! minted once, travels with the asset in its sidecar, and does not change when
//! the file moves.

use std::fmt;
use std::str::FromStr;

/// What a scan found, and the lookups the rest of item 30 asks of it.
pub mod index;
/// Walking a project's `assets/` directory and giving every file that deserves
/// an identity a sidecar holding one.
pub mod scan;
/// The `.meta` sidecar file that pins an asset's identity next to the asset.
pub mod sidecar;

pub use index::AssetIndex;
pub use scan::scan;
pub use sidecar::{hash_file, sidecar_path, Sidecar, SidecarError, SIDECAR_EXTENSION};

/// A stable identity for one asset file, independent of where it lives.
///
/// Randomly generated once, when a scan first finds an asset with no sidecar,
/// and never derived from the file's contents: a content-derived id would
/// change every time an artist saved the file, breaking every reference —
/// catastrophic in an engine whose whole point this item is.
///
/// The `#[serde(transparent)]` is load-bearing. Without it `ron` writes a
/// newtype struct as `guid: ("0193…")`, parenthesised; the sidecar format is
/// specified as a bare string, and sub-item B puts the same value in scene RON
/// where the extra parentheses would be noise a human has to explain.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct AssetGuid(uuid::Uuid);

impl AssetGuid {
    /// Generates a fresh, random identity.
    //
    // No `Default`, deliberately, and the clippy lint that asks for one is
    // wrong here in both directions. Deriving it would hand out the nil UUID,
    // so every asset that was default-constructed rather than scanned would
    // share one "identity" — the exact collision this type exists to prevent.
    // Writing `Default::default() == new()` is no better: it makes any
    // `#[derive(Default)]` on a containing struct mint a real identity by
    // accident, silently and at a distance. Minting an identity is a decision,
    // so it is spelled out at every call site.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl fmt::Display for AssetGuid {
    /// Writes the canonical lowercase hyphenated form, which is what both the
    /// sidecar and (from sub-item B) scene RON store.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for AssetGuid {
    type Err = AssetGuidParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::parse_str(text)
            .map(Self)
            .map_err(|_| AssetGuidParseError(text.to_string()))
    }
}

/// The error [`AssetGuid::from_str`] returns for text that is not an identity.
///
/// Deliberately does not expose the underlying `uuid` error: which UUID crate
/// backs [`AssetGuid`] is an implementation detail, and a caller that matched
/// on it would freeze that choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetGuidParseError(String);

impl fmt::Display for AssetGuidParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a valid asset GUID: `{}`", self.0)
    }
}

impl std::error::Error for AssetGuidParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_guid_round_trips_through_its_text_form() {
        // Sub-item B stores the identity as scene-RON text, which reaches it
        // through Display/FromStr rather than through serde.
        let original = AssetGuid::new();
        let parsed: AssetGuid = original.to_string().parse().expect("parse");
        assert_eq!(parsed, original);
    }

    #[test]
    fn the_text_form_is_the_canonical_hyphenated_uuid() {
        let text = AssetGuid::new().to_string();
        assert_eq!(text.len(), 36, "expected hyphenated form, got `{text}`");
        assert_eq!(text.matches('-').count(), 4, "`{text}`");
        assert!(
            text.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
            "`{text}`"
        );
        assert_eq!(text.to_lowercase(), text, "`{text}`");
    }

    #[test]
    fn text_that_is_not_a_guid_is_an_error_not_a_panic() {
        // Hand-edited scene RON is the expected source of this.
        for bad in ["", "not-a-guid", "0193a7c1-8f2e-7c44-9d61"] {
            assert!(
                bad.parse::<AssetGuid>().is_err(),
                "`{bad}` should not parse"
            );
        }
    }

    #[test]
    fn the_parse_error_names_the_offending_text() {
        let err = "not-a-guid".parse::<AssetGuid>().unwrap_err();
        assert!(
            err.to_string().contains("not-a-guid"),
            "a developer staring at a broken scene file needs to be told which \
             spelling was rejected, got: {err}"
        );
    }
}

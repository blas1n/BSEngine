//! The project's HDR display output setting.

use bevy_ecs::prelude::Resource;

/// HDR display output, from `project.toml`'s `[render] hdr_output`,
/// `hdr_output_format`, `hdr_paper_white_nits` and `hdr_max_nits`: a project
/// setting, opt-in, as Unity's HDR Output and Unreal's
/// `r.HDR.EnableHDROutput` are.
///
/// On, the window's swapchain carries an HDR signal ([`HdrOutputFormat`]) and
/// the frame keeps its highlights up to `max_nits` instead of clipping them at
/// white. `paper_white_nits` is how bright the SDR range's white shows (and the
/// UI with it); the tonemap below scene-linear 1.0 is exactly the SDR one, so
/// mid-tones look as they do in SDR. A display or adapter that offers neither
/// signal, and a browser, render SDR whatever this says.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct HdrOutputSettings {
    /// Whether to ask for HDR output at all.
    pub enabled: bool,
    /// Which HDR signal to send the display.
    pub format: HdrOutputFormat,
    /// The brightness SDR white is shown at, in nits. 200 is the usual
    /// default (Unity's and Windows' own "SDR content brightness" sit near it).
    pub paper_white_nits: f32,
    /// The brightest the display can show, in nits: where the highlights roll
    /// off to. 1000 for a typical HDR10 monitor.
    pub max_nits: f32,
}

/// The HDR signal a window's swapchain carries, from `project.toml`'s
/// `[render] hdr_output_format` (`"auto"`, `"hdr10"` or `"scrgb"`).
///
/// - **HDR10**: a 10-bit swapchain holding BT.2020 primaries under the PQ
///   (SMPTE ST 2084) transfer function, absolute nits up to 10,000 -- what an
///   HDR TV takes, and what Unreal (`r.HDR.Display.OutputDevice` ST-2084) and
///   Unity HDRP (R10G10B10A2) send by default. Half the bandwidth of scRGB.
/// - **scRGB**: a 16-bit float swapchain holding linear BT.709, 1.0 = 80 nits
///   -- what Godot sends, and what Windows' compositor and Apple's EDR take
///   natively.
///
/// `Auto` is HDR10 where the display offers it and scRGB where it offers only
/// that; an explicit choice the display does not offer falls back to the
/// other, then to SDR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HdrOutputFormat {
    /// HDR10 where offered, otherwise scRGB.
    #[default]
    Auto,
    /// HDR10 (PQ, BT.2020) first.
    Hdr10,
    /// scRGB (linear BT.709, float) first.
    Scrgb,
}

/// [`HdrOutputSettings::paper_white_nits`]'s default.
pub const DEFAULT_PAPER_WHITE_NITS: f32 = 200.0;
/// [`HdrOutputSettings::max_nits`]'s default.
pub const DEFAULT_MAX_NITS: f32 = 1000.0;

impl Default for HdrOutputSettings {
    /// Off -- what every project that does not ask for HDR renders with.
    fn default() -> Self {
        Self {
            enabled: false,
            format: HdrOutputFormat::Auto,
            paper_white_nits: DEFAULT_PAPER_WHITE_NITS,
            max_nits: DEFAULT_MAX_NITS,
        }
    }
}

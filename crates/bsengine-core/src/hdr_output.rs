//! The project's HDR display output setting.

use bevy_ecs::prelude::Resource;

/// HDR display output, from `project.toml`'s `[render] hdr_output`,
/// `hdr_paper_white_nits` and `hdr_max_nits`: a project setting, opt-in, as
/// Unity's HDR Output and Unreal's `r.HDR.EnableHDROutput` are.
///
/// On, the window's swapchain is a 16-bit float one, which wgpu presents as
/// scRGB -- linear Rec. 709, 1.0 = 80 nits (Vulkan's extended sRGB, Metal's
/// EDR, DXGI's FP16 swapchain) -- and the frame keeps its highlights up to
/// `max_nits` instead of clipping them at white. `paper_white_nits` is how
/// bright the SDR range's white shows (and the UI with it); the tonemap below
/// scene-linear 1.0 is exactly the SDR one, so mid-tones look as they do in
/// SDR. A display or adapter with no float swapchain, and a browser, render SDR
/// whatever this says.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct HdrOutputSettings {
    /// Whether to ask for HDR output at all.
    pub enabled: bool,
    /// The brightness SDR white is shown at, in nits. 200 is the usual
    /// default (Unity's and Windows' own "SDR content brightness" sit near it).
    pub paper_white_nits: f32,
    /// The brightest the display can show, in nits: where the highlights roll
    /// off to. 1000 for a typical HDR10 monitor.
    pub max_nits: f32,
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
            paper_white_nits: DEFAULT_PAPER_WHITE_NITS,
            max_nits: DEFAULT_MAX_NITS,
        }
    }
}

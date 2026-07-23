//! Single source of truth for the editor's color palette and `egui::Visuals`.
//! Every panel/toolbar reads colors from here rather than hard-coding hex
//! values at each call site.

use egui::{Color32, Rounding, Stroke};

/// Base panel/window background.
pub const BG: Color32 = Color32::from_rgb(0x18, 0x1a, 0x1f);
/// Toolbar, headers, overlays.
pub const BG_RAISED: Color32 = Color32::from_rgb(0x20, 0x23, 0x2a);
/// Viewport background.
pub const BG_VIEWPORT: Color32 = Color32::from_rgb(0x20, 0x23, 0x29);
/// Toolbar group separators / widget borders.
pub const DIVIDER: Color32 = Color32::from_rgb(0x2c, 0x31, 0x3c);
/// Primary text and field values.
pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xea, 0xee);
/// Secondary text, field labels.
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0xb8, 0xbc, 0xc4);
/// Placeholder / disabled text.
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x6a, 0x6f, 0x7a);
/// Selection state, Play button, active tool — reserved for state, not decoration.
pub const ACCENT: Color32 = Color32::from_rgb(0xe0, 0x91, 0x3a);
/// Selected-row background wash (accent at ~13% alpha).
///
/// `Color32::from_rgba_unmultiplied` is not a `const fn` in this workspace's
/// pinned `ecolor` version, so this can't be built with the unmultiplied
/// constructor while staying a `const`. Instead the RGB channels are
/// pre-multiplied by hand (`component * alpha / 255`, rounded) so the
/// premultiplied-alpha invariant (r,g,b <= a) holds: this is accent orange
/// (0xe0, 0x91, 0x3a) blended to ~13% alpha (0x22), premultiplied to
/// (0x1e, 0x13, 0x08).
pub const ACCENT_WASH: Color32 = Color32::from_rgba_premultiplied(0x1e, 0x13, 0x08, 0x22);

/// Corner rounding shared by every widget-state block below.
const ROUNDING: Rounding = Rounding::same(5.0);

/// Builds the editor's `egui::Visuals` from the palette above, starting from
/// `Visuals::dark()` as a base so every field egui expects has a sane
/// default before the hybrid-theme overrides below.
pub fn build_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = BG_RAISED;
    visuals.extreme_bg_color = BG;
    visuals.faint_bg_color = BG_RAISED;
    visuals.selection.bg_fill = ACCENT_WASH;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);

    visuals.widgets.noninteractive.bg_fill = BG;
    visuals.widgets.noninteractive.weak_bg_fill = BG;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, DIVIDER);
    visuals.widgets.noninteractive.rounding = ROUNDING;

    visuals.widgets.inactive.bg_fill = BG_RAISED;
    visuals.widgets.inactive.weak_bg_fill = BG_RAISED;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, DIVIDER);
    visuals.widgets.inactive.rounding = ROUNDING;

    visuals.widgets.hovered.bg_fill = DIVIDER;
    visuals.widgets.hovered.weak_bg_fill = DIVIDER;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.hovered.rounding = ROUNDING;

    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.active.weak_bg_fill = ACCENT;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, BG);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.rounding = ROUNDING;

    visuals.widgets.open.bg_fill = DIVIDER;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.open.weak_bg_fill = DIVIDER;
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.open.rounding = ROUNDING;

    visuals
}

/// Registers the Phosphor icon font and applies the hybrid dark theme to
/// `ctx`. Call exactly once, right after the `egui::Context` is created.
pub fn apply(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
    ctx.set_visuals(build_visuals());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visuals_uses_the_hybrid_dark_palette() {
        let visuals = build_visuals();
        assert_eq!(visuals.panel_fill, BG);
        assert_eq!(visuals.selection.bg_fill, ACCENT_WASH);
        assert_eq!(visuals.selection.stroke.color, ACCENT);
        assert_eq!(visuals.widgets.open.bg_stroke.color, ACCENT);
        assert!(visuals.dark_mode);
    }

    #[test]
    fn accent_wash_matches_hand_premultiplied_accent() {
        let a = ACCENT_WASH.a() as u32;
        let round_div = |c: u8| (c as u32 * a + 127) / 255;
        assert_eq!(ACCENT_WASH.r() as u32, round_div(ACCENT.r()));
        assert_eq!(ACCENT_WASH.g() as u32, round_div(ACCENT.g()));
        assert_eq!(ACCENT_WASH.b() as u32, round_div(ACCENT.b()));
    }
}

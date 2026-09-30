//! Fast approximate antialiasing (FXAA) settings for a camera.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Fast approximate antialiasing: a single post pass over the finished frame
/// that finds high-contrast edges by their luma and blends across them.
///
/// The one antialiasing method all three reference engines ship: Unity URP's
/// FXAA, Unreal's FXAA method, Godot's `screen_space_aa`. It needs no history
/// and no extra samples, so unlike [`crate::Taa`] it has no ghosting and
/// no convergence time -- and unlike TAA it cannot recover detail smaller than
/// a pixel, only soften the stair-steps of an edge. The fields are the three
/// FXAA 3.11 itself exposes, with its defaults (the "default" quality edge
/// thresholds and a 0.75 subpixel amount), which is what Unity and Godot run
/// with and do not expose.
///
/// It runs on the tonemapped image, as all three engines do, so edges are
/// found by the contrast the player sees. With [`crate::Taa`] also on it runs
/// first, and TAA accumulates the smoothed frames.
///
/// Absent means off: a camera without this component renders exactly as it
/// did before.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct Fxaa {
    /// Whether the pass runs at all.
    pub enabled: bool,
    /// Local contrast an edge needs, as a fraction of the brightest luma in
    /// its neighbourhood. Lower finds more edges and costs more: FXAA 3.11
    /// suggests 0.333 (too little), 0.166 (default), 0.125 (high quality).
    pub edge_threshold: f32,
    /// Absolute contrast below which nothing is an edge, so noise in dark
    /// areas is left alone. FXAA 3.11's default is 0.0833.
    pub edge_threshold_min: f32,
    /// How much sub-pixel aliasing -- features thinner than a pixel -- is
    /// softened: 0 none, 1 softest. FXAA 3.11's default is 0.75.
    pub subpixel: f32,
}

impl Default for Fxaa {
    fn default() -> Self {
        Self {
            enabled: true,
            edge_threshold: 0.166,
            edge_threshold_min: 0.0833,
            subpixel: 0.75,
        }
    }
}

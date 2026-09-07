//! Tunables for replication, read from `project.toml`'s `[network]` table.
//!
//! # Why these are settings and not constants
//!
//! Each one is also a measurement instrument. An AOI radius a test can shrink is
//! what makes "the far entity stopped updating" checkable; an interpolation
//! delay a test can pin is what makes "the position is exactly a quarter of the
//! way between two snapshots" checkable; and a latency/loss simulator that can
//! be switched on is the only reason the other two are observable at all — on a
//! perfect link the interpolated position and the newest snapshot agree on every
//! frame, so a completely broken interpolator passes.
//!
//! This is the same reasoning that made occlusion culling a `[render]` setting
//! rather than an always-on behaviour: an off switch is also how you measure
//! what the thing does.

use bevy_ecs::prelude::Resource;

/// Replication tunables.
///
/// Every default reproduces the engine's pre-item-56 behaviour exactly: no
/// interest limit, no interpolation delay, no simulated latency or loss. A
/// project that says nothing about networking therefore behaves as it did.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct NetworkConfig {
    /// Radius around a peer's own entity within which the server sends it
    /// updates.
    ///
    /// `None` sends everything — what the engine did before AOI existed, and
    /// what a peer with no entity of its own gets, since there is no position to
    /// measure from and silently sending nothing would look exactly like a
    /// broken filter.
    pub aoi_radius: Option<f32>,
    /// How many server ticks behind the newest snapshot remote entities render.
    ///
    /// This is the cost of smoothness written as a number: remote entities are
    /// displayed in the past. `0` renders the newest snapshot the moment it
    /// lands, which is what the engine did before.
    pub interpolation_delay_ticks: u32,
    /// Frames to hold a packet before delivering it. `0` delivers immediately.
    pub simulated_latency_frames: u32,
    /// Fraction of packets to drop, clamped to `0.0..=1.0`.
    pub simulated_loss: f32,
    /// Seed for the drop sequence.
    ///
    /// Fixed rather than drawn from the clock **because tests depend on it**. An
    /// unseeded simulator would make every test that uses it flaky, and a flaky
    /// instrument is worse than none: it turns a real regression and an unlucky
    /// roll into the same red.
    pub simulator_seed: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            aoi_radius: None,
            interpolation_delay_ticks: 0,
            simulated_latency_frames: 0,
            simulated_loss: 0.0,
            simulator_seed: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property that lets an existing project upgrade without noticing.
    #[test]
    fn the_defaults_reproduce_pre_item_56_behaviour() {
        let config = NetworkConfig::default();
        assert_eq!(config.aoi_radius, None, "no interest limit");
        assert_eq!(
            config.interpolation_delay_ticks, 0,
            "the newest snapshot renders immediately"
        );
        assert_eq!(config.simulated_latency_frames, 0);
        assert_eq!(config.simulated_loss, 0.0, "the simulator is off");
    }
}

//! Gameplay-systems layer for BSEngine, built on `bevy_app`.
//!
//! Each module is a small, independent `Plugin` (e.g. `VelocityPlugin`,
//! `GravityPlugin`, `TweenPlugin`) that a game wires up via
//! `App::add_plugins`. Also re-exports the app-level schedule labels
//! (`Startup`, `PreUpdate`, `Update`, `PostUpdate`, `Last`) and the
//! `new_app()`/`BsPlugin` entry points.
#![warn(missing_docs)]

/// `AnimationPlugin`: advances `AnimationPlayer` clip time each frame.
pub mod animation_player;
/// `AnimationStateMachinePlugin`: evaluates ASM transitions and drives the `AnimationPlayer`.
pub mod animation_state_machine;
/// `new_app()`/`BsPlugin` entry points and re-exported `bevy_app` schedule labels.
pub mod app;
/// `FollowPlugin`: moves/orients entities toward a target entity (`Follow`, `LookAt`).
pub mod follow;
/// `LifetimePlugin`: despawns entities once their `Lifetime` expires.
pub mod lifetime;
/// `NavMeshPlugin`: paths and moves `NavMeshAgent` entities across a `NavMesh`.
pub mod nav_mesh;
/// `ShieldPlugin`: recharges `Shield` components over time.
pub mod shield;
/// `TimePlugin`: ticks the `Time` resource once per frame.
pub mod time;
/// `TimerPlugin`: ticks `Timer` components once per frame.
pub mod timer;
/// `TweenPlugin`: advances `Tween` components and applies them to `Transform`.
pub mod tween;

pub use animation_player::AnimationPlugin;
pub use animation_state_machine::AnimationStateMachinePlugin;
pub use app::{new_app, App, BsPlugin, Last, PostUpdate, PreUpdate, Startup, Update};
pub use follow::FollowPlugin;
pub use lifetime::LifetimePlugin;
pub use nav_mesh::NavMeshPlugin;
pub use shield::ShieldPlugin;
pub use time::TimePlugin;
pub use timer::TimerPlugin;
pub use tween::TweenPlugin;

mod tests;

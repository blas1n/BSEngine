//! Gameplay-systems layer for BSEngine, built on `bevy_app`.
//!
//! Each module is a small, independent `Plugin` (e.g. `VelocityPlugin`,
//! `GravityPlugin`, `TweenPlugin`) that a game wires up via
//! `App::add_plugins`. Also re-exports the app-level schedule labels
//! (`Startup`, `PreUpdate`, `Update`, `PostUpdate`, `Last`) and the
//! `new_app()`/`BsPlugin` entry points.
#![warn(missing_docs)]

pub mod angular_velocity;
pub mod animation_player;
pub mod animation_state_machine;
pub mod app;
pub mod damping;
pub mod external_impulse;
pub mod follow;
pub mod gravity;
pub mod lifetime;
pub mod nav_mesh;
pub mod shield;
pub mod time;
pub mod timer;
pub mod tween;
pub mod velocity;

pub use angular_velocity::AngularVelocityPlugin;
pub use animation_player::AnimationPlugin;
pub use animation_state_machine::AnimationStateMachinePlugin;
pub use app::{new_app, App, BsPlugin, Last, PostUpdate, PreUpdate, Startup, Update};
pub use damping::DampingPlugin;
pub use external_impulse::ExternalImpulsePlugin;
pub use follow::FollowPlugin;
pub use gravity::GravityPlugin;
pub use lifetime::LifetimePlugin;
pub use nav_mesh::NavMeshPlugin;
pub use shield::ShieldPlugin;
pub use time::TimePlugin;
pub use timer::TimerPlugin;
pub use tween::TweenPlugin;
pub use velocity::VelocityPlugin;

mod tests;

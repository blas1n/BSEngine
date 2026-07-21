//! Thin re-export of the `bevy_ecs` prelude used across BSEngine.
//!
//! No logic of its own — exists so the rest of the workspace depends on
//! one internal crate (`bsengine_ecs::{Query, Commands, Component, ...}`)
//! instead of `bevy_ecs` directly, keeping the ECS dependency version
//! centralized.
#![warn(missing_docs)]

pub use bevy_ecs::prelude::{
    Added, Bundle, Commands, Component, Entity, Event, EventReader, EventWriter, Query, Res,
    ResMut, Resource, With, Without, World,
};
pub use bevy_ecs::schedule::{
    IntoSystemConfigs, IntoSystemSetConfigs, Schedule, ScheduleLabel, SystemSet,
};
pub use bevy_ecs::system::IntoSystem;

mod tests;

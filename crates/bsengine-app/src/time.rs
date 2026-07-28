use bevy_app::prelude::*;
use bsengine_core::Time;
use bsengine_ecs::ResMut;

/// Inserts the `Time` resource and ticks it in `PreUpdate` so every other system
/// sees an up-to-date `delta_seconds` for the current frame.
pub struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::default());
        app.add_systems(PreUpdate, tick_time);
    }
}

fn tick_time(mut time: ResMut<Time>) {
    time.tick();
}

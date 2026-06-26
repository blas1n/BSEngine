use bevy_app::prelude::*;
use bsengine_core::Time;
use bsengine_ecs::ResMut;

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

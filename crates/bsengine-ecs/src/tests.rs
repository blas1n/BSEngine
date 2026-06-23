#[cfg(test)]
mod tests {
    use crate::{Component, Resource, Schedule, ScheduleLabel, World};

    #[derive(Component)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Resource)]
    struct Counter(u32);

    #[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
    struct TestSchedule;

    #[test]
    fn component_and_resource_derive_work() {
        let mut world = World::new();
        world.spawn(Position { x: 1.0, y: 2.0 });
        world.insert_resource(Counter(0));

        let counter = world.resource::<Counter>();
        assert_eq!(counter.0, 0);
    }

    #[test]
    fn schedule_can_be_created() {
        let _schedule = Schedule::new(TestSchedule);
    }

    #[test]
    fn query_can_iterate_components() {
        let mut world = World::new();
        world.spawn(Position { x: 1.0, y: 2.0 });
        world.spawn(Position { x: 3.0, y: 4.0 });

        let mut query = world.query::<&Position>();
        let count = query.iter(&world).count();
        assert_eq!(count, 2);
    }
}

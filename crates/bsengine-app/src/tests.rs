#[cfg(test)]
mod tests {
    use crate::{new_app, App, BsPlugin};

    struct MyPlugin;

    impl BsPlugin for MyPlugin {
        fn build(&self, _app: &mut App) {
            // Minimal plugin — proves the trait impl compiles and runs
        }
    }

    #[test]
    fn new_app_creates_app() {
        let _app = new_app();
    }

    #[test]
    fn plugin_can_be_added_to_app() {
        let mut app = new_app();
        app.add_plugins(MyPlugin);
    }

    #[test]
    fn time_plugin_inserts_time_resource() {
        use crate::TimePlugin;
        use bsengine_core::Time;

        let mut app = new_app();
        app.add_plugins(TimePlugin);
        app.update();

        let time = app.world().resource::<Time>();
        assert!(time.elapsed_seconds >= 0.0);
    }

    #[test]
    fn time_advances_each_frame() {
        use crate::TimePlugin;
        use bsengine_core::Time;

        let mut app = new_app();
        app.add_plugins(TimePlugin);

        app.update();
        let e1 = app.world().resource::<Time>().elapsed_seconds;
        app.update();
        let e2 = app.world().resource::<Time>().elapsed_seconds;

        assert!(e2 > e1, "elapsed_seconds should increase each frame");
    }
}

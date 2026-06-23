#[cfg(test)]
mod tests {
    use crate::{new_app, BsPlugin, App};

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
}

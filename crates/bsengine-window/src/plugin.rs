use crate::runner::winit_runner;
use crate::types::{WindowClosed, WindowCreated, WindowDescriptor, WindowResized};
use bevy_app::{App, Plugin};

#[derive(Default)]
pub struct WindowPlugin {
    pub descriptor: WindowDescriptor,
}


impl Plugin for WindowPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.descriptor.clone())
            .add_event::<WindowCreated>()
            .add_event::<WindowResized>()
            .add_event::<WindowClosed>()
            .set_runner(winit_runner);
    }
}

#[cfg(test)]
mod tests {
    use crate::{WindowClosed, WindowCreated, WindowDescriptor, WindowPlugin, WindowResized};
    use bevy_ecs::event::Events;
    use bsengine_app::new_app;

    #[test]
    fn window_plugin_registers_descriptor_resource() {
        let mut app = new_app();
        app.add_plugins(WindowPlugin::default());
        // WindowDescriptor should be inserted as a resource
        assert!(app.world().get_resource::<WindowDescriptor>().is_some());
    }

    #[test]
    fn window_plugin_registers_events() {
        let mut app = new_app();
        app.add_plugins(WindowPlugin::default());
        // Events<T> resources should exist after plugin build
        assert!(app
            .world()
            .get_resource::<Events<WindowCreated>>()
            .is_some());
        assert!(app
            .world()
            .get_resource::<Events<WindowResized>>()
            .is_some());
        assert!(app.world().get_resource::<Events<WindowClosed>>().is_some());
    }

    #[test]
    fn window_plugin_custom_descriptor() {
        let desc = WindowDescriptor {
            title: "Test".to_string(),
            width: 800,
            height: 600,
            resizable: false,
        };
        let mut app = new_app();
        app.add_plugins(WindowPlugin { descriptor: desc });
        let stored = app.world().resource::<WindowDescriptor>();
        assert_eq!(stored.title, "Test");
        assert_eq!(stored.width, 800);
    }
}

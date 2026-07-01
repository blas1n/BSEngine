use crate::runner::winit_runner;
use crate::types::{WindowClosed, WindowCreated, WindowDescriptor, WindowHandle, WindowResized};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bsengine_core::CursorConfig;
use winit::window::CursorGrabMode;

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
            .add_systems(Update, apply_cursor_config)
            .set_runner(winit_runner);
    }
}

fn apply_cursor_config(config: Option<Res<CursorConfig>>, handle: Option<Res<WindowHandle>>) {
    let (Some(config), Some(handle)) = (config, handle) else {
        return;
    };
    if !config.is_changed() {
        return;
    }
    handle.0.set_cursor_visible(config.visible);
    if config.locked {
        let _ = handle
            .0
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| handle.0.set_cursor_grab(CursorGrabMode::Confined));
    } else {
        let _ = handle.0.set_cursor_grab(CursorGrabMode::None);
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

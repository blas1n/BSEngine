use bevy_app::{App, AppExit};
use bevy_ecs::event::Events;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::types::{WindowClosed, WindowCreated, WindowDescriptor, WindowHandle, WindowResized};

struct BsWinitApp {
    ecs_app: App,
    window: Option<Arc<Window>>,
}

impl ApplicationHandler for BsWinitApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let desc = self
            .ecs_app
            .world()
            .get_resource::<WindowDescriptor>()
            .cloned()
            .unwrap_or_default();

        let attrs = winit::window::WindowAttributes::default()
            .with_title(desc.title)
            .with_inner_size(winit::dpi::LogicalSize::new(desc.width, desc.height))
            .with_resizable(desc.resizable);

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );
        self.window = Some(window.clone());
        self.ecs_app
            .world_mut()
            .insert_resource(WindowHandle(window.clone()));
        window.request_redraw();

        {
            let mut events = self
                .ecs_app
                .world_mut()
                .resource_mut::<Events<WindowCreated>>();
            events.send(WindowCreated);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                {
                    let mut events = self
                        .ecs_app
                        .world_mut()
                        .resource_mut::<Events<WindowClosed>>();
                    events.send(WindowClosed);
                }
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                let mut events = self
                    .ecs_app
                    .world_mut()
                    .resource_mut::<Events<WindowResized>>();
                events.send(WindowResized {
                    width: size.width,
                    height: size.height,
                });
            }
            WindowEvent::KeyboardInput { event, .. } => {
                use bevy_ecs::event::Events;
                use bsengine_input::convert::{convert_element_state, convert_key_code};
                use bsengine_input::KeyInput;
                let key_code = convert_key_code(event.physical_key);
                let state = convert_element_state(event.state);
                if let Some(mut events) = self
                    .ecs_app
                    .world_mut()
                    .get_resource_mut::<Events<KeyInput>>()
                {
                    events.send(KeyInput { key_code, state });
                }
            }
            WindowEvent::MouseInput { button, state, .. } => {
                use bevy_ecs::event::Events;
                use bsengine_input::convert::convert_element_state;
                use bsengine_input::{MouseButton, MouseInput};
                use winit::event::MouseButton as WinitBtn;
                let btn = match button {
                    WinitBtn::Left => MouseButton::Left,
                    WinitBtn::Right => MouseButton::Right,
                    WinitBtn::Middle => MouseButton::Middle,
                    WinitBtn::Other(n) => MouseButton::Other(n),
                    _ => return,
                };
                let state = convert_element_state(state);
                if let Some(mut events) = self
                    .ecs_app
                    .world_mut()
                    .get_resource_mut::<Events<MouseInput>>()
                {
                    events.send(MouseInput { button: btn, state });
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                use bevy_ecs::event::Events;
                use bsengine_input::CursorMoved;
                if let Some(mut events) = self
                    .ecs_app
                    .world_mut()
                    .get_resource_mut::<Events<CursorMoved>>()
                {
                    events.send(CursorMoved {
                        x: position.x,
                        y: position.y,
                    });
                }
            }
            WindowEvent::RedrawRequested => {
                self.ecs_app.update();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }
}

pub fn winit_runner(app: App) -> AppExit {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut winit_app = BsWinitApp {
        ecs_app: app,
        window: None,
    };

    event_loop
        .run_app(&mut winit_app)
        .expect("Event loop error");
    AppExit::Success
}

use bevy_app::{App, AppExit};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};
use std::sync::Arc;
use bevy_ecs::event::Events;

use crate::types::{WindowClosed, WindowCreated, WindowDescriptor, WindowResized};

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
            event_loop.create_window(attrs).expect("Failed to create window"),
        );
        self.window = Some(window.clone());
        window.request_redraw();

        {
            let mut events = self.ecs_app.world_mut().resource_mut::<Events<WindowCreated>>();
            events.send(WindowCreated);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                {
                    let mut events = self.ecs_app.world_mut().resource_mut::<Events<WindowClosed>>();
                    events.send(WindowClosed);
                }
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                let mut events = self.ecs_app.world_mut().resource_mut::<Events<WindowResized>>();
                events.send(WindowResized {
                    width: size.width,
                    height: size.height,
                });
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

    event_loop.run_app(&mut winit_app).expect("Event loop error");
    AppExit::Success
}

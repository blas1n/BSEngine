use bevy_app::{App, AppExit};
use bevy_ecs::message::Messages;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use bsengine_core::{CursorPos, ScreenSize};

use crate::types::{
    FirstFrameGate, SuspendAndResume, WindowClosed, WindowCreated, WindowDescriptor, WindowHandle,
    WindowResized,
};

struct BsWinitApp {
    ecs_app: App,
    window: Option<Arc<Window>>,
    /// Whether the first frame's gate (see [`FirstFrameGate`]) has opened.
    started: bool,
    /// The `AppExit` that ended the loop, which [`winit_runner`] returns: an
    /// error one (no GPU surface, say) must reach the process's exit status.
    exit: AppExit,
    /// Whether the app is in the background: between winit's `suspended`
    /// and the `resumed` that brings it back. No frame runs then -- there is
    /// no surface to draw into, and a game in the background is paused, as
    /// Unity, Unreal and Godot pause theirs.
    suspended: bool,
    /// Whether the safe area must be read again: set when the window appears
    /// or is resized (a rotation moves the bars), cleared once the system
    /// answers. Android only reports insets once the window's view is
    /// attached, which can be a few frames after the window exists.
    #[cfg(target_os = "android")]
    safe_area_pending: bool,
}

impl BsWinitApp {
    /// Reads the safe area if it is pending, into [`bsengine_core::SafeArea`]
    /// -- the insets only: whether UI follows them is the project's setting,
    /// which this keeps.
    #[cfg(target_os = "android")]
    fn refresh_safe_area(&mut self) {
        if !self.safe_area_pending {
            return;
        }
        let Some((left, top, right, bottom)) = crate::android::safe_area_insets() else {
            return;
        };
        self.safe_area_pending = false;
        if let Some(mut area) = self
            .ecs_app
            .world_mut()
            .get_resource_mut::<bsengine_core::SafeArea>()
        {
            (area.left, area.top, area.right, area.bottom) = (left, top, right, bottom);
            tracing::info!(
                "safe area: insets left {left}, top {top}, right {right}, bottom {bottom}"
            );
        }
    }
}

impl BsWinitApp {
    /// Runs the [`bsengine_core::AppLifecycleHooks`] hooks for leaving the screen
    /// (`resume == false`) or coming back.
    fn run_lifecycle_hooks(&mut self, resume: bool) {
        let hooks = self
            .ecs_app
            .world()
            .get_resource::<bsengine_core::AppLifecycleHooks>()
            .map(|l| {
                if resume {
                    l.on_resume.clone()
                } else {
                    l.on_suspend.clone()
                }
            })
            .unwrap_or_default();
        for hook in hooks {
            hook(self.ecs_app.world_mut());
        }
    }

    /// The app has left the screen. Lets go of the window's surface before
    /// returning (see [`bsengine_core::AppLifecycleHooks`]) and stops running frames.
    fn go_to_background(&mut self) {
        if self.window.is_none() || self.suspended {
            return;
        }
        self.suspended = true;
        self.run_lifecycle_hooks(false);
        tracing::info!("in the background: the window's surface is released, frames stop");
    }

    /// The app is back on screen with the window it had: makes the surface
    /// again and runs frames from where they stopped.
    fn come_back(&mut self) {
        if !self.suspended {
            return;
        }
        self.suspended = false;
        // The time spent away is not a frame: without this the first frame
        // back would step every timer, tween and animation by however long
        // the app sat in the background. Unity's `Time.time` and Godot's
        // process delta skip a pause the same way.
        if let Some(mut time) = self
            .ecs_app
            .world_mut()
            .get_resource_mut::<bsengine_core::Time>()
        {
            time.resume_after_pause();
        }
        self.run_lifecycle_hooks(true);
        // A rotation in the background moves the bars too.
        #[cfg(target_os = "android")]
        {
            self.safe_area_pending = true;
        }
        tracing::info!("back from the background: the window has a surface again");
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Whether frames may run: always once started, and before that only
    /// when every [`FirstFrameGate`] check passes.
    fn may_update(&mut self) -> bool {
        if self.started {
            return true;
        }
        let open = self
            .ecs_app
            .world()
            .get_non_send::<FirstFrameGate>()
            .is_none_or(|gate| gate.ready.iter().all(|ready| ready()));
        self.started = open;
        open
    }
}

impl ApplicationHandler for BsWinitApp {
    /// On iOS, where the next frame is asked for: once the loop has handled
    /// what came in, as winit's own examples and Bevy do there. The
    /// `request_redraw` at the end of `RedrawRequested` -- what keeps every
    /// other platform going -- is made while winit's iOS loop is processing
    /// redraws, and does not bring another: the first Simulator run drew a
    /// handful of frames and then nothing, its log silent until the watchdog.
    #[cfg(target_os = "ios")]
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.suspended {
            return;
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Android sends this again each time the app returns from the
        // background, with the window it had and a new native surface under
        // it; desktop window systems send it once.
        if self.window.is_some() {
            self.come_back();
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
        // In a browser the window is a canvas, appended to the page's body.
        #[cfg(target_arch = "wasm32")]
        let attrs = {
            use winit::platform::web::WindowAttributesExtWebSys;
            attrs.with_append(true)
        };

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );
        self.window = Some(window.clone());
        #[cfg(target_os = "android")]
        {
            self.safe_area_pending = true;
        }
        if let Some(mut gate) = self
            .ecs_app
            .world_mut()
            .get_non_send_mut::<FirstFrameGate>()
        {
            for hook in std::mem::take(&mut gate.on_window) {
                hook(window.clone());
            }
        }
        self.ecs_app
            .world_mut()
            .insert_resource(WindowHandle(window.clone()));
        self.ecs_app.world_mut().insert_resource(ScreenSize {
            width: desc.width,
            height: desc.height,
        });
        window.request_redraw();

        {
            let mut events = self
                .ecs_app
                .world_mut()
                .resource_mut::<Messages<WindowCreated>>();
            events.write(WindowCreated);
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.go_to_background();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                {
                    let mut events = self
                        .ecs_app
                        .world_mut()
                        .resource_mut::<Messages<WindowClosed>>();
                    events.write(WindowClosed);
                }
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                {
                    let mut events = self
                        .ecs_app
                        .world_mut()
                        .resource_mut::<Messages<WindowResized>>();
                    events.write(WindowResized {
                        width: size.width,
                        height: size.height,
                    });
                }
                if let Some(mut ss) = self.ecs_app.world_mut().get_resource_mut::<ScreenSize>() {
                    ss.width = size.width;
                    ss.height = size.height;
                }
                #[cfg(target_os = "android")]
                {
                    self.safe_area_pending = true;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                use bevy_ecs::message::Messages;
                use bsengine_input::convert::convert_element_state;
                #[cfg(not(target_os = "android"))]
                use bsengine_input::convert::convert_key_code;
                use bsengine_input::KeyInput;
                // Android's Back button is a key with no physical code: see
                // `convert_key_code_with_back`.
                #[cfg(target_os = "android")]
                let key_code = bsengine_input::convert::convert_key_code_with_back(
                    event.physical_key,
                    &event.logical_key,
                );
                #[cfg(not(target_os = "android"))]
                let key_code = convert_key_code(event.physical_key);
                let state = convert_element_state(event.state);
                let text = event.text.map(|t| t.to_string());
                if let Some(mut events) = self
                    .ecs_app
                    .world_mut()
                    .get_resource_mut::<Messages<KeyInput>>()
                {
                    events.write(KeyInput {
                        key_code,
                        state,
                        text,
                    });
                }
            }
            WindowEvent::MouseInput { button, state, .. } => {
                use bevy_ecs::message::Messages;
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
                    .get_resource_mut::<Messages<MouseInput>>()
                {
                    events.write(MouseInput { button: btn, state });
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                use bevy_ecs::message::Messages;
                use bsengine_input::CursorMoved;
                if let Some(mut events) = self
                    .ecs_app
                    .world_mut()
                    .get_resource_mut::<Messages<CursorMoved>>()
                {
                    events.write(CursorMoved {
                        x: position.x,
                        y: position.y,
                    });
                }
                if let Some(mut cp) = self.ecs_app.world_mut().get_resource_mut::<CursorPos>() {
                    cp.x = position.x as f32;
                    cp.y = position.y as f32;
                }
            }
            WindowEvent::Touch(touch) => {
                use bsengine_input::convert::convert_touch;
                use bsengine_input::TouchInput;
                if let Some(mut events) = self
                    .ecs_app
                    .world_mut()
                    .get_resource_mut::<Messages<TouchInput>>()
                {
                    events.write(convert_touch(&touch));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                use bevy_ecs::message::Messages;
                use bsengine_input::MouseWheel;
                let scroll = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y as f64,
                    winit::event::MouseScrollDelta::PixelDelta(d) => d.y / 40.0,
                };
                if let Some(mut events) = self
                    .ecs_app
                    .world_mut()
                    .get_resource_mut::<Messages<MouseWheel>>()
                {
                    events.write(MouseWheel { delta: scroll });
                }
            }
            WindowEvent::RedrawRequested => {
                // Android stops asking for redraws once the activity pauses,
                // but resumes them before the window is back (`Resume`
                // comes before `InitWindow`): such a frame would find no
                // surface. Nothing asks for another until `come_back`.
                if self.suspended {
                    return;
                }
                if !self.may_update() {
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                #[cfg(target_os = "android")]
                self.refresh_safe_area();
                self.ecs_app.update();

                // Both of the messages' buffers, as Bevy's own runners read
                // them, an error winning over a success beside it. Not only
                // this update's: an exit written in `Startup` -- a window
                // whose GPU surface could not be made -- has already been
                // swapped into the previous buffer by `First` of the same
                // update, and reading the current one alone let that game run
                // on, black, to a status of 0.
                if let Some(exit) = self.ecs_app.should_exit() {
                    self.exit = exit;
                    event_loop.exit();
                    return;
                }

                // `--suspend-after`: through the background and back, by the
                // same two calls winit makes on Android.
                if self
                    .ecs_app
                    .world_mut()
                    .remove_resource::<SuspendAndResume>()
                    .is_some()
                {
                    self.suspended(event_loop);
                    self.resumed(event_loop);
                }

                // Not on iOS: there `about_to_wait` asks for the next frame,
                // and asking here, while winit is processing redraws, only
                // made it warn "processing non `RedrawRequested` event after
                // the main event loop" once a frame.
                #[cfg(not(target_os = "ios"))]
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            use bsengine_input::MouseMotion;
            if let Some(mut events) = self
                .ecs_app
                .world_mut()
                .get_resource_mut::<Messages<MouseMotion>>()
            {
                events.write(MouseMotion { dx, dy });
            }
        }
    }
}

/// Bevy `App` runner that drives a `winit` event loop, translating window and
/// device events into ECS events and calling `app.update()` on each redraw.
pub fn winit_runner(app: App) -> AppExit {
    #[cfg(not(target_os = "android"))]
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    // On Android the loop belongs to the activity, which winit reaches
    // through the `AndroidApp` its entry point was given.
    #[cfg(target_os = "android")]
    let event_loop = {
        use winit::platform::android::EventLoopBuilderExtAndroid;
        EventLoop::builder()
            .with_android_app(crate::android::app())
            .build()
            .expect("Failed to create event loop")
    };
    // On iOS frames come from the display: winit drives `RedrawRequested`
    // from a CADisplayLink, and the `request_redraw` at the end of each frame
    // (`RedrawRequested` above) asks for the next one, so the game runs at the
    // screen's rate with `Wait` as on every other platform with `Poll`. `Poll`
    // there made winit spin `AboutToWait` between display ticks, warning
    // "processing non `RedrawRequested` event after the main event loop" each
    // time: 6.4 million lines in the first Simulator run, which reached the
    // game's first frames and never its 300th.
    #[cfg(target_os = "ios")]
    event_loop.set_control_flow(ControlFlow::Wait);
    #[cfg(not(target_os = "ios"))]
    event_loop.set_control_flow(ControlFlow::Poll);

    let winit_app = BsWinitApp {
        ecs_app: app,
        window: None,
        started: false,
        exit: AppExit::Success,
        suspended: false,
        #[cfg(target_os = "android")]
        safe_area_pending: false,
    };

    // A page's event loop is the browser's: `run_app` would escape it by
    // throwing an exception, which also unwinds whatever future called it.
    // `spawn_app` hands the app to the browser's loop and returns.
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(winit_app);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut winit_app = winit_app;
        event_loop
            .run_app(&mut winit_app)
            .expect("Event loop error");
        winit_app.exit
    }
    // The browser's loop outlives this call; nothing has exited yet.
    #[cfg(target_arch = "wasm32")]
    AppExit::Success
}

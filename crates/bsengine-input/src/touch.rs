use bevy_ecs::prelude::{MessageReader, Res, ResMut, Resource};
use bsengine_ecs::Message;

use crate::{plugin::MouseState, state::Input, types::MouseButton};

/// Where a finger is in its life, as the platform reports it in a
/// [`TouchInput`]: the three events every platform has, plus the system taking
/// the touch away (a gesture the OS claims, the app losing focus).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchEvent {
    /// The finger went down.
    Started,
    /// It moved.
    Moved,
    /// It lifted.
    Ended,
    /// The system took it away; it did not lift on the game's terms.
    Cancelled,
}

/// One finger's event from the window layer (winit's `WindowEvent::Touch`),
/// or from the test protocol, which writes the same message so a recording
/// drives the same code a phone does.
#[derive(Message, Debug, Clone)]
pub struct TouchInput {
    /// The finger, stable from its `Started` to its `Ended`/`Cancelled`.
    pub id: u64,
    /// What happened to it.
    pub event: TouchEvent,
    /// Its position in window pixels, top-left origin -- the space
    /// [`MouseState::position`] is in.
    pub x: f64,
    /// See `x`.
    pub y: f64,
}

/// A touch's phase this frame, Unity's `TouchPhase`: what a script reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    /// Went down this frame.
    Began,
    /// Held, and moved this frame.
    Moved,
    /// Held, and did not move this frame.
    Stationary,
    /// Lifted this frame. Its last frame: it is gone from the next.
    Ended,
    /// Taken away by the system this frame. Its last frame too.
    Canceled,
}

impl TouchPhase {
    /// The name scripts see: `"began"`, `"moved"`, `"stationary"`,
    /// `"ended"`, `"canceled"`.
    pub fn name(self) -> &'static str {
        match self {
            TouchPhase::Began => "began",
            TouchPhase::Moved => "moved",
            TouchPhase::Stationary => "stationary",
            TouchPhase::Ended => "ended",
            TouchPhase::Canceled => "canceled",
        }
    }
}

/// One finger on the screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Touch {
    /// The platform's id for the finger.
    pub id: u64,
    /// Where it is, in window pixels, top-left origin.
    pub position: (f64, f64),
    /// Where it went down: what a virtual stick measures from.
    pub start: (f64, f64),
    /// How far it moved this frame.
    pub delta: (f64, f64),
    /// See [`TouchPhase`].
    pub phase: TouchPhase,
}

/// Every finger on the screen this frame, in the order they went down, plus
/// the ones that lifted this frame (phase `Ended`/`Canceled`), for one frame,
/// as Unity's `Input.touches` keeps them -- so a script sees a lift without
/// having to remember which fingers it saw the frame before.
///
/// A touch that goes down and lifts within one frame is reported once, as
/// `Ended`: the phase is the latest thing that happened to it. At 60 frames a
/// second a tap lasts several frames, so a script that waits for `Began`
/// misses only a tap shorter than a frame; one that wants every tap reads
/// `Ended`.
#[derive(Resource, Debug, Default, Clone)]
pub struct Touches {
    touches: Vec<Touch>,
    /// The finger that drives the emulated mouse; see [`TouchSettings`].
    primary: Option<u64>,
}

impl Touches {
    /// This frame's touches. See [`Touches`].
    pub fn iter(&self) -> impl Iterator<Item = &Touch> {
        self.touches.iter()
    }

    /// The touch with this id, if it is on the screen or lifted this frame.
    pub fn get(&self, id: u64) -> Option<&Touch> {
        self.touches.iter().find(|t| t.id == id)
    }

    /// Fingers still down (every phase but `Ended` and `Canceled`).
    pub fn held(&self) -> usize {
        self.touches
            .iter()
            .filter(|t| !matches!(t.phase, TouchPhase::Ended | TouchPhase::Canceled))
            .count()
    }
}

/// How touches reach the rest of the input layer: `project.toml`'s
/// `[input] emulate_mouse_from_touch`.
#[derive(Resource, Debug, Clone)]
pub struct TouchSettings {
    /// Whether the first finger down is also the mouse: it presses
    /// `MouseButton::Left` and moves the cursor. On by default, as Unity's
    /// `simulateMouseWithTouches` and Godot's `emulate_mouse_from_touch` are:
    /// without it, a game's UI buttons -- which egui reads from the mouse --
    /// and every `Mouse Left` binding do nothing on a phone.
    pub emulate_mouse: bool,
}

impl Default for TouchSettings {
    fn default() -> Self {
        Self {
            emulate_mouse: true,
        }
    }
}

/// Ages last frame's touches (lifted ones go, held ones become `Stationary`
/// with no delta), then applies this frame's events. Runs after the mouse
/// systems in [`crate::InputSystems`], so an emulated press lands after
/// `clear_input_state` and its `just_pressed` lasts this frame, as a real
/// click's does.
pub(crate) fn update_touch_state(
    settings: Res<TouchSettings>,
    mut touches: ResMut<Touches>,
    mut buttons: ResMut<Input<MouseButton>>,
    mut mouse: ResMut<MouseState>,
    mut events: MessageReader<TouchInput>,
) {
    let touches = &mut *touches;
    touches
        .touches
        .retain(|t| !matches!(t.phase, TouchPhase::Ended | TouchPhase::Canceled));
    for t in &mut touches.touches {
        t.phase = TouchPhase::Stationary;
        t.delta = (0.0, 0.0);
    }
    for ev in events.read() {
        let position = (ev.x, ev.y);
        match ev.event {
            TouchEvent::Started => {
                // An id the platform reuses before we saw it lift (a lost
                // `Ended`) starts over rather than leaving two of it.
                touches.touches.retain(|t| t.id != ev.id);
                touches.touches.push(Touch {
                    id: ev.id,
                    position,
                    start: position,
                    delta: (0.0, 0.0),
                    phase: TouchPhase::Began,
                });
                if touches.primary.is_none() {
                    touches.primary = Some(ev.id);
                    if settings.emulate_mouse {
                        mouse.position = position;
                        buttons.press(MouseButton::Left);
                    }
                }
            }
            TouchEvent::Moved => {
                let Some(t) = touches.touches.iter_mut().find(|t| t.id == ev.id) else {
                    continue;
                };
                t.delta.0 += position.0 - t.position.0;
                t.delta.1 += position.1 - t.position.1;
                t.position = position;
                // A finger that went down this frame stays `Began`: a script
                // reading phases would otherwise never see it start.
                if t.phase == TouchPhase::Stationary {
                    t.phase = TouchPhase::Moved;
                }
                if settings.emulate_mouse && touches.primary == Some(ev.id) {
                    mouse.position = position;
                }
            }
            TouchEvent::Ended | TouchEvent::Cancelled => {
                let Some(t) = touches.touches.iter_mut().find(|t| t.id == ev.id) else {
                    continue;
                };
                t.delta.0 += position.0 - t.position.0;
                t.delta.1 += position.1 - t.position.1;
                t.position = position;
                t.phase = if ev.event == TouchEvent::Ended {
                    TouchPhase::Ended
                } else {
                    TouchPhase::Canceled
                };
                if touches.primary == Some(ev.id) {
                    // The next finger down becomes the mouse, not one already
                    // held: handing it over mid-drag would jump the cursor
                    // and press a button nobody pressed.
                    touches.primary = None;
                    if settings.emulate_mouse {
                        mouse.position = position;
                        buttons.release(MouseButton::Left);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InputPlugin;
    use bevy_ecs::message::Messages;
    use bsengine_app::new_app;

    fn app() -> bevy_app::App {
        let mut app = new_app();
        app.add_plugins(InputPlugin);
        app
    }

    fn send(app: &mut bevy_app::App, id: u64, event: TouchEvent, x: f64, y: f64) {
        app.world_mut()
            .resource_mut::<Messages<TouchInput>>()
            .write(TouchInput { id, event, x, y });
    }

    fn touch(app: &bevy_app::App, id: u64) -> Option<Touch> {
        app.world().resource::<Touches>().get(id).cloned()
    }

    #[test]
    fn a_touch_goes_through_its_phases_one_frame_each() {
        let mut app = app();
        send(&mut app, 7, TouchEvent::Started, 10.0, 20.0);
        app.update();
        let t = touch(&app, 7).expect("down");
        assert_eq!(t.phase, TouchPhase::Began);
        assert_eq!(t.position, (10.0, 20.0));

        app.update();
        assert_eq!(touch(&app, 7).unwrap().phase, TouchPhase::Stationary);

        send(&mut app, 7, TouchEvent::Moved, 13.0, 24.0);
        send(&mut app, 7, TouchEvent::Moved, 15.0, 30.0);
        app.update();
        let t = touch(&app, 7).unwrap();
        assert_eq!(t.phase, TouchPhase::Moved);
        assert_eq!(t.position, (15.0, 30.0));
        assert_eq!(t.delta, (5.0, 10.0), "both moves of the frame add up");
        assert_eq!(t.start, (10.0, 20.0));

        app.update();
        let t = touch(&app, 7).unwrap();
        assert_eq!(t.phase, TouchPhase::Stationary);
        assert_eq!(t.delta, (0.0, 0.0), "the delta is this frame's only");

        send(&mut app, 7, TouchEvent::Ended, 15.0, 30.0);
        app.update();
        assert_eq!(touch(&app, 7).unwrap().phase, TouchPhase::Ended);
        assert_eq!(app.world().resource::<Touches>().held(), 0);

        app.update();
        assert!(touch(&app, 7).is_none(), "a lifted touch lasts one frame");
    }

    #[test]
    fn a_moved_finger_that_went_down_this_frame_still_reads_began() {
        let mut app = app();
        send(&mut app, 1, TouchEvent::Started, 0.0, 0.0);
        send(&mut app, 1, TouchEvent::Moved, 4.0, 0.0);
        app.update();
        let t = touch(&app, 1).unwrap();
        assert_eq!(t.phase, TouchPhase::Began);
        assert_eq!(t.delta, (4.0, 0.0));
    }

    #[test]
    fn a_cancelled_touch_reads_canceled_and_lets_the_mouse_go() {
        let mut app = app();
        send(&mut app, 3, TouchEvent::Started, 5.0, 5.0);
        app.update();
        send(&mut app, 3, TouchEvent::Cancelled, 6.0, 5.0);
        app.update();
        assert_eq!(touch(&app, 3).unwrap().phase, TouchPhase::Canceled);
        assert!(!app
            .world()
            .resource::<Input<MouseButton>>()
            .is_pressed(&MouseButton::Left));
    }

    #[test]
    fn the_first_finger_is_the_mouse_and_a_second_one_is_not() {
        let mut app = app();
        send(&mut app, 1, TouchEvent::Started, 100.0, 50.0);
        app.update();
        {
            let buttons = app.world().resource::<Input<MouseButton>>();
            assert!(buttons.just_pressed(&MouseButton::Left));
            assert_eq!(app.world().resource::<MouseState>().position, (100.0, 50.0));
        }

        // A second finger moves and lifts; the cursor and button stay with the first.
        send(&mut app, 2, TouchEvent::Started, 400.0, 300.0);
        send(&mut app, 2, TouchEvent::Moved, 410.0, 300.0);
        app.update();
        send(&mut app, 2, TouchEvent::Ended, 410.0, 300.0);
        app.update();
        {
            let buttons = app.world().resource::<Input<MouseButton>>();
            assert!(buttons.is_pressed(&MouseButton::Left));
            assert!(!buttons.just_released(&MouseButton::Left));
            assert_eq!(app.world().resource::<MouseState>().position, (100.0, 50.0));
        }

        send(&mut app, 1, TouchEvent::Moved, 120.0, 60.0);
        app.update();
        assert_eq!(app.world().resource::<MouseState>().position, (120.0, 60.0));

        send(&mut app, 1, TouchEvent::Ended, 121.0, 61.0);
        app.update();
        let buttons = app.world().resource::<Input<MouseButton>>();
        assert!(buttons.just_released(&MouseButton::Left));
        assert!(!buttons.is_pressed(&MouseButton::Left));
        assert_eq!(app.world().resource::<MouseState>().position, (121.0, 61.0));
    }

    #[test]
    fn a_finger_held_when_the_first_lifts_does_not_take_the_mouse() {
        let mut app = app();
        send(&mut app, 1, TouchEvent::Started, 10.0, 10.0);
        send(&mut app, 2, TouchEvent::Started, 200.0, 200.0);
        app.update();
        send(&mut app, 1, TouchEvent::Ended, 10.0, 10.0);
        app.update();
        send(&mut app, 2, TouchEvent::Moved, 250.0, 250.0);
        app.update();
        assert_eq!(
            app.world().resource::<MouseState>().position,
            (10.0, 10.0),
            "the cursor jumped to a finger that was already down"
        );
        assert!(!app
            .world()
            .resource::<Input<MouseButton>>()
            .is_pressed(&MouseButton::Left));

        // The next finger down is the mouse.
        send(&mut app, 3, TouchEvent::Started, 30.0, 40.0);
        app.update();
        assert!(app
            .world()
            .resource::<Input<MouseButton>>()
            .just_pressed(&MouseButton::Left));
        assert_eq!(app.world().resource::<MouseState>().position, (30.0, 40.0));
    }

    #[test]
    fn with_emulation_off_a_touch_leaves_the_mouse_alone() {
        let mut app = app();
        app.insert_resource(TouchSettings {
            emulate_mouse: false,
        });
        send(&mut app, 1, TouchEvent::Started, 100.0, 50.0);
        app.update();
        // The premise: the touch itself arrived.
        assert_eq!(touch(&app, 1).unwrap().phase, TouchPhase::Began);
        assert!(!app
            .world()
            .resource::<Input<MouseButton>>()
            .is_pressed(&MouseButton::Left));
        assert_eq!(app.world().resource::<MouseState>().position, (0.0, 0.0));
    }

    #[test]
    fn an_emulated_click_reaches_a_mouse_bound_action() {
        // The consumer side of emulation: a `Mouse Left` binding, the way a
        // project would tap to fire.
        let mut app = new_app();
        let actions = crate::InputActions::from_config(
            &std::collections::BTreeMap::from([(
                "fire".to_string(),
                vec!["Mouse:Left".to_string()],
            )]),
            0.2,
        )
        .expect("binding parses");
        app.insert_resource(actions);
        app.add_plugins(InputPlugin);
        send(&mut app, 1, TouchEvent::Started, 1.0, 1.0);
        app.update();
        let fire = app
            .world()
            .resource::<crate::ActionState>()
            .get("fire")
            .expect("the action exists");
        assert!(fire.just_pressed);
    }
}

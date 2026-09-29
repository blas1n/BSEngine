//! Named input actions: "jump" rather than "Space".
//!
//! Unity's Input System, Unreal's Enhanced Input and Godot's InputMap agree on
//! the shape: a project declares named actions, each action lists several
//! bindings across devices, gameplay asks about the action, and the bindings
//! can be changed at runtime without touching gameplay. They also agree that
//! an action has a *strength* (0..1) as well as a pressed state, so a stick
//! pushed halfway and a key held down answer the same question. This module is
//! that shape and no more: no contexts/priorities (Unreal's mapping contexts)
//! and no interactions (Unity's hold/tap) -- a script builds those from
//! `isActionDown` and a timer, and none of this repo's games needs them.
//!
//! Bindings are strings so that `project.toml`, scripts and error messages
//! all speak the same spelling:
//!
//! | binding                | meaning                                       |
//! |------------------------|-----------------------------------------------|
//! | `"Space"`, `"E"`, `"1"`| a key, by its [`KEY_NAMES`] name              |
//! | `"Mouse:Left"`         | `Left`, `Right` or `Middle` mouse button      |
//! | `"Gamepad:South"`      | a [`GamepadButton`], by its variant name      |
//! | `"Gamepad:LeftStickX+"`| one direction of a stick axis (`+` or `-`)    |
//! | `"Gamepad:LeftTrigger"`| an analog trigger's pressure                  |
//!
//! Stick Y is positive *up*, as the gamepad layer reports it.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use bevy_ecs::prelude::{Res, ResMut, Resource};

use crate::{
    state::Input,
    types::{GamepadButton, GamepadSticks, KeyCode, MouseButton, KEY_NAMES},
};

/// The deadzone an action uses when `project.toml` names none: below this
/// much stick travel an action reads 0. Godot's default action deadzone is
/// 0.5 and Unity's press point is 0.5, but both apply a separate, smaller
/// stick deadzone (Unity 0.125, Unreal 0.25) before that; one number has to
/// do both jobs here, and 0.2 keeps a resting stick's drift out without
/// swallowing half the stick's range.
pub const DEFAULT_DEADZONE: f32 = 0.2;

/// An analog gamepad axis a binding can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    /// Left stick, horizontal; positive right.
    LeftStickX,
    /// Left stick, vertical; positive up.
    LeftStickY,
    /// Right stick, horizontal; positive right.
    RightStickX,
    /// Right stick, vertical; positive up.
    RightStickY,
    /// Left trigger pressure, 0..1.
    LeftTrigger,
    /// Right trigger pressure, 0..1.
    RightTrigger,
}

const AXIS_NAMES: &[(GamepadAxis, &str)] = &[
    (GamepadAxis::LeftStickX, "LeftStickX"),
    (GamepadAxis::LeftStickY, "LeftStickY"),
    (GamepadAxis::RightStickX, "RightStickX"),
    (GamepadAxis::RightStickY, "RightStickY"),
    (GamepadAxis::LeftTrigger, "LeftTrigger"),
    (GamepadAxis::RightTrigger, "RightTrigger"),
];

const MOUSE_NAMES: &[(MouseButton, &str)] = &[
    (MouseButton::Left, "Left"),
    (MouseButton::Right, "Right"),
    (MouseButton::Middle, "Middle"),
];

const GAMEPAD_BUTTON_NAMES: &[(GamepadButton, &str)] = &[
    (GamepadButton::South, "South"),
    (GamepadButton::East, "East"),
    (GamepadButton::West, "West"),
    (GamepadButton::North, "North"),
    (GamepadButton::LB, "LB"),
    (GamepadButton::RB, "RB"),
    (GamepadButton::LT, "LT"),
    (GamepadButton::RT, "RT"),
    (GamepadButton::Select, "Select"),
    (GamepadButton::Start, "Start"),
    (GamepadButton::LeftStick, "LeftStick"),
    (GamepadButton::RightStick, "RightStick"),
    (GamepadButton::DPadUp, "DPadUp"),
    (GamepadButton::DPadDown, "DPadDown"),
    (GamepadButton::DPadLeft, "DPadLeft"),
    (GamepadButton::DPadRight, "DPadRight"),
];

/// One physical input an action listens to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Binding {
    /// A keyboard key.
    Key(KeyCode),
    /// A mouse button.
    Mouse(MouseButton),
    /// A digital gamepad button.
    GamepadButton(GamepadButton),
    /// One direction of an analog axis: `positive` reads the axis as is,
    /// otherwise negated, so `LeftStickX-` is "stick pushed left". Triggers
    /// only have the positive direction.
    GamepadAxis {
        /// The axis read.
        axis: GamepadAxis,
        /// Which direction counts.
        positive: bool,
    },
}

fn lookup<T: Copy>(table: &[(T, &str)], name: &str) -> Option<T> {
    table.iter().find(|(_, n)| *n == name).map(|(v, _)| *v)
}

fn name_of<T: PartialEq>(table: &[(T, &'static str)], value: &T) -> &'static str {
    table
        .iter()
        .find(|(v, _)| v == value)
        .map(|(_, n)| *n)
        .unwrap_or("?")
}

impl Binding {
    /// Parses a binding string (see the module docs), or says what is wrong
    /// with it in words a project author can act on.
    pub fn parse(text: &str) -> Result<Binding, String> {
        let unknown = |what: &str, names: Vec<&str>| {
            format!(
                "unknown {what} in binding {text:?}; expected one of {}",
                names.join(", ")
            )
        };
        if let Some(button) = text.strip_prefix("Mouse:") {
            return lookup(MOUSE_NAMES, button)
                .map(Binding::Mouse)
                .ok_or_else(|| {
                    unknown(
                        "mouse button",
                        MOUSE_NAMES.iter().map(|(_, n)| *n).collect(),
                    )
                });
        }
        if let Some(input) = text.strip_prefix("Gamepad:") {
            if let Some(button) = lookup(GAMEPAD_BUTTON_NAMES, input) {
                return Ok(Binding::GamepadButton(button));
            }
            let (axis_name, positive) = match input.as_bytes().last() {
                Some(b'+') => (&input[..input.len() - 1], true),
                Some(b'-') => (&input[..input.len() - 1], false),
                _ => (input, true),
            };
            let axis = lookup(AXIS_NAMES, axis_name).ok_or_else(|| {
                let mut names: Vec<&str> = GAMEPAD_BUTTON_NAMES.iter().map(|(_, n)| *n).collect();
                names.extend([
                    "LeftStickX+/-",
                    "LeftStickY+/-",
                    "RightStickX+/-",
                    "RightStickY+/-",
                    "LeftTrigger",
                    "RightTrigger",
                ]);
                unknown("gamepad input", names)
            })?;
            let is_trigger = matches!(axis, GamepadAxis::LeftTrigger | GamepadAxis::RightTrigger);
            let signed = input.ends_with('+') || input.ends_with('-');
            // A stick axis without a direction would be ambiguous (is the
            // action "left" or "right"?), and a trigger with one would be a
            // direction the trigger never reports. Both are authoring
            // mistakes worth stopping at, not guessing through.
            if is_trigger && signed {
                return Err(format!(
                    "binding {text:?}: a trigger has no direction; write \"Gamepad:{axis_name}\""
                ));
            }
            if !is_trigger && !signed {
                return Err(format!(
                    "binding {text:?}: say which way the stick counts, \
                     \"Gamepad:{axis_name}+\" or \"Gamepad:{axis_name}-\""
                ));
            }
            return Ok(Binding::GamepadAxis { axis, positive });
        }
        KeyCode::from_name(text)
            .map(Binding::Key)
            .ok_or_else(|| unknown("key", KEY_NAMES.iter().map(|(_, n)| *n).collect()))
    }

    /// Whether this binding is a button (on/off) rather than an axis.
    pub fn is_digital(&self) -> bool {
        !matches!(self, Binding::GamepadAxis { .. })
    }
}

impl fmt::Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Binding::Key(code) => write!(f, "{}", code.name().unwrap_or("Unknown")),
            Binding::Mouse(button) => write!(f, "Mouse:{}", name_of(MOUSE_NAMES, button)),
            Binding::GamepadButton(button) => {
                write!(f, "Gamepad:{}", name_of(GAMEPAD_BUTTON_NAMES, button))
            }
            Binding::GamepadAxis { axis, positive } => {
                let name = name_of(AXIS_NAMES, axis);
                match axis {
                    GamepadAxis::LeftTrigger | GamepadAxis::RightTrigger => {
                        write!(f, "Gamepad:{name}")
                    }
                    _ => write!(f, "Gamepad:{name}{}", if *positive { '+' } else { '-' }),
                }
            }
        }
    }
}

/// The project's actions and what each is bound to.
///
/// Built from `project.toml`'s `[input]` table by the runtime; changed at
/// runtime only through [`InputActions::set_bindings`], which keeps the set of
/// action *names* fixed -- a rebinding screen changes what "jump" is bound to,
/// it does not invent a new action nothing reads.
#[derive(Resource, Debug, Clone)]
pub struct InputActions {
    actions: BTreeMap<String, Vec<Binding>>,
    deadzone: f32,
}

impl Default for InputActions {
    fn default() -> Self {
        Self {
            actions: BTreeMap::new(),
            deadzone: DEFAULT_DEADZONE,
        }
    }
}

impl InputActions {
    /// Builds the action set from `name -> [binding, ...]`, as written in
    /// `project.toml`. Every bad binding is reported, each with its action,
    /// so a project with three typos learns about all three at once.
    pub fn from_config(
        actions: &BTreeMap<String, Vec<String>>,
        deadzone: f32,
    ) -> Result<InputActions, String> {
        if !(0.0..1.0).contains(&deadzone) {
            return Err(format!(
                "[input] deadzone must be at least 0 and below 1, got {deadzone}"
            ));
        }
        let mut errors = Vec::new();
        let mut parsed = BTreeMap::new();
        for (name, bindings) in actions {
            let mut list = Vec::new();
            for text in bindings {
                match Binding::parse(text) {
                    Ok(binding) => list.push(binding),
                    Err(e) => errors.push(format!("action {name:?}: {e}")),
                }
            }
            parsed.insert(name.clone(), list);
        }
        if errors.is_empty() {
            Ok(InputActions {
                actions: parsed,
                deadzone,
            })
        } else {
            Err(errors.join("\n"))
        }
    }

    /// Every action's name, sorted.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.actions.keys().map(String::as_str)
    }

    /// What `action` is bound to, or `None` if there is no such action.
    pub fn bindings(&self, action: &str) -> Option<&[Binding]> {
        self.actions.get(action).map(Vec::as_slice)
    }

    /// The stick travel below which an axis binding reads 0.
    pub fn deadzone(&self) -> f32 {
        self.deadzone
    }

    /// Replaces what `action` is bound to. Errors, changing nothing, when
    /// there is no such action.
    pub fn set_bindings(&mut self, action: &str, bindings: Vec<Binding>) -> Result<(), String> {
        match self.actions.get_mut(action) {
            Some(slot) => {
                *slot = bindings;
                Ok(())
            }
            None => Err(format!("unknown action {action:?}")),
        }
    }
}

/// One action's reading for the current frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ActionValue {
    /// 0..1: 1 for a held button, the rescaled travel for an axis; the
    /// strongest of the action's bindings.
    pub strength: f32,
    /// Whether the action is held (strength above 0).
    pub pressed: bool,
    /// Whether the action went from not held to held this frame.
    pub just_pressed: bool,
    /// Whether the action went from held to not held this frame.
    pub just_released: bool,
}

/// Every action's [`ActionValue`] this frame, recomputed in `PreUpdate`
/// right after the devices are read.
#[derive(Resource, Debug, Clone, Default)]
pub struct ActionState {
    values: HashMap<String, ActionValue>,
}

impl ActionState {
    /// `action`'s reading, or `None` if there is no such action.
    pub fn get(&self, action: &str) -> Option<ActionValue> {
        self.values.get(action).copied()
    }

    /// Every action's reading.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ActionValue)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v))
    }
}

/// An axis reading in `direction`, with the deadzone cut off and the rest
/// stretched back over 0..1 -- Godot's action strength. Without the stretch
/// a stick at full travel would read `1 - deadzone` and never reach 1, and
/// "walk at half speed at half travel" would be off by the deadzone.
///
/// One clamp does both ends: travel inside the deadzone, or the other way,
/// comes out negative and is clamped to 0.
fn axis_strength(raw: f32, positive: bool, deadzone: f32) -> f32 {
    let along = if positive { raw } else { -raw };
    ((along - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0)
}

/// The devices an action reads, borrowed for one evaluation.
pub struct Devices<'a> {
    /// Keyboard.
    pub keys: &'a Input<KeyCode>,
    /// Mouse buttons.
    pub mouse: &'a Input<MouseButton>,
    /// Gamepad buttons.
    pub gamepad: &'a Input<GamepadButton>,
    /// Gamepad sticks and triggers.
    pub sticks: &'a GamepadSticks,
}

impl Devices<'_> {
    fn strength(&self, binding: &Binding, deadzone: f32) -> f32 {
        let on = |held: bool| if held { 1.0 } else { 0.0 };
        match binding {
            Binding::Key(code) => on(self.keys.is_pressed(code)),
            Binding::Mouse(button) => on(self.mouse.is_pressed(button)),
            Binding::GamepadButton(button) => on(self.gamepad.is_pressed(button)),
            Binding::GamepadAxis { axis, positive } => {
                let raw = match axis {
                    GamepadAxis::LeftStickX => self.sticks.left.0,
                    GamepadAxis::LeftStickY => self.sticks.left.1,
                    GamepadAxis::RightStickX => self.sticks.right.0,
                    GamepadAxis::RightStickY => self.sticks.right.1,
                    GamepadAxis::LeftTrigger => self.sticks.left_trigger,
                    GamepadAxis::RightTrigger => self.sticks.right_trigger,
                };
                axis_strength(raw, *positive, deadzone)
            }
        }
    }

    /// Whether a button binding went down this frame. Needed on top of the
    /// held state because a key pressed and released between two frames is
    /// never seen held -- `Input` records it only as `just_pressed` and
    /// `just_released` together -- and a jump tapped that fast must still
    /// jump.
    fn tapped(&self, binding: &Binding) -> bool {
        match binding {
            Binding::Key(code) => self.keys.just_pressed(code),
            Binding::Mouse(button) => self.mouse.just_pressed(button),
            Binding::GamepadButton(button) => self.gamepad.just_pressed(button),
            Binding::GamepadAxis { .. } => false,
        }
    }
}

impl ActionState {
    /// Recomputes every action from `devices`, against this state's previous
    /// readings for the edges.
    pub fn update(&mut self, actions: &InputActions, devices: &Devices) {
        let mut next = HashMap::with_capacity(actions.actions.len());
        for (name, bindings) in &actions.actions {
            let strength = bindings
                .iter()
                .map(|b| devices.strength(b, actions.deadzone))
                .fold(0.0_f32, f32::max);
            let was = self.values.get(name).is_some_and(|v| v.pressed);
            let pressed = strength > 0.0;
            let tapped = !was && bindings.iter().any(|b| devices.tapped(b));
            next.insert(
                name.clone(),
                ActionValue {
                    strength,
                    pressed,
                    just_pressed: (pressed && !was) || tapped,
                    just_released: (was && !pressed) || (tapped && !pressed),
                },
            );
        }
        self.values = next;
    }
}

pub(crate) fn update_action_state(
    actions: Res<InputActions>,
    mut state: ResMut<ActionState>,
    keys: Res<Input<KeyCode>>,
    mouse: Res<Input<MouseButton>>,
    gamepad: Res<Input<GamepadButton>>,
    sticks: Res<GamepadSticks>,
) {
    state.update(
        &actions,
        &Devices {
            keys: &keys,
            mouse: &mouse,
            gamepad: &gamepad,
            sticks: &sticks,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ElementState, InputPlugin, KeyInput};
    use bevy_ecs::event::Events;
    use bsengine_app::new_app;

    fn config(pairs: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(n, b)| (n.to_string(), b.iter().map(|s| s.to_string()).collect()))
            .collect()
    }

    #[test]
    fn every_binding_form_parses_and_prints_back_the_same() {
        for text in [
            "Space",
            "E",
            "1",
            "ShiftLeft",
            "Mouse:Left",
            "Mouse:Middle",
            "Gamepad:South",
            "Gamepad:DPadLeft",
            "Gamepad:LeftStickX+",
            "Gamepad:LeftStickX-",
            "Gamepad:RightStickY-",
            "Gamepad:LeftTrigger",
            "Gamepad:RightTrigger",
        ] {
            let binding = Binding::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(binding.to_string(), text);
        }
        assert_eq!(
            Binding::parse("Gamepad:LeftStickX-").unwrap(),
            Binding::GamepadAxis {
                axis: GamepadAxis::LeftStickX,
                positive: false
            }
        );
        assert_eq!(
            Binding::parse("Gamepad:LeftStick").unwrap(),
            Binding::GamepadButton(GamepadButton::LeftStick),
            "the stick *click* is a button, not the axis"
        );
    }

    #[test]
    fn a_bad_binding_is_an_error_that_names_it() {
        for (text, says) in [
            ("space", "unknown key"),
            ("Mouse:Wheel", "unknown mouse button"),
            ("Gamepad:A", "unknown gamepad input"),
            ("Gamepad:LeftStickX", "say which way"),
            ("Gamepad:LeftTrigger+", "a trigger has no direction"),
        ] {
            let err = Binding::parse(text).expect_err(text);
            assert!(
                err.contains(says) && err.contains(&format!("{text:?}")),
                "{text}: {err}"
            );
        }
    }

    /// Every mistake in the table at once, each with its action -- a project
    /// author fixing them one run at a time is the failure this prevents.
    #[test]
    fn from_config_reports_every_bad_binding_with_its_action() {
        let err = InputActions::from_config(
            &config(&[
                ("jump", &["Space", "Gamepad:A"]),
                ("fire", &["Mouse:Wheel"]),
                ("move", &["D"]),
            ]),
            DEFAULT_DEADZONE,
        )
        .expect_err("two bad bindings");
        assert!(
            err.contains("action \"jump\"") && err.contains("\"Gamepad:A\""),
            "{err}"
        );
        assert!(
            err.contains("action \"fire\"") && err.contains("\"Mouse:Wheel\""),
            "{err}"
        );
        assert!(
            !err.contains("\"move\""),
            "a good action is not an error: {err}"
        );

        assert!(InputActions::from_config(&config(&[]), 1.0).is_err());
        assert!(InputActions::from_config(&config(&[]), -0.1).is_err());
    }

    fn app_with(pairs: &[(&str, &[&str])]) -> bevy_app::App {
        let mut app = new_app();
        app.insert_resource(InputActions::from_config(&config(pairs), 0.2).unwrap());
        app.add_plugins(InputPlugin);
        app
    }

    fn key(app: &mut bevy_app::App, code: KeyCode, state: ElementState) {
        app.world_mut()
            .resource_mut::<Events<KeyInput>>()
            .send(KeyInput {
                key_code: code,
                state,
                text: None,
            });
    }

    fn value(app: &bevy_app::App, action: &str) -> ActionValue {
        app.world()
            .resource::<ActionState>()
            .get(action)
            .unwrap_or_else(|| panic!("no action {action:?}"))
    }

    /// The whole life of a button-bound action through the real plugin: key
    /// event in, `PreUpdate` systems, `ActionState` out. Each frame checks
    /// all four fields, so an implementation that only tracked `pressed`, or
    /// never cleared an edge, fails on the frame where it differs.
    #[test]
    fn a_key_drives_its_action_through_press_hold_and_release() {
        let mut app = app_with(&[("jump", &["Space", "Gamepad:South"]), ("fire", &["E"])]);
        app.update();
        assert_eq!(value(&app, "jump"), ActionValue::default());

        key(&mut app, KeyCode::Space, ElementState::Pressed);
        app.update();
        let v = value(&app, "jump");
        assert!(
            v.pressed && v.just_pressed && !v.just_released && v.strength == 1.0,
            "{v:?}"
        );
        assert!(
            !value(&app, "fire").pressed,
            "another action must not follow Space"
        );

        app.update();
        let v = value(&app, "jump");
        assert!(
            v.pressed && !v.just_pressed && !v.just_released,
            "held: {v:?}"
        );

        key(&mut app, KeyCode::Space, ElementState::Released);
        app.update();
        let v = value(&app, "jump");
        assert!(
            !v.pressed && !v.just_pressed && v.just_released && v.strength == 0.0,
            "{v:?}"
        );

        app.update();
        assert_eq!(value(&app, "jump"), ActionValue::default());
    }

    /// Pressed and released between two frames: `Input` never shows the key
    /// held, only both edges. The action must still report a press, or a
    /// fast tap on a slow frame is a jump the player made and the game lost.
    #[test]
    fn a_tap_inside_one_frame_still_presses_the_action() {
        let mut app = app_with(&[("jump", &["Space"])]);
        app.update();
        key(&mut app, KeyCode::Space, ElementState::Pressed);
        key(&mut app, KeyCode::Space, ElementState::Released);
        app.update();
        assert!(
            !app.world()
                .resource::<Input<KeyCode>>()
                .is_pressed(&KeyCode::Space),
            "premise: the key is not held at the end of the frame"
        );
        let v = value(&app, "jump");
        assert!(v.just_pressed && v.just_released && !v.pressed, "{v:?}");
    }

    /// A stick direction through the deadzone: nothing inside it, the rest
    /// stretched to 0..1, nothing in the other direction; and a key on the
    /// same action wins with its full 1.
    #[test]
    fn a_stick_direction_reads_its_travel_past_the_deadzone() {
        let mut app = app_with(&[
            ("right", &["D", "Gamepad:LeftStickX+"]),
            ("left", &["A", "Gamepad:LeftStickX-"]),
        ]);
        let stick = |app: &mut bevy_app::App, x: f32| {
            app.world_mut().resource_mut::<GamepadSticks>().left.0 = x;
            app.update();
        };

        stick(&mut app, 0.15);
        assert_eq!(value(&app, "right").strength, 0.0, "inside the deadzone");
        assert!(!value(&app, "right").pressed);

        stick(&mut app, 0.6);
        let right = value(&app, "right");
        assert!(
            (right.strength - 0.5).abs() < 1e-6,
            "(0.6-0.2)/0.8: {right:?}"
        );
        assert!(right.pressed && right.just_pressed);
        assert_eq!(value(&app, "left").strength, 0.0, "the other direction");

        stick(&mut app, 1.0);
        assert_eq!(value(&app, "right").strength, 1.0, "full travel reaches 1");

        stick(&mut app, -0.6);
        assert!(value(&app, "right").just_released);
        assert!((value(&app, "left").strength - 0.5).abs() < 1e-6);

        key(&mut app, KeyCode::A, ElementState::Pressed);
        stick(&mut app, -0.6);
        assert_eq!(
            value(&app, "left").strength,
            1.0,
            "the strongest binding wins"
        );
    }

    /// Rebinding takes effect for the next frame's reading, the old binding
    /// stops counting, and a name that is not an action is refused.
    #[test]
    fn set_bindings_moves_an_action_to_other_inputs() {
        let mut app = app_with(&[("jump", &["Space"])]);
        app.world_mut()
            .resource_mut::<InputActions>()
            .set_bindings("jump", vec![Binding::Key(KeyCode::J)])
            .unwrap();
        key(&mut app, KeyCode::Space, ElementState::Pressed);
        app.update();
        assert!(!value(&app, "jump").pressed, "Space is no longer jump");
        key(&mut app, KeyCode::J, ElementState::Pressed);
        app.update();
        assert!(value(&app, "jump").just_pressed, "J is");

        let err = app
            .world_mut()
            .resource_mut::<InputActions>()
            .set_bindings("jmup", vec![])
            .expect_err("no such action");
        assert!(err.contains("\"jmup\""), "{err}");
    }
}

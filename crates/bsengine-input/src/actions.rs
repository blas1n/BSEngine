//! Named input actions: "jump" rather than "Space".
//!
//! Unity's Input System, Unreal's Enhanced Input and Godot's InputMap agree on
//! the shape: a project declares named actions, each action lists several
//! bindings across devices, gameplay asks about the action, and the bindings
//! can be changed at runtime without touching gameplay. They also agree that
//! an action has a *strength* (0..1) as well as a pressed state, so a stick
//! pushed halfway and a key held down answer the same question.
//!
//! On top of that shape, the two that have them agree on two more things,
//! and this module has both. **Contexts** (Unreal's mapping contexts; Unity's
//! action maps are the equal-priority case): each [`InputContext`] maps
//! actions to bindings, can be switched on and off, and while on consumes the
//! inputs it maps from every context of lower priority -- a menu takes
//! `Escape` from gameplay without gameplay being switched off. **Interactions**
//! ([`Interaction`]: hold, tap, hold-and-release): when a press fires, read as
//! [`ActionValue::triggered`], with `ongoing` / `canceled` phases. Godot has
//! neither. `[input.actions]` is the context named [`DEFAULT_CONTEXT`], so a
//! project that writes neither behaves exactly as before they existed.
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
use bsengine_core::Time;

use crate::{
    interaction::{Interaction, Tracker},
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

/// The context `[input.actions]` fills: priority 0, enabled. A project that
/// never writes `[input.contexts]` has only this one, and behaves as it did
/// before contexts existed.
pub const DEFAULT_CONTEXT: &str = "default";

/// What one action is bound to inside one context.
#[derive(Debug, Clone, PartialEq)]
pub struct Mapping {
    /// The inputs that drive the action here.
    pub bindings: Vec<Binding>,
    /// When a press of them fires `triggered`; `None` is the press edge.
    pub interaction: Option<Interaction>,
}

/// Mappings that are switched on and off together and ranked against the
/// other contexts -- Unreal's Input Mapping Context.
///
/// While enabled, an input this context maps is *consumed*: a context of
/// lower priority does not see it, mapped or not. That is what lets a menu
/// at priority 10 take `Escape` from gameplay's `pause` without the game
/// switching gameplay off. Contexts of equal priority never consume each
/// other's inputs, so a project that only ever enables and disables contexts
/// gets Unity's Action Maps.
#[derive(Debug, Clone)]
pub struct InputContext {
    /// Higher wins an input both map.
    pub priority: i32,
    /// Whether this context's mappings count at all.
    pub enabled: bool,
    actions: BTreeMap<String, Mapping>,
}

impl InputContext {
    /// What this context binds `action` to, if it maps it.
    pub fn mapping(&self, action: &str) -> Option<&Mapping> {
        self.actions.get(action)
    }

    /// Every action this context maps, sorted.
    pub fn actions(&self) -> impl Iterator<Item = (&str, &Mapping)> {
        self.actions.iter().map(|(n, m)| (n.as_str(), m))
    }
}

/// One mapping as `project.toml` writes it, before anything is parsed.
#[derive(Debug, Clone, Default)]
pub struct MappingConfig {
    /// Binding strings (see the module docs).
    pub bindings: Vec<String>,
    /// `"hold"`, `"tap"` or `"hold_and_release"`.
    pub interaction: Option<String>,
    /// The interaction's threshold in seconds.
    pub time: Option<f32>,
    /// Keys the table had that a mapping does not, to be reported with the
    /// rest rather than ignored: a misspelled `intercation` would otherwise
    /// ship as a plain press.
    pub unknown_keys: Vec<String>,
}

impl From<Vec<String>> for MappingConfig {
    fn from(bindings: Vec<String>) -> Self {
        Self {
            bindings,
            ..Self::default()
        }
    }
}

/// One `[input.contexts.<name>]` table, before anything is parsed.
#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// See [`InputContext::priority`].
    pub priority: i32,
    /// Whether it starts enabled.
    pub enabled: bool,
    /// Action name -> its mapping here.
    pub actions: BTreeMap<String, MappingConfig>,
    /// As [`MappingConfig::unknown_keys`].
    pub unknown_keys: Vec<String>,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            priority: 0,
            enabled: true,
            actions: BTreeMap::new(),
            unknown_keys: Vec::new(),
        }
    }
}

/// The project's actions, the contexts that map them, and what each binds.
///
/// Built from `project.toml`'s `[input]` table by the runtime; changed at
/// runtime only by rebinding and by enabling or disabling a context, which
/// keeps the set of action *names* and contexts fixed -- a rebinding screen
/// changes what "jump" is bound to, it does not invent a new action nothing
/// reads.
#[derive(Resource, Debug, Clone)]
pub struct InputActions {
    contexts: BTreeMap<String, InputContext>,
    deadzone: f32,
}

impl Default for InputActions {
    fn default() -> Self {
        Self {
            contexts: BTreeMap::from([(
                DEFAULT_CONTEXT.to_string(),
                InputContext {
                    priority: 0,
                    enabled: true,
                    actions: BTreeMap::new(),
                },
            )]),
            deadzone: DEFAULT_DEADZONE,
        }
    }
}

impl InputActions {
    /// Builds the action set from `name -> [binding, ...]`: the default
    /// context alone, with no interactions. See [`InputActions::from_contexts`].
    pub fn from_config(
        actions: &BTreeMap<String, Vec<String>>,
        deadzone: f32,
    ) -> Result<InputActions, String> {
        let default = actions
            .iter()
            .map(|(name, bindings)| (name.clone(), MappingConfig::from(bindings.clone())))
            .collect();
        Self::from_contexts(&default, &BTreeMap::new(), deadzone)
    }

    /// Builds the action set from `[input.actions]` (`default`) and
    /// `[input.contexts]`. Every problem is reported, each with its context
    /// and action, so a project with three typos learns about all three at
    /// once.
    pub fn from_contexts(
        default: &BTreeMap<String, MappingConfig>,
        contexts: &BTreeMap<String, ContextConfig>,
        deadzone: f32,
    ) -> Result<InputActions, String> {
        if !(0.0..1.0).contains(&deadzone) {
            return Err(format!(
                "[input] deadzone must be at least 0 and below 1, got {deadzone}"
            ));
        }
        let mut errors = Vec::new();
        if contexts.contains_key(DEFAULT_CONTEXT) {
            errors.push(format!(
                "[input.contexts.{DEFAULT_CONTEXT}]: the {DEFAULT_CONTEXT:?} context is \
                 [input.actions]; give this one another name"
            ));
        }
        let default_config = ContextConfig {
            actions: default.clone(),
            ..ContextConfig::default()
        };
        let all = std::iter::once((DEFAULT_CONTEXT, &default_config)).chain(
            contexts
                .iter()
                .filter(|(name, _)| *name != DEFAULT_CONTEXT)
                .map(|(name, config)| (name.as_str(), config)),
        );
        let mut parsed = BTreeMap::new();
        for (context, config) in all {
            for key in &config.unknown_keys {
                errors.push(format!(
                    "context {context:?}: unknown key {key:?}; expected priority, enabled, actions"
                ));
            }
            let mut actions = BTreeMap::new();
            for (name, mapping) in &config.actions {
                // The default context keeps the wording it had before
                // contexts, so its errors read as they always did.
                let at = if context == DEFAULT_CONTEXT {
                    format!("action {name:?}")
                } else {
                    format!("context {context:?}, action {name:?}")
                };
                for key in &mapping.unknown_keys {
                    errors.push(format!(
                        "{at}: unknown key {key:?}; expected bindings, interaction, time"
                    ));
                }
                let mut bindings = Vec::new();
                for text in &mapping.bindings {
                    match Binding::parse(text) {
                        Ok(binding) => bindings.push(binding),
                        Err(e) => errors.push(format!("{at}: {e}")),
                    }
                }
                let interaction =
                    match Interaction::parse(mapping.interaction.as_deref(), mapping.time) {
                        Ok(interaction) => interaction,
                        Err(e) => {
                            errors.push(format!("{at}: {e}"));
                            None
                        }
                    };
                actions.insert(
                    name.clone(),
                    Mapping {
                        bindings,
                        interaction,
                    },
                );
            }
            parsed.insert(
                context.to_string(),
                InputContext {
                    priority: config.priority,
                    enabled: config.enabled,
                    actions,
                },
            );
        }
        // Which of two equal-priority contexts' interactions applies would
        // depend on a tie-break nobody wrote down. Refused, so the author
        // picks by giving one a different priority.
        let mut interacting: BTreeMap<(&str, i32), Vec<&str>> = BTreeMap::new();
        for (context, ctx) in &parsed {
            for (action, mapping) in &ctx.actions {
                if mapping.interaction.is_some() {
                    interacting
                        .entry((action.as_str(), ctx.priority))
                        .or_default()
                        .push(context.as_str());
                }
            }
        }
        for ((action, priority), contexts) in interacting {
            if contexts.len() > 1 {
                errors.push(format!(
                    "action {action:?} has an interaction in contexts {} at the same priority \
                     {priority}; give one of them another priority",
                    contexts.join(", ")
                ));
            }
        }
        if errors.is_empty() {
            Ok(InputActions {
                contexts: parsed,
                deadzone,
            })
        } else {
            Err(errors.join("\n"))
        }
    }

    /// Every action's name, sorted: every action any context maps, enabled
    /// or not -- disabling a menu does not make its actions stop existing.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        let names: std::collections::BTreeSet<&str> = self
            .contexts
            .values()
            .flat_map(|c| c.actions.keys().map(String::as_str))
            .collect();
        names.into_iter()
    }

    /// What `action` is bound to in the default context, or `None` if the
    /// default context does not map it.
    pub fn bindings(&self, action: &str) -> Option<&[Binding]> {
        self.bindings_in(DEFAULT_CONTEXT, action)
    }

    /// What `action` is bound to in `context`.
    pub fn bindings_in(&self, context: &str, action: &str) -> Option<&[Binding]> {
        self.contexts
            .get(context)?
            .actions
            .get(action)
            .map(|m| m.bindings.as_slice())
    }

    /// The stick travel below which an axis binding reads 0.
    pub fn deadzone(&self) -> f32 {
        self.deadzone
    }

    /// Every context's name, sorted.
    pub fn context_names(&self) -> impl Iterator<Item = &str> {
        self.contexts.keys().map(String::as_str)
    }

    /// The context called `name`.
    pub fn context(&self, name: &str) -> Option<&InputContext> {
        self.contexts.get(name)
    }

    /// Switches a context on or off from the next reading. Errors, changing
    /// nothing, when there is no such context.
    pub fn set_context_enabled(&mut self, name: &str, enabled: bool) -> Result<(), String> {
        match self.contexts.get_mut(name) {
            Some(context) => {
                context.enabled = enabled;
                Ok(())
            }
            None => Err(self.unknown_context(name)),
        }
    }

    /// Replaces what `action` is bound to in the default context. Errors,
    /// changing nothing, when there is no such action.
    pub fn set_bindings(&mut self, action: &str, bindings: Vec<Binding>) -> Result<(), String> {
        self.set_bindings_in(DEFAULT_CONTEXT, action, bindings)
    }

    /// Replaces what `action` is bound to in `context`, keeping its
    /// interaction. Errors, changing nothing, when the context does not exist
    /// or does not map the action -- rebinding adds no mapping a context did
    /// not declare.
    pub fn set_bindings_in(
        &mut self,
        context: &str,
        action: &str,
        bindings: Vec<Binding>,
    ) -> Result<(), String> {
        let unknown_context = self.unknown_context(context);
        let known_action = self.names().any(|n| n == action);
        let Some(ctx) = self.contexts.get_mut(context) else {
            return Err(unknown_context);
        };
        match ctx.actions.get_mut(action) {
            Some(mapping) => {
                mapping.bindings = bindings;
                Ok(())
            }
            None if known_action => Err(format!(
                "input context {context:?} does not map action {action:?}"
            )),
            None => Err(format!("unknown action {action:?}")),
        }
    }

    fn unknown_context(&self, name: &str) -> String {
        format!(
            "unknown input context {name:?}; contexts are {}",
            self.context_names().collect::<Vec<_>>().join(", ")
        )
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
    /// Whether the action fired this frame: its interaction's moment, or the
    /// press edge for an action without one.
    pub triggered: bool,
    /// Whether its interaction has started and neither fired nor been
    /// canceled yet -- a charge bar fills while this is true.
    pub ongoing: bool,
    /// Whether its interaction ended this frame without firing.
    pub canceled: bool,
    /// Seconds the action has been held; 0 when it is not.
    pub held_time: f32,
}

/// Every action's [`ActionValue`] this frame, recomputed in `PreUpdate`
/// right after the devices are read.
#[derive(Resource, Debug, Clone, Default)]
pub struct ActionState {
    values: HashMap<String, ActionValue>,
    trackers: HashMap<String, Tracker>,
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

/// What the enabled contexts make of one action this frame, before edges.
#[derive(Default)]
struct Reading {
    strength: f32,
    tapped: bool,
    interaction: Option<Interaction>,
}

impl ActionState {
    /// Recomputes every action from `devices`, against this state's previous
    /// readings for the edges, advancing interactions by `dt` seconds.
    ///
    /// Enabled contexts are read from the highest priority down. Each
    /// priority level sees only the inputs no higher level maps, then claims
    /// every input it maps itself -- *mapped*, not pressed, so whether a key
    /// reaches gameplay never depends on what else happened to be held.
    pub fn update(&mut self, actions: &InputActions, devices: &Devices, dt: f32) {
        let mut readings: BTreeMap<&str, Reading> =
            actions.names().map(|n| (n, Reading::default())).collect();
        let mut enabled: Vec<&InputContext> =
            actions.contexts.values().filter(|c| c.enabled).collect();
        // Stable, so equal priorities keep the contexts' name order.
        enabled.sort_by_key(|c| std::cmp::Reverse(c.priority));

        let mut claimed: Vec<Binding> = Vec::new();
        for level in enabled.chunk_by(|a, b| a.priority == b.priority) {
            let mut level_claims = Vec::new();
            for context in level {
                for (action, mapping) in &context.actions {
                    let reading = readings
                        .get_mut(action.as_str())
                        .expect("names() covers every mapped action");
                    let mut survived = false;
                    for binding in mapping.bindings.iter().filter(|b| !claimed.contains(b)) {
                        survived = true;
                        reading.strength = reading
                            .strength
                            .max(devices.strength(binding, actions.deadzone));
                        reading.tapped |= devices.tapped(binding);
                    }
                    if survived && reading.interaction.is_none() {
                        reading.interaction = mapping.interaction;
                    }
                    level_claims.extend(mapping.bindings.iter().copied());
                }
            }
            claimed.extend(level_claims);
        }

        let mut next = HashMap::with_capacity(readings.len());
        for (name, reading) in readings {
            let was = self.values.get(name).is_some_and(|v| v.pressed);
            let pressed = reading.strength > 0.0;
            let tapped = !was && reading.tapped;
            let step = self.trackers.entry(name.to_string()).or_default().step(
                reading.interaction,
                was,
                pressed,
                tapped,
                dt,
            );
            next.insert(
                name.to_string(),
                ActionValue {
                    strength: reading.strength,
                    pressed,
                    just_pressed: (pressed && !was) || tapped,
                    just_released: (was && !pressed) || (tapped && !pressed),
                    triggered: step.triggered,
                    ongoing: step.ongoing,
                    canceled: step.canceled,
                    held_time: step.held_time,
                },
            );
        }
        self.values = next;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn update_action_state(
    actions: Res<InputActions>,
    mut state: ResMut<ActionState>,
    keys: Res<Input<KeyCode>>,
    mouse: Res<Input<MouseButton>>,
    gamepad: Res<Input<GamepadButton>>,
    sticks: Res<GamepadSticks>,
    // Optional: an app without a clock (most of the input crate's own tests)
    // still reads actions, with every interaction frozen at 0 s.
    time: Option<Res<Time>>,
) {
    state.update(
        &actions,
        &Devices {
            keys: &keys,
            mouse: &mouse,
            gamepad: &gamepad,
            sticks: &sticks,
        },
        time.map_or(0.0, |t| t.delta_seconds),
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

    fn mapping(bindings: &[&str], interaction: Option<&str>, time: Option<f32>) -> MappingConfig {
        MappingConfig {
            bindings: bindings.iter().map(|s| s.to_string()).collect(),
            interaction: interaction.map(str::to_string),
            time,
            unknown_keys: Vec::new(),
        }
    }

    fn context(priority: i32, enabled: bool, actions: Vec<(&str, MappingConfig)>) -> ContextConfig {
        ContextConfig {
            priority,
            enabled,
            actions: actions.into_iter().map(|(n, m)| (n.to_string(), m)).collect(),
            unknown_keys: Vec::new(),
        }
    }

    fn actions_from(
        default: Vec<(&str, MappingConfig)>,
        contexts: Vec<(&str, ContextConfig)>,
    ) -> Result<InputActions, String> {
        InputActions::from_contexts(
            &default.into_iter().map(|(n, m)| (n.to_string(), m)).collect(),
            &contexts.into_iter().map(|(n, c)| (n.to_string(), c)).collect(),
            DEFAULT_DEADZONE,
        )
    }

    /// The real plugin on a fixed 0.1 s clock. `time_first` picks which
    /// plugin registers its systems first.
    fn timed_app(actions: InputActions, time_first: bool) -> bevy_app::App {
        let mut app = new_app();
        app.insert_resource(actions);
        if time_first {
            app.add_plugins(bsengine_app::TimePlugin);
            app.add_plugins(InputPlugin);
        } else {
            app.add_plugins(InputPlugin);
            app.add_plugins(bsengine_app::TimePlugin);
        }
        app.insert_resource(Time::fixed(0.1));
        app
    }

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    /// Every problem at once, each naming where it is.
    #[test]
    fn from_contexts_reports_every_problem_with_its_context_and_action() {
        let mut typo = mapping(&["E"], None, None);
        typo.unknown_keys.push("intercation".into());
        let mut bad_context = context(5, true, vec![]);
        bad_context.unknown_keys.push("priorty".into());
        let err = actions_from(
            vec![
                ("dodge", mapping(&["ShiftLeft"], Some("dash"), Some(0.2))),
                ("use", typo),
                ("ok", mapping(&["Space"], Some("hold"), Some(0.5))),
            ],
            vec![
                ("menu", context(10, true, vec![("back", mapping(&["Escape"], Some("tap"), Some(0.0)))])),
                ("default", context(0, true, vec![])),
                ("odd", bad_context),
                ("also", context(0, true, vec![("ok", mapping(&["O"], Some("tap"), Some(0.2)))])),
            ],
        )
        .expect_err("several problems");
        for says in [
            "action \"dodge\": unknown interaction \"dash\"",
            "action \"use\": unknown key \"intercation\"",
            "context \"menu\", action \"back\": interaction \"tap\": \"time\" must be a positive",
            "[input.contexts.default]",
            "context \"odd\": unknown key \"priorty\"",
            "action \"ok\" has an interaction in contexts also, default at the same priority 0",
        ] {
            assert!(err.contains(says), "missing {says:?} in:\n{err}");
        }
        assert_eq!(err.lines().count(), 6, "one line each, nothing else:\n{err}");

        let fine = actions_from(
            vec![("ok", mapping(&["Space"], Some("hold"), Some(0.5)))],
            vec![("menu", context(1, true, vec![("ok", mapping(&["O"], Some("tap"), Some(0.2)))]))],
        );
        assert!(fine.is_ok(), "different priorities may both have one: {fine:?}");
    }

    /// An action only a disabled context maps is still an action; rebinding
    /// and switching refuse what does not exist, and say what does.
    #[test]
    fn contexts_keep_their_names_and_refuse_unknown_ones() {
        let mut actions = actions_from(
            vec![("jump", mapping(&["Space"], None, None))],
            vec![("menu", context(10, false, vec![("back", mapping(&["Escape"], None, None))]))],
        )
        .unwrap();
        assert_eq!(actions.names().collect::<Vec<_>>(), ["back", "jump"]);
        assert_eq!(actions.bindings("back"), None, "not in the default context");
        assert_eq!(
            actions.bindings_in("menu", "back"),
            Some(&[Binding::Key(KeyCode::Escape)][..])
        );

        let err = actions.set_context_enabled("hud", true).expect_err("no hud");
        assert!(err.contains("\"hud\"") && err.contains("default, menu"), "{err}");
        let err = actions
            .set_bindings_in("menu", "jump", vec![])
            .expect_err("menu does not map jump");
        assert!(err.contains("does not map action \"jump\""), "{err}");
        let err = actions.set_bindings_in("menu", "jmup", vec![]).expect_err("no jmup");
        assert!(err.contains("unknown action \"jmup\""), "{err}");

        actions.set_context_enabled("menu", true).unwrap();
        assert!(actions.context("menu").unwrap().enabled);
    }

    /// The press is timed with *this* frame's step. The clock is switched to
    /// 0.5 s between two frames, so a reading taken before the tick (last
    /// frame's 0.1) and one taken after it (0.5) differ -- on a constant
    /// fixed clock they never would, and the ordering would go untested.
    /// Both registration orders, because unordered systems still run in
    /// *some* fixed order and one of the two would pass by luck.
    #[test]
    fn a_press_is_timed_by_this_frames_clock_whichever_plugin_comes_first() {
        for time_first in [true, false] {
            let actions = actions_from(
                vec![("charge", mapping(&["Space"], Some("hold"), Some(0.4)))],
                vec![],
            )
            .unwrap();
            let mut app = timed_app(actions, time_first);
            app.update();
            key(&mut app, KeyCode::Space, ElementState::Pressed);
            app.update();
            assert!(value(&app, "charge").ongoing, "premise: the hold started");
            app.insert_resource(Time::fixed(0.5));
            app.update();
            let v = value(&app, "charge");
            assert!(
                approx(v.held_time, 0.5) && v.triggered,
                "time_first={time_first}: held for this frame's 0.5 s, past 0.4: {v:?}"
            );
        }
    }

    /// A menu at priority 10 takes Escape from gameplay's `pause` while it
    /// is enabled, and hands it back the frame after it is disabled.
    #[test]
    fn a_higher_context_consumes_its_inputs_until_it_is_disabled() {
        let actions = actions_from(
            vec![("pause", mapping(&["Escape", "P"], None, None))],
            vec![("menu", context(10, true, vec![("back", mapping(&["Escape"], None, None))]))],
        )
        .unwrap();
        let mut app = timed_app(actions, true);
        key(&mut app, KeyCode::Escape, ElementState::Pressed);
        app.update();
        assert!(value(&app, "back").pressed, "premise: the menu sees Escape");
        assert!(!value(&app, "pause").pressed, "and gameplay does not");

        key(&mut app, KeyCode::P, ElementState::Pressed);
        app.update();
        assert!(value(&app, "pause").pressed, "P is not the menu's, so it still reaches pause");

        key(&mut app, KeyCode::P, ElementState::Released);
        app.world_mut()
            .resource_mut::<InputActions>()
            .set_context_enabled("menu", false)
            .unwrap();
        app.update();
        assert!(!value(&app, "back").pressed, "a disabled context reads nothing");
        assert!(value(&app, "pause").pressed, "Escape is gameplay's again");
    }

    /// Equal priorities never consume: both actions see the key.
    #[test]
    fn contexts_of_equal_priority_both_see_a_shared_input() {
        let actions = actions_from(
            vec![("pause", mapping(&["Escape"], None, None))],
            vec![("hud", context(0, true, vec![("close", mapping(&["Escape"], None, None))]))],
        )
        .unwrap();
        let mut app = timed_app(actions, true);
        key(&mut app, KeyCode::Escape, ElementState::Pressed);
        app.update();
        assert!(value(&app, "pause").pressed && value(&app, "close").pressed);
    }

    /// The context that gave a hold its press goes away mid-hold: the press
    /// ends there, and the hold is canceled, not fired.
    #[test]
    fn disabling_a_context_mid_hold_cancels_the_hold() {
        let actions = actions_from(
            vec![],
            vec![(
                "combat",
                context(5, true, vec![("charge", mapping(&["Space"], Some("hold"), Some(0.5)))]),
            )],
        )
        .unwrap();
        let mut app = timed_app(actions, true);
        key(&mut app, KeyCode::Space, ElementState::Pressed);
        app.update();
        app.update();
        let v = value(&app, "charge");
        assert!(v.ongoing && approx(v.held_time, 0.1), "premise: holding: {v:?}");
        app.world_mut()
            .resource_mut::<InputActions>()
            .set_context_enabled("combat", false)
            .unwrap();
        app.update();
        let v = value(&app, "charge");
        assert!(v.canceled && !v.triggered && !v.pressed, "{v:?}");
    }

    /// One key, two actions: a short press is a dodge, a long one a sprint,
    /// and neither fires for the other's press.
    #[test]
    fn a_tap_and_a_hold_share_one_key() {
        let both = || {
            actions_from(
                vec![
                    ("dodge", mapping(&["ShiftLeft"], Some("tap"), Some(0.2))),
                    ("sprint", mapping(&["ShiftLeft"], Some("hold"), Some(0.2))),
                ],
                vec![],
            )
            .unwrap()
        };
        let fired = |app: &bevy_app::App| (value(app, "dodge").triggered, value(app, "sprint").triggered);

        let mut app = timed_app(both(), true);
        key(&mut app, KeyCode::ShiftLeft, ElementState::Pressed);
        app.update();
        key(&mut app, KeyCode::ShiftLeft, ElementState::Released);
        app.update();
        assert_eq!(fired(&app), (true, false), "a 0.1 s press dodges");
        assert!(value(&app, "sprint").canceled);

        let mut app = timed_app(both(), true);
        key(&mut app, KeyCode::ShiftLeft, ElementState::Pressed);
        let mut frames = Vec::new();
        for _ in 0..5 {
            app.update();
            frames.push(fired(&app));
        }
        assert_eq!(
            frames,
            [(false, false), (false, false), (false, true), (false, false), (false, false)],
            "sprint fires once at 0.2 s; dodge never"
        );
        assert!(value(&app, "dodge").held_time > 0.2, "premise: held past the tap");
    }
}

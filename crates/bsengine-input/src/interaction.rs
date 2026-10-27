//! Interactions: when a press *counts*, beyond "it is held".
//!
//! Unity's Input System (Hold, Tap, SlowTap) and Unreal's Enhanced Input
//! (Hold, Tap, HoldAndRelease triggers) agree on three: fire after the press
//! has lasted a while, fire on a short press, fire on a release after a long
//! one. They also agree on the phases a script sees -- started/ongoing,
//! fired, canceled -- so a charge bar can fill while `ongoing` and empty on
//! `canceled`. Godot has neither. Those three, and those phases, are what
//! this module is; MultiTap, Pulse and Chorded (each in one engine only) are
//! not.
//!
//! Time is measured in frames' `delta_seconds`, not wall-clock instants, so a
//! replay on a fixed clock (`Time::fixed`) times every press the same way it
//! did when it was recorded.

/// How an action's press turns into a `triggered` frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Interaction {
    /// Fires once, on the frame the press has lasted `time` seconds. Let go
    /// earlier and it is canceled. (Unity Hold; Unreal Hold, one-shot.)
    Hold {
        /// Seconds the press must last.
        time: f32,
    },
    /// Fires on the release of a press that lasted at most `time` seconds;
    /// one held longer is canceled while still held. (Unity Tap; Unreal Tap.)
    Tap {
        /// The longest press that still counts as a tap.
        time: f32,
    },
    /// Fires on the release of a press that lasted at least `time` seconds;
    /// one let go sooner is canceled. (Unity SlowTap; Unreal HoldAndRelease.)
    HoldAndRelease {
        /// Seconds the press must last before its release counts.
        time: f32,
    },
}

const NAMES: &[&str] = &["hold", "tap", "hold_and_release"];

impl Interaction {
    /// Builds an interaction from `project.toml`'s `interaction` and `time`
    /// keys. Neither key is no interaction; anything else that is not one of
    /// [`NAMES`] with a positive, finite time is an error that says which.
    pub fn parse(name: Option<&str>, time: Option<f32>) -> Result<Option<Interaction>, String> {
        let Some(name) = name else {
            return match time {
                None => Ok(None),
                Some(_) => Err("\"time\" without an \"interaction\" has nothing to time".to_string()),
            };
        };
        let make: fn(f32) -> Interaction = match name {
            "hold" => |time| Interaction::Hold { time },
            "tap" => |time| Interaction::Tap { time },
            "hold_and_release" => |time| Interaction::HoldAndRelease { time },
            _ => {
                return Err(format!(
                    "unknown interaction {name:?}; expected one of {}",
                    NAMES.join(", ")
                ))
            }
        };
        match time {
            None => Err(format!("interaction {name:?} needs a \"time\" in seconds")),
            Some(t) if !(t.is_finite() && t > 0.0) => Err(format!(
                "interaction {name:?}: \"time\" must be a positive number of seconds, got {t}"
            )),
            Some(t) => Ok(Some(make(t))),
        }
    }

    /// The name `project.toml` spells it with.
    pub fn name(&self) -> &'static str {
        match self {
            Interaction::Hold { .. } => "hold",
            Interaction::Tap { .. } => "tap",
            Interaction::HoldAndRelease { .. } => "hold_and_release",
        }
    }

    /// Its threshold in seconds.
    pub fn time(&self) -> f32 {
        match *self {
            Interaction::Hold { time }
            | Interaction::Tap { time }
            | Interaction::HoldAndRelease { time } => time,
        }
    }
}

/// One frame of an interaction's output.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Step {
    pub triggered: bool,
    pub ongoing: bool,
    pub canceled: bool,
    pub held_time: f32,
}

/// One action's press timing, carried from frame to frame.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Tracker {
    /// Seconds the current press has lasted, as of the last frame.
    held: f32,
    /// This press already fired or was canceled: nothing more until it ends.
    /// Without it a hold would fire on every frame past its threshold.
    done: bool,
    /// The interaction the current press is under, kept for its release: a
    /// press can end *because* its mapping went away (its context was
    /// disabled), and then the frame that ends it has no interaction to ask
    /// -- the hold in progress would vanish instead of being canceled.
    pressed_under: Option<Interaction>,
}

impl Tracker {
    /// Advances one frame. `was` is last frame's `pressed`; `tapped` is a
    /// press that began this frame (which, with `!pressed`, is a press that
    /// began *and ended* between two frames -- `Input` reports only the
    /// edges, and that press lasted less than a frame, so it counts as 0 s).
    ///
    /// A press that ends this frame is credited with this frame's `dt` too:
    /// it was seen held at the last frame and let go some time before this
    /// one, and `held + dt` is the length the frames can see. A hold whose
    /// threshold falls inside that last frame is still canceled, though -- it
    /// never fired while held, and firing on the release would make it a
    /// hold-and-release.
    pub(crate) fn step(
        &mut self,
        interaction: Option<Interaction>,
        was: bool,
        pressed: bool,
        tapped: bool,
        dt: f32,
    ) -> Step {
        let interaction = if pressed {
            interaction
        } else {
            interaction.or(self.pressed_under)
        };
        self.pressed_under = if pressed { interaction } else { None };
        let just_pressed = (pressed && !was) || tapped;
        let released = (was && !pressed) || (tapped && !pressed);
        let duration = if was { self.held + dt } else { 0.0 };
        let held = match (pressed, was) {
            (true, true) => self.held + dt,
            _ => 0.0,
        };
        let mut out = Step {
            held_time: held,
            ..Step::default()
        };
        match interaction {
            None => out.triggered = just_pressed,
            Some(Interaction::Hold { time }) => {
                if pressed && !self.done {
                    if held >= time {
                        out.triggered = true;
                        self.done = true;
                    } else {
                        out.ongoing = true;
                    }
                } else if released && !self.done {
                    out.canceled = true;
                }
            }
            Some(Interaction::Tap { time }) => {
                if pressed && !self.done {
                    if held > time {
                        out.canceled = true;
                        self.done = true;
                    } else {
                        out.ongoing = true;
                    }
                } else if released && !self.done {
                    if duration <= time {
                        out.triggered = true;
                    } else {
                        out.canceled = true;
                    }
                }
            }
            Some(Interaction::HoldAndRelease { time }) => {
                if pressed {
                    out.ongoing = true;
                } else if released {
                    if duration >= time {
                        out.triggered = true;
                    } else {
                        out.canceled = true;
                    }
                }
            }
        }
        if !pressed {
            self.done = false;
        }
        self.held = held;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 0.1;

    /// Runs `frames` of `(pressed, tapped)` through one tracker at a fixed
    /// 0.1 s step. Returns every frame's step.
    fn run(interaction: Option<Interaction>, frames: &[(bool, bool)]) -> Vec<Step> {
        let mut tracker = Tracker::default();
        let mut was = false;
        frames
            .iter()
            .map(|&(pressed, tapped)| {
                let step = tracker.step(interaction, was, pressed, tapped, DT);
                was = pressed;
                step
            })
            .collect()
    }

    /// A press that begins on frame 0 and is held for `held` frames, then
    /// released, then `after` idle frames.
    fn press(held: usize, after: usize) -> Vec<(bool, bool)> {
        let mut frames = vec![(true, true)];
        frames.extend(std::iter::repeat((true, false)).take(held));
        frames.extend(std::iter::repeat((false, false)).take(after + 1));
        frames
    }

    fn fired(steps: &[Step]) -> Vec<usize> {
        steps.iter().enumerate().filter(|(_, s)| s.triggered).map(|(i, _)| i).collect()
    }

    fn canceled(steps: &[Step]) -> Vec<usize> {
        steps.iter().enumerate().filter(|(_, s)| s.canceled).map(|(i, _)| i).collect()
    }

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    /// The hard side of a hold is *once*: it fires on the frame the press
    /// reaches 0.3 s and on none of the frames held after that -- an
    /// implementation that fires "while held past the threshold" fails here.
    #[test]
    fn a_hold_fires_once_on_the_frame_the_press_reaches_its_time() {
        let steps = run(Some(Interaction::Hold { time: 0.3 }), &press(9, 2));
        assert!(
            approx(steps[3].held_time, 0.3),
            "premise: frame 3 is the first at 0.3 s, got {}",
            steps[3].held_time
        );
        assert_eq!(fired(&steps), vec![3], "{steps:?}");
        assert!(canceled(&steps).is_empty(), "a hold that fired is not canceled on release");
        assert!(steps[..3].iter().all(|s| s.ongoing), "ongoing until it fires");
        assert!(steps[3..].iter().all(|s| !s.ongoing), "and not after");
    }

    #[test]
    fn a_hold_let_go_early_is_canceled_and_never_fires() {
        let steps = run(Some(Interaction::Hold { time: 0.3 }), &press(1, 2));
        assert!(approx(steps[1].held_time, 0.1), "premise: held 0.1 s");
        assert!(fired(&steps).is_empty(), "{steps:?}");
        assert_eq!(canceled(&steps), vec![2], "canceled on the release frame");
    }

    /// Released on the frame its press reaches the threshold: held 0.1 s as
    /// last seen, plus this frame's 0.1 -- exactly the tap's 0.2, which still
    /// counts. `<` instead of `<=` fails here.
    #[test]
    fn a_tap_released_within_its_time_fires_on_the_release() {
        let steps = run(Some(Interaction::Tap { time: 0.2 }), &press(1, 1));
        assert!(approx(steps[1].held_time, 0.1), "premise: last seen held at 0.1 s");
        assert_eq!(fired(&steps), vec![2], "{steps:?}");
        assert!(canceled(&steps).is_empty());
        assert!(steps[0].ongoing && steps[1].ongoing && !steps[2].ongoing);
    }

    /// Held past its time, a tap is canceled *while still held* (so the game
    /// can switch to the hold action at once), and the release adds nothing.
    #[test]
    fn a_tap_held_too_long_is_canceled_while_held_and_silent_on_release() {
        let steps = run(Some(Interaction::Tap { time: 0.2 }), &press(5, 2));
        assert_eq!(canceled(&steps), vec![3], "the first frame past 0.2 s: {steps:?}");
        assert!(approx(steps[3].held_time, 0.3), "premise: frame 3 is at 0.3 s");
        assert!(fired(&steps).is_empty(), "the release does not fire it");
    }

    /// Pressed and released between two frames: the press lasted under a
    /// frame, so it is the shortest press there is.
    #[test]
    fn a_press_inside_one_frame_is_a_tap_and_too_short_for_the_others() {
        let frames = [(false, false), (false, true), (false, false)];
        let tap = run(Some(Interaction::Tap { time: 0.2 }), &frames);
        assert_eq!(fired(&tap), vec![1], "{tap:?}");
        let hold = run(Some(Interaction::Hold { time: 0.2 }), &frames);
        assert!(fired(&hold).is_empty() && canceled(&hold) == vec![1], "{hold:?}");
        let slow = run(Some(Interaction::HoldAndRelease { time: 0.2 }), &frames);
        assert!(fired(&slow).is_empty() && canceled(&slow) == vec![1], "{slow:?}");
        let none = run(None, &frames);
        assert_eq!(fired(&none), vec![1], "no interaction: the press edge");
    }

    /// The release of a press of exactly its time counts (`>=`), one frame
    /// shorter does not, and it is ongoing for the whole press either way --
    /// it does not stop at the threshold as a hold does.
    #[test]
    fn hold_and_release_fires_on_releasing_a_long_enough_press() {
        let long = run(Some(Interaction::HoldAndRelease { time: 0.3 }), &press(2, 1));
        assert!(approx(long[2].held_time, 0.2), "premise: 0.2 s seen, released at 0.3");
        assert_eq!(fired(&long), vec![3], "{long:?}");
        assert!(long[..3].iter().all(|s| s.ongoing));

        let short = run(Some(Interaction::HoldAndRelease { time: 0.3 }), &press(1, 1));
        assert!(fired(&short).is_empty(), "{short:?}");
        assert_eq!(canceled(&short), vec![2]);

        let held_on = run(Some(Interaction::HoldAndRelease { time: 0.3 }), &press(8, 0));
        assert!(held_on[..9].iter().all(|s| s.ongoing && !s.triggered), "{held_on:?}");
    }

    /// No interaction: `triggered` is the press edge, nothing is ongoing or
    /// canceled.
    #[test]
    fn without_an_interaction_triggered_is_the_press_edge() {
        let steps = run(None, &press(3, 1));
        assert_eq!(fired(&steps), vec![0]);
        assert!(steps.iter().all(|s| !s.ongoing && !s.canceled));
    }

    /// Two presses in a row each fire: what one press marked done must not
    /// carry into the next. Also the clock really is `dt` -- held time grows
    /// by exactly 0.1 a frame and returns to 0 on release.
    #[test]
    fn each_press_starts_over() {
        let mut frames = press(4, 0);
        frames.extend(press(4, 0));
        let steps = run(Some(Interaction::Hold { time: 0.2 }), &frames);
        assert_eq!(fired(&steps), vec![2, 8], "{steps:?}");
        let times: Vec<f32> = steps[..6].iter().map(|s| s.held_time).collect();
        assert!(
            [0.0, 0.1, 0.2, 0.3, 0.4, 0.0]
                .iter()
                .zip(&times)
                .all(|(a, b)| approx(*a, *b)),
            "{times:?}"
        );
    }

    #[test]
    fn parse_names_what_is_wrong() {
        assert_eq!(Interaction::parse(None, None), Ok(None));
        assert_eq!(
            Interaction::parse(Some("tap"), Some(0.2)),
            Ok(Some(Interaction::Tap { time: 0.2 }))
        );
        for (name, time, says) in [
            (None, Some(0.2), "without an \"interaction\""),
            (Some("press"), Some(0.2), "unknown interaction \"press\""),
            (Some("hold"), None, "needs a \"time\""),
            (Some("hold"), Some(0.0), "positive"),
            (Some("tap"), Some(-1.0), "positive"),
            (Some("tap"), Some(f32::NAN), "positive"),
        ] {
            let err = Interaction::parse(name, time).expect_err(says);
            assert!(err.contains(says), "{name:?} {time:?}: {err}");
        }
        for name in NAMES {
            let parsed = Interaction::parse(Some(name), Some(1.0)).unwrap().unwrap();
            assert_eq!(parsed.name(), *name, "every name round-trips");
            assert_eq!(parsed.time(), 1.0);
        }
    }
}

use bevy_ecs::prelude::Resource;

/// How the game's UI is sized against the window: `project.toml`'s
/// `[ui] scale`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiScaleMode {
    /// The UI is authored for the `[window]` size and grows or shrinks with
    /// the window's *shorter* side, so what fits at the design size fits
    /// everywhere. The default.
    Reference,
    /// A fixed multiplier, whatever the window.
    Fixed(f32),
    /// 1:1 with the window's pixels, as the UI was before scaling existed.
    Off,
}

/// The UI scale rule and the size it is authored for. Inserted by the runtime
/// from `project.toml` before the plugins; read by the renderer each frame to
/// make [`UiScaleFactor`].
///
/// All three reference engines scale game UI against a design resolution on
/// phones, none by the display's DPI: Unity's Canvas Scaler "Scale With Screen
/// Size", Godot's `canvas_items` stretch with a base size, Unreal's DPI curve
/// on the shortest side. A layout authored at 1280x720 and scaled by density
/// instead overflows a landscape phone, which is 411 dp tall. Unreal's rule,
/// the shortest side, is used because a phone's rotation then keeps the scale:
/// 1080x2400 and 2400x1080 are both x1.5 for a 720-side design.
///
/// On by default on every platform (a decision, not a convention: Unity and
/// Godot default to no scaling). A desktop window dragged larger therefore
/// grows its UI with it; `[ui] scale = "none"` is the old behaviour.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct UiScale {
    /// The rule.
    pub mode: UiScaleMode,
    /// The size the UI is authored for, in pixels: `[window] width`/`height`.
    pub reference: (f32, f32),
}

impl Default for UiScale {
    fn default() -> Self {
        // `[window]`'s own defaults.
        Self {
            mode: UiScaleMode::Reference,
            reference: (1280.0, 720.0),
        }
    }
}

impl UiScale {
    /// The multiplier for a window of this size in pixels.
    ///
    /// A zero-sized window (minimised) or a design with a zero side is 1, not
    /// 0 or infinity: either would make every widget vanish or explode for
    /// the frames it lasts.
    pub fn factor(&self, window_w: f32, window_h: f32) -> UiScaleFactor {
        let factor = match self.mode {
            UiScaleMode::Off => 1.0,
            UiScaleMode::Fixed(f) => f,
            UiScaleMode::Reference => {
                let window = window_w.min(window_h);
                let design = self.reference.0.min(self.reference.1);
                if window > 0.0 && design > 0.0 {
                    window / design
                } else {
                    1.0
                }
            }
        };
        UiScaleFactor(factor)
    }
}

/// The multiplier in force this frame: the UI's size in physical pixels over
/// its size in the units scripts author in. Its own type, with a default of 1,
/// because [`crate::UiState`] derives `Default` and a bare `f32` would make a
/// new `UiState` scale everything by 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiScaleFactor(pub f32);

impl Default for UiScaleFactor {
    fn default() -> Self {
        Self(1.0)
    }
}

impl UiScaleFactor {
    /// The multiplier, never zero, negative or not a number: those read as 1.
    pub fn get(self) -> f32 {
        if self.0.is_finite() && self.0 > 0.0 {
            self.0
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_scales_by_the_shorter_side() {
        let scale = UiScale::default();
        // The design size itself, and the phone from the issue, upright and
        // on its side: the same x1.5 either way.
        assert_eq!(scale.factor(1280.0, 720.0).get(), 1.0);
        assert_eq!(scale.factor(1080.0, 2400.0).get(), 1.5);
        assert_eq!(scale.factor(2400.0, 1080.0).get(), 1.5);
        assert_eq!(scale.factor(2560.0, 1440.0).get(), 2.0);
        // Smaller than the design shrinks it.
        assert_eq!(scale.factor(640.0, 360.0).get(), 0.5);
    }

    #[test]
    fn a_square_design_and_an_uneven_one_use_their_shorter_sides() {
        let scale = UiScale {
            reference: (1000.0, 500.0),
            ..UiScale::default()
        };
        assert_eq!(scale.factor(500.0, 2000.0).get(), 1.0);
    }

    #[test]
    fn off_and_fixed_ignore_the_window() {
        let off = UiScale {
            mode: UiScaleMode::Off,
            ..UiScale::default()
        };
        let fixed = UiScale {
            mode: UiScaleMode::Fixed(2.5),
            ..UiScale::default()
        };
        for (w, h) in [(1280.0, 720.0), (1080.0, 2400.0), (50.0, 50.0)] {
            assert_eq!(off.factor(w, h).get(), 1.0);
            assert_eq!(fixed.factor(w, h).get(), 2.5);
        }
    }

    /// A minimised window is 0x0 for a while; nothing may scale by 0 or by
    /// infinity because of it. The premise: the same window is not 1 once it
    /// has a size.
    #[test]
    fn degenerate_sizes_and_factors_read_as_one() {
        let scale = UiScale::default();
        assert_ne!(scale.factor(1080.0, 2400.0).get(), 1.0, "premise");
        assert_eq!(scale.factor(0.0, 0.0).get(), 1.0);
        assert_eq!(scale.factor(0.0, 720.0).get(), 1.0);
        let no_design = UiScale {
            reference: (0.0, 720.0),
            ..scale
        };
        assert_eq!(no_design.factor(1080.0, 2400.0).get(), 1.0);
        for bad in [0.0, -2.0, f32::NAN, f32::INFINITY] {
            assert_eq!(UiScaleFactor(bad).get(), 1.0);
        }
        assert_eq!(UiScaleFactor::default().get(), 1.0);
    }
}

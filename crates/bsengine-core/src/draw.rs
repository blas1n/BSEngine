use bevy_ecs::prelude::Component;

/// Phase of a weapon draw / sheathe cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawPhase {
    /// Weapon is sheathed (away).
    Sheathed,
    /// Transitioning from Sheathed to Drawn.
    Drawing,
    /// Weapon is fully drawn and ready.
    Drawn,
    /// Transitioning from Drawn back to Sheathed.
    Sheathing,
}

/// Weapon draw / sheathe state component.
///
/// Tracks whether a weapon (or tool, shield, etc.) is deployed. Drive
/// animation blending via `transition_fraction()`.
///
/// `tick(dt)` handles:
/// - Drawing → Drawn / Sheathing → Sheathed transitions
/// - Auto-sheathe after `auto_sheathe_delay` seconds of inactivity
///   (`auto_sheathe_delay == 0.0` disables auto-sheathe)
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Draw {
    pub phase: DrawPhase,
    /// Time in seconds the Drawing transition takes.
    pub draw_duration: f32,
    /// Time in seconds the Sheathing transition takes.
    pub sheathe_duration: f32,
    pub draw_timer: f32,
    pub sheathe_timer: f32,
    /// Seconds of inactivity before auto-sheathing (0 = disabled).
    pub auto_sheathe_delay: f32,
    /// Tracks time since last draw/use event for auto-sheathe.
    pub inactivity_timer: f32,
    /// True on the frame the weapon becomes fully Drawn.
    pub just_drawn: bool,
    /// True on the frame the weapon becomes fully Sheathed.
    pub just_sheathed: bool,
    pub enabled: bool,
}

impl Draw {
    pub fn new(draw_duration: f32, sheathe_duration: f32) -> Self {
        Self {
            phase: DrawPhase::Sheathed,
            draw_duration: draw_duration.max(0.0),
            sheathe_duration: sheathe_duration.max(0.0),
            draw_timer: 0.0,
            sheathe_timer: 0.0,
            auto_sheathe_delay: 0.0,
            inactivity_timer: 0.0,
            just_drawn: false,
            just_sheathed: false,
            enabled: true,
        }
    }

    pub fn with_auto_sheathe(mut self, delay: f32) -> Self {
        self.auto_sheathe_delay = delay.max(0.0);
        self
    }

    /// Start with the weapon already drawn (e.g. a character that enters
    /// combat with their weapon out).
    pub fn pre_drawn(mut self) -> Self {
        self.phase = DrawPhase::Drawn;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Begin drawing the weapon. No-op if already drawn or drawing.
    pub fn draw(&mut self) {
        if !self.enabled {
            return;
        }
        match self.phase {
            DrawPhase::Sheathed => {
                self.phase = DrawPhase::Drawing;
                self.draw_timer = self.draw_duration;
                self.inactivity_timer = 0.0;
            }
            DrawPhase::Sheathing => {
                // Reverse mid-sheathe: fraction of sheathe progress becomes draw time.
                let fraction = if self.sheathe_duration > 0.0 {
                    self.sheathe_timer / self.sheathe_duration
                } else {
                    0.0
                };
                self.phase = DrawPhase::Drawing;
                self.draw_timer = self.draw_duration * fraction;
                self.inactivity_timer = 0.0;
            }
            _ => {}
        }
    }

    /// Begin sheathing the weapon. No-op if already sheathed or sheathing.
    pub fn sheathe(&mut self) {
        if !self.enabled {
            return;
        }
        match self.phase {
            DrawPhase::Drawn => {
                self.phase = DrawPhase::Sheathing;
                self.sheathe_timer = self.sheathe_duration;
            }
            DrawPhase::Drawing => {
                let fraction = if self.draw_duration > 0.0 {
                    1.0 - self.draw_timer / self.draw_duration
                } else {
                    1.0
                };
                self.phase = DrawPhase::Sheathing;
                self.sheathe_timer = self.sheathe_duration * fraction;
            }
            _ => {}
        }
    }

    /// Signal weapon activity (attack, aim, etc.) — resets the auto-sheathe timer.
    pub fn notify_use(&mut self) {
        self.inactivity_timer = 0.0;
    }

    /// Advance timers. Call once per frame with the frame delta-time.
    pub fn tick(&mut self, dt: f32) {
        self.just_drawn = false;
        self.just_sheathed = false;

        match self.phase {
            DrawPhase::Drawing => {
                self.draw_timer = (self.draw_timer - dt).max(0.0);
                if self.draw_timer <= 0.0 {
                    self.phase = DrawPhase::Drawn;
                    self.just_drawn = true;
                }
            }
            DrawPhase::Sheathing => {
                self.sheathe_timer = (self.sheathe_timer - dt).max(0.0);
                if self.sheathe_timer <= 0.0 {
                    self.phase = DrawPhase::Sheathed;
                    self.just_sheathed = true;
                }
            }
            DrawPhase::Drawn => {
                if self.auto_sheathe_delay > 0.0 {
                    self.inactivity_timer += dt;
                    if self.inactivity_timer >= self.auto_sheathe_delay {
                        self.sheathe();
                        // Complete zero-duration sheathe within the same tick.
                        if self.phase == DrawPhase::Sheathing && self.sheathe_timer <= 0.0 {
                            self.phase = DrawPhase::Sheathed;
                            self.just_sheathed = true;
                        }
                    }
                }
            }
            DrawPhase::Sheathed => {}
        }
    }

    pub fn is_drawn(&self) -> bool {
        self.phase == DrawPhase::Drawn
    }

    pub fn is_busy(&self) -> bool {
        matches!(self.phase, DrawPhase::Drawing | DrawPhase::Sheathing)
    }

    /// [0.0, 1.0]: 0 = fully sheathed, 1 = fully drawn. Drives animation blend.
    pub fn transition_fraction(&self) -> f32 {
        match self.phase {
            DrawPhase::Sheathed => 0.0,
            DrawPhase::Drawing => {
                if self.draw_duration > 0.0 {
                    1.0 - self.draw_timer / self.draw_duration
                } else {
                    1.0
                }
            }
            DrawPhase::Drawn => 1.0,
            DrawPhase::Sheathing => {
                if self.sheathe_duration > 0.0 {
                    self.sheathe_timer / self.sheathe_duration
                } else {
                    0.0
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw() -> Draw {
        Draw::new(0.4, 0.3)
    }

    #[test]
    fn draw_transitions_to_drawn() {
        let mut d = draw();
        d.draw();
        assert_eq!(d.phase, DrawPhase::Drawing);
        d.tick(0.4);
        assert_eq!(d.phase, DrawPhase::Drawn);
        assert!(d.just_drawn);
    }

    #[test]
    fn sheathe_transitions_to_sheathed() {
        let mut d = draw().pre_drawn();
        d.sheathe();
        d.tick(0.3);
        assert_eq!(d.phase, DrawPhase::Sheathed);
        assert!(d.just_sheathed);
    }

    #[test]
    fn auto_sheathe_triggers_after_delay() {
        let mut d = Draw::new(0.0, 0.0).with_auto_sheathe(1.0).pre_drawn();
        d.tick(0.6);
        assert_eq!(d.phase, DrawPhase::Drawn); // not yet
        d.tick(0.5);
        // auto-sheathe triggered; with 0-duration sheathe, should be Sheathed
        assert_eq!(d.phase, DrawPhase::Sheathed);
    }

    #[test]
    fn notify_use_resets_auto_sheathe_timer() {
        let mut d = Draw::new(0.0, 0.0).with_auto_sheathe(1.0).pre_drawn();
        d.tick(0.8);
        d.notify_use();
        d.tick(0.8); // would have triggered at 1.0 without reset
        assert_eq!(d.phase, DrawPhase::Drawn);
    }

    #[test]
    fn transition_fraction_is_correct() {
        let mut d = draw();
        assert!((d.transition_fraction() - 0.0).abs() < 1e-5);
        d.draw();
        d.tick(0.2); // half of draw_duration 0.4
        assert!((d.transition_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn disabled_ignores_draw() {
        let mut d = draw().disabled();
        d.draw();
        assert_eq!(d.phase, DrawPhase::Sheathed);
    }
}

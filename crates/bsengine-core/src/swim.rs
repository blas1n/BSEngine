use bevy_ecs::prelude::Component;

/// Current locomotion state in water.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwimState {
    /// Out of water or not in a swimming context.
    Dry,
    /// At the water surface; can breathe.
    Surface,
    /// Fully submerged; breath drains.
    Submerged,
    /// Ascending rapidly toward the surface.
    Surfacing,
}

/// Swimming / underwater locomotion component.
///
/// Attach alongside `Buoyancy`. The movement system checks `state` each frame:
///   - `Surface` → apply swim velocity capped by `swim_speed`
///   - `Submerged` → apply dive velocity; drain `breath_remaining` each dt
///   - `Surfacing` → apply upward burst until at surface
///   - `Dry` → no swim behaviour; normal ground movement resumes
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Swim {
    pub state: SwimState,
    /// Top speed while swimming horizontally at the surface (m/s).
    pub swim_speed: f32,
    /// Top speed while swimming downward when submerged (m/s).
    pub dive_speed: f32,
    /// Upward ascent speed when surfacing (m/s).
    pub ascent_speed: f32,
    /// Remaining breath (seconds of air).
    pub breath_remaining: f32,
    /// Maximum breath capacity (seconds).
    pub max_breath: f32,
    /// Rate at which breath drains while submerged (seconds per second, usually 1.0).
    pub breath_drain_rate: f32,
    /// Rate at which breath refills while at the surface (seconds per second).
    pub breath_regen_rate: f32,
    /// Current water depth of the entity (written by the buoyancy/physics system, metres).
    pub depth: f32,
    /// Depth threshold below which the entity is considered fully submerged.
    pub submerge_depth: f32,
    /// True when the character wants to dive (input flag).
    pub wants_dive: bool,
    /// True when the character wants to surface (input flag).
    pub wants_surface: bool,
    pub enabled: bool,
}

impl Swim {
    pub fn new(swim_speed: f32, max_breath: f32) -> Self {
        Self {
            state: SwimState::Dry,
            swim_speed: swim_speed.max(0.0),
            dive_speed: swim_speed * 0.6,
            ascent_speed: swim_speed * 0.8,
            breath_remaining: max_breath,
            max_breath: max_breath.max(0.0),
            breath_drain_rate: 1.0,
            breath_regen_rate: 2.0,
            depth: 0.0,
            submerge_depth: 0.5,
            wants_dive: false,
            wants_surface: false,
            enabled: true,
        }
    }

    pub fn with_dive_speed(mut self, s: f32) -> Self {
        self.dive_speed = s.max(0.0);
        self
    }

    pub fn with_breath_rates(mut self, drain: f32, regen: f32) -> Self {
        self.breath_drain_rate = drain.max(0.0);
        self.breath_regen_rate = regen.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Enter the water. Call when the physics system detects water entry.
    pub fn enter_water(&mut self) {
        if self.enabled && self.state == SwimState::Dry {
            self.state = SwimState::Surface;
        }
    }

    /// Exit the water. Call when the character leaves the water volume.
    pub fn exit_water(&mut self) {
        self.state = SwimState::Dry;
    }

    /// Advance breath and state transitions each frame.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.state == SwimState::Dry {
            return;
        }

        let submerged = self.depth >= self.submerge_depth;

        match self.state {
            SwimState::Dry => {}
            SwimState::Surface => {
                // Refill breath at surface.
                self.breath_remaining =
                    (self.breath_remaining + self.breath_regen_rate * dt).min(self.max_breath);
                if submerged || self.wants_dive {
                    self.state = SwimState::Submerged;
                }
            }
            SwimState::Submerged => {
                // Drain breath while under.
                self.breath_remaining =
                    (self.breath_remaining - self.breath_drain_rate * dt).max(0.0);
                if self.wants_surface || !submerged {
                    self.state = SwimState::Surfacing;
                }
            }
            SwimState::Surfacing => {
                if !submerged {
                    self.state = SwimState::Surface;
                }
            }
        }
    }

    pub fn is_submerged(&self) -> bool {
        self.state == SwimState::Submerged
    }

    pub fn is_in_water(&self) -> bool {
        self.state != SwimState::Dry
    }

    pub fn breath_fraction(&self) -> f32 {
        if self.max_breath <= 0.0 {
            return 1.0;
        }
        (self.breath_remaining / self.max_breath).clamp(0.0, 1.0)
    }

    pub fn is_drowning(&self) -> bool {
        self.is_submerged() && self.breath_remaining <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_water_sets_surface_state() {
        let mut s = Swim::new(5.0, 30.0);
        s.enter_water();
        assert_eq!(s.state, SwimState::Surface);
    }

    #[test]
    fn surface_tick_regens_breath() {
        let mut s = Swim::new(5.0, 30.0);
        s.enter_water();
        s.breath_remaining = 10.0;
        s.tick(1.0);
        assert!(s.breath_remaining > 10.0);
    }

    #[test]
    fn submerged_tick_drains_breath() {
        let mut s = Swim::new(5.0, 30.0);
        s.enter_water();
        s.state = SwimState::Submerged;
        let before = s.breath_remaining;
        s.tick(1.0);
        assert!(s.breath_remaining < before);
    }

    #[test]
    fn wants_dive_transitions_to_submerged() {
        let mut s = Swim::new(5.0, 30.0);
        s.enter_water();
        s.wants_dive = true;
        s.tick(0.016);
        assert!(s.is_submerged());
    }

    #[test]
    fn drowning_detected_when_breath_empty() {
        let mut s = Swim::new(5.0, 30.0);
        s.enter_water();
        s.state = SwimState::Submerged;
        s.breath_remaining = 0.0;
        assert!(s.is_drowning());
    }

    #[test]
    fn exit_water_clears_state() {
        let mut s = Swim::new(5.0, 30.0);
        s.enter_water();
        s.exit_water();
        assert_eq!(s.state, SwimState::Dry);
        assert!(!s.is_in_water());
    }
}

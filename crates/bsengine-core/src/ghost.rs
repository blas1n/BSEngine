use bevy_ecs::prelude::Component;

/// Controls how the entity returns from ghost state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhostExitMode {
    /// Automatically exit after `duration` seconds.
    TimedOut,
    /// Stay in ghost state until explicitly disabled.
    Manual,
}

/// Intangibility / phase-through component.
///
/// While `is_ghost` is true the physics system clears collisions against layers covered by
/// `phase_mask`, making the entity pass through walls, floors, or enemies selectively.
/// The render system may also reduce `alpha` to visually indicate the ghosted state.
///
/// The ability system calls `activate()` to enter ghost mode and `tick(dt)` each frame.
/// On timed exit the component sets `is_ghost = false` and resets itself.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ghost {
    /// Whether the entity is currently intangible.
    pub is_ghost: bool,
    /// Collision layer mask to ignore while ghosted (bit flags matching the `Layer` component).
    pub phase_mask: u32,
    /// Duration of the ghost window (seconds). Ignored when exit_mode is Manual.
    pub duration: f32,
    /// Remaining time in ghost mode.
    pub remaining: f32,
    /// Accumulated total ghost time this session.
    pub total_elapsed: f32,
    /// How the effect ends.
    pub exit_mode: GhostExitMode,
    /// Render alpha while ghosted [0, 1]. The render system blends toward this.
    pub ghost_alpha: f32,
    /// Cooldown before the ability can be activated again (seconds).
    pub cooldown: f32,
    /// Countdown on cooldown.
    pub cooldown_timer: f32,
    pub enabled: bool,
}

impl Ghost {
    pub fn new(duration: f32, phase_mask: u32) -> Self {
        Self {
            is_ghost: false,
            phase_mask,
            duration: duration.max(0.0),
            remaining: 0.0,
            total_elapsed: 0.0,
            exit_mode: GhostExitMode::TimedOut,
            ghost_alpha: 0.3,
            cooldown: 0.0,
            cooldown_timer: 0.0,
            enabled: true,
        }
    }

    pub fn manual(phase_mask: u32) -> Self {
        Self {
            exit_mode: GhostExitMode::Manual,
            ..Self::new(0.0, phase_mask)
        }
    }

    pub fn with_ghost_alpha(mut self, alpha: f32) -> Self {
        self.ghost_alpha = alpha.clamp(0.0, 1.0);
        self
    }

    pub fn with_cooldown(mut self, cooldown: f32) -> Self {
        self.cooldown = cooldown.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Activate ghost mode. Returns false if on cooldown, disabled, or already active.
    pub fn activate(&mut self) -> bool {
        if !self.enabled || self.is_ghost || self.cooldown_timer > 0.0 {
            return false;
        }
        self.is_ghost = true;
        self.remaining = self.duration;
        true
    }

    /// Manually deactivate (no-op for TimedOut mode — let `tick` handle it).
    pub fn deactivate(&mut self) {
        self.is_ghost = false;
        self.remaining = 0.0;
        self.cooldown_timer = self.cooldown;
    }

    /// Advance timers. Returns true if ghost mode ended this frame.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.cooldown_timer > 0.0 {
            self.cooldown_timer = (self.cooldown_timer - dt).max(0.0);
        }

        if !self.is_ghost {
            return false;
        }

        self.total_elapsed += dt;

        if self.exit_mode == GhostExitMode::TimedOut {
            self.remaining -= dt;
            if self.remaining <= 0.0 {
                self.deactivate();
                return true;
            }
        }
        false
    }

    pub fn is_on_cooldown(&self) -> bool {
        self.cooldown_timer > 0.0
    }

    /// Fraction of remaining duration [0, 1], or 1 if manual mode.
    pub fn remaining_fraction(&self) -> f32 {
        if self.exit_mode == GhostExitMode::Manual || self.duration <= 0.0 {
            1.0
        } else {
            (self.remaining / self.duration).clamp(0.0, 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_enters_ghost_mode() {
        let mut g = Ghost::new(1.0, u32::MAX);
        let ok = g.activate();
        assert!(ok);
        assert!(g.is_ghost);
    }

    #[test]
    fn tick_expires_ghost_mode() {
        let mut g = Ghost::new(0.5, u32::MAX);
        g.activate();
        let ended = g.tick(0.6);
        assert!(ended);
        assert!(!g.is_ghost);
    }

    #[test]
    fn double_activate_fails() {
        let mut g = Ghost::new(1.0, u32::MAX);
        g.activate();
        let ok = g.activate();
        assert!(!ok);
    }

    #[test]
    fn cooldown_prevents_reactivation() {
        let mut g = Ghost::new(0.1, u32::MAX).with_cooldown(1.0);
        g.activate();
        g.tick(0.2); // ghost ends, starts cooldown
        let ok = g.activate();
        assert!(!ok);
        assert!(g.is_on_cooldown());
    }

    #[test]
    fn cooldown_expires_and_allows_reactivation() {
        let mut g = Ghost::new(0.1, u32::MAX).with_cooldown(0.2);
        g.activate();
        g.tick(0.2); // ghost ends
        g.tick(0.3); // cooldown drains
        assert!(!g.is_on_cooldown());
        assert!(g.activate());
    }

    #[test]
    fn manual_mode_does_not_auto_expire() {
        let mut g = Ghost::manual(u32::MAX);
        g.is_ghost = true;
        let ended = g.tick(100.0);
        assert!(!ended);
        assert!(g.is_ghost);
    }
}

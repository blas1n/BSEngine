use bevy_ecs::prelude::Component;

/// Thrown grappling hook that latches onto a distant surface or enemy and
/// reels the thrower (or target) toward the anchor point.
///
/// The hook moves through a simple state machine: `Idle → Launched → Latched
/// → Idle`. Call `launch()` to throw; the physics system calls `latch()` when
/// the projectile reaches its target. Once latched, `reel_force()` returns the
/// pull strength per frame; call `release()` to cancel at any time.
///
/// Distinct from `Grapple` (melee clinch — holds an enemy in arm-to-arm
/// contact), `Tether` (persistent elastic link between two entities), and
/// `Rope` (free physics cable): Grapnel is a **thrown ranged hook** — it has a
/// distinct airborne phase before latching, and its reel force is applied by
/// the physics system only after a confirmed latch.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Grapnel {
    /// Maximum throw distance (world units). Clamped ≥ 0.0.
    pub range: f32,
    /// Pull speed applied each second while latched (world units / sec). Clamped ≥ 0.0.
    pub reel_speed: f32,
    pub state: GrapnelState,
    pub just_launched: bool,
    pub just_latched: bool,
    pub just_released: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum GrapnelState {
    #[default]
    Idle,
    Launched,
    Latched,
}

impl Grapnel {
    pub fn new(range: f32, reel_speed: f32) -> Self {
        Self {
            range: range.max(0.0),
            reel_speed: reel_speed.max(0.0),
            state: GrapnelState::Idle,
            just_launched: false,
            just_latched: false,
            just_released: false,
            enabled: true,
        }
    }

    /// Throw the hook (Idle → Launched). No-op if already active or disabled.
    pub fn launch(&mut self) {
        if !self.enabled || !matches!(self.state, GrapnelState::Idle) {
            return;
        }
        self.state = GrapnelState::Launched;
        self.just_launched = true;
    }

    /// Confirm latch (Launched → Latched). No-op unless currently launched.
    pub fn latch(&mut self) {
        if matches!(self.state, GrapnelState::Launched) {
            self.state = GrapnelState::Latched;
            self.just_latched = true;
        }
    }

    /// Cancel or end the grapnel (any active state → Idle). Sets `just_released`.
    pub fn release(&mut self) {
        if !matches!(self.state, GrapnelState::Idle) {
            self.state = GrapnelState::Idle;
            self.just_released = true;
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_launched = false;
        self.just_latched = false;
        self.just_released = false;
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.state, GrapnelState::Idle)
    }

    pub fn is_launched(&self) -> bool {
        matches!(self.state, GrapnelState::Launched)
    }

    pub fn is_latched(&self) -> bool {
        matches!(self.state, GrapnelState::Latched)
    }

    /// Whether the grapnel is in flight or latched (i.e., not idle).
    pub fn is_active(&self) -> bool {
        !self.is_idle()
    }

    /// Pull force to apply this frame while latched. Returns `reel_speed` when
    /// latched and enabled; returns `0.0` otherwise. The owning system applies
    /// this toward the anchor point.
    pub fn reel_force(&self) -> f32 {
        if self.is_latched() && self.enabled {
            self.reel_speed
        } else {
            0.0
        }
    }

    /// Whether a throw target at `distance` world units is within range.
    pub fn in_range(&self, distance: f32) -> bool {
        distance <= self.range
    }
}

impl Default for Grapnel {
    fn default() -> Self {
        Self::new(15.0, 8.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_sets_launched_state() {
        let mut g = Grapnel::new(15.0, 8.0);
        g.launch();
        assert!(g.is_launched());
        assert!(g.just_launched);
    }

    #[test]
    fn launch_no_op_when_already_launched() {
        let mut g = Grapnel::new(15.0, 8.0);
        g.launch();
        g.tick();
        g.launch(); // already active
        assert!(!g.just_launched);
    }

    #[test]
    fn latch_from_launched() {
        let mut g = Grapnel::new(15.0, 8.0);
        g.launch();
        g.latch();
        assert!(g.is_latched());
        assert!(g.just_latched);
    }

    #[test]
    fn latch_no_op_when_idle() {
        let mut g = Grapnel::new(15.0, 8.0);
        g.latch();
        assert!(g.is_idle());
        assert!(!g.just_latched);
    }

    #[test]
    fn release_from_launched() {
        let mut g = Grapnel::new(15.0, 8.0);
        g.launch();
        g.release();
        assert!(g.is_idle());
        assert!(g.just_released);
    }

    #[test]
    fn release_from_latched() {
        let mut g = Grapnel::new(15.0, 8.0);
        g.launch();
        g.latch();
        g.release();
        assert!(g.is_idle());
        assert!(g.just_released);
    }

    #[test]
    fn release_no_op_when_idle() {
        let mut g = Grapnel::new(15.0, 8.0);
        g.release();
        assert!(!g.just_released);
    }

    #[test]
    fn tick_clears_just_launched() {
        let mut g = Grapnel::new(15.0, 8.0);
        g.launch();
        g.tick();
        assert!(!g.just_launched);
    }

    #[test]
    fn reel_force_when_latched() {
        let mut g = Grapnel::new(15.0, 8.0);
        g.launch();
        g.latch();
        assert!((g.reel_force() - 8.0).abs() < 1e-5);
    }

    #[test]
    fn reel_force_zero_when_launched() {
        let mut g = Grapnel::new(15.0, 8.0);
        g.launch();
        assert!((g.reel_force() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn reel_force_zero_when_idle() {
        let g = Grapnel::new(15.0, 8.0);
        assert!((g.reel_force() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn reel_force_zero_when_disabled() {
        let mut g = Grapnel::new(15.0, 8.0);
        g.launch();
        g.latch();
        g.enabled = false;
        assert!((g.reel_force() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn in_range_within() {
        let g = Grapnel::new(15.0, 8.0);
        assert!(g.in_range(10.0));
        assert!(g.in_range(15.0));
    }

    #[test]
    fn in_range_beyond() {
        let g = Grapnel::new(15.0, 8.0);
        assert!(!g.in_range(16.0));
    }

    #[test]
    fn disabled_launch_no_op() {
        let mut g = Grapnel::new(15.0, 8.0);
        g.enabled = false;
        g.launch();
        assert!(g.is_idle());
    }

    #[test]
    fn is_active_when_not_idle() {
        let mut g = Grapnel::new(15.0, 8.0);
        g.launch();
        assert!(g.is_active());
        g.latch();
        assert!(g.is_active());
        g.release();
        assert!(!g.is_active());
    }
}

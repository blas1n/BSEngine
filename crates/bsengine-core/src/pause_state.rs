use bevy_ecs::prelude::Resource;

/// Whether gameplay simulation is currently paused. Gates `PhysicsPlugin` and
/// `NavMeshPlugin`; scripts must check `Bsengine.isPaused()` themselves, since
/// no single delta-time value in this engine is shared by every system
/// (`Time` and script `getDeltaTime()` are independent clocks, and Rapier
/// physics steps on its own fixed timestep regardless of either).
#[derive(Resource, Default, Clone, Copy)]
pub struct PauseState {
    /// `true` while gameplay is paused.
    pub paused: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_not_paused() {
        assert!(!PauseState::default().paused);
    }
}

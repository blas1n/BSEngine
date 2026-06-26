use bevy_ecs::prelude::Component;

/// The kind of input event that activates an interactable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InteractTrigger {
    /// Activated the moment the player presses the interact button while in range.
    #[default]
    OnPress,
    /// Activated the moment the player releases the interact button while in range.
    OnRelease,
    /// Activated every frame the player holds the interact button while in range.
    OnHold,
}

/// Marks an entity as interactable by a player or AI agent.
/// The interaction system checks proximity and input each frame;
/// when conditions are met it fires an `Interacted` event on this entity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Interactable {
    /// Maximum distance from which this entity can be interacted with.
    pub range: f32,
    /// Short label shown in the interaction prompt (e.g. `"Open"`, `"Pick up"`).
    pub prompt: String,
    /// What kind of input activates the interaction.
    pub trigger: InteractTrigger,
    /// How long in seconds the player must hold before the interaction fires.
    /// Only meaningful when `trigger == OnHold`; 0 = immediate.
    pub hold_duration: f32,
    pub enabled: bool,
}

impl Interactable {
    pub fn new(prompt: impl Into<String>, range: f32) -> Self {
        Self {
            range: range.max(0.0),
            prompt: prompt.into(),
            trigger: InteractTrigger::OnPress,
            hold_duration: 0.0,
            enabled: true,
        }
    }

    pub fn with_trigger(mut self, trigger: InteractTrigger) -> Self {
        self.trigger = trigger;
        self
    }

    pub fn with_hold_duration(mut self, seconds: f32) -> Self {
        self.hold_duration = seconds.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactable_defaults() {
        let i = Interactable::new("Open", 2.0);
        assert_eq!(i.prompt, "Open");
        assert!((i.range - 2.0).abs() < 0.001);
        assert_eq!(i.trigger, InteractTrigger::OnPress);
        assert_eq!(i.hold_duration, 0.0);
        assert!(i.enabled);
    }

    #[test]
    fn range_clamped() {
        let i = Interactable::new("Use", -5.0);
        assert_eq!(i.range, 0.0);
    }

    #[test]
    fn hold_duration_clamped() {
        let i = Interactable::new("Hold", 1.5).with_hold_duration(-1.0);
        assert_eq!(i.hold_duration, 0.0);
    }

    #[test]
    fn trigger_variants() {
        let i = Interactable::new("Release", 1.0).with_trigger(InteractTrigger::OnRelease);
        assert_eq!(i.trigger, InteractTrigger::OnRelease);
    }

    #[test]
    fn disabled_flag() {
        let i = Interactable::new("Locked", 1.0).disabled();
        assert!(!i.enabled);
    }
}

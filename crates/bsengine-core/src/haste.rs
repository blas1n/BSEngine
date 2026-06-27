use bevy_ecs::prelude::Component;

/// Direction of the speed modification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasteKind {
    /// Multiplier > 1: character moves faster.
    Haste,
    /// Multiplier < 1: character moves slower.
    Slow,
}

/// A single active haste/slow stack.
#[derive(Debug, Clone, PartialEq)]
pub struct HasteStack {
    pub kind: HasteKind,
    /// Multiplicative speed factor (e.g. 1.5 for 50% faster, 0.5 for 50% slower).
    pub multiplier: f32,
    /// Remaining duration in seconds. Negative = permanent until explicitly removed.
    pub duration: f32,
}

/// Character-level movement-speed modifier component.
///
/// Tracks stacked haste / slow effects. Each stack has its own duration and
/// multiplier. `tick(dt)` expires stacks and computes `effective_multiplier()`
/// as the product of all active stack multipliers.
///
/// Distinct from `boost` (a generic stat-buff set), `slow_mo` (global time
/// dilation), and `stamina` (an energy resource). `Haste` specifically models
/// movement-speed augmentation with stacking and duration management.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Haste {
    stacks: Vec<HasteStack>,
    pub max_stacks: usize,
    pub enabled: bool,
}

impl Haste {
    pub fn new(max_stacks: usize) -> Self {
        Self {
            stacks: Vec::new(),
            max_stacks: max_stacks.max(1),
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Apply a haste or slow with the given multiplier and duration.
    /// Fails silently (does not add) when at max stacks.
    /// Use `duration < 0` for a permanent effect until `clear()`.
    pub fn apply(&mut self, multiplier: f32, duration: f32) {
        if !self.enabled || self.stacks.len() >= self.max_stacks {
            return;
        }
        let kind = if multiplier >= 1.0 {
            HasteKind::Haste
        } else {
            HasteKind::Slow
        };
        self.stacks.push(HasteStack {
            kind,
            multiplier: multiplier.max(0.0),
            duration,
        });
    }

    /// Remove all active stacks.
    pub fn clear(&mut self) {
        self.stacks.clear();
    }

    /// Remove all stacks matching `kind`.
    pub fn clear_kind(&mut self, kind: HasteKind) {
        self.stacks.retain(|s| s.kind != kind);
    }

    /// Expire timed stacks. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled {
            return;
        }
        self.stacks.retain_mut(|s| {
            if s.duration < 0.0 {
                return true; // permanent
            }
            s.duration -= dt;
            s.duration > 0.0
        });
    }

    /// Product of all active multipliers. Returns 1.0 if no stacks.
    pub fn effective_multiplier(&self) -> f32 {
        if !self.enabled || self.stacks.is_empty() {
            return 1.0;
        }
        self.stacks.iter().map(|s| s.multiplier).product()
    }

    pub fn is_hastened(&self) -> bool {
        self.enabled && self.stacks.iter().any(|s| s.kind == HasteKind::Haste)
    }

    pub fn is_slowed(&self) -> bool {
        self.enabled && self.stacks.iter().any(|s| s.kind == HasteKind::Slow)
    }

    pub fn stack_count(&self) -> usize {
        self.stacks.len()
    }

    pub fn stacks(&self) -> &[HasteStack] {
        &self.stacks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_haste_increases_multiplier() {
        let mut h = Haste::new(4);
        h.apply(1.5, 5.0);
        assert!(h.is_hastened());
        assert!((h.effective_multiplier() - 1.5).abs() < 1e-5);
    }

    #[test]
    fn apply_slow_decreases_multiplier() {
        let mut h = Haste::new(4);
        h.apply(0.5, 3.0);
        assert!(h.is_slowed());
        assert!((h.effective_multiplier() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn stacks_multiply() {
        let mut h = Haste::new(4);
        h.apply(1.5, 5.0);
        h.apply(0.8, 5.0);
        assert!((h.effective_multiplier() - 1.2).abs() < 1e-5);
    }

    #[test]
    fn tick_expires_stacks() {
        let mut h = Haste::new(4);
        h.apply(1.5, 1.0);
        h.tick(1.0);
        assert_eq!(h.stack_count(), 0);
        assert!((h.effective_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn permanent_stack_does_not_expire() {
        let mut h = Haste::new(4);
        h.apply(0.5, -1.0); // permanent slow
        h.tick(999.0);
        assert_eq!(h.stack_count(), 1);
    }

    #[test]
    fn max_stacks_enforced() {
        let mut h = Haste::new(2);
        h.apply(1.5, 5.0);
        h.apply(1.5, 5.0);
        h.apply(1.5, 5.0); // should be dropped
        assert_eq!(h.stack_count(), 2);
    }

    #[test]
    fn clear_kind_removes_only_slows() {
        let mut h = Haste::new(4);
        h.apply(1.5, 5.0);
        h.apply(0.5, 5.0);
        h.clear_kind(HasteKind::Slow);
        assert_eq!(h.stack_count(), 1);
        assert!(h.is_hastened());
        assert!(!h.is_slowed());
    }
}

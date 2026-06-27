use bevy_ecs::prelude::Component;

/// Entity-combination tracker for blob, slime, or fluid-mass merge mechanics.
/// Each participant carries a `merge_weight` representing its current mass.
/// When two merge-eligible entities collide, the absorbing entity calls
/// `merge_with(other_weight)` to consume the other's mass, growing up to
/// `max_weight`.
///
/// `merge_with(other_weight)` adds `other_weight` to `merge_weight` (capped
/// at `max_weight`) and sets `just_merged`. No-op when disabled, `can_merge`
/// is `false`, or `other_weight ≤ 0`.
///
/// `can_accept(other_weight)` returns `true` when the entity is enabled,
/// `can_merge` is set, and adding `other_weight` would not exceed `max_weight`.
/// Use this before deciding which entity absorbs which.
///
/// `tick()` clears `just_merged` each frame.
///
/// Distinct from `Magnet` (pulls entities toward the entity — attraction,
/// not absorption), `Absorb`/`Absorption` (absorbs incoming damage, not
/// entity mass), and `Combine` (skill-based combination): Merge is a
/// **physical mass-transfer tracker** for entities that grow by consuming
/// other entities of the same kind (blob enemies, liquid blobs, swarm
/// units that coalesce).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Merge {
    /// Whether this entity is currently eligible to participate in a merge.
    pub can_merge: bool,
    /// Current accumulated mass. Clamped ≥ 0.0.
    pub merge_weight: f32,
    /// Maximum mass this entity can reach through merging. Clamped ≥ merge_weight.
    pub max_weight: f32,
    pub just_merged: bool,
    pub enabled: bool,
}

impl Merge {
    pub fn new(merge_weight: f32, max_weight: f32) -> Self {
        let w = merge_weight.max(0.0);
        let m = max_weight.max(w);
        Self {
            can_merge: true,
            merge_weight: w,
            max_weight: m,
            just_merged: false,
            enabled: true,
        }
    }

    /// Absorb `other_weight` into this entity's mass. `merge_weight` is
    /// capped at `max_weight`. Sets `just_merged` on any successful transfer.
    /// No-op when disabled, `can_merge` is `false`, or `other_weight ≤ 0`.
    pub fn merge_with(&mut self, other_weight: f32) {
        if !self.enabled || !self.can_merge || other_weight <= 0.0 {
            return;
        }
        self.merge_weight = (self.merge_weight + other_weight).min(self.max_weight);
        self.just_merged = true;
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_merged = false;
    }

    /// `true` when the entity is enabled, can merge, and has room to absorb
    /// `other_weight` without exceeding `max_weight`.
    pub fn can_accept(&self, other_weight: f32) -> bool {
        self.enabled && self.can_merge && self.merge_weight + other_weight <= self.max_weight
    }

    /// `true` when `merge_weight` has reached `max_weight`.
    pub fn is_at_max(&self) -> bool {
        self.merge_weight >= self.max_weight
    }

    /// Current mass as a fraction of the maximum [0.0, 1.0].
    pub fn merge_fraction(&self) -> f32 {
        if self.max_weight <= 0.0 {
            return 1.0;
        }
        (self.merge_weight / self.max_weight).clamp(0.0, 1.0)
    }
}

impl Default for Merge {
    fn default() -> Self {
        Self::new(1.0, 10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_has_correct_fields() {
        let m = Merge::new(2.0, 10.0);
        assert!((m.merge_weight - 2.0).abs() < 1e-5);
        assert!((m.max_weight - 10.0).abs() < 1e-5);
        assert!(m.can_merge);
    }

    #[test]
    fn new_max_clamps_to_weight_when_lower() {
        let m = Merge::new(5.0, 2.0);
        assert!((m.max_weight - 5.0).abs() < 1e-5);
    }

    #[test]
    fn merge_with_adds_weight() {
        let mut m = Merge::new(1.0, 10.0);
        m.merge_with(3.0);
        assert!((m.merge_weight - 4.0).abs() < 1e-5);
    }

    #[test]
    fn merge_with_caps_at_max() {
        let mut m = Merge::new(8.0, 10.0);
        m.merge_with(5.0); // would be 13, caps at 10
        assert!((m.merge_weight - 10.0).abs() < 1e-5);
    }

    #[test]
    fn merge_with_sets_just_merged() {
        let mut m = Merge::new(1.0, 10.0);
        m.merge_with(2.0);
        assert!(m.just_merged);
    }

    #[test]
    fn merge_with_no_op_when_disabled() {
        let mut m = Merge::new(1.0, 10.0);
        m.enabled = false;
        m.merge_with(2.0);
        assert!((m.merge_weight - 1.0).abs() < 1e-5);
        assert!(!m.just_merged);
    }

    #[test]
    fn merge_with_no_op_when_cant_merge() {
        let mut m = Merge::new(1.0, 10.0);
        m.can_merge = false;
        m.merge_with(2.0);
        assert!((m.merge_weight - 1.0).abs() < 1e-5);
        assert!(!m.just_merged);
    }

    #[test]
    fn merge_with_no_op_at_zero_weight() {
        let mut m = Merge::new(1.0, 10.0);
        m.merge_with(0.0);
        assert!(!m.just_merged);
    }

    #[test]
    fn merge_with_no_op_at_negative_weight() {
        let mut m = Merge::new(1.0, 10.0);
        m.merge_with(-1.0);
        assert!(!m.just_merged);
    }

    #[test]
    fn tick_clears_just_merged() {
        let mut m = Merge::new(1.0, 10.0);
        m.merge_with(2.0);
        m.tick();
        assert!(!m.just_merged);
    }

    #[test]
    fn can_accept_true_when_within_capacity() {
        let m = Merge::new(3.0, 10.0);
        assert!(m.can_accept(5.0)); // 3 + 5 = 8 <= 10
    }

    #[test]
    fn can_accept_true_at_exact_capacity() {
        let m = Merge::new(3.0, 10.0);
        assert!(m.can_accept(7.0)); // 3 + 7 = 10 = max
    }

    #[test]
    fn can_accept_false_when_would_exceed() {
        let m = Merge::new(5.0, 10.0);
        assert!(!m.can_accept(6.0)); // 5 + 6 = 11 > 10
    }

    #[test]
    fn can_accept_false_when_disabled() {
        let mut m = Merge::new(1.0, 10.0);
        m.enabled = false;
        assert!(!m.can_accept(2.0));
    }

    #[test]
    fn can_accept_false_when_cant_merge() {
        let mut m = Merge::new(1.0, 10.0);
        m.can_merge = false;
        assert!(!m.can_accept(2.0));
    }

    #[test]
    fn is_at_max_false_when_below() {
        let m = Merge::new(5.0, 10.0);
        assert!(!m.is_at_max());
    }

    #[test]
    fn is_at_max_true_when_full() {
        let m = Merge::new(10.0, 10.0);
        assert!(m.is_at_max());
    }

    #[test]
    fn is_at_max_true_after_filling() {
        let mut m = Merge::new(7.0, 10.0);
        m.merge_with(5.0);
        assert!(m.is_at_max());
    }

    #[test]
    fn merge_fraction_at_half() {
        let m = Merge::new(5.0, 10.0);
        assert!((m.merge_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn merge_fraction_at_full() {
        let m = Merge::new(10.0, 10.0);
        assert!((m.merge_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn merge_fraction_at_zero() {
        let m = Merge::new(0.0, 10.0);
        assert!(m.merge_fraction().abs() < 1e-5);
    }

    #[test]
    fn multiple_merges_accumulate() {
        let mut m = Merge::new(1.0, 10.0);
        m.merge_with(2.0); // 3
        m.tick();
        m.merge_with(3.0); // 6
        assert!((m.merge_weight - 6.0).abs() < 1e-5);
    }
}

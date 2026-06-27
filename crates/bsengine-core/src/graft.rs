use bevy_ecs::prelude::Component;

/// Permanent body-modification slot tracker for biological or mechanical
/// augmentations. Each installed graft adds `bonus_per_graft` to a cumulative
/// multiplier returned by `bonus_multiplier()`. The entity's equipment or
/// ability system multiplies relevant base stats by this value.
///
/// `add_graft()` installs one augmentation when `graft_count < max_grafts`.
/// Sets `just_installed` and returns `true` on success. No-op when disabled
/// or at capacity; returns `false`.
///
/// `remove_graft()` uninstalls one augmentation when `graft_count > 0`.
/// Sets `just_removed` and returns `true` on success. No-op when disabled
/// or at zero; returns `false`.
///
/// `tick()` clears `just_installed` and `just_removed` each frame.
///
/// `bonus_multiplier()` returns `1.0 + bonus_per_graft * graft_count` when
/// enabled; returns `1.0` when disabled.
///
/// Distinct from `Upgrade` (tier-based version progression without slots),
/// `Buff` (temporary stat boost with a duration), and `Item` (inventory item
/// that can be carried or dropped): Graft is a **permanent slot-limited
/// modification system** — augmentations take a body slot and stack a
/// cumulative bonus; the entity cannot exceed `max_grafts` total modifications.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Graft {
    /// Currently installed augmentations.
    pub graft_count: u32,
    /// Maximum body slots available. Clamped ≥ 1.
    pub max_grafts: u32,
    /// Stat multiplier increment per installed graft. Clamped ≥ 0.0.
    /// e.g. `0.05` gives +5 % per graft; 4 grafts → 1.20× base.
    pub bonus_per_graft: f32,
    pub just_installed: bool,
    pub just_removed: bool,
    pub enabled: bool,
}

impl Graft {
    pub fn new(max_grafts: u32, bonus_per_graft: f32) -> Self {
        Self {
            graft_count: 0,
            max_grafts: max_grafts.max(1),
            bonus_per_graft: bonus_per_graft.max(0.0),
            just_installed: false,
            just_removed: false,
            enabled: true,
        }
    }

    /// Install one augmentation. Sets `just_installed` and returns `true` on
    /// success. No-op and returns `false` when disabled or at capacity.
    pub fn add_graft(&mut self) -> bool {
        if !self.enabled || self.is_full() {
            return false;
        }
        self.graft_count += 1;
        self.just_installed = true;
        true
    }

    /// Remove one augmentation. Sets `just_removed` and returns `true` on
    /// success. No-op and returns `false` when disabled or at zero.
    pub fn remove_graft(&mut self) -> bool {
        if !self.enabled || self.graft_count == 0 {
            return false;
        }
        self.graft_count -= 1;
        self.just_removed = true;
        true
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_installed = false;
        self.just_removed = false;
    }

    /// `true` when all modification slots are occupied.
    pub fn is_full(&self) -> bool {
        self.graft_count >= self.max_grafts
    }

    /// Cumulative stat multiplier from all installed grafts.
    /// Returns `1.0 + bonus_per_graft * graft_count` when enabled;
    /// returns `1.0` when disabled.
    pub fn bonus_multiplier(&self) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        1.0 + self.bonus_per_graft * self.graft_count as f32
    }

    /// Slot occupancy fraction [0.0 = empty, 1.0 = full].
    pub fn graft_fraction(&self) -> f32 {
        self.graft_count as f32 / self.max_grafts as f32
    }
}

impl Default for Graft {
    fn default() -> Self {
        Self::new(4, 0.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_empty() {
        let g = Graft::new(4, 0.1);
        assert_eq!(g.graft_count, 0);
        assert!(!g.is_full());
    }

    #[test]
    fn add_graft_increments() {
        let mut g = Graft::new(4, 0.1);
        assert!(g.add_graft());
        assert_eq!(g.graft_count, 1);
        assert!(g.just_installed);
    }

    #[test]
    fn add_graft_fails_when_full() {
        let mut g = Graft::new(2, 0.1);
        g.add_graft();
        g.add_graft(); // full
        assert!(!g.add_graft());
        assert_eq!(g.graft_count, 2);
    }

    #[test]
    fn add_graft_fails_when_disabled() {
        let mut g = Graft::new(4, 0.1);
        g.enabled = false;
        assert!(!g.add_graft());
        assert_eq!(g.graft_count, 0);
    }

    #[test]
    fn remove_graft_decrements() {
        let mut g = Graft::new(4, 0.1);
        g.add_graft();
        g.tick();
        assert!(g.remove_graft());
        assert_eq!(g.graft_count, 0);
        assert!(g.just_removed);
    }

    #[test]
    fn remove_graft_fails_when_empty() {
        let mut g = Graft::new(4, 0.1);
        assert!(!g.remove_graft());
        assert!(!g.just_removed);
    }

    #[test]
    fn remove_graft_fails_when_disabled() {
        let mut g = Graft::new(4, 0.1);
        g.add_graft();
        g.tick();
        g.enabled = false;
        assert!(!g.remove_graft());
        assert_eq!(g.graft_count, 1);
    }

    #[test]
    fn tick_clears_just_installed() {
        let mut g = Graft::new(4, 0.1);
        g.add_graft();
        g.tick();
        assert!(!g.just_installed);
    }

    #[test]
    fn tick_clears_just_removed() {
        let mut g = Graft::new(4, 0.1);
        g.add_graft();
        g.tick();
        g.remove_graft();
        g.tick();
        assert!(!g.just_removed);
    }

    #[test]
    fn is_full_true_at_capacity() {
        let mut g = Graft::new(2, 0.1);
        g.add_graft();
        g.add_graft();
        assert!(g.is_full());
    }

    #[test]
    fn is_full_false_when_below_max() {
        let mut g = Graft::new(4, 0.1);
        g.add_graft();
        assert!(!g.is_full());
    }

    #[test]
    fn bonus_multiplier_at_zero_grafts() {
        let g = Graft::new(4, 0.1);
        assert!((g.bonus_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn bonus_multiplier_one_graft() {
        let mut g = Graft::new(4, 0.1);
        g.add_graft();
        // 1 + 0.1 * 1 = 1.1
        assert!((g.bonus_multiplier() - 1.1).abs() < 1e-5);
    }

    #[test]
    fn bonus_multiplier_all_grafts() {
        let mut g = Graft::new(4, 0.1);
        g.add_graft();
        g.add_graft();
        g.add_graft();
        g.add_graft();
        // 1 + 0.1 * 4 = 1.4
        assert!((g.bonus_multiplier() - 1.4).abs() < 1e-5);
    }

    #[test]
    fn bonus_multiplier_one_when_disabled() {
        let mut g = Graft::new(4, 0.1);
        g.add_graft();
        g.add_graft();
        g.enabled = false;
        assert!((g.bonus_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn graft_fraction_at_half() {
        let mut g = Graft::new(4, 0.1);
        g.add_graft();
        g.add_graft();
        assert!((g.graft_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn graft_fraction_at_full() {
        let mut g = Graft::new(2, 0.1);
        g.add_graft();
        g.add_graft();
        assert!((g.graft_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn graft_fraction_at_zero() {
        let g = Graft::new(4, 0.1);
        assert!(g.graft_fraction().abs() < 1e-5);
    }

    #[test]
    fn add_then_remove_cycle() {
        let mut g = Graft::new(4, 0.1);
        g.add_graft();
        g.add_graft();
        g.tick();
        g.remove_graft();
        assert_eq!(g.graft_count, 1);
        assert!((g.bonus_multiplier() - 1.1).abs() < 1e-5);
    }

    #[test]
    fn max_grafts_clamped_to_one() {
        let g = Graft::new(0, 0.1);
        assert_eq!(g.max_grafts, 1);
    }

    #[test]
    fn bonus_per_graft_clamped_non_negative() {
        let g = Graft::new(4, -0.5);
        assert_eq!(g.bonus_per_graft, 0.0);
    }

    #[test]
    fn zero_bonus_multiplier_stays_one() {
        let mut g = Graft::new(4, 0.0);
        g.add_graft();
        g.add_graft();
        assert!((g.bonus_multiplier() - 1.0).abs() < 1e-5);
    }
}

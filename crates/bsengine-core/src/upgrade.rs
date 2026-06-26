use bevy_ecs::prelude::Component;

/// A single upgrade tier definition (inline, not a separate entity).
#[derive(Debug, Clone, PartialEq)]
pub struct UpgradeTier {
    /// XP / cost required to reach this tier from the previous one.
    pub xp_required: f32,
    /// Additive stat multiplier awarded when this tier is unlocked (e.g. 0.1 = +10%).
    pub stat_bonus: f32,
}

impl UpgradeTier {
    pub fn new(xp_required: f32, stat_bonus: f32) -> Self {
        Self {
            xp_required: xp_required.max(0.0),
            stat_bonus,
        }
    }
}

/// Upgrade / progression component — tracks XP and level for an item, ability, or skill.
///
/// Tiers define the XP cost and stat bonus for each level. When `accumulated_xp` exceeds
/// the current tier's `xp_required`, call `try_level_up()` to advance the level and
/// deduct the cost.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Upgrade {
    /// Current level (0-indexed: 0 = base, 1 = first upgrade, …).
    pub level: u32,
    /// Ordered list of upgrade tiers. `tiers.len()` is the max level.
    pub tiers: Vec<UpgradeTier>,
    /// XP accumulated toward the next level.
    pub accumulated_xp: f32,
    /// Whether the upgrade is locked from receiving XP.
    pub locked: bool,
    pub enabled: bool,
}

impl Upgrade {
    /// Create with evenly spaced XP requirements and a flat bonus per level.
    pub fn uniform(max_level: u32, xp_per_level: f32, bonus_per_level: f32) -> Self {
        let tiers = (0..max_level)
            .map(|_| UpgradeTier::new(xp_per_level, bonus_per_level))
            .collect();
        Self {
            level: 0,
            tiers,
            accumulated_xp: 0.0,
            locked: false,
            enabled: true,
        }
    }

    pub fn with_tiers(tiers: Vec<UpgradeTier>) -> Self {
        Self {
            level: 0,
            tiers,
            accumulated_xp: 0.0,
            locked: false,
            enabled: true,
        }
    }

    pub fn locked(mut self) -> Self {
        self.locked = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns true if further levelling is possible.
    pub fn is_maxed(&self) -> bool {
        self.level as usize >= self.tiers.len()
    }

    /// Current tier (the tier that must be completed to reach the next level).
    /// Returns `None` when already maxed.
    pub fn current_tier(&self) -> Option<&UpgradeTier> {
        self.tiers.get(self.level as usize)
    }

    /// Add XP toward the next level. Returns `true` when enough XP has been added
    /// to level up (but does not level up automatically — call `try_level_up()`).
    pub fn add_xp(&mut self, amount: f32) -> bool {
        if self.locked || !self.enabled || self.is_maxed() {
            return false;
        }
        self.accumulated_xp += amount.max(0.0);
        if let Some(tier) = self.current_tier() {
            self.accumulated_xp >= tier.xp_required
        } else {
            false
        }
    }

    /// Level up if enough XP has been accumulated. Returns `true` if a level was gained.
    pub fn try_level_up(&mut self) -> bool {
        if self.is_maxed() {
            return false;
        }
        let Some(tier) = self.current_tier() else {
            return false;
        };
        if self.accumulated_xp < tier.xp_required {
            return false;
        }
        self.accumulated_xp -= tier.xp_required;
        self.level += 1;
        true
    }

    /// Sum of all `stat_bonus` values up to and including the current level.
    pub fn total_bonus(&self) -> f32 {
        self.tiers[..self.level as usize]
            .iter()
            .map(|t| t.stat_bonus)
            .sum()
    }

    /// 0..1 progress within the current tier (1 = ready to level up).
    pub fn tier_fraction(&self) -> f32 {
        let Some(tier) = self.current_tier() else {
            return 1.0;
        };
        if tier.xp_required <= 0.0 {
            return 1.0;
        }
        (self.accumulated_xp / tier.xp_required).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_xp_accumulates_and_signals_ready() {
        let mut u = Upgrade::uniform(3, 100.0, 0.1);
        let ready = u.add_xp(100.0);
        assert!(ready);
        assert!((u.accumulated_xp - 100.0).abs() < 1e-5);
    }

    #[test]
    fn try_level_up_advances_level_and_deducts_xp() {
        let mut u = Upgrade::uniform(3, 100.0, 0.1);
        u.add_xp(150.0);
        let leveled = u.try_level_up();
        assert!(leveled);
        assert_eq!(u.level, 1);
        assert!((u.accumulated_xp - 50.0).abs() < 1e-5);
    }

    #[test]
    fn is_maxed_blocks_further_leveling() {
        let mut u = Upgrade::uniform(1, 100.0, 0.1);
        u.add_xp(100.0);
        u.try_level_up();
        assert!(u.is_maxed());
        let ready = u.add_xp(100.0);
        assert!(!ready);
    }

    #[test]
    fn total_bonus_sums_unlocked_tiers() {
        let mut u = Upgrade::uniform(3, 10.0, 0.1);
        u.add_xp(10.0);
        u.try_level_up();
        u.add_xp(10.0);
        u.try_level_up();
        assert!((u.total_bonus() - 0.2).abs() < 1e-5);
    }

    #[test]
    fn tier_fraction_reports_progress() {
        let mut u = Upgrade::uniform(3, 100.0, 0.1);
        u.add_xp(50.0);
        assert!((u.tier_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn locked_blocks_xp_gain() {
        let mut u = Upgrade::uniform(3, 100.0, 0.1).locked();
        let ready = u.add_xp(200.0);
        assert!(!ready);
        assert_eq!(u.accumulated_xp, 0.0);
    }
}

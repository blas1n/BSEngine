use bevy_ecs::prelude::Component;

/// Range-scaling damage modifier for ranged attackers: damage increases from
/// `base` to `base * (1 + damage_bonus)` as distance grows from `min_range`
/// to `peak_range`, using linear interpolation. At point-blank range (below
/// `min_range`) the entity incurs a `point_blank_penalty` fraction reduction.
/// Beyond `peak_range` the bonus stays at the peak value.
///
/// The damage system calls `effective_damage(base, distance)` on each outgoing
/// hit to apply the appropriate scalar.
///
/// Distinct from `Hone` (hit-count sharpness), `Amplify` (flat damage
/// multiplier), and `Pierce` (penetrates targets in a line): Rifle is a
/// **distance-to-power curve** — it rewards keeping range and penalizes
/// closing in.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Rifle {
    /// Distance below which `point_blank_penalty` applies. Clamped ≥ 0.0.
    pub min_range: f32,
    /// Distance at which `damage_bonus` is fully applied. Clamped > `min_range`.
    pub peak_range: f32,
    /// Maximum extra damage fraction at `peak_range`. e.g. 0.5 = +50% damage.
    /// Clamped ≥ 0.0.
    pub damage_bonus: f32,
    /// Damage reduction fraction when distance < `min_range`. e.g. 0.3 = −30%.
    /// Clamped [0.0, 1.0].
    pub point_blank_penalty: f32,
    pub enabled: bool,
}

impl Rifle {
    pub fn new(
        min_range: f32,
        peak_range: f32,
        damage_bonus: f32,
        point_blank_penalty: f32,
    ) -> Self {
        let min = min_range.max(0.0);
        let peak = peak_range.max(min + 0.001);
        Self {
            min_range: min,
            peak_range: peak,
            damage_bonus: damage_bonus.max(0.0),
            point_blank_penalty: point_blank_penalty.clamp(0.0, 1.0),
            enabled: true,
        }
    }

    /// Effective outgoing damage after applying the range curve.
    ///
    /// - distance < min_range  → `base * (1 - point_blank_penalty)`
    /// - min_range ≤ distance < peak_range → lerp from `base` to `base*(1+damage_bonus)`
    /// - distance ≥ peak_range → `base * (1 + damage_bonus)`
    ///
    /// Returns `base` unchanged when disabled or `base ≤ 0`.
    pub fn effective_damage(&self, base: f32, distance: f32) -> f32 {
        if !self.enabled || base <= 0.0 {
            return base;
        }
        if distance < self.min_range {
            base * (1.0 - self.point_blank_penalty)
        } else if distance >= self.peak_range {
            base * (1.0 + self.damage_bonus)
        } else {
            let t = (distance - self.min_range) / (self.peak_range - self.min_range);
            base * (1.0 + self.damage_bonus * t)
        }
    }

    /// Normalised position on the range curve [0.0 = min_range, 1.0 = peak_range].
    /// Returns 0.0 below min_range; clamped to 1.0 at or above peak_range.
    pub fn range_fraction(&self, distance: f32) -> f32 {
        if distance <= self.min_range {
            return 0.0;
        }
        let span = self.peak_range - self.min_range;
        if span <= 0.0 {
            return 1.0;
        }
        ((distance - self.min_range) / span).min(1.0)
    }

    pub fn is_at_peak(&self, distance: f32) -> bool {
        distance >= self.peak_range
    }

    pub fn is_point_blank(&self, distance: f32) -> bool {
        distance < self.min_range
    }
}

impl Default for Rifle {
    fn default() -> Self {
        Self::new(5.0, 20.0, 0.5, 0.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_at_peak_range_full_bonus() {
        let r = Rifle::new(5.0, 20.0, 0.5, 0.2);
        // 100 * (1 + 0.5) = 150
        assert!((r.effective_damage(100.0, 20.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn damage_beyond_peak_range_still_full_bonus() {
        let r = Rifle::new(5.0, 20.0, 0.5, 0.2);
        assert!((r.effective_damage(100.0, 50.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn damage_at_min_range_no_bonus_no_penalty() {
        let r = Rifle::new(5.0, 20.0, 0.5, 0.2);
        // at exactly min_range, t = 0 → base * (1 + 0.5 * 0) = base
        assert!((r.effective_damage(100.0, 5.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn damage_at_midpoint_lerps_correctly() {
        let r = Rifle::new(0.0, 20.0, 1.0, 0.0);
        // distance = 10, t = 0.5 → 100 * (1 + 1.0 * 0.5) = 150
        assert!((r.effective_damage(100.0, 10.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn damage_point_blank_applies_penalty() {
        let r = Rifle::new(5.0, 20.0, 0.5, 0.3);
        // distance = 2 < 5 → 100 * (1 - 0.3) = 70
        assert!((r.effective_damage(100.0, 2.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn damage_zero_penalty_at_point_blank_returns_base() {
        let r = Rifle::new(5.0, 20.0, 0.5, 0.0);
        assert!((r.effective_damage(100.0, 0.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn damage_full_penalty_at_point_blank_returns_zero() {
        let r = Rifle::new(5.0, 20.0, 0.5, 1.0);
        assert!((r.effective_damage(100.0, 0.0)).abs() < 1e-5);
    }

    #[test]
    fn range_fraction_at_min_range() {
        let r = Rifle::new(5.0, 20.0, 0.5, 0.2);
        assert!((r.range_fraction(5.0)).abs() < 1e-5);
    }

    #[test]
    fn range_fraction_at_peak_range() {
        let r = Rifle::new(5.0, 20.0, 0.5, 0.2);
        assert!((r.range_fraction(20.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn range_fraction_at_midpoint() {
        let r = Rifle::new(0.0, 20.0, 0.5, 0.2);
        assert!((r.range_fraction(10.0) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn range_fraction_clamped_above_peak() {
        let r = Rifle::new(5.0, 20.0, 0.5, 0.2);
        assert!((r.range_fraction(100.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn range_fraction_zero_below_min() {
        let r = Rifle::new(5.0, 20.0, 0.5, 0.2);
        assert!((r.range_fraction(1.0)).abs() < 1e-5);
    }

    #[test]
    fn is_at_peak_true_at_peak() {
        let r = Rifle::new(5.0, 20.0, 0.5, 0.2);
        assert!(r.is_at_peak(20.0));
        assert!(r.is_at_peak(30.0));
        assert!(!r.is_at_peak(19.0));
    }

    #[test]
    fn is_point_blank_true_below_min() {
        let r = Rifle::new(5.0, 20.0, 0.5, 0.2);
        assert!(r.is_point_blank(0.0));
        assert!(r.is_point_blank(4.9));
        assert!(!r.is_point_blank(5.0));
    }

    #[test]
    fn disabled_returns_base() {
        let mut r = Rifle::new(5.0, 20.0, 0.5, 0.2);
        r.enabled = false;
        assert!((r.effective_damage(100.0, 25.0) - 100.0).abs() < 1e-5);
        assert!((r.effective_damage(100.0, 2.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn zero_base_returns_zero() {
        let r = Rifle::new(5.0, 20.0, 0.5, 0.2);
        assert!((r.effective_damage(0.0, 25.0)).abs() < 1e-5);
    }

    #[test]
    fn peak_range_clamped_above_min_range() {
        let r = Rifle::new(10.0, 5.0, 0.5, 0.2); // peak < min → clamped to min + epsilon
        assert!(r.peak_range > r.min_range);
    }

    #[test]
    fn no_bonus_means_base_damage_at_all_valid_ranges() {
        let r = Rifle::new(5.0, 20.0, 0.0, 0.0);
        for d in [5.0f32, 10.0, 15.0, 20.0, 30.0] {
            assert!((r.effective_damage(100.0, d) - 100.0).abs() < 1e-3);
        }
    }
}

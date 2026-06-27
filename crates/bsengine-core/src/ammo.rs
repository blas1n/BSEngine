use bevy_ecs::prelude::Component;

/// Two-tier discrete shot counter: `current` is the magazine (fired directly)
/// and `reserve` is the carry stock. `fire()` spends one magazine round and
/// sets `just_emptied` when the magazine runs dry. `reload()` refills the
/// magazine from reserve (up to `max_capacity`) and sets `just_reloaded`.
///
/// `reserve_max == 0` means the reserve is unlimited — `add_reserve()` then
/// adds freely. When `reserve_max > 0` the reserve is capped at that value.
///
/// `fire()` and `reload()` are no-ops when disabled. `add_reserve()` is
/// always allowed even when disabled (externally collected ammo should count
/// regardless). `tick()` clears one-frame flags each frame.
///
/// Distinct from `Fuel` (continuous movement resource), `Mana` (spell
/// resource), and `Charge` (movement-rush charge): Ammo is a **discrete
/// two-tier shot counter** — magazine-sized batches with a finite carry
/// reserve, matching the reload mechanic of ranged weapons.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ammo {
    /// Current magazine count.
    pub current: u32,
    /// Maximum magazine size. Clamped ≥ 1.
    pub max_capacity: u32,
    /// Reserve ammo count.
    pub reserve: u32,
    /// Maximum reserve. `0` means unlimited.
    pub reserve_max: u32,
    pub just_emptied: bool,
    pub just_reloaded: bool,
    pub enabled: bool,
}

impl Ammo {
    /// Create a fully-loaded Ammo component. `reserve_max == 0` → unlimited.
    pub fn new(max_capacity: u32, reserve_max: u32) -> Self {
        let cap = max_capacity.max(1);
        let rsv = if reserve_max == 0 {
            u32::MAX
        } else {
            reserve_max
        };
        Self {
            current: cap,
            max_capacity: cap,
            reserve: rsv.min(u32::MAX),
            reserve_max,
            just_emptied: false,
            just_reloaded: false,
            enabled: true,
        }
    }

    /// Spend one magazine round. Sets `just_emptied` when `current` hits 0.
    /// Returns `true` when a shot was fired; `false` when the magazine is
    /// empty or the component is disabled.
    pub fn fire(&mut self) -> bool {
        if !self.enabled || self.current == 0 {
            return false;
        }
        self.current -= 1;
        if self.current == 0 {
            self.just_emptied = true;
        }
        true
    }

    /// Refill the magazine from reserve. Transfers up to
    /// `max_capacity - current` rounds. Sets `just_reloaded` when the
    /// transfer completes with at least one round added. No-op when the
    /// magazine is already full, the reserve is empty, or disabled.
    pub fn reload(&mut self) {
        if !self.enabled || self.current == self.max_capacity || self.reserve == 0 {
            return;
        }
        let needed = self.max_capacity - self.current;
        let transferred = needed.min(self.reserve);
        self.current += transferred;
        self.reserve -= transferred;
        if transferred > 0 {
            self.just_reloaded = true;
        }
    }

    /// Add rounds to the reserve. Respects `reserve_max` when non-zero.
    /// No-op when `amount == 0`. Always applies regardless of `enabled`.
    pub fn add_reserve(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        if self.reserve_max == 0 {
            self.reserve = self.reserve.saturating_add(amount);
        } else {
            self.reserve = (self.reserve + amount).min(self.reserve_max);
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_emptied = false;
        self.just_reloaded = false;
    }

    /// `true` when the magazine is empty.
    pub fn is_empty(&self) -> bool {
        self.current == 0
    }

    /// `true` when the magazine is below full capacity.
    pub fn needs_reload(&self) -> bool {
        self.current < self.max_capacity
    }

    /// `true` when at least one reserve round is available.
    pub fn has_reserve(&self) -> bool {
        self.reserve > 0
    }

    /// Magazine fill fraction [0.0 = empty, 1.0 = full].
    pub fn magazine_fraction(&self) -> f32 {
        self.current as f32 / self.max_capacity as f32
    }
}

impl Default for Ammo {
    fn default() -> Self {
        Self::new(30, 90)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_fully_loaded() {
        let a = Ammo::new(10, 30);
        assert_eq!(a.current, 10);
        assert_eq!(a.reserve, 30);
        assert_eq!(a.max_capacity, 10);
    }

    #[test]
    fn new_unlimited_reserve_sets_max() {
        let a = Ammo::new(10, 0);
        assert_eq!(a.reserve, u32::MAX);
        assert_eq!(a.reserve_max, 0);
    }

    #[test]
    fn fire_spends_one_round() {
        let mut a = Ammo::new(10, 30);
        assert!(a.fire());
        assert_eq!(a.current, 9);
    }

    #[test]
    fn fire_returns_false_when_empty() {
        let mut a = Ammo::new(1, 30);
        a.fire();
        assert!(!a.fire()); // empty
    }

    #[test]
    fn fire_sets_just_emptied_on_last_round() {
        let mut a = Ammo::new(2, 30);
        a.fire(); // 1 left
        assert!(!a.just_emptied);
        a.fire(); // 0 left
        assert!(a.just_emptied);
        assert!(a.is_empty());
    }

    #[test]
    fn fire_no_op_when_disabled() {
        let mut a = Ammo::new(10, 30);
        a.enabled = false;
        assert!(!a.fire());
        assert_eq!(a.current, 10);
    }

    #[test]
    fn reload_fills_magazine_from_reserve() {
        let mut a = Ammo::new(10, 30);
        a.fire();
        a.fire();
        a.fire(); // current = 7
        a.reload();
        assert_eq!(a.current, 10);
        assert_eq!(a.reserve, 27);
        assert!(a.just_reloaded);
    }

    #[test]
    fn reload_no_op_when_full() {
        let mut a = Ammo::new(10, 30);
        a.reload();
        assert!(!a.just_reloaded);
    }

    #[test]
    fn reload_no_op_when_reserve_empty() {
        let mut a = Ammo::new(10, 5);
        // drain reserve
        a.reserve = 0;
        a.current = 0;
        a.reload();
        assert_eq!(a.current, 0);
        assert!(!a.just_reloaded);
    }

    #[test]
    fn reload_partial_when_reserve_low() {
        let mut a = Ammo::new(10, 30);
        a.current = 0;
        a.reserve = 3; // less than max_capacity
        a.reload();
        assert_eq!(a.current, 3);
        assert_eq!(a.reserve, 0);
        assert!(a.just_reloaded);
    }

    #[test]
    fn reload_no_op_when_disabled() {
        let mut a = Ammo::new(10, 30);
        a.current = 0;
        a.enabled = false;
        a.reload();
        assert_eq!(a.current, 0);
    }

    #[test]
    fn add_reserve_increases_stock() {
        let mut a = Ammo::new(10, 30);
        a.reserve = 20;
        a.add_reserve(5);
        assert_eq!(a.reserve, 25);
    }

    #[test]
    fn add_reserve_caps_at_reserve_max() {
        let mut a = Ammo::new(10, 30);
        a.reserve = 28;
        a.add_reserve(10); // would exceed 30
        assert_eq!(a.reserve, 30);
    }

    #[test]
    fn add_reserve_unlimited_accumulates() {
        let mut a = Ammo::new(10, 0);
        a.reserve = 100;
        a.add_reserve(50);
        assert_eq!(a.reserve, 150);
    }

    #[test]
    fn add_reserve_works_when_disabled() {
        let mut a = Ammo::new(10, 30);
        a.reserve = 0;
        a.enabled = false;
        a.add_reserve(10);
        assert_eq!(a.reserve, 10); // still applied
    }

    #[test]
    fn tick_clears_just_emptied() {
        let mut a = Ammo::new(1, 30);
        a.fire();
        a.tick();
        assert!(!a.just_emptied);
    }

    #[test]
    fn tick_clears_just_reloaded() {
        let mut a = Ammo::new(10, 30);
        a.current = 5;
        a.reload();
        a.tick();
        assert!(!a.just_reloaded);
    }

    #[test]
    fn needs_reload_true_when_below_max() {
        let mut a = Ammo::new(10, 30);
        a.fire();
        assert!(a.needs_reload());
    }

    #[test]
    fn magazine_fraction_at_half() {
        let mut a = Ammo::new(10, 30);
        a.current = 5;
        assert!((a.magazine_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn has_reserve_false_when_zero() {
        let mut a = Ammo::new(10, 30);
        a.reserve = 0;
        assert!(!a.has_reserve());
    }

    #[test]
    fn max_capacity_clamped_to_one() {
        let a = Ammo::new(0, 30);
        assert_eq!(a.max_capacity, 1);
        assert_eq!(a.current, 1);
    }
}

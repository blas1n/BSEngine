use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wad {
    pub bulk: f32,
    pub max_bulk: f32,
    pub pack_rate: f32,
    pub just_packed: bool,
    pub just_loose: bool,
    pub enabled: bool,
}

impl Default for Wad {
    fn default() -> Self {
        Self {
            bulk: 0.0,
            max_bulk: 100.0,
            pack_rate: 1.0,
            just_packed: false,
            just_loose: false,
            enabled: true,
        }
    }
}

impl Wad {
    pub fn pack(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_packed = false;
        self.just_loose = false;
        let prev = self.bulk;
        self.bulk = (self.bulk + amount).clamp(0.0, self.max_bulk);
        if self.bulk >= self.max_bulk && prev < self.max_bulk {
            self.just_packed = true;
        }
    }

    pub fn unpack(&mut self, amount: f32) {
        if !self.enabled || self.bulk <= 0.0 {
            return;
        }
        self.just_packed = false;
        self.just_loose = false;
        let prev = self.bulk;
        self.bulk = (self.bulk - amount).max(0.0);
        if self.bulk <= 0.0 && prev > 0.0 {
            self.just_loose = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.bulk >= self.max_bulk {
            return;
        }
        self.pack(self.pack_rate * dt);
    }

    pub fn is_packed(&self) -> bool {
        self.enabled && self.bulk >= self.max_bulk
    }

    pub fn is_loose(&self) -> bool {
        self.bulk <= 0.0
    }

    pub fn bulk_fraction(&self) -> f32 {
        if self.max_bulk <= 0.0 {
            return 0.0;
        }
        self.bulk / self.max_bulk
    }

    pub fn effective_mass(&self, scale: f32) -> f32 {
        self.bulk_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wad() -> Wad {
        Wad {
            bulk: 0.0,
            max_bulk: 100.0,
            pack_rate: 10.0,
            just_packed: false,
            just_loose: false,
            enabled: true,
        }
    }

    #[test]
    fn default_bulk_zero() {
        let w = Wad::default();
        assert_eq!(w.bulk, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wad::default().enabled);
    }

    #[test]
    fn pack_increases_bulk() {
        let mut w = wad();
        w.pack(30.0);
        assert_eq!(w.bulk, 30.0);
    }

    #[test]
    fn pack_clamps_at_max() {
        let mut w = wad();
        w.pack(200.0);
        assert_eq!(w.bulk, 100.0);
    }

    #[test]
    fn pack_no_op_when_disabled() {
        let mut w = wad();
        w.enabled = false;
        w.pack(50.0);
        assert_eq!(w.bulk, 0.0);
    }

    #[test]
    fn pack_sets_just_packed_at_max() {
        let mut w = wad();
        w.pack(100.0);
        assert!(w.just_packed);
    }

    #[test]
    fn pack_no_just_packed_if_already_max() {
        let mut w = wad();
        w.bulk = 100.0;
        w.pack(1.0);
        assert!(!w.just_packed);
    }

    #[test]
    fn unpack_decreases_bulk() {
        let mut w = wad();
        w.bulk = 60.0;
        w.unpack(20.0);
        assert_eq!(w.bulk, 40.0);
    }

    #[test]
    fn unpack_clamps_at_zero() {
        let mut w = wad();
        w.bulk = 30.0;
        w.unpack(200.0);
        assert_eq!(w.bulk, 0.0);
    }

    #[test]
    fn unpack_no_op_when_disabled() {
        let mut w = wad();
        w.bulk = 50.0;
        w.enabled = false;
        w.unpack(10.0);
        assert_eq!(w.bulk, 50.0);
    }

    #[test]
    fn unpack_no_op_when_already_loose() {
        let mut w = wad();
        w.unpack(10.0);
        assert_eq!(w.bulk, 0.0);
    }

    #[test]
    fn unpack_sets_just_loose_at_zero() {
        let mut w = wad();
        w.bulk = 10.0;
        w.unpack(10.0);
        assert!(w.just_loose);
    }

    #[test]
    fn unpack_no_just_loose_if_already_zero() {
        let mut w = wad();
        w.unpack(1.0);
        assert!(!w.just_loose);
    }

    #[test]
    fn tick_increases_bulk() {
        let mut w = wad();
        w.tick(1.0);
        assert_eq!(w.bulk, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wad();
        w.tick(2.0);
        assert_eq!(w.bulk, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wad();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.bulk, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_packed() {
        let mut w = wad();
        w.bulk = 100.0;
        w.tick(1.0);
        assert_eq!(w.bulk, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wad();
        w.pack_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.bulk, 0.0);
    }

    #[test]
    fn is_packed_true_at_max() {
        let mut w = wad();
        w.bulk = 100.0;
        assert!(w.is_packed());
    }

    #[test]
    fn is_packed_false_below_max() {
        let mut w = wad();
        w.bulk = 50.0;
        assert!(!w.is_packed());
    }

    #[test]
    fn is_packed_false_when_disabled() {
        let mut w = wad();
        w.bulk = 100.0;
        w.enabled = false;
        assert!(!w.is_packed());
    }

    #[test]
    fn is_loose_true_at_zero() {
        let w = wad();
        assert!(w.is_loose());
    }

    #[test]
    fn is_loose_false_above_zero() {
        let mut w = wad();
        w.bulk = 1.0;
        assert!(!w.is_loose());
    }

    #[test]
    fn bulk_fraction_zero_when_loose() {
        let w = wad();
        assert_eq!(w.bulk_fraction(), 0.0);
    }

    #[test]
    fn bulk_fraction_one_at_max() {
        let mut w = wad();
        w.bulk = 100.0;
        assert_eq!(w.bulk_fraction(), 1.0);
    }

    #[test]
    fn bulk_fraction_half_at_midpoint() {
        let mut w = wad();
        w.bulk = 50.0;
        assert_eq!(w.bulk_fraction(), 0.5);
    }

    #[test]
    fn bulk_fraction_zero_when_max_zero() {
        let mut w = wad();
        w.max_bulk = 0.0;
        assert_eq!(w.bulk_fraction(), 0.0);
    }

    #[test]
    fn effective_mass_scales() {
        let mut w = wad();
        w.bulk = 50.0;
        assert_eq!(w.effective_mass(2.0), 1.0);
    }

    #[test]
    fn effective_mass_zero_when_loose() {
        let w = wad();
        assert_eq!(w.effective_mass(10.0), 0.0);
    }

    #[test]
    fn just_packed_cleared_on_next_pack() {
        let mut w = wad();
        w.pack(100.0);
        assert!(w.just_packed);
        w.pack(1.0);
        assert!(!w.just_packed);
    }

    #[test]
    fn just_loose_cleared_on_next_unpack() {
        let mut w = wad();
        w.bulk = 10.0;
        w.unpack(10.0);
        assert!(w.just_loose);
        w.bulk = 10.0;
        w.unpack(1.0);
        assert!(!w.just_loose);
    }
}

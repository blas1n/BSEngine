use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wand {
    pub charge: f32,
    pub max_charge: f32,
    pub channel_rate: f32,
    pub just_charged: bool,
    pub just_spent: bool,
    pub enabled: bool,
}

impl Default for Wand {
    fn default() -> Self {
        Self {
            charge: 0.0,
            max_charge: 100.0,
            channel_rate: 1.0,
            just_charged: false,
            just_spent: false,
            enabled: true,
        }
    }
}

impl Wand {
    pub fn channel(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_charged = false;
        self.just_spent = false;
        let prev = self.charge;
        self.charge = (self.charge + amount).clamp(0.0, self.max_charge);
        if self.charge >= self.max_charge && prev < self.max_charge {
            self.just_charged = true;
        }
    }

    pub fn discharge(&mut self, amount: f32) {
        if !self.enabled || self.charge <= 0.0 {
            return;
        }
        self.just_charged = false;
        self.just_spent = false;
        let prev = self.charge;
        self.charge = (self.charge - amount).max(0.0);
        if self.charge <= 0.0 && prev > 0.0 {
            self.just_spent = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.charge >= self.max_charge {
            return;
        }
        self.channel(self.channel_rate * dt);
    }

    pub fn is_charged(&self) -> bool {
        self.enabled && self.charge >= self.max_charge
    }

    pub fn is_spent(&self) -> bool {
        self.charge <= 0.0
    }

    pub fn charge_fraction(&self) -> f32 {
        if self.max_charge <= 0.0 {
            return 0.0;
        }
        self.charge / self.max_charge
    }

    pub fn effective_power(&self, scale: f32) -> f32 {
        self.charge_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wand() -> Wand {
        Wand {
            charge: 0.0,
            max_charge: 100.0,
            channel_rate: 10.0,
            just_charged: false,
            just_spent: false,
            enabled: true,
        }
    }

    #[test]
    fn default_charge_zero() {
        let w = Wand::default();
        assert_eq!(w.charge, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wand::default().enabled);
    }

    #[test]
    fn channel_increases_charge() {
        let mut w = wand();
        w.channel(30.0);
        assert_eq!(w.charge, 30.0);
    }

    #[test]
    fn channel_clamps_at_max() {
        let mut w = wand();
        w.channel(200.0);
        assert_eq!(w.charge, 100.0);
    }

    #[test]
    fn channel_no_op_when_disabled() {
        let mut w = wand();
        w.enabled = false;
        w.channel(50.0);
        assert_eq!(w.charge, 0.0);
    }

    #[test]
    fn channel_sets_just_charged_at_max() {
        let mut w = wand();
        w.channel(100.0);
        assert!(w.just_charged);
    }

    #[test]
    fn channel_no_just_charged_if_already_max() {
        let mut w = wand();
        w.charge = 100.0;
        w.channel(1.0);
        assert!(!w.just_charged);
    }

    #[test]
    fn discharge_decreases_charge() {
        let mut w = wand();
        w.charge = 60.0;
        w.discharge(20.0);
        assert_eq!(w.charge, 40.0);
    }

    #[test]
    fn discharge_clamps_at_zero() {
        let mut w = wand();
        w.charge = 30.0;
        w.discharge(200.0);
        assert_eq!(w.charge, 0.0);
    }

    #[test]
    fn discharge_no_op_when_disabled() {
        let mut w = wand();
        w.charge = 50.0;
        w.enabled = false;
        w.discharge(10.0);
        assert_eq!(w.charge, 50.0);
    }

    #[test]
    fn discharge_no_op_when_already_spent() {
        let mut w = wand();
        w.discharge(10.0);
        assert_eq!(w.charge, 0.0);
    }

    #[test]
    fn discharge_sets_just_spent_at_zero() {
        let mut w = wand();
        w.charge = 10.0;
        w.discharge(10.0);
        assert!(w.just_spent);
    }

    #[test]
    fn discharge_no_just_spent_if_already_zero() {
        let mut w = wand();
        w.discharge(1.0);
        assert!(!w.just_spent);
    }

    #[test]
    fn tick_increases_charge() {
        let mut w = wand();
        w.tick(1.0);
        assert_eq!(w.charge, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wand();
        w.tick(2.0);
        assert_eq!(w.charge, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wand();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.charge, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_charged() {
        let mut w = wand();
        w.charge = 100.0;
        w.tick(1.0);
        assert_eq!(w.charge, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wand();
        w.channel_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.charge, 0.0);
    }

    #[test]
    fn is_charged_true_at_max() {
        let mut w = wand();
        w.charge = 100.0;
        assert!(w.is_charged());
    }

    #[test]
    fn is_charged_false_below_max() {
        let mut w = wand();
        w.charge = 50.0;
        assert!(!w.is_charged());
    }

    #[test]
    fn is_charged_false_when_disabled() {
        let mut w = wand();
        w.charge = 100.0;
        w.enabled = false;
        assert!(!w.is_charged());
    }

    #[test]
    fn is_spent_true_at_zero() {
        let w = wand();
        assert!(w.is_spent());
    }

    #[test]
    fn is_spent_false_above_zero() {
        let mut w = wand();
        w.charge = 1.0;
        assert!(!w.is_spent());
    }

    #[test]
    fn charge_fraction_zero_when_spent() {
        let w = wand();
        assert_eq!(w.charge_fraction(), 0.0);
    }

    #[test]
    fn charge_fraction_one_at_max() {
        let mut w = wand();
        w.charge = 100.0;
        assert_eq!(w.charge_fraction(), 1.0);
    }

    #[test]
    fn charge_fraction_half_at_midpoint() {
        let mut w = wand();
        w.charge = 50.0;
        assert_eq!(w.charge_fraction(), 0.5);
    }

    #[test]
    fn charge_fraction_zero_when_max_zero() {
        let mut w = wand();
        w.max_charge = 0.0;
        assert_eq!(w.charge_fraction(), 0.0);
    }

    #[test]
    fn effective_power_scales() {
        let mut w = wand();
        w.charge = 50.0;
        assert_eq!(w.effective_power(2.0), 1.0);
    }

    #[test]
    fn effective_power_zero_when_spent() {
        let w = wand();
        assert_eq!(w.effective_power(10.0), 0.0);
    }

    #[test]
    fn just_charged_cleared_on_next_channel() {
        let mut w = wand();
        w.channel(100.0);
        assert!(w.just_charged);
        w.channel(1.0);
        assert!(!w.just_charged);
    }

    #[test]
    fn just_spent_cleared_on_next_discharge() {
        let mut w = wand();
        w.charge = 10.0;
        w.discharge(10.0);
        assert!(w.just_spent);
        w.charge = 10.0;
        w.discharge(1.0);
        assert!(!w.just_spent);
    }
}

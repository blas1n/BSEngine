use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Warrant {
    pub authority: f32,
    pub max_authority: f32,
    pub sanction_rate: f32,
    pub just_authorized: bool,
    pub just_revoked: bool,
    pub enabled: bool,
}

impl Default for Warrant {
    fn default() -> Self {
        Self {
            authority: 0.0,
            max_authority: 100.0,
            sanction_rate: 1.0,
            just_authorized: false,
            just_revoked: false,
            enabled: true,
        }
    }
}

impl Warrant {
    pub fn sanction(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_authorized = false;
        self.just_revoked = false;
        let prev = self.authority;
        self.authority = (self.authority + amount).clamp(0.0, self.max_authority);
        if self.authority >= self.max_authority && prev < self.max_authority {
            self.just_authorized = true;
        }
    }

    pub fn revoke(&mut self, amount: f32) {
        if !self.enabled || self.authority <= 0.0 {
            return;
        }
        self.just_authorized = false;
        self.just_revoked = false;
        let prev = self.authority;
        self.authority = (self.authority - amount).max(0.0);
        if self.authority <= 0.0 && prev > 0.0 {
            self.just_revoked = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.authority >= self.max_authority {
            return;
        }
        self.sanction(self.sanction_rate * dt);
    }

    pub fn is_authorized(&self) -> bool {
        self.enabled && self.authority >= self.max_authority
    }

    pub fn is_revoked(&self) -> bool {
        self.authority <= 0.0
    }

    pub fn authority_fraction(&self) -> f32 {
        if self.max_authority <= 0.0 {
            return 0.0;
        }
        self.authority / self.max_authority
    }

    pub fn effective_mandate(&self, scale: f32) -> f32 {
        self.authority_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warrant() -> Warrant {
        Warrant {
            authority: 0.0,
            max_authority: 100.0,
            sanction_rate: 10.0,
            just_authorized: false,
            just_revoked: false,
            enabled: true,
        }
    }

    #[test]
    fn default_authority_zero() {
        let w = Warrant::default();
        assert_eq!(w.authority, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Warrant::default().enabled);
    }

    #[test]
    fn sanction_increases_authority() {
        let mut w = warrant();
        w.sanction(30.0);
        assert_eq!(w.authority, 30.0);
    }

    #[test]
    fn sanction_clamps_at_max() {
        let mut w = warrant();
        w.sanction(200.0);
        assert_eq!(w.authority, 100.0);
    }

    #[test]
    fn sanction_no_op_when_disabled() {
        let mut w = warrant();
        w.enabled = false;
        w.sanction(50.0);
        assert_eq!(w.authority, 0.0);
    }

    #[test]
    fn sanction_sets_just_authorized_at_max() {
        let mut w = warrant();
        w.sanction(100.0);
        assert!(w.just_authorized);
    }

    #[test]
    fn sanction_no_just_authorized_if_already_max() {
        let mut w = warrant();
        w.authority = 100.0;
        w.sanction(1.0);
        assert!(!w.just_authorized);
    }

    #[test]
    fn revoke_decreases_authority() {
        let mut w = warrant();
        w.authority = 60.0;
        w.revoke(20.0);
        assert_eq!(w.authority, 40.0);
    }

    #[test]
    fn revoke_clamps_at_zero() {
        let mut w = warrant();
        w.authority = 30.0;
        w.revoke(200.0);
        assert_eq!(w.authority, 0.0);
    }

    #[test]
    fn revoke_no_op_when_disabled() {
        let mut w = warrant();
        w.authority = 50.0;
        w.enabled = false;
        w.revoke(10.0);
        assert_eq!(w.authority, 50.0);
    }

    #[test]
    fn revoke_no_op_when_already_revoked() {
        let mut w = warrant();
        w.revoke(10.0);
        assert_eq!(w.authority, 0.0);
    }

    #[test]
    fn revoke_sets_just_revoked_at_zero() {
        let mut w = warrant();
        w.authority = 10.0;
        w.revoke(10.0);
        assert!(w.just_revoked);
    }

    #[test]
    fn revoke_no_just_revoked_if_already_zero() {
        let mut w = warrant();
        w.revoke(1.0);
        assert!(!w.just_revoked);
    }

    #[test]
    fn tick_increases_authority() {
        let mut w = warrant();
        w.tick(1.0);
        assert_eq!(w.authority, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = warrant();
        w.tick(2.0);
        assert_eq!(w.authority, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = warrant();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.authority, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_authorized() {
        let mut w = warrant();
        w.authority = 100.0;
        w.tick(1.0);
        assert_eq!(w.authority, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = warrant();
        w.sanction_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.authority, 0.0);
    }

    #[test]
    fn is_authorized_true_at_max() {
        let mut w = warrant();
        w.authority = 100.0;
        assert!(w.is_authorized());
    }

    #[test]
    fn is_authorized_false_below_max() {
        let mut w = warrant();
        w.authority = 50.0;
        assert!(!w.is_authorized());
    }

    #[test]
    fn is_authorized_false_when_disabled() {
        let mut w = warrant();
        w.authority = 100.0;
        w.enabled = false;
        assert!(!w.is_authorized());
    }

    #[test]
    fn is_revoked_true_at_zero() {
        let w = warrant();
        assert!(w.is_revoked());
    }

    #[test]
    fn is_revoked_false_above_zero() {
        let mut w = warrant();
        w.authority = 1.0;
        assert!(!w.is_revoked());
    }

    #[test]
    fn authority_fraction_zero_when_revoked() {
        let w = warrant();
        assert_eq!(w.authority_fraction(), 0.0);
    }

    #[test]
    fn authority_fraction_one_at_max() {
        let mut w = warrant();
        w.authority = 100.0;
        assert_eq!(w.authority_fraction(), 1.0);
    }

    #[test]
    fn authority_fraction_half_at_midpoint() {
        let mut w = warrant();
        w.authority = 50.0;
        assert_eq!(w.authority_fraction(), 0.5);
    }

    #[test]
    fn authority_fraction_zero_when_max_zero() {
        let mut w = warrant();
        w.max_authority = 0.0;
        assert_eq!(w.authority_fraction(), 0.0);
    }

    #[test]
    fn effective_mandate_scales() {
        let mut w = warrant();
        w.authority = 50.0;
        assert_eq!(w.effective_mandate(2.0), 1.0);
    }

    #[test]
    fn effective_mandate_zero_when_revoked() {
        let w = warrant();
        assert_eq!(w.effective_mandate(10.0), 0.0);
    }

    #[test]
    fn just_authorized_cleared_on_next_sanction() {
        let mut w = warrant();
        w.sanction(100.0);
        assert!(w.just_authorized);
        w.sanction(1.0);
        assert!(!w.just_authorized);
    }

    #[test]
    fn just_revoked_cleared_on_next_revoke() {
        let mut w = warrant();
        w.authority = 10.0;
        w.revoke(10.0);
        assert!(w.just_revoked);
        w.authority = 10.0;
        w.revoke(1.0);
        assert!(!w.just_revoked);
    }
}

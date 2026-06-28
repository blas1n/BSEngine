use bevy_ecs::component::Component;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wager {
    pub stake: f32,
    pub max_stake: f32,
    pub risk_rate: f32,
    pub just_won: bool,
    pub just_lost: bool,
    pub enabled: bool,
}

impl Default for Wager {
    fn default() -> Self {
        Self {
            stake: 0.0,
            max_stake: 100.0,
            risk_rate: 1.0,
            just_won: false,
            just_lost: false,
            enabled: true,
        }
    }
}

impl Wager {
    pub fn bet(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }
        self.just_won = false;
        self.just_lost = false;
        let prev = self.stake;
        self.stake = (self.stake + amount).clamp(0.0, self.max_stake);
        if self.stake >= self.max_stake && prev < self.max_stake {
            self.just_won = true;
        }
    }

    pub fn forfeit(&mut self, amount: f32) {
        if !self.enabled || self.stake <= 0.0 {
            return;
        }
        self.just_won = false;
        self.just_lost = false;
        let prev = self.stake;
        self.stake = (self.stake - amount).max(0.0);
        if self.stake <= 0.0 && prev > 0.0 {
            self.just_lost = true;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.enabled || self.stake >= self.max_stake {
            return;
        }
        self.bet(self.risk_rate * dt);
    }

    pub fn is_won(&self) -> bool {
        self.enabled && self.stake >= self.max_stake
    }

    pub fn is_lost(&self) -> bool {
        self.stake <= 0.0
    }

    pub fn stake_fraction(&self) -> f32 {
        if self.max_stake <= 0.0 {
            return 0.0;
        }
        self.stake / self.max_stake
    }

    pub fn effective_odds(&self, scale: f32) -> f32 {
        self.stake_fraction() * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wager() -> Wager {
        Wager {
            stake: 0.0,
            max_stake: 100.0,
            risk_rate: 10.0,
            just_won: false,
            just_lost: false,
            enabled: true,
        }
    }

    #[test]
    fn default_stake_zero() {
        let w = Wager::default();
        assert_eq!(w.stake, 0.0);
    }

    #[test]
    fn default_enabled() {
        assert!(Wager::default().enabled);
    }

    #[test]
    fn bet_increases_stake() {
        let mut w = wager();
        w.bet(30.0);
        assert_eq!(w.stake, 30.0);
    }

    #[test]
    fn bet_clamps_at_max() {
        let mut w = wager();
        w.bet(200.0);
        assert_eq!(w.stake, 100.0);
    }

    #[test]
    fn bet_no_op_when_disabled() {
        let mut w = wager();
        w.enabled = false;
        w.bet(50.0);
        assert_eq!(w.stake, 0.0);
    }

    #[test]
    fn bet_sets_just_won_at_max() {
        let mut w = wager();
        w.bet(100.0);
        assert!(w.just_won);
    }

    #[test]
    fn bet_no_just_won_if_already_max() {
        let mut w = wager();
        w.stake = 100.0;
        w.bet(1.0);
        assert!(!w.just_won);
    }

    #[test]
    fn forfeit_decreases_stake() {
        let mut w = wager();
        w.stake = 60.0;
        w.forfeit(20.0);
        assert_eq!(w.stake, 40.0);
    }

    #[test]
    fn forfeit_clamps_at_zero() {
        let mut w = wager();
        w.stake = 30.0;
        w.forfeit(200.0);
        assert_eq!(w.stake, 0.0);
    }

    #[test]
    fn forfeit_no_op_when_disabled() {
        let mut w = wager();
        w.stake = 50.0;
        w.enabled = false;
        w.forfeit(10.0);
        assert_eq!(w.stake, 50.0);
    }

    #[test]
    fn forfeit_no_op_when_already_zero() {
        let mut w = wager();
        w.forfeit(10.0);
        assert_eq!(w.stake, 0.0);
    }

    #[test]
    fn forfeit_sets_just_lost_at_zero() {
        let mut w = wager();
        w.stake = 10.0;
        w.forfeit(10.0);
        assert!(w.just_lost);
    }

    #[test]
    fn forfeit_no_just_lost_if_already_zero() {
        let mut w = wager();
        w.forfeit(1.0);
        assert!(!w.just_lost);
    }

    #[test]
    fn tick_increases_stake() {
        let mut w = wager();
        w.tick(1.0);
        assert_eq!(w.stake, 10.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = wager();
        w.tick(2.0);
        assert_eq!(w.stake, 20.0);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = wager();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.stake, 0.0);
    }

    #[test]
    fn tick_no_op_when_already_won() {
        let mut w = wager();
        w.stake = 100.0;
        w.tick(1.0);
        assert_eq!(w.stake, 100.0);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = wager();
        w.risk_rate = 0.0;
        w.tick(1.0);
        assert_eq!(w.stake, 0.0);
    }

    #[test]
    fn is_won_true_at_max() {
        let mut w = wager();
        w.stake = 100.0;
        assert!(w.is_won());
    }

    #[test]
    fn is_won_false_below_max() {
        let mut w = wager();
        w.stake = 50.0;
        assert!(!w.is_won());
    }

    #[test]
    fn is_won_false_when_disabled() {
        let mut w = wager();
        w.stake = 100.0;
        w.enabled = false;
        assert!(!w.is_won());
    }

    #[test]
    fn is_lost_true_at_zero() {
        let w = wager();
        assert!(w.is_lost());
    }

    #[test]
    fn is_lost_false_above_zero() {
        let mut w = wager();
        w.stake = 1.0;
        assert!(!w.is_lost());
    }

    #[test]
    fn stake_fraction_zero_when_lost() {
        let w = wager();
        assert_eq!(w.stake_fraction(), 0.0);
    }

    #[test]
    fn stake_fraction_one_at_max() {
        let mut w = wager();
        w.stake = 100.0;
        assert_eq!(w.stake_fraction(), 1.0);
    }

    #[test]
    fn stake_fraction_half_at_midpoint() {
        let mut w = wager();
        w.stake = 50.0;
        assert_eq!(w.stake_fraction(), 0.5);
    }

    #[test]
    fn stake_fraction_zero_when_max_zero() {
        let mut w = wager();
        w.max_stake = 0.0;
        assert_eq!(w.stake_fraction(), 0.0);
    }

    #[test]
    fn effective_odds_scales() {
        let mut w = wager();
        w.stake = 50.0;
        assert_eq!(w.effective_odds(2.0), 1.0);
    }

    #[test]
    fn effective_odds_zero_when_lost() {
        let w = wager();
        assert_eq!(w.effective_odds(10.0), 0.0);
    }

    #[test]
    fn just_won_cleared_on_next_bet() {
        let mut w = wager();
        w.bet(100.0);
        assert!(w.just_won);
        w.bet(1.0);
        assert!(!w.just_won);
    }

    #[test]
    fn just_lost_cleared_on_next_forfeit() {
        let mut w = wager();
        w.stake = 10.0;
        w.forfeit(10.0);
        assert!(w.just_lost);
        w.stake = 10.0;
        w.forfeit(1.0);
        assert!(!w.just_lost);
    }
}

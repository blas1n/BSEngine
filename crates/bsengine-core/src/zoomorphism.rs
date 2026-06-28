use bevy_ecs::prelude::Component;

/// Animal-form-attribution accumulation tracker named after zoomorphism,
/// the representation of a deity, spiritual being, or abstract force in
/// animal shape — or more broadly the attribution of animal character-
/// istics to any entity not ordinarily conceived as an animal. The
/// practice is among the oldest documented religious behaviours: the
/// jackal-headed Anubis and ibis-headed Thoth of the Egyptian pantheon,
/// the eagle-lion griffin of Mesopotamia, the feathered-serpent
/// Quetzalcóatl of Mesoamerica, and the bear-clan totems of the
/// Northwest Coast peoples are all zoomorphic expressions of sacred
/// power. Ancient Greeks distinguished their own anthropomorphic
/// tendency — gods made in human likeness — from the zoomorphism of
/// neighbouring cultures; Herodotus noted with some puzzlement that
/// Egyptian priests regarded the sacred ibis or crocodile as a direct
/// earthly vehicle of the divine. In art history the term extends to
/// secular ornament: Hiberno-Saxon illuminators crammed the Book of
/// Kells with intertwined zoomorphic knots in which serpentine bodies
/// mutate seamlessly into stylised animal heads; Viking carvers covered
/// ship prows and runestones with curling beasts whose legs become
/// foliage and whose open jaws swallow their own tails. `expression`
/// builds via `channel(amount)` and accumulates passively at
/// `channel_rate` per second in `tick(dt)` or withdraws via
/// `withdraw(amount)`.
///
/// Models zoomorphic-expression fill levels, totem-animal saturation
/// bars, deity-in-beast-form manifestation trackers, animal-mask ritual-
/// power gauges, shapeshifter-charge fill levels, spirit-beast resonance
/// saturation indicators, ornamental-beast-coil complexity accumulators,
/// divine-animal-form attunement bars, theriomorphic-power charge
/// meters, or any mechanic where a character, artefact, or faction slowly
/// channels the sacred energy of an animal archetype — moving from the
/// decorative bead carved with a bear to the full totem-pole guardian
/// whose presence reshapes the community beneath it.
///
/// `channel(amount)` adds expression; fires `just_manifested` when first
/// reaching `max_expression`. No-op when disabled.
///
/// `withdraw(amount)` reduces expression immediately; fires `just_lapsed`
/// when reaching 0. No-op when disabled or already lapsed.
///
/// `tick(dt)` clears both flags, then increases expression by
/// `channel_rate * dt` (capped at `max_expression`). Fires
/// `just_manifested` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_manifested()` returns `expression >= max_expression && enabled`.
///
/// `is_lapsed()` returns `expression == 0.0` (not gated by `enabled`).
///
/// `expression_fraction()` returns
/// `(expression / max_expression).clamp(0, 1)`.
///
/// `effective_totem(scale)` returns `scale * expression_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — channels at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoomorphism {
    pub expression: f32,
    pub max_expression: f32,
    pub channel_rate: f32,
    pub just_manifested: bool,
    pub just_lapsed: bool,
    pub enabled: bool,
}

impl Zoomorphism {
    pub fn new(max_expression: f32, channel_rate: f32) -> Self {
        Self {
            expression: 0.0,
            max_expression: max_expression.max(0.1),
            channel_rate: channel_rate.max(0.0),
            just_manifested: false,
            just_lapsed: false,
            enabled: true,
        }
    }

    /// Add expression; fires `just_manifested` when first reaching max.
    /// No-op when disabled.
    pub fn channel(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.expression < self.max_expression;
        self.expression = (self.expression + amount).min(self.max_expression);
        if was_below && self.expression >= self.max_expression {
            self.just_manifested = true;
        }
    }

    /// Reduce expression; fires `just_lapsed` when reaching 0.
    /// No-op when disabled or already lapsed.
    pub fn withdraw(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.expression <= 0.0 {
            return;
        }
        self.expression = (self.expression - amount).max(0.0);
        if self.expression <= 0.0 {
            self.just_lapsed = true;
        }
    }

    /// Clear flags, then increase expression by `channel_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_manifested = false;
        self.just_lapsed = false;
        if self.enabled && self.channel_rate > 0.0 && self.expression < self.max_expression {
            let was_below = self.expression < self.max_expression;
            self.expression = (self.expression + self.channel_rate * dt).min(self.max_expression);
            if was_below && self.expression >= self.max_expression {
                self.just_manifested = true;
            }
        }
    }

    /// `true` when expression is at maximum and component is enabled.
    pub fn is_manifested(&self) -> bool {
        self.expression >= self.max_expression && self.enabled
    }

    /// `true` when expression is 0 (not gated by `enabled`).
    pub fn is_lapsed(&self) -> bool {
        self.expression == 0.0
    }

    /// Fraction of maximum expression [0.0, 1.0].
    pub fn expression_fraction(&self) -> f32 {
        (self.expression / self.max_expression).clamp(0.0, 1.0)
    }

    /// Returns `scale * expression_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_totem(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.expression_fraction()
    }
}

impl Default for Zoomorphism {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoomorphism {
        Zoomorphism::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_lapsed() {
        let z = z();
        assert_eq!(z.expression, 0.0);
        assert!(z.is_lapsed());
        assert!(!z.is_manifested());
    }

    #[test]
    fn new_clamps_max_expression() {
        let z = Zoomorphism::new(-5.0, 1.5);
        assert!((z.max_expression - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_channel_rate() {
        let z = Zoomorphism::new(100.0, -1.5);
        assert_eq!(z.channel_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoomorphism::default();
        assert!((z.max_expression - 100.0).abs() < 1e-5);
        assert!((z.channel_rate - 1.5).abs() < 1e-5);
    }

    // --- channel ---

    #[test]
    fn channel_adds_expression() {
        let mut z = z();
        z.channel(40.0);
        assert!((z.expression - 40.0).abs() < 1e-3);
    }

    #[test]
    fn channel_clamps_at_max() {
        let mut z = z();
        z.channel(200.0);
        assert!((z.expression - 100.0).abs() < 1e-3);
    }

    #[test]
    fn channel_fires_just_manifested_at_max() {
        let mut z = z();
        z.channel(100.0);
        assert!(z.just_manifested);
        assert!(z.is_manifested());
    }

    #[test]
    fn channel_no_just_manifested_when_already_at_max() {
        let mut z = z();
        z.expression = 100.0;
        z.channel(10.0);
        assert!(!z.just_manifested);
    }

    #[test]
    fn channel_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.channel(50.0);
        assert_eq!(z.expression, 0.0);
    }

    #[test]
    fn channel_no_op_when_amount_zero() {
        let mut z = z();
        z.channel(0.0);
        assert_eq!(z.expression, 0.0);
    }

    // --- withdraw ---

    #[test]
    fn withdraw_reduces_expression() {
        let mut z = z();
        z.expression = 60.0;
        z.withdraw(20.0);
        assert!((z.expression - 40.0).abs() < 1e-3);
    }

    #[test]
    fn withdraw_clamps_at_zero() {
        let mut z = z();
        z.expression = 30.0;
        z.withdraw(200.0);
        assert_eq!(z.expression, 0.0);
    }

    #[test]
    fn withdraw_fires_just_lapsed_at_zero() {
        let mut z = z();
        z.expression = 30.0;
        z.withdraw(30.0);
        assert!(z.just_lapsed);
    }

    #[test]
    fn withdraw_no_op_when_already_lapsed() {
        let mut z = z();
        z.withdraw(10.0);
        assert!(!z.just_lapsed);
    }

    #[test]
    fn withdraw_no_op_when_disabled() {
        let mut z = z();
        z.expression = 50.0;
        z.enabled = false;
        z.withdraw(50.0);
        assert!((z.expression - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_channels_expression() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.expression - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_manifested_on_channel_to_max() {
        let mut z = Zoomorphism::new(100.0, 200.0);
        z.expression = 95.0;
        z.tick(1.0);
        assert!(z.just_manifested);
        assert!(z.is_manifested());
    }

    #[test]
    fn tick_no_channel_when_already_manifested() {
        let mut z = z();
        z.expression = 100.0;
        z.tick(1.0);
        assert!(!z.just_manifested);
    }

    #[test]
    fn tick_no_channel_when_rate_zero() {
        let mut z = Zoomorphism::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.expression, 0.0);
    }

    #[test]
    fn tick_no_channel_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.expression, 0.0);
    }

    #[test]
    fn tick_clears_just_manifested() {
        let mut z = Zoomorphism::new(100.0, 200.0);
        z.expression = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_manifested);
    }

    #[test]
    fn tick_clears_just_lapsed() {
        let mut z = z();
        z.expression = 10.0;
        z.withdraw(10.0);
        z.tick(0.016);
        assert!(!z.just_lapsed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.expression - 9.0).abs() < 1e-3);
    }

    // --- is_manifested / is_lapsed ---

    #[test]
    fn is_manifested_false_when_disabled() {
        let mut z = z();
        z.expression = 100.0;
        z.enabled = false;
        assert!(!z.is_manifested());
    }

    #[test]
    fn is_lapsed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_lapsed());
    }

    // --- expression_fraction / effective_totem ---

    #[test]
    fn expression_fraction_zero_when_lapsed() {
        assert_eq!(z().expression_fraction(), 0.0);
    }

    #[test]
    fn expression_fraction_half_at_midpoint() {
        let mut z = z();
        z.expression = 50.0;
        assert!((z.expression_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_totem_zero_when_lapsed() {
        assert_eq!(z().effective_totem(100.0), 0.0);
    }

    #[test]
    fn effective_totem_scales_with_expression() {
        let mut z = z();
        z.expression = 75.0;
        assert!((z.effective_totem(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_totem_zero_when_disabled() {
        let mut z = z();
        z.expression = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_totem(100.0), 0.0);
    }
}

use bevy_ecs::prelude::Component;

/// Hebrew-letter inscription tracker named after zayin (ז), the
/// seventh letter of the Hebrew alphabet, traditionally depicted
/// as a crowned weapon or sceptre whose angular stroke sits at the
/// junction of word and silence. Zayin has numerical value seven,
/// appears in words for arms, time, and nourishment, and gives its
/// shape — two vertical strokes joined by a horizontal bar — to
/// the letter Z itself through the Phoenician → Greek → Latin
/// chain of transmission. `inscription` builds via
/// `inscribe(amount)` and accumulates passively at `scribe_rate`
/// per second in `tick(dt)` or is erased via `erase(amount)`.
///
/// Models sacred-text transcription fill levels, illuminated-
/// manuscript completion bars, runic-inscription saturation
/// trackers, calligraphy-practice accumulation gauges, scribal-
/// workshop output meters, stone-carving progress indicators,
/// Torah-scroll completion fill bars, cuneiform-tablet inscription
/// accumulation trackers, ancient-letter-form preservation bars,
/// or any mechanic where patient letter-by-letter inscription
/// gradually fills a surface with the kind of dense, angular
/// script that carries meaning across millennia — right up
/// until a damp cloth or a chisel strike erases every careful
/// stroke back to bare parchment.
///
/// `inscribe(amount)` adds inscription; fires `just_inscribed`
/// when first reaching `max_inscription`. No-op when disabled.
///
/// `erase(amount)` reduces inscription immediately; fires
/// `just_blank` when reaching 0. No-op when disabled or already
/// blank.
///
/// `tick(dt)` clears both flags, then increases inscription by
/// `scribe_rate * dt` (capped at `max_inscription`). Fires
/// `just_inscribed` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_inscribed()` returns `inscription >= max_inscription && enabled`.
///
/// `is_blank()` returns `inscription == 0.0` (not gated by `enabled`).
///
/// `inscription_fraction()` returns `(inscription / max_inscription).clamp(0, 1)`.
///
/// `effective_script(scale)` returns `scale * inscription_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — scribes at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zayin {
    pub inscription: f32,
    pub max_inscription: f32,
    pub scribe_rate: f32,
    pub just_inscribed: bool,
    pub just_blank: bool,
    pub enabled: bool,
}

impl Zayin {
    pub fn new(max_inscription: f32, scribe_rate: f32) -> Self {
        Self {
            inscription: 0.0,
            max_inscription: max_inscription.max(0.1),
            scribe_rate: scribe_rate.max(0.0),
            just_inscribed: false,
            just_blank: false,
            enabled: true,
        }
    }

    /// Add inscription; fires `just_inscribed` when first reaching max.
    /// No-op when disabled.
    pub fn inscribe(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.inscription < self.max_inscription;
        self.inscription = (self.inscription + amount).min(self.max_inscription);
        if was_below && self.inscription >= self.max_inscription {
            self.just_inscribed = true;
        }
    }

    /// Reduce inscription; fires `just_blank` when reaching 0.
    /// No-op when disabled or already blank.
    pub fn erase(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.inscription <= 0.0 {
            return;
        }
        self.inscription = (self.inscription - amount).max(0.0);
        if self.inscription <= 0.0 {
            self.just_blank = true;
        }
    }

    /// Clear flags, then increase inscription by `scribe_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_inscribed = false;
        self.just_blank = false;
        if self.enabled && self.scribe_rate > 0.0 && self.inscription < self.max_inscription {
            let was_below = self.inscription < self.max_inscription;
            self.inscription = (self.inscription + self.scribe_rate * dt).min(self.max_inscription);
            if was_below && self.inscription >= self.max_inscription {
                self.just_inscribed = true;
            }
        }
    }

    /// `true` when inscription is at maximum and component is enabled.
    pub fn is_inscribed(&self) -> bool {
        self.inscription >= self.max_inscription && self.enabled
    }

    /// `true` when inscription is 0 (not gated by `enabled`).
    pub fn is_blank(&self) -> bool {
        self.inscription == 0.0
    }

    /// Fraction of maximum inscription [0.0, 1.0].
    pub fn inscription_fraction(&self) -> f32 {
        (self.inscription / self.max_inscription).clamp(0.0, 1.0)
    }

    /// Returns `scale * inscription_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_script(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.inscription_fraction()
    }
}

impl Default for Zayin {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zayin {
        Zayin::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_blank() {
        let z = z();
        assert_eq!(z.inscription, 0.0);
        assert!(z.is_blank());
        assert!(!z.is_inscribed());
    }

    #[test]
    fn new_clamps_max_inscription() {
        let z = Zayin::new(-5.0, 1.5);
        assert!((z.max_inscription - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_scribe_rate() {
        let z = Zayin::new(100.0, -1.5);
        assert_eq!(z.scribe_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zayin::default();
        assert!((z.max_inscription - 100.0).abs() < 1e-5);
        assert!((z.scribe_rate - 1.5).abs() < 1e-5);
    }

    // --- inscribe ---

    #[test]
    fn inscribe_adds_inscription() {
        let mut z = z();
        z.inscribe(40.0);
        assert!((z.inscription - 40.0).abs() < 1e-3);
    }

    #[test]
    fn inscribe_clamps_at_max() {
        let mut z = z();
        z.inscribe(200.0);
        assert!((z.inscription - 100.0).abs() < 1e-3);
    }

    #[test]
    fn inscribe_fires_just_inscribed_at_max() {
        let mut z = z();
        z.inscribe(100.0);
        assert!(z.just_inscribed);
        assert!(z.is_inscribed());
    }

    #[test]
    fn inscribe_no_just_inscribed_when_already_at_max() {
        let mut z = z();
        z.inscription = 100.0;
        z.inscribe(10.0);
        assert!(!z.just_inscribed);
    }

    #[test]
    fn inscribe_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.inscribe(50.0);
        assert_eq!(z.inscription, 0.0);
    }

    #[test]
    fn inscribe_no_op_when_amount_zero() {
        let mut z = z();
        z.inscribe(0.0);
        assert_eq!(z.inscription, 0.0);
    }

    // --- erase ---

    #[test]
    fn erase_reduces_inscription() {
        let mut z = z();
        z.inscription = 60.0;
        z.erase(20.0);
        assert!((z.inscription - 40.0).abs() < 1e-3);
    }

    #[test]
    fn erase_clamps_at_zero() {
        let mut z = z();
        z.inscription = 30.0;
        z.erase(200.0);
        assert_eq!(z.inscription, 0.0);
    }

    #[test]
    fn erase_fires_just_blank_at_zero() {
        let mut z = z();
        z.inscription = 30.0;
        z.erase(30.0);
        assert!(z.just_blank);
    }

    #[test]
    fn erase_no_op_when_already_blank() {
        let mut z = z();
        z.erase(10.0);
        assert!(!z.just_blank);
    }

    #[test]
    fn erase_no_op_when_disabled() {
        let mut z = z();
        z.inscription = 50.0;
        z.enabled = false;
        z.erase(50.0);
        assert!((z.inscription - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_scribes_inscription() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.inscription - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_inscribed_on_scribe_to_max() {
        let mut z = Zayin::new(100.0, 200.0);
        z.inscription = 95.0;
        z.tick(1.0);
        assert!(z.just_inscribed);
        assert!(z.is_inscribed());
    }

    #[test]
    fn tick_no_scribe_when_already_inscribed() {
        let mut z = z();
        z.inscription = 100.0;
        z.tick(1.0);
        assert!(!z.just_inscribed);
    }

    #[test]
    fn tick_no_scribe_when_rate_zero() {
        let mut z = Zayin::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.inscription, 0.0);
    }

    #[test]
    fn tick_no_scribe_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.inscription, 0.0);
    }

    #[test]
    fn tick_clears_just_inscribed() {
        let mut z = Zayin::new(100.0, 200.0);
        z.inscription = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_inscribed);
    }

    #[test]
    fn tick_clears_just_blank() {
        let mut z = z();
        z.inscription = 10.0;
        z.erase(10.0);
        z.tick(0.016);
        assert!(!z.just_blank);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.inscription - 9.0).abs() < 1e-3);
    }

    // --- is_inscribed / is_blank ---

    #[test]
    fn is_inscribed_false_when_disabled() {
        let mut z = z();
        z.inscription = 100.0;
        z.enabled = false;
        assert!(!z.is_inscribed());
    }

    #[test]
    fn is_blank_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_blank());
    }

    // --- inscription_fraction / effective_script ---

    #[test]
    fn inscription_fraction_zero_when_blank() {
        assert_eq!(z().inscription_fraction(), 0.0);
    }

    #[test]
    fn inscription_fraction_half_at_midpoint() {
        let mut z = z();
        z.inscription = 50.0;
        assert!((z.inscription_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_script_zero_when_blank() {
        assert_eq!(z().effective_script(100.0), 0.0);
    }

    #[test]
    fn effective_script_scales_with_inscription() {
        let mut z = z();
        z.inscription = 75.0;
        assert!((z.effective_script(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_script_zero_when_disabled() {
        let mut z = z();
        z.inscription = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_script(100.0), 0.0);
    }
}

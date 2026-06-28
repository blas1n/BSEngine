use bevy_ecs::prelude::Component;

/// Inscription-record accumulation tracker named after write, the
/// verb meaning to mark letters, words, or other symbols on a
/// surface; to compose and record in a lasting form; to inscribe
/// — from the Old English wrītan (to score, to scratch, to
/// carve, to write), from the Proto-Germanic wrītaną (to
/// tear, to scratch, to carve), from the Proto-Indo-European
/// root wreid- (to tear, to scratch). The original sense is
/// physical and violent: to write was to scratch, to incise,
/// to score a surface with a pointed tool, leaving marks that
/// could later be read. The earliest writing systems — cuneiform
/// on clay tablets, runes on stone and bone — were literally
/// scratched into their surfaces, and the instruments of their
/// inscription were styluses, chisels, and knives. The transition
/// from scratching to inking, from scoring to painting, from
/// carving to typing, has changed the physical act of writing
/// beyond recognition while leaving the word unchanged. To
/// write remains to make a permanent mark — to cause something
/// to persist that would otherwise be lost. In metaphorical
/// usage, writing is the paradigmatic act of fixing the
/// transient: to write something down is to rescue it from
/// time, to make it recoverable, to ensure it will outlast
/// the moment that produced it. In game mechanics, a write
/// mechanic models the slow accumulation of inscription —
/// the filling of a page, the completion of a record, the
/// build of text that eventually reaches the threshold at
/// which a message is sent, a scroll is complete, or an
/// inscription is legible. `inscription` builds via
/// `inscribe(amount)` and accumulates passively at `scribe_rate`
/// per second in `tick(dt)` or is erased via `erase(amount)`.
///
/// Models inscription-fill levels, scroll-saturation bars,
/// record-accumulation trackers, glyph-build gauges, text-
/// completion fill levels, rune-saturation indicators, tome-
/// accumulation bars, document meters, codex-completion fill
/// levels, or any mechanic where a character, scribe, or
/// entity slowly accumulates the written content, runes,
/// glyphs, or inscriptions required to complete a scroll,
/// send a message, trigger an inscription-based effect, or
/// fill a codex to capacity.
///
/// `inscribe(amount)` adds inscription; fires `just_written`
/// when first reaching `max_inscription`. No-op when disabled.
///
/// `erase(amount)` reduces inscription immediately; fires
/// `just_blank` when reaching 0. No-op when disabled or
/// already blank.
///
/// `tick(dt)` clears both flags, then increases inscription
/// by `scribe_rate * dt` (capped at `max_inscription`). Fires
/// `just_written` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_written()` returns `inscription >= max_inscription && enabled`.
///
/// `is_blank()` returns `inscription == 0.0` (not gated by
/// `enabled`).
///
/// `inscription_fraction()` returns
/// `(inscription / max_inscription).clamp(0, 1)`.
///
/// `effective_script(scale)` returns `scale * inscription_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — scribes at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Write {
    pub inscription: f32,
    pub max_inscription: f32,
    pub scribe_rate: f32,
    pub just_written: bool,
    pub just_blank: bool,
    pub enabled: bool,
}

impl Write {
    pub fn new(max_inscription: f32, scribe_rate: f32) -> Self {
        Self {
            inscription: 0.0,
            max_inscription: max_inscription.max(0.1),
            scribe_rate: scribe_rate.max(0.0),
            just_written: false,
            just_blank: false,
            enabled: true,
        }
    }

    /// Add inscription; fires `just_written` when first reaching max.
    /// No-op when disabled.
    pub fn inscribe(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.inscription < self.max_inscription;
        self.inscription = (self.inscription + amount).min(self.max_inscription);
        if was_below && self.inscription >= self.max_inscription {
            self.just_written = true;
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
        self.just_written = false;
        self.just_blank = false;
        if self.enabled && self.scribe_rate > 0.0 && self.inscription < self.max_inscription {
            let was_below = self.inscription < self.max_inscription;
            self.inscription = (self.inscription + self.scribe_rate * dt).min(self.max_inscription);
            if was_below && self.inscription >= self.max_inscription {
                self.just_written = true;
            }
        }
    }

    /// `true` when inscription is at maximum and component is enabled.
    pub fn is_written(&self) -> bool {
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

impl Default for Write {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Write {
        Write::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_blank() {
        let w = w();
        assert_eq!(w.inscription, 0.0);
        assert!(w.is_blank());
        assert!(!w.is_written());
    }

    #[test]
    fn new_clamps_max_inscription() {
        let w = Write::new(-5.0, 1.5);
        assert!((w.max_inscription - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_scribe_rate() {
        let w = Write::new(100.0, -1.5);
        assert_eq!(w.scribe_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Write::default();
        assert!((w.max_inscription - 100.0).abs() < 1e-5);
        assert!((w.scribe_rate - 1.5).abs() < 1e-5);
    }

    // --- inscribe ---

    #[test]
    fn inscribe_adds_inscription() {
        let mut w = w();
        w.inscribe(40.0);
        assert!((w.inscription - 40.0).abs() < 1e-3);
    }

    #[test]
    fn inscribe_clamps_at_max() {
        let mut w = w();
        w.inscribe(200.0);
        assert!((w.inscription - 100.0).abs() < 1e-3);
    }

    #[test]
    fn inscribe_fires_just_written_at_max() {
        let mut w = w();
        w.inscribe(100.0);
        assert!(w.just_written);
        assert!(w.is_written());
    }

    #[test]
    fn inscribe_no_just_written_when_already_at_max() {
        let mut w = w();
        w.inscription = 100.0;
        w.inscribe(10.0);
        assert!(!w.just_written);
    }

    #[test]
    fn inscribe_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.inscribe(50.0);
        assert_eq!(w.inscription, 0.0);
    }

    #[test]
    fn inscribe_no_op_when_amount_zero() {
        let mut w = w();
        w.inscribe(0.0);
        assert_eq!(w.inscription, 0.0);
    }

    // --- erase ---

    #[test]
    fn erase_reduces_inscription() {
        let mut w = w();
        w.inscription = 60.0;
        w.erase(20.0);
        assert!((w.inscription - 40.0).abs() < 1e-3);
    }

    #[test]
    fn erase_clamps_at_zero() {
        let mut w = w();
        w.inscription = 30.0;
        w.erase(200.0);
        assert_eq!(w.inscription, 0.0);
    }

    #[test]
    fn erase_fires_just_blank_at_zero() {
        let mut w = w();
        w.inscription = 30.0;
        w.erase(30.0);
        assert!(w.just_blank);
    }

    #[test]
    fn erase_no_op_when_already_blank() {
        let mut w = w();
        w.erase(10.0);
        assert!(!w.just_blank);
    }

    #[test]
    fn erase_no_op_when_disabled() {
        let mut w = w();
        w.inscription = 50.0;
        w.enabled = false;
        w.erase(50.0);
        assert!((w.inscription - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_inscription() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.inscription - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_written_on_inscription_to_max() {
        let mut w = Write::new(100.0, 200.0);
        w.inscription = 95.0;
        w.tick(1.0);
        assert!(w.just_written);
        assert!(w.is_written());
    }

    #[test]
    fn tick_no_build_when_already_written() {
        let mut w = w();
        w.inscription = 100.0;
        w.tick(1.0);
        assert!(!w.just_written);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Write::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.inscription, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.inscription, 0.0);
    }

    #[test]
    fn tick_clears_just_written() {
        let mut w = Write::new(100.0, 200.0);
        w.inscription = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_written);
    }

    #[test]
    fn tick_clears_just_blank() {
        let mut w = w();
        w.inscription = 10.0;
        w.erase(10.0);
        w.tick(0.016);
        assert!(!w.just_blank);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.inscription - 9.0).abs() < 1e-3);
    }

    // --- is_written / is_blank ---

    #[test]
    fn is_written_false_when_disabled() {
        let mut w = w();
        w.inscription = 100.0;
        w.enabled = false;
        assert!(!w.is_written());
    }

    #[test]
    fn is_blank_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_blank());
    }

    // --- inscription_fraction / effective_script ---

    #[test]
    fn inscription_fraction_zero_when_blank() {
        assert_eq!(w().inscription_fraction(), 0.0);
    }

    #[test]
    fn inscription_fraction_half_at_midpoint() {
        let mut w = w();
        w.inscription = 50.0;
        assert!((w.inscription_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_script_zero_when_blank() {
        assert_eq!(w().effective_script(100.0), 0.0);
    }

    #[test]
    fn effective_script_scales_with_inscription() {
        let mut w = w();
        w.inscription = 75.0;
        assert!((w.effective_script(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_script_zero_when_disabled() {
        let mut w = w();
        w.inscription = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_script(100.0), 0.0);
    }
}

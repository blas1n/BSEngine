use bevy_ecs::prelude::Component;

/// Metrical-composition accumulation tracker named after verse, the
/// noun that carries within a single syllable the entire history of
/// the human impulse to organise language into pattern. The word
/// derives from the Latin versus, meaning a line of writing or
/// ploughing — from vertere, to turn — because the act of writing
/// a verse was understood through the agricultural metaphor of the
/// ploughman reaching the end of a furrow and turning the ox to
/// begin the next: the line is finite, it ends at a boundary, and
/// then the work resumes from the left margin with the same
/// deliberate rhythm. In English, verse names simultaneously the
/// individual line of a poem, the stanza that groups those lines,
/// and poetry as a mode of discourse distinguished from prose by its
/// organisation around pattern — of syllable stress, of syllable
/// count, of rhyme, of breath, of meaning distributed across
/// measured units. The major traditions of English verse — Old
/// English alliterative, Middle English syllabic, Renaissance
/// iambic, Romantic lyric, Modernist free — each represent a
/// different negotiation with the question of what constraint can
/// do for language: whether the resistance of the pattern generates
/// meaning that unresisted syntax could not discover, whether the
/// pressure of the form compresses ordinary words into something
/// that retains its shape after the reading is done. In sacred texts
/// the verse is the unit of scriptural division: a discrete semantic
/// and syntactic block that can be memorised, cited, debated, and
/// built into argument without losing its boundary with adjacent
/// blocks. In game narrative, a verse mechanic models the slow
/// accumulation of lyric intensity — word by word, measure by
/// measure — until the line is complete and can be delivered, sung,
/// or spoken with the full weight of its form. `meter` builds via
/// `compose(amount)` and accumulates passively at `cadence_rate` per
/// second in `tick(dt)` or falls silent via `silence(amount)`.
///
/// Models metrical-composition fill levels, poetic-intensity
/// saturation bars, lyric-tension accumulators, stanza-completion
/// gauges, scriptural-citation fill levels, rhythmic-saturation
/// indicators, cadence-build accumulation bars, narrative-verse
/// meters, hymn-completion fill levels, or any mechanic where a
/// character, choir, oracle, or storytelling system slowly builds
/// the metrical and tonal weight of a verse until the line is full
/// and the poem can be delivered — the accumulated pattern finally
/// given voice — or until silence intervenes and the composition
/// collapses back to its first uncertain syllable.
///
/// `compose(amount)` adds meter; fires `just_composed` when first
/// reaching `max_meter`. No-op when disabled.
///
/// `silence(amount)` reduces meter immediately; fires `just_silenced`
/// when reaching 0. No-op when disabled or already silent.
///
/// `tick(dt)` clears both flags, then increases meter by
/// `cadence_rate * dt` (capped at `max_meter`). Fires `just_composed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_composed()` returns `meter >= max_meter && enabled`.
///
/// `is_silenced()` returns `meter == 0.0` (not gated by `enabled`).
///
/// `meter_fraction()` returns `(meter / max_meter).clamp(0, 1)`.
///
/// `effective_verse(scale)` returns `scale * meter_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — composes at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Verse {
    pub meter: f32,
    pub max_meter: f32,
    pub cadence_rate: f32,
    pub just_composed: bool,
    pub just_silenced: bool,
    pub enabled: bool,
}

impl Verse {
    pub fn new(max_meter: f32, cadence_rate: f32) -> Self {
        Self {
            meter: 0.0,
            max_meter: max_meter.max(0.1),
            cadence_rate: cadence_rate.max(0.0),
            just_composed: false,
            just_silenced: false,
            enabled: true,
        }
    }

    /// Add meter; fires `just_composed` when first reaching max.
    /// No-op when disabled.
    pub fn compose(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.meter < self.max_meter;
        self.meter = (self.meter + amount).min(self.max_meter);
        if was_below && self.meter >= self.max_meter {
            self.just_composed = true;
        }
    }

    /// Reduce meter; fires `just_silenced` when reaching 0.
    /// No-op when disabled or already silent.
    pub fn silence(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.meter <= 0.0 {
            return;
        }
        self.meter = (self.meter - amount).max(0.0);
        if self.meter <= 0.0 {
            self.just_silenced = true;
        }
    }

    /// Clear flags, then increase meter by `cadence_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_composed = false;
        self.just_silenced = false;
        if self.enabled && self.cadence_rate > 0.0 && self.meter < self.max_meter {
            let was_below = self.meter < self.max_meter;
            self.meter = (self.meter + self.cadence_rate * dt).min(self.max_meter);
            if was_below && self.meter >= self.max_meter {
                self.just_composed = true;
            }
        }
    }

    /// `true` when meter is at maximum and component is enabled.
    pub fn is_composed(&self) -> bool {
        self.meter >= self.max_meter && self.enabled
    }

    /// `true` when meter is 0 (not gated by `enabled`).
    pub fn is_silenced(&self) -> bool {
        self.meter == 0.0
    }

    /// Fraction of maximum meter [0.0, 1.0].
    pub fn meter_fraction(&self) -> f32 {
        (self.meter / self.max_meter).clamp(0.0, 1.0)
    }

    /// Returns `scale * meter_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_verse(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.meter_fraction()
    }
}

impl Default for Verse {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Verse {
        Verse::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_silenced() {
        let v = v();
        assert_eq!(v.meter, 0.0);
        assert!(v.is_silenced());
        assert!(!v.is_composed());
    }

    #[test]
    fn new_clamps_max_meter() {
        let v = Verse::new(-5.0, 1.5);
        assert!((v.max_meter - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_cadence_rate() {
        let v = Verse::new(100.0, -1.5);
        assert_eq!(v.cadence_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Verse::default();
        assert!((v.max_meter - 100.0).abs() < 1e-5);
        assert!((v.cadence_rate - 1.5).abs() < 1e-5);
    }

    // --- compose ---

    #[test]
    fn compose_adds_meter() {
        let mut v = v();
        v.compose(40.0);
        assert!((v.meter - 40.0).abs() < 1e-3);
    }

    #[test]
    fn compose_clamps_at_max() {
        let mut v = v();
        v.compose(200.0);
        assert!((v.meter - 100.0).abs() < 1e-3);
    }

    #[test]
    fn compose_fires_just_composed_at_max() {
        let mut v = v();
        v.compose(100.0);
        assert!(v.just_composed);
        assert!(v.is_composed());
    }

    #[test]
    fn compose_no_just_composed_when_already_at_max() {
        let mut v = v();
        v.meter = 100.0;
        v.compose(10.0);
        assert!(!v.just_composed);
    }

    #[test]
    fn compose_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.compose(50.0);
        assert_eq!(v.meter, 0.0);
    }

    #[test]
    fn compose_no_op_when_amount_zero() {
        let mut v = v();
        v.compose(0.0);
        assert_eq!(v.meter, 0.0);
    }

    // --- silence ---

    #[test]
    fn silence_reduces_meter() {
        let mut v = v();
        v.meter = 60.0;
        v.silence(20.0);
        assert!((v.meter - 40.0).abs() < 1e-3);
    }

    #[test]
    fn silence_clamps_at_zero() {
        let mut v = v();
        v.meter = 30.0;
        v.silence(200.0);
        assert_eq!(v.meter, 0.0);
    }

    #[test]
    fn silence_fires_just_silenced_at_zero() {
        let mut v = v();
        v.meter = 30.0;
        v.silence(30.0);
        assert!(v.just_silenced);
    }

    #[test]
    fn silence_no_op_when_already_silenced() {
        let mut v = v();
        v.silence(10.0);
        assert!(!v.just_silenced);
    }

    #[test]
    fn silence_no_op_when_disabled() {
        let mut v = v();
        v.meter = 50.0;
        v.enabled = false;
        v.silence(50.0);
        assert!((v.meter - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_meter() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.meter - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_composed_on_meter_to_max() {
        let mut v = Verse::new(100.0, 200.0);
        v.meter = 95.0;
        v.tick(1.0);
        assert!(v.just_composed);
        assert!(v.is_composed());
    }

    #[test]
    fn tick_no_build_when_already_composed() {
        let mut v = v();
        v.meter = 100.0;
        v.tick(1.0);
        assert!(!v.just_composed);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Verse::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.meter, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.meter, 0.0);
    }

    #[test]
    fn tick_clears_just_composed() {
        let mut v = Verse::new(100.0, 200.0);
        v.meter = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_composed);
    }

    #[test]
    fn tick_clears_just_silenced() {
        let mut v = v();
        v.meter = 10.0;
        v.silence(10.0);
        v.tick(0.016);
        assert!(!v.just_silenced);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.meter - 9.0).abs() < 1e-3);
    }

    // --- is_composed / is_silenced ---

    #[test]
    fn is_composed_false_when_disabled() {
        let mut v = v();
        v.meter = 100.0;
        v.enabled = false;
        assert!(!v.is_composed());
    }

    #[test]
    fn is_silenced_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_silenced());
    }

    // --- meter_fraction / effective_verse ---

    #[test]
    fn meter_fraction_zero_when_silenced() {
        assert_eq!(v().meter_fraction(), 0.0);
    }

    #[test]
    fn meter_fraction_half_at_midpoint() {
        let mut v = v();
        v.meter = 50.0;
        assert!((v.meter_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_verse_zero_when_silenced() {
        assert_eq!(v().effective_verse(100.0), 0.0);
    }

    #[test]
    fn effective_verse_scales_with_meter() {
        let mut v = v();
        v.meter = 75.0;
        assert!((v.effective_verse(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_verse_zero_when_disabled() {
        let mut v = v();
        v.meter = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_verse(100.0), 0.0);
    }
}

use bevy_ecs::prelude::Component;

/// Animal-distribution survey tracker named after zoography, the
/// branch of zoology concerned with describing animals and mapping
/// their geographical ranges — where each species lives, how far
/// its population extends, what barriers divide one subspecies from
/// another, and how range edges shift as climate and habitat
/// change. A zoographer's output is a range map: a bounded region
/// coloured by presence, absence, and abundance, assembled from
/// thousands of point records accumulated over years of fieldwork.
/// `survey` builds via `chart(amount)` and accumulates passively
/// at `map_rate` per second in `tick(dt)` or is reduced via
/// `cull(amount)`.
///
/// Models wildlife-range mapping completion bars, species-
/// distribution survey fill levels, geographical-range
/// accumulation meters, ecological-atlas progress trackers,
/// biodiversity-hotspot coverage gauges, range-shift monitoring
/// saturation bars, fauna-survey completeness indicators,
/// biogeographic-province fill levels, zoogeographic-region
/// mapping progress bars, or any mechanic where slow accumulation
/// of observation records gradually fills a range map until the
/// complete distribution of a species is documented — right up
/// until habitat destruction or a disease outbreak erases a
/// population from the map and the survey must begin again.
///
/// `chart(amount)` adds survey; fires `just_mapped` when first
/// reaching `max_survey`. No-op when disabled.
///
/// `cull(amount)` reduces survey immediately; fires `just_void`
/// when reaching 0. No-op when disabled or already void.
///
/// `tick(dt)` clears both flags, then increases survey by
/// `map_rate * dt` (capped at `max_survey`). Fires `just_mapped`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_mapped()` returns `survey >= max_survey && enabled`.
///
/// `is_void()` returns `survey == 0.0` (not gated by `enabled`).
///
/// `survey_fraction()` returns `(survey / max_survey).clamp(0, 1)`.
///
/// `effective_range(scale)` returns `scale * survey_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — maps at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoography {
    pub survey: f32,
    pub max_survey: f32,
    pub map_rate: f32,
    pub just_mapped: bool,
    pub just_void: bool,
    pub enabled: bool,
}

impl Zoography {
    pub fn new(max_survey: f32, map_rate: f32) -> Self {
        Self {
            survey: 0.0,
            max_survey: max_survey.max(0.1),
            map_rate: map_rate.max(0.0),
            just_mapped: false,
            just_void: false,
            enabled: true,
        }
    }

    /// Add survey; fires `just_mapped` when first reaching max.
    /// No-op when disabled.
    pub fn chart(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.survey < self.max_survey;
        self.survey = (self.survey + amount).min(self.max_survey);
        if was_below && self.survey >= self.max_survey {
            self.just_mapped = true;
        }
    }

    /// Reduce survey; fires `just_void` when reaching 0.
    /// No-op when disabled or already void.
    pub fn cull(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.survey <= 0.0 {
            return;
        }
        self.survey = (self.survey - amount).max(0.0);
        if self.survey <= 0.0 {
            self.just_void = true;
        }
    }

    /// Clear flags, then increase survey by `map_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_mapped = false;
        self.just_void = false;
        if self.enabled && self.map_rate > 0.0 && self.survey < self.max_survey {
            let was_below = self.survey < self.max_survey;
            self.survey = (self.survey + self.map_rate * dt).min(self.max_survey);
            if was_below && self.survey >= self.max_survey {
                self.just_mapped = true;
            }
        }
    }

    /// `true` when survey is at maximum and component is enabled.
    pub fn is_mapped(&self) -> bool {
        self.survey >= self.max_survey && self.enabled
    }

    /// `true` when survey is 0 (not gated by `enabled`).
    pub fn is_void(&self) -> bool {
        self.survey == 0.0
    }

    /// Fraction of maximum survey [0.0, 1.0].
    pub fn survey_fraction(&self) -> f32 {
        (self.survey / self.max_survey).clamp(0.0, 1.0)
    }

    /// Returns `scale * survey_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_range(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.survey_fraction()
    }
}

impl Default for Zoography {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoography {
        Zoography::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_void() {
        let z = z();
        assert_eq!(z.survey, 0.0);
        assert!(z.is_void());
        assert!(!z.is_mapped());
    }

    #[test]
    fn new_clamps_max_survey() {
        let z = Zoography::new(-5.0, 1.5);
        assert!((z.max_survey - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_map_rate() {
        let z = Zoography::new(100.0, -1.5);
        assert_eq!(z.map_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoography::default();
        assert!((z.max_survey - 100.0).abs() < 1e-5);
        assert!((z.map_rate - 1.5).abs() < 1e-5);
    }

    // --- chart ---

    #[test]
    fn chart_adds_survey() {
        let mut z = z();
        z.chart(40.0);
        assert!((z.survey - 40.0).abs() < 1e-3);
    }

    #[test]
    fn chart_clamps_at_max() {
        let mut z = z();
        z.chart(200.0);
        assert!((z.survey - 100.0).abs() < 1e-3);
    }

    #[test]
    fn chart_fires_just_mapped_at_max() {
        let mut z = z();
        z.chart(100.0);
        assert!(z.just_mapped);
        assert!(z.is_mapped());
    }

    #[test]
    fn chart_no_just_mapped_when_already_at_max() {
        let mut z = z();
        z.survey = 100.0;
        z.chart(10.0);
        assert!(!z.just_mapped);
    }

    #[test]
    fn chart_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.chart(50.0);
        assert_eq!(z.survey, 0.0);
    }

    #[test]
    fn chart_no_op_when_amount_zero() {
        let mut z = z();
        z.chart(0.0);
        assert_eq!(z.survey, 0.0);
    }

    // --- cull ---

    #[test]
    fn cull_reduces_survey() {
        let mut z = z();
        z.survey = 60.0;
        z.cull(20.0);
        assert!((z.survey - 40.0).abs() < 1e-3);
    }

    #[test]
    fn cull_clamps_at_zero() {
        let mut z = z();
        z.survey = 30.0;
        z.cull(200.0);
        assert_eq!(z.survey, 0.0);
    }

    #[test]
    fn cull_fires_just_void_at_zero() {
        let mut z = z();
        z.survey = 30.0;
        z.cull(30.0);
        assert!(z.just_void);
    }

    #[test]
    fn cull_no_op_when_already_void() {
        let mut z = z();
        z.cull(10.0);
        assert!(!z.just_void);
    }

    #[test]
    fn cull_no_op_when_disabled() {
        let mut z = z();
        z.survey = 50.0;
        z.enabled = false;
        z.cull(50.0);
        assert!((z.survey - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_maps_survey() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.survey - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_mapped_on_map_to_max() {
        let mut z = Zoography::new(100.0, 200.0);
        z.survey = 95.0;
        z.tick(1.0);
        assert!(z.just_mapped);
        assert!(z.is_mapped());
    }

    #[test]
    fn tick_no_map_when_already_mapped() {
        let mut z = z();
        z.survey = 100.0;
        z.tick(1.0);
        assert!(!z.just_mapped);
    }

    #[test]
    fn tick_no_map_when_rate_zero() {
        let mut z = Zoography::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.survey, 0.0);
    }

    #[test]
    fn tick_no_map_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.survey, 0.0);
    }

    #[test]
    fn tick_clears_just_mapped() {
        let mut z = Zoography::new(100.0, 200.0);
        z.survey = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_mapped);
    }

    #[test]
    fn tick_clears_just_void() {
        let mut z = z();
        z.survey = 10.0;
        z.cull(10.0);
        z.tick(0.016);
        assert!(!z.just_void);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.survey - 9.0).abs() < 1e-3);
    }

    // --- is_mapped / is_void ---

    #[test]
    fn is_mapped_false_when_disabled() {
        let mut z = z();
        z.survey = 100.0;
        z.enabled = false;
        assert!(!z.is_mapped());
    }

    #[test]
    fn is_void_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_void());
    }

    // --- survey_fraction / effective_range ---

    #[test]
    fn survey_fraction_zero_when_void() {
        assert_eq!(z().survey_fraction(), 0.0);
    }

    #[test]
    fn survey_fraction_half_at_midpoint() {
        let mut z = z();
        z.survey = 50.0;
        assert!((z.survey_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_range_zero_when_void() {
        assert_eq!(z().effective_range(100.0), 0.0);
    }

    #[test]
    fn effective_range_scales_with_survey() {
        let mut z = z();
        z.survey = 75.0;
        assert!((z.effective_range(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_range_zero_when_disabled() {
        let mut z = z();
        z.survey = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_range(100.0), 0.0);
    }
}

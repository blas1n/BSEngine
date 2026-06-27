use bevy_ecs::prelude::Component;

/// Self-publishing creativity tracker. `content` builds via `write(amount)`
/// and fills passively at `draft_rate` per second in `tick(dt)` or is
/// scrapped immediately via `scrap(amount)`.
///
/// Models DIY-magazine creative fill meters, underground-press output bars,
/// fanzine content accumulators, photocopied-publication readiness gauges,
/// indie-zine page-count trackers, artist-collective output fill levels,
/// cut-and-paste layout progress indicators, risograph print-run charge
/// bars, letter-press composition meters, or any mechanic where raw creative
/// energy is steadily channelled into stapled pages until the print run is
/// ready to scatter through community spaces.
///
/// `write(amount)` adds content; fires `just_published` when first reaching
/// `max_content`. No-op when disabled.
///
/// `scrap(amount)` reduces content immediately; fires `just_blank` when
/// reaching 0. No-op when disabled or already blank.
///
/// `tick(dt)` clears both flags, then increases content by
/// `draft_rate * dt` (capped at `max_content`). Fires `just_published`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_published()` returns `content >= max_content && enabled`.
///
/// `is_blank()` returns `content == 0.0` (not gated by `enabled`).
///
/// `content_fraction()` returns `(content / max_content).clamp(0, 1)`.
///
/// `effective_reach(scale)` returns `scale * content_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — drafts at 5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zine {
    pub content: f32,
    pub max_content: f32,
    pub draft_rate: f32,
    pub just_published: bool,
    pub just_blank: bool,
    pub enabled: bool,
}

impl Zine {
    pub fn new(max_content: f32, draft_rate: f32) -> Self {
        Self {
            content: 0.0,
            max_content: max_content.max(0.1),
            draft_rate: draft_rate.max(0.0),
            just_published: false,
            just_blank: false,
            enabled: true,
        }
    }

    /// Add content; fires `just_published` when first reaching max.
    /// No-op when disabled.
    pub fn write(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.content < self.max_content;
        self.content = (self.content + amount).min(self.max_content);
        if was_below && self.content >= self.max_content {
            self.just_published = true;
        }
    }

    /// Reduce content; fires `just_blank` when reaching 0.
    /// No-op when disabled or already blank.
    pub fn scrap(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.content <= 0.0 {
            return;
        }
        self.content = (self.content - amount).max(0.0);
        if self.content <= 0.0 {
            self.just_blank = true;
        }
    }

    /// Clear flags, then increase content by `draft_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_published = false;
        self.just_blank = false;
        if self.enabled && self.draft_rate > 0.0 && self.content < self.max_content {
            let was_below = self.content < self.max_content;
            self.content = (self.content + self.draft_rate * dt).min(self.max_content);
            if was_below && self.content >= self.max_content {
                self.just_published = true;
            }
        }
    }

    /// `true` when content is at maximum and component is enabled.
    pub fn is_published(&self) -> bool {
        self.content >= self.max_content && self.enabled
    }

    /// `true` when content is 0 (not gated by `enabled`).
    pub fn is_blank(&self) -> bool {
        self.content == 0.0
    }

    /// Fraction of maximum content [0.0, 1.0].
    pub fn content_fraction(&self) -> f32 {
        (self.content / self.max_content).clamp(0.0, 1.0)
    }

    /// Returns `scale * content_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_reach(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.content_fraction()
    }
}

impl Default for Zine {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zine {
        Zine::new(100.0, 5.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_blank() {
        let z = z();
        assert_eq!(z.content, 0.0);
        assert!(z.is_blank());
        assert!(!z.is_published());
    }

    #[test]
    fn new_clamps_max_content() {
        let z = Zine::new(-5.0, 5.0);
        assert!((z.max_content - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_draft_rate() {
        let z = Zine::new(100.0, -3.0);
        assert_eq!(z.draft_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zine::default();
        assert!((z.max_content - 100.0).abs() < 1e-5);
        assert!((z.draft_rate - 5.0).abs() < 1e-5);
    }

    // --- write ---

    #[test]
    fn write_adds_content() {
        let mut z = z();
        z.write(40.0);
        assert!((z.content - 40.0).abs() < 1e-3);
    }

    #[test]
    fn write_clamps_at_max() {
        let mut z = z();
        z.write(200.0);
        assert!((z.content - 100.0).abs() < 1e-3);
    }

    #[test]
    fn write_fires_just_published_at_max() {
        let mut z = z();
        z.write(100.0);
        assert!(z.just_published);
        assert!(z.is_published());
    }

    #[test]
    fn write_no_just_published_when_already_at_max() {
        let mut z = z();
        z.content = 100.0;
        z.write(10.0);
        assert!(!z.just_published);
    }

    #[test]
    fn write_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.write(50.0);
        assert_eq!(z.content, 0.0);
    }

    #[test]
    fn write_no_op_when_amount_zero() {
        let mut z = z();
        z.write(0.0);
        assert_eq!(z.content, 0.0);
    }

    // --- scrap ---

    #[test]
    fn scrap_reduces_content() {
        let mut z = z();
        z.content = 60.0;
        z.scrap(20.0);
        assert!((z.content - 40.0).abs() < 1e-3);
    }

    #[test]
    fn scrap_clamps_at_zero() {
        let mut z = z();
        z.content = 30.0;
        z.scrap(200.0);
        assert_eq!(z.content, 0.0);
    }

    #[test]
    fn scrap_fires_just_blank_at_zero() {
        let mut z = z();
        z.content = 30.0;
        z.scrap(30.0);
        assert!(z.just_blank);
    }

    #[test]
    fn scrap_no_op_when_already_blank() {
        let mut z = z();
        z.scrap(10.0);
        assert!(!z.just_blank);
    }

    #[test]
    fn scrap_no_op_when_disabled() {
        let mut z = z();
        z.content = 50.0;
        z.enabled = false;
        z.scrap(50.0);
        assert!((z.content - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_drafts_content() {
        let mut z = z(); // rate=5
        z.tick(2.0); // 0 + 5*2 = 10
        assert!((z.content - 10.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_published_on_draft_to_max() {
        let mut z = Zine::new(100.0, 200.0);
        z.content = 95.0;
        z.tick(1.0);
        assert!(z.just_published);
        assert!(z.is_published());
    }

    #[test]
    fn tick_no_draft_when_already_published() {
        let mut z = z();
        z.content = 100.0;
        z.tick(1.0);
        assert!(!z.just_published);
    }

    #[test]
    fn tick_no_draft_when_rate_zero() {
        let mut z = Zine::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.content, 0.0);
    }

    #[test]
    fn tick_no_draft_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.content, 0.0);
    }

    #[test]
    fn tick_clears_just_published() {
        let mut z = Zine::new(100.0, 200.0);
        z.content = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_published);
    }

    #[test]
    fn tick_clears_just_blank() {
        let mut z = z();
        z.content = 10.0;
        z.scrap(10.0);
        z.tick(0.016);
        assert!(!z.just_blank);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=5
        z.tick(4.0); // 5*4 = 20
        assert!((z.content - 20.0).abs() < 1e-3);
    }

    // --- is_published / is_blank ---

    #[test]
    fn is_published_false_when_disabled() {
        let mut z = z();
        z.content = 100.0;
        z.enabled = false;
        assert!(!z.is_published());
    }

    #[test]
    fn is_blank_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_blank());
    }

    // --- content_fraction / effective_reach ---

    #[test]
    fn content_fraction_zero_when_blank() {
        assert_eq!(z().content_fraction(), 0.0);
    }

    #[test]
    fn content_fraction_half_at_midpoint() {
        let mut z = z();
        z.content = 50.0;
        assert!((z.content_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_reach_zero_when_blank() {
        assert_eq!(z().effective_reach(100.0), 0.0);
    }

    #[test]
    fn effective_reach_scales_with_content() {
        let mut z = z();
        z.content = 65.0;
        assert!((z.effective_reach(100.0) - 65.0).abs() < 1e-3);
    }

    #[test]
    fn effective_reach_zero_when_disabled() {
        let mut z = z();
        z.content = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_reach(100.0), 0.0);
    }
}

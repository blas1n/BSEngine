use bevy_ecs::prelude::Component;

/// Alert state component for AI entities that have detected a threat.
///
/// Call `trigger()` when the entity spots an intruder — it enters alert state
/// for `alert_duration` seconds. During that time, `is_alert()` returns `true`
/// and AI systems should broadcast the threat to allies within `detection_radius`
/// world units. `tick(dt)` counts down and sets `just_calmed` when the alert
/// expires naturally; `calm()` ends it immediately.
///
/// The component does not manage detection logic itself — callers decide when
/// to trigger based on their own line-of-sight or sound rules. Retriggering
/// while already alert restarts the full `alert_duration`.
///
/// Distinct from `Notice` (passive acknowledgement), `Radar` (detection sweep),
/// and `Faction` (affiliation): Alarm is the active-alert state machine —
/// it tracks how long an entity stays alarmed after a confirmed detection.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Alarm {
    /// How long the alarm stays active after triggering, in seconds.
    pub alert_duration: f32,
    pub timer: f32,
    /// World-unit radius within which this entity broadcasts the alert to allies.
    pub detection_radius: f32,
    pub just_triggered: bool,
    pub just_calmed: bool,
    pub enabled: bool,
}

impl Alarm {
    pub fn new(alert_duration: f32, detection_radius: f32) -> Self {
        Self {
            alert_duration: alert_duration.max(0.0),
            timer: 0.0,
            detection_radius: detection_radius.max(0.0),
            just_triggered: false,
            just_calmed: false,
            enabled: true,
        }
    }

    /// Trigger (or retrigger) the alarm. Restarts the full `alert_duration`
    /// timer even if already alert. No-op when disabled.
    pub fn trigger(&mut self) {
        if !self.enabled {
            return;
        }
        self.timer = self.alert_duration;
        self.just_triggered = true;
    }

    /// End the alarm immediately.
    pub fn calm(&mut self) {
        if self.is_alert() {
            self.timer = 0.0;
            self.just_calmed = true;
        }
    }

    /// Advance the timer; sets `just_calmed` when the alert expires naturally.
    pub fn tick(&mut self, dt: f32) {
        self.just_triggered = false;
        self.just_calmed = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.just_calmed = true;
            }
        }
    }

    pub fn is_alert(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the alert duration remaining [1.0 = just triggered, 0.0 = calm].
    pub fn remaining_fraction(&self) -> f32 {
        if self.alert_duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.alert_duration).clamp(0.0, 1.0)
    }
}

impl Default for Alarm {
    fn default() -> Self {
        Self::new(10.0, 15.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_activates_alarm() {
        let mut a = Alarm::new(10.0, 15.0);
        a.trigger();
        assert!(a.is_alert());
        assert!(a.just_triggered);
    }

    #[test]
    fn trigger_restarts_timer_when_already_alert() {
        let mut a = Alarm::new(10.0, 15.0);
        a.trigger();
        a.tick(5.0);
        a.trigger();
        assert!((a.timer - 10.0).abs() < 1e-4);
    }

    #[test]
    fn calm_ends_alert_early() {
        let mut a = Alarm::new(10.0, 15.0);
        a.trigger();
        a.calm();
        assert!(!a.is_alert());
        assert!(a.just_calmed);
    }

    #[test]
    fn tick_expires_alarm() {
        let mut a = Alarm::new(5.0, 15.0);
        a.trigger();
        a.tick(5.1);
        assert!(!a.is_alert());
        assert!(a.just_calmed);
    }

    #[test]
    fn tick_clears_just_triggered() {
        let mut a = Alarm::new(10.0, 15.0);
        a.trigger();
        a.tick(0.016);
        assert!(!a.just_triggered);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut a = Alarm::new(10.0, 15.0);
        a.trigger();
        a.tick(5.0);
        assert!((a.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_when_calm() {
        let a = Alarm::new(10.0, 15.0);
        assert!((a.remaining_fraction() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_trigger_no_op() {
        let mut a = Alarm::new(10.0, 15.0);
        a.enabled = false;
        a.trigger();
        assert!(!a.is_alert());
    }

    #[test]
    fn negative_params_clamped_to_zero() {
        let a = Alarm::new(-5.0, -10.0);
        assert!((a.alert_duration - 0.0).abs() < 1e-5);
        assert!((a.detection_radius - 0.0).abs() < 1e-5);
    }

    #[test]
    fn calm_no_op_when_already_calm() {
        let mut a = Alarm::new(10.0, 15.0);
        a.calm(); // should not set just_calmed when not active
        assert!(!a.just_calmed);
    }
}

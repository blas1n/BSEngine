use bevy_ecs::prelude::Component;

/// Shape/form transformation component for entities that can shift between
/// distinct states (e.g. werewolf human↔wolf, mech folded↔deployed,
/// slime small↔large).
///
/// Form indices are game-defined; this component only drives timing. The
/// animation and stats systems query `form` for the resolved shape and
/// `morph_fraction` for blending. `just_started` / `just_finished` provide
/// hooks for VFX and stat swaps.
///
/// Call `begin(target)` to start a transition; `tick(dt)` advances it.
/// `cancel()` rolls back to the original form immediately. `instant(form)`
/// skips the transition animation entirely.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Morph {
    /// Current resolved form (the one the entity occupies between transforms).
    pub form: u32,
    /// Target form; equals `form` when no transform is in progress.
    pub target_form: u32,
    /// Seconds for a full transformation between any two forms.
    pub morph_time: f32,
    /// Elapsed time in the current transformation [0.0, `morph_time`].
    pub morph_timer: f32,
    /// True while a transformation is in progress.
    pub is_morphing: bool,
    /// True on the first frame a transformation begins.
    pub just_started: bool,
    /// True on the first frame a transformation completes.
    pub just_finished: bool,
    pub enabled: bool,
}

impl Morph {
    pub fn new(initial_form: u32, morph_time: f32) -> Self {
        Self {
            form: initial_form,
            target_form: initial_form,
            morph_time: morph_time.max(0.0),
            morph_timer: 0.0,
            is_morphing: false,
            just_started: false,
            just_finished: false,
            enabled: true,
        }
    }

    /// Start transforming towards `target`. No-op if already targeting that form.
    /// If `morph_time` is 0, the transform resolves instantly.
    pub fn begin(&mut self, target: u32) {
        if !self.enabled || target == self.form {
            return;
        }
        if self.morph_time <= 0.0 {
            self.form = target;
            self.target_form = target;
            self.just_finished = true;
            return;
        }
        self.target_form = target;
        self.morph_timer = 0.0;
        self.is_morphing = true;
        self.just_started = true;
    }

    /// Advance the transformation by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.just_started = false;
        self.just_finished = false;

        if !self.is_morphing {
            return;
        }

        self.morph_timer += dt;
        if self.morph_timer >= self.morph_time {
            self.morph_timer = self.morph_time;
            self.form = self.target_form;
            self.is_morphing = false;
            self.just_finished = true;
        }
    }

    /// Cancel an in-progress transformation and snap back to the original form.
    pub fn cancel(&mut self) {
        if self.is_morphing {
            self.target_form = self.form;
            self.morph_timer = 0.0;
            self.is_morphing = false;
        }
    }

    /// Switch forms instantly without animation.
    pub fn instant(&mut self, form: u32) {
        self.form = form;
        self.target_form = form;
        self.morph_timer = 0.0;
        self.is_morphing = false;
        self.just_finished = true;
    }

    /// Blend fraction [0.0, 1.0] from `form` towards `target_form`.
    /// 0.0 = fully in `form`; 1.0 = fully in `target_form`.
    pub fn morph_fraction(&self) -> f32 {
        if !self.is_morphing || self.morph_time <= 0.0 {
            return if self.form == self.target_form {
                0.0
            } else {
                1.0
            };
        }
        (self.morph_timer / self.morph_time).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_starts_morph() {
        let mut m = Morph::new(0, 2.0);
        m.begin(1);
        assert!(m.is_morphing);
        assert_eq!(m.target_form, 1);
        assert!(m.just_started);
    }

    #[test]
    fn same_form_noop() {
        let mut m = Morph::new(0, 2.0);
        m.begin(0);
        assert!(!m.is_morphing);
    }

    #[test]
    fn tick_completes_morph() {
        let mut m = Morph::new(0, 2.0);
        m.begin(1);
        m.tick(2.1);
        assert!(!m.is_morphing);
        assert_eq!(m.form, 1);
        assert!(m.just_finished);
    }

    #[test]
    fn tick_clears_just_started() {
        let mut m = Morph::new(0, 2.0);
        m.begin(1);
        m.tick(0.5);
        assert!(!m.just_started);
    }

    #[test]
    fn cancel_aborts_morph() {
        let mut m = Morph::new(0, 2.0);
        m.begin(1);
        m.tick(0.5);
        m.cancel();
        assert!(!m.is_morphing);
        assert_eq!(m.form, 0);
        assert_eq!(m.target_form, 0);
    }

    #[test]
    fn instant_changes_form() {
        let mut m = Morph::new(0, 2.0);
        m.instant(3);
        assert_eq!(m.form, 3);
        assert!(!m.is_morphing);
        assert!(m.just_finished);
    }

    #[test]
    fn morph_fraction_mid_transition() {
        let mut m = Morph::new(0, 4.0);
        m.begin(1);
        m.tick(1.0);
        assert!((m.morph_fraction() - 0.25).abs() < 1e-5);
    }

    #[test]
    fn zero_morph_time_resolves_instantly() {
        let mut m = Morph::new(0, 0.0);
        m.begin(1);
        assert_eq!(m.form, 1);
        assert!(!m.is_morphing);
        assert!(m.just_finished);
    }

    #[test]
    fn disabled_ignores_begin() {
        let mut m = Morph::new(0, 2.0);
        m.enabled = false;
        m.begin(1);
        assert!(!m.is_morphing);
    }
}

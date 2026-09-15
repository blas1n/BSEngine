use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// One clip's place along a [`BlendTree1D`]'s parameter axis.
#[derive(Debug, Clone, PartialEq, Default, Reflect, serde::Serialize, serde::Deserialize)]
pub struct BlendClip {
    /// Name/identifier of the animation clip.
    pub clip: String,
    /// The parameter value at which this clip plays alone.
    pub threshold: f32,
}

/// Blends several clips along one continuous parameter — a 1D blend space.
///
/// At a parameter value between two thresholds the two neighbouring clips play
/// together, weighted by how close the value sits to each. Below the lowest
/// threshold or above the highest, the end clip plays alone.
///
/// This is what a state uses instead of a single clip when the motion is a
/// continuum rather than a set of poses: walking does not become running at a
/// threshold, it becomes running gradually, and a crossfade between the two
/// only looks right for the instant it is halfway.
#[derive(Debug, Clone, PartialEq, Default, Reflect, serde::Serialize, serde::Deserialize)]
pub struct BlendTree1D {
    /// Name of the float parameter driving the blend, read from
    /// [`AnimationStateMachine::params_float`].
    pub param: String,
    /// The clips along the axis. Sorted by threshold when sampled, so scene
    /// files do not have to be written in order.
    pub clips: Vec<BlendClip>,
}

impl BlendTree1D {
    /// The clips contributing at `value`, each with its weight.
    ///
    /// Returns one entry when the value sits on or past an end of the axis, and
    /// two while between thresholds. An empty tree returns nothing, which the
    /// caller treats as "play the state's own clip".
    pub fn sample(&self, value: f32) -> Vec<(String, f32)> {
        let mut sorted: Vec<&BlendClip> = self.clips.iter().collect();
        sorted.sort_by(|a, b| {
            a.threshold
                .partial_cmp(&b.threshold)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let Some(&first) = sorted.first() else {
            return Vec::new();
        };
        if value <= first.threshold {
            return vec![(first.clip.clone(), 1.0)];
        }
        let last = sorted[sorted.len() - 1];
        if value >= last.threshold {
            return vec![(last.clip.clone(), 1.0)];
        }
        for pair in sorted.windows(2) {
            let (lo, hi) = (pair[0], pair[1]);
            if value >= lo.threshold && value <= hi.threshold {
                let span = hi.threshold - lo.threshold;
                // Equal thresholds would divide by zero; treat the pair as a
                // step and let the upper clip take it.
                if span <= f32::EPSILON {
                    return vec![(hi.clip.clone(), 1.0)];
                }
                let t = (value - lo.threshold) / span;
                return vec![(lo.clip.clone(), 1.0 - t), (hi.clip.clone(), t)];
            }
        }
        vec![(last.clip.clone(), 1.0)]
    }
}

/// One clip placed at a point in a 2D blend space.
#[derive(Debug, Clone, Reflect, serde::Serialize, serde::Deserialize)]
pub struct BlendClip2D {
    /// Name/identifier of the animation clip.
    pub clip: String,
    /// Position on the first parameter's axis.
    pub x: f32,
    /// Position on the second parameter's axis.
    pub y: f32,
}

/// Blends clips placed anywhere on a plane — a 2D blend space.
///
/// The usual shape is locomotion: forward speed on one axis, strafe or turn on
/// the other, with a clip at each combination worth authoring.
///
/// # Why triangles
///
/// The samples are triangulated, and a parameter falling inside a triangle
/// blends exactly that triangle's three clips by barycentric weight. Unreal's
/// Blend Space and Godot's `AnimationNodeBlendSpace2D` both work this way;
/// Unity's Freeform Cartesian mode instead derives weights from pairwise
/// projections without triangulating. Two of the three agree, so this follows
/// them.
///
/// The practical difference is what a point is allowed to influence. Under
/// triangulation a clip contributes only where its own triangles reach, so a
/// sprint clip placed far out cannot bleed a little weight into a standing
/// idle. Gradient bands let every sample contribute everywhere, which is more
/// forgiving of sparse authoring and less predictable to author against.
#[derive(Debug, Clone, Reflect, serde::Serialize, serde::Deserialize)]
pub struct BlendTree2D {
    /// Name of the float parameter driving the first axis.
    pub param_x: String,
    /// Name of the float parameter driving the second axis.
    pub param_y: String,
    /// The clips, placed anywhere on the plane. Order does not matter.
    pub clips: Vec<BlendClip2D>,
}

impl BlendTree2D {
    /// The clips contributing at `(x, y)`, each with its weight.
    ///
    /// Weights always sum to 1 when any clip is returned. Degenerate authoring
    /// is handled rather than rejected, because a blend space is edited
    /// incrementally and is legitimately degenerate on the way: no clips
    /// returns nothing (the caller plays the state's own clip), one clip plays
    /// alone, and clips that are all collinear — including exactly two — blend
    /// along that line, which is a 1D blend space and the only sensible
    /// reading.
    pub fn sample(&self, x: f32, y: f32) -> Vec<(String, f32)> {
        let pts: Vec<(f32, f32)> = self.clips.iter().map(|c| (c.x, c.y)).collect();
        match self.clips.len() {
            0 => return Vec::new(),
            1 => return vec![(self.clips[0].clip.clone(), 1.0)],
            _ => {}
        }
        let tris = triangulate(&pts);
        if tris.is_empty() {
            // Collinear or coincident samples: no triangle has area, so fall
            // back to the nearest edge of the point set.
            return self.along_nearest_edge(x, y);
        }
        for &(a, b, c) in &tris {
            if let Some((wa, wb, wc)) = barycentric(pts[a], pts[b], pts[c], (x, y)) {
                if wa >= -1e-6 && wb >= -1e-6 && wc >= -1e-6 {
                    return vec![
                        (self.clips[a].clip.clone(), wa.max(0.0)),
                        (self.clips[b].clip.clone(), wb.max(0.0)),
                        (self.clips[c].clip.clone(), wc.max(0.0)),
                    ];
                }
            }
        }
        // Outside the hull. Clamping to the nearest edge keeps a parameter that
        // overshoots — a speed above the fastest authored clip — playing that
        // edge rather than snapping to one arbitrary corner.
        self.along_nearest_edge(x, y)
    }

    /// Blends the two clips whose connecting segment is nearest to `(x, y)`.
    fn along_nearest_edge(&self, x: f32, y: f32) -> Vec<(String, f32)> {
        let mut best: Option<(f32, usize, usize, f32)> = None;
        for i in 0..self.clips.len() {
            for j in (i + 1)..self.clips.len() {
                let (ax, ay) = (self.clips[i].x, self.clips[i].y);
                let (bx, by) = (self.clips[j].x, self.clips[j].y);
                let (dx, dy) = (bx - ax, by - ay);
                let len2 = dx * dx + dy * dy;
                let t = if len2 <= f32::EPSILON {
                    0.0
                } else {
                    (((x - ax) * dx + (y - ay) * dy) / len2).clamp(0.0, 1.0)
                };
                let (px, py) = (ax + dx * t, ay + dy * t);
                let d2 = (x - px) * (x - px) + (y - py) * (y - py);
                if best.is_none_or(|(bd, _, _, _)| d2 < bd) {
                    best = Some((d2, i, j, t));
                }
            }
        }
        match best {
            Some((_, i, j, t)) => vec![
                (self.clips[i].clip.clone(), 1.0 - t),
                (self.clips[j].clip.clone(), t),
            ],
            None => Vec::new(),
        }
    }
}

/// Barycentric coordinates of `p` in triangle `abc`, or `None` if degenerate.
fn barycentric(
    a: (f32, f32),
    b: (f32, f32),
    c: (f32, f32),
    p: (f32, f32),
) -> Option<(f32, f32, f32)> {
    let det = (b.1 - c.1) * (a.0 - c.0) + (c.0 - b.0) * (a.1 - c.1);
    if det.abs() <= 1e-12 {
        return None;
    }
    let wa = ((b.1 - c.1) * (p.0 - c.0) + (c.0 - b.0) * (p.1 - c.1)) / det;
    let wb = ((c.1 - a.1) * (p.0 - c.0) + (a.0 - c.0) * (p.1 - c.1)) / det;
    Some((wa, wb, 1.0 - wa - wb))
}

/// Delaunay triangulation of `pts`, as index triples.
///
/// Bowyer-Watson: start from a triangle large enough to contain everything,
/// insert each point by deleting the triangles whose circumcircle it falls
/// inside and re-filling that cavity, then drop anything still touching the
/// outer scaffold. Returns empty for fewer than three points or for a set with
/// no area, which the caller reads as "blend along a line instead".
///
/// Blend spaces hold a handful of clips, so the straightforward quadratic form
/// is the right one: it is short enough to check by eye, which a faster
/// incremental structure would not be.
fn triangulate(pts: &[(f32, f32)]) -> Vec<(usize, usize, usize)> {
    if pts.len() < 3 {
        return Vec::new();
    }
    let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
    let (mut max_x, mut max_y) = (f32::MIN, f32::MIN);
    for &(x, y) in pts {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    let dx = (max_x - min_x).max(1.0);
    let dy = (max_y - min_y).max(1.0);
    let m = dx.max(dy) * 10.0;
    let (cx, cy) = ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
    // The scaffold vertices live past the end of `pts`; every triangle still
    // referencing one at the end was never fully surrounded by real samples.
    let mut work: Vec<(f32, f32)> = pts.to_vec();
    work.push((cx - m, cy - m));
    work.push((cx + m, cy - m));
    work.push((cx, cy + m));
    let n = pts.len();
    let mut tris: Vec<(usize, usize, usize)> = vec![(n, n + 1, n + 2)];

    for i in 0..n {
        let p = work[i];
        let mut cavity: Vec<(usize, usize)> = Vec::new();
        tris.retain(|&(a, b, c)| {
            if in_circumcircle(work[a], work[b], work[c], p) {
                for e in [(a, b), (b, c), (c, a)] {
                    cavity.push(e);
                }
                false
            } else {
                true
            }
        });
        // An edge shared by two removed triangles is interior to the cavity and
        // must not be re-filled; only the boundary is.
        for k in 0..cavity.len() {
            let (a, b) = cavity[k];
            let shared = cavity
                .iter()
                .enumerate()
                .any(|(l, &(c, d))| l != k && ((c, d) == (b, a) || (c, d) == (a, b)));
            if !shared {
                tris.push((a, b, i));
            }
        }
    }
    tris.retain(|&(a, b, c)| a < n && b < n && c < n);
    tris
}

/// Whether `p` falls strictly inside the circumcircle of `abc`.
fn in_circumcircle(a: (f32, f32), b: (f32, f32), c: (f32, f32), p: (f32, f32)) -> bool {
    let ax = a.0 as f64 - p.0 as f64;
    let ay = a.1 as f64 - p.1 as f64;
    let bx = b.0 as f64 - p.0 as f64;
    let by = b.1 as f64 - p.1 as f64;
    let cx = c.0 as f64 - p.0 as f64;
    let cy = c.1 as f64 - p.1 as f64;
    // f64 throughout: the determinant multiplies four coordinates together, so
    // f32 loses the sign on nearly-cocircular points and the triangulation
    // develops holes that only show up as a clip silently dropping out.
    let det = (ax * ax + ay * ay) * (bx * cy - by * cx) - (bx * bx + by * by) * (ax * cy - ay * cx)
        + (cx * cx + cy * cy) * (ax * by - ay * bx);
    det > 0.0
}

/// A single named animation state within an [`AnimationStateMachine`], describing
/// which clip plays and how, while that state is active.
#[derive(Debug, Clone, Reflect, serde::Serialize, serde::Deserialize)]
pub struct AsmState {
    /// Name/identifier of the animation clip this state plays.
    ///
    /// Ignored when [`blend`](AsmState::blend) is set.
    pub clip: String,
    /// A blend space to play instead of the single [`clip`](AsmState::clip).
    ///
    /// Optional and defaulted so every scene written before blend trees
    /// existed keeps parsing unchanged.
    pub blend: Option<BlendTree1D>,
    /// A 2D blend space, taking precedence over [`blend`](AsmState::blend).
    ///
    /// Optional for the same reason `blend` is: a scene written before 2D
    /// blend spaces existed parses unchanged.
    pub blend2d: Option<BlendTree2D>,
    /// Whether the clip wraps back to the start after reaching `duration`.
    pub looping: bool,
    /// Playback rate multiplier (1.0 = normal speed).
    pub speed: f32,
    /// Length of the clip, in seconds.
    pub duration: f32,
    /// When true, entering this state activates the entity's `Ragdoll`, and
    /// leaving it blends the skeleton back to animation.
    ///
    /// Optional and defaulted so every scene written before ragdolls
    /// existed keeps parsing unchanged -- the same reason
    /// [`blend`](AsmState::blend) is defaulted.
    #[serde(default)]
    pub ragdoll: bool,
}

impl AsmState {
    /// Creates a state for the given clip, looping at normal speed with zero duration.
    pub fn new(clip: impl Into<String>) -> Self {
        Self {
            clip: clip.into(),
            blend: None,
            blend2d: None,
            looping: true,
            speed: 1.0,
            duration: 0.0,
            ragdoll: false,
        }
    }

    /// Makes this state play a blend space instead of its single clip.
    pub fn with_blend(mut self, blend: BlendTree1D) -> Self {
        self.blend = Some(blend);
        self
    }

    /// Makes this state play a 2D blend space, taking precedence over a 1D one.
    pub fn with_blend2d(mut self, blend: BlendTree2D) -> Self {
        self.blend2d = Some(blend);
        self
    }

    /// Sets whether the clip loops when it reaches the end.
    pub fn with_looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// Sets the playback rate multiplier.
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// Sets the clip duration, clamped to be non-negative.
    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration.max(0.0);
        self
    }
}

/// Predicate evaluated against an [`AnimationStateMachine`]'s parameters to decide
/// whether an [`AsmTransition`] should fire.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub enum TransitionCondition {
    /// Fires once when the named trigger parameter has been set, then clears it.
    Trigger(String),
    /// Fires while the named float parameter is greater than `threshold`.
    FloatGreater {
        /// Name of the float parameter to compare.
        param: String,
        /// Value the parameter must exceed.
        threshold: f32,
    },
    /// Fires while the named float parameter is less than `threshold`.
    FloatLess {
        /// Name of the float parameter to compare.
        param: String,
        /// Value the parameter must fall below.
        threshold: f32,
    },
    /// Fires while the named bool parameter is `true`.
    BoolTrue(String),
    /// Fires while the named bool parameter is `false`.
    BoolFalse(String),
    /// Fires once the current state's clip has finished playing.
    Finished,
}

/// A directed edge in the state machine graph: when `condition` holds while
/// in state `from`, playback crossfades to state `to` over `blend_duration`.
#[derive(Debug, Clone, Reflect)]
pub struct AsmTransition {
    /// Source state name, or `"*"` to match any state.
    pub from: String,
    /// Destination state name to transition into.
    pub to: String,
    /// Predicate that must hold for this transition to fire.
    pub condition: TransitionCondition,
    /// Crossfade duration, in seconds, when this transition fires.
    pub blend_duration: f32,
}

/// Component that drives an `AnimationPlayer` through a named-state graph.
///
/// Each frame the ECS system evaluates transitions in order and fires the first
/// matching one, consuming Trigger parameters in the process.  Crossfade blend
/// weight is exposed via `blend_weight` so renderers can lerp bone poses.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct AnimationStateMachine {
    /// All named states in this machine's graph.
    pub states: HashMap<String, AsmState>,
    /// Edges evaluated, in order, to decide when to change state.
    pub transitions: Vec<AsmTransition>,
    /// Name of the state currently driving playback.
    pub current_state: String,
    /// Float parameters read by `FloatGreater`/`FloatLess` conditions.
    pub params_float: HashMap<String, f32>,
    /// Bool parameters read by `BoolTrue`/`BoolFalse` conditions.
    pub params_bool: HashMap<String, bool>,
    /// Trigger parameters pending consumption by a `Trigger` condition.
    pub triggers: HashSet<String>,
    /// The state being blended *out* during a crossfade (None when not blending).
    pub blend_from: Option<String>,
    /// 0.0 = fully `blend_from`, 1.0 = fully `current_state`.
    pub blend_weight: f32,
    /// Total duration of the in-progress crossfade, in seconds.
    pub blend_duration: f32,
    /// Time elapsed since the in-progress crossfade began, in seconds.
    pub blend_elapsed: f32,
}

impl Default for AnimationStateMachine {
    fn default() -> Self {
        Self::new("")
    }
}

impl AnimationStateMachine {
    /// Creates a state machine with no states or transitions, starting in `initial_state`.
    pub fn new(initial_state: impl Into<String>) -> Self {
        Self {
            states: HashMap::new(),
            transitions: Vec::new(),
            current_state: initial_state.into(),
            params_float: HashMap::new(),
            params_bool: HashMap::new(),
            triggers: HashSet::new(),
            blend_from: None,
            blend_weight: 1.0,
            blend_duration: 0.0,
            blend_elapsed: 0.0,
        }
    }

    /// Registers a named state in the graph, replacing any existing state of the same name.
    pub fn add_state(&mut self, name: impl Into<String>, state: AsmState) -> &mut Self {
        self.states.insert(name.into(), state);
        self
    }

    /// Registers a transition edge from `from` to `to`, evaluated when `condition` holds.
    pub fn add_transition(
        &mut self,
        from: impl Into<String>,
        to: impl Into<String>,
        condition: TransitionCondition,
        blend_duration: f32,
    ) -> &mut Self {
        self.transitions.push(AsmTransition {
            from: from.into(),
            to: to.into(),
            condition,
            blend_duration: blend_duration.max(0.0),
        });
        self
    }

    /// Arms the named trigger parameter so a `Trigger` condition can consume it.
    pub fn set_trigger(&mut self, name: impl Into<String>) {
        self.triggers.insert(name.into());
    }

    /// Sets the named float parameter, read by `FloatGreater`/`FloatLess` conditions.
    pub fn set_float(&mut self, name: impl Into<String>, value: f32) {
        self.params_float.insert(name.into(), value);
    }

    /// Sets the named bool parameter, read by `BoolTrue`/`BoolFalse` conditions.
    pub fn set_bool(&mut self, name: impl Into<String>, value: bool) {
        self.params_bool.insert(name.into(), value);
    }

    /// Returns true while a crossfade between two states is in progress.
    pub fn is_blending(&self) -> bool {
        self.blend_from.is_some()
    }
}

#[cfg(test)]
mod blend2d_tests {
    use super::*;

    fn tree(points: &[(&str, f32, f32)]) -> BlendTree2D {
        BlendTree2D {
            param_x: "x".into(),
            param_y: "y".into(),
            clips: points
                .iter()
                .map(|(c, x, y)| BlendClip2D {
                    clip: (*c).into(),
                    x: *x,
                    y: *y,
                })
                .collect(),
        }
    }

    fn weight_of(got: &[(String, f32)], clip: &str) -> f32 {
        got.iter()
            .find(|(c, _)| c == clip)
            .map(|(_, w)| *w)
            .unwrap_or(0.0)
    }

    /// A right triangle with distinct legs, so swapping the axes or two
    /// vertices cannot read as correct.
    fn right_triangle() -> BlendTree2D {
        tree(&[
            ("origin", 0.0, 0.0),
            ("east", 4.0, 0.0),
            ("north", 0.0, 2.0),
        ])
    }

    #[test]
    fn a_point_at_a_sample_plays_that_clip_alone() {
        let got = right_triangle().sample(4.0, 0.0);
        assert!(
            (weight_of(&got, "east") - 1.0).abs() < 1e-4,
            "a parameter sitting exactly on a sample must be that clip alone, got {got:?}"
        );
    }

    #[test]
    fn a_point_inside_a_triangle_blends_its_three_clips_by_area() {
        // The centroid weights all three equally; any other point does not.
        let got = right_triangle().sample(4.0 / 3.0, 2.0 / 3.0);
        for c in ["origin", "east", "north"] {
            assert!(
                (weight_of(&got, c) - 1.0 / 3.0).abs() < 1e-3,
                "the centroid must weight {c} at 1/3, got {got:?}"
            );
        }
    }

    #[test]
    fn weights_always_sum_to_one() {
        let t = right_triangle();
        // Inside, on an edge, at a vertex, and far outside the hull.
        for (x, y) in [(1.0, 0.5), (2.0, 0.0), (0.0, 2.0), (99.0, -40.0)] {
            let got = t.sample(x, y);
            let sum: f32 = got.iter().map(|(_, w)| w).sum();
            assert!(
                (sum - 1.0).abs() < 1e-3,
                "weights at ({x}, {y}) sum to {sum}, not 1: {got:?}"
            );
        }
    }

    /// The property that distinguishes triangulation from a distance falloff.
    #[test]
    fn a_clip_outside_the_containing_triangle_contributes_nothing() {
        // A square of four clips triangulates into two triangles. A point well
        // inside the lower-left triangle must not be tinted by the opposite
        // corner, which a gradient-band or inverse-distance scheme would do.
        let t = tree(&[
            ("sw", 0.0, 0.0),
            ("se", 10.0, 0.0),
            ("nw", 0.0, 10.0),
            ("ne", 10.0, 10.0),
        ]);
        let got = t.sample(1.0, 1.0);
        assert_eq!(
            got.len(),
            3,
            "a point inside the hull blends one triangle: {got:?}"
        );
        let far = weight_of(&got, "ne");
        assert!(
            far < 1e-6,
            "the far corner must contribute nothing at (1, 1), got {far} in {got:?}"
        );
        assert!(
            weight_of(&got, "sw") > 0.5,
            "the nearest corner should dominate: {got:?}"
        );
    }

    #[test]
    fn a_point_outside_the_hull_clamps_to_the_nearest_edge() {
        // Overshooting a parameter -- a speed past the fastest authored clip --
        // must keep playing that edge rather than snapping to one corner.
        let got = right_triangle().sample(10.0, 0.0);
        assert!(
            (weight_of(&got, "east") - 1.0).abs() < 1e-4,
            "past the east sample along the x axis, east should play alone: {got:?}"
        );
        let got = right_triangle().sample(2.0, -5.0);
        assert!(
            weight_of(&got, "origin") > 0.0 && weight_of(&got, "east") > 0.0,
            "below the origin-east edge, both its ends should blend: {got:?}"
        );
        assert!(
            weight_of(&got, "north") < 1e-6,
            "the opposite vertex must not contribute: {got:?}"
        );
    }

    #[test]
    fn collinear_samples_blend_along_their_line() {
        // No triangle has area, which must degrade to a 1D blend rather than
        // returning nothing and silently muting the state.
        let t = tree(&[("a", 0.0, 0.0), ("b", 1.0, 1.0), ("c", 2.0, 2.0)]);
        let got = t.sample(0.5, 0.5);
        let sum: f32 = got.iter().map(|(_, w)| w).sum();
        assert!((sum - 1.0).abs() < 1e-3, "{got:?}");
        assert!(
            (weight_of(&got, "a") - 0.5).abs() < 1e-3 && (weight_of(&got, "b") - 0.5).abs() < 1e-3,
            "halfway between a and b should be an even blend of those two: {got:?}"
        );
    }

    #[test]
    fn two_samples_blend_as_a_one_dimensional_space() {
        let t = tree(&[("a", 0.0, 0.0), ("b", 10.0, 0.0)]);
        let got = t.sample(2.5, 0.0);
        assert!((weight_of(&got, "a") - 0.75).abs() < 1e-3, "{got:?}");
        assert!((weight_of(&got, "b") - 0.25).abs() < 1e-3, "{got:?}");
    }

    #[test]
    fn a_degenerate_space_does_not_panic_or_invent_a_clip() {
        assert!(tree(&[]).sample(0.0, 0.0).is_empty(), "no clips, no output");
        let one = tree(&[("only", 3.0, 4.0)]).sample(-100.0, 100.0);
        assert_eq!(one.len(), 1);
        assert!((one[0].1 - 1.0).abs() < 1e-4, "{one:?}");
        // Every sample at the same point: no area, no distinct edge.
        let same = tree(&[("a", 1.0, 1.0), ("b", 1.0, 1.0)]).sample(5.0, 5.0);
        let sum: f32 = same.iter().map(|(_, w)| w).sum();
        assert!((sum - 1.0).abs() < 1e-3, "coincident samples: {same:?}");
    }

    /// The axes must not be interchangeable.
    #[test]
    fn the_two_axes_are_not_swapped() {
        let t = right_triangle();
        let a = t.sample(3.0, 0.5);
        let b = t.sample(0.5, 3.0);
        assert!(
            weight_of(&a, "east") > weight_of(&b, "east"),
            "a point far along x must favour the east clip more than one far \
             along y does; x={a:?} y={b:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_asm_starts_in_initial_state() {
        let asm = AnimationStateMachine::new("idle");
        assert_eq!(asm.current_state, "idle");
        assert!(!asm.is_blending());
        assert_eq!(asm.blend_weight, 1.0);
    }

    #[test]
    fn add_state_stores_state() {
        let mut asm = AnimationStateMachine::new("idle");
        asm.add_state("idle", AsmState::new("idle_clip").with_duration(1.0));
        assert!(asm.states.contains_key("idle"));
        assert_eq!(asm.states["idle"].clip, "idle_clip");
    }

    #[test]
    fn set_trigger_inserts() {
        let mut asm = AnimationStateMachine::new("idle");
        asm.set_trigger("move");
        assert!(asm.triggers.contains("move"));
    }

    #[test]
    fn set_float_and_bool() {
        let mut asm = AnimationStateMachine::new("idle");
        asm.set_float("speed", 1.5);
        asm.set_bool("grounded", true);
        assert!((asm.params_float["speed"] - 1.5).abs() < 1e-6);
        assert!(asm.params_bool["grounded"]);
    }

    #[test]
    fn asm_state_builder() {
        let s = AsmState::new("run")
            .with_looping(false)
            .with_speed(2.0)
            .with_duration(0.8);
        assert_eq!(s.clip, "run");
        assert!(!s.looping);
        assert!((s.speed - 2.0).abs() < 1e-6);
        assert!((s.duration - 0.8).abs() < 1e-6);
    }

    #[test]
    fn transition_condition_eq() {
        assert_eq!(TransitionCondition::Finished, TransitionCondition::Finished);
        assert_ne!(
            TransitionCondition::Trigger("a".into()),
            TransitionCondition::Trigger("b".into())
        );
    }

    #[test]
    fn an_asm_state_without_a_ragdoll_field_still_parses() {
        // THE test for this task. Every scene with an AnimationStateMachine
        // predates this field, so without `#[serde(default)]` they all stop
        // loading -- an already-shipped asset refusing to parse. Assert it
        // rather than assuming it.
        //
        // The RON omits only `ragdoll`; all other fields were present in every
        // scene written before this field was added.
        let ron = r#"(clip: "idle", blend: None, looping: true, speed: 1.0, duration: 1.0)"#;
        let s: AsmState = ron::from_str(ron).expect("must parse without `ragdoll`");
        assert!(!s.ragdoll, "the default must be false, not true");
    }

    #[test]
    fn an_asm_state_can_declare_itself_a_ragdoll_state() {
        // The field round-trips when present.
        let ron = r#"(clip: "death", blend: None, looping: false, speed: 1.0, duration: 0.0, ragdoll: true)"#;
        let s: AsmState = ron::from_str(ron).expect("must parse with `ragdoll: true`");
        assert!(s.ragdoll, "ragdoll must be true when the field is set");
        assert_eq!(s.clip, "death");
    }
}

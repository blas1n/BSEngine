//! Behaviour trees: an AI's decisions as a tree asset of composites,
//! decorators and tasks, ticked every frame against a blackboard.
//!
//! Unreal's model, the one engine of the three that ships behaviour trees
//! built in (Unity and Godot leave them to packages): a `.bt.ron` asset holds
//! the tree, a [`Blackboard`] on the entity holds what the AI knows, and the
//! tree reads and writes it. The composites and decorators are Unreal's
//! standard set -- Selector, Sequence, Parallel; blackboard conditions with
//! observer aborts, Cooldown, Time Limit, Loop, and the Inverter / Force
//! Success / Force Failure the other libraries all add -- and the built-in
//! tasks are Wait, MoveTo (through the entity's `NavMeshAgent`) and blackboard
//! writes.
//!
//! The executor here is pure: it sees the world only through [`BtContext`],
//! so everything about *what the tree decides* is testable without an app.
//! `bsengine-app`'s `BehaviorTreePlugin` loads the assets and ticks the trees.

use std::collections::HashMap;

use bevy_ecs::prelude::{Component, ReflectComponent, SystemSet};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use serde::{Deserialize, Serialize};

use crate::ReflectVec3;

/// The systems that tick behaviour trees, in `Update`. Ordered before
/// navigation by the plugin, so a `MoveTo` issued this frame is steered this
/// frame; scripts that write the blackboard order themselves before it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BehaviorTreeSystems;

/// One value on a blackboard -- Unreal's blackboard key types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect)]
pub enum BbValue {
    /// A flag.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// A number.
    Float(f32),
    /// A position or direction.
    Vec3(ReflectVec3),
    /// Text.
    Str(String),
    /// Another entity, by its `Name` -- what a `MoveTo` can chase.
    Entity(String),
}

impl Default for BbValue {
    fn default() -> Self {
        BbValue::Bool(false)
    }
}

impl BbValue {
    fn as_number(&self) -> Option<f64> {
        match self {
            BbValue::Int(i) => Some(*i as f64),
            BbValue::Float(f) => Some(f64::from(*f)),
            _ => None,
        }
    }
}

/// What an entity's AI knows: named values the tree reads (conditions,
/// `MoveTo` targets) and writes (`SetValue`), and that scripts and other
/// systems fill in -- Unreal's Blackboard component.
#[derive(Component, Debug, Clone, Default, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct Blackboard {
    /// Key -> value. A key that is absent is "not set".
    pub values: HashMap<String, BbValue>,
}

impl Blackboard {
    /// The value at `key`, if set.
    pub fn get(&self, key: &str) -> Option<&BbValue> {
        self.values.get(key)
    }

    /// Sets `key`.
    pub fn set(&mut self, key: impl Into<String>, value: BbValue) {
        self.values.insert(key.into(), value);
    }

    /// Unsets `key`.
    pub fn clear(&mut self, key: &str) {
        self.values.remove(key);
    }
}

/// Runs the behaviour tree asset at `tree` (project-relative, a `.bt.ron`)
/// against this entity's [`Blackboard`] every frame.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct BehaviorTree {
    /// The tree asset's project-relative path.
    pub tree: String,
    /// Whether the tree ticks. Turning it off aborts whatever was running
    /// (a `MoveTo` stops its agent).
    pub enabled: bool,
    /// The root's status on the last tick. Output.
    pub last_status: BtStatus,
    /// The execution state, with the path it was compiled from. Internal;
    /// rebuilt when `tree` names another asset.
    #[reflect(ignore)]
    pub runtime: Option<(String, BtRuntime)>,
}

impl Default for BehaviorTree {
    fn default() -> Self {
        Self {
            tree: String::new(),
            enabled: true,
            last_status: BtStatus::Running,
            runtime: None,
        }
    }
}

/// A node's result for one tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
pub enum BtStatus {
    /// Still working; ticked again next frame.
    #[default]
    Running,
    /// Done, and it worked.
    Success,
    /// Done, and it did not.
    Failure,
}

/// How a [`BtNode::Condition`] reacts to its condition changing while the
/// tree runs -- Unreal's "Observer aborts".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AbortMode {
    /// Checked only when the node is entered.
    #[default]
    None,
    /// While its own subtree runs, the condition turning false aborts it and
    /// the node fails.
    Self_,
    /// While a lower-priority sibling (to its right, under the same
    /// Selector) runs, the condition turning true aborts that sibling and
    /// this branch runs instead.
    LowerPriority,
    /// Both.
    Both,
}

/// A blackboard test -- Unreal's Blackboard decorator's key queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BbCompare {
    /// The key has a value.
    IsSet,
    /// The key has none.
    IsNotSet,
    /// Equal to `value` (numbers compare as numbers across Int and Float).
    Equal,
    /// Not equal to `value`.
    NotEqual,
    /// A number below `value`.
    Less,
    /// A number at most `value`.
    LessOrEqual,
    /// A number above `value`.
    Greater,
    /// A number at least `value`.
    GreaterOrEqual,
}

/// Whether a Parallel needs all its children or any one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ParallelPolicy {
    /// Succeeds once every child has; fails as soon as one fails.
    #[default]
    All,
    /// Succeeds as soon as one child does; fails once every child has.
    Any,
}

/// A behaviour tree node, as written in a `.bt.ron` asset.
///
/// ```ron
/// Selector([
///     Condition(key: "enemy", op: IsSet, abort: Both,
///         child: Sequence([MoveTo(key: "enemy", acceptance: 1.5), Wait(seconds: 0.5)])),
///     Sequence([SetValue(key: "patrolling", value: Bool(true)), Wait(seconds: 2.0)]),
/// ])
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BtNode {
    /// Runs children in order until one fails.
    Sequence(Vec<BtNode>),
    /// Runs children in order until one succeeds.
    Selector(Vec<BtNode>),
    /// Runs every child each tick; see [`ParallelPolicy`].
    Parallel {
        /// When the whole succeeds.
        #[serde(default)]
        policy: ParallelPolicy,
        /// The children.
        children: Vec<BtNode>,
    },
    /// Runs `child` only while the blackboard test holds.
    Condition {
        /// The blackboard key.
        key: String,
        /// The test.
        op: BbCompare,
        /// What to compare with (not needed for `IsSet` / `IsNotSet`).
        #[serde(default)]
        value: Option<BbValue>,
        /// Observer aborts.
        #[serde(default)]
        abort: AbortMode,
        /// The guarded subtree.
        child: Box<BtNode>,
    },
    /// Success becomes failure and failure success.
    Inverter(Box<BtNode>),
    /// Always succeeds once the child is done.
    ForceSuccess(Box<BtNode>),
    /// Always fails once the child is done.
    ForceFailure(Box<BtNode>),
    /// Runs `child` again after each success -- `count` times, or forever
    /// with none; a failure ends it. One pass per tick.
    Repeat {
        /// How many successes end it.
        #[serde(default)]
        count: Option<u32>,
        /// The repeated subtree.
        child: Box<BtNode>,
    },
    /// Fails without running `child` until `seconds` have passed since it
    /// last finished.
    Cooldown {
        /// The wait after each run.
        seconds: f32,
        /// The guarded subtree.
        child: Box<BtNode>,
    },
    /// Aborts `child` and fails if it runs longer than `seconds`.
    TimeLimit {
        /// The limit.
        seconds: f32,
        /// The limited subtree.
        child: Box<BtNode>,
    },
    /// Succeeds after `seconds`.
    Wait {
        /// How long.
        seconds: f32,
    },
    /// Sets a blackboard key; succeeds at once.
    SetValue {
        /// The key.
        key: String,
        /// The value.
        value: BbValue,
    },
    /// Unsets a blackboard key; succeeds at once.
    ClearValue {
        /// The key.
        key: String,
    },
    /// Walks the entity's `NavMeshAgent` to the position in `key` -- a
    /// `Vec3`, or an `Entity`'s current position (followed as it moves).
    /// Succeeds on arrival, fails with no path or no target.
    MoveTo {
        /// The blackboard key holding the target.
        key: String,
        /// Arrival distance, overriding the agent's own stopping distance.
        #[serde(default)]
        acceptance: Option<f32>,
    },
    /// Succeeds at once.
    Succeed,
    /// Fails at once.
    Fail,
}

/// A behaviour tree asset: the `.bt.ron` file's contents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BehaviorTreeAsset {
    /// The tree.
    pub root: BtNode,
}

/// What a `MoveTo` reports back from the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveState {
    /// Walking there.
    Moving,
    /// There.
    Arrived,
    /// Cannot get there (no path, no agent).
    Failed,
}

/// Where a `MoveTo` is going, resolved from the blackboard.
#[derive(Debug, Clone, PartialEq)]
pub enum MoveTarget {
    /// A fixed position.
    Position(glam::Vec3),
    /// An entity, by name.
    Entity(String),
}

/// The world as the executor sees it.
pub trait BtContext {
    /// Seconds since start -- what timers are measured against.
    fn now(&self) -> f64;
    /// The entity's blackboard.
    fn blackboard(&mut self) -> &mut Blackboard;
    /// Starts (or retargets) the move; called on a `MoveTo`'s first tick
    /// and on each tick after, so a moving entity target is followed.
    fn move_to(&mut self, target: &MoveTarget, acceptance: Option<f32>) -> bool;
    /// How the move started by the last `move_to` is going. Not called on
    /// the tick the move was issued: the agent has not run yet, and its
    /// state is still the last move's.
    fn move_state(&mut self) -> MoveState;
    /// Stops the move (a `MoveTo` aborted).
    fn stop_move(&mut self);
}

/// Slack on every timer comparison, in seconds. Durations are written as
/// `f32` and the clock is `f64`: 0.3 widened is 0.30000001192..., more than
/// the 0.30000000000000004 three 0.1 s frames add up to, so without it a wait
/// that lands exactly on its duration would finish a frame late. A tenth of a
/// millisecond is far below any frame.
const TIME_EPSILON: f64 = 1e-4;

/// Whether `seconds` have passed between `since` and `now`.
fn elapsed(now: f64, since: f64, seconds: f32) -> bool {
    now - since + TIME_EPSILON >= f64::from(seconds)
}

#[derive(Debug, Clone)]
enum Kind {
    Sequence,
    Selector,
    Parallel(ParallelPolicy),
    Condition {
        key: String,
        op: BbCompare,
        value: Option<BbValue>,
        abort: AbortMode,
    },
    Inverter,
    ForceSuccess,
    ForceFailure,
    Repeat(Option<u32>),
    Cooldown(f32),
    TimeLimit(f32),
    Wait(f32),
    SetValue(String, BbValue),
    ClearValue(String),
    MoveTo(String, Option<f32>),
    Succeed,
    Fail,
}

#[derive(Debug, Clone, Default)]
struct NodeState {
    running: bool,
    /// Sequence / Selector: the child being run.
    current: usize,
    /// Parallel: which children have finished, and how.
    finished: Vec<Option<BtStatus>>,
    /// Repeat: successes so far.
    count: u32,
    /// Wait / TimeLimit: when it started. Cooldown: when the child last
    /// finished.
    since: Option<f64>,
    /// MoveTo: issued this move already.
    issued: bool,
}

/// A tree compiled for ticking, with its execution state.
#[derive(Debug, Clone)]
pub struct BtRuntime {
    kinds: Vec<Kind>,
    children: Vec<Vec<usize>>,
    names: Vec<&'static str>,
    state: Vec<NodeState>,
}

impl BtRuntime {
    /// Compiles `asset`, every node idle.
    pub fn new(asset: &BehaviorTreeAsset) -> Self {
        let mut rt = BtRuntime {
            kinds: Vec::new(),
            children: Vec::new(),
            names: Vec::new(),
            state: Vec::new(),
        };
        rt.compile(&asset.root);
        rt
    }

    fn compile(&mut self, node: &BtNode) -> usize {
        let index = self.kinds.len();
        self.kinds.push(Kind::Succeed);
        self.children.push(Vec::new());
        self.names.push("");
        self.state.push(NodeState::default());
        let (kind, name, kids): (Kind, &'static str, Vec<&BtNode>) = match node {
            BtNode::Sequence(c) => (Kind::Sequence, "Sequence", c.iter().collect()),
            BtNode::Selector(c) => (Kind::Selector, "Selector", c.iter().collect()),
            BtNode::Parallel { policy, children } => (
                Kind::Parallel(*policy),
                "Parallel",
                children.iter().collect(),
            ),
            BtNode::Condition {
                key,
                op,
                value,
                abort,
                child,
            } => (
                Kind::Condition {
                    key: key.clone(),
                    op: *op,
                    value: value.clone(),
                    abort: *abort,
                },
                "Condition",
                vec![child],
            ),
            BtNode::Inverter(c) => (Kind::Inverter, "Inverter", vec![c]),
            BtNode::ForceSuccess(c) => (Kind::ForceSuccess, "ForceSuccess", vec![c]),
            BtNode::ForceFailure(c) => (Kind::ForceFailure, "ForceFailure", vec![c]),
            BtNode::Repeat { count, child } => (Kind::Repeat(*count), "Repeat", vec![child]),
            BtNode::Cooldown { seconds, child } => {
                (Kind::Cooldown(*seconds), "Cooldown", vec![child])
            }
            BtNode::TimeLimit { seconds, child } => {
                (Kind::TimeLimit(*seconds), "TimeLimit", vec![child])
            }
            BtNode::Wait { seconds } => (Kind::Wait(*seconds), "Wait", vec![]),
            BtNode::SetValue { key, value } => (
                Kind::SetValue(key.clone(), value.clone()),
                "SetValue",
                vec![],
            ),
            BtNode::ClearValue { key } => (Kind::ClearValue(key.clone()), "ClearValue", vec![]),
            BtNode::MoveTo { key, acceptance } => {
                (Kind::MoveTo(key.clone(), *acceptance), "MoveTo", vec![])
            }
            BtNode::Succeed => (Kind::Succeed, "Succeed", vec![]),
            BtNode::Fail => (Kind::Fail, "Fail", vec![]),
        };
        let kid_indices: Vec<usize> = kids.into_iter().map(|k| self.compile(k)).collect();
        self.kinds[index] = kind;
        self.names[index] = name;
        self.children[index] = kid_indices;
        if let Kind::Parallel(_) = self.kinds[index] {
            self.state[index].finished = vec![None; self.children[index].len()];
        }
        index
    }

    /// Ticks the tree once from the root. A root that finishes starts over
    /// on the next tick, as Unreal's does.
    pub fn tick(&mut self, ctx: &mut dyn BtContext) -> BtStatus {
        let status = self.tick_node(0, ctx);
        if status != BtStatus::Running {
            self.reset(0, ctx);
        }
        status
    }

    /// Aborts everything that is running (a disabled or removed tree).
    pub fn abort(&mut self, ctx: &mut dyn BtContext) {
        self.reset(0, ctx);
    }

    /// The names of the nodes running now, root first -- the active branch,
    /// as Unreal's debugger highlights it.
    pub fn running_path(&self) -> Vec<&'static str> {
        let mut path = Vec::new();
        let mut node = 0;
        while self.state[node].running {
            path.push(self.names[node]);
            let Some(next) = self.children[node]
                .iter()
                .copied()
                .find(|c| self.state[*c].running)
            else {
                break;
            };
            node = next;
        }
        path
    }

    fn reset(&mut self, node: usize, ctx: &mut dyn BtContext) {
        if matches!(self.kinds[node], Kind::MoveTo(..)) && self.state[node].issued {
            ctx.stop_move();
        }
        let keep_since = matches!(self.kinds[node], Kind::Cooldown(_));
        let since = self.state[node].since;
        let len = self.state[node].finished.len();
        self.state[node] = NodeState {
            finished: vec![None; len],
            since: if keep_since { since } else { None },
            ..Default::default()
        };
        for child in self.children[node].clone() {
            self.reset(child, ctx);
        }
    }

    fn check(&self, key: &str, op: BbCompare, value: &Option<BbValue>, bb: &Blackboard) -> bool {
        let got = bb.get(key);
        match op {
            BbCompare::IsSet => got.is_some(),
            BbCompare::IsNotSet => got.is_none(),
            BbCompare::Equal | BbCompare::NotEqual => {
                let equal = match (got, value) {
                    (Some(a), Some(b)) => match (a.as_number(), b.as_number()) {
                        (Some(x), Some(y)) => x == y,
                        _ => a == b,
                    },
                    (None, None) => true,
                    _ => false,
                };
                equal == (op == BbCompare::Equal)
            }
            _ => {
                let (Some(x), Some(y)) = (
                    got.and_then(BbValue::as_number),
                    value.as_ref().and_then(BbValue::as_number),
                ) else {
                    return false;
                };
                match op {
                    BbCompare::Less => x < y,
                    BbCompare::LessOrEqual => x <= y,
                    BbCompare::Greater => x > y,
                    _ => x >= y,
                }
            }
        }
    }

    fn condition_holds(&self, node: usize, ctx: &mut dyn BtContext) -> bool {
        let Kind::Condition { key, op, value, .. } = &self.kinds[node] else {
            return true;
        };
        self.check(key, *op, value, ctx.blackboard())
    }

    fn tick_node(&mut self, node: usize, ctx: &mut dyn BtContext) -> BtStatus {
        let status = self.tick_kind(node, ctx);
        self.state[node].running = status == BtStatus::Running;
        status
    }

    fn tick_kind(&mut self, node: usize, ctx: &mut dyn BtContext) -> BtStatus {
        let kids = self.children[node].clone();
        match self.kinds[node].clone() {
            Kind::Sequence => {
                let mut c = self.state[node].current;
                while c < kids.len() {
                    match self.tick_node(kids[c], ctx) {
                        BtStatus::Running => {
                            self.state[node].current = c;
                            return BtStatus::Running;
                        }
                        BtStatus::Failure => {
                            self.state[node].current = 0;
                            return BtStatus::Failure;
                        }
                        BtStatus::Success => c += 1,
                    }
                }
                self.state[node].current = 0;
                BtStatus::Success
            }
            Kind::Selector => {
                let mut c = self.state[node].current;
                // Lower-priority aborts: a higher-priority Condition whose
                // test now holds takes over from the running child.
                if self.state[kids[c]].running {
                    for (j, &kid) in kids.iter().enumerate().take(c) {
                        let observes = matches!(
                            self.kinds[kid],
                            Kind::Condition {
                                abort: AbortMode::LowerPriority | AbortMode::Both,
                                ..
                            }
                        );
                        if observes && self.condition_holds(kid, ctx) {
                            self.reset(kids[c], ctx);
                            c = j;
                            break;
                        }
                    }
                }
                while c < kids.len() {
                    match self.tick_node(kids[c], ctx) {
                        BtStatus::Running => {
                            self.state[node].current = c;
                            return BtStatus::Running;
                        }
                        BtStatus::Success => {
                            self.state[node].current = 0;
                            return BtStatus::Success;
                        }
                        BtStatus::Failure => c += 1,
                    }
                }
                self.state[node].current = 0;
                BtStatus::Failure
            }
            Kind::Parallel(policy) => {
                for (i, &kid) in kids.iter().enumerate() {
                    if self.state[node].finished[i].is_none() {
                        let s = self.tick_node(kid, ctx);
                        if s != BtStatus::Running {
                            self.state[node].finished[i] = Some(s);
                        }
                    }
                }
                let done = &self.state[node].finished;
                let successes = done
                    .iter()
                    .filter(|s| **s == Some(BtStatus::Success))
                    .count();
                let failures = done
                    .iter()
                    .filter(|s| **s == Some(BtStatus::Failure))
                    .count();
                let result = match policy {
                    ParallelPolicy::All if failures > 0 => BtStatus::Failure,
                    ParallelPolicy::All if successes == kids.len() => BtStatus::Success,
                    ParallelPolicy::Any if successes > 0 => BtStatus::Success,
                    ParallelPolicy::Any if failures == kids.len() => BtStatus::Failure,
                    _ => BtStatus::Running,
                };
                if result != BtStatus::Running {
                    for &kid in &kids {
                        self.reset(kid, ctx);
                    }
                    let len = kids.len();
                    self.state[node].finished = vec![None; len];
                }
                result
            }
            Kind::Condition { abort, .. } => {
                let was_running = self.state[node].running;
                let holds = self.condition_holds(node, ctx);
                if was_running && !matches!(abort, AbortMode::Self_ | AbortMode::Both) {
                    // Not observing itself: once entered, runs to the end.
                    return self.tick_node(kids[0], ctx);
                }
                if !holds {
                    if was_running {
                        self.reset(kids[0], ctx);
                    }
                    return BtStatus::Failure;
                }
                self.tick_node(kids[0], ctx)
            }
            Kind::Inverter => match self.tick_node(kids[0], ctx) {
                BtStatus::Success => BtStatus::Failure,
                BtStatus::Failure => BtStatus::Success,
                BtStatus::Running => BtStatus::Running,
            },
            Kind::ForceSuccess => match self.tick_node(kids[0], ctx) {
                BtStatus::Running => BtStatus::Running,
                _ => BtStatus::Success,
            },
            Kind::ForceFailure => match self.tick_node(kids[0], ctx) {
                BtStatus::Running => BtStatus::Running,
                _ => BtStatus::Failure,
            },
            Kind::Repeat(count) => match self.tick_node(kids[0], ctx) {
                BtStatus::Running => BtStatus::Running,
                BtStatus::Failure => BtStatus::Failure,
                BtStatus::Success => {
                    self.reset(kids[0], ctx);
                    self.state[node].count += 1;
                    match count {
                        Some(n) if self.state[node].count >= n => BtStatus::Success,
                        _ => BtStatus::Running,
                    }
                }
            },
            Kind::Cooldown(seconds) => {
                let now = ctx.now();
                if !self.state[node].running {
                    if let Some(last) = self.state[node].since {
                        if !elapsed(now, last, seconds) {
                            return BtStatus::Failure;
                        }
                    }
                }
                let s = self.tick_node(kids[0], ctx);
                if s != BtStatus::Running {
                    self.state[node].since = Some(now);
                }
                s
            }
            Kind::TimeLimit(seconds) => {
                let now = ctx.now();
                let started = *self.state[node].since.get_or_insert(now);
                if elapsed(now, started, seconds) {
                    self.reset(kids[0], ctx);
                    self.state[node].since = None;
                    return BtStatus::Failure;
                }
                let s = self.tick_node(kids[0], ctx);
                if s != BtStatus::Running {
                    self.state[node].since = None;
                }
                s
            }
            Kind::Wait(seconds) => {
                let now = ctx.now();
                let started = *self.state[node].since.get_or_insert(now);
                if elapsed(now, started, seconds) {
                    self.state[node].since = None;
                    BtStatus::Success
                } else {
                    BtStatus::Running
                }
            }
            Kind::SetValue(key, value) => {
                ctx.blackboard().set(key, value);
                BtStatus::Success
            }
            Kind::ClearValue(key) => {
                ctx.blackboard().clear(&key);
                BtStatus::Success
            }
            Kind::MoveTo(key, acceptance) => {
                let target = match ctx.blackboard().get(&key) {
                    Some(BbValue::Vec3(v)) => MoveTarget::Position(v.0),
                    Some(BbValue::Entity(name)) => MoveTarget::Entity(name.clone()),
                    _ => {
                        self.stop_if_issued(node, ctx);
                        return BtStatus::Failure;
                    }
                };
                if !ctx.move_to(&target, acceptance) {
                    self.stop_if_issued(node, ctx);
                    return BtStatus::Failure;
                }
                if !self.state[node].issued {
                    self.state[node].issued = true;
                    return BtStatus::Running;
                }
                match ctx.move_state() {
                    MoveState::Moving => BtStatus::Running,
                    MoveState::Arrived => {
                        self.state[node].issued = false;
                        BtStatus::Success
                    }
                    MoveState::Failed => {
                        self.state[node].issued = false;
                        BtStatus::Failure
                    }
                }
            }
            Kind::Succeed => BtStatus::Success,
            Kind::Fail => BtStatus::Failure,
        }
    }

    fn stop_if_issued(&mut self, node: usize, ctx: &mut dyn BtContext) {
        if self.state[node].issued {
            ctx.stop_move();
            self.state[node].issued = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world with a clock, a blackboard and a pretend agent that arrives
    /// after `arrive_after` ticks of moving.
    #[derive(Default)]
    struct World {
        now: f64,
        bb: Blackboard,
        moving_to: Option<MoveTarget>,
        ticks_moving: u32,
        arrive_after: u32,
        no_path: bool,
        stops: u32,
        moves_issued: u32,
    }

    impl BtContext for World {
        fn now(&self) -> f64 {
            self.now
        }
        fn blackboard(&mut self) -> &mut Blackboard {
            &mut self.bb
        }
        fn move_to(&mut self, target: &MoveTarget, _acceptance: Option<f32>) -> bool {
            if self.moving_to.as_ref() != Some(target) {
                self.moves_issued += 1;
                self.ticks_moving = 0;
            }
            self.moving_to = Some(target.clone());
            true
        }
        fn move_state(&mut self) -> MoveState {
            self.ticks_moving += 1;
            if self.no_path {
                MoveState::Failed
            } else if self.ticks_moving >= self.arrive_after {
                MoveState::Arrived
            } else {
                MoveState::Moving
            }
        }
        fn stop_move(&mut self) {
            self.stops += 1;
            self.moving_to = None;
        }
    }

    fn rt(ron_text: &str) -> BtRuntime {
        let asset: BehaviorTreeAsset = ron::from_str(ron_text).expect("the tree parses");
        BtRuntime::new(&asset)
    }

    /// Ticks at 10 Hz, returning each tick's status.
    fn run(rt: &mut BtRuntime, w: &mut World, ticks: usize) -> Vec<BtStatus> {
        (0..ticks)
            .map(|_| {
                let s = rt.tick(w);
                w.now += 0.1;
                s
            })
            .collect()
    }

    use BtStatus::{Failure as F, Running as R, Success as S};

    #[test]
    fn a_sequence_runs_in_order_and_stops_at_the_first_failure() {
        let mut w = World::default();
        let mut t = rt(
            r#"(root: Sequence([SetValue(key: "a", value: Int(1)), Fail, SetValue(key: "b", value: Int(2))]))"#,
        );
        assert_eq!(run(&mut t, &mut w, 1), vec![F]);
        assert_eq!(w.bb.get("a"), Some(&BbValue::Int(1)), "ran the first child");
        assert_eq!(
            w.bb.get("b"),
            None,
            "and never reached the one after the failure"
        );
    }

    #[test]
    fn a_selector_takes_the_first_child_that_succeeds() {
        let mut w = World::default();
        let mut t = rt(
            r#"(root: Selector([Fail, SetValue(key: "picked", value: Str("second")), SetValue(key: "picked", value: Str("third"))]))"#,
        );
        assert_eq!(run(&mut t, &mut w, 1), vec![S]);
        assert_eq!(w.bb.get("picked"), Some(&BbValue::Str("second".into())));
    }

    /// A running child is resumed, not restarted: the Sequence does not
    /// re-run the SetValue before its Wait every tick. The key is cleared
    /// from outside after the first tick; a Sequence that restarted from
    /// its first child would set it again.
    #[test]
    fn a_running_child_is_resumed_on_the_next_tick() {
        let mut w = World::default();
        let mut t = rt(
            r#"(root: Sequence([SetValue(key: "n", value: Int(1)), Wait(seconds: 0.3), Succeed]))"#,
        );
        assert_eq!(run(&mut t, &mut w, 1), vec![R]);
        assert_eq!(
            w.bb.get("n"),
            Some(&BbValue::Int(1)),
            "premise: the first child ran"
        );
        w.bb.clear("n");
        assert_eq!(
            run(&mut t, &mut w, 4),
            vec![R, R, S, R],
            "the wait finishes on its fourth tick, then the root starts over"
        );
        assert_eq!(t.running_path(), vec!["Sequence", "Wait"]);
        assert_eq!(
            w.bb.get("n"),
            Some(&BbValue::Int(1)),
            "set again only by the restart after the root finished"
        );
        w.bb.clear("n");
        run(&mut t, &mut w, 2);
        assert_eq!(w.bb.get("n"), None, "and not while the Wait runs");
    }

    #[test]
    fn wait_counts_seconds() {
        let mut w = World::default();
        let mut t = rt(r#"(root: Wait(seconds: 0.25))"#);
        assert_eq!(run(&mut t, &mut w, 4), vec![R, R, R, S]);
    }

    /// Self abort: the guarded subtree is cut short the tick its condition
    /// turns false, and a MoveTo in it stops its agent.
    #[test]
    fn a_condition_observing_itself_aborts_its_running_subtree() {
        let mut w = World {
            arrive_after: 100,
            ..Default::default()
        };
        w.bb.set("target", BbValue::Vec3(glam::Vec3::X.into()));
        let mut t = rt(
            r#"(root: Condition(key: "target", op: IsSet, abort: Self_, child: MoveTo(key: "target")))"#,
        );
        assert_eq!(run(&mut t, &mut w, 3), vec![R, R, R]);
        assert!(w.moving_to.is_some(), "premise: it is moving");
        w.bb.clear("target");
        assert_eq!(
            run(&mut t, &mut w, 1),
            vec![F],
            "aborted the tick the key went"
        );
        assert_eq!(w.stops, 1, "and stopped the agent");
    }

    /// Without an abort mode the same subtree runs on after its condition
    /// stops holding -- the condition is checked on entry only.
    #[test]
    fn a_condition_without_aborts_is_checked_on_entry_only() {
        let mut w = World {
            arrive_after: 100,
            ..Default::default()
        };
        w.bb.set("target", BbValue::Vec3(glam::Vec3::X.into()));
        let mut t =
            rt(r#"(root: Condition(key: "target", op: IsSet, child: Wait(seconds: 10.0)))"#);
        run(&mut t, &mut w, 2);
        w.bb.clear("target");
        assert_eq!(run(&mut t, &mut w, 1), vec![R]);
    }

    /// Lower-priority abort: the patrol (lower priority) is running when an
    /// enemy is spotted; the chase branch takes over that tick, the patrol's
    /// move is stopped, and the new move is issued.
    #[test]
    fn a_higher_priority_condition_interrupts_a_lower_priority_branch() {
        let mut w = World {
            arrive_after: 100,
            ..Default::default()
        };
        w.bb.set("waypoint", BbValue::Vec3(glam::Vec3::Z.into()));
        let mut t = rt(r#"(root: Selector([
            Condition(key: "enemy", op: IsSet, abort: LowerPriority, child: MoveTo(key: "enemy")),
            MoveTo(key: "waypoint"),
        ]))"#);
        run(&mut t, &mut w, 3);
        assert_eq!(
            w.moving_to,
            Some(MoveTarget::Position(glam::Vec3::Z)),
            "premise: patrolling"
        );
        w.bb.set("enemy", BbValue::Entity("Player".into()));
        run(&mut t, &mut w, 1);
        assert_eq!(
            w.moving_to,
            Some(MoveTarget::Entity("Player".into())),
            "chasing now"
        );
        assert_eq!(w.stops, 1, "the patrol's move was stopped");
    }

    /// And the check is only made against lower-priority work: a
    /// LowerPriority condition does not abort its own running subtree when
    /// it stops holding (that is `Self_`).
    #[test]
    fn a_lower_priority_abort_does_not_cut_its_own_branch() {
        let mut w = World {
            arrive_after: 100,
            ..Default::default()
        };
        w.bb.set("enemy", BbValue::Entity("Player".into()));
        let mut t = rt(r#"(root: Selector([
            Condition(key: "enemy", op: IsSet, abort: LowerPriority, child: Wait(seconds: 10.0)),
            Succeed,
        ]))"#);
        run(&mut t, &mut w, 2);
        w.bb.clear("enemy");
        assert_eq!(run(&mut t, &mut w, 1), vec![R]);
    }

    #[test]
    fn comparisons_read_numbers_across_int_and_float() {
        let mut w = World::default();
        w.bb.set("hp", BbValue::Int(30));
        let t = rt(r#"(root: Succeed)"#);
        let cmp = |op, v| t.check("hp", op, &Some(v), &w.bb);
        assert!(cmp(BbCompare::Less, BbValue::Float(30.5)));
        assert!(!cmp(BbCompare::Greater, BbValue::Int(30)));
        assert!(cmp(BbCompare::GreaterOrEqual, BbValue::Float(30.0)));
        assert!(cmp(BbCompare::Equal, BbValue::Float(30.0)), "30 == 30.0");
        assert!(cmp(BbCompare::NotEqual, BbValue::Str("30".into())));
        assert!(
            !t.check("mana", BbCompare::Less, &Some(BbValue::Int(1)), &w.bb),
            "unset is not a number"
        );
        w.bb.set("name", BbValue::Str("orc".into()));
        assert!(t.check(
            "name",
            BbCompare::Equal,
            &Some(BbValue::Str("orc".into())),
            &w.bb
        ));
        let _ = &mut w;
    }

    #[test]
    fn parallel_policies() {
        let mut w = World::default();
        let mut all = rt(
            r#"(root: Parallel(policy: All, children: [Wait(seconds: 0.1), Wait(seconds: 0.3)]))"#,
        );
        assert_eq!(
            run(&mut all, &mut w, 4),
            vec![R, R, R, S],
            "All waits for the longer"
        );
        let mut any = rt(
            r#"(root: Parallel(policy: Any, children: [Wait(seconds: 0.1), Wait(seconds: 0.3)]))"#,
        );
        assert_eq!(
            run(&mut any, &mut w, 2),
            vec![R, S],
            "Any finishes with the shorter"
        );
        let mut fails = rt(r#"(root: Parallel(children: [Fail, Wait(seconds: 1.0)]))"#);
        assert_eq!(
            run(&mut fails, &mut w, 1),
            vec![F],
            "All fails on the first failure"
        );
    }

    #[test]
    fn decorators_invert_force_and_repeat() {
        let mut w = World::default();
        assert_eq!(run(&mut rt("(root: Inverter(Fail))"), &mut w, 1), vec![S]);
        assert_eq!(
            run(&mut rt("(root: Inverter(Succeed))"), &mut w, 1),
            vec![F]
        );
        assert_eq!(
            run(&mut rt("(root: ForceSuccess(Fail))"), &mut w, 1),
            vec![S]
        );
        assert_eq!(
            run(&mut rt("(root: ForceFailure(Succeed))"), &mut w, 1),
            vec![F]
        );
        assert_eq!(
            run(
                &mut rt("(root: Repeat(count: Some(3), child: Succeed))"),
                &mut w,
                3
            ),
            vec![R, R, S],
            "three passes, one a tick"
        );
        assert_eq!(
            run(&mut rt("(root: Repeat(child: Fail))"), &mut w, 1),
            vec![F]
        );
        assert_eq!(
            run(&mut rt("(root: Repeat(child: Succeed))"), &mut w, 5),
            vec![R; 5],
            "forever without a count"
        );
    }

    /// Cooldown: after its child finishes it refuses to run it for the
    /// cooldown, then allows it again.
    #[test]
    fn cooldown_refuses_until_its_time_is_up() {
        let mut w = World::default();
        let mut t = rt(r#"(root: Cooldown(seconds: 0.25, child: Succeed))"#);
        assert_eq!(run(&mut t, &mut w, 5), vec![S, F, F, S, F]);
    }

    #[test]
    fn a_time_limit_aborts_a_child_that_runs_too_long() {
        let mut w = World {
            arrive_after: 100,
            ..Default::default()
        };
        w.bb.set("goal", BbValue::Vec3(glam::Vec3::X.into()));
        let mut t = rt(r#"(root: TimeLimit(seconds: 0.25, child: MoveTo(key: "goal")))"#);
        assert_eq!(run(&mut t, &mut w, 4), vec![R, R, R, F]);
        assert_eq!(w.stops, 1, "the move was stopped");
    }

    /// MoveTo issues the move, waits a tick before reading the agent (whose
    /// state still belongs to the last move), and succeeds on arrival; no
    /// target, or no path, fails.
    #[test]
    fn move_to_walks_arrives_and_fails_without_a_target_or_path() {
        let mut w = World {
            arrive_after: 2,
            ..Default::default()
        };
        w.bb.set("goal", BbValue::Vec3(glam::Vec3::X.into()));
        let mut t = rt(r#"(root: MoveTo(key: "goal"))"#);
        assert_eq!(run(&mut t, &mut w, 3), vec![R, R, S]);
        assert_eq!(w.moves_issued, 1);

        w.bb.clear("goal");
        assert_eq!(run(&mut t, &mut w, 1), vec![F], "no target");

        w.bb.set("goal", BbValue::Vec3(glam::Vec3::Y.into()));
        w.no_path = true;
        assert_eq!(run(&mut t, &mut w, 2), vec![R, F], "no path");
    }

    #[test]
    fn an_asset_round_trips_through_ron() {
        let text = r#"(root: Selector([
            Condition(key: "enemy", op: IsSet, abort: Both, child: Sequence([MoveTo(key: "enemy", acceptance: Some(1.5)), Wait(seconds: 0.5)])),
            Cooldown(seconds: 2.0, child: SetValue(key: "bored", value: Bool(true))),
        ]))"#;
        let asset: BehaviorTreeAsset = ron::from_str(text).unwrap();
        let back: BehaviorTreeAsset = ron::from_str(&ron::to_string(&asset).unwrap()).unwrap();
        assert_eq!(asset, back);
    }
}

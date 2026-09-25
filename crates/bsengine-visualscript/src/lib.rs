//! Node-graph script authoring: a graph in, a JavaScript file out.
//!
//! A script is authored as a `.scriptgraph.ron` file -- the same RON
//! convention the engine uses for scenes, prefabs and shader graphs -- and
//! compiled to a `.js` file that a scene's `script:` names exactly as it would
//! a hand-written one. Nothing downstream changes: loading, hot reload, the
//! packager's walk, `api.d.ts` and every MCP tool that reads or writes a
//! script keep working unmodified, because the generated file *is* a script.
//! The graph route is an addition to the text path, not a replacement for it,
//! which is the decision `bsengine-shadergraph` made for WGSL and for the same
//! reason: the text is what an AI agent reads and edits, so it stays primary.
//!
//! # The model the reference engines converge on
//!
//! Unreal's Blueprint, Unity's Visual Scripting and Godot 3's VisualScript
//! (removed in Godot 4) all draw the same graph: **flow** ports that decide
//! what runs and in what order, **data** ports that carry values, **event**
//! nodes where a flow starts, `Branch` and `Sequence` for control, variables,
//! and nodes that call into the engine's API. That is the node set here.
//! Where they diverge is execution -- Unreal compiles to bytecode, Unity
//! interprets -- and this crate compiles, to the one language the runtime
//! already speaks.
//!
//! # Why the events are dispatched from `onUpdate`
//!
//! The scripting runtime calls exactly one function per entity per frame,
//! `onUpdate(self)`, and `self` -- the entity's name -- exists only inside it.
//! Every event this crate offers is therefore expressed from there: `OnStart`
//! is the first frame (a guard, as Unity's `Start` precedes the first
//! `Update`), `OnKeyPressed` is a per-frame `isKeyPressed` check, and
//! `OnCollision` registers its callback on that first frame, once `self` is
//! known. One entry point, one place the generated code can be read from top
//! to bottom.
//!
//! The crate depends on no ECS, GPU or UI types, so it is a leaf both the
//! editor and asset tooling can use, and compilation is a pure function unit
//! tests drive without a V8 isolate. Whether the emitted JavaScript *runs* is
//! `bsengine-scripting`'s test to make, and it makes it.

#![deny(missing_docs)]

pub mod compile;
pub mod graph;
pub mod ops;

pub use compile::{compile, Port, PortType, ValueType};
pub use graph::{CompareOp, Edge, GraphError, GraphNode, NodeKind, ScriptGraph, Value, Variable};
pub use ops::{op, OpSpec, OPS};

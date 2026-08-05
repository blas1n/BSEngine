//! A static catalogue of the engine's components and scripting ops.
//!
//! Answers "does this concept already exist, and who owns it" before a new
//! component or op gets written. Parsed from source rather than read from a
//! running app's type registry, because the registry only sees registered
//! types and the question is asked before the code exists.

#![deny(missing_docs)]

pub mod parse;

pub use parse::{Component, Field, Op};

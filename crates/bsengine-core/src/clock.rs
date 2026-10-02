//! Wall-clock time that works in every build.
//!
//! `std::time::Instant::now()` and `SystemTime::now()` panic in a browser
//! build ("time not implemented on this platform"): wasm32-unknown-unknown has
//! no clock of its own, only the page's `performance.now()` and `Date.now()`.
//! These are `web-time`'s, which read those there and are the `std` types
//! themselves everywhere else -- so a native build is unchanged, and code
//! that runs in both builds asks for the time through here.

pub use web_time::{Instant, SystemTime, UNIX_EPOCH};

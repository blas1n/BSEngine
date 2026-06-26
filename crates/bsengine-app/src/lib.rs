pub mod app;
pub mod damping;
pub mod lifetime;
pub mod time;
pub mod tween;
pub mod velocity;

pub use app::{new_app, App, BsPlugin, Last, PostUpdate, PreUpdate, Startup, Update};
pub use damping::DampingPlugin;
pub use lifetime::LifetimePlugin;
pub use time::TimePlugin;
pub use tween::TweenPlugin;
pub use velocity::VelocityPlugin;

mod tests;

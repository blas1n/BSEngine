use std::collections::HashMap;

use bevy_ecs::prelude::Resource;

/// Resource mapping named HUD text slots (e.g. "score", "health") to their
/// current display strings, read by the HUD rendering system.
#[derive(Resource, Default, Clone)]
pub struct HudTexts(pub HashMap<String, String>);

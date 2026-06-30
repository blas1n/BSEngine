use std::collections::HashMap;

use bevy_ecs::prelude::Resource;

#[derive(Resource, Default, Clone)]
pub struct HudTexts(pub HashMap<String, String>);

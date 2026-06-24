use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone, Default)]
pub struct Material {
    pub texture_id: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_default_has_no_texture() {
        let m = Material::default();
        assert!(m.texture_id.is_none());
    }
}

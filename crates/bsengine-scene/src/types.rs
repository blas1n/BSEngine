use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDescriptor {
    pub entities: Vec<EntityDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDescriptor {
    pub name: String,
    pub components: Vec<(String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_descriptor_deserializes_from_ron() {
        let ron_str = r#"
            SceneDescriptor(
                entities: [
                    EntityDescriptor(
                        name: "Player",
                        components: [],
                    ),
                    EntityDescriptor(
                        name: "Camera",
                        components: [],
                    ),
                ],
            )
        "#;
        let scene: SceneDescriptor = ron::from_str(ron_str).expect("Failed to parse RON");
        assert_eq!(scene.entities.len(), 2);
        assert_eq!(scene.entities[0].name, "Player");
        assert_eq!(scene.entities[1].name, "Camera");
    }

    #[test]
    fn entity_descriptor_has_components() {
        let ron_str = r#"
            EntityDescriptor(
                name: "Enemy",
                components: [
                    ("Transform", "{\"x\": 0.0}"),
                    ("Health", "{\"max_hp\": 50}"),
                ],
            )
        "#;
        let entity: EntityDescriptor = ron::from_str(ron_str).expect("Failed to parse RON");
        assert_eq!(entity.name, "Enemy");
        assert_eq!(entity.components.len(), 2);
        assert_eq!(entity.components[0].0, "Transform");
    }
}

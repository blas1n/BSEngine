use bevy_app::{App, Plugin, Startup};
use bevy_ecs::prelude::{Commands, Component};
use crate::types::SceneDescriptor;

#[derive(Component, Debug, Clone)]
pub struct Name(pub String);

pub struct ScenePlugin {
    path: String,
}

impl ScenePlugin {
    pub fn from_file(path: &str) -> Self {
        Self { path: path.to_string() }
    }
}

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        let path = self.path.clone();
        app.add_systems(Startup, move |mut commands: Commands| {
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read scene {path}: {e}"));
            let scene: SceneDescriptor = ron::from_str(&content)
                .unwrap_or_else(|e| panic!("Failed to parse scene {path}: {e}"));
            for entity in &scene.entities {
                commands.spawn(Name(entity.name.clone()));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use bsengine_app::new_app;
    use super::{Name, ScenePlugin};

    fn write_temp_scene(filename: &str, content: &str) -> String {
        let path = std::env::temp_dir().join(filename);
        std::fs::write(&path, content).unwrap();
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn scene_plugin_spawns_entities() {
        let ron = r#"SceneDescriptor(entities: [EntityDescriptor(name: "Player", components: []), EntityDescriptor(name: "Camera", components: [])])"#;
        let path = write_temp_scene("test_spawn.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app.world_mut().query::<&Name>();
        let names: Vec<String> = q.iter(app.world()).map(|n| n.0.clone()).collect();
        assert!(names.contains(&"Player".to_string()), "Player missing: {:?}", names);
        assert!(names.contains(&"Camera".to_string()), "Camera missing: {:?}", names);
    }

    #[test]
    fn scene_plugin_empty_scene() {
        let ron = r#"SceneDescriptor(entities: [])"#;
        let path = write_temp_scene("test_empty.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app.world_mut().query::<&Name>();
        assert_eq!(q.iter(app.world()).count(), 0);
    }
}

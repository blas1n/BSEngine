use bsengine_app::new_app;
use bsengine_asset::{AssetPlugin, AssetServerResource};
use bsengine_scene::{Name, ScenePlugin};

fn write_temp_scene(filename: &str, content: &str) -> String {
    let path = std::env::temp_dir().join(filename);
    std::fs::write(&path, content).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn scene_and_asset_plugins_work_together() {
    let ron = r#"SceneDescriptor(entities: [EntityDescriptor(name: "Hero", components: []), EntityDescriptor(name: "Villain", components: [])])"#;
    let path = write_temp_scene("integration_scene.ron", ron);

    let mut app = new_app();
    app.add_plugins(AssetPlugin)
       .add_plugins(ScenePlugin::from_file(&path));
    app.update();

    assert!(app.world().get_resource::<AssetServerResource>().is_some());

    let mut q = app.world_mut().query::<&Name>();
    let names: Vec<String> = q.iter(app.world()).map(|n| n.0.clone()).collect();
    assert!(names.contains(&"Hero".to_string()), "Hero missing: {:?}", names);
    assert!(names.contains(&"Villain".to_string()), "Villain missing: {:?}", names);
}

#[test]
fn asset_server_loads_bytes_from_app() {
    let path = std::env::temp_dir().join("test_bytes_integ.bin");
    std::fs::write(&path, b"binary content").unwrap();

    let mut app = new_app();
    app.add_plugins(AssetPlugin);
    app.update();

    let server = &app.world().resource::<AssetServerResource>().0;
    let bytes = server.load_bytes(path.to_str().unwrap()).unwrap();
    assert_eq!(bytes, b"binary content");
}
